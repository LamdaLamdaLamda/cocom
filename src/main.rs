//! Cocom - NTP client implementation purely written in Rust.
use crate::ntp::NTP;
use crate::parser::Parser;
use crate::client::{Client};

mod ntp;
mod client;
mod parser;

/// Entry-Point.
fn main() {
    let parser: Parser = Parser::new();
    let ntp_server : &str = parser.eval_default_host();
    let binding_address : &str = parser.eval_binding_address();
    let mut packet : ntp::NTP = NTP::new();
    let client: client::Client = Client::new(ntp_server, binding_address);

    packet.set_client_mode();
    parser.evaluate(client);
}
