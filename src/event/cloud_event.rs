use crate::event::event_type::{derive_aggregate_type, get_aggregate_id};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json::{Map, Number, Value};
use uuid::Uuid;

const CORE_ATTRIBUTES: &[&str] = &[
    "id",
    "source",
    "type",
    "specversion",
    "time",
    "subject",
    "datacontenttype",
    "dataschema",
    "data",
];

const PORTAL_EXTENSIONS: &[&str] = &["host", "user", "nonce", "aggregatetype", "aggregateversion"];

#[derive(Debug, Clone)]
pub struct NormalizedEvent {
    pub value: Value,
    pub id: Uuid,
    pub host_id: Uuid,
    pub user_id: Uuid,
    pub event_type: String,
    pub event_ts: DateTime<Utc>,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub aggregate_version: i64,
}

impl NormalizedEvent {
    pub fn metadata(&self) -> Value {
        let Some(object) = self.value.as_object() else {
            return Value::Object(Map::new());
        };
        let mut metadata = Map::new();
        for (key, value) in object {
            if CORE_ATTRIBUTES.contains(&key.as_str()) || PORTAL_EXTENSIONS.contains(&key.as_str())
            {
                continue;
            }
            metadata.insert(key.clone(), value.clone());
        }
        Value::Object(metadata)
    }

    pub fn set_nonce(&mut self, nonce: i64) {
        if let Some(object) = self.value.as_object_mut() {
            object.insert("nonce".to_string(), Value::Number(Number::from(nonce)));
        }
    }

    pub fn payload(&self) -> Value {
        self.value.clone()
    }
}

pub fn normalize_event(mut value: Value, recompute_subject: bool) -> Result<NormalizedEvent> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow!("event must be a JSON object"))?;

    let event_type = required_string(object, "type")?;
    let aggregate_version = aggregate_version(object)?;
    object.insert(
        "aggregateversion".to_string(),
        Value::Number(Number::from(aggregate_version)),
    );

    let aggregate_type = match object.get("aggregatetype").and_then(Value::as_str) {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => {
            let aggregate_type = derive_aggregate_type(&event_type)
                .ok_or_else(|| anyhow!("cannot derive aggregatetype from {event_type}"))?;
            object.insert(
                "aggregatetype".to_string(),
                Value::String(aggregate_type.clone()),
            );
            aggregate_type
        }
    };

    if recompute_subject || object.get("subject").and_then(Value::as_str).is_none() {
        let data = object
            .get("data")
            .ok_or_else(|| anyhow!("CloudEvent missing data for subject derivation"))?;
        if let Some(subject) = get_aggregate_id(&event_type, data) {
            object.insert("subject".to_string(), Value::String(subject));
        }
    }

    let id = parse_uuid(&required_string(object, "id")?, "id")?;
    let host_id = parse_uuid(&required_string(object, "host")?, "host")?;
    let user_id = parse_uuid(&required_string(object, "user")?, "user")?;
    let event_ts = parse_time(&required_string(object, "time")?)?;
    let aggregate_id = required_string(object, "subject")?;

    if aggregate_id.trim().is_empty() {
        return Err(anyhow!("CloudEvent missing required subject/aggregate id"));
    }

    Ok(NormalizedEvent {
        value,
        id,
        host_id,
        user_id,
        event_type,
        event_ts,
        aggregate_id,
        aggregate_type,
        aggregate_version,
    })
}

fn aggregate_version(object: &Map<String, Value>) -> Result<i64> {
    for key in ["aggregateversion", "aggregateVersion"] {
        if let Some(value) = object.get(key) {
            return value_as_i64(value)
                .ok_or_else(|| anyhow!("{key} must be a number or numeric string"));
        }
    }
    Ok(1)
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(value) => value.as_i64(),
        Value::String(value) if !value.is_empty() => value.parse().ok(),
        _ => None,
    }
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(|value| match value {
            Value::String(value) if !value.is_empty() => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .ok_or_else(|| anyhow!("CloudEvent missing required {key}"))
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| anyhow!("{field} must be a UUID: {err}"))
}

fn parse_time(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_aggregate_version_alias_and_subject() {
        let value = serde_json::json!({
            "id": "01964b05-552a-7c4b-9184-6857e7f3dc5f",
            "source": "test",
            "type": "ApiVersionCreatedEvent",
            "time": "2026-05-27T12:00:00Z",
            "specversion": "1.0",
            "datacontenttype": "application/json",
            "host": "01964b05-552a-7c4b-9184-6857e7f3dc60",
            "user": "01964b05-552a-7c4b-9184-6857e7f3dc61",
            "aggregateVersion": 3,
            "data": {
                "hostId": "01964b05-552a-7c4b-9184-6857e7f3dc60",
                "apiVersionId": "v1"
            }
        });

        let event = normalize_event(value, true).unwrap();

        assert_eq!(event.aggregate_version, 3);
        assert_eq!(
            event.aggregate_id,
            "01964b05-552a-7c4b-9184-6857e7f3dc60|v1"
        );
        assert_eq!(event.aggregate_type, "ApiVersion");
        assert_eq!(event.value["aggregateversion"], 3);
    }
}
