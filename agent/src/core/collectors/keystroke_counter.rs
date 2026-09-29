use crate::core::metrics::ProductivityBasicItem;
use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};

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

    /// Reads the current instantaneous counters without modifying them.
    #[inline(always)]
    pub fn get_current_counts() -> (u64, u64) {
        (
            KEYSTROKE_COUNT.load(Ordering::Relaxed),
            CLICK_COUNT.load(Ordering::Relaxed),
        )
    }

    /// Takes a snapshot of current frequency counters and resets them to zero atomically.
    pub fn snapshot_and_reset() -> ProductivityBasicItem {
        let kpm = KEYSTROKE_COUNT.swap(0, Ordering::Relaxed);
        let cpm = CLICK_COUNT.swap(0, Ordering::Relaxed);

        ProductivityBasicItem {
            minute: Utc::now(),
            keystrokes: kpm,
            clicks: cpm,
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
        assert_eq!(metric.keystrokes, 2);
        assert_eq!(metric.clicks, 1);

        // Next snapshot should be zero
        let next = KeystrokeCollector::snapshot_and_reset();
        assert_eq!(next.keystrokes, 0);
        assert_eq!(next.clicks, 0);
    }
}
