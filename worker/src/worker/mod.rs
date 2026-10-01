//! Modulos del orquestador PART3 (runtime Cloudflare).
//!
//! Regla de autoridad: Cloudflare ORCHESTRATES, PERSISTS, DEDUPLICATES,
//! QUEUES, LIMITS y OBSERVES. GitHub Actions es la autoridad de VERIFY;
//! este runtime nunca declara CI PASS, aprueba ni fusiona PRs.

pub mod anti_loop;
pub mod incident_state;
pub mod model;
pub mod queue_consumer;
pub mod quota;
pub mod security;

/// Umbrales del gate: fuente de verdad docs/GOVERNANCE.md (0.55 / 0.45).
pub const MIN_CONFIDENCE: f32 = 0.55;
pub const MAX_RISK: f32 = 0.45;

/// Binding del Durable Object de estado (worker/wrangler.toml).
pub const DO_BINDING: &str = "INCIDENT_STATE";

/// Milisegundos desde epoch usando el reloj del runtime.
pub fn now_ms() -> i64 {
    worker::js_sys::Date::now() as i64
}

/// Llama al Durable Object del repositorio con un path+query y devuelve el
/// cuerpo de la respuesta como texto. Fail-closed: cualquier error sube al
/// llamador, que decide 503 o retry con backoff.
#[worker::send]
pub async fn call_do(env: worker::Env, repo: String, path_and_query: String) -> worker::Result<String> {
    let stub = env
        .durable_object(DO_BINDING)?
        .id_from_name(&repo)?
        .get_stub()?;
    let resp = stub
        .fetch_with_str(&format!("https://incident-state{}", path_and_query))
        .await?;
    resp.text().await
}
