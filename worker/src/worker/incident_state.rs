//! Durable Object (SQLite) con el estado de incidentes de reparacion.
//!
//! Gestiona: incident, repair_case (state), attempts, locks (un DO es
//! single-threaded: el lock es estructural), idempotency, quota contable,
//! anti_loop, verification y timestamps. Es la unica pieza transaccional
//! del runtime: TODA decision de dedup/quota/anti-loop pasa por aqui.
//!
//! Un objeto por repositorio (id_from_name(repo)): serializa el estado del
//! repo y hace triviales los contadores por repo (max_open_repairs,
//! max_repairs_per_repo, daily_budget). Cloudflare Free solo permite DO
//! SQLite: este esquema es SQLite-backed (ver wrangler.toml migrations).
//!
//! Los valores configurables NO viven aqui: llegan en cada llamada desde las
//! vars del Worker (separacion config/estado).
//!
//! Regla del API SQLite de DO: una query no se considera completa hasta que
//! su cursor se agota; un cursor abandonado puede CANCELAR la query. Todo
//! write pasa por exec_write, que consume el cursor siempre.

use serde::Deserialize;
use worker::*;

use crate::runtime::{
    anti_loop::{self, AntiLoopConfig, LoopSignals},
    now_ms,
    quota::{self, QuotaConfig, QuotaUsage, QuotaVerdict},
};

