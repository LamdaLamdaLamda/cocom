//! Implementation of the CLI argument parsing. Calls specific `NTP` logic.
use clap::Parser as ClapParser;
use crate::auth::{self, Key};
use crate::client::{Client, DEFAULT_NTP_HOST_PTB_BRSCHW, DEFAULT_BIND_ADDR, DEFAULT_NTP_PORT};
use crate::clock;
use crate::drift::{Sample, SlidingWindow, WINDOW_SIZE};
use crate::ntp::{NTP, Timestamp};
use crate::offset::SyncResult;
use crate::state;
use std::io::Error;
use std::path::PathBuf;
use std::time::Duration;

/// CLI arguments, derived from `Cargo.toml` metadata (name, version, author, description).
#[derive(ClapParser)]
#[command(name = "Cocom", author, version, about, long_about = None)]
struct Args {
    /// Specifies the desired NTP-server.
    host : Option<String>,

    /// Binding address for the UDP socket (IP:PORT).
    #[arg(short, long)]
    bind : Option<String>,

    /// Activates terminal output
    #[arg(short, long)]
    verbose : bool,

    /// Prints the fields of the received NTP-packet.
    #[arg(short, long)]
    debug : bool,

    /// Prints the round-trip delay and clock offset.
    #[arg(short, long)]
    offset : bool,

    /// Continuously polls, reporting offset and drift.
    #[arg(short, long)]
    sync : bool,

    /// Poll interval for --sync, in seconds.
    #[arg(short, long, default_value_t = 64, value_name = "SECONDS")]
    interval : u64,

    /// Applies the offset to the system clock.
    #[arg(short, long)]
    apply : bool,

    /// Allows a correction beyond the 1000s sanity limit.
    #[arg(short = 'f', long = "force-large-step")]
    force_large_step : bool,

    /// Persists the sliding window across runs.
    #[arg(long, value_name = "PATH")]
    state_file : Option<PathBuf>,

    /// Verifies responses via a symmetric key file (see docs).
    #[arg(long, value_name = "PATH")]
    auth_key_file : Option<PathBuf>,
}

/// Settings controlling how (and whether) a measured offset is applied to the system clock —
/// shared across every mode.
struct ApplyOptions {
    /// Apply the measured offset to the system clock (`-a`/`--apply`).
    apply : bool,
    /// Override the panic threshold that otherwise refuses large corrections.
    force_large_step : bool,
    /// Optional file to persist the sliding window to/from, surviving restarts.
    state_file : Option<PathBuf>,
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

    /// Performs a single request/response exchange against the given server. If `auth_keys` is
    /// set, the request is signed and the response must verify against it — a failure here is
    /// a normal `Err`, so callers get fail-closed behavior for free through their existing
    /// error handling (fatal for one-shot modes, logged-and-skipped for `--sync`).
    ///
    /// Returns `Result` with the `NTP` packet and the `SyncResult`, or the specific error.
    fn poll_once(host : &str, bind_addr : &str, auth_keys : Option<&[Key]>) -> Result<(NTP, SyncResult), Error> {
        let mut client : Client = Client::new(host, bind_addr, auth_keys.map(|keys| keys.to_vec()))?;
        client.request()?;
        client.receive()
    }

    /// Determines the offset to actually apply for a one-shot correction. Without a state
    /// file, this is just the raw measurement, unchanged. With one, loads the persisted
    /// window, adds this measurement, and uses the window's minimum-delay ("best") offset —
    /// which is just this fresh measurement on the very first run, but benefits from the
    /// same filtering `--sync` gets once history has accumulated. Saves the updated window
    /// back either way (a save failure is logged, not fatal).
    fn offset_to_apply(sync_result : &SyncResult, state_file : Option<&PathBuf>) -> i128 {
        let path : &PathBuf = match state_file {
            Some(path) => path,
            None => return sync_result.offset,
        };

        let mut window : SlidingWindow = state::load(path);
        window.push(Sample {
            local_time_nanos : Timestamp::now().to_unix_nanos(),
            offset_nanos : sync_result.offset,
            delay_nanos : sync_result.delay,
        });

        let offset : i128 = window.best_offset().map(|s| s.offset_nanos).unwrap_or(sync_result.offset);

        if let Err(e) = state::save(path, &window) {
            eprintln!("[-] Failed to save state file: {}", e);
        }

        offset
    }

