//! GitHub ↔ Cloudflare API Contract Definitions.
//!
//! Provides strict type definitions, versioning, idempotency key calculation,
//! evidence requirements for verification, error matrix policies, and GitHub permissions.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{FailureSignature, VerificationResult};

/// Active contract version supported by this crate.
pub const CONTRACT_VERSION: u16 = 1;

/// Default TTL for idempotency records in seconds (24 hours).
pub const IDEMPOTENCY_TTL_SECONDS: u64 = 86_400;

/// Officially supported GitHub webhook events triggering auto-repair evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubEventType {
    Issues,
    WorkflowRun,
    CheckRun,
    PullRequest,
    RepositoryDispatch,
}

impl GitHubEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Issues => "issues",
            Self::WorkflowRun => "workflow_run",
            Self::CheckRun => "check_run",
            Self::PullRequest => "pull_request",
            Self::RepositoryDispatch => "repository_dispatch",
        }
    }
}

/// Inbound GitHub → Cloudflare Repair Event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairEvent {
    #[serde(default = "default_contract_version")]
    pub contract_version: u16,
    pub event: GitHubEventType,
    pub delivery_id: String,
    pub incident_id: String,
    pub repository: String,
    pub commit: String,
    #[serde(default)]
    pub branch: String,
    pub signature: FailureSignature,
    pub correlation_id: String,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

fn default_contract_version() -> u16 {
    CONTRACT_VERSION
}

fn default_schema_version() -> u32 {
    1
}

impl RepairEvent {
    /// Validates the event against current contract constraints.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.contract_version > CONTRACT_VERSION {
            return Err(ContractError::UnsupportedMajorVersion {
                provided: self.contract_version,
                max_supported: CONTRACT_VERSION,
            });
        }
        if self.delivery_id.trim().is_empty() {
            return Err(ContractError::InvalidPayload {
                reason: String::from("delivery_id cannot be empty"),
            });
        }
        if self.incident_id.trim().is_empty() {
            return Err(ContractError::InvalidPayload {
                reason: String::from("incident_id cannot be empty"),
            });
        }
        if self.repository.trim().is_empty() {
            return Err(ContractError::InvalidPayload {
                reason: String::from("repository cannot be empty"),
            });
        }
        Ok(())
    }

    /// Derives the canonical idempotency key for this event.
    pub fn idempotency_key(&self) -> String {
        compute_idempotency_key(
            &self.repository,
            &self.incident_id,
            &self.delivery_id,
            &self.signature.fingerprint,
        )
    }
}

/// Compute canonical idempotency key using FNV-1a 64-bit hash.
pub fn compute_idempotency_key(
    repository: &str,
    incident_id: &str,
    delivery_id: &str,
    signature_fingerprint: &str,
) -> String {
    let input = format!(
        "{}|{}|{}|{}",
        repository, incident_id, delivery_id, signature_fingerprint
    );
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// Candidate patch proposal outputted by repair engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidatePatch {
    pub operator_id: u8,
    pub parameters: BTreeMap<String, String>,
    pub patch_summary: String,
    pub diff: Option<String>,
    pub has_diff_generator: bool,
}

impl CandidatePatch {
    pub fn is_blocked(&self) -> bool {
        if !self.has_diff_generator {
            return true;
        }
        match &self.diff {
            Some(d) => d.trim().is_empty(),
            None => true,
        }
    }
}

/// Outbound Cloudflare → GitHub Pull Request payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutboundPRRequest {
    pub incident_id: String,
    pub correlation_id: String,
    pub head_branch: String,
    pub base_branch: String,
    pub title: String,
    pub body: String,
    pub patch: CandidatePatch,
    pub labels: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

impl OutboundPRRequest {
    pub fn is_actionable(&self) -> bool {
        !self.patch.is_blocked()
    }
}

/// Verified evidence requirement for CI evaluation.
///
/// GitHub Actions is the single authority for verification.
/// No `VerificationResult::Pass` can be valid without associated GitHub Actions evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedResult {
    pub result: VerificationResult,
    pub workflow_run_id: String,
    pub commit_sha: String,
    pub evidence_ref: String,
}

impl VerifiedResult {
    pub fn new(
        result: VerificationResult,
        workflow_run_id: impl Into<String>,
        commit_sha: impl Into<String>,
        evidence_ref: impl Into<String>,
    ) -> Self {
        Self {
            result,
            workflow_run_id: workflow_run_id.into(),
            commit_sha: commit_sha.into(),
            evidence_ref: evidence_ref.into(),
        }
    }

