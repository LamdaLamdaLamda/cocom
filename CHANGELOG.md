# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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