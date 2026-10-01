//! Cloudflare Worker (workers-rs). Orquestador: NO es el motor de computo.
//!
//! PART3 runtime async:
//!   POST /webhook -> secret fail-closed -> Durable Object (dedup +
//!   idempotencia + quota + anti-loop) -> Queue -> 202 con correlation_id.
//!   El consumidor de la cola ejecuta el pipeline (features -> NN -> gate)
//!   y registra decision y verificacion en el Durable Object.
//!
//! La NN se enlaza como crate Rust (repair_nn_core, no_std + alloc) dentro
//! del propio modulo WASM del Worker. Por eso wrangler.toml no necesita
//! [wasm_modules].
//!
//! Regla de autoridad: Cloudflare ORCHESTRATES, PERSISTS, DEDUPLICATES,
//! QUEUES, LIMITS, OBSERVES. GitHub Actions es la autoridad de VERIFY;
//! este Worker nunca declara CI PASS, aprueba ni fusiona PRs.
//! No hay ruta LLM ni generacion libre de codigo (docs/NO_LLM_POLICY.md).
//! Sin unwrap() en el path de request.

// El modulo interno NO puede llamarse `worker`: colisiona con el crate
// externo `worker` y hace ambigua cada ruta `use worker::...` (error
// E0659). El job `worker` de CI detecto exactamente eso en c5fadd48.
// El modulo se llama `runtime` y se mapea al mismo directorio src/worker/.
#[path = "worker/mod.rs"]
mod runtime;

use feature_engine::extract;
use repair_operators::gate;
use repair_types::FailureSignature;
use worker::*;

use crate::runtime::{
    model,
    queue_consumer::{self, QueueTask, WebhookPayload},
    quota::QuotaConfig,
    anti_loop::AntiLoopConfig,
    security::{fnv1a64, urlencode, verify_webhook_secret},
    MIN_CONFIDENCE, MAX_RISK,
};

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    Router::new()
        .get("/", |_, _| Response::ok("AUTO-REPAIR LAB"))
        .get("/health", |_, _| Response::ok("ok"))
        .get_async("/model", |_, ctx| async move {
            model::report(ctx.env).await
        })
        .post_async("/webhook", handle_webhook)
        .run(req, env)
   
     .await
}

/// Consumidor de cola (produccion, staging y DLQ): retry con backoff,
/// hard-stop y DLQ. Ver worker/queue_consumer.rs.
#[event(queue)]
pub async fn queue_main(batch: MessageBatch<QueueTask>, env: Env, _ctx: Context) -> Result<()> {
    queue_consumer::consume(batch, env).await
}

