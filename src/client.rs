use std::net::UdpSocket;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::ntp::{NTP, NTP_SIZE};
use std::io::Error;

const DEFAULT_BIND_ADDR : &str = "0.0.0.0:35000";
const DEFAULT_NTP_PORT : u8 = 123;

pub(crate) struct Client {
    socket : UdpSocket,
    data : NTP,
    buffer : [u8; 1000],
    host : String,
}

impl Client {
    pub fn new(host : &str) -> Client {
        Client {
            socket : UdpSocket::bind(DEFAULT_BIND_ADDR).expect("Unable to bind socket..."),
            data : NTP::new(),
            buffer : [0; 1000],
            host : format!("{host}:{port}", host = host, port = DEFAULT_NTP_PORT),
        }
    }

    pub fn request(&mut self) -> usize {
        self.data.set_client_mode();

        let packet: Vec<u8> = self.data.as_vec_u8().unwrap();
        println!("[*] Requesting {}", self.host.as_str());

        self.socket.send_to(&packet, self.host.as_str()).unwrap()
    }

    pub fn receive(mut self) -> Result<NTP, std::io::Error> {
        let mut ntp_packet: NTP = NTP::new();
        let (size, _) = self.socket.recv_from(&mut self.buffer).expect("No data received");
        ntp_packet = NTP::as_ntp(&self.buffer.to_vec())?;

        drop(self.socket);
        Ok(ntp_packet)
    }
}