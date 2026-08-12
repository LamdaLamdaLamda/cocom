//! Persists a `SlidingWindow`'s samples to disk so drift estimation can start "warm"
//! instead of from scratch after a restart. Opt-in only, via the CLI's `--state-file` —
//! Cocom never writes a file unless explicitly told to.
//!
//! The format is a deliberately simple, human-inspectable text file: one sample per line,
//! `local_time_nanos offset_nanos delay_nanos`, oldest first. No serialization library is
//! needed for three plain integers, and `i128` doesn't round-trip cleanly through most JSON
//! parsers (JSON numbers are commonly backed by `f64`) — plain text sidesteps that entirely.

use crate::drift::{Sample, SlidingWindow};
use crate::ntp::Timestamp;
use std::fs;
use std::io;
use std::path::Path;

/// If the most recently persisted sample is older than this, the whole window is discarded
/// on load rather than treated as a warm start — conditions (network path, actual clock
/// drift) may have changed too much for stale samples to still be meaningful.
pub const MAX_SAMPLE_AGE_NANOS : i128 = 3_600_000_000_000; // 1 hour

/// Loads a previously saved `SlidingWindow` from `path`.
///
/// Returns an empty window — not an error — if the file doesn't exist, is empty, fails to
/// parse, or its newest sample is older than `MAX_SAMPLE_AGE_NANOS`. A missing, corrupt, or
/// stale state file is a cold start, not a fatal error.
pub fn load(path : &Path) -> SlidingWindow {
    let contents : String = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(_) => return SlidingWindow::new(),
    };

    let samples : Vec<Sample> = contents.lines().filter_map(parse_line).collect();
    let window : SlidingWindow = SlidingWindow::from_samples(samples);

    match window.latest() {
        Some(latest) => {
            let age_nanos : i128 = Timestamp::now().to_unix_nanos() - latest.local_time_nanos;
            if age_nanos > MAX_SAMPLE_AGE_NANOS {
                SlidingWindow::new()
            } else {
                window
            }
        }
        None => window,
    }
}

/// Parses a single `local_time_nanos offset_nanos delay_nanos` line. Returns `None` for a
/// malformed line, which callers skip rather than treat as a hard parse failure.
fn parse_line(line : &str) -> Option<Sample> {
    let mut parts = line.split_whitespace();
    let local_time_nanos : i128 = parts.next()?.parse().ok()?;
    let offset_nanos : i128 = parts.next()?.parse().ok()?;
    let delay_nanos : i128 = parts.next()?.parse().ok()?;
    Some(Sample { local_time_nanos, offset_nanos, delay_nanos })
}

/// Saves `window`'s samples to `path`, one per line, oldest first, overwriting any existing
/// content. Creates the parent directory if it doesn't already exist.
pub fn save(path : &Path, window : &SlidingWindow) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let mut contents : String = String::new();
    for sample in window.samples() {
        contents.push_str(&format!(
            "{} {} {}\n",
            sample.local_time_nanos, sample.offset_nanos, sample.delay_nanos
        ));
    }

    fs::write(path, contents)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A unique path under the OS temp dir, so parallel test runs don't collide.
    fn temp_path(name : &str) -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("cocom-state-test-{}-{}", name, nanos))
    }

    fn sample(local_time_nanos : i128, offset_nanos : i128, delay_nanos : i128) -> Sample {
        Sample { local_time_nanos, offset_nanos, delay_nanos }
    }

    #[test]
    fn test_load_missing_file_returns_empty_window() {
        let path = temp_path("missing");

        let window = load(&path);

        assert_eq!(window.len(), 0);
    }

    #[test]
    fn test_save_then_load_roundtrips_samples() {
        let path = temp_path("roundtrip");
        let now = Timestamp::now().to_unix_nanos();
        let original = SlidingWindow::from_samples(vec![
            sample(now - 2_000_000_000, 5_000_000, 10_000_000),
            sample(now - 1_000_000_000, 6_000_000, 12_000_000),
            sample(now, 7_000_000, 9_000_000),
        ]);

        save(&path, &original).unwrap();
        let loaded = load(&path);

        let original_samples : Vec<Sample> = original.samples().copied().collect();
        let loaded_samples : Vec<Sample> = loaded.samples().copied().collect();
        assert_eq!(loaded_samples, original_samples);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_malformed_file_returns_empty_window() {
        let path = temp_path("malformed");
        fs::write(&path, "not a valid sample line\n").unwrap();

        let window = load(&path);

        assert_eq!(window.len(), 0);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_discards_stale_samples() {
        let path = temp_path("stale");
        let ancient = sample(0, 5_000_000, 10_000_000); // local time 0 == the Unix epoch
        fs::write(&path, format!("{} {} {}\n", ancient.local_time_nanos, ancient.offset_nanos, ancient.delay_nanos)).unwrap();

        let window = load(&path);

        assert_eq!(window.len(), 0);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_save_creates_parent_directory() {
        let dir = temp_path("parent-dir");
        let path = dir.join("state");
        let window = SlidingWindow::from_samples(vec![sample(1, 2, 3)]);

        save(&path, &window).unwrap();

        assert!(path.exists());
        fs::remove_dir_all(&dir).ok();
    }
}
