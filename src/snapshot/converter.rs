use crate::cli::ConvertArgs;
use crate::db::Database;
use crate::event::event_type::{
    derive_aggregate_type, get_aggregate_id, get_aggregate_id_for_type, table_to_created_event_type,
};
use crate::io::{read_to_string, write_string};
use crate::snapshot::dependency_graph::sorted_tables;
use crate::snapshot::table_rules::should_skip_conversion_table;
use anyhow::{anyhow, Result};
use chrono::Utc;
use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use tracing::debug;
use uuid::Uuid;

pub async fn run_convert(args: ConvertArgs, db: Option<&Database>) -> Result<()> {
    let snapshot_json = read_to_string(&args.filename).await?;
    let events = convert_snapshot_to_events(
        &snapshot_json,
        &args.target_host_id,
        &args.admin_user_id,
        db,
    )
    .await?;
    let output = serde_json::to_string_pretty(&events)?;
    write_string(args.output.as_deref(), &output).await
}

pub async fn convert_snapshot_to_events(
    snapshot_json: &str,
    target_host_id: &str,
    admin_user_id: &str,
    db: Option<&Database>,
) -> Result<Vec<Value>> {
    let snapshot: Value = serde_json::from_str(snapshot_json)?;
    let source_host_id = snapshot
        .get("sourceHostId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let tables = snapshot
        .get("tables")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("snapshot missing tables object"))?;
    if tables.is_empty() {
        return Ok(Vec::new());
    }

    let auth_provider_keys_by_provider_id = build_auth_provider_keys_by_provider_id(tables);
    let auth_client_owners_by_owner_id = build_auth_client_owners_by_owner_id(tables);
    let user_host_rows_by_user_id = build_rows_by_user_id(tables, "user_host_t");
    let customer_rows_by_user_id = build_rows_by_user_id(tables, "customer_t");
    let employee_rows_by_user_id = build_rows_by_user_id(tables, "employee_t");
    let api_endpoints_by_version_id = build_api_endpoints_by_version_id(tables);

    let sorted = sorted_tables(tables.keys().cloned(), db).await?;
    let now = Utc::now().to_rfc3339();
    let mut events = Vec::new();

    for table_name in sorted {
        if should_skip_conversion_table(&table_name) {
            debug!(table_name, "skipping projection-owned/runtime table");
            continue;
        }

        let rows = rows_for_table(tables, &table_name);
        if rows.is_empty() {
            continue;
        }

        let event_type = table_to_created_event_type(&table_name);
        let aggregate_type = derive_aggregate_type(&event_type)
            .ok_or_else(|| anyhow!("cannot derive aggregate type for {event_type}"))?;

        for row in rows {
            let mut row = row.clone();
            if let Some(object) = row.as_object_mut() {
                merge_dependent_data(
                    &table_name,
                    object,
                    &auth_provider_keys_by_provider_id,
                    &auth_client_owners_by_owner_id,
                    &user_host_rows_by_user_id,
                    &customer_rows_by_user_id,
                    &employee_rows_by_user_id,
                    &api_endpoints_by_version_id,
                );
            }

            let mut converted_row = deep_copy_and_replace_strings(
                &row,
                source_host_id.as_deref(),
                Some(target_host_id),
            );
            if let Some(object) = converted_row.as_object_mut() {
                object.insert(
                    "aggregateVersion".to_string(),
                    Value::Number(Number::from(0)),
                );
                object.insert(
                    "newAggregateVersion".to_string(),
                    Value::Number(Number::from(1)),
                );
            }

            let aggregate_id = get_aggregate_id(&event_type, &converted_row)
                .or_else(|| get_aggregate_id_for_type(&aggregate_type, &converted_row))
                .ok_or_else(|| {
                    anyhow!(
                        "cannot derive aggregate id for table {table_name} and event type {event_type}"
                    )
                })?;

            let mut event = Map::new();
            event.insert("id".to_string(), Value::String(Uuid::new_v4().to_string()));
            event.insert("data".to_string(), converted_row);
            event.insert(
                "host".to_string(),
                Value::String(target_host_id.to_string()),
            );
            event.insert("time".to_string(), Value::String(now.clone()));
            event.insert("type".to_string(), Value::String(event_type.clone()));
            event.insert("user".to_string(), Value::String(admin_user_id.to_string()));
            event.insert("nonce".to_string(), Value::String("0".to_string()));
            event.insert(
                "source".to_string(),
                Value::String("https://github.com/lightapi/light-portal".to_string()),
            );
            event.insert("subject".to_string(), Value::String(aggregate_id));
            event.insert("specversion".to_string(), Value::String("1.0".to_string()));
            event.insert(
                "aggregatetype".to_string(),
                Value::String(aggregate_type.clone()),
            );
            event.insert(
                "datacontenttype".to_string(),
                Value::String("application/json".to_string()),
            );
            event.insert(
                "aggregateversion".to_string(),
                Value::Number(Number::from(1)),
            );
            events.push(Value::Object(event));
        }
    }

    Ok(events)
}

type RowMap = Map<String, Value>;

