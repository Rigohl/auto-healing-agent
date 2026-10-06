//! Consumidor de la cola de reparaciones.
//!
//! Reglas PART3: retry acotado (nunca infinito: max_retries=3 de la cola ->
//! DLQ), deteccion de veneno en el consumidor de la DLQ, correlation_id en
//! cada mensaje e idempotencia via el Durable Object (la puerta /attempt
//! decide si el intento procede; un duplicado nunca consume quota dos veces).

use serde::{Deserialize, Serialize};
use worker::*;

use feature_engine::extract;
use repair_operators::{diff, gate};
use repair_types::{
    FailureSignature, Incident, OperatorId, RepairAction, RepairCase, VerificationResult,
};

use crate::runtime::{
    anti_loop::AntiLoopConfig,
    model, param_derive,
    quota::QuotaConfig,
    rules,
    security::{fnv1a64, urlencode},
    MAX_RISK, MIN_CONFIDENCE,
};

pub const QUEUE_BINDING: &str = "REPAIR_QUEUE";
pub const QUEUE_PROD: &str = "auto-healing-repairs";
pub const QUEUE_STAGING: &str = "auto-healing-repairs-staging";
pub const DLQ_PROD: &str = "auto-healing-repairs-dlq";
pub const DLQ_STAGING: &str = "auto-healing-repairs-dlq-staging";

/// Binding KV de RepairCases (PASO 3; ver worker/wrangler.toml).
pub const REPAIR_CASES_KV: &str = "REPAIR_CASES_KV";

/// Delay fijo de reintento. Antes habia un "contador propio" (MAX_QUEUE_ATTEMPTS)
/// que era inalcanzable: Cloudflare reentrega el MISMO body al reintentar, asi
/// que el contador del mensaje nunca avanzaba y la rama nunca se ejecutaba
/// (dead code). El hard stop autoritativo de intentos es el DO (`/attempt`,
/// max_attempts_per_incident) y el tope de reintentos es `max_retries=3` de la
/// cola (trascendido, el mensaje cae a la DLQ). Nunca retry infinito.
const RETRY_DELAY_SECONDS: u32 = 10;

/// Cuerpo entrante del webhook. Todo opcional: un payload parcial nunca
/// rompe el isolate.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub error_code: String,
    #[serde(default)]
    pub error_step: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub stack_hint: String,
    #[serde(default)]
    pub language_hint: String,
    #[serde(default)]
    pub framework_hint: String,
}

impl WebhookPayload {
    pub fn incident_id(&self) -> String {
        if self.id.is_empty() {
            "unidentified".to_string()
        } else {
            self.id.clone()
        }
    }

    pub fn repo(&self) -> String {
        if self.project.is_empty() {
            "default".to_string()
        } else {
            self.project.clone()
        }
    }

    /// Firma canonica para dedup/anti-loop (determinista, sin reloj).
    pub fn signature_str(&self) -> String {
        format!("{}|{}|{}", self.error_code, self.error_step, self.command)
    }

    pub fn to_incident(&self) -> Incident {
        Incident {
            id: self.incident_id(),
            source: if self.source.is_empty() {
                "webhook".to_string()
            } else {
                self.source.clone()
            },
            error_code: self.error_code.clone(),
            error_step: self.error_step.clone(),
            command: self.command.clone(),
            message: self.message.clone(),
            project: self.project.clone(),
            attempts: self.attempts,
            stack_hint: self.stack_hint.clone(),
            language_hint: self.language_hint.clone(),
            framework_hint: self.framework_hint.clone(),
            verified: false,
            status: "open".to_string(),
        }
    }
}

/// Mensaje de cola: correlation_id SIEMPRE presente para trazabilidad.
///
/// Sin campo `attempts`: el body del mensaje es inmutable al reintentar,
/// asi que un contador aqui no sobreviviria a los redeliveries. El numero de
/// intentos REALES vive en el DO (columna `incidents.attempts`, la incrementa
/// `/attempt`); ver `RETRY_DELAY_SECONDS`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueTask {
    pub correlation_id: String,
    pub incident_id: String,
    pub repo: String,
    pub signature: String,
    pub payload: WebhookPayload,
    pub enqueued_at: i64,
}

