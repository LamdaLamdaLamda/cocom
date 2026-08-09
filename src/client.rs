//! Implementation of the NTP client.
use std::net::UdpSocket;
use crate::ntp::NTP;
use std::io::{Error};
use std::time::Duration;

/// Default IPv4 binding address for the UDP sockets.
pub(crate) const DEFAULT_BIND_ADDR : &str = "0.0.0.0:35000";

/// Default NTP server.
pub(crate) const DEFAULT_NTP_HOST_PTB_BRSCHW : &str = "192.53.103.108";

/// Default `NTP` port.
const DEFAULT_NTP_PORT : u8 = 123;

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
        })
    }

    /// Applying `NTP` request to a server instance.
    ///
    /// Returns `Result` with the size of the requested packet size or the specific error.
    pub fn request(&mut self) -> Result<usize, Error> {
        self.data.set_client_mode();

        let packet : Vec<u8> = self.data.as_vec_u8()?;
        let bytes_sent : usize = self.socket.send_to(&packet, self.host.as_str())?;
        Ok(bytes_sent)
    }

    /// Waits for the receive of the NTP packet after `request` was called.
    ///
    /// Returns `Result` with the `NTP` packet or the specific error.
    pub fn receive(mut self) -> Result<NTP, Error> {
        self.socket.recv_from(&mut self.buffer)?;

        let ntp_packet: NTP = NTP::as_ntp(&self.buffer.to_vec())?;

        drop(self.socket);
        Ok(ntp_packet)
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
        let result : Result<NTP, Error> = client.receive();
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