fn rows_for_table<'a>(tables: &'a Map<String, Value>, table_name: &str) -> Vec<&'a Value> {
    tables
        .get(table_name)
        .and_then(|table| table.get("rows"))
        .and_then(Value::as_array)
        .map(|rows| rows.iter().collect())
        .unwrap_or_default()
}

fn merge_dependent_data(
    table_name: &str,
    row: &mut RowMap,
    auth_provider_keys_by_provider_id: &HashMap<String, Value>,
    auth_client_owners_by_owner_id: &HashMap<String, RowMap>,
    user_host_rows_by_user_id: &HashMap<String, RowMap>,
    customer_rows_by_user_id: &HashMap<String, RowMap>,
    employee_rows_by_user_id: &HashMap<String, RowMap>,
    api_endpoints_by_version_id: &HashMap<String, Vec<Value>>,
) {
    match table_name {
        "auth_provider_t" => {
            if let Some(provider_id) = row_string(row, "providerId") {
                if let Some(keys) = auth_provider_keys_by_provider_id.get(&provider_id) {
                    row.insert("keys".to_string(), keys.clone());
                }
            }
        }
        "auth_client_t" => {
            if let Some(owner_id) = row_string(row, "ownerId") {
                if let Some(owner) = auth_client_owners_by_owner_id.get(&owner_id) {
                    for field in [
                        "ownerType",
                        "appId",
                        "apiVersionId",
                        "instanceId",
                        "ownerName",
                        "description",
                        "contactEmail",
                        "reviewTs",
                    ] {
                        if let Some(value) = owner.get(field) {
                            row.insert(field.to_string(), value.clone());
                        }
                    }
                }
            }
        }
        "user_t" => {
            merge_user_dependent_data(
                row,
                user_host_rows_by_user_id,
                customer_rows_by_user_id,
                employee_rows_by_user_id,
            );
        }
        "api_version_t" => {
            if let Some(api_version_id) = row_string(row, "apiVersionId") {
                if let Some(endpoints) = api_endpoints_by_version_id.get(&api_version_id) {
                    row.insert("endpoints".to_string(), Value::Array(endpoints.clone()));
                }
            }
        }
        _ => {}
    }
}

fn merge_user_dependent_data(
    user_row: &mut RowMap,
    user_host_rows_by_user_id: &HashMap<String, RowMap>,
    customer_rows_by_user_id: &HashMap<String, RowMap>,
    employee_rows_by_user_id: &HashMap<String, RowMap>,
) {
    let Some(user_id) = row_string(user_row, "userId") else {
        return;
    };

    if let Some(user_host_row) = user_host_rows_by_user_id.get(&user_id) {
        copy_if_present(user_host_row, user_row, "hostId");
        copy_if_present(user_host_row, user_row, "current");
    }

    match row_string(user_row, "userType").as_deref() {
        Some("C") => {
            if let Some(customer_row) = customer_rows_by_user_id.get(&user_id) {
                if let Some(customer_id) = customer_row.get("customerId") {
                    user_row.insert("entityId".to_string(), customer_id.clone());
                }
                copy_if_present(customer_row, user_row, "referralId");
            }
        }
        Some("E") => {
            if let Some(employee_row) = employee_rows_by_user_id.get(&user_id) {
                if let Some(employee_id) = employee_row.get("employeeId") {
                    user_row.insert("entityId".to_string(), employee_id.clone());
                }
                copy_if_present(employee_row, user_row, "managerId");
            }
        }
        _ => {}
    }
}

fn build_auth_provider_keys_by_provider_id(tables: &Map<String, Value>) -> HashMap<String, Value> {
    let mut keys_by_provider = HashMap::<String, Map<String, Value>>::new();
    for row in rows_for_table(tables, "auth_provider_key_t") {
        let Some(object) = row.as_object() else {
            continue;
        };
        let Some(provider_id) = row_string(object, "providerId") else {
            continue;
        };
        let Some(key_type) = row_string(object, "keyType") else {
            continue;
        };

        let mut payload = Map::new();
        for field in ["kid", "keyType", "publicKey", "privateKey"] {
            if let Some(value) = object.get(field) {
                payload.insert(field.to_string(), value.clone());
            }
        }

        keys_by_provider
            .entry(provider_id)
            .or_default()
            .insert(key_type, Value::Object(payload));
    }

    keys_by_provider
        .into_iter()
        .map(|(provider_id, keys)| (provider_id, Value::Object(keys)))
        .collect()
}

fn build_auth_client_owners_by_owner_id(tables: &Map<String, Value>) -> HashMap<String, RowMap> {
    rows_for_table(tables, "auth_client_owner_t")
        .into_iter()
        .filter_map(|row| {
            let object = row.as_object()?.clone();
            let owner_id = row_string(&object, "ownerId")?;
            Some((owner_id, object))
        })
        .collect()
}

fn build_rows_by_user_id(tables: &Map<String, Value>, table_name: &str) -> HashMap<String, RowMap> {
    rows_for_table(tables, table_name)
        .into_iter()
        .filter_map(|row| {
            let object = row.as_object()?.clone();
            let user_id = row_string(&object, "userId")?;
            Some((user_id, object))
        })
        .collect()
}

