//! HMAC-SHA256 based authentication for NTP requests/responses, following the symmetric-key
//! MAC approach from [RFC 5905, Appendix A](https://tools.ietf.org/html/rfc5905#appendix-A)
//! (originally RFC 1305) with SHA-256 substituted for the historical MD5. See
//! [docs/ntp-response-authentication.md](../docs/ntp-response-authentication.md) for the design
//! rationale. Opt-in only, via `--auth-key-file` — without it, packets are neither signed nor
//! trailer-checked, matching every other flag added so far.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::convert::TryInto;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::Path;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Length of the HMAC-SHA256 digest, in bytes.
pub const MAC_LEN : usize = 32;

/// Length of the Key ID field, in bytes.
pub const KEY_ID_LEN : usize = 4;

/// A symmetric key used to authenticate NTP packets, identified by `id`
/// ([RFC 5905, Appendix A](https://tools.ietf.org/html/rfc5905#appendix-A)).
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    /// Key ID, sent alongside the digest so a server (or client) can pick the right secret.
    pub id : u32,
    /// The shared secret itself.
    pub secret : Vec<u8>,
}

/// Computes the HMAC-SHA256 tag over a 48-byte NTP packet.
pub fn compute_mac(secret : &[u8], packet_bytes : &[u8; 48]) -> [u8; MAC_LEN] {
    let mut mac : HmacSha256 = HmacSha256::new_from_slice(secret).expect("HMAC accepts a key of any length");
    mac.update(packet_bytes);

    let mut digest : [u8; MAC_LEN] = [0; MAC_LEN];
    digest.copy_from_slice(&mac.finalize().into_bytes());
    digest
}

/// Verifies `received` against a freshly computed tag, in constant time.
pub fn verify_mac(secret : &[u8], packet_bytes : &[u8; 48], received : &[u8]) -> bool {
    if received.len() != MAC_LEN {
        return false;
    }

    let expected : [u8; MAC_LEN] = compute_mac(secret, packet_bytes);
    expected.ct_eq(received).into()
}

/// Appends the `[ Key ID (4 bytes) | HMAC-SHA256 digest (32 bytes) ]` trailer to a 48-byte
/// packet, keyed by `key`.
pub fn append_trailer(packet_bytes : &[u8; 48], key : &Key) -> Vec<u8> {
    let mut out : Vec<u8> = Vec::with_capacity(48 + KEY_ID_LEN + MAC_LEN);
    out.extend_from_slice(packet_bytes);
    out.extend_from_slice(&key.id.to_be_bytes());
    out.extend_from_slice(&compute_mac(&key.secret, packet_bytes));
    out
}

/// Verifies a received trailer against `keys`, looking up the key by the ID carried in the
/// trailer itself. Returns an error describing why verification failed — missing/short
/// trailer, unknown key ID, or a bad digest — rather than a bare `bool`, so callers can
/// propagate a meaningful message through the existing `Result`-based error handling.
pub fn verify_trailer(packet_bytes : &[u8; 48], trailer : &[u8], keys : &[Key]) -> Result<(), Error> {
    if trailer.len() != KEY_ID_LEN + MAC_LEN {
        return Err(Error::new(ErrorKind::InvalidData, "response has no authentication trailer"));
    }

    let key_id : u32 = u32::from_be_bytes(trailer[..KEY_ID_LEN].try_into().expect("slice is exactly 4 bytes"));
    let digest : &[u8] = &trailer[KEY_ID_LEN..];

    let key : &Key = find_key(keys, key_id)
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, format!("response uses unknown key ID {}", key_id)))?;

    if verify_mac(&key.secret, packet_bytes, digest) {
        Ok(())
    } else {
        Err(Error::new(ErrorKind::InvalidData, "response failed HMAC verification"))
    }
}

/// Finds the key with the given ID.
pub fn find_key(keys : &[Key], id : u32) -> Option<&Key> {
    keys.iter().find(|key| key.id == id)
}

