use std::collections::HashMap;
use servai::domain::engines::{
    ansible::{AnsibleFacts, AnsibleModuleResult, AnsibleRunResult, AnsibleTaskEvent},
    crossplane::{
        CrossplaneClaim, CrossplaneCondition, CrossplaneReconcileResult, CrossplaneResource,
    },
    opentofu::{TofuApplyResult, TofuPlanResult, TofuPlanSummary, TofuState},
};

#[test]
fn test_opentofu_plan_summary_and_validation() {
    let summary = TofuPlanSummary::new(3, 1, 0);
    assert_eq!(summary.to_add, 3);
    assert_eq!(summary.to_change, 1);
    assert_eq!(summary.to_destroy, 0);
    assert!(summary.has_changes());

    let clean_summary = TofuPlanSummary::new(0, 0, 0);
    assert!(!clean_summary.has_changes());

    let plan = TofuPlanResult::new(
        summary,
        vec![
            "hcloud_server.web_node[0]".to_string(),
            "hcloud_server.web_node[1]".to_string(),
            "hcloud_server.web_node[2]".to_string(),
            "hcloud_firewall.default_fw".to_string(),
        ],
    );

    assert_eq!(plan.resources().len(), 4);
    assert!(!plan.is_clean());
}

#[test]
fn test_opentofu_apply_result_and_state() {
    let mut outputs = HashMap::new();
    outputs.insert("server_ip".to_string(), "192.168.1.100".to_string());
    outputs.insert("ssh_port".to_string(), "22".to_string());

    let apply = TofuApplyResult::new(3, 1, 0, outputs.clone(), true);
    assert!(apply.is_success());
    assert_eq!(apply.outputs().get("server_ip").unwrap(), "192.168.1.100");

    let state = TofuState::new(
        1,
        "uuid-lineage-123".to_string(),
        vec!["hcloud_server.web_node".to_string()],
        outputs,
    );
    assert_eq!(state.serial(), 1);
    assert_eq!(state.lineage(), "uuid-lineage-123");
    assert_eq!(state.resources().len(), 1);
}

#[test]
fn test_ansible_run_result_and_task_events() {
    let events = vec![
        AnsibleTaskEvent::new("Gathering Facts", "10.0.0.1", "ok", None),
        AnsibleTaskEvent::new("Install Nginx", "10.0.0.1", "changed", Some("nginx installed".to_string())),
        AnsibleTaskEvent::new("Start Service", "10.0.0.1", "ok", None),
    ];

    let result = AnsibleRunResult::new(2, 1, 0, 0, events);
    assert!(result.is_successful());
    assert_eq!(result.ok_count(), 2);
    assert_eq!(result.changed_count(), 1);
    assert_eq!(result.failed_count(), 0);

    let failed_events = vec![
        AnsibleTaskEvent::new("Gathering Facts", "10.0.0.2", "unreachable", Some("SSH connection refused".to_string())),
    ];
    let failed_result = AnsibleRunResult::new(0, 0, 0, 1, failed_events);
    assert!(!failed_result.is_successful());
    assert!(failed_result.has_unreachable());
}

#[test]
fn test_ansible_facts_parsing_and_module_execution() {
    let facts = AnsibleFacts::new(
        "Ubuntu".to_string(),
        "24.04".to_string(),
        "6.8.0-generic".to_string(),
        "3.12.3".to_string(),
        vec!["eth0".to_string(), "docker0".to_string()],
        16384,
    );

    assert_eq!(facts.distribution, "Ubuntu");
    assert_eq!(facts.distribution_version, "24.04");
    assert_eq!(facts.memtotal_mb, 16384);
    assert!(facts.interfaces.contains(&"docker0".to_string()));

    let module_res = AnsibleModuleResult::new("service", true, "nginx reloaded", 0);
    assert!(module_res.is_success());
    assert!(module_res.changed);
}

#[test]
fn test_crossplane_claim_and_condition_validation() {
    let mut params = HashMap::new();
    params.insert("nodePoolSize".to_string(), "3".to_string());
    params.insert("region".to_string(), "eu-west-1".to_string());

    let claim = CrossplaneClaim::new(
        "database.servai.io/v1alpha1",
        "PostgreSQLInstance",
        "production-db",
        "default",
        Some("postgres-ha-composition".to_string()),
        params,
    )
    .unwrap();

    assert_eq!(claim.api_version(), "database.servai.io/v1alpha1");
    assert_eq!(claim.kind(), "PostgreSQLInstance");
    assert_eq!(claim.name(), "production-db");
    assert_eq!(claim.namespace(), "default");
    assert_eq!(claim.composition_ref(), Some("postgres-ha-composition"));

    // Validation edge case: empty name
    let invalid = CrossplaneClaim::new(
        "database.servai.io/v1alpha1",
        "PostgreSQLInstance",
        "",
        "default",
        None,
        HashMap::new(),
    );
    assert!(invalid.is_err());
}

#[test]
fn test_crossplane_reconcile_result_and_managed_resources() {
    let conditions = vec![
        CrossplaneCondition::new("Synced", "True", "ReconcileSuccess", "Resource synced successfully"),
        CrossplaneCondition::new("Ready", "True", "Available", "Composite resource is ready"),
    ];

    let managed = vec![
        CrossplaneResource::new("Instance", "postgres-m6i", "default", true, true, "provider-aws"),
        CrossplaneResource::new("SubnetGroup", "postgres-subnets", "default", true, true, "provider-aws"),
    ];

    let result = CrossplaneReconcileResult::new(
        "production-db",
        "PostgreSQLInstance",
        true,
        true,
        conditions,
        managed,
    );

    assert!(result.is_ready());
    assert!(result.is_synced());
    assert_eq!(result.managed_resources().len(), 2);
    assert_eq!(result.managed_resources()[0].provider, "provider-aws");

    let degraded = CrossplaneReconcileResult::new(
        "broken-cluster",
        "KubernetesCluster",
        false,
        true,
        vec![CrossplaneCondition::new("Ready", "False", "CreatingNodePool", "Nodes provisioning")],
        vec![],
    );
    assert!(!degraded.is_ready());
    assert!(degraded.is_synced());
}

