//! SIGNATURE LEDGER: memoria determinista de "que ya funciono" (adopcion
//! del namespace huerfano STATE, decision del dueno 2026-10-08).
//!
//! Tras un PASS real de GitHub Actions, el callback registra firma ->
//! accion verificada. El consumidor reusa (replay) esa accion en vez de
//! re-inferir: misma firma, mismo edit conocido-bueno. La senal SIEMPRE es
//! de Actions (autoridad VERIFY); este modulo nunca inventa verificaciones.
//!
//! Reglas:
//! - Best-effort total: sin KV, KV caido o entrada malformada => None y el
//!   consumidor sigue por el camino normal (NN). El replay es una mejora,
//!   no una dependencia.
//! - Fail-closed de contenido: una entrada con operador no accionable o
//!   confidence/risk no finitos no es reutilizable.
//! - Clave por (repo, fingerprint): un edit verificado en un repo no se
//!   traslada a otro (el parametro `file` puede diferir).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use worker::Env;

use repair_types::RepairAction;

use crate::runtime::security::fnv1a64;

/// Binding del KV del ledger (wrangler.toml: namespace STATE).
pub const KV_BINDING: &str = "STATE";
pub const PREFIX: &str = "ledger";

/// Accion verificada PASS por Actions, lista para replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub operator: u8,
    pub parameters: BTreeMap<String, String>,
    pub confidence: f32,
    pub risk: f32,
    pub pr_url: String,
    pub correlation_id: String,
    pub updated_at_unix: u64,
}

impl LedgerEntry {
    /// Reconstruye la RepairAction para el replay. node_id marcado para
    /// que la auditoria distinga replay de prediccion NN.
    pub fn to_action(&self) -> RepairAction {
        RepairAction {
            node_id: String::from("ledger:replay"),
            repair_operator: repair_types::OperatorId::from_u8(self.operator),
            parameters: self.parameters.clone(),
            confidence: self.confidence,
            risk: self.risk,
        }
    }

    /// Entrada utilizable: operador accionable y valores finitos. Una
    /// entrada corrupta jamas produce un replay.
    pub fn is_replayable(&self) -> bool {
        self.confidence.is_finite()
            && self.risk.is_finite()
            && !matches!(
                repair_types::OperatorId::from_u8(self.operator),
                repair_types::OperatorId::NoOp | repair_types::OperatorId::Unknown
            )
    }
}

/// Clave determinista por (repo, fingerprint) con FNV-1a (sin caracteres
/// problematicos para KV).
pub fn key(repo: &str, fingerprint: &str) -> String {
    format!(
        "{}:{:x}",
        PREFIX,
        fnv1a64(format!("{}|{}", repo, fingerprint).as_bytes())
    )
}

/// Busca un replay verificado. Best-effort: sin binding, sin clave o
/// entrada malformada => None (camino normal por la NN).
pub async fn lookup(env: &Env, repo: &str, fingerprint: &str) -> Option<LedgerEntry> {
    let kv = env.kv(KV_BINDING).ok()?;
    let raw = kv.get(&key(repo, fingerprint)).text().await.ok().flatten()?;
    let entry: LedgerEntry = serde_json::from_str(&raw).ok()?;
    if !entry.is_replayable() {
        return None;
    }
    Some(entry)
}

/// Registra un PASS verificado por Actions (best-effort: un fallo de KV no
/// revoca el callback; el DO ya registro la decision autoritativa).
pub async fn record_pass(env: &Env, repo: &str, fingerprint: &str, entry: LedgerEntry) {
    if let Ok(kv) = env.kv(KV_BINDING) {
        if let Ok(serialized) = serde_json::to_string(&entry) {
            if let Ok(builder) = kv.put(&key(repo, fingerprint), serialized) {
                if let Err(e) = builder.execute().await {
                    console_error!("ledger kv put failed: {e}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(operator: u8, confidence: f32, risk: f32) -> LedgerEntry {
        LedgerEntry {
            operator,
            parameters: BTreeMap::new(),
            confidence,
            risk,
            pr_url: String::from("https://github.com/acme/api/pull/1"),
            correlation_id: String::from("inc-1"),
            updated_at_unix: 1,
        }
    }

    #[test]
    fn replayable_requires_actionable_operator_and_finite_values() {
        assert!(entry(1, 0.9, 0.1).is_replayable());
        assert!(!entry(0, 0.9, 0.1).is_replayable()); // NoOp
        assert!(!entry(255, 0.9, 0.1).is_replayable()); // Unknown
        assert!(!entry(1, f32::NAN, 0.1).is_replayable());
        assert!(!entry(1, 0.9, f32::INFINITY).is_replayable());
    }

    #[test]
    fn to_action_marks_replay_node_id() {
        let action = entry(1, 0.9, 0.1).to_action();
        assert_eq!(action.node_id, "ledger:replay");
        assert_eq!(action.repair_operator, repair_types::OperatorId::DependencyRepair);
    }

    #[test]
    fn key_is_deterministic_and_repo_scoped() {
        let a = key("acme/api", "e500|build|cargo build|rust|");
        assert_eq!(a, key("acme/api", "e500|build|cargo build|rust|"));
        assert_ne!(a, key("acme/other", "e500|build|cargo build|rust|"));
        assert!(a.starts_with("ledger:"));
    }
}