/// Punto de entrada del evento de cola. Un solo handler para produccion,
/// staging y DLQ: se despacha por nombre de cola.
#[worker::send]
pub async fn consume(batch: MessageBatch<QueueTask>, env: Env) -> Result<()> {
    let queue_name = batch.queue();
    if queue_name == DLQ_PROD || queue_name == DLQ_STAGING {
        // DLQ: registrar el veneno en el DO (best-effort) y asentar. El
        // nombre de la cola REAL va al DO: un veneno de staging no se
        // registra como produccion.
        for message in batch.messages()? {
            let task = message.body().clone();
            if let Err(e) = record_poison(env.clone(), task, &queue_name).await {
                console_error!("dlq record failed: {}", e);
            }
        }
        batch.ack_all();
        return Ok(());
    }
    if queue_name != QUEUE_PROD && queue_name != QUEUE_STAGING {
        console_error!("unknown queue: {}", queue_name);
        return Ok(());
    }

    let quota_cfg = QuotaConfig::from_env(&env);
    let anti_cfg = AntiLoopConfig::from_env(&env);
    for message in batch.messages()? {
        let task = message.body().clone();
        match process(env.clone(), task.clone(), quota_cfg, anti_cfg).await {
            Ok(()) => {
                message.ack();
            }
            Err(err) => {
                // Fallo transitorio (DO o KV no disponible). Se reintenta con
                // delay fijo; el TOPE es max_retries=3 de la cola, y ahi el
                // mensaje cae a la DLQ (su consumidor lo registra como
                // veneno). El corte autoritativo por numero de intentos es el
                // DO (/attempt), no este consumidor.
                console_warn!(
                    "retry correlation_id={} en {}s: {}",
                    task.correlation_id,
                    RETRY_DELAY_SECONDS,
                    err
                );
                message.retry_with_options(
                    &QueueRetryOptionsBuilder::new()
                        .with_delay_seconds(RETRY_DELAY_SECONDS)
                        .build(),
                );
            }
        }
    }
    Ok(())
}

