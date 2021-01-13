# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v1.1.2] - 2020-01-13

### Added

- Support for alternate bind-address, when creating the network socket.
- Commandline argument for the bind-address added.


## [v1.1.1] - 2020-01-12

### Fixed

- Bitmask for received mode field (byte) fixed. Received byte was not masked properly.

### Removed

- Removed function: `set_leap_indicator`, `set_version`, `set_operation_mode`.

### Added

- Mask for received mode byte.

## [v1.1.0] - 2020-01-07

### Added

- New module ``parser.rs`` added.
- Refactored the argument parsing to `parser.rs`.

### Changed

- Derive Clone, Copy for the `NTP` in `ntp.rs`. 

## [v1.0.0] - 2020-01-04

### Features

- Basic NTP-request, which returns UTC-time.
- Flag for printing debugging information about the received NTP packet.
- Flag for printing additional terminal output.