use std::sync::atomic::{AtomicU64, Ordering};
use chrono::Utc;
use crate::core::metrics::ProductivityMetric;

static KEYSTROKE_COUNT: AtomicU64 = AtomicU64::new(0);
static CLICK_COUNT: AtomicU64 = AtomicU64::new(0);

pub struct KeystrokeCollector;

impl KeystrokeCollector {
    /// O(1) block-free callback to increment keystroke counter.
    /// Never allocates memory on heap, never acquires heavy locks.
    #[inline(always)]
    pub fn record_keystroke() {
        KEYSTROKE_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    /// O(1) block-free callback to increment click counter.
    #[inline(always)]
    pub fn record_click() {
        CLICK_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    /// Takes a snapshot of current frequency counters and resets them to zero atomically.
    pub fn snapshot_and_reset() -> ProductivityMetric {
        let kpm = KEYSTROKE_COUNT.swap(0, Ordering::Relaxed);
        let cpm = CLICK_COUNT.swap(0, Ordering::Relaxed);

        ProductivityMetric {
            client_timestamp: Utc::now(),
            keystrokes_per_minute: kpm,
            clicks_per_minute: cpm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_counters_record_and_reset() {
        KeystrokeCollector::snapshot_and_reset(); // Clean slate

        KeystrokeCollector::record_keystroke();
        KeystrokeCollector::record_keystroke();
        KeystrokeCollector::record_click();

        let metric = KeystrokeCollector::snapshot_and_reset();
        assert_eq!(metric.keystrokes_per_minute, 2);
        assert_eq!(metric.clicks_per_minute, 1);

        // Next snapshot should be zero
        let next = KeystrokeCollector::snapshot_and_reset();
        assert_eq!(next.keystrokes_per_minute, 0);
        assert_eq!(next.clicks_per_minute, 0);
    }
}
