//! System clock correction. Applies a measured offset either as a gradual slew
//! (`adjtime(2)`, the clock stays monotonically increasing throughout, never jumps) or a
//! hard step (`clock_settime(2)`, instant but can move timestamps backwards), depending on
//! the offset's magnitude — matching classic `ntpd`'s step/slew split. Requires elevated
//! privileges (`CAP_SYS_TIME`/root on Linux, admin on macOS).
//!
//! The decision of *how* to handle a given offset is a pure function, kept separate from
//! the actual (unsafe, OS-mutating) syscalls so it can be unit-tested.

use std::io::{Error, ErrorKind};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Minimum absolute offset worth correcting at all. Below this, a correction is skipped —
/// not worth the disruption for a fraction of a millisecond.
pub const MIN_STEP_THRESHOLD_NANOS : i128 = 1_000_000; // 1 ms

/// Offset magnitude up to which a gradual slew is used instead of a hard step, matching
/// classic `ntpd`'s step/slew boundary. Above this, a slew (bounded to roughly 500 ppm) would
/// take impractically long to catch up, so a step is used instead.
pub const MAX_SLEW_THRESHOLD_NANOS : i128 = 128_000_000; // 128 ms, ntpd's classic threshold

/// Offset magnitude above which a correction is refused by default, matching classic `ntpd`'s
/// "panic" behavior. Cocom trusts the server's response completely and has no multi-server
/// comparison to catch a misconfigured or spoofed one — this bound is the last line of defense
/// against silently correcting the clock by an implausible amount. Override with the CLI's
/// `--force-large-step`.
pub const PANIC_THRESHOLD_NANOS : i128 = 1_000_000_000_000; // 1000 s, ntpd's classic default

/// How `plan_correction` decided to handle a given offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Correction {
    /// Offset is below `MIN_STEP_THRESHOLD_NANOS` — not worth correcting.
    Skip,
    /// Offset is small enough to gradually slew (`slew_clock`).
    Slew,
    /// Offset is large enough that a slew would take too long — step instead (`step_clock`).
    Step,
    /// Offset exceeds `PANIC_THRESHOLD_NANOS` and `force_large_step` was not set — refused.
    Refuse,
}

/// Decides how an offset should be handled: skipped, slewed, stepped, or refused outright.
pub fn plan_correction(offset_nanos : i128, force_large_step : bool) -> Correction {
    let magnitude : u128 = offset_nanos.unsigned_abs();

    if magnitude > PANIC_THRESHOLD_NANOS as u128 && !force_large_step {
        return Correction::Refuse;
    }
    if magnitude < MIN_STEP_THRESHOLD_NANOS as u128 {
        return Correction::Skip;
    }
    if magnitude <= MAX_SLEW_THRESHOLD_NANOS as u128 {
        return Correction::Slew;
    }
    Correction::Step
}

/// Steps the system clock directly to `SystemTime::now() + offset_nanos`. Instant, but can
/// move timestamps backwards.
///
/// Returns an `Error` if the underlying `clock_settime` call fails — most commonly due
/// to insufficient privileges.
#[cfg(unix)]
pub fn step_clock(offset_nanos : i128) -> Result<(), Error> {
    let now : SystemTime = SystemTime::now();
    let corrected : SystemTime = if offset_nanos >= 0 {
        now + Duration::from_nanos(offset_nanos as u64)
    } else {
        now - Duration::from_nanos((-offset_nanos) as u64)
    };

    let since_epoch : Duration = corrected.duration_since(UNIX_EPOCH)
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "corrected time predates the Unix epoch"))?;

    let ts = libc::timespec {
        tv_sec : since_epoch.as_secs() as libc::time_t,
        tv_nsec : since_epoch.subsec_nanos() as _,
    };

    // SAFETY: `ts` is a fully-initialized `timespec` living on the stack for the duration
    // of this call; `clock_settime` only reads through the pointer and does not retain it.
    let result : libc::c_int = unsafe { libc::clock_settime(libc::CLOCK_REALTIME, &ts) };

    if result == 0 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

/// Gradually adjusts the clock by `offset_nanos` via `adjtime(2)`. The kernel absorbs the
/// correction over time at a bounded rate (traditionally ~500 ppm) rather than instantly —
/// the clock stays monotonically increasing throughout, unlike `step_clock`. `adjtime`'s
/// `timeval` only has microsecond resolution, so any sub-microsecond part of `offset_nanos`
/// is truncated.
///
/// Returns an `Error` if the underlying `adjtime` call fails — most commonly due to
/// insufficient privileges.
#[cfg(unix)]
pub fn slew_clock(offset_nanos : i128) -> Result<(), Error> {
    let micros : i64 = (offset_nanos / 1_000) as i64;
    let sec : i64 = micros.div_euclid(1_000_000);
    let usec : i64 = micros.rem_euclid(1_000_000);

    let delta = libc::timeval {
        tv_sec : sec as libc::time_t,
        tv_usec : usec as _,
    };

    // SAFETY: `delta` is a fully-initialized `timeval` living on the stack for the duration
    // of this call; passing `null_mut()` for the previous-adjustment output means we don't
    // need to inspect it. `adjtime` only reads through the first pointer and does not retain
    // either.
    let result : libc::c_int = unsafe { libc::adjtime(&delta, std::ptr::null_mut()) };

    if result == 0 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_plan_correction_skip_below_min() {
        assert_eq!(plan_correction(500_000, false), Correction::Skip);
    }

    #[test]
    fn test_plan_correction_skip_negative_below_min() {
        assert_eq!(plan_correction(-500_000, false), Correction::Skip);
    }

    #[test]
    fn test_plan_correction_skip_zero() {
        assert_eq!(plan_correction(0, false), Correction::Skip);
    }

    #[test]
    fn test_plan_correction_slew_at_min_threshold() {
        assert_eq!(plan_correction(MIN_STEP_THRESHOLD_NANOS, false), Correction::Slew);
    }

    #[test]
    fn test_plan_correction_slew_typical_offset() {
        assert_eq!(plan_correction(50_000_000, false), Correction::Slew); // 50 ms
    }

    #[test]
    fn test_plan_correction_slew_negative_typical_offset() {
        assert_eq!(plan_correction(-50_000_000, false), Correction::Slew);
    }

    #[test]
    fn test_plan_correction_slew_at_max_slew_threshold() {
        assert_eq!(plan_correction(MAX_SLEW_THRESHOLD_NANOS, false), Correction::Slew);
    }

    #[test]
    fn test_plan_correction_step_just_above_slew_threshold() {
        assert_eq!(plan_correction(MAX_SLEW_THRESHOLD_NANOS + 1, false), Correction::Step);
    }

    #[test]
    fn test_plan_correction_step_typical_offset() {
        assert_eq!(plan_correction(10_000_000_000, false), Correction::Step); // 10 s
    }

    #[test]
    fn test_plan_correction_step_at_panic_threshold() {
        assert_eq!(plan_correction(PANIC_THRESHOLD_NANOS, false), Correction::Step);
    }

    #[test]
    fn test_plan_correction_refuse_above_panic_threshold() {
        assert_eq!(plan_correction(PANIC_THRESHOLD_NANOS + 1, false), Correction::Refuse);
    }

    #[test]
    fn test_plan_correction_refuse_large_negative_offset() {
        assert_eq!(plan_correction(-(PANIC_THRESHOLD_NANOS + 1), false), Correction::Refuse);
    }

    #[test]
    fn test_plan_correction_force_overrides_refuse() {
        assert_eq!(plan_correction(PANIC_THRESHOLD_NANOS + 1, true), Correction::Step);
    }
}