/// Loads keys from a plain-text `KEYID SECRET` file, one pair per line (blank lines and `#`
/// comments ignored) — loosely modeled on the classic `ntp.keys` format used by `ntpd`/`chrony`.
/// The first key in the file is used to sign outgoing requests; every key is available to
/// verify responses, so a rotation can list the outgoing and an incoming key side by side.
pub fn load_keys(path : &Path) -> Result<Vec<Key>, Error> {
    let content : String = fs::read_to_string(path)?;
    let mut keys : Vec<Key> = Vec::new();

    for (line_no, raw_line) in content.lines().enumerate() {
        let line : &str = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let id_str : &str = parts.next().expect("split always yields at least one part");
        let secret_str : &str = parts.next().unwrap_or("").trim();

        if secret_str.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("{}:{}: expected 'KEYID SECRET'", path.display(), line_no + 1),
            ));
        }

        let id : u32 = id_str.parse().map_err(|_| {
            Error::new(
                ErrorKind::InvalidData,
                format!("{}:{}: '{}' is not a valid key ID", path.display(), line_no + 1, id_str),
            )
        })?;

        keys.push(Key { id, secret : secret_str.as_bytes().to_vec() });
    }

    if keys.is_empty() {
        return Err(Error::new(ErrorKind::InvalidData, format!("{}: no keys found", path.display())));
    }

    Ok(keys)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A unique path under the OS temp dir, so parallel test runs don't collide.
    fn temp_path(name : &str) -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("cocom-auth-test-{}-{}", name, nanos))
    }

    fn packet(fill : u8) -> [u8; 48] {
        [fill; 48]
    }

    #[test]
    fn test_compute_mac_is_deterministic() {
        let a = compute_mac(b"secret", &packet(1));
        let b = compute_mac(b"secret", &packet(1));
        assert_eq!(a, b);
    }

    #[test]
    fn test_compute_mac_differs_by_secret() {
        let a = compute_mac(b"secret-a", &packet(1));
        let b = compute_mac(b"secret-b", &packet(1));
        assert_ne!(a, b);
    }

    #[test]
    fn test_compute_mac_differs_by_packet() {
        let a = compute_mac(b"secret", &packet(1));
        let b = compute_mac(b"secret", &packet(2));
        assert_ne!(a, b);
    }

    #[test]
    fn test_verify_mac_accepts_matching_tag() {
        let tag = compute_mac(b"secret", &packet(7));
        assert!(verify_mac(b"secret", &packet(7), &tag));
    }

    #[test]
    fn test_verify_mac_rejects_wrong_secret() {
        let tag = compute_mac(b"secret", &packet(7));
        assert!(!verify_mac(b"other-secret", &packet(7), &tag));
    }

    #[test]
    fn test_verify_mac_rejects_tampered_packet() {
        let tag = compute_mac(b"secret", &packet(7));
        assert!(!verify_mac(b"secret", &packet(8), &tag));
    }

    #[test]
    fn test_verify_mac_rejects_wrong_length() {
        assert!(!verify_mac(b"secret", &packet(7), &[0; 16]));
    }

    #[test]
    fn test_append_then_verify_trailer_roundtrips() {
        let key = Key { id : 1, secret : b"secret".to_vec() };
        let packet_bytes = packet(3);

        let with_trailer = append_trailer(&packet_bytes, &key);
        assert_eq!(with_trailer.len(), 48 + KEY_ID_LEN + MAC_LEN);

        let trailer = &with_trailer[48..];
        assert!(verify_trailer(&packet_bytes, trailer, &[key]).is_ok());
    }

    #[test]
    fn test_verify_trailer_rejects_unknown_key_id() {
        let signing_key = Key { id : 1, secret : b"secret".to_vec() };
        let trust_key = Key { id : 2, secret : b"secret".to_vec() };
        let packet_bytes = packet(3);

        let with_trailer = append_trailer(&packet_bytes, &signing_key);
        let trailer = &with_trailer[48..];

        assert!(verify_trailer(&packet_bytes, trailer, &[trust_key]).is_err());
    }

    #[test]
    fn test_verify_trailer_rejects_short_trailer() {
        assert!(verify_trailer(&packet(3), &[0; 10], &[]).is_err());
    }

    #[test]
    fn test_find_key_matches_by_id() {
        let keys = vec![
            Key { id : 1, secret : b"a".to_vec() },
            Key { id : 2, secret : b"b".to_vec() },
        ];

        assert_eq!(find_key(&keys, 2).unwrap().secret, b"b");
        assert!(find_key(&keys, 3).is_none());
    }

    #[test]
    fn test_load_keys_parses_multiple_lines_with_comments() {
        let path = temp_path("valid");
        fs::write(&path, "# trust set\n1 first-secret\n\n2 second-secret\n").unwrap();

        let keys = load_keys(&path).unwrap();

        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0], Key { id : 1, secret : b"first-secret".to_vec() });
        assert_eq!(keys[1], Key { id : 2, secret : b"second-secret".to_vec() });
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_keys_missing_file_errors() {
        let path = temp_path("missing");
        assert!(load_keys(&path).is_err());
    }

    #[test]
    fn test_load_keys_rejects_malformed_line() {
        let path = temp_path("malformed");
        fs::write(&path, "not-a-key-line\n").unwrap();

        assert!(load_keys(&path).is_err());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_keys_rejects_empty_file() {
        let path = temp_path("empty");
        fs::write(&path, "# only a comment\n").unwrap();

        assert!(load_keys(&path).is_err());
        fs::remove_file(&path).ok();
    }
}
