//! Core contracts — CODEMAP / Prompt Pad V0.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod contract;

pub use contract::{
    compute_idempotency_key, get_error_policy, CandidatePatch, ContractError, ContractErrorCode,
    DuplicatePRBehavior, DuplicateResponse, ErrorPolicy, GitHubEventType, GitHubPermission,
    MinimumPermissions, OutboundPRRequest, RepairEvent, VerifiedResult, CONTRACT_VERSION,
    IDEMPOTENCY_TTL_SECONDS,
};

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum OperatorId {
    NoOp = 0,
    DependencyRepair = 1,
    SyntaxFix = 2,
    ConfigRepair = 3,
    BuildScriptFix = 4,
    EnvVarRepair = 5,
    LockfileRefresh = 6,
    TypeAnnotationFix = 7,
    ImportPathFix = 8,
    VersionPin = 9,
    CacheClear = 10,
    TestRepair = 11,
    SourceRepair = 12,
    Unknown = 255,
}

pub const OPERATOR_COUNT: usize = 13;

impl OperatorId {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::NoOp,
            1 => Self::DependencyRepair,
            2 => Self::SyntaxFix,
            3 => Self::ConfigRepair,
            4 => Self::BuildScriptFix,
            5 => Self::EnvVarRepair,
            6 => Self::LockfileRefresh,
            7 => Self::TypeAnnotationFix,
            8 => Self::ImportPathFix,
            9 => Self::VersionPin,
            10 => Self::CacheClear,
            11 => Self::TestRepair,
            12 => Self::SourceRepair,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoOp => "NO_OP",
            Self::DependencyRepair => "DEPENDENCY_REPAIR",
            Self::SyntaxFix => "SYNTAX_FIX",
            Self::ConfigRepair => "CONFIG_REPAIR",
            Self::BuildScriptFix => "BUILD_SCRIPT_FIX",
            Self::EnvVarRepair => "ENV_VAR_REPAIR",
            Self::LockfileRefresh => "LOCKFILE_REFRESH",
            Self::TypeAnnotationFix => "TYPE_ANNOTATION_FIX",
            Self::ImportPathFix => "IMPORT_PATH_FIX",
            Self::VersionPin => "VERSION_PIN",
            Self::CacheClear => "CACHE_CLEAR",
            Self::TestRepair => "TEST_REPAIR",
            Self::SourceRepair => "SOURCE_REPAIR",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub source: String,
    pub error_code: String,
    pub error_step: String,
    pub command: String,
    pub message: String,
    pub project: String,
    pub attempts: u32,
    pub stack_hint: String,
    pub language_hint: String,
    pub framework_hint: String,
    pub verified: bool,
    pub status: String,
}

