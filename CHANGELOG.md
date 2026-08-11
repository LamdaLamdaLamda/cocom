# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

### Changed

- Tuned the `[profile.release]` build for binary size, relevant for embedded/production
  deployment: `opt-level = "z"`, `codegen-units = 1`, `panic = "abort"`, `strip = true` (added),
  keeping the existing `lto = true`. Reduces the release binary from ~788 KB to ~443 KB
  (measured on macOS/arm64). `panic = "abort"` only affects `cargo build --release`; `cargo test`
  still uses the dev/test profile with unwinding, so `#[should_panic]` tests are unaffected.

### Added

- CI: both `linux.yml` and `macos.yml` now verify `--apply` twice, using the default NTP host
  (matching the existing smoke-test steps, rather than the differently-behaving `pool.ntp.org`).
  First, an unprivileged run is asserted to fail cleanly with a permission error — safe, no clock
  change, exercises the whole code path up to the actual syscall. Second, as the deliberately-last
  step in the job, a `sudo` run performs a real clock step and prints the time before/after —
  since a hard clock step could disrupt any later network/TLS-dependent step (cert validity
  checks, log/artifact upload), this must never run before other steps. Both steps treat a
  network-related failure (e.g. `EAGAIN`/"Resource temporarily unavailable" from the client's
  5s read timeout) as a warning rather than a hard CI failure, consistent with the pre-existing,
  documented unreliability of outbound UDP to public NTP servers from some CI runners (the same
  reason the `client.rs` live-network tests are `#[ignore]`d).
- System clock correction via a new `-a`/`--apply` flag (Unix only). `src/clock.rs` adds
  `should_step` (pure threshold check: skips corrections below 1ms, `MIN_STEP_THRESHOLD_NANOS`)
  and `step_clock` (a hard step to the corrected time via `clock_settime(2)`, called through
  `libc`). Requires elevated privileges (root/`CAP_SYS_TIME` on Linux, admin on macOS); a
  permission failure is a fatal error for a one-shot `--apply`, but only logged (loop continues)
  for `--sync --apply`. In `--sync --apply`, corrections use the sliding window's minimum-delay
  ("best") offset once at least 2 samples are available, never the raw single-poll offset. This
  is a hard step, not a gradual slew — see the README's Precision & Limitations and the updated
  Roadmap.
- Periodic re-synchronization with clock-drift compensation via a new `-s`/`--sync` flag.
  Continuously polls the server at a fixed interval (`-i`/`--interval`, default 64s). A failed
  poll is logged and does not stop the loop; `Ctrl-C` stops it (uses the default OS SIGINT
  behavior already restored earlier).
- `src/drift.rs`: `SlidingWindow`, holding the last 8 samples (`WINDOW_SIZE`, matching RFC 5905's
  clock-filter shift-register size). `SlidingWindow::best_offset` returns the sample with the
  lowest observed round-trip delay (the same intuition as NTP's clock filter: low delay implies
  a more symmetric, more trustworthy path). `SlidingWindow::estimate_drift` computes the drift
  rate via ordinary least-squares linear regression of offset against local time across all
  samples in the window, instead of a naive two-point difference — a unit test demonstrates this
  stays close to the true drift rate under per-sample jitter that flips the sign of a two-point
  estimate. Plus a pure `extrapolate_offset` function. All decoupled from I/O and the system
  clock, with unit tests. `--sync`'s output includes the per-poll raw offset/delay, the window's
  best offset, the drift estimate, and the current window fill level.
- `Client`'s `DEFAULT_NTP_PORT` constant is now `pub(crate)`, reused by `parser.rs` for the
  `--verbose` request-announcement line instead of a duplicated literal.
- Round-trip delay and clock-offset calculation, wired into the CLI via a new `-o`/`--offset` flag and
  included in `-v`/`--verbose` output. `Client::request` now records the local send time (T1) and
  `Client::receive` records the local receive time (T4); together with the server's receive/transmit
  timestamps (T2/T3, already in the response packet) these are passed to `offset::compute` to produce a
  `SyncResult`. The default (no-flag) output is unchanged — it still prints the server's timestamp as-is,
  uncorrected.

### Fixed

- The CI `--apply` verification step misreported a passing result as a failure whenever the
  measured offset was below the 1ms step threshold: the tool correctly skips the privileged
  syscall entirely in that case ("not applying") and exits `0`, which the check had only ever
  interpreted as "succeeded without privileges" — an actual bug. Now distinguishes exit `0` with
  "not applying" (offset too small, syscall never attempted — pass) from exit `0` without it (the
  clock was actually stepped without privileges — a real failure).
