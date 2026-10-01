use repair_types::contract::*;
use repair_types::{FailureSignature, VerificationResult};
use std::collections::BTreeMap;

fn sample_event() -> RepairEvent {
    RepairEvent {
        contract_version: CONTRACT_VERSION,
        event: GitHubEventType::WorkflowRun,
        delivery_id: String::from("deliv-12345"),
        incident_id: String::from("inc-998877"),
        repository: String::from("Rigohl/auto-healing-agent"),
        commit: String::from("8f3a9b2c1d0e4f5a6b7c8d9e0f1a2b3c4d5e6f7a"),
        branch: String::from("main"),
        signature: FailureSignature {
            error_code: String::from("ERR_BUILD_FAILED"),
            error_step: String::from("cargo check"),
            command_family: String::from("cargo"),
            language: String::from("rust"),
            framework: String::from("none"),
            fingerprint: String::from("err_build_failed|cargo check|cargo|rust|none"),
        },
        correlation_id: String::from("inc-998877-a1b2c3d4"),
        timestamp: 1700000000000,
        schema_version: 1,
    }
}

#[test]
fn round_trip() {
    let original = sample_event();
    let json = serde_json::to_string(&original).expect("serialize failed");
    let deserialized: RepairEvent = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(original, deserialized);
    assert!(deserialized.validate().is_ok());
}

#[test]
fn partial_payload() {
    let raw_json = r#"{
        "contract_version": 1,
        "event": "workflow_run",
        "delivery_id": "deliv-partial",
        "incident_id": "inc-partial",
        "repository": "owner/repo",
        "commit": "abc1234",
        "signature": {
            "error_code": "ERR_1",
            "error_step": "test",
            "command_family": "npm",
            "language": "js",
            "framework": "node",
            "fingerprint": "err_1|test|npm|js|node"
        },
        "correlation_id": "corr-123"
    }"#;

    let event: RepairEvent = serde_json::from_str(raw_json).expect("partial payload parse");
    assert_eq!(event.contract_version, 1);
    assert_eq!(event.branch, "");
    assert_eq!(event.timestamp, 0);
    assert_eq!(event.schema_version, 1);
    assert!(event.validate().is_ok());
}

#[test]
fn unknown_fields() {
    let raw_json = r#"{
        "contract_version": 1,
        "event": "check_run",
        "delivery_id": "deliv-unknown",
        "incident_id": "inc-unknown",
        "repository": "owner/repo",
        "commit": "def5678",
        "branch": "feat/new",
        "signature": {
            "error_code": "ERR_2",
            "error_step": "build",
            "command_family": "other",
            "language": "rust",
            "framework": "actix",
            "fingerprint": "err_2|build|other|rust|actix"
        },
        "correlation_id": "corr-456",
        "timestamp": 1700000001000,
        "schema_version": 1,
        "future_flag": true,
        "extra_object": { "key": "value" }
    }"#;

    let event: RepairEvent = serde_json::from_str(raw_json).expect("unknown fields ignored");
    assert_eq!(event.event, GitHubEventType::CheckRun);
    assert_eq!(event.delivery_id, "deliv-unknown");
}

#[test]
fn unsupported_major_version() {
    let mut event = sample_event();
    event.contract_version = 99;

    let err = event
        .validate()
        .expect_err("should reject major version > 1");
    assert_eq!(
        err,
        ContractError::UnsupportedMajorVersion {
            provided: 99,
            max_supported: 1
        }
    );
}

#[test]
fn idempotency() {
    let event = sample_event();
    let key1 = event.idempotency_key();
    let key2 = compute_idempotency_key(
        "Rigohl/auto-healing-agent",
        "inc-998877",
        "deliv-12345",
        "err_build_failed|cargo check|cargo|rust|none",
    );

    assert_eq!(key1, key2);
    assert_eq!(key1.len(), 16);

    let key3 = compute_idempotency_key(
        "Rigohl/auto-healing-agent",
        "inc-998877",
        "deliv-67890",
        "err_build_failed|cargo check|cargo|rust|none",
    );
    assert_ne!(key1, key3);
}

