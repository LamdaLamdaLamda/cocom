use crate::ntp::NTP;
use crate::client::Client;
use std::io::Error;
use time::Timespec;

mod ntp;
mod client;

fn main() {
    let mut packet : ntp::NTP = NTP::new();
    let mut client : client::Client = Client::new("pool.ntp.org");

    packet.set_client_mode();

    println!("[*] NTP-request...");

    client.request();

    match client.receive() {
        Ok(ntp) => {
            println!("[*] Received NTP-data...");
            let t : Timespec = NTP::to_timespec(ntp.rx_timestamp_seconds, ntp.rx_timestamp_seconds_fraction);
            println!("[*] Time {} sec : {} nsec", t.sec, t.nsec);
        }
        Err(e) => {
            eprintln!("[-] Error: {}", e.to_string());
        }
    }
}
