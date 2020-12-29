use crate::ntp::NTP;
use crate::client::Client;
use time::Timespec;
use clap::{Arg, App, SubCommand};

mod ntp;
mod client;

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
            .long("debug")
            .help("Prints the fields of the received NTP-packet."))
        .get_matches();
}