    /// Enforces that `Pass` requires valid GitHub Actions evidence.
    pub fn is_valid(&self) -> bool {
        match self.result {
            VerificationResult::Pass => {
                !self.workflow_run_id.trim().is_empty()
                    && !self.commit_sha.trim().is_empty()
                    && !self.evidence_ref.trim().is_empty()
                    && (self.evidence_ref.starts_with("evidence://")
                        || self.evidence_ref.starts_with("https://"))
            }
            VerificationResult::Fail
            | VerificationResult::Blocked
            | VerificationResult::Skipped => true,
        }
    }
}

/// Behavior when duplicate delivery or request is encountered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateResponse {
    QueuedOriginal {
        correlation_id: String,
    },
    AlreadyProcessed {
        correlation_id: String,
        pr_url: Option<String>,
    },
    InFlight {
        correlation_id: String,
    },
    IgnoredDuplicate {
        reason: String,
    },
}

/// Behavior when creating PR for an incident that already has an open PR.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DuplicatePRBehavior {
    #[serde(rename = "return_existing_pr")]
    ReturnExistingPR,
    #[serde(rename = "update_branch")]
    UpdateBranch,
    #[serde(rename = "skip_duplicate")]
    SkipDuplicate,
}

/// Contract Error Matrix categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractErrorCode {
    Http401Unauthorized,
    Http403Forbidden,
    Http404NotFound,
    Http409Duplicate,
    Http429RateLimited,
    Http5xxServerError,
    InvalidPayload,
    VerifyMissing,
    UnsafePatch,
}

/// Operational policy for a given error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorPolicy {
    pub retryable: bool,
    pub send_to_dlq: bool,
    pub requires_human: bool,
}

/// Returns the strict policy for a given contract error code.
pub fn get_error_policy(code: ContractErrorCode) -> ErrorPolicy {
    match code {
        ContractErrorCode::Http401Unauthorized => ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true,
        },
        ContractErrorCode::Http403Forbidden => ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true,
        },
        ContractErrorCode::Http404NotFound => ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true,
        },
        ContractErrorCode::Http409Duplicate => ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: false,
        },
        ContractErrorCode::Http429RateLimited => ErrorPolicy {
            retryable: true,
            send_to_dlq: true,
            requires_human: false,
        },
        ContractErrorCode::Http5xxServerError => ErrorPolicy {
            retryable: true,
            send_to_dlq: true,
            requires_human: false,
        },
        ContractErrorCode::InvalidPayload => ErrorPolicy {
            retryable: false,
            send_to_dlq: true,
            requires_human: true,
        },
        ContractErrorCode::VerifyMissing => ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true,
        },
        ContractErrorCode::UnsafePatch => ErrorPolicy {
            retryable: false,
            send_to_dlq: false,
            requires_human: true,
        },
    }
}

/// Granular GitHub Permissions (Least Privilege).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubPermission {
    ContentsWrite,
    PullRequestsWrite,
    ActionsRead,
    ChecksRead,
    MetadataRead,
}

/// Minimum permission specification required for the GitHub App / Fine-Grained PAT.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MinimumPermissions {
    pub permissions: Vec<GitHubPermission>,
}

impl MinimumPermissions {
    pub fn default_required() -> Self {
        Self {
            permissions: alloc::vec![
                GitHubPermission::ContentsWrite,
                GitHubPermission::PullRequestsWrite,
                GitHubPermission::ActionsRead,
                GitHubPermission::ChecksRead,
                GitHubPermission::MetadataRead,
            ],
        }
    }
}

/// Contract Errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractError {
    UnsupportedMajorVersion { provided: u16, max_supported: u16 },
    MissingEvidence { message: String },
    MissingDiffGenerator { reason: String },
    InvalidPayload { reason: String },
}

impl core::fmt::Display for ContractError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedMajorVersion {
                provided,
                max_supported,
            } => {
                write!(
                    f,
                    "Unsupported contract version {} (max supported: {})",
                    provided, max_supported
                )
            }
            Self::MissingEvidence { message } => write!(f, "Missing verify evidence: {}", message),
            Self::MissingDiffGenerator { reason } => {
                write!(f, "Missing diff generator: {}", reason)
            }
            Self::InvalidPayload { reason } => write!(f, "Invalid payload: {}", reason),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ContractError {}
