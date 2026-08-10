//! Pure clock-offset and round-trip-delay computation, decoupled from sockets and the
//! system clock so it can be unit-tested with fixed inputs.
//!
//! See [RFC 5905, section 8](https://tools.ietf.org/html/rfc5905#section-8) for the formulas.

/// Result of comparing the four NTP protocol timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncResult {
    /// Clock offset in nanoseconds. Positive means the local clock is behind the server.
    pub offset : i128,

    /// Round-trip delay in nanoseconds.
    pub delay : i128,
}

/// Computes clock offset and round-trip delay from the four NTP timestamps, each expressed
/// as nanoseconds since the Unix epoch.
///
/// - `t1`: local time the request was sent (originate timestamp)
/// - `t2`: server time the request was received
/// - `t3`: server time the response was sent
/// - `t4`: local time the response was received
pub fn compute(t1 : i128, t2 : i128, t3 : i128, t4 : i128) -> SyncResult {
    SyncResult {
        offset: ((t2 - t1) + (t3 - t4)) / 2,
        delay: (t4 - t1) - (t3 - t2),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_compute_zero_offset_zero_delay() {
        let result : SyncResult = compute(1_000, 1_000, 1_000, 1_000);

        assert_eq!(result.offset, 0);
        assert_eq!(result.delay, 0);
    }

    #[test]
    fn test_compute_symmetric_delay_no_offset() {
        // Request takes 10ns out, server takes 5ns to process, response takes 10ns back.
        // t1=0, t2=10, t3=15, t4=25
        let result : SyncResult = compute(0, 10, 15, 25);

        assert_eq!(result.offset, 0);
        assert_eq!(result.delay, 20);
    }

    #[test]
    fn test_compute_positive_offset_local_clock_behind() {
        // Server clock is 100ns ahead of the local clock, network delay is negligible.
        let result : SyncResult = compute(0, 100, 100, 0);

        assert_eq!(result.offset, 100);
        assert_eq!(result.delay, 0);
    }

    #[test]
    fn test_compute_negative_offset_local_clock_ahead() {
        // Server clock is 100ns behind the local clock, network delay is negligible.
        let result : SyncResult = compute(0, -100, -100, 0);

        assert_eq!(result.offset, -100);
        assert_eq!(result.delay, 0);
    }

    #[test]
    fn test_compute_rfc5905_worked_example() {
        // t1=0, t2=8, t3=9, t4=20 -> delay = (20-0)-(9-8) = 19, offset = ((8-0)+(9-20))/2 = -1.5 -> -1 (truncated)
        let result : SyncResult = compute(0, 8, 9, 20);

        assert_eq!(result.delay, 19);
        assert_eq!(result.offset, -1);
    }
}
