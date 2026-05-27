use crate::db::Database;
use anyhow::Result;
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};

pub async fn sorted_tables(
    table_names: impl IntoIterator<Item = String>,
    db: Option<&Database>,
) -> Result<Vec<String>> {
    let table_names: BTreeSet<String> = table_names.into_iter().collect();
    let edges = if let Some(db) = db {
        database_edges(db, &table_names).await?
    } else {
        embedded_edges()
            .iter()
            .map(|(parent, child)| ((*parent).to_string(), (*child).to_string()))
            .collect()
    };
    Ok(topological_sort(&table_names, &edges))
}

fn topological_sort(table_names: &BTreeSet<String>, edges: &[(String, String)]) -> Vec<String> {
    let mut children = BTreeMap::<String, BTreeSet<String>>::new();
    let mut in_degree = BTreeMap::<String, usize>::new();
    for table in table_names {
        children.insert(table.clone(), BTreeSet::new());
        in_degree.insert(table.clone(), 0);
    }

    for (parent, child) in edges {
        if parent == child || !table_names.contains(parent) || !table_names.contains(child) {
            continue;
        }
        if children
            .entry(parent.clone())
            .or_default()
            .insert(child.clone())
        {
            *in_degree.entry(child.clone()).or_default() += 1;
        }
    }

    let mut queue: BTreeSet<String> = in_degree
        .iter()
        .filter_map(|(table, degree)| (*degree == 0).then_some(table.clone()))
        .collect();
    let mut sorted = Vec::with_capacity(table_names.len());

    while let Some(table) = queue.pop_first() {
        sorted.push(table.clone());
        if let Some(child_tables) = children.get(&table) {
            for child in child_tables {
                let degree = in_degree.get_mut(child).expect("child exists");
                *degree -= 1;
                if *degree == 0 {
                    queue.insert(child.clone());
                }
            }
        }
    }

    if sorted.len() < table_names.len() {
        for table in table_names {
            if !sorted.contains(table) {
                sorted.push(table.clone());
            }
        }
    }

    sorted
}

async fn database_edges(
    db: &Database,
    table_names: &BTreeSet<String>,
) -> Result<Vec<(String, String)>> {
    let rows = sqlx::query(
        r#"
        SELECT
          ccu.table_name AS parent_table,
          tc.table_name AS child_table
        FROM information_schema.table_constraints AS tc
        JOIN information_schema.key_column_usage AS kcu
          ON tc.constraint_name = kcu.constraint_name
         AND tc.table_schema = kcu.table_schema
        JOIN information_schema.constraint_column_usage AS ccu
          ON ccu.constraint_name = tc.constraint_name
         AND ccu.table_schema = tc.table_schema
        WHERE tc.constraint_type = 'FOREIGN KEY'
          AND tc.table_schema = current_schema()
        "#,
    )
    .fetch_all(db.pool())
    .await?;

    let mut edges = Vec::new();
    for row in rows {
        let parent: String = row.try_get("parent_table")?;
        let child: String = row.try_get("child_table")?;
        if table_names.contains(&parent) && table_names.contains(&child) && parent != child {
            edges.push((parent, child));
        }
    }
    Ok(edges)
}

