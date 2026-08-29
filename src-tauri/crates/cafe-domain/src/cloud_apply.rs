//! Cloud apply semantics for payment close. Mirrors `apply_domain_event`.

use std::collections::HashMap;

use serde_json::Value;

use crate::tax;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyStatus {
    Applied,
    AlreadyProcessed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    SequenceGap { expected: i64, got: i64 },
    OrderNotFound,
    WrongBranch,
    OrderNotPayable,
    NotCheckoutPending,
    AmountMismatch,
    TotalIdentity,
    AlreadyPaid,
    ReceiptAlreadyStored,
    DuplicateCapturedSale,
    OrderNotPaid,
}

#[derive(Debug, Clone)]
pub struct CloudOrder {
    pub id: String,
    pub branch_id: String,
    pub status: String,
    pub subtotal_minor: i64,
    pub discount_minor: i64,
    pub tax_minor: i64,
    pub tax_rate_bps: i64,
    pub total_minor: i64,
    pub amount_paid_minor: i64,
    pub change_minor: i64,
    pub receipt_number: Option<String>,
    pub receipt_snapshot: Option<Value>,
    pub closed_by: Option<String>,
    pub closed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CloudPayment {
    pub id: String,
    pub order_id: String,
    pub payment_type: String,
    pub status: String,
    pub amount_applied_minor: i64,
}

pub struct ApplyRequest<'a> {
    pub event_id: &'a str,
    pub branch_id: &'a str,
    pub device_id: &'a str,
    pub local_sequence: i64,
    pub event_type: &'a str,
    pub payload: &'a Value,
    pub payload_hash: &'a str,
}

#[derive(Debug, Default)]
pub struct CloudLedger {
    pub orders: HashMap<String, CloudOrder>,
    pub payments: Vec<CloudPayment>,
    processed: HashMap<String, String>,
    last_seq: HashMap<String, i64>,
}

impl CloudLedger {
    pub fn seed_checkout_order(&mut self, order: CloudOrder) {
        self.orders.insert(order.id.clone(), order);
    }

    pub fn apply(&mut self, req: ApplyRequest<'_>) -> Result<ApplyStatus, ApplyError> {
        if let Some(hash) = self.processed.get(req.event_id) {
            if hash != req.payload_hash {
                return Err(ApplyError::AmountMismatch);
            }
            return Ok(ApplyStatus::AlreadyProcessed);
        }

        let last = self.last_seq.get(req.device_id).copied().unwrap_or(0);
        if req.local_sequence != last + 1 {
            return Err(ApplyError::SequenceGap {
                expected: last + 1,
                got: req.local_sequence,
            });
        }

        match req.event_type {
            "payment.captured" => self.apply_payment_captured(req.branch_id, req.payload)?,
            "order.paid" => self.apply_order_paid(req.branch_id, req.payload)?,
            "payment.reversed" => self.apply_payment_reversed(req.branch_id, req.payload)?,
            _ => {}
        }

        self.processed
            .insert(req.event_id.to_string(), req.payload_hash.to_string());
        self.last_seq
            .insert(req.device_id.to_string(), req.local_sequence);
        Ok(ApplyStatus::Applied)
    }

