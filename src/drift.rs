//! Sliding-window drift-rate estimation and offset extrapolation for periodic
//! re-synchronization, decoupled from I/O and the system clock so it can be
//! unit-tested with fixed inputs.

use std::collections::VecDeque;

/// A single clock-offset measurement at a point in local time, together with the
/// round-trip delay observed for that measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Local time of the measurement, in nanoseconds since the Unix epoch.
    pub local_time_nanos : i128,
    /// Measured clock offset at that time, in nanoseconds (see `offset::SyncResult`).
    pub offset_nanos : i128,
    /// Round-trip delay observed for this measurement, in nanoseconds.
    pub delay_nanos : i128,
}

/// Number of most recent samples kept for drift estimation and minimum-delay filtering,
/// matching the classic NTP clock-filter shift-register size ([RFC 5905, section 10](https://tools.ietf.org/html/rfc5905#section-10)).
pub const WINDOW_SIZE : usize = 8;

/// A bounded history of the most recent offset samples. Used to make `--sync`'s offset and
/// drift readings resistant to per-exchange network jitter, instead of trusting only the
/// two most recent raw measurements.
pub struct SlidingWindow {
    samples : VecDeque<Sample>,
}

impl SlidingWindow {
    /// Instantiates an empty `SlidingWindow`.
    pub fn new() -> Self {
        SlidingWindow { samples : VecDeque::with_capacity(WINDOW_SIZE) }
    }

