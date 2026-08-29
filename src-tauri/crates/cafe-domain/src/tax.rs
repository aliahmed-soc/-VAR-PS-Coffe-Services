//! Tax-ready snapshots. MVP stores zeros and never calculates VAT.

use crate::money::{self, Minor, MoneyError};

pub const MVP_TAX_MINOR: Minor = 0;
pub const MVP_TAX_RATE_BPS: Minor = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxSnapshot {
    pub tax_minor: Minor,
    pub tax_rate_bps: Minor,
}

impl TaxSnapshot {
    pub fn mvp() -> Self {
        Self {
            tax_minor: MVP_TAX_MINOR,
            tax_rate_bps: MVP_TAX_RATE_BPS,
        }
    }
}

pub fn reject_negative_tax(tax_minor: Minor, tax_rate_bps: Minor) -> Result<(), MoneyError> {
    if tax_minor < 0 || tax_rate_bps < 0 {
        return Err(MoneyError::Negative);
    }
    Ok(())
}

pub fn subtotal(
    product_subtotal_minor: Minor,
    gaming_subtotal_minor: Minor,
) -> Result<Minor, MoneyError> {
    money::add(product_subtotal_minor, gaming_subtotal_minor)
}

/// `subtotal_minor + tax_minor - discount_minor = total_minor`
pub fn total(
    subtotal_minor: Minor,
    tax_minor: Minor,
    discount_minor: Minor,
) -> Result<Minor, MoneyError> {
    reject_negative_tax(tax_minor, 0)?;
    if discount_minor < 0 {
        return Err(MoneyError::Negative);
    }
    money::sub(money::add(subtotal_minor, tax_minor)?, discount_minor)
}

pub fn canonical_total(
    product_subtotal_minor: Minor,
    gaming_subtotal_minor: Minor,
    tax_minor: Minor,
    discount_minor: Minor,
) -> Result<Minor, MoneyError> {
    total(
        subtotal(product_subtotal_minor, gaming_subtotal_minor)?,
        tax_minor,
        discount_minor,
    )
}

pub fn identity_holds(
    subtotal_minor: Minor,
    tax_minor: Minor,
    discount_minor: Minor,
    total_minor: Minor,
) -> bool {
    total_minor == subtotal_minor + tax_minor - discount_minor
}

/// Replay copies payment-time tax. It must not derive tax from `tax_rate_bps` or a later rate.
pub fn tax_from_receipt_snapshot(snapshot: &serde_json::Value) -> Result<TaxSnapshot, MoneyError> {
    let tax_minor = snapshot
        .get("tax_minor")
        .and_then(|v| v.as_i64())
        .ok_or(MoneyError::MissingSnapshot)?;
    let tax_rate_bps = snapshot
        .get("tax_rate_bps")
        .and_then(|v| v.as_i64())
        .ok_or(MoneyError::MissingSnapshot)?;
    reject_negative_tax(tax_minor, tax_rate_bps)?;
    Ok(TaxSnapshot {
        tax_minor,
        tax_rate_bps,
    })
}

pub fn replay_tax(
    snapshot: &serde_json::Value,
    _current_rate_bps: Minor,
) -> Result<TaxSnapshot, MoneyError> {
    tax_from_receipt_snapshot(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mvp_tax_defaults_to_zero() {
        let snap = TaxSnapshot::mvp();
        assert_eq!(snap.tax_minor, 0);
        assert_eq!(snap.tax_rate_bps, 0);
        assert_eq!(canonical_total(2500, 3000, 0, 0).unwrap(), 5500);
        assert!(identity_holds(5500, 0, 0, 5500));
    }

    #[test]
    fn negative_tax_is_rejected() {
        assert_eq!(reject_negative_tax(-1, 0), Err(MoneyError::Negative));
        assert_eq!(reject_negative_tax(0, -1), Err(MoneyError::Negative));
        assert_eq!(canonical_total(100, 0, -5, 0), Err(MoneyError::Negative));
        assert_eq!(
            tax_from_receipt_snapshot(&serde_json::json!({"tax_minor": -1, "tax_rate_bps": 0})),
            Err(MoneyError::Negative)
        );
    }

    #[test]
    fn identity_with_nonzero_historical_tax() {
        assert!(identity_holds(10_000, 1_400, 0, 11_400));
        assert_eq!(total(10_000, 1_400, 0).unwrap(), 11_400);
    }

    #[test]
    fn replay_copies_snapshot_and_does_not_recalculate() {
        let zero = serde_json::json!({
            "tax_minor": 0,
            "tax_rate_bps": 0,
            "subtotal_minor": 5275,
            "total_minor": 5275
        });
        assert_eq!(replay_tax(&zero, 14_00).unwrap(), TaxSnapshot::mvp());

        let historical = serde_json::json!({
            "tax_minor": 500,
            "tax_rate_bps": 1400,
            "subtotal_minor": 10_000,
            "total_minor": 10_500
        });
        let copied = replay_tax(&historical, 0).unwrap();
        assert_eq!(copied.tax_minor, 500);
        assert_eq!(copied.tax_rate_bps, 1400);
        assert_ne!(copied.tax_minor, 10_000 * 1400 / 10_000);
    }
}
