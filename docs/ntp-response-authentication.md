# NTP Response Authentication

**Status: implemented, via `--auth-key-file`.** See the [README Roadmap](../README.md#roadmap) for
how this fits alongside multi-server comparison as the remaining open gap. This document captures
the design and the reasoning behind it, following the same approach as
[sync-and-clock-correction.md](sync-and-clock-correction.md).

## Table of Contents

- [Why this matters](#why-this-matters)
- [Two approaches, and why this document picks one](#two-approaches-and-why-this-document-picks-one)
- [Why not the PTB default server](#why-not-the-ptb-default-server)
- [Why HMAC](#why-hmac)
- [Wire format](#wire-format)
- [Design](#design)
- [Key management](#key-management)
- [Replay protection](#replay-protection)
- [Fail-closed behavior](#fail-closed-behavior)
- [What this is *not*](#what-this-is-not)

## Why this matters

Cocom currently trusts a queried server's response completely. There is a sanity/panic
threshold (`clock::PANIC_THRESHOLD_NANOS`, see
[sync-and-clock-correction.md](sync-and-clock-correction.md#system-clock-correction---apply))
that bounds *how far* a correction can go, but nothing that verifies the response actually came
from the expected server and wasn't tampered with or spoofed in transit. Combined with `--apply`
actually mutating the system clock, an on-path attacker (or a misbehaving server) can steer the
clock anywhere within that bound today.

## Two approaches, and why this document picks one

**Symmetric-key MAC** ([RFC 5905, Appendix A](https://tools.ietf.org/html/rfc5905#appendix-A),
originally RFC 1305) — client and server share a pre-distributed secret per Key ID. The response
packet is appended with a Message Authentication Code computed over it; the client recomputes
the MAC with its own copy of the secret and compares. Relatively small implementation surface:
no new transport, just an authentication tag appended to the existing 48-byte packet.

**NTS — Network Time Security** ([RFC 8915](https://www.rfc-editor.org/rfc/rfc8915)) — a TLS 1.3
handshake (NTS Key Establishment, TCP port 4460) derives session key material and hands the
client encrypted "cookies", which are then presented back to the server on each subsequent NTP
request over UDP, authenticated via an AEAD extension field. Designed specifically so a client
can authenticate a server it has no prior relationship with, without any manual key exchange.

This document picks **symmetric-key MAC** as the first step. NTS would require a TLS 1.3 client,
AEAD crypto, cookie state management, and a whole additional NTP extension-field wire format that
Cocom's `ntp.rs` doesn't parse today — realistically more new code than everything built so far
combined, and it pulls a TLS stack into a project whose whole pitch is a small, auditable,
single-binary, minimal-dependency client. NTS is left as a separate, larger future item.

## Why not the PTB default server

Cocom's own default server (`DEFAULT_NTP_HOST_PTB_BRSCHW`, PTB Braunschweig) is a real example of
exactly the tradeoff above. PTB used to run a symmetric-key "Authenticated Time Synchronization"
service, but keys had to be individually applied for (`ntp-admin@ptb.de`) — never anonymous or
public. That service was shut down at the end of 2025 and replaced entirely by NTS, which PTB
now offers free and **without registration**, specifically because it doesn't have the
key-distribution bottleneck symmetric-key has at public scale.

This means: symmetric-key auth does **not** work against Cocom's own default server — only against
a self-hosted server where you control both ends and can distribute the key yourself (matching the
README's already-documented "small, trusted network" use case; see
[When to Use Cocom](../README.md#when-to-use-cocom)). Sources: [PTB — Zeitsynchronisation von
Rechnern mit Hilfe des NTP](https://www.ptb.de/cms/ptb/fachabteilungen/abt9/gruppe-95/ref-952/zeitsynchronisation-von-rechnern-mit-hilfe-des-network-time-protocol-ntp.html),
[Kuketz IT-Security Forum: PTB replaces Authenticated Time Synchronization with NTS](https://www.kuketz-forum.de/t/ptb-braunschweig-ersetzt-authentifizierte-zeitsynchronisation-durch-nts-network-time-security-protocol/16347).

## Why HMAC

A plain hash over the packet (`SHA256(message)`) only proves the message wasn't corrupted, not
who produced it — anyone can compute a hash. A naive keyed variant (`SHA256(secret || message)`)
looks like it should fix that, but is vulnerable to length-extension attacks against
Merkle–Damgård hash functions (SHA-256, MD5, and SHA-1 all included): an attacker can extend a
valid message and its hash into a *different* valid message/hash pair without ever knowing the
secret.

HMAC ([RFC 2104](https://www.rfc-editor.org/rfc/rfc2104)) is the standard, well-analyzed
construction that specifically closes this gap, which is why it's the right choice here instead
of a hand-rolled keyed hash — no need to invent or reason about a custom MAC scheme, and it's
available via small, well-vetted, pure-Rust crates (RustCrypto's `hmac` + `sha2`, no OpenSSL
dependency, consistent with how `libc` was the minimal necessary addition for `clock.rs`'s
syscalls rather than something larger). The historical NTP spec defaults to HMAC-**MD5**; this
design uses HMAC-**SHA256** instead, since MD5 is now considered weak — swapping the underlying
hash function is a trivial substitution within the same HMAC construction, nothing else changes.

## Wire format

RFC 5905, Appendix A extends the base 48-byte packet with an optional trailer:

```
[ 48 bytes: standard NTP packet ] [ 4 bytes: Key ID ] [ 32 bytes: HMAC-SHA256 digest ]
```

- **Key ID** (4 bytes, big-endian `u32`) — which key to use; allows multiple keys / rotation
  without changing the protocol.
- **Digest** (32 bytes for HMAC-SHA256, vs. 16 for the historical MD5) — computed over the
  preceding 48 bytes using the secret associated with the Key ID.

Request and response are each authenticated independently: the client appends its own MAC (over
the outgoing 48 bytes) when sending, and the server appends its own MAC (over the 48-byte
response) when replying. Both sides use the same shared secret for a given Key ID.

## Design

`src/auth.rs` mirrors the existing pattern of pure, unit-tested logic decoupled from I/O
(`offset.rs`, `drift.rs`, `clock.rs`):

```rust
pub struct Key {
    pub id: u32,
    pub secret: Vec<u8>,
}

/// Computes the HMAC-SHA256 tag over a 48-byte NTP packet.
pub fn compute_mac(secret: &[u8], packet_bytes: &[u8; 48]) -> [u8; 32];

/// Verifies `received` against a freshly computed tag, in constant time.
pub fn verify_mac(secret: &[u8], packet_bytes: &[u8; 48], received: &[u8]) -> bool;

/// Appends the `[ Key ID | digest ]` trailer to a 48-byte packet, keyed by `key`.
pub fn append_trailer(packet_bytes: &[u8; 48], key: &Key) -> Vec<u8>;

/// Verifies a received trailer against a set of trusted keys, looked up by the Key ID it carries.
pub fn verify_trailer(packet_bytes: &[u8; 48], trailer: &[u8], keys: &[Key]) -> Result<(), Error>;
```

`compute_mac`/`verify_mac` use the RustCrypto project's `hmac` + `sha2` crates (pure Rust, no
OpenSSL/system dependency — consistent with how `libc` was the minimal necessary addition for
`clock.rs`'s syscalls, rather than pulling in something larger). `verify_mac` compares the
received vs. computed tag via the `subtle` crate's constant-time equality check.

`ntp.rs`'s 48-byte (de)serialization (`NTP::as_vec_u8`/`NTP::as_ntp`) is untouched — the trailer
lives entirely in `auth.rs` instead: `Client::request` calls `as_vec_u8`, then, if a key is
configured, hands the resulting 48 bytes to `append_trailer` before sending. `Client::receive`
always parses the first 48 bytes as a normal `NTP` packet; if a key is configured, it additionally
hands those same 48 bytes plus whatever follows in the socket buffer to `verify_trailer`. This
keeps the base wire format's own tests untouched and confines every auth-specific code path to one
module. The key file's first entry signs outgoing requests; every loaded key is checked against a
response's Key ID, so a rotation can list an outgoing and an about-to-expire incoming key side by
side.

## Key management

No secret as a CLI argument — it would be visible in `ps`/shell history, the same reasoning that
already led `--state-file` to be a file path rather than inline data. `--auth-key-file <PATH>`
points to a small plain-text file, one `KEYID SECRET` pair per line (blank lines and `#` comments
ignored) — loosely modeled on the classic `ntp.keys` file format used by `ntpd`/`chrony`, so a key
file could plausibly be shared with an existing self-hosted NTP daemon rather than needing a
Cocom-specific format.

## Replay protection

A MAC alone doesn't stop an old, legitimately-authenticated response from being captured and
replayed later. Cocom checks the response's echoed **Originate Timestamp** against what it
actually sent as T1 — which surfaced a pre-existing gap: outgoing packets never set a real
`tx_timestamp` (`NTP::new()` left it zero), so there was nothing meaningful for a server to echo
back and nothing to check. This is fixed as part of authentication: `Client::request` stamps the
outgoing packet's `tx_timestamp` with the same value used as T1, and, when a key is configured,
`Client::receive` rejects a response whose Originate Timestamp doesn't match. The timestamp is
now always stamped (harmless either way, and arguably more correct on its own), but the mismatch
check itself is only enforced when `--auth-key-file` is set — without a key, Cocom has no
integrity guarantee anyway, so rejecting on this alone would be a false sense of security.

## Fail-closed behavior

If `--auth-key-file` is set and a response either has no MAC trailer or fails verification, Cocom
must reject it outright rather than falling back to using it unauthenticated:

- One-shot modes (default output, `-o`, `-v`, `-d`, `-a`): a verification failure is a fatal
  error, consistent with how `Client::receive` already propagates other errors.
- `--sync`: a verification failure is logged and the sample is **not** added to the
  `SlidingWindow` — treated like a failed poll (see
  [sync-and-clock-correction.md](sync-and-clock-correction.md#sliding-window-drift-estimation)),
  so a single forged response can't poison the window the way an unauthenticated jittery-but-real
  sample already can't (via `best_offset`'s minimum-delay filtering) — here it's excluded
  entirely rather than merely down-weighted.

Without `--auth-key-file` (the default), behavior is unchanged — purely opt-in, matching every
other flag added so far (`--apply`, `--state-file`).

## What this is *not*

- Not a solution for public/anonymous servers — see [Why not the PTB default
  server](#why-not-the-ptb-default-server). Only useful against a self-hosted server whose key
  you control.
- Not NTS — no transport encryption, no forward secrecy. A leaked key compromises every exchange
  authenticated with it until rotated; this needs the same operational discipline as any
  long-lived secret (e.g. an SSH private key), not a fundamental flaw for Cocom's targeted
  trusted-network use case.
- Does not by itself address the other open gap on the roadmap, multi-server comparison /
  outlier rejection — authentication proves a response came from the expected server, not that
  the expected server itself is correct.