    /// Adds a new sample, evicting the oldest one once the window holds `WINDOW_SIZE` samples.
    pub fn push(&mut self, sample : Sample) {
        if self.samples.len() == WINDOW_SIZE {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Number of samples currently held (at most `WINDOW_SIZE`).
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// The sample with the lowest round-trip delay currently in the window — the single most
    /// trustworthy offset estimate available. A low delay implies a more symmetric, less
    /// distorted network path, the same intuition behind NTP's clock-filter algorithm.
    pub fn best_offset(&self) -> Option<&Sample> {
        self.samples.iter().min_by_key(|sample| sample.delay_nanos)
    }

    /// Estimates the clock drift rate via ordinary least-squares linear regression of offset
    /// against local time, across all samples currently in the window. Returns a fraction
    /// (seconds of offset drift per second of elapsed local time).
    ///
    /// Returns `None` if fewer than two samples are available, or if all samples share the
    /// same local time.
    pub fn estimate_drift(&self) -> Option<f64> {
        let n : usize = self.samples.len();
        if n < 2 {
            return None;
        }

        // Normalize local times relative to the oldest sample so the regression works with
        // small numbers regardless of how large the underlying Unix-epoch nanosecond
        // timestamps are.
        let reference_time : i128 = self.samples.front()?.local_time_nanos;

        let mut sum_x : f64 = 0.0;
        let mut sum_y : f64 = 0.0;
        let mut sum_xy : f64 = 0.0;
        let mut sum_xx : f64 = 0.0;

        for sample in &self.samples {
            let x : f64 = (sample.local_time_nanos - reference_time) as f64;
            let y : f64 = sample.offset_nanos as f64;
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
        }

        let n : f64 = n as f64;
        let denominator : f64 = n * sum_xx - sum_x * sum_x;
        if denominator == 0.0 {
            return None;
        }

        Some((n * sum_xy - sum_x * sum_y) / denominator)
    }
}

/// Extrapolates the clock offset at `now_nanos`, given a reference sample (typically
/// `SlidingWindow::best_offset`) and an estimated drift rate (as returned by
/// `SlidingWindow::estimate_drift`).
///
/// Not yet called from the `--sync` loop, which currently only reports the drift rate
/// itself rather than an extrapolated "now" between polls — kept as a tested building
/// block for that.
#[allow(dead_code)]
pub fn extrapolate_offset(reference : &Sample, drift_rate : f64, now_nanos : i128) -> i128 {
    let elapsed : i128 = now_nanos - reference.local_time_nanos;
    (reference.offset_nanos as f64 + drift_rate * elapsed as f64).round() as i128
}

#[cfg(test)]
mod test {
    use super::*;

    fn sample(local_time_nanos : i128, offset_nanos : i128, delay_nanos : i128) -> Sample {
        Sample { local_time_nanos, offset_nanos, delay_nanos }
    }

    #[test]
    fn test_empty_window_has_no_drift() {
        let window = SlidingWindow::new();

        assert_eq!(window.estimate_drift(), None);
        assert_eq!(window.best_offset(), None);
        assert_eq!(window.len(), 0);
    }

    #[test]
    fn test_single_sample_has_no_drift() {
        let mut window = SlidingWindow::new();
        window.push(sample(0, 0, 10_000_000));

        assert_eq!(window.estimate_drift(), None);
    }

    #[test]
    fn test_two_samples_matches_simple_two_point_slope() {
        let mut window = SlidingWindow::new();
        window.push(sample(0, 0, 10_000_000));
        window.push(sample(1_000_000_000, 10_000_000, 10_000_000));

        let drift = window.estimate_drift().unwrap();

        assert!((drift - 0.01).abs() < 1e-9);
    }

    #[test]
    fn test_perfect_linear_drift_recovered_exactly() {
        let mut window = SlidingWindow::new();
        // offset = 0.005 * local_time, no noise: 8 points, 1s apart.
        for i in 0..8_i128 {
            let t = i * 1_000_000_000;
            let offset = (t as f64 * 0.005).round() as i128;
            window.push(sample(t, offset, 20_000_000));
        }

        let drift = window.estimate_drift().unwrap();

        assert!((drift - 0.005).abs() < 1e-9);
    }

    #[test]
    fn test_window_evicts_oldest_beyond_capacity() {
        let mut window = SlidingWindow::new();
        for i in 0..(WINDOW_SIZE as i128 + 3) {
            window.push(sample(i * 1_000_000_000, i * 1_000_000, 10_000_000));
        }

        assert_eq!(window.len(), WINDOW_SIZE);
        // The oldest surviving sample should be the 4th pushed (index 3), since indices 0-2
        // were evicted.
        let oldest_surviving_time = 3 * 1_000_000_000;
        assert!(window.estimate_drift().is_some());
        assert_eq!(window.best_offset().unwrap().local_time_nanos >= oldest_surviving_time, true);
    }

    #[test]
    fn test_best_offset_picks_minimum_delay_not_most_recent() {
        let mut window = SlidingWindow::new();
        window.push(sample(0, 5_000_000, 80_000_000));
        window.push(sample(1_000_000_000, 6_000_000, 15_000_000));
        window.push(sample(2_000_000_000, 7_000_000, 90_000_000));

        let best = window.best_offset().unwrap();

        assert_eq!(best.local_time_nanos, 1_000_000_000);
        assert_eq!(best.delay_nanos, 15_000_000);
    }

    #[test]
    fn test_regression_resistanttest() {
        // True line: offset = 0.005 * local_time ("5000 ppm" drift), with small, alternating
        // jitter added to every sample, roughly matching the ms-level network noise observed
        // in practice.
        let true_slope = 0.005_f64;
        let jitter_ns = [3_000_000, -4_000_000, 6_000_000, -2_000_000,
                          5_000_000, -6_000_000, 4_000_000, -3_000_000];

        let mut window = SlidingWindow::new();
        let mut samples = Vec::new();
        for (i, jitter) in jitter_ns.iter().enumerate() {
            let t = (i as i128) * 1_000_000_000;
            let true_offset = (t as f64 * true_slope).round() as i128;
            let observed_offset = true_offset + jitter;
            let s = sample(t, observed_offset, 20_000_000);
            samples.push(s);
            window.push(s);
        }

        let windowed_drift = window.estimate_drift().unwrap();

        // Naive two-point estimate using only the last two (noisy) samples.
        let last = samples[samples.len() - 1];
        let second_last = samples[samples.len() - 2];
        let naive_drift = (last.offset_nanos - second_last.offset_nanos) as f64
            / (last.local_time_nanos - second_last.local_time_nanos) as f64;

        // The naive two-point estimate is thrown off badly enough by jitter to even flip sign.
        assert!(naive_drift < 0.0);
        // The windowed regression stays close to the true drift rate despite the same jitter.
        assert!((windowed_drift - true_slope).abs() < 0.001);
    }

    #[test]
    fn test_extrapolate_offset_with_drift() {
        let reference = sample(1_000_000_000, 10_000_000, 10_000_000);

        let extrapolated = extrapolate_offset(&reference, 0.01, 2_000_000_000);

        assert_eq!(extrapolated, 20_000_000);
    }

    #[test]
    fn test_extrapolate_offset_no_drift_stays_constant() {
        let reference = sample(1_000_000_000, 10_000_000, 10_000_000);

        let extrapolated = extrapolate_offset(&reference, 0.0, 5_000_000_000);

        assert_eq!(extrapolated, 10_000_000);
    }
}