- `just install` failed on macOS with `install: .bak: No such file or directory`. The `justfile` used the
  GNU-`install`-specific `-S suffix` flag to name a backup file, but BSD `install` (macOS) treats `-S` as a
  standalone boolean flag ("flush to disk") and uses `-B suffix` instead — so `.bak` was parsed as an extra
  positional source file that doesn't exist. Dropped the (non-essential) backup flags entirely so the
  recipe works identically on both `install` implementations.

## [v1.2.0] - 2026-08-09

### Added

- `justfile` as the project's command runner, replacing the `makefile`.
- `src/offset.rs`: pure clock-offset and round-trip-delay computation (RFC 5905, section 8),
  decoupled from sockets and the system clock, with unit tests.
- Bidirectional conversion between NTP `Timestamp` and `std::time::Duration`
  (`Timestamp::to_unix_duration`, `Timestamp::from_unix_duration`, `Timestamp::now`).
- Separate CI workflows for Linux (`linux.yml`) and macOS (`macos.yml`), replacing the single
  matrix-based workflow. Both now build debug and release, run the full test suite
  (`cargo test`), and smoke-test the resulting binaries — triggered on every push/pull request
  instead of only on `main`.
- `CONTRIBUTING.md` documenting the fork/branch/PR workflow, coding guidelines, and commit
  message conventions.
- `Cargo.toml` package metadata: `description`, `license`, `repository`, `readme`.

### Changed

- Reworked `README.md`: accurate feature list, an explicit roadmap with checkboxes, real usage
  examples, and a development/contributing section.
- `Dockerfile` and CI now use `just` instead of `make`.
- `Dockerfile` base image bumped from `rust:1.48` to `rust:latest`. The old image was based on
  Debian Buster, which has since gone EOL — its `apt-get` mirrors were moved to
  `archive.debian.org`, so `apt-get update` failed with 404s and broke the Docker CI build.
- Updated `chrono` to `0.4.45`.
- `Client` now sets a 5-second read timeout on its UDP socket instead of blocking indefinitely
  for a server response; `Client::receive` propagates the timeout as a normal `Result::Err`
  instead of panicking.
- Marked the two `client.rs` tests that require live outbound UDP connectivity to a public NTP
  pool server as `#[ignore]`, since that connectivity is not reliably available on every CI
  runner (observed as an indefinite hang on macOS CI). Run them manually via
  `cargo test -- --ignored`.
- Migrated CLI argument parsing from `clap 2` (builder API) to `clap 4` (derive API). The CLI
  name, version, author, and description are now derived directly from `Cargo.toml` instead of
  a set of hand-maintained, independently-drifting constants (`Cargo.toml` had `1.1.0`, the CLI
  reported `1.0.1`, and the `README.md` example showed `1.1.3`). `Cargo.toml` version bumped to
  `1.2.0` as the single source of truth going forward.
- `Client::new` and `Client::request` now return `Result` instead of panicking via `.unwrap()`
  / `.expect()` on socket errors. `main` propagates errors up through `Parser::evaluate` and
  exits with a non-zero `std::process::ExitCode` on failure instead of always exiting `0`.

### Removed

- Removed the `time` crate dependency ([RUSTSEC-2020-0071](https://rustsec.org/advisories/RUSTSEC-2020-0071.html));
  the transitive pull-in via `chrono` is also gone after the version bump.
- Removed the `makefile`.
- Removed the `nix` dependency ([RUSTSEC-2021-0119](https://rustsec.org/advisories/RUSTSEC-2021-0119.html))
  along with the custom `SIGINT` handler it was used for. That handler intercepted `Ctrl-C` and
  printed `"Unable to quit."` without ever exiting, so the process couldn't be interrupted.
  `Ctrl-C` now uses the default OS behavior and terminates the process immediately.

## [v1.1.3] - 2021-02-18

### Changed

- Refactoring of the timestamp member: seconds/fractions replaced with Timestamp type.

## [v1.1.2] - 2021-01-13

### Added

- Support for alternate bind-address, when creating the network socket.
- Commandline argument for the bind-address added.


## [v1.1.1] - 2021-01-12

### Fixed

- Bitmask for received mode field (byte) fixed. Received byte was not masked properly.

### Removed

- Removed function: `set_leap_indicator`, `set_version`, `set_operation_mode`.

### Added

- Mask for received mode byte.

## [v1.1.0] - 2021-01-07

### Added

- New module ``parser.rs`` added.
- Refactored the argument parsing to `parser.rs`.

### Changed

- Derive Clone, Copy for the `NTP` in `ntp.rs`. 

## [v1.0.0] - 2021-01-04

### Features

- Basic NTP-request, which returns UTC-time.
- Flag for printing debugging information about the received NTP packet.
- Flag for printing additional terminal output.