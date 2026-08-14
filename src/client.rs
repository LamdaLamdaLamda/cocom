//! Implementation of the NTP client.
use std::net::UdpSocket;
use crate::auth::{self, Key};
use crate::ntp::{NTP, Timestamp};
use crate::offset::{self, SyncResult};
use std::convert::TryInto;
use std::io::{Error, ErrorKind};
use std::time::Duration;

/// Default IPv4 binding address for the UDP sockets.
pub(crate) const DEFAULT_BIND_ADDR : &str = "0.0.0.0:35000";

/// Default NTP server.
pub(crate) const DEFAULT_NTP_HOST_PTB_BRSCHW : &str = "192.53.103.108";

/// Default `NTP` port.
pub(crate) const DEFAULT_NTP_PORT : u8 = 123;

/// Maximum time to wait for an NTP server response before giving up.
const DEFAULT_READ_TIMEOUT : Duration = Duration::from_secs(5);

/// `NTP` client.
pub(crate) struct Client {
    /// UDP socket.
    socket : UdpSocket,
    /// `NTP` packet.
    data : NTP,
    /// RX/TX buffer for the UDP socket.
    buffer : [u8; 1000],
    /// NTP server.
    pub(crate) host : String,
    /// Local time the request was sent (T1), set by `request`.
    origin_time : Option<Timestamp>,
    /// Symmetric keys for request signing / response verification (`--auth-key-file`). The
    /// first key signs outgoing requests; every key is checked against a response's Key ID.
    /// `None` means authentication is off — requests are sent bare and responses are accepted
    /// unconditionally, exactly as before this feature existed.
    auth_keys : Option<Vec<Key>>,
}

/// Implementation of the `Client`.
impl Client {
    /// Instantiation of a new `Client`.
    ///
    /// 1. Parameter - NTP server were the requests will be send.
    /// 2. Parameter - Binding address for the UDP socket.
    /// 3. Parameter - Symmetric keys loaded from `--auth-key-file`, or `None` to leave requests
    ///    unsigned and responses unverified.
    ///
    /// Returns `Result` with the `Client` or the specific error.
    pub fn new(host : &str, address_bind : &str, auth_keys : Option<Vec<Key>>) -> Result<Client, Error> {
        Self::new_with_port(host, DEFAULT_NTP_PORT as u16, address_bind, auth_keys)
    }

    /// Like `new`, but targets an explicit port instead of the standard NTP port 123. Binding
    /// port 123 requires elevated privileges, so tests that spin up a local mock server on an
    /// OS-assigned loopback port go through this instead.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new_with_port(host : &str, port : u16, address_bind : &str, auth_keys : Option<Vec<Key>>) -> Result<Client, Error> {
        let socket : UdpSocket = UdpSocket::bind(address_bind)?;
        socket.set_read_timeout(Some(DEFAULT_READ_TIMEOUT))?;