    fn order_mut<'a>(
        orders: &'a mut HashMap<String, CloudOrder>,
        payload: &Value,
    ) -> Result<&'a mut CloudOrder, ApplyError> {
        let id = payload
            .get("order_id")
            .and_then(|v| v.as_str())
            .ok_or(ApplyError::OrderNotFound)?;
        orders.get_mut(id).ok_or(ApplyError::OrderNotFound)
    }

    fn apply_payment_captured(
        &mut self,
        branch_id: &str,
        payload: &Value,
    ) -> Result<(), ApplyError> {
        let order = Self::order_mut(&mut self.orders, payload)?;
        if order.branch_id != branch_id {
            return Err(ApplyError::WrongBranch);
        }
        if order.status != "open" && order.status != "checkout_pending" {
            return Err(ApplyError::OrderNotPayable);
        }
        let applied = payload
            .get("amount_applied_minor")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        let tendered = payload
            .get("amount_tendered_minor")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        if applied < 0 || tendered < applied {
            return Err(ApplyError::AmountMismatch);
        }
        if order.status == "open" {
            order.status = "checkout_pending".into();
        }
        Ok(())
    }

    fn apply_order_paid(&mut self, branch_id: &str, payload: &Value) -> Result<(), ApplyError> {
        let order_id = payload
            .get("order_id")
            .and_then(|v| v.as_str())
            .ok_or(ApplyError::OrderNotFound)?
            .to_string();
        let payment_id = payload
            .get("payment_id")
            .and_then(|v| v.as_str())
            .ok_or(ApplyError::AmountMismatch)?
            .to_string();

        {
            let order = self
                .orders
                .get_mut(&order_id)
                .ok_or(ApplyError::OrderNotFound)?;
            if order.branch_id != branch_id {
                return Err(ApplyError::WrongBranch);
            }
            if order.status == "paid" {
                return Err(ApplyError::AlreadyPaid);
            }
            if order.status != "checkout_pending" {
                return Err(ApplyError::NotCheckoutPending);
            }
            if !tax::identity_holds(
                order.subtotal_minor,
                order.tax_minor,
                order.discount_minor,
                order.total_minor,
            ) {
                return Err(ApplyError::TotalIdentity);
            }
            if order.receipt_snapshot.is_some() {
                return Err(ApplyError::ReceiptAlreadyStored);
            }

            let snap = payload
                .get("receipt_snapshot")
                .cloned()
                .unwrap_or(Value::Null);
            let tax_snap = tax::replay_tax(&snap, 14_00).map_err(|_| ApplyError::AmountMismatch)?;
            let subtotal = snap
                .get("subtotal_minor")
                .and_then(|v| v.as_i64())
                .or_else(|| payload.get("subtotal_minor").and_then(|v| v.as_i64()))
                .unwrap_or(order.subtotal_minor);
            let total = payload
                .get("total_minor")
                .and_then(|v| v.as_i64())
                .or_else(|| snap.get("total_minor").and_then(|v| v.as_i64()))
                .unwrap_or(order.total_minor);
            if !tax::identity_holds(subtotal, tax_snap.tax_minor, order.discount_minor, total) {
                return Err(ApplyError::TotalIdentity);
            }
            let applied = payload.get("amount_applied_minor").and_then(|v| v.as_i64());
            let tendered = payload
                .get("amount_tendered_minor")
                .and_then(|v| v.as_i64());
            let change = payload.get("change_minor").and_then(|v| v.as_i64());
            let (Some(applied), Some(tendered), Some(change)) = (applied, tendered, change) else {
                return Err(ApplyError::AmountMismatch);
            };
            if applied != total || tendered < applied || change != tendered - applied {
                return Err(ApplyError::AmountMismatch);
            }

            order.status = "paid".into();
            order.amount_paid_minor = applied;
            order.change_minor = change;
            order.tax_minor = tax_snap.tax_minor;
            order.tax_rate_bps = tax_snap.tax_rate_bps;
            order.subtotal_minor = subtotal;
            order.total_minor = total;
            order.receipt_number = payload
                .get("receipt_number")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            order.receipt_snapshot = Some(snap);
            order.closed_by = payload
                .get("closed_by")
                .or_else(|| payload.get("cashier_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            order.closed_at = payload
                .get("closed_at")
                .or_else(|| payload.get("paid_at"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        if self.payments.iter().any(|p| {
            p.order_id == order_id
                && p.payment_type == "sale"
                && p.status == "captured"
                && p.id != payment_id
        }) {
            return Err(ApplyError::DuplicateCapturedSale);
        }
        if !self.payments.iter().any(|p| p.id == payment_id) {
            let applied = payload["amount_applied_minor"].as_i64().unwrap_or(0);
            self.payments.push(CloudPayment {
                id: payment_id,
                order_id: order_id.clone(),
                payment_type: "sale".into(),
                status: "captured".into(),
                amount_applied_minor: applied,
            });
        }
        let captured = self
            .payments
            .iter()
            .filter(|p| {
                p.order_id == order_id && p.payment_type == "sale" && p.status == "captured"
            })
            .count();
        if captured != 1 {
            return Err(ApplyError::DuplicateCapturedSale);
        }
        Ok(())
    }

    fn apply_payment_reversed(
        &mut self,
        branch_id: &str,
        payload: &Value,
    ) -> Result<(), ApplyError> {
        let order = Self::order_mut(&mut self.orders, payload)?;
        if order.branch_id != branch_id {
            return Err(ApplyError::WrongBranch);
        }
        if order.status != "paid" {
            return Err(ApplyError::OrderNotPaid);
        }
        let parent = payload
            .get("parent_payment_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        for p in &mut self.payments {
            if p.id == parent && p.payment_type == "sale" {
                p.status = "reversed".into();
            }
        }
        order.status = "checkout_pending".into();
        order.amount_paid_minor = 0;
        order.change_minor = 0;
        order.closed_by = None;
        order.closed_at = None;
        Ok(())
    }

    pub fn paid_orders(&self) -> usize {
        self.orders.values().filter(|o| o.status == "paid").count()
    }

    pub fn captured_sales(&self) -> usize {
        self.payments
            .iter()
            .filter(|p| p.payment_type == "sale" && p.status == "captured")
            .count()
    }
}

pub fn sample_checkout(id: &str, branch: &str, total: i64) -> CloudOrder {
    CloudOrder {
        id: id.into(),
        branch_id: branch.into(),
        status: "checkout_pending".into(),
        subtotal_minor: total,
        discount_minor: 0,
        tax_minor: 0,
        tax_rate_bps: 0,
        total_minor: total,
        amount_paid_minor: 0,
        change_minor: 0,
        receipt_number: None,
        receipt_snapshot: None,
        closed_by: None,
        closed_at: None,
    }
}

pub fn paid_payload(
    order_id: &str,
    branch: &str,
    payment_id: &str,
    total: i64,
    tendered: i64,
) -> Value {
    let change = tendered - total;
    serde_json::json!({
        "order_id": order_id,
        "branch_id": branch,
        "payment_id": payment_id,
        "total_minor": total,
        "amount_tendered_minor": tendered,
        "amount_applied_minor": total,
        "change_minor": change,
        "currency_code": "EGP",
        "receipt_number": "B-20260829-0001",
        "receipt_snapshot": {
            "tax_minor": 0,
            "tax_rate_bps": 0,
            "subtotal_minor": total,
            "total_minor": total
        },
        "closed_by": "u-c1",
        "closed_at": "2026-08-29T08:00:00Z",
        "cashier_id": "u-c1",
        "paid_at": "2026-08-29T08:00:00Z",
        "payment_method_id": "11111111-1111-1111-1111-111111111111",
        "amount_due_minor": total,
        "subtotal_minor": total,
        "tax_minor": 0,
        "tax_rate_bps": 0,
        "discount_minor": 0
    })
}

pub fn captured_payload(
    order_id: &str,
    branch: &str,
    payment_id: &str,
    total: i64,
    tendered: i64,
) -> Value {
    serde_json::json!({
        "payment_id": payment_id,
        "order_id": order_id,
        "branch_id": branch,
        "payment_method_id": "11111111-1111-1111-1111-111111111111",
        "amount_due_minor": total,
        "amount_tendered_minor": tendered,
        "amount_applied_minor": total,
        "change_minor": tendered - total,
        "cashier_id": "u-c1",
        "paid_at": "2026-08-29T08:00:00Z"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push<'a>(
        ledger: &mut CloudLedger,
        event_id: &'a str,
        branch_id: &'a str,
        seq: i64,
        event_type: &'a str,
        payload: &'a Value,
        hash: &'a str,
    ) -> Result<ApplyStatus, ApplyError> {
        ledger.apply(ApplyRequest {
            event_id,
            branch_id,
            device_id: "d1",
            local_sequence: seq,
            event_type,
            payload,
            payload_hash: hash,
        })
    }

    fn captured_then_paid(ledger: &mut CloudLedger) {
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let cap = captured_payload("o1", "b1", "p1", 5500, 20_000);
        let paid = paid_payload("o1", "b1", "p1", 5500, 20_000);
        assert_eq!(
            push(ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap(),
            ApplyStatus::Applied
        );
        assert_eq!(
            push(ledger, "e2", "b1", 2, "order.paid", &paid, "h2").unwrap(),
            ApplyStatus::Applied
        );
    }

    #[test]
    fn payment_captured_alone_does_not_mark_paid() {
        let mut ledger = CloudLedger::default();
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let cap = captured_payload("o1", "b1", "p1", 5500, 20_000);
        push(&mut ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap();
        assert_eq!(ledger.orders["o1"].status, "checkout_pending");
        assert_eq!(ledger.paid_orders(), 0);
        assert_eq!(ledger.captured_sales(), 0);
        assert!(ledger.orders["o1"].receipt_snapshot.is_none());
    }

    #[test]
    fn payment_captured_alone_is_not_a_completed_sale() {
        let mut ledger = CloudLedger::default();
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let cap = captured_payload("o1", "b1", "p1", 5500, 20_000);
        push(&mut ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap();
        assert_eq!(ledger.paid_orders(), 0);
        assert_eq!(ledger.captured_sales(), 0);
    }

    #[test]
    fn drop_between_captured_and_paid_is_recoverable() {
        let mut ledger = CloudLedger::default();
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let cap = captured_payload("o1", "b1", "p1", 5500, 20_000);
        let paid = paid_payload("o1", "b1", "p1", 5500, 20_000);
        push(&mut ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap();
        assert_eq!(ledger.paid_orders(), 0);
        push(&mut ledger, "e2", "b1", 2, "order.paid", &paid, "h2").unwrap();
        assert_eq!(ledger.paid_orders(), 1);
        assert_eq!(ledger.captured_sales(), 1);
    }

    #[test]
    fn order_paid_atomically_finalizes() {
        let mut ledger = CloudLedger::default();
        captured_then_paid(&mut ledger);
        let order = &ledger.orders["o1"];
        assert_eq!(order.status, "paid");
        assert_eq!(ledger.captured_sales(), 1);
        assert!(order.receipt_snapshot.is_some());
        assert_eq!(order.closed_by.as_deref(), Some("u-c1"));
        assert!(order.closed_at.is_some());
        assert_eq!(order.receipt_number.as_deref(), Some("B-20260829-0001"));
    }

    #[test]
    fn receipt_snapshot_stored_exactly_once() {
        let mut ledger = CloudLedger::default();
        captured_then_paid(&mut ledger);
        let paid = paid_payload("o1", "b1", "p2", 5500, 20_000);
        let err = push(&mut ledger, "e3", "b1", 3, "order.paid", &paid, "h3").unwrap_err();
        assert_eq!(err, ApplyError::AlreadyPaid);
        assert_eq!(ledger.captured_sales(), 1);
    }

    #[test]
    fn tax_snapshot_copied_not_recalculated() {
        let mut ledger = CloudLedger::default();
        captured_then_paid(&mut ledger);
        let snap = ledger.orders["o1"].receipt_snapshot.as_ref().unwrap();
        let replayed = tax::replay_tax(snap, 1400).unwrap();
        assert_eq!(replayed.tax_minor, 0);
        assert_eq!(replayed.tax_rate_bps, 0);
        assert_eq!(ledger.orders["o1"].tax_minor, 0);
    }

    #[test]
    fn duplicate_payment_captured_is_harmless() {
        let mut ledger = CloudLedger::default();
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let cap = captured_payload("o1", "b1", "p1", 5500, 20_000);
        push(&mut ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap();
        let again = push(&mut ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap();
        assert_eq!(again, ApplyStatus::AlreadyProcessed);
        assert_eq!(ledger.captured_sales(), 0);
        assert_eq!(ledger.paid_orders(), 0);
    }

    #[test]
    fn duplicate_order_paid_is_harmless() {
        let mut ledger = CloudLedger::default();
        captured_then_paid(&mut ledger);
        let paid = paid_payload("o1", "b1", "p1", 5500, 20_000);
        let again = push(&mut ledger, "e2", "b1", 2, "order.paid", &paid, "h2").unwrap();
        assert_eq!(again, ApplyStatus::AlreadyProcessed);
        assert_eq!(ledger.captured_sales(), 1);
        assert_eq!(ledger.paid_orders(), 1);
    }

    #[test]
    fn sequence_gap_rejected() {
        let mut ledger = CloudLedger::default();
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let paid = paid_payload("o1", "b1", "p1", 5500, 20_000);
        let err = push(&mut ledger, "e2", "b1", 2, "order.paid", &paid, "h2").unwrap_err();
        assert_eq!(
            err,
            ApplyError::SequenceGap {
                expected: 1,
                got: 2
            }
        );
        assert_eq!(ledger.paid_orders(), 0);
    }

    #[test]
    fn wrong_branch_rejected() {
        let mut ledger = CloudLedger::default();
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let cap = captured_payload("o1", "b1", "p1", 5500, 20_000);
        let err = push(&mut ledger, "e1", "b2", 1, "payment.captured", &cap, "h1").unwrap_err();
        assert_eq!(err, ApplyError::WrongBranch);
    }

    #[test]
    fn amount_mismatch_rejected() {
        let mut ledger = CloudLedger::default();
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let cap = captured_payload("o1", "b1", "p1", 5500, 20_000);
        push(&mut ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap();
        let mut paid = paid_payload("o1", "b1", "p1", 5500, 20_000);
        paid["amount_applied_minor"] = serde_json::json!(100);
        let err = push(&mut ledger, "e2", "b1", 2, "order.paid", &paid, "h2").unwrap_err();
        assert_eq!(err, ApplyError::AmountMismatch);
        assert_eq!(ledger.paid_orders(), 0);
        assert_eq!(ledger.captured_sales(), 0);
    }

    #[test]
    fn reverse_requires_order_paid() {
        let mut ledger = CloudLedger::default();
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let cap = captured_payload("o1", "b1", "p1", 5500, 20_000);
        push(&mut ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap();
        let rev = serde_json::json!({
            "payment_id": "r1",
            "parent_payment_id": "p1",
            "order_id": "o1",
            "branch_id": "b1",
            "amount_applied_minor": 5500,
            "reversed_by": "u-admin",
            "reason": "too soon"
        });
        let err = push(&mut ledger, "e2", "b1", 2, "payment.reversed", &rev, "h3").unwrap_err();
        assert_eq!(err, ApplyError::OrderNotPaid);

        let paid = paid_payload("o1", "b1", "p1", 5500, 20_000);
        push(&mut ledger, "e3", "b1", 2, "order.paid", &paid, "h2").unwrap();
        push(&mut ledger, "e4", "b1", 3, "payment.reversed", &rev, "h4").unwrap();
        assert_eq!(ledger.orders["o1"].status, "checkout_pending");
    }

    #[test]
    fn timeout_replay_cannot_create_second_payment() {
        let mut ledger = CloudLedger::default();
        captured_then_paid(&mut ledger);
        let paid = paid_payload("o1", "b1", "p1", 5500, 20_000);
        push(&mut ledger, "e2", "b1", 2, "order.paid", &paid, "h2").unwrap();
        assert_eq!(ledger.captured_sales(), 1);
        assert_eq!(ledger.paid_orders(), 1);
    }

    #[test]
    fn full_sequence_with_interruption_and_restart() {
        let mut ledger = CloudLedger::default();
        ledger.seed_checkout_order(sample_checkout("o1", "b1", 5500));
        let cap = captured_payload("o1", "b1", "p1", 5500, 20_000);
        let paid = paid_payload("o1", "b1", "p1", 5500, 20_000);
        push(&mut ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap();
        // network drop / worker restart: only sequence 1 is on the cloud
        assert_eq!(ledger.paid_orders(), 0);
        assert_eq!(ledger.captured_sales(), 0);
        push(&mut ledger, "e1", "b1", 1, "payment.captured", &cap, "h1").unwrap();
        push(&mut ledger, "e2", "b1", 2, "order.paid", &paid, "h2").unwrap();
        assert_eq!(ledger.captured_sales(), 1);
        assert_eq!(ledger.paid_orders(), 1);
        assert_eq!(
            ledger
                .orders
                .values()
                .filter(|o| o.receipt_snapshot.is_some())
                .count(),
            1
        );
    }
}
