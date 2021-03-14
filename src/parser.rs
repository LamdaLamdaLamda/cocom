//! Implementation of the CLI argument parsing. Calls specific `NTP` logic.
use clap::{Arg, App, ArgMatches};
use crate::client::{Client, DEFAULT_NTP_HOST_PTB_BRSCHW, DEFAULT_BIND_ADDR};
use time::Timespec;
use nix::sys::signal::SigHandler;
use nix::sys::signal;

/// Application name.
const APP_NAME : &str = "Cocom";

/// Application version.
const APP_VERSION : &str = "1.0.1";

/// Application author.
const AUTHOR : &str = "LamdaLamdaLamda";

/// Brief description.
const ABOUT : &str = "NTP-Client purely written in Rust.";

/// Host argument.
const ARG_HOST : &str = "HOST";

/// Help text for the host argument.
const ARG_HOST_HELP : &str = "Specifies the desired NTP-server.";

/// Verbose argument.
const ARG_VERBOSE : &str = "v";

/// Verbose argument as long version.
const ARG_VERBOSE_LONG : &str = "verbose";

/// Help text for the verbose argument.
const ARG_VERBOSE_HELP : &str = "Activates terminal output";

/// Debug argument.
const ARG_DEBUG : &str = "d";

/// Debug argument as long format.
const ARG_DEBUG_LONG : &str = "debug";

/// Help text for the debug argument.
const ARG_DEBUG_HELP : &str = "Prints the fields of the received NTP-packet.";

/// Binding address.
const ARG_BIND_NAME : &str = "bind";

/// Help text for the binding address argument.
const ARG_BIND_HELP : &str = "Specifies the binding address for the UDP socket. \
                              The following format is required; [IP]:[PORT]";

/// Bind argument - short
const ARG_BIND_SHORT : &str = "b";

/// Bind argument - long
const ARG_BIND_LONG : &str = "bind";

/// `Parser` for the the CLI arguments.
pub(crate) struct Parser<'a> {
    /// Arguments, including the flags of the CLI
    arg :  ArgMatches<'a>
}

/// Implementation of `Parser`.
impl<'a> Parser<'a> {
    /// Initializes new `Parser` instance. Sets up arguments/outline for the CLI.
    ///
    /// Returns `Parser`.
    pub(crate) fn new() -> Self {
        let sig_action = signal::SigAction::new(
            SigHandler::Handler(Parser::handle_sigint),
            signal::SaFlags::empty(),
            signal::SigSet::empty(),
        );
        unsafe {
            match signal::sigaction(signal::SIGINT, &sig_action) {
                Ok(_) => (),
                Err(e) => panic!("[-] Unable to register SIGINT --> {}", e),
            }
        }

        Parser {
            arg : App::new(APP_NAME)
                .version(APP_VERSION)
                .author(AUTHOR)
                .about(ABOUT)
                .arg(Arg::with_name(ARG_HOST)
                    .help(ARG_HOST_HELP)
                    .required(false)
                    .index(1))
                .arg(Arg::with_name(ARG_BIND_NAME)
                    .help(ARG_BIND_HELP)
                    .short(ARG_BIND_SHORT)
                    .long(ARG_BIND_LONG)
                    .takes_value(true)
                    .required(false))
                .arg(Arg::with_name(ARG_VERBOSE)
                    .short(ARG_VERBOSE)
                    .long(ARG_VERBOSE_LONG)
                    .help(ARG_VERBOSE_HELP))
                .arg(Arg::with_name(ARG_DEBUG)
                    .short(ARG_DEBUG)
                    .long(ARG_DEBUG_LONG)
                    .help(ARG_DEBUG_HELP))
                .get_matches()
        }
    }

    /// Verbose-mode functionality of the `Cocom` client. Called when the verbose flag is provided.
    /// Prints additional information for further information during the `NTP´ request.
    ///
    /// 1. Parameter - NTP-`Client`.
    fn verbose(& mut self, mut client: Client) {
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
    fn debug(&mut self, mut client: Client) {
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
    fn default(&mut self, mut client: Client) {
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
    pub fn eval_default_host(&self) -> &str {
        match self.arg.value_of("HOST") {
            Some(value) => {
                value
            }
            None => {
                DEFAULT_NTP_HOST_PTB_BRSCHW
            }
        }
    }

    /// Evaluates whether the default binding address for the UDP socket is
    /// supposed to be used or not.
    /// This function does not determine the validity of the IP, respectively
    /// of the content of `ArgMatches`.
    pub fn eval_binding_address(&self) -> &str {
        match self.arg.value_of("bind") {
            Some(value) => {
                value
            }
            None => {
                DEFAULT_BIND_ADDR
            }
        }
    }

    /// Evaluates which CLI argument was passed and runs the corresponding function.
    ///
    /// 1.Argument - The desired `NTP` client.
    pub fn evaluate(mut self, client : Client) {
        if self.arg.is_present("v") {
            self.verbose(client);
        } else if self.arg.is_present("d") {
            self.debug(client);
        } else {
            self.default(client);
        }
    }

    /// SIGINT-signal-handler
    ///
    /// 1. Parameter - Unused, but needs to be set, to fulfill the function signature.
    pub extern "C" fn handle_sigint(_: i32) {
        println!("[*] Unable to quit.");
        println!("[*] Waiting for NTP-response...");
    }

}

