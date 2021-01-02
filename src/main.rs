//! Cocom - NTP client implementation purely written in Rust.
use crate::ntp::NTP;
use crate::client::Client;
use time::Timespec;
use clap::{Arg, App};

mod ntp;
mod client;

/// Verbose-mode functionality of the `Cocom` client. Called when the verbose flag is provided.
/// Prints additional information for further information during the `NTP´ request.
///
/// 1. Parameter - NTP-`Client`.
fn verbose(mut client: Client) {
    println!("[*] Requesting {}", client.host.as_str());
    client.request();

    match client.receive() {
        Ok(mut ntp) => {
            println!("[*] Received NTP-data...");
            let t : Timespec = ntp.get_timespec();
            println!("[*] Time {} sec : {} nsec", t.sec, t.nsec);
            println!("{}", ntp);
        }
        Err(e) => {
            eprintln!("[-] Error: {}", e.to_string());
        }
    }
}

/// Debugging functionality of the `Cocom` client. Called when the debug flag is provided.
/// Prints the `NTP` packet content for debugging purposes.
///
/// 1. Parameter - NTP-`Client`.
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

/// Default functionality of the `Cocom` client. Prints received time as datetime.
/// Called when no flag is provided.
///
/// 1. Parameter - NTP-`Client`.
fn default(mut client: Client) {
    client.request();

    match client.receive() {
        Ok(mut ntp) => {
            println!("{}", ntp.as_datetime());
        }
        Err(e) => {
            eprintln!("[-] Error: {}", e.to_string());
        }
    }
}

/// Entry-Point.
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
    let client : client::Client = Client::new(ntp_server);

    packet.set_client_mode();

    if matches.is_present("v") {
        verbose(client);
    } else if matches.is_present("d") {
        debug(client);
    } else {
        default(client);
    }
}
