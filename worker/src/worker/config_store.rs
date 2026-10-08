//! CONFIG EN CALIENTE desde el KV `AGENT_CONFIG` (adopcion del namespace
//! huerfano agent-config, decision del dueno 2026-10-08).
//!
//! Jerarquia (conservadora): KV primero, [vars] de wrangler.toml de
//! fallback. Cambiar REPAIR_RULES o las cuotas ya NO exige redeploy.
//!
//! Fail-closed para REGLAS: JSON invalido en el KV => Err y el consumidor
//! BLOQUEA (identico a una var REPAIR_RULES invalida). Fail-safe para
//! CUOTAS: JSON invalido o valor ilegible => se conserva el valor de
//! [vars] (la cuota de vars es una configuracion valida, no una apertura).
//!
//! Formato del KV (misma cuenta, namespace agent-config):
//!   config/rules : mismo JSON que REPAIR_RULES ([{id, expression, action}]).
//!   config/quota : objeto con claves opcionales (nombres de las vars):
//!     {"QUOTA_DAILY_BUDGET": 5, "QUOTA_MAX_OPEN_REPAIRS": "1", ...}.
//! Sin claves o vacio => comportamiento previo exacto ([vars]).

use worker::Env;

use crate::runtime::quota::{self, QuotaConfig};
use crate::runtime::rules::{self, Ruleset};

/// Binding del KV de configuracion (wrangler.toml: namespace agent-config).
pub const KV_BINDING: &str = "AGENT_CONFIG";
pub const KEY_RULES: &str = "config/rules";
pub const KEY_QUOTA: &str = "config/quota";

/// Reglas en caliente: KV `config/rules` si existe (fail-closed si su JSON
/// es invalido), si no la var REPAIR_RULES (comportamiento previo exacto).
pub async fn load_ruleset(env: &Env) -> Result<Ruleset, String> {
    if let Ok(kv) = env.kv(KV_BINDING) {
        if let Ok(Some(raw)) = kv.get(KEY_RULES).text().await {
            if !raw.trim().is_empty() {
                return rules::Ruleset::from_json(&raw);
            }
        }
    }
    rules::load(env)
}

/// Cuotas en caliente: [vars] como base + overrides del KV `config/quota`.
/// Los overrides ilegibles se ignoran (fail-safe documentado arriba).
pub async fn quota(env: &Env) -> QuotaConfig {
    let base = QuotaConfig::from_env(env);
    let Some(raw) = read_kv(env, KEY_QUOTA).await else {
        return base;
    };
    apply_quota_overrides(base, &raw).unwrap_or(base)
}

/// Lee una clave no vacia del KV de config. Best-effort: sin binding o sin
/// clave => None (el llamador conserva el fallback de [vars]).
async fn read_kv(env: &Env, key: &str) -> Option<String> {
    let kv = env.kv(KV_BINDING).ok()?;
    let raw = kv.get(key).text().await.ok().flatten()?;
    if raw.trim().is_empty() {
        return None;
    }
    Some(raw)
}

/// Puro (testeado): aplica overrides JSON sobre la config base. None SOLO
/// si el JSON no es un objeto (el llamador conserva la base: fail-safe).
pub fn apply_quota_overrides(base: QuotaConfig, raw: &str) -> Option<QuotaConfig> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let obj = value.as_object()?;
    let mut cfg = base;
    for (key, val) in obj {
        let text = match val {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        match key.as_str() {
            quota::VAR_MAX_ATTEMPTS_PER_INCIDENT => {
                cfg.max_attempts_per_incident =
                    text.parse().unwrap_or(cfg.max_attempts_per_incident);
            }
            quota::VAR_MAX_REPAIRS_PER_REPO => {
                cfg.max_repairs_per_repo = text.parse().unwrap_or(cfg.max_repairs_per_repo);
            }
            quota::VAR_MAX_OPEN_REPAIRS => {
                cfg.max_open_repairs = text.parse().unwrap_or(cfg.max_open_repairs);
            }
            quota::VAR_COOLDOWN_SECONDS => {
                // cooldown_seconds es i64 y acepta negativos: un cooldown
                // negativo desactivaria la cuota en el DO (review PR #114).
                // Solo se aceptan valores >= 0; lo demas conserva la base.
                if let Ok(v) = text.parse::<i64>() {
                    if v >= 0 {
                        cfg.cooldown_seconds = v;
                    }
                }
            }
            quota::VAR_DAILY_BUDGET => {
                cfg.daily_budget = text.parse().unwrap_or(cfg.daily_budget);
            }
            _ => {}
        }
    }
    Some(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_overrides_apply_only_known_keys() {
        let base = QuotaConfig::default();
        let raw =
            r#"{"QUOTA_DAILY_BUDGET": 5, "QUOTA_MAX_OPEN_REPAIRS": "1", "UNKNOWN_KEY": "99"}"#;
        let cfg = apply_quota_overrides(base, raw).expect("json objeto valido");
        assert_eq!(cfg.daily_budget, 5);
        assert_eq!(cfg.max_open_repairs, 1);
        assert_eq!(
            cfg.max_attempts_per_incident,
            base.max_attempts_per_incident
        );
    }

    #[test]
    fn quota_overrides_reject_non_object() {
        let base = QuotaConfig::default();
        assert!(apply_quota_overrides(base, "[1,2]").is_none());
        assert!(apply_quota_overrides(base, "no-json{").is_none());
    }

    #[test]
    fn quota_overrides_ignore_garbage_values() {
        let base = QuotaConfig::default();
        let cfg = apply_quota_overrides(base, r#"{"QUOTA_DAILY_BUDGET": "abc"}"#)
            .expect("json objeto valido");
        assert_eq!(cfg.daily_budget, base.daily_budget);
    }
}
