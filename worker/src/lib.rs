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
//! Capa HTTP: Router de Axum (patron del ejemplo oficial examples/axum de
//! workers-rs: worker features "http" + "axum"). La migracion conserva
//! cuerpos y codigos de respuesta exactos: el smoke test de deploy.yml y
//! los claims de scripts/verify_repo.py dependen de ellos.
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

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response as AxumResponse};
use axum::routing::{get, post};
use axum::Router;
use tower_service::Service;
use worker::*;

use feature_engine::extract;
use repair_operators::gate;
use repair_types::{compute_idempotency_key, FailureSignature};

use crate::runtime::{
    anti_loop::AntiLoopConfig,
    dashboard,
    model, monitor,
    queue_consumer::{self, QueueTask, WebhookPayload},
    quota::QuotaConfig,
    security::{fnv1a64, urlencode, verify_webhook_secret},
    MAX_RISK, MIN_CONFIDENCE,
};

/// 503 fail-closed con el cuerpo literal que el Router legacy producia con
/// Response::error(msg, 503): el smoke test de deploy.yml lo espera tal cual.
fn svc_unavailable(body: &'static str) -> AxumResponse {
    (StatusCode::SERVICE_UNAVAILABLE, body).into_response()
}

/// 400 con cuerpo literal (invalid_body | empty_body | invalid_json).
fn bad_request(body: &'static str) -> AxumResponse {
    (StatusCode::BAD_REQUEST, body).into_response()
}

/// 500 con cuerpo literal para fallos internos de generacion de respuesta.
fn internal_error(body: &'static str) -> AxumResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
}

/// Router de Axum con Env como estado compartido. Misma superficie HTTP
/// que el Router legacy de workers-rs: GET /, /health, /model y
/// POST /webhook, /github/callback.
fn router(env: Env) -> Router {
    Router::new()
        .route("/", get(|| async { "AUTO-REPAIR LAB" }))
        .route("/health", get(|| async { "ok" }))
        .route("/model", get(model_report))
        .route("/dashboard", get(dashboard_report))
        .route("/webhook", post(handle_webhook))
        .route("/github/callback", post(handle_github_callback))
        .with_state(env)
}

#[event(fetch)]
async fn main(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    console_error_panic_hook::set_once();
    Ok(router(env).call(req).await?)
}

/// GET /model: informe de salud del registro de modelos. `model::report`
/// devuelve una respuesta del crate `worker`; se transporta preservando
/// status y cuerpo en la respuesta de Axum.
#[worker::send]
async fn model_report(State(env): State<Env>) -> AxumResponse {
    let mut report = match model::report(&env).await {
        Ok(report) => report,
        Err(e) => {
            console_error!("model report failed: {e}");
            return internal_error("model_report_failed");
        }
    };
    let code = report.status_code();
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    match report.text().await {
        Ok(text) => (status, text).into_response(),
        Err(e) => {
            console_error!("model report body failed: {e}");
            internal_error("model_report_failed")
        }
    }
}

/// GET /dashboard: observabilidad de negocio (FASE 1, docs/DASHBOARD.md).
/// Agrega los RepairCases de REPAIR_CASES_KV y pinta HTML+SVG server-side
/// (runtime::dashboard). Autorizacion fail-closed identica a /webhook pero
/// con su propio secret: sin DASHBOARD_TOKEN configurado -> 503; token
/// incorrecto -> 401 (comparacion en tiempo constante). Solo lectura:
/// nunca repara, nunca encola, nunca toca la autoridad de VERIFY.
#[worker::send]
async fn dashboard_report(State(env): State<Env>, headers: HeaderMap) -> AxumResponse {
    let secret = match env.secret("DASHBOARD_TOKEN") {
        Ok(s) => s.to_string(),
        Err(_) => return svc_unavailable("dashboard_token_not_configured"),
    };
    if secret.is_empty() {
        return svc_unavailable("dashboard_token_not_configured");
    }
    let header = headers
        .get("x-dashboard-token")
        .and_then(|v| v.to_str().ok());
    if !verify_webhook_secret(header, &secret) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }
    let mut report = match dashboard::render(&env).await {
        Ok(r) => r,
        Err(e) => {
            console_error!("dashboard render failed: {e}");
            return internal_error("dashboard_render_failed");
        }
    };
    let code = report.status_code();
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    match report.text().await {
        Ok(html) => {
            let mut resp = (status, html).into_response();
            resp.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("text/html; charset=utf-8"),
            );
            resp
        }
        Err(e) => {
            console_error!("dashboard body failed: {e}");
            internal_error("dashboard_render_failed")
        }
    }
}

