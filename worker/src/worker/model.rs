//! Registro de modelo: current, stable, rollback y promocion.
//!
//! Regla PART3 (no negociable):
//!   current failure -> stable -> si no disponible -> BLOCKED
//! Nunca: current failure -> zeros -> PASS. Sin pesos validos no hay
//! inferencia: se devuelve error y el llamador registra BLOCKED.

use repair_nn_core::{RepairNet, WEIGHT_COUNT};
use worker::*;

pub const KV_BINDING: &str = "MODEL_KV";
pub const KEY_CURRENT: &str = "model/current";
pub const KEY_STABLE: &str = "model/stable";
pub const KEY_ROLLBACK: &str = "model/rollback";

pub struct LoadedModel {
    pub net: RepairNet,
    pub source: String,
}

/// Carga current; si esta ausente o malformado cae a stable; si tampoco hay,
/// Err. El fallback es REAL: si current falla a mitad de dia, stable responde.
pub async fn load(env: &Env) -> Result<LoadedModel> {
    if let Some(net) = load_from(env, KEY_CURRENT).await {
        return Ok(LoadedModel {
            net,
            source: format!("kv:{}", KEY_CURRENT),
        });
    }
    if let Some(net) = load_from(env, KEY_STABLE).await {
        return Ok(LoadedModel {
            net,
            source: format!("kv:{}", KEY_STABLE),
        });
    }
    Err(Error::JsError("blocked_no_model".into()))
}

/// Lee pesos planos desde KV. Formato: WEIGHT_COUNT f32 legibles con
/// split_whitespace. Cualquier desviacion devuelve None y NO se cae a
/// ceros: una red malformada no puede producir una confianza inventada.
async fn load_from(env: &Env, key: &str) -> Option<RepairNet> {
    let kv = env.kv(KV_BINDING).ok()?;
    let raw = kv.get(key).text().await.ok().flatten()?;
    if raw.trim().is_empty() {
        return None;
    }
    parse_plain_weights(&raw)
}

/// Parser compartido de pesos planos: WEIGHT_COUNT f32 legibles con
/// split_whitespace. Un solo formato de pesos en todo el sistema: lo usan
/// el champion (este modulo) y el challenger (candidate.rs). Cualquier
/// desviacion devuelve None y NO se cae a ceros: una red malformada no
/// puede producir una confianza inventada.
pub fn parse_plain_weights(raw: &str) -> Option<RepairNet> {
    let mut values: Vec<f32> = Vec::with_capacity(WEIGHT_COUNT);
    for token in raw.split_whitespace() {
        match token.parse::<f32>() {
            Ok(v) if v.is_finite() => values.push(v),
            // Un token no numerico invalida el archivo entero: no se ignora
            // en silencio, porque dejaria la red con pesos desplazados.
            _ => return None,
        }
        if values.len() > WEIGHT_COUNT {
            return None;
        }
    }
    RepairNet::from_weights(&values).ok()
}

/// ¿Existe la clave en MODEL_KV?
///
/// `async fn` y no un closure `|key: &str| async move { .. }` a proposito:
/// dentro de un closure el lifetime de `key` es independiente del de la future
/// devuelta, y el compilador rechaza el `await` con "lifetime may not live long
/// enough" (E0621 en el CI del 2026-10-02, item 51 de DISCREPANCIES). Una
/// `async fn` de nivel de modulo conecta ambos lifetimes correctamente.
async fn exists(env: &Env, key: &str) -> bool {
    match env.kv(KV_BINDING) {
        Ok(kv) => matches!(kv.get(key).text().await, Ok(Some(_))),
        Err(_) => false,
    }
}

/// GET /model: observabilidad del registro. Declara la politica de fallback y
/// el estado de cada puntero. No expone los pesos.
pub async fn report(env: &Env) -> Result<Response> {
    let current = exists(env, KEY_CURRENT).await;
    let stable = exists(env, KEY_STABLE).await;
    let rollback = exists(env, KEY_ROLLBACK).await;
    let (current_s, stable_s, rollback_s, kv_s) = match env.kv(KV_BINDING) {
        Ok(_) => (
            if current { "ok" } else { "missing" },
            if stable { "ok" } else { "missing" },
            if rollback { "ok" } else { "missing" },
            "bound",
        ),
        Err(_) => ("unknown", "unknown", "unknown", "unbound"),
    };
    Response::ok(
        serde_json::json!({
            "model": {
                "current": current_s,
                "stable": stable_s,
                "rollback": rollback_s
            },
            "kv": kv_s,
            "policy": "current -> stable -> BLOCKED (zeros forbidden)",
            "verify": "GitHub Actions"
        })
        .to_string(),
    )
}

/// Salud del registro de modelo para el MONITOR del cron: solo si existen
/// los punteros current/stable. No expone pesos ni promociona nada.
pub struct ModelHealth {
    pub current: bool,
    pub stable: bool,
}

pub async fn health(env: &Env) -> ModelHealth {
    ModelHealth {
        current: exists(env, KEY_CURRENT).await,
        stable: exists(env, KEY_STABLE).await,
    }
}
