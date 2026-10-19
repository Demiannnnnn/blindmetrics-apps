//! Pure rules of the local attendance notices (RF-AGENT-12-13, spec §8). The clock is always an
//! argument: nothing here reads the system time.
//!
//! - Missing mark: 30 min after an agreed start/end/break time without that mark, once per mark
//!   and day, only while the service runs. The notice is due during the hour that follows (a
//!   service started much later does not report stale reminders). End and break reminders need a
//!   start mark that day (an absent worker is not nagged; the server alerts the employer).
//! - Disconnection: during the 30 minutes before a remote/hybrid worker's disconnection window.

use agent_ipc::MarkType;
use chrono::{Duration, NaiveTime};

/// Minutes after the agreed time before reminding.
pub const MISSING_MARK_GRACE_MIN: i64 = 30;
/// How long the reminder stays due after the grace period.
pub const MISSING_MARK_WINDOW_MIN: i64 = 60;
/// Minutes before the disconnection window.
pub const DISCONNECTION_NOTICE_MIN: i64 = 30;

/// Parses `HH:MM` or `HH:MM:SS`.
pub fn parse_time(raw: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(raw, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(raw, "%H:%M"))
        .ok()
}

/// Agreed times of one day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AgreedTimes {
    pub start: Option<NaiveTime>,
    pub end: Option<NaiveTime>,
    pub break_start: Option<NaiveTime>,
    pub break_end: Option<NaiveTime>,
}

impl AgreedTimes {
    pub fn from_strings(
        start: Option<&str>,
        end: Option<&str>,
        break_start: Option<&str>,
        break_end: Option<&str>,
    ) -> Self {
        Self {
            start: start.and_then(parse_time),
            end: end.and_then(parse_time),
            break_start: break_start.and_then(parse_time),
            break_end: break_end.and_then(parse_time),
        }
    }

    fn entries(&self) -> [(MarkType, Option<NaiveTime>); 4] {
        [
            (MarkType::WorkdayStart, self.start),
            (MarkType::BreakStart, self.break_start),
            (MarkType::BreakEnd, self.break_end),
            (MarkType::WorkdayEnd, self.end),
        ]
    }

    /// True when the shift crosses midnight (its end belongs to the next day).
    fn overnight(&self) -> bool {
        matches!((self.start, self.end), (Some(s), Some(e)) if e <= s)
    }
}

/// Minutes from `from` to `to` on a 24 h clock, in `0..1440`.
fn minutes_until(from: NaiveTime, to: NaiveTime) -> i64 {
    let diff = (to - from).num_minutes();
    diff.rem_euclid(24 * 60)
}

/// Agreed marks whose reminder is due now: `(type, agreed time)`.
pub fn due_missing_marks(
    agreed: &AgreedTimes,
    marks_today: &[MarkType],
    now: NaiveTime,
) -> Vec<(MarkType, NaiveTime)> {
    let started = marks_today.contains(&MarkType::WorkdayStart);
    let grace = Duration::minutes(MISSING_MARK_GRACE_MIN);
    let mut due = Vec::new();
    for (mark_type, time) in agreed.entries() {
        let Some(time) = time else { continue };
        if marks_today.contains(&mark_type) {
            continue;
        }
        if mark_type != MarkType::WorkdayStart && !started {
            continue;
        }
        // Only same-day reminders: the end of an overnight shift belongs to the next day.
        if mark_type == MarkType::WorkdayEnd && agreed.overnight() {
            continue;
        }
        let (remind_at, wrapped) = time.overflowing_add_signed(grace);
        if wrapped != 0 {
            continue;
        }
        if now < remind_at {
            continue;
        }
        if (now - remind_at).num_minutes() >= MISSING_MARK_WINDOW_MIN {
            continue;
        }
        due.push((mark_type, time));
    }
    due
}

/// True during the 30 minutes before the disconnection window starts.
pub fn disconnection_notice_due(start: NaiveTime, now: NaiveTime) -> bool {
    let until = minutes_until(now, start);
    until > 0 && until <= DISCONNECTION_NOTICE_MIN
}

/// True while `now` is inside the disconnection window (it may cross midnight). DP-12.
pub fn in_window(start: NaiveTime, end: NaiveTime, now: NaiveTime) -> bool {
    if start <= end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

/// `HH:MM` for notices.
pub fn hhmm(t: NaiveTime) -> String {
    t.format("%H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn office() -> AgreedTimes {
        AgreedTimes::from_strings(
            Some("09:00"),
            Some("18:00:00"),
            Some("13:00"),
            Some("14:00"),
        )
    }

    #[test]
    fn test_reminders_missing_start_after_30_minutes_only() {
        let a = office();
        assert!(due_missing_marks(&a, &[], at(9, 29)).is_empty());
        assert_eq!(
            due_missing_marks(&a, &[], at(9, 30)),
            vec![(MarkType::WorkdayStart, at(9, 0))]
        );
        assert_eq!(due_missing_marks(&a, &[], at(10, 29)).len(), 1);
        // An hour after the reminder time it is stale.
        assert!(due_missing_marks(&a, &[], at(10, 30)).is_empty());
        // Marked: nothing to remind.
        assert!(due_missing_marks(&a, &[MarkType::WorkdayStart], at(9, 45)).is_empty());
    }

    #[test]
    fn test_reminders_end_and_break_need_a_start() {
        let a = office();
        let started = [MarkType::WorkdayStart];
        assert_eq!(
            due_missing_marks(&a, &started, at(13, 30)),
            vec![(MarkType::BreakStart, at(13, 0))]
        );
        assert_eq!(
            due_missing_marks(&a, &started, at(18, 30)),
            vec![(MarkType::WorkdayEnd, at(18, 0))]
        );
        assert!(due_missing_marks(&a, &[], at(18, 30)).is_empty());
        assert!(due_missing_marks(
            &a,
            &[MarkType::WorkdayStart, MarkType::WorkdayEnd],
            at(18, 30)
        )
        .is_empty());
    }

    #[test]
    fn test_reminders_overnight_shift_skips_next_day_end() {
        let a = AgreedTimes::from_strings(Some("22:00"), Some("06:00"), None, None);
        assert_eq!(
            due_missing_marks(&a, &[], at(22, 30)),
            vec![(MarkType::WorkdayStart, at(22, 0))]
        );
        assert!(due_missing_marks(&a, &[MarkType::WorkdayStart], at(6, 30)).is_empty());
        // 23:45 + 30 min wraps past midnight: not reminded the same day.
        let late = AgreedTimes::from_strings(Some("23:45"), None, None, None);
        assert!(due_missing_marks(&late, &[], at(23, 59)).is_empty());
    }

    #[test]
    fn test_reminders_disconnection_window() {
        assert!(!disconnection_notice_due(at(20, 0), at(19, 29)));
        assert!(disconnection_notice_due(at(20, 0), at(19, 30)));
        assert!(disconnection_notice_due(at(20, 0), at(19, 59)));
        assert!(!disconnection_notice_due(at(20, 0), at(20, 0)));
        // Window starting just after midnight.
        assert!(disconnection_notice_due(at(0, 10), at(23, 50)));

        assert!(in_window(at(20, 0), at(8, 0), at(23, 0)));
        assert!(in_window(at(20, 0), at(8, 0), at(7, 59)));
        assert!(!in_window(at(20, 0), at(8, 0), at(12, 0)));
        assert!(in_window(at(13, 0), at(14, 0), at(13, 30)));
    }
}
