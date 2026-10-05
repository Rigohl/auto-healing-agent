//! Deterministic operators. No free-form codegen. AST real = later.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod diff;

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

/// Deterministic operators.
///
/// One explicit arm per `OperatorId`: adding a variant to the enum breaks the
/// build here instead of silently falling into a catch-all. That is the
/// allowlist — there is no free-form codegen path and no default branch.
pub fn apply(action: &RepairAction, incident: &Incident) -> CandidatePatch {
    match action.repair_operator {
        OperatorId::NoOp => escalate(action.repair_operator, "no actionable repair"),
        OperatorId::Unknown => escalate(action.repair_operator, "unknown operator"),

        OperatorId::DependencyRepair => CandidatePatch {
            operator: OperatorId::DependencyRepair,
            summary: alloc::format!("deps project={}", incident.project),
            files: alloc::vec![String::from("package.json")],
            steps: alloc::vec![String::from(
                "align missing deps from error; no major bumps"
            )],
            advisory: false,
        },
        OperatorId::SyntaxFix => CandidatePatch {
            operator: OperatorId::SyntaxFix,
            summary: alloc::format!("syntax node={}", action.node_id),
            files: Vec::new(),
            steps: alloc::vec![String::from(
                "localize parse error; bounded token repair or needs_human",
            )],
            advisory: false,
        },
        OperatorId::ConfigRepair => CandidatePatch {
            operator: OperatorId::ConfigRepair,
            summary: String::from("config repair"),
            files: alloc::vec![String::from("tsconfig.json"), String::from("vercel.json")],
            steps: alloc::vec![String::from("safe documented defaults only")],
            advisory: false,
        },
        OperatorId::BuildScriptFix => CandidatePatch {
            operator: OperatorId::BuildScriptFix,
            summary: alloc::format!("build script {}", incident.command),
            files: alloc::vec![String::from("package.json")],
            steps: alloc::vec![String::from("fix misnamed scripts only")],
            advisory: false,
        },
        OperatorId::LockfileRefresh => CandidatePatch {
            operator: OperatorId::LockfileRefresh,
            summary: String::from("lockfile refresh"),
            files: alloc::vec![String::from("package-lock.json")],
            steps: alloc::vec![String::from("regenerate within existing ranges")],
            advisory: false,
        },
        OperatorId::VersionPin => CandidatePatch {
            operator: OperatorId::VersionPin,
            summary: String::from("version pin"),
            files: alloc::vec![String::from("package.json")],
            steps: alloc::vec![String::from("pin to a known-good resolved version")],
            advisory: false,
        },
        OperatorId::ImportPathFix => CandidatePatch {
            operator: OperatorId::ImportPathFix,
            summary: String::from("import path"),
            files: Vec::new(),
            steps: alloc::vec![String::from("rewrite import specifier to an existing path")],
            advisory: false,
        },
        OperatorId::TypeAnnotationFix => CandidatePatch {
            operator: OperatorId::TypeAnnotationFix,
            summary: String::from("type annotation"),
            files: Vec::new(),
            steps: alloc::vec![String::from("add or widen a type annotation only")],
            advisory: false,
        },
        OperatorId::TestRepair => CandidatePatch {
            operator: OperatorId::TestRepair,
            summary: String::from("test repair"),
            files: Vec::new(),
            steps: alloc::vec![String::from(
                "fix test fixture or assertion, never the intent"
            )],
            advisory: false,
        },
        OperatorId::SourceRepair => CandidatePatch {
            operator: OperatorId::SourceRepair,
            summary: String::from("source repair"),
            files: Vec::new(),
            steps: alloc::vec![String::from(
                "bounded deterministic edit inside one allowlisted node",
            )],
            advisory: false,
        },

        // Advisory by policy: these never write. NO_LLM_POLICY.md defaults table
        // lists secret/env operators as advisory/block, and cache clear has no
        // repo-side artifact to touch.
        OperatorId::EnvVarRepair => escalate(
            OperatorId::EnvVarRepair,
            "env advisory — never invent secrets",
        ),
        OperatorId::CacheClear => escalate(
            OperatorId::CacheClear,
            "cache advisory — no repo-side change",
        ),
    }
}

/// No file may be touched and the step escalates instead of patching.
fn escalate(op: OperatorId, msg: &str) -> CandidatePatch {
    CandidatePatch {
        operator: op,
        summary: String::from(msg),
        files: Vec::new(),
        steps: alloc::vec![String::from("escalate_or_document")],
        advisory: true,
    }
}

pub fn gate(action: &RepairAction, min_c: f32, max_r: f32) -> Result<(), PipelineReport> {
    if action.is_actionable(min_c, max_r) {
        return Ok(());
    }
    let status = if matches!(
        action.repair_operator,
        OperatorId::NoOp | OperatorId::Unknown
    ) {
        AgentStatus::NeedsHuman
    } else {
        AgentStatus::BlockedByPolicy
    };
    Err(PipelineReport {
        status,
        phase: PipelinePhase::Policy,
        operator_id: Some(action.repair_operator as u8),
        policy_decision: PolicyDecision::DenyWithReason(alloc::format!(
            "c={:.3} r={:.3} op={}",
            action.confidence,
            action.risk,
            action.repair_operator.as_str()
        )),
        verify_result: None,
        message: String::from("policy gate — no LLM codegen path"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;
    use repair_types::OPERATOR_COUNT;

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
        assert!(matches!(
            err.policy_decision,
            PolicyDecision::DenyWithReason(_)
        ));
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
        // Once the gate allows an action, raising confidence must never block it.
        let mut prev = false;
        for c in [0.0f32, 0.3, 0.5, 0.5499, 0.55, 0.7, 1.0] {
            let allowed = gate(&action(OperatorId::DependencyRepair, c, 0.0), MIN_C, MAX_R).is_ok();
            if prev {
                assert!(allowed, "gate blocked after allowing at confidence {c}");
            }
            prev = allowed;
        }
        assert!(prev, "confidence 1.0 must be allowed");
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
        let inc = Incident {
            project: String::from("demo"),
            ..Default::default()
        };
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
        assert!(p.advisory);
        assert!(p.files.is_empty());
        assert!(
            p.summary.contains("never invent secrets"),
            "summary = {}",
            p.summary
        );
        assert_eq!(p.steps[0], "escalate_or_document");
    }

    #[test]
    fn gate_allows_env_operator_that_apply_keeps_advisory() {
        // Documented intent, not a contradiction: env/secret and cache operators
        // clear governance but must never mutate anything. The gate authorises
        // the decision to be considered; apply() still refuses to write.
        let inc = Incident::default();
        for op in [OperatorId::EnvVarRepair, OperatorId::CacheClear] {
            let a = action(op, 0.9, 0.1);
            assert!(
                gate(&a, MIN_C, MAX_R).is_ok(),
                "{op:?} should clear the gate"
            );
            let p = apply(&a, &inc);
            assert!(p.advisory, "{op:?} must remain advisory after the gate");
            assert!(p.files.is_empty(), "{op:?} must not touch files");
        }
    }

    #[test]
    fn every_advisory_operator_is_blocked_or_side_effect_free() {
        // Safety property over the whole allowlist: whenever apply() reports
        // advisory, it must not list any file to modify.
        let inc = Incident::default();
        for raw in 0u8..OPERATOR_COUNT as u8 {
            let op = OperatorId::from_u8(raw);
            let p = apply(&action(op, 0.9, 0.1), &inc);
            if p.advisory {
                assert!(
                    p.files.is_empty(),
                    "{op:?} is advisory but lists files: {:?}",
                    p.files
                );
            }
        }
    }
}
