//! Authoritative MVP gaming pricing. UI must not reimplement this formula.
//!
//! `charge_minor = floor(rate_minor_per_hour * actual_duration_seconds / 3600)`
//!
//! Stepped rules and billing-increment rounding are not MVP behavior.

use serde::{Deserialize, Serialize};

use super::money::Minor;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    Linear,
    /// Reserved for a future Phase-2 product change. Never charged in MVP.
    Stepped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PricingError {
    UnsupportedRule,
    MissingRate,
    NegativeRate,
}

impl std::fmt::Display for PricingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedRule => write!(f, "stepped pricing is not supported in MVP"),
            Self::MissingRate => write!(f, "linear pricing requires rate_minor_per_hour"),
            Self::NegativeRate => write!(f, "rate_minor_per_hour must be >= 0"),
        }
    }
}

/// Historical session snapshot. Reserved increment/step fields may appear as
/// null (or leftover JSON) but must never change the MVP charge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PricingSnapshot {
    pub rule_type: RuleType,
    pub rate_minor_per_hour: Option<Minor>,
    #[serde(default)]
    pub billing_increment_seconds: Option<i64>,
    #[serde(default)]
    pub base_duration_seconds: Option<i64>,
    #[serde(default)]
    pub base_charge_minor: Option<Minor>,
    #[serde(default)]
    pub step_duration_seconds: Option<i64>,
    #[serde(default)]
    pub step_charge_minor: Option<Minor>,
    #[serde(default)]
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

/// Active-rule factory. Rejects stepped activation and negative rates.
pub fn mvp_snapshot(
    rule_type: &str,
    rate_minor_per_hour: Minor,
) -> Result<PricingSnapshot, PricingError> {
    if rule_type != "linear" {
        return Err(PricingError::UnsupportedRule);
    }
    if rate_minor_per_hour < 0 {
        return Err(PricingError::NegativeRate);
    }
    Ok(PricingSnapshot {
        rule_type: RuleType::Linear,
        rate_minor_per_hour: Some(rate_minor_per_hour),
        billing_increment_seconds: None,
        base_duration_seconds: None,
        base_charge_minor: None,
        step_duration_seconds: None,
        step_charge_minor: None,
        round_partial_step_up: false,
    })
}

pub fn require_mvp_linear(snapshot: &PricingSnapshot) -> Result<Minor, PricingError> {
    match snapshot.rule_type {
        RuleType::Linear => {}
        RuleType::Stepped => return Err(PricingError::UnsupportedRule),
    }
    let rate = snapshot
        .rate_minor_per_hour
        .ok_or(PricingError::MissingRate)?;
    if rate < 0 {
        return Err(PricingError::NegativeRate);
    }
    Ok(rate)
}