impl Default for Incident {
    fn default() -> Self {
        Self {
            id: String::new(),
            source: String::from("unknown"),
            error_code: String::new(),
            error_step: String::new(),
            command: String::new(),
            message: String::new(),
            project: String::new(),
            attempts: 0,
            stack_hint: String::new(),
            language_hint: String::new(),
            framework_hint: String::new(),
            verified: false,
            status: String::from("open"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FailureSignature {
    pub error_code: String,
    pub error_step: String,
    pub command_family: String,
    pub language: String,
    pub framework: String,
    pub fingerprint: String,
}

impl FailureSignature {
    pub fn from_incident(inc: &Incident) -> Self {
        let family = command_family(&inc.command);
        let fingerprint = alloc::format!(
            "{}|{}|{}|{}|{}",
            inc.error_code.to_lowercase(),
            inc.error_step.to_lowercase(),
            family,
            inc.language_hint.to_lowercase(),
            inc.framework_hint.to_lowercase()
        );
        Self {
            error_code: inc.error_code.clone(),
            error_step: inc.error_step.clone(),
            command_family: family,
            language: inc.language_hint.clone(),
            framework: inc.framework_hint.clone(),
            fingerprint,
        }
    }
}

fn command_family(cmd: &str) -> String {
    let c = cmd.to_lowercase();
    if c.contains("vercel-build") || c.contains("next build") {
        String::from("vercel_build")
    } else if c.contains("npm") || c.contains("pnpm") || c.contains("yarn") {
        String::from("node_package")
    } else if c.contains("cargo") {
        String::from("cargo")
    } else {
        String::from("other")
    }
}

/// Structured action — never free-form source text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairAction {
    pub node_id: String,
    pub repair_operator: OperatorId,
    pub parameters: BTreeMap<String, String>,
    pub confidence: f32,
    pub risk: f32,
}

impl RepairAction {
    pub fn is_actionable(&self, min_confidence: f32, max_risk: f32) -> bool {
        self.confidence >= min_confidence
            && self.risk <= max_risk
            && !matches!(self.repair_operator, OperatorId::NoOp | OperatorId::Unknown)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationResult {
    Pass,
    Fail,
    Blocked,
    Skipped,
}

/// Caso de reparacion persistible: incidente + firma + accion +
/// verificacion + parche + reward. CONTRATO PART4: este struct es el
/// schema de la persistencia (diseño original: Mongo Atlas PYH-32; V1
/// implementada como KV `REPAIR_CASES_KV`, ver
/// `worker/src/worker/queue_consumer.rs` `persist_case`) y del ciclo de
/// aprendizaje. Consumidor real desde el 2026-10-05: el consumidor de
/// cola persiste el RepairCase tras abrir el PR. Hasta esa fecha fue
/// contrato versionado sin consumidores (DISCREPANCIES item 74).
/// No eliminar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairCase {
    pub incident_id: String,
    pub signature: FailureSignature,
    pub action: RepairAction,
    pub verification: VerificationResult,
    pub patch_summary: String,
    pub pr_url: Option<String>,
    pub reward: f32,
    pub created_at_unix: u64,
}

/// Ejemplo de entrenamiento derivado de un caso real (features,
/// operador, nodo, reward): fila de la coleccion `training_examples`
/// en Mongo (PYH-32). CONTRATO PART4: `repair_train` y la promocion a
/// MODEL_KV lo consumiran. Sin consumidores actuales por diseno
/// (auditoria 2026-10-03, DISCREPANCIES item 74). No eliminar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingExample {
    pub features: Vec<f32>,
    pub operator: u8,
    pub node_id: String,
    pub reward: f32,
    pub verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Success,
    BlockedByPolicy,
    NeedsHuman,
    RolledBack,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelinePhase {
    Detect,
    Evidence,
    FailureSignature,
    Diagnose,
    NeuralPropose,
    Policy,
    Repair,
    PatchValidation,
    Ci,
    Verify,
    PushAuthorization,
    Audit,
    Postmortem,
    Blocked,
    NeedsHuman,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    Deny,
    DenyWithReason(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineReport {
    pub status: AgentStatus,
    pub phase: PipelinePhase,
    pub operator_id: Option<u8>,
    pub policy_decision: PolicyDecision,
    pub verify_result: Option<VerificationResult>,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub struct FeatureVector {
    pub values: [f32; 64],
}

impl FeatureVector {
    pub const DIM: usize = 64;
    pub const SCHEMA_VERSION: u32 = 2;

    pub fn zeros() -> Self {
        Self { values: [0.0; 64] }
    }

    pub fn as_slice(&self) -> &[f32; 64] {
        &self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_ids_are_dense_zero_to_twelve() {
        for v in 0..OPERATOR_COUNT as u8 {
            assert_ne!(OperatorId::from_u8(v), OperatorId::Unknown);
        }
        assert_eq!(OperatorId::from_u8(13), OperatorId::Unknown);
        assert_eq!(OperatorId::from_u8(255), OperatorId::Unknown);
    }

    #[test]
    fn feature_vector_dim_is_64() {
        assert_eq!(FeatureVector::DIM, 64);
        assert_eq!(FeatureVector::zeros().as_slice().len(), 64);
    }

    #[test]
    fn gate_threshold_is_confidence_and_risk() {
        let mut a = RepairAction {
            node_id: String::from("n1"),
            repair_operator: OperatorId::DependencyRepair,
            parameters: BTreeMap::new(),
            confidence: 0.54,
            risk: 0.10,
        };
        // 0.54 < MIN_CONFIDENCE 0.55 -> not actionable
        assert!(!a.is_actionable(0.55, 0.45));
        a.confidence = 0.55;
        assert!(a.is_actionable(0.55, 0.45));
        a.risk = 0.46;
        assert!(!a.is_actionable(0.55, 0.45));
    }

    #[test]
    fn noop_is_never_actionable_even_with_high_confidence() {
        let a = RepairAction {
            node_id: String::from("n1"),
            repair_operator: OperatorId::NoOp,
            parameters: BTreeMap::new(),
            confidence: 1.0,
            risk: 0.0,
        };
        assert!(!a.is_actionable(0.0, 1.0));
    }

    #[test]
    fn pipeline_phase_covers_incident_to_audit_span() {
        // Terminal reporting phases required by the SYSTEM PROMPT output contract.
        assert_ne!(PipelinePhase::Verify, PipelinePhase::Ci);
        assert_ne!(PipelinePhase::PatchValidation, PipelinePhase::Verify);
        assert_ne!(PipelinePhase::PushAuthorization, PipelinePhase::Verify);
        assert_ne!(PipelinePhase::Postmortem, PipelinePhase::Audit);
        assert_ne!(PipelinePhase::NeedsHuman, PipelinePhase::Blocked);
        assert_ne!(PipelinePhase::FailureSignature, PipelinePhase::Evidence);
    }

    #[test]
    fn pipeline_report_carries_optional_verify_result() {
        let r = PipelineReport {
            status: AgentStatus::Success,
            phase: PipelinePhase::Verify,
            operator_id: Some(1),
            policy_decision: PolicyDecision::Allow,
            verify_result: Some(VerificationResult::Pass),
            message: String::from("ok"),
        };
        assert_eq!(r.verify_result, Some(VerificationResult::Pass));
    }
}