/// Procesa un intento: puerta DO -> pipeline -> registro de resultado.
#[worker::send]
async fn process(
    env: Env,
    task: QueueTask,
    quota_cfg: QuotaConfig,
    anti_cfg: AntiLoopConfig,
) -> Result<()> {
    // 1. Puerta: quota + anti-loop (las 4 señales) + contador de attempts en el DO.
    let qs = format!(
        "/attempt?correlation_id={}&incident_id={}&signature={}&max_attempts_per_incident={}&max_same_signature={}&max_same_fingerprint={}&max_same_failing_verification={}&window_seconds={}",
        urlencode(&task.correlation_id),
        urlencode(&task.incident_id),
        urlencode(&task.signature),
        quota_cfg.max_attempts_per_incident,
        anti_cfg.max_same_signature,
        anti_cfg.max_same_fingerprint,
        anti_cfg.max_same_failing_verification,
        anti_cfg.window_seconds
    );
    let text = crate::runtime::call_do(env.clone(), task.repo.clone(), qs).await?;
    let verdict: serde_json::Value = serde_json::from_str(&text)?;
    if !verdict
        .get("allowed")
        .and_then(|a| a.as_bool())
        .unwrap_or(false)
    {
        // Bloqueado por quota/anti-loop/hard-stop (ya registrado en el DO).
        return Ok(());
    }

    // 2. Pipeline determinista: Incident -> features -> NN -> gate.
    let incident = task.payload.to_incident();
    let signature = FailureSignature::from_incident(&incident);
    let features = extract(&incident, &signature);

    // 2.5 Reglas declarativas (P1): filtrado adicional fail-closed.
    // REPAIR_RULES invalido => bloqueado (nunca dejar pasar por defecto);
    // sin var o "[]" => sin reglas (no-op). Esquema y sintaxis: rules.rs.
    let ruleset = match rules::load(&env) {
        Ok(r) => r,
        Err(e) => {
            console_error!("rules config invalid: {}", e);
            let rqs = blocked_result_qs(&task, "blocked_rules", "rules_invalid_config");
            crate::runtime::call_do(env, task.repo.clone(), rqs).await?;
            return Ok(());
        }
    };

    let loaded = match model::load(&env).await {
        Ok(m) => m,
        Err(_) => {
            // current y stable ausentes o malformados: BLOCKED. Nunca zeros.
            let rqs = blocked_result_qs(&task, "blocked_model", "no_model_current_stable");
            crate::runtime::call_do(env, task.repo.clone(), rqs).await?;
            return Ok(());
        }
    };
    let action = loaded.net.predict(&features);
    let gate_ok = gate(&action, MIN_CONFIDENCE, MAX_RISK).is_ok();

    // 2.6 Evaluacion declarativa: las reglas solo restringen (block) u
    // observan (observe); NUNCA permiten saltarse el gate determinista
    // (autoridad: repair_operators::gate + VERIFY en GitHub Actions).
    let rule_ctx = rules::RuleContext::from_incident_and_action(
        &incident,
        &task.repo,
        &task.signature,
        action.repair_operator.as_str(),
        action.confidence,
        action.risk,
    );
    match ruleset.evaluate(&rule_ctx) {
        Ok((Some(rule_id), observed)) => {
            if !observed.is_empty() {
                console_log!("rules observed: {}", observed.join(","));
            }
            console_warn!("rule blocked: {}", rule_id);
            let reason = format!("rule:{}", rule_id);
            let rqs = blocked_result_qs(&task, "blocked_by_rule", &reason);
            crate::runtime::call_do(env, task.repo, rqs).await?;
            return Ok(());
        }
        Ok((None, observed)) => {
            if !observed.is_empty() {
                console_log!("rules observed: {}", observed.join(","));
            }
        }
        Err(e) => {
            // No deberia ocurrir (compile valida campos y tipos); el
            // path sigue fail-closed: nunca unwrap().
            console_error!("rules eval error: {}", e);
            let rqs = blocked_result_qs(&task, "blocked_rules", "rules_eval_error");
            crate::runtime::call_do(env, task.repo, rqs).await?;
            return Ok(());
        }
    }

    let fingerprint = format!(
        "{:x}",
        fnv1a64(format!("{}|{}", action.repair_operator as u8, task.signature).as_bytes())
    );

    // 3. PASO 2+3: reparacion real cuando el gate permite. Edit acotado ->
    //    rama -> commit -> PR abierto -> RepairCase en KV. VERIFY nunca
    //    se declara aqui: GitHub Actions reporta a /github/callback.
    let (decision, verify_status, evidence_ref, reason) = if gate_ok {
        match attempt_repair(&env, &task, &action, &incident).await? {
            RepairOutcome::Repaired { pr_url } => ("allow", "pending_ci", pr_url, String::new()),
            RepairOutcome::Blocked { reason } => {
                ("blocked_by_policy", "blocked", String::new(), reason)
            }
        }
    } else {
        (
            "blocked_by_policy",
            "blocked",
            String::new(),
            String::from("gate_denied"),
        )
    };

    // 4. Registrar decision + verificacion en el DO.
    let rqs = format!(
        "/result?correlation_id={}&incident_id={}&decision={}&fingerprint={}&verify_status={}&evidence_ref={}&reason={}",
        urlencode(&task.correlation_id),
        urlencode(&task.incident_id),
        decision,
        urlencode(&fingerprint),
        verify_status,
        urlencode(&evidence_ref),
        urlencode(&reason)
    );
    crate::runtime::call_do(env, task.repo, rqs).await?;
    Ok(())
}