/// Charge from the frozen session snapshot using actual elapsed seconds only.
pub fn calculate(
    snapshot: &PricingSnapshot,
    started_at_unix: i64,
    ended_at_unix: i64,
) -> Result<PricingResult, PricingError> {
    let rate = require_mvp_linear(snapshot)?;
    let seconds = duration_seconds(started_at_unix, ended_at_unix);
    Ok(PricingResult {
        duration_seconds: seconds,
        charge_minor: linear_charge_minor(rate, seconds),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(rate: Minor) -> PricingSnapshot {
        mvp_snapshot("linear", rate).unwrap()
    }

    #[test]
    fn exact_hour() {
        assert_eq!(linear_charge_minor(3000, 3600), 3000);
        assert_eq!(
            calculate(&linear(3000), 0, 3600).unwrap().charge_minor,
            3000
        );
    }

    #[test]
    fn thirty_minutes() {
        assert_eq!(linear_charge_minor(3000, 1800), 1500);
        assert_eq!(
            calculate(&linear(3000), 0, 1800).unwrap().charge_minor,
            1500
        );
    }

    #[test]
    fn one_hour_45m_30s() {
        // 6330 seconds * 3000 / 3600 = 5275
        assert_eq!(linear_charge_minor(3000, 6330), 5275);
        let r = calculate(&linear(3000), 0, 6330).unwrap();
        assert_eq!(r.duration_seconds, 6330);
        assert_eq!(r.charge_minor, 5275);
    }

    #[test]
    fn one_second_floors_without_increment() {
        assert_eq!(linear_charge_minor(3000, 1), 0);
        let r = calculate(&linear(3000), 0, 1).unwrap();
        assert_eq!(r.duration_seconds, 1);
        assert_eq!(r.charge_minor, 0);
        assert_eq!(linear_charge_minor(3000, 2), 1);
    }

    #[test]
    fn sixty_one_seconds_uses_actual_elapsed() {
        let r = calculate(&linear(3000), 0, 61).unwrap();
        assert_eq!(r.duration_seconds, 61);
        assert_eq!(r.charge_minor, 61 * 3000 / 3600);
        assert_eq!(r.charge_minor, 50);
        assert_ne!(r.duration_seconds, 60);
        assert_ne!(r.duration_seconds, 120);
        assert_ne!(r.charge_minor, 100);
    }

    #[test]
    fn negative_duration_becomes_zero() {
        assert_eq!(duration_seconds(100, 50), 0);
        assert_eq!(linear_charge_minor(3000, -90), 0);
        let r = calculate(&linear(3000), 100, 50).unwrap();
        assert_eq!(r.duration_seconds, 0);
        assert_eq!(r.charge_minor, 0);
    }

    #[test]
    fn long_session_stays_integer_safe() {
        let day = 24 * 3600;
        assert_eq!(linear_charge_minor(3000, day), 72_000);
        let ten_years = 365 * day * 10;
        assert_eq!(linear_charge_minor(3000, ten_years), 262_800_000);
        let huge = linear_charge_minor(i64::MAX / 4000, 10_000);
        assert!(huge >= 0);
        let saturated = linear_charge_minor(i64::MAX, i64::MAX);
        assert_eq!(saturated, i64::MAX / 3600);
    }

    #[test]
    fn snapshot_not_recalculated_from_new_rate() {
        let monday = linear(3000);
        let r = calculate(&monday, 0, 3600).unwrap();
        assert_eq!(r.charge_minor, 3000);
        let tuesday = linear(3500);
        assert_eq!(calculate(&tuesday, 0, 3600).unwrap().charge_minor, 3500);
        assert_eq!(calculate(&monday, 0, 3600).unwrap().charge_minor, 3000);
    }

    #[test]
    fn stepped_activation_rejected() {
        assert_eq!(
            mvp_snapshot("stepped", 3000),
            Err(PricingError::UnsupportedRule)
        );
        let snap = PricingSnapshot {
            rule_type: RuleType::Stepped,
            rate_minor_per_hour: Some(3000),
            billing_increment_seconds: None,
            base_duration_seconds: Some(3600),
            base_charge_minor: Some(3000),
            step_duration_seconds: Some(1800),
            step_charge_minor: Some(1500),
            round_partial_step_up: true,
        };
        assert_eq!(
            calculate(&snap, 0, 3601),
            Err(PricingError::UnsupportedRule)
        );
        assert_eq!(
            require_mvp_linear(&snap),
            Err(PricingError::UnsupportedRule)
        );
    }

    #[test]
    fn billing_increment_cannot_alter_mvp_charge() {
        let mut snap = linear(3000);
        snap.billing_increment_seconds = Some(60);
        snap.round_partial_step_up = true;
        snap.base_charge_minor = Some(9999);
        snap.step_charge_minor = Some(1500);
        snap.step_duration_seconds = Some(60);
        let one = calculate(&snap, 0, 1).unwrap();
        assert_eq!(one.duration_seconds, 1);
        assert_eq!(one.charge_minor, 0);
        let sixty_one = calculate(&snap, 0, 61).unwrap();
        assert_eq!(sixty_one.duration_seconds, 61);
        assert_eq!(sixty_one.charge_minor, 50);
    }

    #[test]
    fn negative_rate_rejected() {
        assert_eq!(mvp_snapshot("linear", -1), Err(PricingError::NegativeRate));
        let snap = PricingSnapshot {
            rule_type: RuleType::Linear,
            rate_minor_per_hour: Some(-5),
            billing_increment_seconds: None,
            base_duration_seconds: None,
            base_charge_minor: None,
            step_duration_seconds: None,
            step_charge_minor: None,
            round_partial_step_up: false,
        };
        assert_eq!(calculate(&snap, 0, 3600), Err(PricingError::NegativeRate));
    }
}