/// POST /webhook (PASO 2): secret fail-closed -> Durable Object (dedup,
/// idempotencia, quota, anti-loop) -> Queue. Los extractores de Axum
/// (State, HeaderMap, Bytes) sustituyen a Request/RouteContext; el
/// algoritmo y los cuerpos de error son identicos al del Router legacy.
#[worker::send]
async fn handle_webhook(State(env): State<Env>, headers: HeaderMap, body: Bytes) -> AxumResponse {
    // 1. Autorizacion fail-closed. Sin secret configurado el endpoint queda
    //    cerrado (503); con secret, comparacion en tiempo constante. Un secret
    //    VACIO cuenta como no configurado: con un secret "" el header vacio
    //    `x-webhook-secret: ` pasaria la comparacion y abriria el endpoint.
    let secret = match env.secret("WEBHOOK_SECRET") {
        Ok(s) => s.to_string(),
        Err(_) => return svc_unavailable("webhook_secret_not_configured"),
    };
    if secret.is_empty() {
        return svc_unavailable("webhook_secret_not_configured");
    }
    let header = headers
        .get("x-webhook-secret")
        .and_then(|v| v.to_str().ok());
    if !verify_webhook_secret(header, &secret) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    // 1b. delivery_id real de la entrega (BUG-03): el header
    //     X-GitHub-Delivery identifica la ENTREGA, distinta del incidente
    //     (payload.id). Derivar ambos del mismo campo colapsaba la clave
    //     de idempotencia en "por incidente" y descartaba entregas nuevas
    //     del mismo incidente como duplicadas durante el TTL. Sin header,
    //     fallback al incident_id (comportamiento anterior, conservador).
    let header_delivery: String = headers
        .get("x-github-delivery")
        .and_then(|v| v.to_str().ok())
        .filter(|d| !d.is_empty())
        .map(|d| d.to_string())
        .unwrap_or_default();

    // 2. Body. Un payload corrupto no debe tumbar el isolate.
    let body = match std::str::from_utf8(&body) {
        Ok(text) => text,
        Err(_) => return bad_request("invalid_body"),
    };
    if body.trim().is_empty() {
        return bad_request("empty_body");
    }
    let payload: WebhookPayload = match serde_json::from_str(body) {
        Ok(p) => p,
        Err(_) => return bad_request("invalid_json"),
    };

    // 3. Correlacion estructurada + clave de idempotencia CANONICA del
    //    contrato (CONTRACT.md §5 / repair_types::compute_idempotency_key):
    //    FNV-1a(repo | incident | delivery | fingerprint). Antes el worker
    //    usaba una formula propia (delivery|signature) distinta de la que el
    //    contrato documenta; ahora hay UNA sola fuente de verdad.
    let incident = payload.to_incident();
    let fsig = FailureSignature::from_incident(&incident);
    let incident_id = payload.incident_id();
    let repo = payload.repo();
    let signature = payload.signature_str();
    let delivery_id = if header_delivery.is_empty() {
        incident_id.clone()
    } else {
        header_delivery
    };
    let now = crate::runtime::now_ms();
    let correlation_id = format!(
        "{}-{:x}",
        incident_id,
        fnv1a64(format!("{}|{}|{}", incident_id, signature, now).as_bytes())
    );
    let idem_key = compute_idempotency_key(&repo, &incident_id, &delivery_id, &fsig.fingerprint);

    // 4. Durable Object: dedup, idempotencia, quota y anti-loop ANTES de
    //    encolar. Si el DO no responde: fail-closed 503, sin efectos.
    let quota_cfg = QuotaConfig::from_env(&env);
    let anti_cfg = AntiLoopConfig::from_env(&env);
    let qs = format!(
        // Sin `delivery_id`: el DO deduplica por `idem_key`, que ya lo
        // codifica (FNV-1a(repo | incident | delivery | fingerprint)).
        // Mandarlo era ruido.
        "/ingest?repo={}&incident_id={}&signature={}&idem_key={}&correlation_id={}&max_attempts_per_incident={}&max_repairs_per_repo={}&max_open_repairs={}&cooldown_seconds={}&daily_budget={}&max_same_incident={}&max_same_signature={}&max_same_fingerprint={}&max_same_failing_verification={}&window_seconds={}",
        urlencode(&repo),
        urlencode(&incident_id),
        urlencode(&signature),
        urlencode(&idem_key),
        urlencode(&correlation_id),
        quota_cfg.max_attempts_per_incident,
        quota_cfg.max_repairs_per_repo,
        quota_cfg.max_open_repairs,
        quota_cfg.cooldown_seconds,
        quota_cfg.daily_budget,
        anti_cfg.max_same_incident,
        anti_cfg.max_same_signature,
        anti_cfg.max_same_fingerprint,
        anti_cfg.max_same_failing_verification,
        anti_cfg.window_seconds
    );
    let do_text = match crate::runtime::call_do(env.clone(), repo.clone(), qs).await {
        Ok(t) => t,
        Err(_) => return svc_unavailable("state_store_unavailable"),
    };
    let verdict: serde_json::Value = match serde_json::from_str(&do_text) {
        Ok(v) => v,
        Err(_) => return svc_unavailable("state_store_invalid_response"),
    };
    let status = verdict
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("error");
    if status != "queued" {
        // duplicate | blocked_quota | blocked_anti_loop: misma decision para
        // la misma entrega; el sender NO debe reintentar (200).
        return (StatusCode::OK, do_text).into_response();
    }

    // 5. Preview determinista del gate. Compatibilidad con el smoke test de
    //    deploy.yml (exige operator_id en la respuesta). La decision
    //    autoritativa es la del consumidor asincrono; VERIFY = Actions.
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
        enqueued_at: now,
    };
    let queue = match env.queue(queue_consumer::QUEUE_BINDING) {
        Ok(queue) => queue,
        Err(e) => {
            console_error!("queue binding failed: {e}");
            return svc_unavailable("queue_unavailable");
        }
    };
    if let Err(e) = queue.send(task).await {
        console_error!("queue send failed: {}", e);
        // Fail-closed: dejar el incidente bloqueado en el DO antes del 503.
        let rqs = format!(
            "/result?correlation_id={}&incident_id={}&decision=blocked&fingerprint=&verify_status=blocked&evidence_ref=&reason=queue_send_failed",
            urlencode(&correlation_id),
            urlencode(&incident_id)
        );
        let _ = crate::runtime::call_do(env, repo, rqs).await;
        return svc_unavailable("queue_unavailable");
    }

    let resp = serde_json::json!({
        "status": "accepted",
        "correlation_id": correlation_id,
        "incident_id": incident_id,
        "queued": true,
        "preview": preview,
        "pr": null,
        "note": "async repair queued; VERIFY authority = GitHub Actions"
    })
    .to_string();
    (StatusCode::OK, resp).into_response()
}

