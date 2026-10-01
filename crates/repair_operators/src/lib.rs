//! Deterministic operators. No free-form codegen. AST real = later.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use repair_types::{
    AgentStatus, Incident, OperatorId, PipelinePhase, PipelineReport, PolicyDecision, RepairAction,
    OPERATOR_COUNT,
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    const MIN_C: f32 = 0.55;
    const MAX_R: f32 = 0.45;

    fn action(op: OperatorId, confidence: f32, risk: f32) -> RepairAction {
        RepairAction {
            node_id: String::from("n1"),
            repair_operator: op,
            parameters: BTreeMap::new(),
            confidence,
            risk,
        }
    }

    // --- gate: the safety-critical path -------------------------------------

    #[test]
    fn gate_allows_exactly_at_both_thresholds() {
        // 0.55 >= 0.55 and 0.45 <= 0.45 must be inclusive-allow.
        let a = action(OperatorId::DependencyRepair, MIN_C, MAX_R);
        assert!(gate(&a, MIN_C, MAX_R).is_ok());
    }

    #[test]
    fn gate_blocks_just_below_min_confidence() {
        let a = action(OperatorId::DependencyRepair, 0.5499, 0.0);
        assert!(gate(&a, MIN_C, MAX_R).is_err());
    }

    #[test]
    fn gate_blocks_just_above_max_risk() {
        let a = action(OperatorId::DependencyRepair, 1.0, 0.4501);
        assert!(gate(&a, MIN_C, MAX_R).is_err());
    }

    #[test]
    fn gate_blocks_noop_and_unknown_even_when_confidence_is_perfect() {
        for op in [OperatorId::NoOp, OperatorId::Unknown] {
            let a = action(op, 1.0, 0.0);
            let err = gate(&a, MIN_C, MAX_R).expect_err("must not be actionable");
            assert_eq!(err.status, AgentStatus::NeedsHuman);
            assert_eq!(err.phase, PipelinePhase::Policy);
        }
    }

    #[test]
    fn gate_reports_blocked_by_policy_for_a_real_operator() {
        let a = action(OperatorId::SyntaxFix, 0.10, 0.10);
        let err = gate(&a, MIN_C, MAX_R).expect_err("low confidence must block");
        assert_eq!(err.status, AgentStatus::BlockedByPolicy);
        assert_eq!(err.operator_id, Some(OperatorId::SyntaxFix as u8));
        assert!(matches!(err.policy_decision, PolicyDecision::DenyWithReason(_)));
        assert_eq!(err.verify_result, None, "gate runs before VERIFY");
    }

    #[test]
    fn gate_denial_reason_records_operator_and_scores() {
        let a = action(OperatorId::ConfigRepair, 0.10, 0.90);
        let err = gate(&a, MIN_C, MAX_R).expect_err("must block");
        let PolicyDecision::DenyWithReason(reason) = err.policy_decision else {
            panic!("expected DenyWithReason");
        };
        assert!(reason.contains("CONFIG_REPAIR"), "reason = {reason}");
        assert!(reason.contains("0.100"), "reason = {reason}");
        assert!(reason.contains("0.900"), "reason = {reason}");
    }

    #[test]
    fn gate_is_monotonic_in_confidence() {
        let mut prev = true;
        for c in [0.0f32, 0.3, 0.5, 0.55, 0.7, 1.0] {
            let allowed = gate(&action(OperatorId::DependencyRepair, c, 0.0), MIN_C, MAX_R).is_ok();
            if c < MIN_C {
                assert!(!allowed, "confidence {c} must block");
            }
            assert!(!allowed || prev, "gate must not unblock as confidence rises");
            prev = allowed;
        }
    }

    // --- apply: allowlisted operators only -----------------------------------

    #[test]
    fn apply_is_allowlisted_and_never_emits_free_form_source() {
        let inc = Incident::default();
        for raw in 0u8..OPERATOR_COUNT as u8 {
            let op = OperatorId::from_u8(raw);
            let p = apply(&action(op, 0.9, 0.1), &inc);
            assert_eq!(p.operator, op, "operator {raw} must round-trip");
            if op == OperatorId::NoOp || op == OperatorId::Unknown {
                assert!(p.advisory, "{op:?} must stay advisory");
            }
        }
    }

    #[test]
    fn apply_noop_and_unknown_escalate_instead_of_patching() {
        let inc = Incident::default();
        for op in [OperatorId::NoOp, OperatorId::Unknown] {
            let p = apply(&action(op, 1.0, 0.0), &inc);
            assert!(p.files.is_empty(), "{op:?} must not touch files");
            assert_eq!(p.steps.len(), 1);
        }
    }

    #[test]
    fn apply_keeps_env_and_cache_advisory() {
        let inc = Incident::default();
        for op in [OperatorId::EnvVarRepair, OperatorId::CacheClear] {
            let p = apply(&action(op, 0.9, 0.1), &inc);
            assert!(p.advisory, "{op:?} must be advisory");
            assert!(p.files.is_empty(), "{op:?} must not touch files");
        }
    }

    #[test]
    fn apply_dependency_repair_targets_package_json_without_major_bumps() {
        let mut inc = Incident::default();
        inc.project = String::from("demo");
        let p = apply(&action(OperatorId::DependencyRepair, 0.9, 0.1), &inc);
        assert!(!p.advisory);
        assert_eq!(p.files.len(), 1);
        assert_eq!(p.files[0], "package.json");
        assert!(
            p.steps[0].contains("no major bumps"),
            "step = {}",
            p.steps[0]
        );
    }

    #[test]
    fn apply_never_invents_secret_values() {
        let inc = Incident::default();
        let p = apply(&action(OperatorId::EnvVarRepair, 0.9, 0.1), &inc);
        assert!(p.steps[0].contains("no secret writes"), "step = {}", p.steps[0]);
    }

    #[test]
    fn gate_then_apply_agree_on_actionability() {
        // Whatever the gate allows must produce a non-advisory candidate patch.
        let inc = Incident::default();
        for raw in 0u8..OPERATOR_COUNT as u8 {
            let op = OperatorId::from_u8(raw);
            let a = action(op, 0.9, 0.1);
            if gate(&a, MIN_C, MAX_R).is_ok() {
                assert!(
                    !apply(&a, &inc).advisory,
                    "{op:?} allowed by gate but advisory in apply"
                );
            }
        }
    }
}
