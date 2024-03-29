# Cocom - NTP client written in Rust [![Cocom build](https://github.com/LamdaLamdaLamda/cocom/actions/workflows/build.yml/badge.svg)](https://github.com/LamdaLamdaLamda/cocom/actions/workflows/build.yml) [![License: GPL v3](https://img.shields.io/badge/License-GPL%20v3-blue.svg)](http://www.gnu.org/licenses/gpl-3.0)


Cocom is an implementation of the NTP-protocol, to receive 
the time from NTP-server. The client does not necessarily need arguments.

The implementation does not use any NTP-libraries.

For further information on NTP, see [RFC-5905](https://tools.ietf.org/html/rfc5905#section-7).

## Setup
The installation can be applied by running the following [makefile](makefile) target:

```
make build && make install 
```

Cocom will be installed to **/usr/local/bin/** and the installation target might need higher access privileges. Therefore, it is supposed to be in the global interpreter path. 
If an installation is not intended, it can be run locally. As a default host the NTP
server from the [PTB-Braunschweig](https://www.ptb.de) is set.

```
make run-dev
```

Alternatively it can be run with the release profile.

```
make run
```

## Usage

Currently, Cocom expects an NTP host, if not provided the default NTP server is used. If no flag is provided the client will
 return the current timestamp from the given NTP host.

```sh
Cocom 1.1.3
LamdaLamdaLamda
NTP-Client purely written in Rust.

USAGE:
    cocom [FLAGS] [OPTIONS] [HOST]

FLAGS:
    -d, --debug      Prints the fields of the received NTP-packet.
    -h, --help       Prints help information
    -v, --verbose    Activates terminal output
    -V, --version    Prints version information

OPTIONS:
    -b, --bind <bind>    Specifies the binding address for the UDP socket. The following format is required; [IP]:[PORT]

ARGS:
    <HOST>    Specifies the desired NTP-server.
```
## Docker

A Docker-container can be build by running the `Dockerfile`:

```sh
docker build -t cocom . && docker run cocom 
```

## Documentation

The documentation can be generated with the following [makefile](makefile) target:

```sh
make doc
```

Documentation artifacts can be found in **cocom/doc**.

## Licence

See [LICENSE](LICENSE).

## Further-Reading

[NTP.org](http://www.ntp.org/) 

[RFC-5905](https://tools.ietf.org/html/rfc5905#section-7)

[NTP-Pool](https://www.ntppool.org/en/)