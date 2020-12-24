use crate::ntp::NTP;
mod ntp;

fn main() {
    let mut packet: ntp::NTP = NTP::new();
    packet.set_client_mode();

}
