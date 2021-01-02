//! Implementation of the NTP client.
use std::net::UdpSocket;
use crate::ntp::{NTP};
use std::io::{Error};

/// Default IPv4 binding address for the UDP sockets.
const DEFAULT_BIND_ADDR : &str = "0.0.0.0:35000";

/// Default `NTP` port.
const DEFAULT_NTP_PORT : u8 = 123;

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
    /// Returns `Client`.
    pub fn new(host : &str) -> Client {
        Client {
            socket : UdpSocket::bind(DEFAULT_BIND_ADDR).expect("Unable to bind socket..."),
            data : NTP::new(),
            buffer : [0; 1000],
            host : format!("{host}:{port}", host = host, port = DEFAULT_NTP_PORT),
        }
    }

    /// Applying `NTP` request to a server instance.
    ///
    /// Returns the size of the requested packet size.
    pub fn request(&mut self) -> usize {
        self.data.set_client_mode();

        let packet: Vec<u8> = self.data.as_vec_u8().unwrap();
        self.socket.send_to(&packet, self.host.as_str()).unwrap()
    }

    /// Waits for the receive of the NTP packet after `request` was called.
    ///
    /// Returns `Result` with the `NTP` packet or the specific error.
    pub fn receive(mut self) -> Result<NTP, Error> {
        self.socket.recv_from(&mut self.buffer).expect("No data received");

        let ntp_packet: NTP = NTP::as_ntp(&self.buffer.to_vec())?;

        drop(self.socket);
        Ok(ntp_packet)
    }
}