    /// Applies `offset_nanos` to the system clock, printing the outcome either way. Uses
    /// `clock::plan_correction` to decide between skipping, a gradual slew, a hard step, or
    /// refusing outright (see its doc comment for the thresholds). Callers decide whether a
    /// failure here should be propagated (one-shot modes) or only logged (`--sync`, so one
    /// failed application doesn't stop the loop).
    fn apply_correction(offset_nanos : i128, force_large_step : bool) -> Result<(), Error> {
        match clock::plan_correction(offset_nanos, force_large_step) {
            clock::Correction::Refuse => Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "refusing to correct the clock by {:+.3} s: exceeds the {:.0} s sanity threshold \
                     (use --force-large-step to override)",
                    offset_nanos as f64 / 1_000_000_000.0,
                    clock::PANIC_THRESHOLD_NANOS as f64 / 1_000_000_000.0
                ),
            )),
            clock::Correction::Skip => {
                println!(
                    "[*] Offset below the {:.3} ms step threshold, not applying",
                    clock::MIN_STEP_THRESHOLD_NANOS as f64 / 1_000_000.0
                );
                Ok(())
            }
            clock::Correction::Slew => {
                clock::slew_clock(offset_nanos)?;
                println!(
                    "[*] System clock slewing by {:+.3} ms (gradual, via adjtime)",
                    offset_nanos as f64 / 1_000_000.0
                );
                Ok(())
            }
            clock::Correction::Step => {
                clock::step_clock(offset_nanos)?;
                println!("[*] System clock stepped by {:+.3} ms", offset_nanos as f64 / 1_000_000.0);
                Ok(())
            }
        }
    }

    /// Verbose-mode functionality of the `Cocom` client. Called when the verbose flag is provided.
    /// Prints additional information for further information during the `NTP´ request.
    fn verbose(host : &str, bind_addr : &str, opts : &ApplyOptions, auth_keys : Option<&[Key]>) -> Result<(), Error> {
        println!("[*] Requesting {}:{}", host, DEFAULT_NTP_PORT);
        let (ntp, sync) = Self::poll_once(host, bind_addr, auth_keys)?;

        println!("[*] Received NTP-data...");
        let t : Duration = ntp.get_duration();
        println!("[*] Time {} sec : {} nsec", t.as_secs(), t.subsec_nanos());
        println!("{}", ntp);
        Self::print_sync_result(&sync);
        if opts.apply {
            let offset : i128 = Self::offset_to_apply(&sync, opts.state_file.as_ref());
            Self::apply_correction(offset, opts.force_large_step)?;
        }
        Ok(())
    }

    /// Debugging functionality of the `Cocom` client. Called when the debug flag is provided.
    /// Prints the `NTP` packet content for debugging purposes.
    fn debug(host : &str, bind_addr : &str, opts : &ApplyOptions, auth_keys : Option<&[Key]>) -> Result<(), Error> {
        let (ntp, sync) = Self::poll_once(host, bind_addr, auth_keys)?;
        println!("{}", ntp);
        if opts.apply {
            let offset : i128 = Self::offset_to_apply(&sync, opts.state_file.as_ref());
            Self::apply_correction(offset, opts.force_large_step)?;
        }
        Ok(())
    }

    /// Default functionality of the `Cocom` client. Prints received time as datetime.
    /// Called when no flag is provided.
    fn default(host : &str, bind_addr : &str, opts : &ApplyOptions, auth_keys : Option<&[Key]>) -> Result<(), Error> {
        let (ntp, sync) = Self::poll_once(host, bind_addr, auth_keys)?;
        println!("{}", ntp.as_datetime());
        if opts.apply {
            let offset : i128 = Self::offset_to_apply(&sync, opts.state_file.as_ref());
            Self::apply_correction(offset, opts.force_large_step)?;
        }
        Ok(())
    }

    /// Offset-mode functionality of the `Cocom` client. Called when the offset flag is provided.
    /// Prints the round-trip delay and clock offset relative to the server.
    fn offset(host : &str, bind_addr : &str, opts : &ApplyOptions, auth_keys : Option<&[Key]>) -> Result<(), Error> {
        let (_ntp, sync) = Self::poll_once(host, bind_addr, auth_keys)?;
        Self::print_sync_result(&sync);
        if opts.apply {
            let offset : i128 = Self::offset_to_apply(&sync, opts.state_file.as_ref());
            Self::apply_correction(offset, opts.force_large_step)?;
        }
        Ok(())
    }

    /// Sync-mode functionality of the `Cocom` client. Called when the sync flag is provided.
    /// Repeatedly queries the server at `interval_secs`, keeping the last `WINDOW_SIZE`
    /// measurements in a `SlidingWindow` — pre-populated from `opts.state_file` on startup, and
    /// saved back to it after every poll, if set. Prints the raw per-poll offset/delay, the
    /// minimum-delay ("best") offset in the window, and a drift-rate estimate from a linear
    /// regression across the window once at least two samples are available. If `opts.apply`
    /// is set, once the window holds at least two samples, applies the filtered ("best")
    /// offset to the system clock each poll — the raw single-poll offset is never applied
    /// directly, to avoid stepping the clock based on jitter. A failed poll or a failed
    /// application is logged and does not stop the loop. Runs until interrupted (Ctrl-C).
    fn sync(host : &str, bind_addr : &str, interval_secs : u64, opts : &ApplyOptions, auth_keys : Option<&[Key]>) -> Result<(), Error> {
        let interval : Duration = Duration::from_secs(interval_secs);
        let mut window : SlidingWindow = match &opts.state_file {
            Some(path) => state::load(path),
            None => SlidingWindow::new(),
        };

        println!("[*] Syncing with {} every {}s (Ctrl-C to stop)", host, interval_secs);
        if window.len() > 0 {
            println!("[*] Loaded {} persisted sample(s)", window.len());
        }

        loop {
            match Self::poll_once(host, bind_addr, auth_keys) {
                Ok((ntp, sync_result)) => {
                    let sample = Sample {
                        local_time_nanos : Timestamp::now().to_unix_nanos(),
                        offset_nanos : sync_result.offset,
                        delay_nanos : sync_result.delay,
                    };
                    window.push(sample);

                    if let Some(path) = &opts.state_file {
                        if let Err(e) = state::save(path, &window) {
                            eprintln!("[-] Failed to save state file: {}", e);
                        }
                    }

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

                    if opts.apply {
                        if window.len() >= 2 {
                            if let Err(e) = Self::apply_correction(best.offset_nanos, opts.force_large_step) {
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
        let interval_secs : u64 = self.args.interval;
        let sync_mode : bool = self.args.sync;
        let verbose_mode : bool = self.args.verbose;
        let debug_mode : bool = self.args.debug;
        let offset_mode : bool = self.args.offset;

        let auth_keys : Option<Vec<Key>> = match &self.args.auth_key_file {
            Some(path) => Some(auth::load_keys(path)?),
            None => None,
        };
        let auth_keys : Option<&[Key]> = auth_keys.as_deref();

        let opts = ApplyOptions {
            apply : self.args.apply,
            force_large_step : self.args.force_large_step,
            state_file : self.args.state_file,
        };

        if sync_mode {
            Self::sync(host, bind_addr, interval_secs, &opts, auth_keys)
        } else if verbose_mode {
            Self::verbose(host, bind_addr, &opts, auth_keys)
        } else if debug_mode {
            Self::debug(host, bind_addr, &opts, auth_keys)
        } else if offset_mode {
            Self::offset(host, bind_addr, &opts, auth_keys)
        } else {
            Self::default(host, bind_addr, &opts, auth_keys)
        }
    }
}
