use crate::core::metrics::ProductivityBasicItem;
use chrono::Utc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static KEYSTROKE_COUNT: AtomicU64 = AtomicU64::new(0);
static CLICK_COUNT: AtomicU64 = AtomicU64::new(0);
static HOOK_ENABLED: AtomicBool = AtomicBool::new(false);
static HOOK_STARTED: AtomicBool = AtomicBool::new(false);

pub struct KeystrokeCollector;

impl KeystrokeCollector {
    /// O(1) block-free callback to increment keystroke counter.
    /// Never allocates memory on heap, never acquires heavy locks, zero keylogging (DP-01).
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

    /// Enables or disables hook processing dynamically based on consent and run mode (DP-06, DP-07).
    pub fn set_hook_enabled(enabled: bool) {
        HOOK_ENABLED.store(enabled, Ordering::Relaxed);
    }

    /// Returns whether the hook is currently enabled.
    pub fn is_hook_enabled() -> bool {
        HOOK_ENABLED.load(Ordering::Relaxed)
    }

    /// Starts the low-priority OS input listener thread via `rdev` (RF-AGENT-02-1).
    /// Guarded by an atomic flag to ensure it is started at most once.
    pub fn start_hook() {
        if HOOK_STARTED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            if let Err(err) = std::thread::Builder::new()
                .name("input-hook".into())
                .spawn(|| {
                    let res = rdev::listen(|event| {
                        if !HOOK_ENABLED.load(Ordering::Relaxed) {
                            return;
                        }
                        match event.event_type {
                            rdev::EventType::KeyPress(_) => {
                                KEYSTROKE_COUNT.fetch_add(1, Ordering::Relaxed);
                            }
                            rdev::EventType::ButtonPress(_) => {
                                CLICK_COUNT.fetch_add(1, Ordering::Relaxed);
                            }
                            _ => {}
                        }
                    });

                    if let Err(err) = res {
                        tracing::warn!(
                            "Input hook listener terminated or lacks permissions: {:?}",
                            err
                        );
                    }
                })
            {
                tracing::error!("Failed to spawn input hook thread: {:?}", err);
                HOOK_STARTED.store(false, Ordering::SeqCst);
            }
        }
    }

    /// Takes a snapshot of current frequency counters and resets them to zero atomically (DoD 2).
    pub fn snapshot_and_reset() -> ProductivityBasicItem {
        let kpm = KEYSTROKE_COUNT.swap(0, Ordering::Relaxed);
        let cpm = CLICK_COUNT.swap(0, Ordering::Relaxed);

        tracing::debug!("Productivity metrics collected: KPM={}, CPM={}", kpm, cpm);

        ProductivityBasicItem {
            minute: Utc::now(),
            keystrokes: kpm,
            clicks: cpm,
        }
    }
}

/// Backwards compatibility alias matching TASK-AGENT-02 naming
pub type ProductivityCollector = KeystrokeCollector;

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

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

    #[test]
    fn test_snapshot_resets_counter() {
        KeystrokeCollector::snapshot_and_reset(); // Clean slate

        for _ in 0..50 {
            KeystrokeCollector::record_keystroke();
        }

        let metric = KeystrokeCollector::snapshot_and_reset();
        assert_eq!(metric.keystrokes, 50);

        let next = KeystrokeCollector::snapshot_and_reset();
        assert_eq!(next.keystrokes, 0);
    }

    #[test]
    fn test_atomic_counters_concurrency() {
        KeystrokeCollector::snapshot_and_reset(); // Clean slate

        let mut handles = Vec::new();
        for _ in 0..10 {
            let handle = thread::spawn(|| {
                for _ in 0..1_000 {
                    KeystrokeCollector::record_keystroke();
                    KeystrokeCollector::record_click();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let metric = KeystrokeCollector::snapshot_and_reset();
        assert_eq!(metric.keystrokes, 10_000);
        assert_eq!(metric.clicks, 10_000);
    }

    #[test]
    fn test_hook_enabled_toggle() {
        KeystrokeCollector::set_hook_enabled(false);
        assert!(!KeystrokeCollector::is_hook_enabled());

        KeystrokeCollector::set_hook_enabled(true);
        assert!(KeystrokeCollector::is_hook_enabled());
    }
}