/// POST /github/callback (PASO 4): GitHub Actions reporta el resultado
/// de VERIFY (CI del PR de reparacion) y el DO asienta la verificacion.
/// Este endpoint NUNCA declara PASS por si mismo: solo registra lo que CI
/// envia. Misma autorizacion fail-closed que /webhook.
#[worker::send]
async fn handle_github_callback(
    State(env): State<Env>,
    headers: HeaderMap,
    body: Bytes,
) -> AxumResponse {
    let secret = match env.secret("WEBHOOK_SECRET") {
        Ok(s) => s.to_string(),
        Err(_) => return svc_unavailable("webhook_secret_not_configured"),
    };
    if secret.is_empty() {
        return svc_unavailable("webhook_secret_not_configured");
    }
    let header = headers
        .get("x-webhook-secret")
        .and_then(|v| v.to_str().ok());
    if !verify_webhook_secret(header, &secret) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    let body = match std::str::from_utf8(&body) {
        Ok(t) => t,
        Err(_) => return bad_request("invalid_body"),
    };
    let payload: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return bad_request("invalid_json"),
    };
    let get_str = |k: &str| -> String {
        payload
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let correlation_id = get_str("correlation_id");
    if correlation_id.is_empty() {
        return bad_request("missing_correlation_id");
    }
    let incident_id = get_str("incident_id");
    let repo = get_str("repo");
    let evidence_ref = get_str("evidence_ref");
    let fingerprint = get_str("fingerprint");

    // verify_status: pass | fail | blocked (autoridad: GitHub Actions).
    let verify_status = match get_str("verify_status").to_lowercase().as_str() {
        "pass" => "pass",
        "fail" => "fail",
        "blocked" => "blocked",
        _ => return bad_request("invalid_verify_status"),
    };
    // El DO marca done solo si CI pasa; fail deja el incidente bloqueado
    // con la evidencia para auditoria.
    let decision = if verify_status == "pass" {
        "allow"
    } else {
        "ci_failed"
    };

    let qs = format!(
        "/result?correlation_id={}&incident_id={}&decision={}&fingerprint={}&verify_status={}&evidence_ref={}&reason=github_actions_verify",
        urlencode(&correlation_id),
        urlencode(&incident_id),
        decision,
        urlencode(&fingerprint),
        verify_status,
        urlencode(&evidence_ref)
    );
    let do_text = match crate::runtime::call_do(env, repo, qs).await {
        Ok(t) => t,
        Err(_) => return svc_unavailable("state_store_unavailable"),
    };
    let resp = serde_json::json!({
        "status": "recorded",
        "correlation_id": correlation_id,
        "verify_status": verify_status,
        "state_store": do_text,
    })
    .to_string();
    (StatusCode::OK, resp).into_response()
}

/// Consumidor de cola (produccion, staging y DLQ): retry con backoff,
/// hard-stop y DLQ. Ver worker/src/worker/queue_consumer.rs.
#[event(queue)]
pub async fn queue_main(batch: MessageBatch<QueueTask>, env: Env, _ctx: Context) -> Result<()> {
    queue_consumer::consume(batch, env).await
}

/// MONITOR (cron scheduled): retencion del DO + salud del registro de
/// modelo (worker/src/worker/monitor.rs). Observabilidad y limpieza:
/// nunca repara, nunca encola, nunca toca la autoridad de VERIFY
/// (GitHub Actions). Requiere [triggers] crons en wrangler.toml.
///
/// Devuelve () a proposito: el glue de #[event(scheduled)] en workers-rs
/// descarta el Result del handler, y devolver Result<()> activaria
/// unused_must_use bajo `clippy -D warnings`. El error solo se loguea.
#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    if let Err(e) = monitor::run(&env).await {
        console_error!("MONITOR cron failed: {e}");
    }
}
