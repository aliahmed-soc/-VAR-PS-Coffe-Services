//! Authoritative pricing. UI must not reimplement these formulas.

use serde::{Deserialize, Serialize};

use super::money::Minor;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    Linear,
    Stepped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PricingSnapshot {
    pub rule_type: RuleType,
    pub rate_minor_per_hour: Option<Minor>,
    pub billing_increment_seconds: Option<i64>,
    pub base_duration_seconds: Option<i64>,
    pub base_charge_minor: Option<Minor>,
    pub step_duration_seconds: Option<i64>,
    pub step_charge_minor: Option<Minor>,
    pub round_partial_step_up: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricingResult {
    pub duration_seconds: i64,
    pub charge_minor: Minor,
}

pub fn duration_seconds(started_at_unix: i64, ended_at_unix: i64) -> i64 {
    (ended_at_unix - started_at_unix).max(0)
}

/// Linear exact: floor((rate_minor_per_hour * duration_seconds) / 3600)
pub fn linear_charge_minor(rate_minor_per_hour: Minor, duration_seconds: i64) -> Minor {
    let duration_seconds = duration_seconds.max(0);
    rate_minor_per_hour.saturating_mul(duration_seconds) / 3600
}

pub fn calculate(
    snapshot: &PricingSnapshot,
    started_at_unix: i64,
    ended_at_unix: i64,
) -> PricingResult {
    let mut seconds = duration_seconds(started_at_unix, ended_at_unix);
    if let Some(inc) = snapshot.billing_increment_seconds {
        if inc > 0 && seconds > 0 {
            let rem = seconds % inc;
            if rem != 0 {
                seconds += inc - rem;
            }
        }
    }
    let charge = match snapshot.rule_type {
        RuleType::Linear => linear_charge_minor(snapshot.rate_minor_per_hour.unwrap_or(0), seconds),
        RuleType::Stepped => stepped_charge_minor(snapshot, seconds),
    };
    PricingResult {
        duration_seconds: seconds,
        charge_minor: charge,
    }
}

fn stepped_charge_minor(snapshot: &PricingSnapshot, duration_seconds: i64) -> Minor {
    let base_dur = snapshot.base_duration_seconds.unwrap_or(0).max(0);
    let base_charge = snapshot.base_charge_minor.unwrap_or(0);
    if duration_seconds <= 0 {
        return 0;
    }
    if duration_seconds <= base_dur {
        return base_charge;
    }
    let remaining = duration_seconds - base_dur;
    let step_dur = snapshot.step_duration_seconds.unwrap_or(1).max(1);
    let step_charge = snapshot.step_charge_minor.unwrap_or(0);
    let steps = if snapshot.round_partial_step_up {
        (remaining + step_dur - 1) / step_dur
    } else {
        remaining / step_dur
    };
    base_charge.saturating_add(step_charge.saturating_mul(steps))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(rate: Minor) -> PricingSnapshot {
        PricingSnapshot {
            rule_type: RuleType::Linear,
            rate_minor_per_hour: Some(rate),
            billing_increment_seconds: None,
            base_duration_seconds: None,
            base_charge_minor: None,
            step_duration_seconds: None,
            step_charge_minor: None,
            round_partial_step_up: true,
        }
    }

    #[test]
    fn exact_hour() {
        assert_eq!(linear_charge_minor(3000, 3600), 3000);
    }

    #[test]
    fn thirty_minutes() {
        assert_eq!(linear_charge_minor(3000, 1800), 1500);
    }

    #[test]
    fn one_hour_45m_30s() {
        // 6330 seconds * 3000 / 3600 = 5275
        assert_eq!(linear_charge_minor(3000, 6330), 5275);
        let r = calculate(&linear(3000), 0, 6330);
        assert_eq!(r.duration_seconds, 6330);
        assert_eq!(r.charge_minor, 5275);
    }

    #[test]
    fn floor_remainder() {
        // 1 second at 3000/hour = 3000/3600 = 0
        assert_eq!(linear_charge_minor(3000, 1), 0);
        // 2 seconds still 0; 2 * 3000 = 6000 / 3600 = 1
        assert_eq!(linear_charge_minor(3000, 2), 1);
    }

    #[test]
    fn zero_duration() {
        assert_eq!(linear_charge_minor(3000, 0), 0);
        assert_eq!(duration_seconds(100, 50), 0);
    }

    #[test]
    fn long_session() {
        let day = 24 * 3600;
        assert_eq!(linear_charge_minor(3000, day), 72000);
    }

    #[test]
    fn per_minute_increment() {
        let mut snap = linear(3000);
        snap.billing_increment_seconds = Some(60);
        let r = calculate(&snap, 0, 61);
        assert_eq!(r.duration_seconds, 120);
        assert_eq!(r.charge_minor, 100);
    }

    #[test]
    fn stepped_first_hour_then_30_min() {
        let snap = PricingSnapshot {
            rule_type: RuleType::Stepped,
            rate_minor_per_hour: None,
            billing_increment_seconds: None,
            base_duration_seconds: Some(3600),
            base_charge_minor: Some(3000),
            step_duration_seconds: Some(1800),
            step_charge_minor: Some(1500),
            round_partial_step_up: true,
        };
        assert_eq!(calculate(&snap, 0, 3600).charge_minor, 3000);
        assert_eq!(calculate(&snap, 0, 3601).charge_minor, 4500);
        assert_eq!(calculate(&snap, 0, 5400).charge_minor, 4500);
        assert_eq!(calculate(&snap, 0, 5401).charge_minor, 6000);
    }

    #[test]
    fn snapshot_not_recalculated_from_new_rate() {
        let monday = linear(3000);
        let r = calculate(&monday, 0, 3600);
        assert_eq!(r.charge_minor, 3000);
        let tuesday = linear(3500);
        assert_eq!(calculate(&tuesday, 0, 3600).charge_minor, 3500);
        assert_eq!(calculate(&monday, 0, 3600).charge_minor, 3000);
    }
}
