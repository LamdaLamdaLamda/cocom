//! Implementation of the CLI argument parsing. Calls specific `NTP` logic.
use clap::Parser as ClapParser;
use crate::client::{Client, DEFAULT_NTP_HOST_PTB_BRSCHW, DEFAULT_BIND_ADDR, DEFAULT_NTP_PORT};
use crate::clock;
use crate::drift::{Sample, SlidingWindow, WINDOW_SIZE};
use crate::ntp::{NTP, Timestamp};
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

    /// Runs continuously, re-querying the server at a fixed interval and reporting offset,
    /// delay, and estimated clock drift. Runs until interrupted (Ctrl-C).
    #[arg(short, long)]
    sync : bool,

    /// Poll interval in seconds, used together with `--sync`.
    #[arg(short, long, default_value_t = 64, value_name = "SECONDS")]
    interval : u64,

    /// Applies the measured offset to the system clock (a hard step, not a gradual slew).
    /// Requires elevated privileges (root / CAP_SYS_TIME on Linux, admin on macOS). Without
    /// this flag, Cocom only measures and reports — it never touches the system clock.
    #[arg(short, long)]
    apply : bool,

    /// Overrides the sanity threshold that otherwise refuses --apply corrections larger than
    /// 1000 seconds, matching classic ntpd's "panic" behavior. Only relevant with --apply.
    #[arg(short = 'f', long = "force-large-step")]
    force_large_step : bool,
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

    /// Performs a single request/response exchange against the given server.
    ///
    /// Returns `Result` with the `NTP` packet and the `SyncResult`, or the specific error.
    fn poll_once(host : &str, bind_addr : &str) -> Result<(NTP, SyncResult), Error> {
        let mut client : Client = Client::new(host, bind_addr)?;
        client.request()?;
        client.receive()
    }

    /// Applies `offset_nanos` to the system clock if it exceeds `clock::MIN_STEP_THRESHOLD_NANOS`,
    /// printing the outcome either way. Refuses offsets larger than `clock::PANIC_THRESHOLD_NANOS`
    /// unless `force_large_step` is set — a misconfigured or spoofed server should not be able to
    /// silently step the clock by an implausible amount. Callers decide whether a failure here
    /// should be propagated (one-shot modes) or only logged (`--sync`, so one failed application
    /// doesn't stop the loop).
    fn apply_correction(offset_nanos : i128, force_large_step : bool) -> Result<(), Error> {
        if clock::exceeds_panic_threshold(offset_nanos) && !force_large_step {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "refusing to step the clock by {:+.3} s: exceeds the {:.0} s sanity threshold \
                     (use --force-large-step to override)",
                    offset_nanos as f64 / 1_000_000_000.0,
                    clock::PANIC_THRESHOLD_NANOS as f64 / 1_000_000_000.0
                ),
            ));
        }

        if !clock::should_step(offset_nanos) {
            println!(
                "[*] Offset below the {:.3} ms step threshold, not applying",
                clock::MIN_STEP_THRESHOLD_NANOS as f64 / 1_000_000.0
            );
            return Ok(());
        }

        clock::step_clock(offset_nanos)?;
        println!("[*] System clock stepped by {:+.3} ms", offset_nanos as f64 / 1_000_000.0);
        Ok(())
    }

    /// Verbose-mode functionality of the `Cocom` client. Called when the verbose flag is provided.
    /// Prints additional information for further information during the `NTP´ request.
    fn verbose(host : &str, bind_addr : &str, apply : bool, force_large_step : bool) -> Result<(), Error> {
        println!("[*] Requesting {}:{}", host, DEFAULT_NTP_PORT);
        let (ntp, sync) = Self::poll_once(host, bind_addr)?;

        println!("[*] Received NTP-data...");
        let t : Duration = ntp.get_duration();
        println!("[*] Time {} sec : {} nsec", t.as_secs(), t.subsec_nanos());
        println!("{}", ntp);
        Self::print_sync_result(&sync);
        if apply {
            Self::apply_correction(sync.offset, force_large_step)?;
        }
        Ok(())
    }

    /// Debugging functionality of the `Cocom` client. Called when the debug flag is provided.
    /// Prints the `NTP` packet content for debugging purposes.
    fn debug(host : &str, bind_addr : &str, apply : bool, force_large_step : bool) -> Result<(), Error> {
        let (ntp, sync) = Self::poll_once(host, bind_addr)?;
        println!("{}", ntp);
        if apply {
            Self::apply_correction(sync.offset, force_large_step)?;
        }
        Ok(())
    }

    /// Default functionality of the `Cocom` client. Prints received time as datetime.
    /// Called when no flag is provided.
    fn default(host : &str, bind_addr : &str, apply : bool, force_large_step : bool) -> Result<(), Error> {
        let (ntp, sync) = Self::poll_once(host, bind_addr)?;
        println!("{}", ntp.as_datetime());
        if apply {
            Self::apply_correction(sync.offset, force_large_step)?;
        }
        Ok(())
    }

    /// Offset-mode functionality of the `Cocom` client. Called when the offset flag is provided.
    /// Prints the round-trip delay and clock offset relative to the server.
    fn offset(host : &str, bind_addr : &str, apply : bool, force_large_step : bool) -> Result<(), Error> {
        let (_ntp, sync) = Self::poll_once(host, bind_addr)?;
        Self::print_sync_result(&sync);
        if apply {
            Self::apply_correction(sync.offset, force_large_step)?;
        }
        Ok(())
    }

    /// Sync-mode functionality of the `Cocom` client. Called when the sync flag is provided.
    /// Repeatedly queries the server at `interval_secs`, keeping the last `WINDOW_SIZE`
    /// measurements in a `SlidingWindow`. Prints the raw per-poll offset/delay, the
    /// minimum-delay ("best") offset in the window, and a drift-rate estimate from a linear
    /// regression across the window once at least two samples are available. If `apply` is
    /// set, once the window holds at least two samples, applies the filtered ("best") offset
    /// to the system clock each poll — the raw single-poll offset is never applied directly,
    /// to avoid stepping the clock based on jitter. A failed poll or a failed application is
    /// logged and does not stop the loop. Runs until interrupted (Ctrl-C).
    fn sync(host : &str, bind_addr : &str, interval_secs : u64, apply : bool, force_large_step : bool) -> Result<(), Error> {
        let interval : Duration = Duration::from_secs(interval_secs);
        let mut window : SlidingWindow = SlidingWindow::new();

        println!("[*] Syncing with {} every {}s (Ctrl-C to stop)", host, interval_secs);

        loop {
            match Self::poll_once(host, bind_addr) {
                Ok((ntp, sync_result)) => {
                    let sample = Sample {
                        local_time_nanos : Timestamp::now().to_unix_nanos(),
                        offset_nanos : sync_result.offset,
                        delay_nanos : sync_result.delay,
                    };
                    window.push(sample);

                    let best : &Sample = window.best_offset().expect("window has at least one sample");
                    let best_offset_ms : f64 = best.offset_nanos as f64 / 1_000_000.0;

                    match window.estimate_drift() {
                        Some(rate) => println!(
                            "[*] {}  offset: {:+.3} ms  delay: {:.3} ms  drift: {:+.3} ppm  \
                             (best: {:+.3} ms, window: {}/{})",
                            ntp.as_datetime(),
                            sync_result.offset as f64 / 1_000_000.0,
                            sync_result.delay as f64 / 1_000_000.0,
                            rate * 1_000_000.0,
                            best_offset_ms,
                            window.len(), WINDOW_SIZE
                        ),
                        None => println!(
                            "[*] {}  offset: {:+.3} ms  delay: {:.3} ms  drift: n/a (warming up, \
                             window: {}/{})",
                            ntp.as_datetime(),
                            sync_result.offset as f64 / 1_000_000.0,
                            sync_result.delay as f64 / 1_000_000.0,
                            window.len(), WINDOW_SIZE
                        ),
                    }

                    if apply {
                        if window.len() >= 2 {
                            if let Err(e) = Self::apply_correction(best.offset_nanos, force_large_step) {
                                eprintln!("[-] Failed to apply clock correction: {}", e);
                            }
                        } else {
                            println!("[*] Waiting for at least 2 samples before applying corrections");
                        }
                    }
                }
                Err(e) => eprintln!("[-] Error: {}", e),
            }

            std::thread::sleep(interval);
        }
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
    /// 1. Parameter - NTP server.
    /// 2. Parameter - Binding address for the UDP socket.
    pub fn evaluate(self, host : &str, bind_addr : &str) -> Result<(), Error> {
        let apply : bool = self.args.apply;
        let force_large_step : bool = self.args.force_large_step;

        if self.args.sync {
            Self::sync(host, bind_addr, self.args.interval, apply, force_large_step)
        } else if self.args.verbose {
            Self::verbose(host, bind_addr, apply, force_large_step)
        } else if self.args.debug {
            Self::debug(host, bind_addr, apply, force_large_step)
        } else if self.args.offset {
            Self::offset(host, bind_addr, apply, force_large_step)
        } else {
            Self::default(host, bind_addr, apply, force_large_step)
        }
    }
}