        Ok(Client {
            socket,
            data : NTP::new(),
            buffer : [0; 1000],
            host : format!("{host}:{port}", host = host, port = port),
            origin_time : None,
            auth_keys,
        })
    }

    /// Applying `NTP` request to a server instance.
    ///
    /// Stamps the outgoing packet's Transmit Timestamp and records the same value as the local
    /// send time (T1), used both for the later round-trip/offset calculation and — when
    /// authentication is enabled — to check the response's echoed Originate Timestamp. If
    /// `auth_keys` is set, appends a `[ Key ID | HMAC-SHA256 ]` trailer signed with the first key.
    ///
    /// Returns `Result` with the size of the sent packet or the specific error.
    pub fn request(&mut self) -> Result<usize, Error> {
        self.data.set_client_mode();

        let sent_time : Timestamp = Timestamp::now();
        self.data.tx_timestamp = sent_time;
        self.origin_time = Some(sent_time);

        let packet : Vec<u8> = self.data.as_vec_u8()?;
        let packet : Vec<u8> = match self.auth_keys.as_ref().and_then(|keys| keys.first()) {
            Some(signing_key) => {
                let packet_bytes : [u8; 48] = packet.as_slice().try_into().expect("NTP packet is always 48 bytes");
                auth::append_trailer(&packet_bytes, signing_key)
            }
            None => packet,
        };

        let bytes_sent : usize = self.socket.send_to(&packet, self.host.as_str())?;
        Ok(bytes_sent)
    }

    /// Waits for the receive of the NTP packet after `request` was called.
    ///
    /// Records the local receive time (T4) and computes the round-trip delay and clock
    /// offset from the four NTP timestamps ([RFC 5905, section 8](https://tools.ietf.org/html/rfc5905#section-8)).
    ///
    /// If `auth_keys` is set, fails closed: the response must carry a trailer that verifies
    /// against one of the loaded keys, and its Originate Timestamp must match the request's
    /// Transmit Timestamp (replay protection) — otherwise this returns an error instead of the
    /// packet, same as any other I/O failure.
    ///
    /// Returns `Result` with the `NTP` packet and the `SyncResult`, or the specific error.
    ///
    /// # Panics
    ///
    /// Panics if called before `request`, which is always set by the caller.
    pub fn receive(mut self) -> Result<(NTP, SyncResult), Error> {
        let (bytes_received, _) = self.socket.recv_from(&mut self.buffer)?;
        let destination_time : Timestamp = Timestamp::now();

        if bytes_received < 48 {
            return Err(Error::new(ErrorKind::InvalidData, "response is shorter than a valid NTP packet"));
        }

        let ntp_packet : NTP = NTP::as_ntp(&self.buffer[..48].to_vec())?;
        let origin_time : Timestamp = self.origin_time.expect("receive called before request");

        if let Some(keys) = &self.auth_keys {
            let packet_bytes : [u8; 48] = self.buffer[..48].try_into().expect("slice is exactly 48 bytes");
            let trailer : &[u8] = &self.buffer[48..bytes_received];
            auth::verify_trailer(&packet_bytes, trailer, keys)?;

            if ntp_packet.originate_timestamp != origin_time {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "response's Originate Timestamp doesn't match the request (possible replay)",
                ));
            }
        }

        let sync_result : SyncResult = offset::compute(
            origin_time.to_unix_nanos(),
            ntp_packet.rx_timestamp.to_unix_nanos(),
            ntp_packet.tx_timestamp.to_unix_nanos(),
            destination_time.to_unix_nanos(),
        );

        drop(self.socket);
        Ok((ntp_packet, sync_result))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Requires outbound UDP connectivity to a public NTP pool server, which is not reliably
    /// available on every CI runner. Run manually via `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_client_new_request_valid_host() {
        let mut client : Client = Client::new("0.us.pool.ntp.org", "0.0.0.0:35000", None).unwrap();
        client.request().unwrap();
        let result : Result<(NTP, SyncResult), Error> = client.receive();
        assert_eq!(result.is_ok(), true);
    }

    #[test]
    #[should_panic]
    fn test_client_new_request_invalid_host() {
        let mut client : Client = Client::new("onmywaytothemagicunicorn", "0.0.0.0:35001", None).unwrap();
        client.request().unwrap();
    }

    /// Requires outbound UDP connectivity to a public NTP pool server, which is not reliably
    /// available on every CI runner. Run manually via `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_packet_size() {
        let mut client : Client = Client::new("1.us.pool.ntp.org", "0.0.0.0:35002", None).unwrap();
        assert_eq!(client.request().unwrap(), 48)
    }

    // --- Mock self-hosted server, over loopback ------------------------------------------
    //
    // These mimic a self-hosted server without needing outbound network access or root (which
    // binding the real port 123 would require) — a single-shot UDP responder on an OS-assigned
    // loopback port, driven by `Client::new_with_port`. Covers both a plain (no `--auth-key-file`)
    // exchange and the HMAC-authenticated one: matching keys, a non-cooperating/unsigned server,
    // a wrong key, and a replayed Originate Timestamp.

    /// Builds a plausible response `NTP` packet, echoing `request`'s Transmit Timestamp as the
    /// Originate Timestamp — what a well-behaved server does, and what satisfies the client's
    /// replay check when authentication is enabled.
    fn mock_response(request : &NTP) -> NTP {
        let mut response : NTP = NTP::new();
        response.stratum = 1;
        response.originate_timestamp = request.tx_timestamp;
        response.rx_timestamp = Timestamp::now();
        response.tx_timestamp = Timestamp::now();
        response
    }

    /// Spawns a one-shot mock server on an OS-assigned loopback port: waits for a single
    /// request, parses it, hands it to `respond` to build the raw response bytes, and sends
    /// those back. Returns the loopback host/port to point a `Client` at, plus the thread's
    /// `JoinHandle` so the test can `join` it and surface any panic inside the closure.
    fn spawn_mock_server<F>(respond : F) -> (String, u16, std::thread::JoinHandle<()>)
    where
        F : FnOnce(NTP) -> Vec<u8> + Send + 'static,
    {
        let socket : UdpSocket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let addr = socket.local_addr().unwrap();

        let handle = std::thread::spawn(move || {
            let mut buffer : [u8; 1000] = [0; 1000];
            let (bytes_received, from) = socket.recv_from(&mut buffer).unwrap();
            let request : NTP = NTP::as_ntp(&buffer[..48].to_vec()).unwrap();
            let _ = bytes_received;

            let response_bytes : Vec<u8> = respond(request);
            socket.send_to(&response_bytes, from).unwrap();
        });

        (addr.ip().to_string(), addr.port(), handle)
    }

    #[test]
    fn test_mock_server_unauthenticated_roundtrip_succeeds() {
        let (host, port, server) = spawn_mock_server(|request| {
            mock_response(&request).as_vec_u8().unwrap()
        });

        let mut client : Client = Client::new_with_port(&host, port, "127.0.0.1:0", None).unwrap();
        client.request().unwrap();
        let result : Result<(NTP, SyncResult), Error> = client.receive();

        server.join().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_server_authenticated_roundtrip_with_matching_key_succeeds() {
        let key : Key = Key { id : 1, secret : b"shared-secret".to_vec() };
        let server_key : Key = key.clone();

        let (host, port, server) = spawn_mock_server(move |request| {
            let response_bytes : Vec<u8> = mock_response(&request).as_vec_u8().unwrap();
            let response_array : [u8; 48] = response_bytes.as_slice().try_into().unwrap();
            auth::append_trailer(&response_array, &server_key)
        });

        let mut client : Client = Client::new_with_port(&host, port, "127.0.0.1:0", Some(vec![key])).unwrap();
        client.request().unwrap();
        let result : Result<(NTP, SyncResult), Error> = client.receive();

        server.join().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_server_authenticated_rejects_unsigned_response() {
        // Simulates a non-cooperating server (e.g. a public server without symmetric-key
        // support) that just replies with a plain, trailer-less packet.
        let key : Key = Key { id : 1, secret : b"shared-secret".to_vec() };

        let (host, port, server) = spawn_mock_server(|request| {
            mock_response(&request).as_vec_u8().unwrap()
        });

        let mut client : Client = Client::new_with_port(&host, port, "127.0.0.1:0", Some(vec![key])).unwrap();
        client.request().unwrap();
        let result : Result<(NTP, SyncResult), Error> = client.receive();

        server.join().unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_server_authenticated_rejects_wrong_key() {
        let server_key : Key = Key { id : 1, secret : b"servers-secret".to_vec() };
        let client_key : Key = Key { id : 1, secret : b"clients-secret".to_vec() };
        let signing_key : Key = server_key.clone();

        let (host, port, server) = spawn_mock_server(move |request| {
            let response_bytes : Vec<u8> = mock_response(&request).as_vec_u8().unwrap();
            let response_array : [u8; 48] = response_bytes.as_slice().try_into().unwrap();
            auth::append_trailer(&response_array, &signing_key)
        });

        let mut client : Client = Client::new_with_port(&host, port, "127.0.0.1:0", Some(vec![client_key])).unwrap();
        client.request().unwrap();
        let result : Result<(NTP, SyncResult), Error> = client.receive();

        server.join().unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_server_authenticated_rejects_replayed_response() {
        // A validly-signed response, but one that doesn't echo the request's Transmit Timestamp
        // as its Originate Timestamp — as if an attacker captured an old, genuinely-signed
        // response and replayed it against a later request.
        let key : Key = Key { id : 1, secret : b"shared-secret".to_vec() };
        let server_key : Key = key.clone();

        let (host, port, server) = spawn_mock_server(move |request| {
            let mut response : NTP = mock_response(&request);
            response.originate_timestamp = Timestamp { seconds : 1, fraction : 0 };
            let response_bytes : Vec<u8> = response.as_vec_u8().unwrap();
            let response_array : [u8; 48] = response_bytes.as_slice().try_into().unwrap();
            auth::append_trailer(&response_array, &server_key)
        });

        let mut client : Client = Client::new_with_port(&host, port, "127.0.0.1:0", Some(vec![key])).unwrap();
        client.request().unwrap();
        let result : Result<(NTP, SyncResult), Error> = client.receive();

        server.join().unwrap();
        assert!(result.is_err());
    }
}





