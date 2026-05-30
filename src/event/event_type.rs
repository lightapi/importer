use serde_json::Value;

const EVENT_SUFFIXES: &[&str] = &[
    "CreatedEvent",
    "UpdatedEvent",
    "DeletedEvent",
    "OnboardedEvent",
    "ConfirmedEvent",
    "VerifiedEvent",
    "ForgotEvent",
    "ResetEvent",
    "ChangedEvent",
    "LockedEvent",
    "UnlockedEvent",
    "CancelledEvent",
    "DeliveredEvent",
    "SwitchedEvent",
    "SentEvent",
    "RotatedEvent",
    "QueriedEvent",
    "ClonedEvent",
    "StartedEvent",
    "AppendedEvent",
    "CompactedEvent",
    "RetainedEvent",
    "LinkedEvent",
    "UnlinkedEvent",
];

pub fn derive_aggregate_type(event_type: &str) -> Option<String> {
    for suffix in EVENT_SUFFIXES {
        if let Some(aggregate) = event_type.strip_suffix(suffix) {
            if !aggregate.is_empty() {
                return Some(aggregate.to_string());
            }
        }
    }
    match event_type {
        "PlatformQueriedEvent" => Some("Platform".to_string()),
        _ => None,
    }
}

pub fn get_aggregate_id(event_type: &str, data: &Value) -> Option<String> {
    let aggregate_type = derive_aggregate_type(event_type)?;
    get_aggregate_id_for_type(&aggregate_type, data)
}

