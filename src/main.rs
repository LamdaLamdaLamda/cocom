//! Cocom - NTP client implementation purely written in Rust.
use crate::parser::Parser;
use std::process::ExitCode;

mod ntp;
mod client;
mod parser;
mod offset;
mod drift;
mod clock;
mod state;
mod auth;

/// Entry-Point.
fn main() -> ExitCode {
    let parser : Parser = Parser::new();
    let ntp_server : String = parser.eval_default_host().to_string();
    let binding_address : String = parser.eval_binding_address().to_string();

    match parser.evaluate(&ntp_server, &binding_address) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[-] Error: {}", e);
            ExitCode::FAILURE
        }
    }
}
