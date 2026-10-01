//! Cloudflare Worker (workers-rs). Orquestador: NO es el motor de cómputo.
//!
//! Pipeline edge (Incident → features → NN → gate), sin PR todavía:
//!   POST /webhook → Incident → feature_engine::extract → RepairNet::predict
//!                → repair_operators::gate → respuesta con PipelineReport
//!
//! La NN se enlaza como crate Rust (`repair_nn_core`, no_std + alloc) dentro del
//! propio módulo WASM del Worker. Por eso `wrangler.toml` no necesita
//! `[wasm_modules]`: `repair_nn_wasm` existe para consumidores JS/navegador.
//!
//! Autoridad de VERIFY = GitHub Actions, nunca la confidence del modelo.
//! No hay ruta LLM ni generación libre de código (docs/NO_LLM_POLICY.md).
//! Sin `unwrap()` en el path de request.

use feature_engine::extract;
use repair_nn_core::{RepairNet, WEIGHT_COUNT};
use repair_operators::gate;
use repair_types::{FailureSignature, Incident, RepairAction};
use serde::Deserialize;
use worker::*;

/// Umbrales: fuente de verdad docs/GOVERNANCE.md (0.55 / 0.45).
const MIN_CONFIDENCE: f32 = 0.55;
const MAX_RISK: f32 = 0.45;

/// Constant-time string comparison to prevent timing attacks on WEBHOOK_SECRET.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut res = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        res |= x ^ y;
    }
    res == 0
}

/// Cuerpo entrante del webhook. Todo opcional y con `#[serde(default)]` para
/// que un payload parcial nunca rompa el isolate.
#[derive(Debug, Default, Deserialize)]
struct WebhookPayload {
    #[serde(default)]
    id: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    error_code: String,
    #[serde(default)]
    error_step: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    attempts: u32,
    #[serde(default)]
    stack_hint: String,
    #[serde(default)]
    language_hint: String,
    #[serde(default)]
    framework_hint: String,
}

impl WebhookPayload {
    fn into_incident(self) -> Incident {
        Incident {
            id: if self.id.is_empty() {
                String::from("unidentified")
            } else {
                self.id
            },
            source: if self.source.is_empty() {
                String::from("webhook")
            } else {
                self.source
            },
            error_code: self.error_code,
            error_step: self.error_step,
            command: self.command,
            message: self.message,
            project: self.project,
            attempts: self.attempts,
            stack_hint: self.stack_hint,
            language_hint: self.language_hint,
            framework_hint: self.framework_hint,
            verified: false,
            status: String::from("open"),
        }
    }
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    Router::new()
        .get("/", |_, _| Response::ok("AUTO-REPAIR LAB"))
        .get("/health", |_, _| Response::ok("ok"))
        .get_async("/model", |_, ctx| async move {
            match ctx.kv("MODEL_KV") {
                Ok(kv) => {
                    let ptr = kv.get("model/current").text().await.ok().flatten();
                    match ptr {
                        Some(p) => Response::ok(format!(r#"{{"model_ptr":"{}"}}"#, p)),
                        None => Response::ok(r#"{"model_ptr":null}"#),
                    }
                }
                Err(_) => Response::ok(r#"{"model_ptr":null,"kv":"unbound"}"#),
            }
        })
        .post_async("/webhook", handle_webhook)
        .run(req, env)
        .await
}

async fn handle_webhook(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let env = ctx.env;

    // 1. Autorización. Sin secret configurado el endpoint queda cerrado:
    //    nunca "fail open" en un path que puede abrir PRs.
    match env.secret("WEBHOOK_SECRET") {
        Ok(secret) => {
            let header = req.headers().get("x-webhook-secret")?.unwrap_or_default();
            let secret_str = secret.to_string();
            if !constant_time_eq(header.as_bytes(), secret_str.as_bytes()) {
                return Response::error("unauthorized", 401);
            }
        }
        Err(_) => {
            return Response::error("webhook_secret_not_configured", 503);
        }
    }

    // 2. Body. Un payload corrupto no debe tumbar el isolate.
    let body = match req.text().await {
        Ok(text) => text,
        Err(_) => return Response::error("invalid_body", 400),
    };
    if body.trim().is_empty() {
        return Response::error("empty_body", 400);
    }
    let payload: WebhookPayload = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(_) => return Response::error("invalid_json", 400),
    };

    // 3. Pipeline: Incident → features → NN → gate.
    let incident = payload.into_incident();
    let signature = FailureSignature::from_incident(&incident);
    let features = extract(&incident, &signature);

    // 4. Pesos. model/*.json sigue en weights:null, así que por defecto se usa
    //    una red de ceros y la respuesta lo declara. No se finge inferencia real.
    let (net, weights_source) = match load_weights(&env).await {
        Some(n) => (n, "kv:model/current"),
        None => (RepairNet::zeros(), "zeros:no_weights"),
    };

    let action: RepairAction = net.predict(&features);

    // 5. Gate. Este es el punto de decisión; el gate nunca se puede saltar.
    let decision = match gate(&action, MIN_CONFIDENCE, MAX_RISK) {
        Ok(()) => format!(
            r#"{{"status":"success","phase":"policy","operator_id":{},"operator":"{}","confidence":{:.3},"risk":{:.3},"weights":"{}","pr":null,"note":"gate allowed; PR creation not implemented"}}"#,
            action.repair_operator as u8,
            action.repair_operator.as_str(),
            action.confidence,
            action.risk,
            weights_source
        ),
        // `report` no se interpola en el JSON: su reason lleva texto libre y
        // aquí se reconstruye desde `action`, que son los mismos valores.
        Err(_report) => format!(
            r#"{{"status":"blocked_by_policy","phase":"policy","operator_id":{},"operator":"{}","confidence":{:.3},"risk":{:.3},"weights":"{}","reason":"c={:.3} r={:.3} op={}"}}"#,
            action.repair_operator as u8,
            action.repair_operator.as_str(),
            action.confidence,
            action.risk,
            weights_source,
            action.confidence,
            action.risk,
            action.repair_operator.as_str()
        ),
    };

    Response::ok(decision)
}

/// Lee pesos planos desde KV. Formato esperado: 2863 f32 legibles por `split_whitespace`
/// (`WEIGHT_COUNT`). Cualquier desviación devuelve `None` y se cae a ceros:
/// una red malformada no puede producir una acción con confianza inventada.
async fn load_weights(env: &Env) -> Option<RepairNet> {
    let kv = env.kv("MODEL_KV").ok()?;
    let raw = kv.get("model/current").text().await.ok().flatten()?;
    if raw.trim().is_empty() {
        return None;
    }
    let mut values: Vec<f32> = Vec::with_capacity(WEIGHT_COUNT);
    for token in raw.split_whitespace() {
        match token.parse::<f32>() {
            Ok(v) if v.is_finite() => values.push(v),
            // Un token no numérico invalida el archivo entero: no se ignora en
            // silencio, porque dejaría la red con pesos desplazados.
            _ => return None,
        }
        if values.len() > WEIGHT_COUNT {
            return None;
        }
    }
    RepairNet::from_weights(&values).ok()
}
