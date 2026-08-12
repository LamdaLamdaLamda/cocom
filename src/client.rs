//! Implementation of the NTP client.
use std::net::UdpSocket;
use crate::ntp::{NTP, Timestamp};
use crate::offset::{self, SyncResult};
use std::io::{Error};
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
}

/// Implementation of the `Client`.
impl Client {
    /// Instantiation of a new `Client`.
    ///
    /// 1. Parameter - NTP server were the requests will be send.
    /// 2. Parameter - Binding address for the UDP socket.
    ///
    /// Returns `Result` with the `Client` or the specific error.
    pub fn new(host : &str, address_bind : &str) -> Result<Client, Error> {
        let socket : UdpSocket = UdpSocket::bind(address_bind)?;
        socket.set_read_timeout(Some(DEFAULT_READ_TIMEOUT))?;

        Ok(Client {
            socket,
            data : NTP::new(),
            buffer : [0; 1000],
            host : format!("{host}:{port}", host = host, port = DEFAULT_NTP_PORT),
            origin_time : None,
        })
    }

    /// Applying `NTP` request to a server instance.
    ///
    /// Records the local send time (T1) for the later round-trip/offset calculation.
    ///
    /// Returns `Result` with the size of the requested packet size or the specific error.
    pub fn request(&mut self) -> Result<usize, Error> {
        self.data.set_client_mode();

        let packet : Vec<u8> = self.data.as_vec_u8()?;
        self.origin_time = Some(Timestamp::now());
        let bytes_sent : usize = self.socket.send_to(&packet, self.host.as_str())?;
        Ok(bytes_sent)
    }

    /// Waits for the receive of the NTP packet after `request` was called.
    ///
    /// Records the local receive time (T4) and computes the round-trip delay and clock
    /// offset from the four NTP timestamps ([RFC 5905, section 8](https://tools.ietf.org/html/rfc5905#section-8)).
    ///
    /// Returns `Result` with the `NTP` packet and the `SyncResult`, or the specific error.
    ///
    /// # Panics
    ///
    /// Panics if called before `request`, which is always set by the caller.
    pub fn receive(mut self) -> Result<(NTP, SyncResult), Error> {
        self.socket.recv_from(&mut self.buffer)?;
        let destination_time : Timestamp = Timestamp::now();

        let ntp_packet : NTP = NTP::as_ntp(&self.buffer.to_vec())?;

        let origin_time : Timestamp = self.origin_time.expect("receive called before request");
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
        let mut client : Client = Client::new("0.us.pool.ntp.org", "0.0.0.0:35000").unwrap();
        client.request().unwrap();
        let result : Result<(NTP, SyncResult), Error> = client.receive();
        assert_eq!(result.is_ok(), true);
    }

    #[test]
    #[should_panic]
    fn test_client_new_request_invalid_host() {
        let mut client : Client = Client::new("onmywaytothemagicunicorn", "0.0.0.0:35001").unwrap();
        client.request().unwrap();
    }

    /// Requires outbound UDP connectivity to a public NTP pool server, which is not reliably
    /// available on every CI runner. Run manually via `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_packet_size() {
        let mut client : Client = Client::new("1.us.pool.ntp.org", "0.0.0.0:35002").unwrap();
        assert_eq!(client.request().unwrap(), 48)
    }
}





