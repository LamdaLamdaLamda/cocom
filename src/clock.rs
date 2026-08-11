//! System clock correction. Applies a measured offset via `clock_settime(2)` — a hard
//! step, not a gradual slew (see the README's Precision & Limitations for why that's a
//! deliberate simplification). Requires elevated privileges (`CAP_SYS_TIME`/root on
//! Linux, admin on macOS).
//!
//! The decision of *whether* an offset is worth stepping for is a pure function, kept
//! separate from the actual (unsafe, OS-mutating) syscall so it can be unit-tested.

use std::io::{Error, ErrorKind};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Minimum absolute offset worth stepping the clock for. Below this, a correction is
/// skipped — not worth the disruption of a clock step for a fraction of a millisecond.
pub const MIN_STEP_THRESHOLD_NANOS : i128 = 1_000_000; // 1 ms

/// Decides whether an offset is large enough to justify stepping the system clock.
pub fn should_step(offset_nanos : i128) -> bool {
    offset_nanos.unsigned_abs() >= MIN_STEP_THRESHOLD_NANOS as u128
}

/// Steps the system clock directly to `SystemTime::now() + offset_nanos`.
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_should_step_true_above_threshold() {
        assert!(should_step(5_000_000));
    }

    #[test]
    fn test_should_step_true_for_negative_offset_above_threshold() {
        assert!(should_step(-5_000_000));
    }

    #[test]
    fn test_should_step_false_below_threshold() {
        assert!(!should_step(500_000));
    }

    #[test]
    fn test_should_step_false_for_negative_offset_below_threshold() {
        assert!(!should_step(-500_000));
    }

    #[test]
    fn test_should_step_false_for_zero() {
        assert!(!should_step(0));
    }

    #[test]
    fn test_should_step_true_exactly_at_threshold() {
        assert!(should_step(MIN_STEP_THRESHOLD_NANOS));
    }
}
