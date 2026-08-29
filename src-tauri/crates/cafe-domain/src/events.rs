use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const CATALOG_JSON: &str = include_str!("../../../../contracts/events.json");

#[derive(Debug, Deserialize)]
pub struct EventCatalog {
    pub version: u32,
    pub events: Vec<EventSpec>,
}

#[derive(Debug, Deserialize)]
pub struct EventSpec {
    #[serde(rename = "type")]
    pub event_type: String,
    pub aggregate: String,
    pub payload_required: Vec<String>,
}

pub fn catalog() -> EventCatalog {
    serde_json::from_str(CATALOG_JSON).expect("event catalog must parse")
}

pub fn is_known_event(event_type: &str) -> bool {
    catalog().events.iter().any(|e| e.event_type == event_type)
}

pub fn aggregate_for(event_type: &str) -> Option<String> {
    catalog()
        .events
        .iter()
        .find(|e| e.event_type == event_type)
        .map(|e| e.aggregate.clone())
}

pub fn validate_payload(event_type: &str, payload: &serde_json::Value) -> Result<(), String> {
    let catalog = catalog();
    let spec = catalog
        .events
        .iter()
        .find(|e| e.event_type == event_type)
        .ok_or_else(|| format!("unknown event type {event_type}"))?;
    let obj = payload
        .as_object()
        .ok_or_else(|| "payload must be an object".to_string())?;
    for key in &spec.payload_required {
        if !obj.contains_key(key) {
            return Err(format!("missing payload field {key} for {event_type}"));
        }
    }
    Ok(())
}

pub fn payload_hash(payload: &serde_json::Value) -> String {
    let canonical = serde_json::to_vec(payload).unwrap_or_default();
    let digest = Sha256::digest(canonical);
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_mvp_events() {
        let types: Vec<_> = catalog().events.into_iter().map(|e| e.event_type).collect();
        for required in [
            "session.started",
            "session.stopped",
            "order.item_added",
            "order.paid",
            "payment.reversed",
            "inventory.adjusted",
        ] {
            assert!(types.contains(&required.to_string()), "missing {required}");
        }
    }

    #[test]
    fn rejects_unknown_and_incomplete() {
        assert!(!is_known_event("money.laundered"));
        let payload = serde_json::json!({"order_id": "x"});
        assert!(validate_payload("order.paid", &payload).is_err());
    }

    #[test]
    fn hash_is_stable() {
        let a = serde_json::json!({"a": 1, "b": 2});
        let b = serde_json::json!({"a": 1, "b": 2});
        assert_eq!(payload_hash(&a), payload_hash(&b));
    }
}
