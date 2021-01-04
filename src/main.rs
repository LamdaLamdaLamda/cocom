//! Cocom - NTP client implementation purely written in Rust.
use crate::ntp::NTP;
use crate::client::{Client, DEFAULT_NTP_HOST_PTB_BRSCHW};
use time::Timespec;
use clap::{Arg, App, ArgMatches};

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

/// Evaluates whether the default NTP host is supposed to be used or not.
/// This function does not determine the validity of the IP, respectively
/// of the content of `ArgMatches`.
///
/// 1. parameter - Passed program arguments.
fn eval_default_host<'a>(arg : &'a ArgMatches) -> &'a str {
    match arg.value_of("HOST") {
        Some(value) => {
            value
        }
        None => {
            DEFAULT_NTP_HOST_PTB_BRSCHW
        }
    }
}

/// Entry-Point.
fn main() {
    let matches = App::new("Cocom")
        .version("1.0.0")
        .author("LamdaLamdaLamda ")
        .about("NTP-Client purely written in Rust.")
        .arg(Arg::with_name("HOST")
            .help("Specifies the desired NTP-server.")
            .required(false)
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

    let ntp_server : &str = eval_default_host(&matches);
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