#[worker::send]
async fn handle_webhook(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let env = ctx.env;

    // 1. Autorizacion fail-closed. Sin secret configurado el endpoint queda
    //    cerrado (503); con secret, comparacion en tiempo constante.
    let secret = match env.secret("WEBHOOK_SECRET") {
        Ok(s) => s.to_string(),
        Err(_) => return Response::error("webhook_secret_not_configured", 503),
    };
    let header = req.headers().get("x-webhook-secret")?;
    if !verify_webhook_secret(header.as_deref(), &secret) {
        return Response::error("unauthorized", 401);
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

    // 3. Correlacion estructurada + clave de idempotencia (delivery id).
    let incident_id = payload.incident_id();
    let repo = payload.repo();
    let signature = payload.signature_str();
    let delivery_id = if payload.id.is_empty() { incident_id.clone() } else { payload.id.clone() };
    let now = crate::runtime::now_ms();
    let correlation_id = format!(
        "{}-{:x}",
        incident_id,
        fnv1a64(format!("{}|{}|{}", incident_id, signature, now).as_bytes())
    );
    let idem_key = format!(
        "{:x}",
        fnv1a64(format!("{}|{}", delivery_id, signature).as_bytes())
    );

    // 4. Dura
ble Object: dedup, idempotencia, quota y anti-loop ANTES de
    //    encolar. Si el DO no responde: fail-closed 503, sin efectos.
    let quota_cfg = QuotaConfig::from_env(&env);
    let anti_cfg = AntiLoopConfig::from_env(&env);
    let qs = format!(
        "/ingest?repo={}&incident_id={}&signature={}&delivery_id={}&idem_key={}&correlation_id={}&max_attempts_per_incident={}&max_repairs_per_repo={}&max_open_repairs={}&cooldown_seconds={}&daily_budget={}&max_same_incident={}&max_same_signature={}&window_seconds={}",
        urlencode(&repo),
        urlencode(&incident_id),
        urlencode(&signature),
        urlencode(&delivery_id),
        urlencode(&idem_key),
        urlencode(&correlation_id),
        quota_cfg.max_attempts_per_incident,
        quota_cfg.max_repairs_per_repo,
        quota_cfg.max_open_repairs,
        quota_cfg.cooldown_seconds,
        quota_cfg.daily_budget,
        anti_cfg.max_same_incident,
        anti_cfg.max_same_signature,
        anti_cfg.window_seconds
    );
    let do_text = match crate::runtime::call_do(env.clone(), repo.clone(), qs).await {
        Ok(t) => t,
        Err(_) => return Response::error("state_store_unavailable", 503),
    };
    let verdict: serde_json::Value = match serde_json::from_str(&do_text) {
        Ok(v) => v,
        Err(_) => return Response::error("state_store_invalid_response", 503),
    };
    let status = verdict.get("status").and_then(|s| s.as_str()).unwrap_or("error");
    if status != "queued" {
        // duplicate | blocked_quota | blocked_anti_loop: misma decision para
        // la misma entrega; el sender NO debe reintentar (200).
        return Response::ok(do_text);
    }

    // 5. Preview determinista del gate. Compatibilidad con el smoke test de
    //    deploy.yml (exige operator_id en la respuesta). La decision
    //    autoritativa es la del consumidor asincrono; VERIFY = Actions.
    let incident = payload.to_incident();
    let fsig = FailureSignature::from_incident(&inciden
t);
    let features = extract(&incident, &fsig);
    let preview = match model::load(&env).await {
        Ok(loaded) => {
            let action = loaded.net.predict(&features);
            let gate_ok = gate(&action, MIN_CONFIDENCE, MAX_RISK).is_ok();
            serde_json::json!({
                "operator_id": action.repair_operator as u8,
                "operator": action.repair_operator.as_str(),
                "confidence": action.confidence,
                "risk": action.risk,
                "gate": if gate_ok { "allow" } else { "blocked_by_policy" },
                "weights": loaded.source,
            })
        }
        // Sin modelo (current ni stable): BLOCKED. Nunca zeros -> PASS.
        Err(_) => serde_json::json!({ "gate": "blocked_no_model" }),
    };

    // 6. Encolar el trabajo asincrono.
    let task = QueueTask {
        correlation_id: correlation_id.clone(),
        incident_id: incident_id.clone(),
        repo: repo.clone(),
        signature: signature.clone(),
        payload,
        attempts: 0,
        enqueued_at: now,
    };
    let queue = env.queue(queue_consumer::QUEUE_BINDING)?;
    if let Err(e) = queue.send(task).await {
        console_error!("queue send failed: {}", e);
        // Fail-closed: dejar el incidente bloqueado en el DO antes del 503.
        let rqs = format!(
            "/result?correlation_id={}&incident_id={}&decision=blocked&fingerprint=&verify_status=blocked&evidence_ref=&reason=queue_send_failed",
            urlencode(&correlation_id),
            urlencode(&incident_id)
        );
        let _ = crate::runtime::call_do(env, repo, rqs).await;
        return Response::error("queue_unavailable", 503);
    }

    Response::ok(
        serde_json::json!({
            "status": "accepted",
            "correlation_id": correlation_id,
            "incident_id": incident_id,
            "queued": true,
            "preview": preview,
            "pr": null,
            "note": "async repair queued; VER
IFY authority = GitHub Actions"
        })
        .to_string(),
    )
}
