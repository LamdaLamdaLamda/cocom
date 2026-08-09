# Cocom

[![Linux](https://github.com/LamdaLamdaLamda/cocom/actions/workflows/linux.yml/badge.svg)](https://github.com/LamdaLamdaLamda/cocom/actions/workflows/linux.yml)
[![macOS](https://github.com/LamdaLamdaLamda/cocom/actions/workflows/macos.yml/badge.svg)](https://github.com/LamdaLamdaLamda/cocom/actions/workflows/macos.yml)
[![Docker Image CI](https://github.com/LamdaLamdaLamda/cocom/actions/workflows/docker-image.yml/badge.svg)](https://github.com/LamdaLamdaLamda/cocom/actions/workflows/docker-image.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPL%20v3-blue.svg)](LICENSE)

Cocom is a minimal [NTP](http://www.ntp.org/) client, written entirely in Rust. It speaks the NTP wire protocol
directly over UDP — no `ntpd`, `chrony`, `systemd-timesyncd`, or third-party NTP library involved.

## Why

- **No system daemon required.** Cocom talks UDP directly to an NTP server, which makes it a good fit for
  embedded targets and minimal Linux environments where running a full NTP daemon isn't practical.
- **Single, portable binary.** As a Rust binary it cross-compiles to x86, ARM, and RISC-V without code changes.
- **Memory-safe by construction.** Unlike `ntpd`/`chrony` (C), packet parsing happens in safe Rust — no
  buffer overflows, no use-after-free.

## Features

- Sends an NTP client-mode request (RFC 5905) and parses the 48-byte response packet
- Configurable NTP server (defaults to the [PTB Braunschweig](https://www.ptb.de) time server)
- Configurable UDP bind address
- Verbose and debug output modes for inspecting raw NTP packet fields

> **Note:** Cocom currently performs a single request/response exchange and reports the server's timestamp.
> Round-trip delay/clock-offset calculation and periodic re-synchronization are on the [roadmap](#roadmap)
> but not implemented yet — see the [Changelog](CHANGELOG.md) for what has actually shipped.

## Installation

### Build from source

Requires [`just`](https://just.systems) as the command runner.

```sh
just build && just install
```

This installs Cocom to `/usr/local/bin`; the install step may need elevated privileges. To build and run
without installing:

```sh
just run-dev   # debug build
just run       # release build
```

### Docker

```sh
docker build -t cocom . && docker run cocom
```

## Usage

If no host is given, Cocom queries the default NTP server. With no flags, it prints the received time.

```
NTP-Client purely written in Rust.

Usage: cocom [OPTIONS] [HOST]

Arguments:
  [HOST]  Specifies the desired NTP-server

Options:
  -b, --bind <BIND>  Specifies the binding address for the UDP socket. The following format is required; [IP]:[PORT]
  -v, --verbose      Activates terminal output
  -d, --debug        Prints the fields of the received NTP-packet
  -h, --help         Print help
  -V, --version      Print version
```

Examples:

```sh
# Query the default server, print the parsed date/time
cocom

# Query a specific pool server
cocom pool.ntp.org

# Bind the UDP socket to a specific local address/port and show raw packet fields
cocom -b 0.0.0.0:12345 -d pool.ntp.org
```

## How it works

Cocom builds a 48-byte NTP packet in client mode, sends it over UDP to the target server's port 123, and
parses the returned packet's timestamp fields. See [RFC 5905, section 7](https://tools.ietf.org/html/rfc5905#section-7)
for the wire format.

## Roadmap

- [ ] Round-trip delay and clock-offset calculation from the four NTP timestamps
- [ ] Periodic re-synchronization with drift compensation
- [ ] Dependency modernization (replace unmaintained/advisory-flagged crates)

## Development

Recipes are defined in the [justfile](justfile) and run via [`just`](https://just.systems).

| Command          | Description                                  |
|------------------|-----------------------------------------------|
| `just build-dev` | Debug build                                   |
| `just build`     | Release build                                 |
| `just run-dev`   | Build and run (debug)                         |
| `just run`       | Build and run (release)                       |
| `just test`      | Run the test suite                            |
| `just doc`       | Generate documentation into `doc/`            |
| `just install`   | Install the release binary to `$PREFIX/bin`   |

## Contributing

Issues and pull requests are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for the full process. Please
run `just test` before submitting a change, and keep packet parsing/serialization changes aligned with
[RFC 5905](https://tools.ietf.org/html/rfc5905#section-7).

## License

GPL-3.0 — see [LICENSE](LICENSE).

## Further reading

- [NTP.org](http://www.ntp.org/)
- [RFC 5905](https://tools.ietf.org/html/rfc5905#section-7)
- [NTP Pool Project](https://www.ntppool.org/en/)