pub fn get_aggregate_id_for_type(aggregate_type: &str, data: &Value) -> Option<String> {
    match aggregate_type {
        "Config" => value(data, "configId"),
        "ConfigProperty" => value(data, "propertyId"),
        "ConfigDeploymentInstance" => {
            compound(data, &["hostId", "deploymentInstanceId", "propertyId"])
        }
        "ConfigEnvironment" => compound(data, &["hostId", "environment", "propertyId"]),
        "ConfigInstance" | "InstanceProperty" => {
            compound(data, &["hostId", "instanceId", "propertyId"])
        }
        "ConfigInstanceApi" | "InstanceApiProperty" => {
            compound(data, &["hostId", "instanceApiId", "propertyId"])
        }
        "ConfigInstanceApp" | "InstanceAppProperty" => {
            compound(data, &["hostId", "instanceAppId", "propertyId"])
        }
        "ConfigInstanceAppApi" | "InstanceAppApiProperty" => compound(
            data,
            &["hostId", "instanceAppId", "instanceApiId", "propertyId"],
        ),
        "ConfigInstanceFile" | "InstanceFile" => compound(data, &["hostId", "instanceFileId"]),
        "ConfigProduct" | "ProductProperty" => compound(data, &["productId", "propertyId"]),
        "ConfigProductVersion" | "ProductVersionProperty" => {
            compound(data, &["hostId", "productVersionId", "propertyId"])
        }
        "ConfigSnapshot" => compound(data, &["hostId", "snapshotId"]),
        "RefTable" => optional_host_compound(data, &["tableId"]),
        "RefValue" => value(data, "valueId"),
        "RefLocale" => compound(data, &["valueId", "language"]),
        "RefRelationType" => value(data, "relationId"),
        "RefRelation" => compound(data, &["relationId", "valueIdFrom", "valueIdTo"]),
        "Api" => compound(data, &["hostId", "apiId"]),
        "ApiVersion" | "ApiVersionSpec" => compound(data, &["hostId", "apiVersionId"]),
        "ApiEndpoint" => compound(data, &["hostId", "endpointId"]),
        "ApiEndpointScope" => compound(data, &["hostId", "endpointId", "scope"]),
        "ApiEndpointRule" => compound(data, &["hostId", "endpointId", "ruleId"]),
        "Rule" => optional_host_compound(data, &["ruleId"]),
        "RuleTestCase" => optional_host_compound(data, &["ruleId", "testId"]),
        "Role" => compound(data, &["hostId", "roleId"]),
        "RolePermission" | "RoleColFilter" => compound(data, &["hostId", "roleId", "endpointId"]),
        "RoleUser" => compound(data, &["hostId", "roleId", "userId"]),
        "RoleRowFilter" => compound(data, &["hostId", "roleId", "endpointId", "colName"]),
        "Group" => compound(data, &["hostId", "groupId"]),
        "GroupPermission" | "GroupColFilter" => {
            compound(data, &["hostId", "groupId", "endpointId"])
        }
        "GroupUser" => compound(data, &["hostId", "groupId", "userId"]),
        "GroupRowFilter" => compound(data, &["hostId", "groupId", "endpointId", "colName"]),
        "Position" => compound(data, &["hostId", "positionId"]),
        "PositionPermission" | "PositionColFilter" => {
            compound(data, &["hostId", "positionId", "endpointId"])
        }
        "PositionUser" => compound(data, &["hostId", "positionId", "userId"]),
        "PositionRowFilter" => compound(data, &["hostId", "positionId", "endpointId", "colName"]),
        "Attribute" => compound(data, &["hostId", "attributeId"]),
        "AttributePermission" | "AttributeColFilter" => {
            compound(data, &["hostId", "attributeId", "endpointId"])
        }
        "AttributeUser" => compound(data, &["hostId", "attributeId", "userId"]),
        "AttributeRowFilter" => compound(data, &["hostId", "attributeId", "endpointId", "colName"]),
        "Deployment" => compound(data, &["hostId", "deploymentId"]),
        "DeploymentInstance" => compound(data, &["hostId", "deploymentInstanceId"]),
        "Platform" => compound(data, &["hostId", "platformId"]),
        "Pipeline" => compound(data, &["hostId", "pipelineId"]),
        "ProductVersion" => compound(data, &["hostId", "productVersionId"]),
        "ProductVersionConfig" => compound(data, &["hostId", "productVersionId", "configId"]),
        "ProductVersionConfigProperty" => {
            compound(data, &["hostId", "productVersionId", "propertyId"])
        }
        "ProductVersionEnvironment" => compound(
            data,
            &["hostId", "productVersionId", "systemEnv", "runtimeEnv"],
        ),
        "ProductVersionPipeline" => compound(data, &["hostId", "productVersionId", "pipelineId"]),
        "Instance" => compound(data, &["hostId", "instanceId"]),
        "InstanceApi" | "CompositeInstanceApi" => compound(data, &["hostId", "instanceApiId"]),
        "InstanceApp" | "CompositeInstanceApp" => compound(data, &["hostId", "instanceAppId"]),
        "InstanceAppApi" => compound(data, &["hostId", "instanceAppId", "instanceApiId"]),
        "InstanceApiPathPrefix" => compound(data, &["hostId", "instanceApiId", "pathPrefix"]),
        "App" => compound(data, &["hostId", "appId"]),
        "Client" => compound(data, &["hostId", "clientId"]),
        "ClientToken" => compound(data, &["hostId", "clientId", "tokenId"]),
        "AuthProvider" => compound(data, &["hostId", "providerId"]),
        "AuthProviderApi" => compound(data, &["hostId", "providerId", "apiId"]),
        "AuthProviderClient" => compound(data, &["hostId", "providerId", "clientId"]),
        "AuthRefToken" => compound(data, &["hostId", "refToken"]),
        "PiiTokenScheme" => value(data, "schemeId"),
        "PiiTokenVault" => compound(data, &["hostId", "token"]),
        "Category" => optional_host_compound(data, &["categoryId"]),
        "EntityCategory" => compound(data, &["entityId", "entityType", "categoryId"]),
        "Host" => value(data, "hostId"),
        "UserHost" => compound(data, &["userId", "hostId"]),
        "Org" => value(data, "domain"),
        "Schema" => optional_host_compound(data, &["schemaId"]),
        "Schedule" => compound(data, &["hostId", "scheduleId"]),
        "Tag" => optional_host_compound(data, &["tagId"]),
        "EntityTag" => compound(data, &["entityId", "entityType", "tagId"]),
        "User" => value(data, "userId"),
        "PrivateMessage" => value(data, "conversationId"),
        "AgentDefinition" => {
            let agent_def_id = value(data, "agentDefId").or_else(|| value(data, "apiVersionId"))?;
            Some(format!("{}|{agent_def_id}", value(data, "hostId")?))
        }
        "Tool" => compound(data, &["hostId", "toolId"]),
        "ToolParam" => compound(data, &["hostId", "paramId"]),
        "Skill" => compound(data, &["hostId", "skillId"]),
        "SkillTaxonomy" => {
            let host_id = value(data, "hostId")?;
            let skill_id = value(data, "skillId")?;
            Some(format!("{host_id}|{skill_id}|taxonomy"))
        }
        "SkillTool" => compound(data, &["hostId", "skillId", "toolId"]),
        "SkillWorkflow" => compound(data, &["hostId", "skillId", "wfDefId", "workflowRole"]),
        "SkillDependency" => compound(data, &["hostId", "skillId", "dependsOnSkillId"]),
        "AgentSkill" => compound(data, &["hostId", "agentDefId", "skillId"]),
        "AgentSessionHistory" => {
            if let Some(id) = compound(data, &["hostId", "bankId", "sessionId"]) {
                Some(id)
            } else {
                compound(data, &["hostId", "sessionHistoryId"])
            }
        }
        "AgentMemoryBank" => compound(data, &["hostId", "bankId"]),
        "AgentMemoryDoc" => compound(data, &["hostId", "bankId", "docId"]),
        "AgentMemoryUnit" => compound(data, &["hostId", "bankId", "unitId"]),
        "AgentMemoryEntity" => compound(data, &["hostId", "bankId", "entityId"]),
        "AgentMemoryUnitEntity" => compound(data, &["hostId", "bankId", "unitId", "entityId"]),
        "AgentMemoryLink" => compound(data, &["hostId", "bankId", "fromUnitId", "toUnitId", "linkType"]),
        "AgentMemoryDirective" => compound(data, &["hostId", "bankId", "directiveId"]),
        "AgentMemoryReflection" => compound(data, &["hostId", "bankId", "reflectionId"]),
        "WorkflowDefinition" => compound(data, &["hostId", "wfDefId"]),
        "AuditLog" => compound(data, &["hostId", "auditLogId"]),
        "Workflow" => compound(data, &["hostId", "wfInstanceId"]),
        "RuntimeInstance" => compound(data, &["hostId", "runtimeInstanceId"]),
        "Worklist" => compound(data, &["hostId", "assigneeId", "categoryId"]),
        "WorklistColumn" => compound(data, &["hostId", "assigneeId", "categoryId", "sequenceId"]),
        "ProcessInfo" => compound(data, &["hostId", "processId"]),
        "TaskInfo" => compound(data, &["hostId", "taskId"]),
        "TaskAssignment" => compound(data, &["hostId", "taskAsstId"]),
        _ => fallback_id(aggregate_type, data),
    }
}

