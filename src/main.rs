use crate::ntp::NTP;
use crate::client::Client;
use time::Timespec;
use clap::{Arg, App, SubCommand};

mod ntp;
mod client;

fn verbose(mut client: Client) {
    println!("[*] Requesting {}", client.host.as_str());
    client.request();

    match client.receive() {
        Ok(ntp) => {
            println!("[*] Received NTP-data...");
            let t : Timespec = NTP::to_timespec(ntp.rx_timestamp_seconds, ntp.rx_timestamp_seconds_fraction);
            println!("[*] Time {} sec : {} nsec", t.sec, t.nsec);
            println!("{}", ntp);
        }
        Err(e) => {
            eprintln!("[-] Error: {}", e.to_string());
        }
    }
}

fn debug(mut client: Client) {
    client.request();

    match client.receive() {
        Ok(ntp) => {
            println!("{}", ntp);
        }
        Err(e) => {
            eprintln!("[-] Error: {}", e.to_string());
        }
    }
}

fn default(mut client: Client) {
    client.request();

    match client.receive() {
        Ok(ntp) => {
            let t : Timespec = NTP::to_timespec(ntp.rx_timestamp_seconds, ntp.rx_timestamp_seconds_fraction);
            println!("{} {}", t.sec, t.nsec);
        }
        Err(e) => {
            eprintln!("[-] Error: {}", e.to_string());
        }
    }
}

fn main() {
    let matches = App::new("Cocom")
        .version("0.1.0")
        .author("LamdaLamdaLamda ")
        .about("NTP-Client purely written in Rust.")
        .arg(Arg::with_name("HOST")
            .help("Specifies the desired NTP-server.")
            .required(true)
            .index(1))
        .arg(Arg::with_name("v")
            .short("v")
            .long("verbose")
            .help("Activates terminal output"))
        .arg(Arg::with_name("d")
            .short("d")
            .long("debug")
            .help("Prints the fields of the received NTP-packet."))
        .get_matches();

    let ntp_server : &str = matches.value_of("HOST").unwrap();
    let mut packet : ntp::NTP = NTP::new();
    let mut client : client::Client = Client::new(ntp_server);

    packet.set_client_mode();

    if matches.is_present("v") {
        verbose(client);
    } else if matches.is_present("d") {
        debug(client);
    } else {
        default(client);
    }
}
