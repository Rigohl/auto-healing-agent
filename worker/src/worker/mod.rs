//! Modulos del orquestador PART3 (runtime Cloudflare).
//!
//! El modulo se llama `runtime` y NO `worker`: ese nombre colisiona con
//! el crate externo `worker` y hace ambigua cada ruta `use worker::...`
//! (E0659, detectado por el job `worker` de CI en c5fadd48). El modulo se
//! mapea al directorio src/worker/ con #[path]; cada submodule lleva su
//! propio #[path] explicito para que la resolucion no dependa del nombre
//! del modulo.
//!
//! Regla de autoridad: Cloudflare ORCHESTRATES, PERSISTS, DEDUPLICATES,
//! QUEUES, LIMITS y OBSERVES. GitHub Actions es la autoridad de VERIFY;
//! este runtime nunca declara CI PASS, aprueba ni fusiona PRs.

#[path = "anti_loop.rs"]
pub mod anti_loop;
#[path = "incident_state.rs"]
pub mod incident_state;
#[path = "model.rs"]
pub mod model;
#[path = "monitor.rs"]
pub mod monitor;
#[path = "github_client.rs"]
pub mod github_client;
#[path = "queue_consumer.rs"]
pub mod queue_consumer;
#[path = "quota.rs"]
pub mod quota;
#[path = "rules.rs"]
pub mod rules;
#[path = "security.rs"]
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
pub async fn call_do(
    env: worker::Env,
    repo: String,
    path_and_query: String,
) -> worker::Result<String> {
    let stub = env
        .durable_object(DO_BINDING)?
        .id_from_name(&repo)?
        .get_stub()?;
    // Response::text() requiere &mut self (workers-rs 0.8): consumir el
    // cuerpo muta el Response. Sin `mut` esto no compila (E0596).
    let mut resp = stub
        .fetch_with_str(&format!("https://incident-state{}", path_and_query))
        .await?;
    resp.text().await
}