pub fn table_to_created_event_type(table_name: &str) -> String {
    match table_name {
        "environment_property_t" => "ConfigEnvironmentCreatedEvent",
        "instance_api_property_t" => "ConfigInstanceApiCreatedEvent",
        "instance_app_property_t" => "ConfigInstanceAppCreatedEvent",
        "instance_app_api_property_t" => "ConfigInstanceAppApiCreatedEvent",
        "instance_file_t" => "ConfigInstanceFileCreatedEvent",
        "deployment_instance_property_t" => "ConfigDeploymentInstanceCreatedEvent",
        "instance_property_t" => "ConfigInstanceCreatedEvent",
        "product_property_t" => "ConfigProductCreatedEvent",
        "product_version_property_t" => "ConfigProductVersionCreatedEvent",
        "value_locale_t" => "RefLocaleCreatedEvent",
        "relation_type_t" => "RefRelationTypeCreatedEvent",
        "relation_t" => "RefRelationCreatedEvent",
        "auth_client_t" => "ClientCreatedEvent",
        "wf_definition_t" => "WorkflowDefinitionCreatedEvent",
        "agent_memory_bank_t" => "AgentMemoryBankCreatedEvent",
        "agent_memory_doc_t" => "AgentMemoryDocCreatedEvent",
        "agent_memory_unit_t" => "AgentMemoryUnitRetainedEvent",
        "agent_memory_entity_t" => "AgentMemoryEntityCreatedEvent",
        "agent_memory_unit_entity_t" => "AgentMemoryUnitEntityLinkedEvent",
        "agent_memory_link_t" => "AgentMemoryLinkCreatedEvent",
        "agent_memory_directive_t" => "AgentMemoryDirectiveCreatedEvent",
        "agent_memory_reflection_t" => "AgentMemoryReflectionCreatedEvent",
        "agent_session_history_t" => "AgentSessionHistoryCreatedEvent",
        _ => return format!("{}CreatedEvent", table_to_aggregate_type(table_name)),
    }
    .to_string()
}

pub fn table_to_aggregate_type(table_name: &str) -> String {
    let base = table_name.strip_suffix("_t").unwrap_or(table_name);
    snake_to_pascal(base)
}

fn fallback_id(aggregate_type: &str, data: &Value) -> Option<String> {
    let mut chars = aggregate_type.chars();
    let first = chars.next()?.to_lowercase().to_string();
    let id_field = format!("{first}{}Id", chars.as_str());

    value(data, &id_field)
        .or_else(|| value(data, "email"))
        .or_else(|| value(data, "roleId"))
        .or_else(|| {
            if data.as_object().map(|object| object.len()) == Some(1) {
                value(data, "hostId")
            } else {
                None
            }
        })
}

fn compound(data: &Value, fields: &[&str]) -> Option<String> {
    let mut parts = Vec::with_capacity(fields.len());
    for field in fields {
        parts.push(value(data, field)?);
    }
    Some(parts.join("|"))
}

fn optional_host_compound(data: &Value, fields: &[&str]) -> Option<String> {
    let mut parts = Vec::with_capacity(fields.len() + 1);
    if let Some(host_id) = value(data, "hostId") {
        parts.push(host_id);
    }
    for field in fields {
        parts.push(value(data, field)?);
    }
    Some(parts.join("|"))
}

fn value(data: &Value, field: &str) -> Option<String> {
    let value = data.get(field)?;
    match value {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn snake_to_pascal(value: &str) -> String {
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_host_scoped_aggregate_id() {
        let data = serde_json::json!({
            "hostId": "h1",
            "apiVersionId": "v1"
        });
        assert_eq!(
            get_aggregate_id("ApiVersionCreatedEvent", &data).as_deref(),
            Some("h1|v1")
        );
    }

    #[test]
    fn maps_snapshot_table_override() {
        assert_eq!(
            table_to_created_event_type("auth_client_t"),
            "ClientCreatedEvent"
        );
        assert_eq!(
            table_to_created_event_type("pii_token_scheme_t"),
            "PiiTokenSchemeCreatedEvent"
        );
    }
}
