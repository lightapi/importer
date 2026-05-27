use anyhow::{anyhow, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct EventMutator {
    replacement_rules: Vec<ReplacementRule>,
    enrichment_rules: Vec<EnrichmentRule>,
    generated_id_map: HashMap<String, String>,
}

impl EventMutator {
    pub fn new(replacement_json: Option<&str>, enrichment_json: Option<&str>) -> Result<Self> {
        Ok(Self {
            replacement_rules: parse_replacements(replacement_json)?,
            enrichment_rules: parse_enrichments(enrichment_json)?,
            generated_id_map: HashMap::new(),
        })
    }

    pub fn has_replacements(&self) -> bool {
        !self.replacement_rules.is_empty()
    }

    pub fn mutate(&mut self, value: &mut Value) {
        for rule in &self.replacement_rules {
            if looks_like_uuid(&rule.from) && looks_like_uuid(&rule.to) {
                replace_string_occurrences(value, &rule.from, &rule.to);
            }
            if let Some(field) = rule.field.as_deref() {
                replace_field_values(value, field, &rule.from, &rule.to);
            }
        }

        let rules = self.enrichment_rules.clone();
        for rule in rules {
            let generated = match rule.action.as_str() {
                action if action.eq_ignore_ascii_case("generateUUID") => {
                    Some(Uuid::new_v4().to_string())
                }
                action
                    if action.eq_ignore_ascii_case("mapGenerate")
                        || action.eq_ignore_ascii_case("mapAndGenerate") =>
                {
                    let original = rule
                        .source_field
                        .as_deref()
                        .and_then(|field| value.get(field))
                        .and_then(value_to_string);
                    Some(match original {
                        Some(original) => self
                            .generated_id_map
                            .entry(format!("{}:{original}", rule.field))
                            .or_insert_with(|| Uuid::new_v4().to_string())
                            .clone(),
                        None => Uuid::new_v4().to_string(),
                    })
                }
                _ => None,
            };

            if let Some(generated) = generated {
                if let Some(object) = value.as_object_mut() {
                    if object.contains_key(&rule.field) {
                        object.insert(rule.field, Value::String(generated));
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplacementRule {
    field: Option<String>,
    from: String,
    to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EnrichmentRule {
    field: String,
    action: String,
    source_field: Option<String>,
}

fn parse_replacements(json: Option<&str>) -> Result<Vec<ReplacementRule>> {
    let Some(json) = json.filter(|value| !value.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    let value: Value = serde_json::from_str(json)?;
    let rules = rules_array(&value, "replacement")?;
    rules
        .iter()
        .map(|rule| {
            let object = rule
                .as_object()
                .ok_or_else(|| anyhow!("replacement rule must be an object"))?;
            let from = string_field(object, &["from", "fromValue"])
                .ok_or_else(|| anyhow!("replacement rule missing from/fromValue"))?;
            let to = string_field(object, &["to", "toValue"])
                .ok_or_else(|| anyhow!("replacement rule missing to/toValue"))?;
            Ok(ReplacementRule {
                field: string_field(object, &["field", "fieldName"]),
                from,
                to,
            })
        })
        .collect()
}

fn parse_enrichments(json: Option<&str>) -> Result<Vec<EnrichmentRule>> {
    let Some(json) = json.filter(|value| !value.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    let value: Value = serde_json::from_str(json)?;
    let rules = rules_array(&value, "enrichment")?;
    rules
        .iter()
        .map(|rule| {
            let object = rule
                .as_object()
                .ok_or_else(|| anyhow!("enrichment rule must be an object"))?;
            let field = string_field(object, &["field", "fieldName"])
                .ok_or_else(|| anyhow!("enrichment rule missing field/fieldName"))?;
            let action = string_field(object, &["action"])
                .ok_or_else(|| anyhow!("enrichment rule missing action"))?;
            Ok(EnrichmentRule {
                field,
                action,
                source_field: string_field(object, &["sourceField", "source_field"]),
            })
        })
        .collect()
}

fn rules_array<'a>(value: &'a Value, object_key: &str) -> Result<&'a Vec<Value>> {
    if let Some(array) = value.as_array() {
        return Ok(array);
    }
    value
        .get(object_key)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("mutation rules must be a JSON array or a versioned rule document"))
}

fn string_field(object: &Map<String, Value>, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| object.get(*name))
        .and_then(value_to_string)
        .filter(|value| !value.is_empty())
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn replace_string_occurrences(value: &mut Value, from: &str, to: &str) {
    match value {
        Value::String(text) if text.contains(from) => {
            *text = text.replace(from, to);
        }
        Value::Array(values) => {
            for value in values {
                replace_string_occurrences(value, from, to);
            }
        }
        Value::Object(object) => {
            for value in object.values_mut() {
                replace_string_occurrences(value, from, to);
            }
        }
        _ => {}
    }
}

fn replace_field_values(value: &mut Value, field: &str, from: &str, to: &str) {
    match value {
        Value::Array(values) => {
            for value in values {
                replace_field_values(value, field, from, to);
            }
        }
        Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                if key == field && value_to_string(value).as_deref() == Some(from) {
                    *value = Value::String(to.to_string());
                } else {
                    replace_field_values(value, field, from, to);
                }
            }
        }
        _ => {}
    }
}

fn looks_like_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_uuid_strings_recursively_and_field_aliases() {
        let from = "01964b05-552a-7c4b-9184-6857e7f3dc5f";
        let to = "01964b05-5532-7c79-8cde-191dcbd421b8";
        let mut mutator = EventMutator::new(
            Some(&format!(
                r#"[{{"fieldName":"hostId","fromValue":"{from}","toValue":"{to}"}}]"#
            )),
            None,
        )
        .unwrap();
        let mut value = serde_json::json!({
            "host": from,
            "data": {"hostId": from, "nested": ["prefix-01964b05-552a-7c4b-9184-6857e7f3dc5f"]}
        });

        mutator.mutate(&mut value);

        assert_eq!(value["host"], to);
        assert_eq!(value["data"]["hostId"], to);
        assert_eq!(value["data"]["nested"][0], format!("prefix-{to}"));
    }

    #[test]
    fn parses_versioned_enrichment_doc() {
        let mut mutator = EventMutator::new(
            None,
            Some(
                r#"{
                    "schemaVersion": 1,
                    "enrichment": [{"field":"id","action":"generateUUID"}]
                }"#,
            ),
        )
        .unwrap();
        let mut value = serde_json::json!({"id":"old"});
        mutator.mutate(&mut value);
        assert_ne!(value["id"], "old");
    }
}