pub const STATE_QUEUED: &str = "queued";
pub const STATE_REPAIRING: &str = "repairing";
pub const STATE_DONE: &str = "done";
pub const STATE_BLOCKED: &str = "blocked";
pub const STATE_DEAD_LETTER: &str = "dead_letter";

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS incidents (
    id TEXT PRIMARY KEY,
    repository TEXT NOT NULL,
    signature TEXT NOT NULL,
    state TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    correlation_id TEXT NOT NULL,
    last_fingerprint TEXT,
    last_reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_incidents_repo_state ON incidents (repository, state);
CREATE INDEX IF NOT EXISTS idx_incidents_updated ON incidents (updated_at);
CREATE TABLE IF NOT EXISTS idempotency (
    key TEXT 
PRIMARY KEY,
    response TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS transitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    incident_id TEXT NOT NULL,
    from_state TEXT NOT NULL,
    to_state TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_transitions_incident ON transitions (incident_id);
CREATE TABLE IF NOT EXISTS verification (
    incident_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    evidence_ref TEXT,
    fingerprint TEXT,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS signature_events (
    signature TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_signature_events ON signature_events (signature, created_at);
CREATE TABLE IF NOT EXISTS fingerprints (
    fingerprint TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_fingerprints ON fingerprints (fingerprint, created_at);
CREATE TABLE IF NOT EXISTS repair_events (
    repository TEXT NOT NULL,
    day TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_repair_events ON repair_events (repository, day, created_at);
"#;

/// DO de estado por repositorio. Solo guarda el handle SQL (Send+Sync).
#[durable_object]
pub struct IncidentState {
    sql: SqlStorage,
}

/// Ejecuta una query sin filas esperadas (DDL/DML) y AGOTA el cursor: el
/// API SQLite de DO puede cancelar una query cuyo cursor no se consume, y
/// un INSERT "olvidado" no se garantiza que se complete.
fn exec_write(sql: &SqlStorage, query: &str, bindings: Vec<SqlStorageValue>) -> Result<()> {
    let cursor = sql.exec(query, bindings)?;
    let _rows = cursor.to_array::<serde_json::Value>()?;
    Ok(())
}

impl DurableObject for IncidentState {
    fn new(state: State, _env: Env) -> Self {
        let sql = state.storage().sql();
        // Igual que el ejemplo oficial de workers-rs: el esquema es
        // idempotente (IF NOT EXISTS) 
y se prepara en el constructor.
        for stmt in SCHEMA.split(';') {
            let stmt = stmt.trim();
            if !stmt.is_empty() {
                exec_write(&sql, stmt, Vec::new()).expect("incident_state schema");
            }
        }
        Self { sql }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let path = req.path();
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match parts.as_slice() {
            ["health"] => Response::ok("ok"),
            ["ingest"] => self.ingest(&req),
            ["attempt"] => self.attempt(&req),
            ["result"] => self.result(&req),
            ["poison"] => self.poison(&req),
            ["state"] => self.state(&req),
            _ => Response::error("not_found", 404),
        }
    }
}

// Todos los query params llegan como strings; se parsean explicitamente.
#[derive(Debug, Default, Deserialize)]
struct IngestQuery {
    #[serde(default)] repo: String,
    #[serde(default)] incident_id: String,
    #[serde(default)] signature: String,
    #[serde(default)] delivery_id: String,
    #[serde(default)] idem_key: String,
    #[serde(default)] correlation_id: String,
    #[serde(default)] max_attempts_per_incident: String,
    #[serde(default)] max_repairs_per_repo: String,
    #[serde(default)] max_open_repairs: String,
    #[serde(default)] cooldown_seconds: String,
    #[serde(default)] daily_budget: String,
    #[serde(default)] max_same_incident: String,
    #[serde(default)] max_same_signature: String,
    #[serde(default)] window_seconds: String,
}

#[derive(Debug, Default, Deserialize)]
struct AttemptQuery {
    #[serde(default)] correlation_id: String,
    #[serde(default)] incident_id: String,
    #[serde(default)] signature: String,
    #[serde(default)] max_attempts_per_incident: String,
    #[serde(default)] max_same_signature: String,
    #[serde(default)] window_seconds: String,
}

#[derive(Debug, Default, Deserialize)]
stru
ct ResultQuery {
    #[serde(default)] correlation_id: String,
    #[serde(default)] incident_id: String,
    #[serde(default)] decision: String,
    #[serde(default)] fingerprint: String,
    #[serde(default)] verify_status: String,
    #[serde(default)] evidence_ref: String,
    #[serde(default)] reason: String,
}

#[derive(Debug, Default, Deserialize)]
struct PoisonQuery {
    #[serde(default)] correlation_id: String,
    #[serde(default)] incident_id: String,
    #[serde(default)] queue: String,
}

#[derive(Debug, Default, Deserialize)]
struct StateQuery {
    #[serde(default)] incident_id: String,
    #[serde(default)] correlation_id: String,
}

#[derive(Debug, Deserialize)]
struct CountRow {
    count: i64,
}

#[derive(Debug, Deserialize)]
struct IncidentRow {
    #[serde(default)] id: String,
    #[serde(default)] attempts: i64,
}

impl IncidentState {
    fn count(&self, query: &str, bindings: Vec<SqlStorageValue>) -> Result<i64> {
        let row: CountRow = self.sql.exec(query, bindings)?.one()?;
        Ok(row.count)
    }

    fn insert_transition(
        &self,
        incident_id: &str,
        from: &str,
        to: &str,
        correlation_id: &str,
        now: i64,
    ) -> Result<()> {
        exec_write(
            &self.sql,
            "INSERT INTO transitions (incident_id, from_state, to_state, correlation_id, created_at) VALUES (?, ?, ?, ?, ?)",
            vec![
                SqlStorageValue::from(incident_id),
                SqlStorageValue::from(from),
                SqlStorageValue::from(to),
                SqlStorageValue::from(correlation_id),
                SqlStorageValue::from(now),
            ],
        )
    }

    fn store_idempotency(&self, key: &str, response: &str, now: i64) -> Result<()> {
        exec_write(
            &self.sql,
            "INSERT OR REPLACE INTO idempotency (key, response, created_at) VALUES (?, ?, ?)",
            vec![
                SqlStorageValue::from(key),
                SqlStorageValu
e::from(response),
                SqlStorageValue::from(now),
            ],
        )
    }

    fn blocked_response(
        &self,
        status: &str,
        reason: &str,
        incident_id: &str,
        correlation_id: &str,
    ) -> String {
        // La respuesta bloqueada TAMBIEN se guarda en idempotency: reintentar
        // la misma entrega devuelve la misma decision (no revivir un bloqueo).
        serde_json::json!({
            "status": status,
            "reason": reason,
            "incident_id": incident_id,
            "correlation_id": correlation_id,
        })
        .to_string()
    }

    /// POST /ingest: dedup + idempotencia + quota + anti-loop + alta.
    fn ingest(&self, req: &Request) -> Result<Response> {
        let q: IngestQuery = req.query()?;
        let now = now_ms();
        let defaults = QuotaConfig::default();
        let anti_defaults = AntiLoopConfig::default();
        let window_ms = q
            .window_seconds
            .parse::<i64>()
            .unwrap_or(anti_defaults.window_seconds)
            * 1000;

        // 1. Idempotencia: misma entrega -> misma respuesta sin reprocesar.
        let existing: Option<IdemRow> = self
            .sql
            .exec(
                "SELECT response FROM idempotency WHERE key = ?",
                vec![SqlStorageValue::from(q.idem_key.as_str())],
            )?
            .to_array()?
            .into_iter()
            .next();
        if let Some(row) = existing {
            return Response::ok(row.response);
        }

        // 2. Anti-loop (senales dentro de la ventana).
        let loop_signals = LoopSignals {
            same_incident_recent: self.count(
                "SELECT COUNT(*) AS count FROM incidents WHERE id = ? AND updated_at > ?",
                vec![
                    SqlStorageValue::from(q.incident_id.as_str()),
                    SqlStorageValue::from(now - window_ms),
                ],
            )? as u32,
            same_si
gnature_recent: self.count(
                "SELECT COUNT(*) AS count FROM signature_events WHERE signature = ? AND created_at > ?",
                vec![
                    SqlStorageValue::from(q.signature.as_str()),
                    SqlStorageValue::from(now - window_ms),
                ],
            )? as u32,
            same_fingerprint_recent: 0, // la huella aun no existe en ingest
            same_failing_verification_recent: 0,
        };
        let anti_cfg = AntiLoopConfig {
            max_same_incident: q.max_same_incident.parse().unwrap_or(anti_defaults.max_same_incident),
            max_same_signature: q.max_same_signature.parse().unwrap_or(anti_defaults.max_same_signature),
            max_same_fingerprint: anti_defaults.max_same_fingerprint,
            max_same_failing_verification: anti_defaults.max_same_failing_verification,
            window_seconds: q.window_seconds.parse().unwrap_or(anti_defaults.window_seconds),
        };
        if let anti_loop::LoopVerdict::Blocked(reason) = anti_loop::evaluate(&anti_cfg, &loop_signals) {
            let resp = self.blocked_response("blocked_anti_loop", reason, &q.incident_id, &q.correlation_id);
            self.store_idempotency(&q.idem_key, &resp, now)?;
            return Response::ok(resp);
        }

        // 3. Quota (estado contable de este repositorio).
        let cooldown_ms = q.cooldown_seconds.parse::<i64>().unwrap_or(defaults.cooldown_seconds) * 1000;
        let day = (now / 86_400_000).to_string();
        let usage = QuotaUsage {
            attempts_of_incident: self.count(
                "SELECT COUNT(*) AS count FROM incidents WHERE id = ?",
                vec![SqlStorageValue::from(q.incident_id.as_str())],
            )? as u32,
            repairs_in_window: self.count(
                "SELECT COUNT(*) AS count FROM repair_events WHERE repository = ? AND created_at > ?",
                vec![
                    SqlStorageValue::from(q.repo.as_str()),
                 
   SqlStorageValue::from(now - cooldown_ms),
                ],
            )? as u32,
            open_repairs: self.count(
                "SELECT COUNT(*) AS count FROM incidents WHERE repository = ? AND state IN ('queued', 'repairing')",
                vec![SqlStorageValue::from(q.repo.as_str())],
            )? as u32,
            daily_repairs: self.count(
                "SELECT COUNT(*) AS count FROM repair_events WHERE repository = ? AND day = ?",
                vec![
                    SqlStorageValue::from(q.repo.as_str()),
                    SqlStorageValue::from(day.as_str()),
                ],
            )? as u32,
        };
        let quota_cfg = QuotaConfig {
            max_attempts_per_incident: q
                .max_attempts_per_incident
                .parse()
                .unwrap_or(defaults.max_attempts_per_incident),
            max_repairs_per_repo: q.max_repairs_per_repo.parse().unwrap_or(defaults.max_repairs_per_repo),
            max_open_repairs: q.max_open_repairs.parse().unwrap_or(defaults.max_open_repairs),
            cooldown_seconds: q.cooldown_seconds.parse().unwrap_or(defaults.cooldown_seconds),
            daily_budget: q.daily_budget.parse().unwrap_or(defaults.daily_budget),
        };
        if let QuotaVerdict::Blocked(reason) = quota_cfg.evaluate(&usage) {
            let resp = self.blocked_response("blocked_quota", reason, &q.incident_id, &q.correlation_id);
            self.store_idempotency(&q.idem_key, &resp, now)?;
            return Response::ok(resp);
        }

        // 4. Alta (upsert) + auditoria.
        exec_write(
            &self.sql,
            "INSERT INTO incidents (id, repository, signature, state, attempts, correlation_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, 0, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET state = excluded.state, correlation_id = excluded.correlation_id, updated_at = excluded.updated_at",
            vec![
                SqlStorageValue::from(q
.incident_id.as_str()),
                SqlStorageValue::from(q.repo.as_str()),
                SqlStorageValue::from(q.signature.as_str()),
                SqlStorageValue::from(STATE_QUEUED),
                SqlStorageValue::from(q.correlation_id.as_str()),
                SqlStorageValue::from(now),
                SqlStorageValue::from(now),
            ],
        )?;
        exec_write(
            &self.sql,
            "INSERT INTO signature_events (signature, created_at) VALUES (?, ?)",
            vec![
                SqlStorageValue::from(q.signature.as_str()),
                SqlStorageValue::from(now),
            ],
        )?;
        self.insert_transition(&q.incident_id, "", STATE_QUEUED, &q.correlation_id, now)?;
        let resp = serde_json::json!({
            "status": "queued",
            "incident_id": q.incident_id,
            "correlation_id": q.correlation_id,
            "repo": q.repo,
        })
        .to_string();
        self.store_idempotency(&q.idem_key, &resp, now)?;
        Response::ok(resp)
    }

    /// POST /attempt: el consumidor pide permiso para un intento mas.
    fn attempt(&self, req: &Request) -> Result<Response> {
        let q: AttemptQuery = req.query()?;
        let now = now_ms();
        let defaults = QuotaConfig::default();
        let anti_defaults = AntiLoopConfig::default();

        let row: Option<IncidentRow> = self
            .sql
            .exec(
                "SELECT id, attempts FROM incidents WHERE correlation_id = ?",
                vec![SqlStorageValue::from(q.correlation_id.as_str())],
            )?
            .to_array()?
            .into_iter()
            .next();
        let attempts = row.map(|r| r.attempts).unwrap_or(0) + 1;
        let max_attempts = q
            .max_attempts_per_incident
            .parse()
            .unwrap_or(defaults.max_attempts_per_incident) as i64;
        if attempts > max_attempts {
            // Hard stop: registrar el bloqueo y responder denega
do.
            exec_write(
                &self.sql,
                "UPDATE incidents SET state = ?, updated_at = ? WHERE correlation_id = ?",
                vec![
                    SqlStorageValue::from(STATE_BLOCKED),
                    SqlStorageValue::from(now),
                    SqlStorageValue::from(q.correlation_id.as_str()),
                ],
            )?;
            self.insert_transition(&q.incident_id, STATE_REPAIRING, STATE_BLOCKED, &q.correlation_id, now)?;
            return Response::ok(
                serde_json::json!({
                    "allowed": false,
                    "reason": "max_attempts_per_incident",
                    "attempts": attempts,
                    "correlation_id": q.correlation_id,
                })
                .to_string(),
            );
        }

        let window_ms = q.window_seconds.parse::<i64>().unwrap_or(anti_defaults.window_seconds) * 1000;
        let same_signature_recent = self.count(
            "SELECT COUNT(*) AS count FROM signature_events WHERE signature = ? AND created_at > ?",
            vec![
                SqlStorageValue::from(q.signature.as_str()),
                SqlStorageValue::from(now - window_ms),
            ],
        )? as u32;
        let anti_cfg = AntiLoopConfig {
            max_same_signature: q.max_same_signature.parse().unwrap_or(anti_defaults.max_same_signature),
            ..anti_defaults
        };
        let signals = LoopSignals {
            same_signature_recent,
            ..LoopSignals::default()
        };
        if let anti_loop::LoopVerdict::Blocked(reason) = anti_loop::evaluate(&anti_cfg, &signals) {
            exec_write(
                &self.sql,
                "UPDATE incidents SET state = ?, last_reason = ?, updated_at = ? WHERE correlation_id = ?",
                vec![
                    SqlStorageValue::from(STATE_BLOCKED),
                    SqlStorageValue::from(reason),
                    SqlStorageValue::from(now),
         
           SqlStorageValue::from(q.correlation_id.as_str()),
                ],
            )?;
            self.insert_transition(&q.incident_id, STATE_REPAIRING, STATE_BLOCKED, &q.correlation_id, now)?;
            return Response::ok(
                serde_json::json!({
                    "allowed": false,
                    "reason": reason,
                    "attempts": attempts,
                    "correlation_id": q.correlation_id,
                })
                .to_string(),
            );
        }

        exec_write(
            &self.sql,
            "UPDATE incidents SET attempts = ?, state = ?, updated_at = ? WHERE correlation_id = ?",
            vec![
                SqlStorageValue::from(attempts),
                SqlStorageValue::from(STATE_REPAIRING),
                SqlStorageValue::from(now),
                SqlStorageValue::from(q.correlation_id.as_str()),
            ],
        )?;
        self.insert_transition(&q.incident_id, STATE_QUEUED, STATE_REPAIRING, &q.correlation_id, now)?;
        Response::ok(
            serde_json::json!({
                "allowed": true,
                "attempts": attempts,
                "correlation_id": q.correlation_id,
            })
            .to_string(),
        )
    }

    /// POST /result: decision final del intento + verificacion + huella.
    fn result(&self, req: &Request) -> Result<Response> {
        let q: ResultQuery = req.query()?;
        let now = now_ms();
        let final_state = if q.decision == "allow" { STATE_DONE } else { STATE_BLOCKED };
        exec_write(
            &self.sql,
            "UPDATE incidents SET state = ?, last_fingerprint = ?, last_reason = ?, updated_at = ? WHERE correlation_id = ?",
            vec![
                SqlStorageValue::from(final_state),
                SqlStorageValue::from(q.fingerprint.as_str()),
                SqlStorageValue::from(q.reason.as_str()),
                SqlStorageValue::from(now),
                SqlStorageValue::from
(q.correlation_id.as_str()),
            ],
        )?;
        exec_write(
            &self.sql,
            "INSERT INTO verification (incident_id, status, evidence_ref, fingerprint, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(incident_id) DO UPDATE SET status = excluded.status, evidence_ref = excluded.evidence_ref, fingerprint = excluded.fingerprint, created_at = excluded.created_at",
            vec![
                SqlStorageValue::from(q.incident_id.as_str()),
                SqlStorageValue::from(q.verify_status.as_str()),
                SqlStorageValue::from(q.evidence_ref.as_str()),
                SqlStorageValue::from(q.fingerprint.as_str()),
                SqlStorageValue::from(now),
            ],
        )?;
        if !q.fingerprint.is_empty() {
            exec_write(
                &self.sql,
                "INSERT INTO fingerprints (fingerprint, created_at) VALUES (?, ?)",
                vec![
                    SqlStorageValue::from(q.fingerprint.as_str()),
                    SqlStorageValue::from(now),
                ],
            )?;
        }
        if q.decision == "allow" {
            // Solo un intento autorizado consume presupuesto de reparacion.
            let day = (now / 86_400_000).to_string();
            let repo: String = self
                .sql
                .exec(
                    "SELECT repository FROM incidents WHERE correlation_id = ?",
                    vec![SqlStorageValue::from(q.correlation_id.as_str())],
                )?
                .to_array::<RepoRow>()?
                .into_iter()
                .next()
                .map(|r| r.repository)
                .unwrap_or_default();
            if !repo.is_empty() {
                exec_write(
                    &self.sql,
                    "INSERT INTO repair_events (repository, day, created_at) VALUES (?, ?, ?)",
                    vec![
                        SqlStorageValue::from(repo.as_str()),
          
              SqlStorageValue::from(day.as_str()),
                        SqlStorageValue::from(now),
                    ],
                )?;
            }
        }
        self.insert_transition(&q.incident_id, STATE_REPAIRING, final_state, &q.correlation_id, now)?;
        Response::ok(serde_json::json!({ "ok": true }).to_string())
    }

    /// POST /poison: mensaje caido en DLQ.
    fn poison(&self, req: &Request) -> Result<Response> {
        let q: PoisonQuery = req.query()?;
        let now = now_ms();
        exec_write(
            &self.sql,
            "UPDATE incidents SET state = ?, last_reason = ?, updated_at = ? WHERE correlation_id = ?",
            vec![
                SqlStorageValue::from(STATE_DEAD_LETTER),
                SqlStorageValue::from(format!("dlq:{}", q.queue)),
                SqlStorageValue::from(now),
                SqlStorageValue::from(q.correlation_id.as_str()),
            ],
        )?;
        exec_write(
            &self.sql,
            "INSERT INTO verification (incident_id, status, evidence_ref, created_at)
             VALUES (?, 'poison', ?, ?)
             ON CONFLICT(incident_id) DO UPDATE SET status = 'poison', evidence_ref = excluded.evidence_ref, created_at = excluded.created_at",
            vec![
                SqlStorageValue::from(q.incident_id.as_str()),
                SqlStorageValue::from(q.queue.as_str()),
                SqlStorageValue::from(now),
            ],
        )?;
        self.insert_transition(&q.incident_id, STATE_REPAIRING, STATE_DEAD_LETTER, &q.correlation_id, now)?;
        Response::ok(serde_json::json!({ "ok": true }).to_string())
    }

    /// GET /state: observabilidad del incidente.
    fn state(&self, req: &Request) -> Result<Response> {
        let q: StateQuery = req.query()?;
        let where_clause = if q.correlation_id.is_empty() { "id = ?" } else { "correlation_id = ?" };
        let value = if q.correlation_id.is_empty() { q.incident_id.clone() } else { q.correlati
on_id.clone() };
        let incidents: Vec<serde_json::Value> = self
            .sql
            .exec(
                &format!("SELECT id, repository, signature, state, attempts, correlation_id, last_fingerprint, last_reason, created_at, updated_at FROM incidents WHERE {}", where_clause),
                vec![SqlStorageValue::from(value.as_str())],
            )?
            .to_array()?;
        let transitions: Vec<serde_json::Value> = self
            .sql
            .exec(
                "SELECT from_state, to_state, correlation_id, created_at FROM transitions WHERE incident_id = ? ORDER BY id",
                vec![SqlStorageValue::from(q.incident_id.as_str())],
            )?
            .to_array()?;
        let verification: Vec<serde_json::Value> = self
            .sql
            .exec(
                "SELECT status, evidence_ref, created_at FROM verification WHERE incident_id = ?",
                vec![SqlStorageValue::from(q.incident_id.as_str())],
            )?
            .to_array()?;
        Response::ok(
            serde_json::json!({
                "incident": incidents,
                "transitions": transitions,
                "verification": verification,
            })
            .to_string(),
        )
    }
}

#[derive(Debug, Deserialize)]
struct IdemRow {
    response: String,
}

#[derive(Debug, Deserialize)]
struct RepoRow {
    repository: String,
}
