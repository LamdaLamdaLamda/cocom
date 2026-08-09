//! Cocom - NTP client implementation purely written in Rust.
use crate::parser::Parser;
use crate::client::Client;
use std::process::ExitCode;

mod ntp;
mod client;
mod parser;
mod offset;

/// Entry-Point.
fn main() -> ExitCode {
    let parser : Parser = Parser::new();
    let ntp_server : &str = parser.eval_default_host();
    let binding_address : &str = parser.eval_binding_address();

    let result = Client::new(ntp_server, binding_address)
        .and_then(move |client| parser.evaluate(client));

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[-] Error: {}", e);
            ExitCode::FAILURE
        }
    }
}
