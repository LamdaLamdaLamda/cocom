# Synchronization and Clock Correction

Deep-dive documentation for `-o`/`--offset`, `-s`/`--sync`, and `-a`/`--apply`. For a quick overview,
usage examples, and installation, see the [README](../README.md).

## Table of Contents

- [Round-trip delay and clock offset](#round-trip-delay-and-clock-offset)
- [Sliding-window drift estimation](#sliding-window-drift-estimation)
- [System clock correction (`--apply`)](#system-clock-correction---apply)
- [State persistence (`--state-file`)](#state-persistence---state-file)

## Round-trip delay and clock offset

Every NTP exchange involves four timestamps ([RFC 5905, section 8](https://tools.ietf.org/html/rfc5905#section-8)):

- **T1** — local time the request was sent (recorded by `Client::request`)
- **T2** — server time the request was received (`rx_timestamp` in the response packet)
- **T3** — server time the response was sent (`tx_timestamp` in the response packet)
- **T4** — local time the response was received (recorded by `Client::receive`)

Two different questions get answered from these:

**Clock offset** — "how far apart are the two clocks?"

```
offset = ((T2 − T1) + (T3 − T4)) / 2
```

Positive means the local clock is behind the server; negative means it's ahead.

**Round-trip delay** — "how long did the network round trip take?" (server processing time removed)

```
delay = (T4 − T1) − (T3 − T2)
```

### Why both matter

The offset formula assumes the outbound and inbound legs of the trip take the same amount of time
(symmetric latency). Real networks rarely guarantee that exactly — asymmetric routing, queueing
differences in each direction, etc. Any deviation from that assumption directly biases the computed
offset, roughly by up to half the asymmetry.

The delay is therefore not just a network statistic — it's a **confidence indicator** for the offset. A
small, stable delay suggests a symmetric, clean path, so the offset is trustworthy. A large or
fluctuating delay means the offset should be treated with more caution.

This computation lives in [`src/offset.rs`](../src/offset.rs) as a pure function
(`offset::compute(t1, t2, t3, t4) -> SyncResult`), fully decoupled from sockets and the system clock so
it's unit-testable with fixed inputs.

## Sliding-window drift estimation

A single offset measurement only describes "how wrong the clock was at this instant." Because no local
oscillator runs at exactly the nominal frequency, that measurement goes stale immediately — the clock
keeps drifting away from correct at some rate (**drift**, typically expressed in ppm).

### The problem with a naive two-point estimate

The obvious way to estimate drift is to compare the two most recent offset measurements:

```
drift = (offset₂ − offset₁) / (t₂ − t₁)
```

The problem: every individual offset measurement already carries network jitter as noise (see above).
A two-point difference trusts both of the last two measurements completely — one noisy poll is enough
to produce a wildly wrong (even sign-flipped) drift estimate. This was directly observed in practice:
over a handful of polls at a short interval, the two-point estimate swung between roughly -13,000 ppm
and +13,500 ppm, nowhere near a physically plausible drift rate.

### The fix: a sliding window with minimum-delay filtering

[`src/drift.rs`](../src/drift.rs)'s `SlidingWindow` keeps the last `WINDOW_SIZE` (8, matching the
classic NTP clock-filter shift-register size) samples — each an `{ local_time, offset, delay }` triple
— and offers two operations:

- **`best_offset()`** returns the sample with the **lowest observed delay** in the window, not the most
  recent one. Same intuition as the offset/delay relationship above: low delay implies a more
  trustworthy measurement. This is the same principle behind NTP's own clock-filter algorithm.
- **`estimate_drift()`** computes the drift rate via **ordinary least-squares linear regression** of
  offset against local time, across *all* samples currently in the window — not just the last two.
  Local timestamps are normalized relative to the oldest sample in the window before the regression, to
  keep the arithmetic well-behaved regardless of how large the underlying Unix-epoch nanosecond values
  are.

A unit test (`test_regression_resistanttest`) demonstrates the improvement concretely: given a true
5000 ppm drift with small per-sample jitter added (roughly matching observed real-world network noise),
a naive two-point estimate using the last two samples comes out negative — wrong sign entirely — while
the windowed regression across all 8 samples stays close to the true rate.

Live behavior mirrors this: over a real `--sync` run, the drift estimate started at -3705 ppm with only
2 samples in the window and settled down to -91 ppm once the window filled to 8/8 — even after a
noticeably jittery poll partway through (-27ms offset, 107ms delay) that `best_offset()` correctly
ignored in favor of a low-delay sample from earlier in the window.

### What this is *not*

It's a deliberately simplified version of what `chrony`/`ntpd` do:

- No delay-threshold filtering beyond "pick the single minimum" — no weighted regression, no discarding
  samples above some delay cutoff.
- No adaptive poll interval (`ntpd` grows/shrinks its polling interval based on measured stability).
- Fixed window size, not a configurable or adaptive one.

## System clock correction (`--apply`)

By default Cocom only measures and reports. `-a`/`--apply` opts in to actually correcting the system
clock, and picks one of four outcomes based on the offset's magnitude:

```
Offset magnitude:   0 ─────── 1ms ─────────── 128ms ─────────── 1000s ─────── ∞
Action:             Skip   │       Slew        │        Step        │  Refuse
                             (adjtime, gradual)   (clock_settime, hard)
```

### Skip (`< 1ms`, `clock::MIN_STEP_THRESHOLD_NANOS`)

Not worth correcting — the disruption of touching the clock isn't justified for a sub-millisecond
offset. Nothing happens.

### Slew (`1ms` to `128ms`, `clock::MAX_SLEW_THRESHOLD_NANOS`)

Applied via `adjtime(2)` (`clock::slew_clock`). The kernel gradually absorbs the offset by running the
clock very slightly fast or slow — bounded to roughly 500 ppm — until the correction is complete. The
clock stays **monotonically increasing** throughout; it never jumps.

The 128ms boundary is classic `ntpd`'s own step/slew threshold. It falls out of the slew rate: at
500 ppm, correcting a 128ms offset takes roughly 256 seconds (~4 minutes) — still practical. A full
second of offset would take ~33 minutes, which is why slewing isn't used unconditionally for every
offset size.

### Step (`128ms` to `1000s`, `clock::PANIC_THRESHOLD_NANOS`)

Applied via `clock_settime(2)` (`clock::step_clock`) — an instant jump to the corrected time. Simple and
immediate, but can move timestamps **backwards**, which some software doesn't expect (file mtime
ordering, log ordering, naive duration calculations using wall-clock time instead of `Instant`).

### Refuse (`> 1000s`, unless `-f`/`--force-large-step`)

Not a size-based tier so much as a safety cap layered on top of Step. Cocom trusts the queried server's
response completely — there's no multi-server comparison and no response authentication (no NTS or
symmetric-key auth, see the [README Roadmap](../README.md#roadmap)) — so a misconfigured or spoofed
server is the only thing standing between "plausible correction" and "silently set the clock to
whenever." The 1000-second bound is `ntpd`'s own classic "panic" threshold: refuse instead of silently
complying, and require an explicit override (`--force-large-step`) to proceed anyway.

### Boundary semantics

Each threshold is inclusive on the *lower* tier's side: exactly `1ms` is already `Slew` (not `Skip`),
exactly `128ms` is still `Slew` (not yet `Step`), exactly `1000s` is still `Step` (not yet `Refuse`).
Only the panic threshold is a strict `>` comparison. See `clock.rs`'s unit tests for the exact boundary
coverage.

### How this composes with `--sync`

In `--sync --apply`, corrections use `SlidingWindow::best_offset()` — the filtered, minimum-delay
estimate — not the raw per-poll offset, and only once the window holds at least 2 samples. A failed
correction attempt (e.g. missing privileges) is logged and doesn't stop the polling loop; for a one-shot
`--apply` (no `--sync`), the same failure is fatal.

### What this is *not*

- No true PLL/FLL frequency discipline the way the kernel's own NTP subsystem (`ntp_adjtime`/`adjtimex`
  on Linux) or `chrony`/`ntpd` implement — this is a simpler one-shot-per-poll slew/step decision, not a
  continuously-tuned frequency correction.

## State persistence (`--state-file`)

Without `--state-file`, the `SlidingWindow` lives purely in process memory — a restart (crash, reboot,
service restart, or simply invoking Cocom fresh each time) throws away all history and starts
"warming up" from 0/8 again.

`--state-file <PATH>` opts in to persisting the window's samples to a plain text file (one
`local_time_nanos offset_nanos delay_nanos` line per sample, oldest first — see
[`src/state.rs`](../src/state.rs)). No serialization library is used: three plain integers don't need
one, and `i128` doesn't round-trip cleanly through most JSON parsers anyway (JSON numbers are commonly
backed by `f64`). The format is human-inspectable on purpose.

### Behavior

- **`--sync --state-file <PATH>`**: loads the file on startup (if present) to pre-populate the window,
  and saves it back after every poll. Persists regardless of whether `--apply` is also set — even in
  observe-only mode, a warm start means a more stable drift estimate immediately instead of the noisy
  first few polls after every restart.
- **One-shot `-a`/`--apply --state-file <PATH>` (no `--sync`)**: loads the file, adds this run's fresh
  measurement, uses the window's minimum-delay ("best") offset as the value actually applied (which is
  just the fresh measurement itself on the first run, before any history exists), and saves the updated
  window back. This lets repeated one-shot invocations — e.g. `cocom -a --state-file ... ` on a cron
  schedule instead of a long-running `--sync` process — benefit from much of the same filtering quality,
  without a persistent daemon. Without `--apply`, a one-shot invocation has no use for a window at all,
  so `--state-file` alone (no `-a`) has no effect outside `--sync`.
- A missing, empty, or malformed state file is treated as a cold start, not an error — Cocom just starts
  with an empty window and carries on.
- **Staleness**: if the newest sample in the file is older than `state::MAX_SAMPLE_AGE_NANOS` (1 hour),
  the whole window is discarded on load instead of trusted. Conditions (the network path, the actual
  clock drift) may have changed too much for old samples to still be meaningful.
- A failed write (e.g. an unwritable path) is logged and does not stop `--sync`'s polling loop or fail a
  one-shot invocation on its own — persistence is a best-effort enhancement, not a correctness
  requirement.

Verified across a real restart: after 4 polls (~12s) `--sync --state-file` was interrupted and restarted
against the same file. The second run logged `Loaded 4 persisted sample(s)`, resumed at window 5/8
instead of 1/8, and its drift estimate was immediately far more stable (tens of ppm) than the typical
cold-start swings (thousands of ppm) seen in the first few polls of a fresh run.
