//! Growing delay after wrong PINs (SEC-19, RF-AGENT-12-6).
//!
//! The `n`-th consecutive failure imposes `2^(n-1)` seconds from the second failure on, capped at
//! 60 s, and resets on the first correct PIN. There is never a permanent lock (ATT-02): after the
//! delay the worker can always try again, and `Olvidé mi PIN` resets it by email.

use chrono::{DateTime, Duration, Utc};

/// Largest delay between attempts (SEC-19).
pub const MAX_PIN_DELAY_SECS: u64 = 60;

/// Delay imposed after `failures` consecutive wrong PINs.
pub fn delay_after_failures(failures: u32) -> u64 {
    if failures < 2 {
        return 0;
    }
    let exp = failures.saturating_sub(1).min(16);
    (1u64 << exp).min(MAX_PIN_DELAY_SECS)
}

/// Consecutive wrong-PIN counter of this service process.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PinAttempts {
    failures: u32,
    locked_until: Option<DateTime<Utc>>,
}

impl PinAttempts {
    /// `Err(seconds)` while the delay of the last failure has not elapsed.
    pub fn check(&self, now: DateTime<Utc>) -> Result<(), u64> {
        match self.locked_until {
            Some(until) if now < until => {
                let remaining = (until - now).num_milliseconds().max(0) as u64;
                Err(remaining.div_ceil(1000).max(1))
            }
            _ => Ok(()),
        }
    }

    /// Records a wrong PIN and returns the delay before the next attempt (0 = immediately).
    pub fn record_failure(&mut self, now: DateTime<Utc>) -> u64 {
        self.failures = self.failures.saturating_add(1);
        let delay = delay_after_failures(self.failures);
        self.locked_until = (delay > 0).then(|| now + Duration::seconds(delay as i64));
        delay
    }

    /// A correct PIN resets the counter.
    pub fn record_success(&mut self) {
        self.failures = 0;
        self.locked_until = None;
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_pin_guard_delay_grows_and_caps_at_60() {
        let delays: Vec<u64> = (1..=9).map(delay_after_failures).collect();
        assert_eq!(delays, vec![0, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(delay_after_failures(u32::MAX), 60);
    }

    #[test]
    fn test_pin_guard_never_locks_permanently_and_resets_on_success() {
        let t0 = Utc.with_ymd_and_hms(2026, 10, 11, 9, 0, 0).unwrap();
        let mut guard = PinAttempts::default();
        assert_eq!(guard.record_failure(t0), 0);
        assert!(guard.check(t0).is_ok());
        assert_eq!(guard.record_failure(t0), 2);
        assert_eq!(guard.check(t0), Err(2));
        assert_eq!(guard.check(t0 + Duration::seconds(1)), Err(1));
        assert!(guard.check(t0 + Duration::seconds(2)).is_ok());

        // Many failures: still only 60 s, then free again.
        let mut t = t0;
        for _ in 0..50 {
            t += Duration::seconds(61);
            guard.record_failure(t);
        }
        assert_eq!(guard.check(t), Err(60));
        assert!(guard.check(t + Duration::seconds(60)).is_ok());

        guard.record_success();
        assert_eq!(guard.failures(), 0);
        assert!(guard.check(t).is_ok());
    }
}
