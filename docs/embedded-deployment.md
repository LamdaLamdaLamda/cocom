# Embedded Deployment

Guidance for running Cocom on embedded Linux/UNIX targets — read-only root filesystems, static
IPs instead of DNS, supervision without `systemd`, and auth-key provisioning across a device
fleet. This is operational guidance, not a design doc — see
[sync-and-clock-correction.md](sync-and-clock-correction.md) and
[ntp-response-authentication.md](ntp-response-authentication.md) for how the underlying
mechanisms work.

## Table of Contents

- [Read-only root filesystems](#read-only-root-filesystems)
- [Static IPs instead of DNS](#static-ips-instead-of-dns)
- [Running without systemd](#running-without-systemd)
- [Auth-key provisioning for a device fleet](#auth-key-provisioning-for-a-device-fleet)
- [Resource footprint](#resource-footprint)
- [Known caveats on embedded targets](#known-caveats-on-embedded-targets)

## Read-only root filesystems

Two flags touch the filesystem, and they have opposite requirements:

- `--auth-key-file <PATH>` is **read-only at startup** — loaded once in `Parser::evaluate`, never
  written back. It works unmodified on a read-only rootfs or a read-only config partition; ship
  it wherever the rest of the device's static configuration lives.
- `--state-file <PATH>` is **read-write** — `--sync` rewrites it after every poll, and one-shot
  `--apply` runs read-then-write it each invocation (see
  [sync-and-clock-correction.md](sync-and-clock-correction.md#state-persistence---state-file)).
  It needs a writable, persistent location: a dedicated data partition, or the writable upper
  layer of an overlayfs setup. A plain `tmpfs` will "work" (no write errors) but defeats the
  point — the window is empty again after every reboot, exactly the cold-start case
  `--state-file` exists to avoid.

If `--state-file` points at a path that isn't writable (or doesn't exist and can't be created),
`state::save` returns an `io::Error`, which is logged (`--sync`) or surfaced (one-shot) but never
fatal — a missing/unwritable state file degrades to "always cold start," not a crash. See
[`src/state.rs`](../src/state.rs).

**Flash wear.** `--sync`'s default 64s interval means roughly 1350 writes/day to whatever
`--state-file` points at. On a filesystem with real wear-leveling (UBIFS, JFFS2 on raw NAND;
any modern eMMC's internal FTL) this is unremarkable. On a bare partition without wear-leveling,
it isn't — either use a wear-leveled filesystem for that path, or pick a longer `--interval`.
There's no built-in write-throttling independent of the poll interval today; if this matters for
your target, it's a reasonable place to contribute (see [CONTRIBUTING.md](../CONTRIBUTING.md)).

## Static IPs instead of DNS

Cocom resolves whatever string is passed as the host via the standard library's `ToSocketAddrs`
(triggered inside `Client::request`'s `send_to` call in [`src/client.rs`](../src/client.rs)) —
a hostname goes through the system resolver exactly like any other Rust networking code.

Many embedded images either have no resolver configured at all, or bring up DNS late in boot
(after Cocom might run, if invoked early via an init script). A literal IP address sidesteps
this entirely — no `/etc/resolv.conf` dependency, no boot-order coupling to network bring-up
beyond the interface itself being up. Cocom's own default already follows this: the built-in PTB
Braunschweig host is a literal IP (`192.53.103.108`), not a hostname, precisely to avoid an
implicit DNS dependency.

Recommendation: point `--auth-key-file`-secured, self-hosted deployments at a literal IP too,
especially if Cocom runs before DNS is guaranteed to be up.

## Running without systemd

Cocom doesn't daemonize itself — a one-shot invocation exits when done, and `--sync` runs as an
ordinary foreground process until it receives `SIGINT`/`SIGTERM` (default OS signal handling,
nothing custom installed). This maps directly onto embedded init/supervision models that expect
supervised foreground processes rather than self-daemonizing ones:

- **`runit`/`s6`/`daemontools`-style supervision trees**: a `run` script that just `exec`s
  `cocom -s --apply --state-file /data/cocom/state <host>` is a complete service definition —
  the supervisor restarts it on exit/crash like any other supervised process.
- **BusyBox `init`/`inittab`**: a `respawn`-type entry works the same way.
- **Cron-style one-shot correction**: for devices that don't need continuous discipline, a
  periodic one-shot `cocom -a --state-file /data/cocom/state <host>` from `busybox crond` (or
  equivalent) benefits from the same warm-start behavior one-shot `--apply` gets from
  `--state-file` — see
  [sync-and-clock-correction.md](sync-and-clock-correction.md#state-persistence---state-file).

No unit files, no daemon-manager integration to write or maintain — the "no system daemon
required" pitch (see the [README](../README.md#why)) extends naturally to embedded
init systems that were never going to have `systemd` in the first place.

## Auth-key provisioning for a device fleet

`--auth-key-file`'s trust model — one operator-controlled server, a pre-shared symmetric key —
maps well onto a fleet of embedded devices talking to a time server you also operate (see
[ntp-response-authentication.md](ntp-response-authentication.md) for why this doesn't extend to
public servers).

- **Bake the key in at image build time.** A Buildroot post-build script or a Yocto recipe
  installing a file to a fixed path (e.g. `/etc/cocom/auth.keys`) at image-build time keeps the
  key out of any runtime provisioning step or network round-trip.
- **Permissions are the operator's responsibility.** Cocom reads the key file with
  `fs::read_to_string` and does not check or enforce its permission bits (unlike, say, `ssh`
  refusing a loose private key). Ship it root-readable-only (`chmod 600`) as part of the image
  build, the same discipline as any other embedded secret (device certs, WireGuard keys).
- **Per-device keys limit blast radius.** Because `--auth-key-file` supports multiple
  `KEYID SECRET` lines and every loaded key is checked against a response's Key ID (see
  [`src/auth.rs`](../src/auth.rs)), a fleet can use a distinct key per device, provided the
  server-side trusts every device's key. A single extracted device's key can then be revoked
  server-side without re-keying the rest of the fleet — Cocom itself is agnostic to whether the
  key file holds one shared secret or a device-unique one; that choice lives entirely in how the
  server and the image-build pipeline are set up.
- **Rotation** works the same way it does for a single host: list the outgoing key first and an
  about-to-expire incoming key alongside it, roll the fleet, then drop the old key in a
  follow-up image — see
  [ntp-response-authentication.md](ntp-response-authentication.md#key-management).

## Resource footprint

`[profile.release]` in [`Cargo.toml`](../Cargo.toml) is already tuned for a small static binary:
`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`. A local
release build on this machine comes out under 500 KB; the exact number on a given embedded target
depends on the toolchain (musl vs. glibc, target arch) — cross-compile and measure for your
actual target rather than assuming this figure transfers directly. Dependencies are deliberately
minimal (`clap`, `chrono`, `byteorder`, `libc`, plus `hmac`/`sha2`/`subtle` for
`--auth-key-file`) — no async runtime, no TLS stack, no allocator beyond the system default.

## Known caveats on embedded targets

- **No dedicated cross-compilation CI check today.** Linux/macOS CI runs on `ubuntu-latest`/
  `macos-latest` (desktop-class x86_64/arm64), not against an embedded target triple (e.g.
  `armv7-unknown-linux-musleabihf`). `adjtime`/`clock_settime` are implemented in musl, so a
  musl-target build is expected to work, but it isn't exercised in CI — verify on your actual
  target before relying on it.
- **Kiss-of-Death / Leap-Indicator are not handled.** A server signaling rate-limiting (a
  stratum-0 KoD response) or an impending leap second is processed like any other response today
  — see the [Roadmap](../README.md#roadmap) for the open multi-server-comparison item this is
  adjacent to.
- **No structured/JSON output.** `--sync`/`-v`/`-d` print human-readable lines to stdout/stderr;
  if your fleet's log pipeline wants structured records, that currently means parsing this
  output rather than consuming a native format.
