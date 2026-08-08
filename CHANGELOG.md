# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

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

### Changed

- Reworked `README.md`: accurate feature list, an explicit roadmap with checkboxes, real usage
  examples, and a development/contributing section.
- `Dockerfile` and CI now use `just` instead of `make`.
- Updated `chrono` to `0.4.45`.
- `Client` now sets a 5-second read timeout on its UDP socket instead of blocking indefinitely
  for a server response; `Client::receive` propagates the timeout as a normal `Result::Err`
  instead of panicking.
- Marked the two `client.rs` tests that require live outbound UDP connectivity to a public NTP
  pool server as `#[ignore]`, since that connectivity is not reliably available on every CI
  runner (observed as an indefinite hang on macOS CI). Run them manually via
  `cargo test -- --ignored`.

### Removed

- Removed the `time` crate dependency ([RUSTSEC-2020-0071](https://rustsec.org/advisories/RUSTSEC-2020-0071.html));
  the transitive pull-in via `chrono` is also gone after the version bump.
- Removed the `makefile`.

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