fn embedded_edges() -> &'static [(&'static str, &'static str)] {
    &[
        ("instance_app_api_t", "instance_app_api_property_t"),
        ("config_property_t", "instance_app_api_property_t"),
        ("instance_t", "instance_file_t"),
        ("auth_session_t", "auth_refresh_token_t"),
        ("auth_session_t", "auth_code_t"),
        ("host_t", "message_t"),
        ("host_t", "product_version_t"),
        ("api_endpoint_t", "api_endpoint_scope_t"),
        ("rule_t", "api_endpoint_rule_t"),
        ("instance_t", "instance_property_t"),
        ("api_endpoint_t", "api_endpoint_rule_t"),
        ("app_t", "app_api_t"),
        ("config_t", "chain_handler_t"),
        ("config_t", "config_property_t"),
        ("host_t", "environment_property_t"),
        ("config_property_t", "instance_property_t"),
        ("config_property_t", "product_property_t"),
        ("product_version_t", "instance_t"),
        ("rule_t", "rule_test_case_t"),
        ("api_t", "api_version_t"),
        ("api_version_t", "api_endpoint_t"),
        ("api_endpoint_scope_t", "app_api_t"),
        ("config_property_t", "environment_property_t"),
        ("platform_t", "pipeline_t"),
        ("instance_t", "deployment_instance_t"),
        ("deployment_instance_t", "deployment_instance_property_t"),
        ("config_property_t", "deployment_instance_property_t"),
        ("instance_t", "instance_api_t"),
        ("api_version_t", "instance_api_t"),
        ("instance_api_t", "instance_api_property_t"),
        ("config_property_t", "instance_api_property_t"),
        ("instance_api_t", "instance_api_path_prefix_t"),
        ("instance_t", "instance_app_t"),
        ("app_t", "instance_app_t"),
        ("instance_app_t", "instance_app_property_t"),
        ("config_property_t", "instance_app_property_t"),
        ("instance_app_t", "instance_app_api_t"),
        ("instance_api_t", "instance_app_api_t"),
        ("product_version_t", "product_version_environment_t"),
        ("product_version_t", "product_version_config_t"),
        ("config_t", "product_version_config_t"),
        ("product_version_t", "product_version_config_property_t"),
        ("config_property_t", "product_version_config_property_t"),
        ("product_version_t", "product_version_property_t"),
        ("config_property_t", "product_version_property_t"),
        ("product_version_t", "product_version_pipeline_t"),
        ("pipeline_t", "product_version_pipeline_t"),
        ("deployment_instance_t", "deployment_t"),
        ("org_t", "host_t"),
        ("ref_table_t", "ref_value_t"),
        ("ref_value_t", "value_locale_t"),
        ("relation_type_t", "relation_t"),
        ("ref_value_t", "relation_t"),
        ("user_t", "user_host_t"),
        ("host_t", "user_host_t"),
        ("user_t", "user_crypto_wallet_t"),
        ("user_host_t", "customer_t"),
        ("user_host_t", "employee_t"),
        ("position_t", "user_position_t"),
        ("position_t", "position_permission_t"),
        ("api_endpoint_t", "position_permission_t"),
        ("position_t", "position_row_filter_t"),
        ("api_endpoint_t", "position_row_filter_t"),
        ("position_t", "position_col_filter_t"),
        ("api_endpoint_t", "position_col_filter_t"),
        ("host_t", "role_t"),
        ("role_t", "role_permission_t"),
        ("api_endpoint_t", "role_permission_t"),
        ("role_t", "role_row_filter_t"),
        ("api_endpoint_t", "role_row_filter_t"),
        ("role_t", "role_col_filter_t"),
        ("api_endpoint_t", "role_col_filter_t"),
        ("user_t", "role_user_t"),
        ("role_t", "role_user_t"),
        ("user_t", "user_permission_t"),
        ("api_endpoint_t", "user_permission_t"),
        ("user_t", "user_row_filter_t"),
        ("api_endpoint_t", "user_row_filter_t"),
        ("user_t", "user_col_filter_t"),
        ("api_endpoint_t", "user_col_filter_t"),
        ("group_t", "group_permission_t"),
        ("api_endpoint_t", "group_permission_t"),
        ("group_t", "group_row_filter_t"),
        ("api_endpoint_t", "group_row_filter_t"),
        ("group_t", "group_col_filter_t"),
        ("api_endpoint_t", "group_col_filter_t"),
        ("user_t", "group_user_t"),
        ("group_t", "group_user_t"),
        ("user_t", "attribute_user_t"),
        ("attribute_t", "attribute_user_t"),
        ("api_endpoint_t", "attribute_permission_t"),
        ("attribute_t", "attribute_permission_t"),
        ("api_endpoint_t", "attribute_row_filter_t"),
        ("attribute_t", "attribute_row_filter_t"),
        ("api_endpoint_t", "attribute_col_filter_t"),
        ("attribute_t", "attribute_col_filter_t"),
        ("host_t", "auth_provider_t"),
        ("auth_provider_t", "auth_provider_key_t"),
        ("auth_provider_t", "auth_provider_api_t"),
        ("api_t", "auth_provider_api_t"),
        ("app_t", "auth_client_owner_t"),
        ("api_version_t", "auth_client_owner_t"),
        ("instance_t", "auth_client_owner_t"),
        ("host_t", "auth_client_owner_t"),
        ("auth_client_owner_t", "auth_client_t"),
        ("app_t", "auth_client_t"),
        ("api_version_t", "auth_client_t"),
        ("host_t", "auth_client_t"),
        ("auth_client_t", "auth_client_token_t"),
        ("auth_provider_t", "auth_provider_client_t"),
        ("auth_client_t", "auth_provider_client_t"),
        ("user_t", "auth_code_t"),
        ("auth_provider_client_t", "auth_code_t"),
        ("host_t", "auth_code_t"),
        ("user_t", "auth_refresh_token_t"),
        ("auth_provider_client_t", "auth_refresh_token_t"),
        ("host_t", "auth_refresh_token_t"),
        ("user_t", "auth_session_t"),
        ("auth_provider_client_t", "auth_session_t"),
        ("host_t", "auth_session_t"),
        ("host_t", "auth_session_audit_t"),
        ("host_t", "auth_ref_token_t"),
        ("auth_client_t", "auth_ref_token_t"),
        ("host_t", "notification_t"),
        ("host_t", "private_conversation_t"),
        ("private_conversation_t", "private_message_t"),
        ("private_message_t", "private_message_state_t"),
        ("deployment_t", "config_snapshot_t"),
        ("user_t", "config_snapshot_t"),
        ("host_t", "config_snapshot_t"),
        ("instance_t", "config_snapshot_t"),
        ("config_snapshot_t", "config_snapshot_property_t"),
        ("config_snapshot_t", "snapshot_instance_file_t"),
        (
            "config_snapshot_t",
            "snapshot_deployment_instance_property_t",
        ),
        ("config_snapshot_t", "snapshot_instance_api_property_t"),
        ("config_snapshot_t", "snapshot_instance_app_property_t"),
        ("config_snapshot_t", "snapshot_instance_app_api_property_t"),
        ("config_snapshot_t", "snapshot_instance_property_t"),
        ("config_snapshot_t", "snapshot_environment_property_t"),
        ("config_snapshot_t", "snapshot_product_property_t"),
        ("config_snapshot_t", "snapshot_product_version_property_t"),
        ("worklist_t", "worklist_column_t"),
        ("wf_definition_t", "process_info_t"),
        ("process_info_t", "task_info_t"),
        ("task_info_t", "task_asst_t"),
        ("api_version_t", "agent_definition_t"),
        ("api_endpoint_t", "tool_t"),
        ("tool_t", "tool_param_t"),
        ("skill_t", "skill_dependency_t"),
        ("agent_definition_t", "agent_skill_t"),
        ("skill_t", "agent_skill_t"),
        ("skill_t", "skill_tool_t"),
        ("tool_t", "skill_tool_t"),
        ("skill_t", "skill_workflow_t"),
        ("wf_definition_t", "skill_workflow_t"),
        ("host_t", "agent_memory_bank_t"),
        ("agent_definition_t", "agent_memory_bank_t"),
        ("user_t", "agent_memory_bank_t"),
        ("agent_memory_bank_t", "agent_memory_doc_t"),
        ("agent_memory_bank_t", "agent_memory_unit_t"),
        ("agent_memory_doc_t", "agent_memory_unit_t"),
        ("agent_memory_bank_t", "agent_memory_entity_t"),
        ("user_t", "agent_memory_entity_t"),
        ("agent_memory_unit_t", "agent_memory_unit_entity_t"),
        ("agent_memory_entity_t", "agent_memory_unit_entity_t"),
        ("agent_memory_entity_t", "agent_memory_entity_cooccur_t"),
        ("agent_memory_unit_t", "agent_memory_link_t"),
        ("agent_memory_bank_t", "agent_memory_directive_t"),
        ("agent_memory_bank_t", "agent_memory_reflection_t"),
        ("agent_memory_bank_t", "agent_session_history_t"),
        ("host_t", "pii_token_vault_t"),
        ("pii_token_scheme_t", "pii_token_vault_t"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_sort_places_parent_before_child() {
        let tables = BTreeSet::from([
            "api_endpoint_t".to_string(),
            "api_version_t".to_string(),
            "api_endpoint_scope_t".to_string(),
        ]);
        let edges = embedded_edges()
            .iter()
            .map(|(parent, child)| ((*parent).to_string(), (*child).to_string()))
            .collect::<Vec<_>>();
        let sorted = topological_sort(&tables, &edges);
        assert!(
            sorted.iter().position(|table| table == "api_version_t")
                < sorted.iter().position(|table| table == "api_endpoint_t")
        );
        assert!(
            sorted.iter().position(|table| table == "api_endpoint_t")
                < sorted
                    .iter()
                    .position(|table| table == "api_endpoint_scope_t")
        );
    }
}
