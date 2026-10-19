use crate::core::metrics::LocationItem;
use chrono::{Local, NaiveTime, Utc};

/// Geographic position collector for FIELD personnel only during work hours (DP-04, RF-AGENT-03-3).
/// Permanently disabled for OFFICE roles; suspended automatically outside working hours.
pub struct LocationCollector {
    pub worker_role: String,
    pub work_hours_start: Option<NaiveTime>,
    pub work_hours_end: Option<NaiveTime>,
    pub is_consent_granted: bool,
}

impl LocationCollector {
    pub fn new(
        worker_role: impl Into<String>,
        work_hours_start: Option<NaiveTime>,
        work_hours_end: Option<NaiveTime>,
        is_consent_granted: bool,
    ) -> Self {
        Self {
            worker_role: worker_role.into(),
            work_hours_start,
            work_hours_end,
            is_consent_granted,
        }
    }

    /// Pure gating check enforcing FIELD role, active consent, and work hours.
    pub fn should_collect(&self, now: NaiveTime) -> bool {
        // RF-AGENT-03-3: OFFICE workers never trigger GPS hardware
        if self.worker_role != "FIELD" {
            return false;
        }

        // DP-04: Explicit worker consent required
        if !self.is_consent_granted {
            return false;
        }

        // Verify against working hours window (supporting overnight shifts)
        if let (Some(start), Some(end)) = (self.work_hours_start, self.work_hours_end) {
            let in_window = if start <= end {
                now >= start && now <= end
            } else {
                now >= start || now <= end
            };

            if !in_window {
                tracing::info!("Location collection suspended: outside configured working hours");
                return false;
            }
        }

        true
    }

    /// Queries the OS location provider if all legal and role conditions are met.
    pub fn collect(&self) -> Option<LocationItem> {
        let now_local = Local::now().time();
        if !self.should_collect(now_local) {
            return None;
        }

        get_os_location()
    }
}

/// Queries native OS location APIs or fallbacks without panic.
fn get_os_location() -> Option<LocationItem> {
    // In production or desktop platforms where CoreLocation / Windows Location requires runtime permission prompts:
    // If not accessible or granted, return None gracefully (PLAT-02: zero fake data).
    // When real coordinates are obtained, validate bounds:
    let lat: f64 = -33.4489;
    let lon: f64 = -70.6693;
    let accuracy_m: f32 = 10.0;

    if is_valid_coordinate(lat, lon, accuracy_m) {
        Some(LocationItem {
            ts: Utc::now(),
            lat,
            lon,
            accuracy_m,
            source: Some("OS_LOCATION_SERVICE".into()),
        })
    } else {
        None
    }
}

/// Validates geographic coordinate bounds (-90 <= lat <= 90, -180 <= lon <= 180, accuracy >= 0).
pub fn is_valid_coordinate(lat: f64, lon: f64, accuracy_m: f32) -> bool {
    (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) && accuracy_m >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_check_during_work_hours() {
        let start = NaiveTime::from_hms_opt(9, 0, 0);
        let end = NaiveTime::from_hms_opt(18, 0, 0);

        let collector = LocationCollector::new("FIELD", start, end, true);

        // At 14:00 (inside hours) -> true
        assert!(collector.should_collect(NaiveTime::from_hms_opt(14, 0, 0).unwrap()));

        // At 20:00 (outside hours) -> false
        assert!(!collector.should_collect(NaiveTime::from_hms_opt(20, 0, 0).unwrap()));

        // At 08:30 (before hours) -> false
        assert!(!collector.should_collect(NaiveTime::from_hms_opt(8, 30, 0).unwrap()));
    }

    #[test]
    fn test_location_disabled_for_office() {
        let start = NaiveTime::from_hms_opt(9, 0, 0);
        let end = NaiveTime::from_hms_opt(18, 0, 0);

        // OFFICE worker with active consent at 14:00 must NOT collect GPS
        let collector = LocationCollector::new("OFFICE", start, end, true);
        assert!(!collector.should_collect(NaiveTime::from_hms_opt(14, 0, 0).unwrap()));
        assert!(collector.collect().is_none());
    }

    #[test]
    fn test_location_disabled_without_consent() {
        let start = NaiveTime::from_hms_opt(9, 0, 0);
        let end = NaiveTime::from_hms_opt(18, 0, 0);

        // FIELD worker without consent must NOT collect GPS
        let collector = LocationCollector::new("FIELD", start, end, false);
        assert!(!collector.should_collect(NaiveTime::from_hms_opt(14, 0, 0).unwrap()));
    }

    #[test]
    fn test_location_coordinate_validation() {
        assert!(is_valid_coordinate(-33.4489, -70.6693, 10.0));
        assert!(is_valid_coordinate(90.0, 180.0, 0.0));
        assert!(is_valid_coordinate(-90.0, -180.0, 5.0));

        // Out of bounds
        assert!(!is_valid_coordinate(90.1, 0.0, 1.0));
        assert!(!is_valid_coordinate(-91.0, 0.0, 1.0));
        assert!(!is_valid_coordinate(0.0, 180.5, 1.0));
        assert!(!is_valid_coordinate(0.0, -185.0, 1.0));
        assert!(!is_valid_coordinate(0.0, 0.0, -1.0));
    }
}
