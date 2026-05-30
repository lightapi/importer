use std::collections::HashSet;

pub fn should_skip_conversion_table(table_name: &str) -> bool {
    auth_state_skip_tables().contains(table_name)
        || conversion_skip_tables().contains(table_name)
        || table_name.starts_with("snapshot_")
}

fn auth_state_skip_tables() -> HashSet<&'static str> {
    HashSet::from([
        "auth_session_audit_t",
        "auth_session_t",
        "auth_refresh_token_t",
        "auth_code_t",
        "auth_ref_token_t",
        "auth_client_token_t",
    ])
}

fn conversion_skip_tables() -> HashSet<&'static str> {
    HashSet::from([
        "employee_t",
        "customer_t",
        "notification_t",
        "user_host_t",
        "user_crypto_wallet_t",
        "auth_provider_key_t",
        "auth_client_owner_t",
        "api_endpoint_t",
        "api_endpoint_scope_t",
        "private_conversation_t",
        "private_message_t",
        "private_message_state_t",
        "agent_memory_entity_cooccur_t",
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_explicit_auth_runtime_skip_list() {
        assert!(should_skip_conversion_table("auth_session_t"));
        assert!(!should_skip_conversion_table("auth_provider_t"));
        assert!(!should_skip_conversion_table("auth_client_t"));
    }
}
