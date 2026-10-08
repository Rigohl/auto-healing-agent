//! CHAMPION/CHALLENGER de pesos (adopcion del namespace huerfano
//! neural-net-weights, decision del dueno 2026-10-08).
//!
//! MODEL_KV (current/stable) sigue siendo el UNICO champion que sirve
//! inferencia (model.rs). Este registro recibe pesos CANDIDATOS propuestos
//! por repair_train / promote-model.yml: este modulo solo VALIDA su
//! integridad (formato plano, WEIGHT_COUNT exacto, tokens finitos) y
//! reporta. La promocion (candidate -> MODEL_KV/model/current) la decide
//! promote-model.yml con revision humana (AUTO_DEPLOY=false): el pipeline
//! NUNCA consume pesos de aqui. Fail-closed de contenido: pesos
//! malformados = "invalid", nunca zeros.

use worker::Env;

use crate::runtime::model;

/// Binding del KV de pesos candidatos (wrangler.toml: namespace
/// neural-net-weights).
pub const KV_BINDING: &str = "NN_WEIGHTS";
pub const KEY_CANDIDATE: &str = "model/candidate";

/// Estado del candidato (solo observabilidad; sin efecto en inferencia).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateStatus {
    /// Presentes y con formato/numero exacto: listos para que
    /// promote-model.yml los evalue y promueva (con revision humana).
    Valid,
    /// Presentes pero malformados: NO promocionar.
    Invalid,
    /// Clave ausente (todavia sin candidato).
    Missing,
    /// KV ilegible en este momento.
    Unknown,
    /// Binding ausente en wrangler.toml (config rota).
    Unbound,
}

impl CandidateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::Missing => "missing",
            Self::Unknown => "unknown",
            Self::Unbound => "unbound",
        }
    }
}

/// Lee y valida el candidato con el MISMO parser del champion
/// (model::parse_plain_weights): un solo formato de pesos en todo el
/// sistema. Nunca expone pesos ni sirve inferencia.
pub async fn status(env: &Env) -> CandidateStatus {
    let Ok(kv) = env.kv(KV_BINDING) else {
        return CandidateStatus::Unbound;
    };
    let raw = match kv.get(KEY_CANDIDATE).text().await {
        Ok(Some(raw)) => raw,
        // get Err (KV ilegible) o clave ausente: unknown/missing no cambian
        // ninguna decision (el cron solo loguea).
        Err(_) => return CandidateStatus::Unknown,
        Ok(None) => return CandidateStatus::Missing,
    };
    if raw.trim().is_empty() {
        return CandidateStatus::Missing;
    }
    if model::parse_plain_weights(&raw).is_some() {
        CandidateStatus::Valid
    } else {
        CandidateStatus::Invalid
    }
}

/// GET /model/candidate: informe de estado + politica de promocion.
pub async fn report(env: &Env) -> worker::Result<worker::Response> {
    let status = status(env).await;
    worker::Response::ok(
        serde_json::json!({
            "candidate": status.as_str(),
            "source": format!("kv:{}:{}", KV_BINDING, KEY_CANDIDATE),
            "policy": "candidate never serves inference; promotion = promote-model.yml with human review",
            "verify": "GitHub Actions"
        })
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_labels_are_stable() {
        assert_eq!(CandidateStatus::Valid.as_str(), "valid");
        assert_eq!(CandidateStatus::Invalid.as_str(), "invalid");
        assert_eq!(CandidateStatus::Missing.as_str(), "missing");
        assert_eq!(CandidateStatus::Unknown.as_str(), "unknown");
        assert_eq!(CandidateStatus::Unbound.as_str(), "unbound");
    }
}
