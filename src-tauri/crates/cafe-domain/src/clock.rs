use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct ClockObservation {
    pub now: DateTime<Utc>,
    pub jump_detected: bool,
    pub jump_seconds: i64,
}

pub fn elapsed_seconds(started_at: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
    now.signed_duration_since(started_at).num_seconds().max(0)
}

pub fn detect_jump(previous: DateTime<Utc>, now: DateTime<Utc>, threshold_seconds: i64) -> ClockObservation {
    let delta = now.signed_duration_since(previous).num_seconds();
    let jump = delta < 0 || delta > threshold_seconds;
    ClockObservation {
        now,
        jump_detected: jump,
        jump_seconds: delta,
    }
}

pub fn parse_utc(iso: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(iso)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn crash_recovery_elapsed() {
        let start = Utc.with_ymd_and_hms(2026, 8, 28, 18, 30, 0).unwrap();
        let restart = Utc.with_ymd_and_hms(2026, 8, 28, 19, 20, 0).unwrap();
        assert_eq!(elapsed_seconds(start, restart), 50 * 60);
    }

    #[test]
    fn never_negative() {
        let start = Utc.with_ymd_and_hms(2026, 8, 28, 18, 30, 0).unwrap();
        let back = Utc.with_ymd_and_hms(2026, 8, 28, 12, 0, 0).unwrap();
        assert_eq!(elapsed_seconds(start, back), 0);
    }

    #[test]
    fn midnight_crossing() {
        let start = Utc.with_ymd_and_hms(2026, 8, 28, 23, 50, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 8, 29, 0, 10, 0).unwrap();
        assert_eq!(elapsed_seconds(start, end), 20 * 60);
    }

    #[test]
    fn flags_large_jump() {
        let a = Utc.with_ymd_and_hms(2026, 8, 28, 18, 30, 0).unwrap();
        let b = Utc.with_ymd_and_hms(2026, 8, 28, 12, 0, 0).unwrap();
        let obs = detect_jump(a, b, 120);
        assert!(obs.jump_detected);
        assert!(obs.jump_seconds < 0);
    }
}