fn blocked_result_qs(task: &QueueTask, decision: &str, reason: &str) -> String {
    format!(
        "/result?correlation_id={}&incident_id={}&decision={}&fingerprint=&verify_status=blocked&evidence_ref=&reason={}",
        urlencode(&task.correlation_id),
        urlencode(&task.incident_id),
        decision,
        urlencode(reason)
    )
}

#[worker::send]
async fn record_poison(env: Env, task: QueueTask, queue_name: &str) -> Result<()> {
    let qs = format!(
        "/poison?correlation_id={}&incident_id={}&queue={}",
        urlencode(&task.correlation_id),
        urlencode(&task.incident_id),
        urlencode(queue_name)
    );
    crate::runtime::call_do(env, task.repo, qs)
        .await
        .map(|_| ())
}

/// Resultado de un intento de reparacion (PASO 2+3 del roadmap).
enum RepairOutcome {
    /// PR abierto; VERIFY queda pending_ci hasta el callback de Actions.
    Repaired { pr_url: String },
    /// Fail-closed permanente: bloqueado con razon, sin retry.
    Blocked { reason: String },
}

/// PASO 2 (GitHub API) + PASO 3 (KV) del roadmap: edit acotado -> rama ->
/// commit -> PR -> RepairCase en KV. Los errores transitorios suben como
/// Err para que la cola reintente (max_retries=3 -> DLQ); los permanentes
/// se reportan como Blocked (fail-closed, nunca retry infinito).
#[worker::send]
async fn attempt_repair(
    env: &worker::Env,
    task: &QueueTask,
    action: &RepairAction,
    incident: &Incident,
) -> worker::Result<RepairOutcome> {
    use crate::runtime::github_client::{GitHubClient, GitHubError};

    // Fail-closed: sin token no hay camino de escritura.
    let client = match GitHubClient::from_env(env) {
        Ok(c) => c.for_repo(&task.repo),
        Err(_) => {
            return Ok(RepairOutcome::Blocked {
                reason: String::from("github_token_not_configured"),
            })
        }
    };

    // Target file del edit acotado (paso 1): package.json para operadores de
    // dependencias; el resto lo nombra el parametro `file` de la accion.
    let target_path = match action.repair_operator {
        OperatorId::DependencyRepair | OperatorId::VersionPin => String::from("package.json"),
        _ => match action.parameters.get("file") {
            Some(f) if !f.is_empty() => f.clone(),
            _ => {
                return Ok(RepairOutcome::Blocked {
                    reason: String::from("no_bounded_target_file"),
                })
            }
        },
    };

    // La NN clasifica el OPERADOR pero V0 no emite parametros (no tiene
    // cabezas para ellos): sin derivarlos, todo intento con el gate en
    // verde terminaba bloqueado con missing_param aunque el diagnostico
    // fuera correcto (auditoria de codigo 2026-10-05). Derivacion textual
    // acotada y determinista (param_derive); lo no derivable sigue
    // bloqueado (fail-closed: nunca se inventa un valor).
    let mut action = action.clone();
    param_derive::ensure_params(&mut action, incident);
    // Contenido actual (todo el I/O vive aqui, no en el crate no_std).
    let before = match client.get_file(&target_path).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Ok(RepairOutcome::Blocked {
                reason: format!("file_unavailable:{}", target_path),
            })
        }
        Err(GitHubError::Transient(e)) => return Err(e),
        Err(GitHubError::Permanent(reason)) => return Ok(RepairOutcome::Blocked { reason }),
    };

    // Edit acotado + diff unificado (crates/repair_operators/src/diff.rs).
    let edit = match diff::generate_edit(&action, incident, |p: &str| {
        if p == target_path {
            Some(before.clone())
        } else {
            None
        }
    }) {
        Ok(e) => e,
        Err(e) => {
            return Ok(RepairOutcome::Blocked {
                reason: diff_blocked_reason(&e),
            })
        }
    };
    let patch = diff::unified_diff(&edit);
    if patch.is_empty() {
        return Ok(RepairOutcome::Blocked {
            reason: String::from("no_textual_change"),
        });
    }

    let title = format!(
        "auto-heal: {} [{}]",
        action.repair_operator.as_str(),
        task.incident_id
    );
    let commit_message = format!(
        "auto-heal: {} ({})",
        action.repair_operator.as_str(),
        task.correlation_id
    );
    let pr_body = format!(
        "## Auto-repair (bounded, deterministic)\n\n- operator: `{}`\n- correlation_id: `{}`\n- incident: `{}`\n- confidence: `{:.3}` risk: `{:.3}`\n\n```diff\n{}\n```\n\nVERIFY authority = GitHub Actions. CI results post to `/github/callback`. This PR is never auto-approved or auto-merged.",
        action.repair_operator.as_str(),
        task.correlation_id,
        task.incident_id,
        action.confidence,
        action.risk,
        patch
    );

    let pr_url = match client
        .open_repair_pr(
            &task.correlation_id,
            &edit.path,
            &edit.after,
            &title,
            &pr_body,
            &commit_message,
        )
        .await
    {
        Ok(u) => u,
        Err(GitHubError::Transient(e)) => return Err(e),
        Err(GitHubError::Permanent(reason)) => return Ok(RepairOutcome::Blocked { reason }),
    };

    // PASO 3: RepairCase persistible en KV (alternativa sin Mongo).
    persist_case(env, task, incident, &action, &pr_url).await;

    console_log!("repair pr opened: {} {}", task.correlation_id, pr_url);
    Ok(RepairOutcome::Repaired { pr_url })
}

