//! Deterministic operators. No free-form codegen. AST real = later.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use repair_types::{
    AgentStatus, Incident, OperatorId, PipelinePhase, PipelineReport, PolicyDecision, RepairAction,
};

#[derive(Debug, Clone)]
pub struct CandidatePatch {
    pub operator: OperatorId,
    pub summary: String,
    pub files: Vec<String>,
    pub steps: Vec<String>,
    pub advisory: bool,
}

pub fn apply(action: &RepairAction, incident: &Incident) -> CandidatePatch {
    match action.repair_operator {
        OperatorId::NoOp | OperatorId::Unknown => CandidatePatch {
            operator: action.repair_operator,
            summary: String::from("needs_human_or_noop"),
            files: Vec::new(),
            steps: alloc::vec![String::from("escalate")],
            advisory: true,
        },
        OperatorId::DependencyRepair => CandidatePatch {
            operator: OperatorId::DependencyRepair,
            summary: alloc::format!("deps {}", incident.project),
            files: alloc::vec![String::from("package.json")],
            steps: alloc::vec![String::from("align missing deps; no major bumps")],
            advisory: false,
        },
        OperatorId::SyntaxFix => CandidatePatch {
            operator: OperatorId::SyntaxFix,
            summary: alloc::format!("syntax node={}", action.node_id),
            files: Vec::new(),
            steps: alloc::vec![String::from("bounded token repair or needs_human")],
            advisory: false,
        },
        OperatorId::ConfigRepair => CandidatePatch {
            operator: OperatorId::ConfigRepair,
            summary: String::from("config"),
            files: alloc::vec![String::from("tsconfig.json"), String::from("vercel.json")],
            steps: alloc::vec![String::from("documented safe defaults only")],
            advisory: false,
        },
        OperatorId::EnvVarRepair | OperatorId::CacheClear => CandidatePatch {
            operator: action.repair_operator,
            summary: String::from("advisory"),
            files: Vec::new(),
            steps: alloc::vec![String::from("document only; no secret writes")],
            advisory: true,
        },
        other => CandidatePatch {
            operator: other,
            summary: String::from(other.as_str()),
            files: Vec::new(),
            steps: alloc::vec![String::from("allowlisted transform only")],
            advisory: false,
        },
    }
}

pub fn gate(action: &RepairAction, min_c: f32, max_r: f32) -> Result<(), PipelineReport> {
    if action.is_actionable(min_c, max_r) {
        return Ok(());
    }
    let status = if matches!(action.repair_operator, OperatorId::NoOp | OperatorId::Unknown) {
        AgentStatus::NeedsHuman
    } else {
        AgentStatus::BlockedByPolicy
    };
    Err(PipelineReport {
        status,
        phase: PipelinePhase::Policy,
        operator_id: Some(action.repair_operator as u8),
        policy_decision: PolicyDecision::DenyWithReason(alloc::format!(
            "c={:.3} r={:.3}",
            action.confidence,
            action.risk
        )),
        verify_result: None,
        message: String::from("policy gate — no LLM codegen path"),
    })
}