fn build_api_endpoints_by_version_id(tables: &Map<String, Value>) -> HashMap<String, Vec<Value>> {
    let scopes_by_endpoint_id = build_scopes_by_endpoint_id(tables);
    let mut endpoints_by_version_id = HashMap::<String, Vec<Value>>::new();

    for row in rows_for_table(tables, "api_endpoint_t") {
        let Some(object) = row.as_object() else {
            continue;
        };
        let Some(api_version_id) = row_string(object, "apiVersionId") else {
            continue;
        };
        let Some(endpoint_id) = row_string(object, "endpointId") else {
            continue;
        };

        let mut endpoint = Map::new();
        for field in [
            "endpointId",
            "endpoint",
            "httpMethod",
            "endpointPath",
            "endpointName",
            "toolSchema",
            "toolMetadata",
            "routingDomain",
            "semanticNamespace",
            "sensitivityTier",
            "semanticWeight",
            "sourceProtocol",
            "targetPersonas",
            "endpointDesc",
        ] {
            if let Some(value) = object.get(field) {
                endpoint.insert(field.to_string(), value.clone());
            }
        }
        if let Some(scopes) = scopes_by_endpoint_id.get(&endpoint_id) {
            endpoint.insert(
                "scopes".to_string(),
                Value::Array(scopes.iter().cloned().map(Value::String).collect()),
            );
        }

        endpoints_by_version_id
            .entry(api_version_id)
            .or_default()
            .push(Value::Object(endpoint));
    }

    endpoints_by_version_id
}

fn build_scopes_by_endpoint_id(tables: &Map<String, Value>) -> HashMap<String, Vec<String>> {
    let mut scopes_by_endpoint_id = HashMap::<String, Vec<String>>::new();

    for row in rows_for_table(tables, "api_endpoint_scope_t") {
        let Some(object) = row.as_object() else {
            continue;
        };
        let Some(endpoint_id) = row_string(object, "endpointId") else {
            continue;
        };
        let Some(mut scope) = row_string(object, "scope") else {
            continue;
        };
        if let Some(scope_desc) = row_string(object, "scopeDesc") {
            if !scope_desc.trim().is_empty() {
                scope = format!("{scope}:{scope_desc}");
            }
        }
        scopes_by_endpoint_id
            .entry(endpoint_id)
            .or_default()
            .push(scope);
    }

    scopes_by_endpoint_id
}

fn deep_copy_and_replace_strings(value: &Value, from: Option<&str>, to: Option<&str>) -> Value {
    match value {
        Value::String(value) => {
            if let (Some(from), Some(to)) = (from, to) {
                if !from.is_empty() {
                    return Value::String(value.replace(from, to));
                }
            }
            Value::String(value.clone())
        }
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| deep_copy_and_replace_strings(value, from, to))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), deep_copy_and_replace_strings(value, from, to)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn row_string(row: &RowMap, field: &str) -> Option<String> {
    match row.get(field)? {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn copy_if_present(source: &RowMap, target: &mut RowMap, field: &str) {
    if let Some(value) = source.get(field) {
        target.insert(field.to_string(), value.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rewrites_host_before_subject_derivation() {
        let source_host = "01964b05-552a-7c4b-9184-6857e7f3dc5f";
        let target_host = "01964b05-5532-7c79-8cde-191dcbd421b8";
        let snapshot = serde_json::json!({
            "sourceHostId": source_host,
            "tables": {
                "api_version_t": {
                    "rows": [{
                        "hostId": source_host,
                        "apiVersionId": "api-v1",
                        "apiId": "api",
                        "apiVersion": "1.0.0"
                    }]
                }
            }
        });

        let events = convert_snapshot_to_events(
            &serde_json::to_string(&snapshot).unwrap(),
            target_host,
            "01964b05-5532-7c79-8cde-191dcbd421b9",
            None,
        )
        .await
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["data"]["hostId"], target_host);
        assert_eq!(events[0]["subject"], format!("{target_host}|api-v1"));
    }

    #[tokio::test]
    async fn merges_auth_client_owner_payload() {
        let host = "01964b05-552a-7c4b-9184-6857e7f3dc5f";
        let snapshot = serde_json::json!({
            "sourceHostId": host,
            "tables": {
                "auth_client_owner_t": {
                    "rows": [{
                        "ownerId": "owner-1",
                        "ownerType": "APP",
                        "ownerName": "Owner"
                    }]
                },
                "auth_client_t": {
                    "rows": [{
                        "hostId": host,
                        "clientId": "client-1",
                        "ownerId": "owner-1"
                    }]
                }
            }
        });

        let events = convert_snapshot_to_events(
            &serde_json::to_string(&snapshot).unwrap(),
            host,
            "01964b05-5532-7c79-8cde-191dcbd421b9",
            None,
        )
        .await
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "ClientCreatedEvent");
        assert_eq!(events[0]["data"]["ownerType"], "APP");
        assert_eq!(events[0]["data"]["ownerName"], "Owner");
    }
}