/// Razon estable (auditable en el DO) por cada DiffError.
fn diff_blocked_reason(e: &diff::DiffError) -> String {
    match e {
        diff::DiffError::UnsupportedOperator(_) => String::from("operator_never_emits_diff"),
        diff::DiffError::MissingParam(k) => format!("missing_param:{}", k.replace(' ', "_")),
        diff::DiffError::FileUnavailable(_) => String::from("file_unavailable"),
        diff::DiffError::FileTooLarge(_) => String::from("file_too_large"),
        diff::DiffError::PatternNotFound(_) => String::from("pattern_not_found"),
        diff::DiffError::MajorBump { .. } => String::from("major_bump_refused"),
    }
}

/// PASO 3: RepairCase en REPAIR_CASES_KV, clave repair_case:{correlation_id}.
/// Best-effort: un fallo de KV no revierte la reparacion (el DO ya registro
/// decision + verificacion); solo se loguea.
#[worker::send]
async fn persist_case(
    env: &worker::Env,
    task: &QueueTask,
    incident: &Incident,
    action: &RepairAction,
    pr_url: &str,
) {
    let case = RepairCase {
        incident_id: task.incident_id.clone(),
        signature: FailureSignature::from_incident(incident),
        action: action.clone(),
        // Pendiente de VERIFY: Skipped = aun sin verificacion (PASS/FAIL
        // llega por /github/callback; el reward se computa al verificar).
        verification: VerificationResult::Skipped,
        patch_summary: format!("{} pr={}", action.repair_operator.as_str(), pr_url),
        pr_url: Some(pr_url.to_string()),
        reward: 0.0,
        created_at_unix: crate::runtime::now_ms() as u64,
    };
    let key = format!("repair_case:{}", task.correlation_id);
    let Ok(serialized) = serde_json::to_string(&case) else {
        console_error!("repair_case serialize failed: {}", task.correlation_id);
        return;
    };
    match env.kv(REPAIR_CASES_KV) {
        Ok(kv) => match kv.put(&key, serialized) {
            Ok(builder) => {
                if let Err(e) = builder.execute().await {
                    console_error!("repair_case kv put failed: {}", e);
                } else {
                    console_log!("repair_case stored: {}", key);
                }
            }
            Err(e) => console_error!("repair_case kv builder failed: {}", e),
        },
        Err(e) => console_error!("REPAIR_CASES_KV unavailable: {}", e),
    }
}