#[test]
fn duplicate_delivery() {
    let resp = DuplicateResponse::AlreadyProcessed {
        correlation_id: String::from("corr-789"),
        pr_url: Some(String::from("https://github.com/owner/repo/pull/42")),
    };
    let json = serde_json::to_string(&resp).expect("serialize duplicate response");
    assert!(json.contains("already_processed"));
    assert!(json.contains("corr-789"));

    let behavior = DuplicatePRBehavior::ReturnExistingPR;
    let b_json = serde_json::to_string(&behavior).expect("serialize behavior");
    assert_eq!(b_json, "\"return_existing_pr\"");
}

#[test]
fn verify_requires_evidence() {
    let pass_no_evidence = VerifiedResult::new(VerificationResult::Pass, "", "", "");
    assert!(!pass_no_evidence.is_valid());

    let pass_partial = VerifiedResult::new(VerificationResult::Pass, "run-1", "sha123", "");
    assert!(!pass_partial.is_valid());

    let fail_no_evidence = VerifiedResult::new(VerificationResult::Fail, "", "", "");
    assert!(fail_no_evidence.is_valid());
}

#[test]
fn pass_requires_actions_evidence() {
    let result = VerifiedResult::new(
        VerificationResult::Pass,
        "workflow-123",
        "abc123",
        "evidence://workflow-123",
    );
    assert!(result.is_valid());

    let result_https = VerifiedResult::new(
        VerificationResult::Pass,
        "workflow-123",
        "abc123",
        "https://github.com/owner/repo/actions/runs/123",
    );
    assert!(result_https.is_valid());
}

#[test]
fn failure_mapping() {
    assert_eq!(
        get_error_policy(ContractErrorCode::Http401Unauthorized),
        ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true
        }
    );
    assert_eq!(
        get_error_policy(ContractErrorCode::Http403Forbidden),
        ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true
        }
    );
    assert_eq!(
        get_error_policy(ContractErrorCode::Http404NotFound),
        ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true
        }
    );
    assert_eq!(
        get_error_policy(ContractErrorCode::Http409Duplicate),
        ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: false
        }
    );
    assert_eq!(
        get_error_policy(ContractErrorCode::Http429RateLimited),
        ErrorPolicy {
            retryable: true,
            send_to_dlq: true,
            requires_human: false
        }
    );
    assert_eq!(
        get_error_policy(ContractErrorCode::Http5xxServerError),
        ErrorPolicy {
            retryable: true,
            send_to_dlq: true,
            requires_human: false
        }
    );
    assert_eq!(
        get_error_policy(ContractErrorCode::InvalidPayload),
        ErrorPolicy {
            retryable: false,
            send_to_dlq: true,
            requires_human: true
        }
    );
    assert_eq!(
        get_error_policy(ContractErrorCode::VerifyMissing),
        ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true
        }
    );
    assert_eq!(
        get_error_policy(ContractErrorCode::UnsafePatch),
        ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true
        }
    );
}

#[test]
fn serialization() {
    let events = vec![
        (GitHubEventType::Issues, "\"issues\""),
        (GitHubEventType::WorkflowRun, "\"workflow_run\""),
        (GitHubEventType::CheckRun, "\"check_run\""),
        (GitHubEventType::PullRequest, "\"pull_request\""),
        (
            GitHubEventType::RepositoryDispatch,
            "\"repository_dispatch\"",
        ),
    ];

    for (event, expected_json) in events {
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, expected_json);
    }

    let patch_blocked = CandidatePatch {
        operator_id: 1,
        parameters: BTreeMap::new(),
        patch_summary: String::from("bump package.json"),
        diff: None,
        has_diff_generator: false,
    };
    assert!(patch_blocked.is_blocked());

    let patch_no_diff = CandidatePatch {
        operator_id: 1,
        parameters: BTreeMap::new(),
        patch_summary: String::from("bump package.json"),
        diff: Some(String::from("")),
        has_diff_generator: true,
    };
    assert!(patch_no_diff.is_blocked());

    let patch_valid = CandidatePatch {
        operator_id: 1,
        parameters: BTreeMap::new(),
        patch_summary: String::from("bump package.json"),
        diff: Some(String::from("--- a/package.json\n+++ b/package.json\n")),
        has_diff_generator: true,
    };
    assert!(!patch_valid.is_blocked());

    let min_perms = MinimumPermissions::default_required();
    assert_eq!(min_perms.permissions.len(), 5);
}
