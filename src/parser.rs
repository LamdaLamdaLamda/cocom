//! Implementation of the CLI argument parsing. Calls specific `NTP` logic.
use clap::Parser as ClapParser;
use crate::client::{Client, DEFAULT_NTP_HOST_PTB_BRSCHW, DEFAULT_BIND_ADDR};
use crate::offset::SyncResult;
use std::io::Error;
use std::time::Duration;

/// CLI arguments, derived from `Cargo.toml` metadata (name, version, author, description).
#[derive(ClapParser)]
#[command(name = "Cocom", author, version, about, long_about = None)]
struct Args {
    /// Specifies the desired NTP-server.
    host : Option<String>,

    /// Specifies the binding address for the UDP socket. The following format is required; [IP]:[PORT]
    #[arg(short, long)]
    bind : Option<String>,

    /// Activates terminal output
    #[arg(short, long)]
    verbose : bool,

    /// Prints the fields of the received NTP-packet.
    #[arg(short, long)]
    debug : bool,

    /// Prints the round-trip delay and clock offset relative to the server.
    #[arg(short, long)]
    offset : bool,
}

/// `Parser` for the the CLI arguments.
pub(crate) struct Parser {
    /// Parsed CLI arguments.
    args : Args,
}

/// Implementation of `Parser`.
impl Parser {
    /// Initializes new `Parser` instance. Parses the CLI arguments.
    ///
    /// Returns `Parser`.
    pub(crate) fn new() -> Self {
        Parser { args : Args::parse() }
    }

    /// Verbose-mode functionality of the `Cocom` client. Called when the verbose flag is provided.
    /// Prints additional information for further information during the `NTP´ request.
    ///
    /// 1. Parameter - NTP-`Client`.
    fn verbose(mut client: Client) -> Result<(), Error> {
        println!("[*] Requesting {}", client.host.as_str());
        client.request()?;

        let (ntp, sync) = client.receive()?;
        println!("[*] Received NTP-data...");
        let t : Duration = ntp.get_duration();
        println!("[*] Time {} sec : {} nsec", t.as_secs(), t.subsec_nanos());
        println!("{}", ntp);
        Self::print_sync_result(&sync);
        Ok(())
    }

    /// Debugging functionality of the `Cocom` client. Called when the debug flag is provided.
    /// Prints the `NTP` packet content for debugging purposes.
    ///
    /// 1. Parameter - NTP-`Client`.
    fn debug(mut client: Client) -> Result<(), Error> {
        client.request()?;
        let (ntp, _sync) = client.receive()?;
        println!("{}", ntp);
        Ok(())
    }

    /// Default functionality of the `Cocom` client. Prints received time as datetime.
    /// Called when no flag is provided.
    ///
    /// 1. Parameter - NTP-`Client`.
    fn default(mut client: Client) -> Result<(), Error> {
        client.request()?;
        let (ntp, _sync) = client.receive()?;
        println!("{}", ntp.as_datetime());
        Ok(())
    }

    /// Offset-mode functionality of the `Cocom` client. Called when the offset flag is provided.
    /// Prints the round-trip delay and clock offset relative to the server.
    ///
    /// 1. Parameter - NTP-`Client`.
    fn offset(mut client: Client) -> Result<(), Error> {
        client.request()?;
        let (_ntp, sync) = client.receive()?;
        Self::print_sync_result(&sync);
        Ok(())
    }

    /// Prints a `SyncResult` (round-trip delay and clock offset) in milliseconds.
    fn print_sync_result(sync : &SyncResult) {
        let offset_ms : f64 = sync.offset as f64 / 1_000_000.0;
        let delay_ms : f64 = sync.delay as f64 / 1_000_000.0;
        let direction : &str = if sync.offset >= 0 { "behind" } else { "ahead of" };

        println!("[*] Clock offset: {:.3} ms (local clock is {} the server)", offset_ms.abs(), direction);
        println!("[*] Round-trip delay: {:.3} ms", delay_ms);
    }

    /// Evaluates whether the default NTP host is supposed to be used or not.
    pub fn eval_default_host(&self) -> &str {
        self.args.host.as_deref().unwrap_or(DEFAULT_NTP_HOST_PTB_BRSCHW)
    }

    /// Evaluates whether the default binding address for the UDP socket is
    /// supposed to be used or not.
    pub fn eval_binding_address(&self) -> &str {
        self.args.bind.as_deref().unwrap_or(DEFAULT_BIND_ADDR)
    }

    /// Evaluates which CLI argument was passed and runs the corresponding function.
    ///
    /// 1.Argument - The desired `NTP` client.
    pub fn evaluate(self, client : Client) -> Result<(), Error> {
        if self.args.verbose {
            Self::verbose(client)
        } else if self.args.debug {
            Self::debug(client)
        } else if self.args.offset {
            Self::offset(client)
        } else {
            Self::default(client)
        }
    }
}
