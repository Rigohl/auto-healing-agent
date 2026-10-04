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
//!
//! Regla de lectura: `cursor.one()` LANZA si el resultado tiene cero filas
//! (docs de Durable Objects), asi que solo se usa donde hay exactamente una
//! fila garantizada (`COUNT(*)`). Una lectura que puede no encontrar fila
//! pasa por `one_opt`.

use serde::Deserialize;
use worker::*;

// TTL canonica del contrato (CONTRACT.md §5): la declaracion vive en
// repair_types, no en una constante suelta.
use repair_types::IDEMPOTENCY_TTL_SECONDS;

use crate::runtime::{
    anti_loop::{self, AntiLoopConfig, LoopSignals},
    now_ms,
    // Sin `self`: este archivo usa QuotaConfig/QuotaUsage/QuotaVerdict por
    // nombre, nunca `quota::algo`. El `self` sin usar era warning, y
    // `worker-clippy` corre con -D warnings (item 51 de DISCREPANCIES).
    quota::{QuotaConfig, QuotaUsage, QuotaVerdict},
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
    key TEXT PRIMARY KEY,
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
        // idempotente (IF NOT EXISTS) y se prepara en el constructor.
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
    #[serde(default)]
    repo: String,
    #[serde(default)]
    incident_id: String,
    #[serde(default)]
    signature: String,
    // Sin `delivery_id`: la deduplicacion va por `idem_key`, que el Worker
    // calcula con `repair_types::compute_idempotency_key` (FNV-1a
    // repo|incident|delivery|fingerprint, CONTRACT.md §5). Reenviarlo seria
    // mandar un parametro que nadie lee.
    #[serde(default)]
    idem_key: String,
    #[serde(default)]
    correlation_id: String,
    #[serde(default)]
    max_attempts_per_incident: String,
    #[serde(default)]
    max_repairs_per_repo: String,
    #[serde(default)]
    max_open_repairs: String,
    #[serde(default)]
    cooldown_seconds: String,
    #[serde(default)]
    daily_budget: String,
    #[serde(default)]
    max_same_incident: String,
    #[serde(default)]
    max_same_signature: String,
    #[serde(default)]
    max_same_fingerprint: String,
    #[serde(default)]
    max_same_failing_verification: String,
    #[serde(default)]
    window_seconds: String,
}

#[derive(Debug, Default, Deserialize)]
struct AttemptQuery {
    #[serde(default)]
    correlation_id: String,
    #[serde(default)]
    incident_id: String,
    #[serde(default)]
    signature: String,
    #[serde(default)]
    max_attempts_per_incident: String,
    #[serde(default)]
    max_same_signature: String,
    #[serde(default)]
    max_same_fingerprint: String,
    #[serde(default)]
    max_same_failing_verification: String,
    #[serde(default)]
    window_seconds: String,
}

#[derive(Debug, Default, Deserialize)]
struct ResultQuery {
    #[serde(default)]
    correlation_id: String,
    #[serde(default)]
    incident_id: String,
    #[serde(default)]
    decision: String,
    #[serde(default)]
    fingerprint: String,
    #[serde(default)]
    verify_status: String,
    #[serde(default)]
    evidence_ref: String,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Default, Deserialize)]
struct PoisonQuery {
    #[serde(default)]
    correlation_id: String,
    #[serde(default)]
    incident_id: String,
    #[serde(default)]
    queue: String,
}

#[derive(Debug, Default, Deserialize)]
struct StateQuery {
    #[serde(default)]
    incident_id: String,
    #[serde(default)]
    correlation_id: String,
}

#[derive(Debug, Deserialize)]
struct CountRow {
    count: i64,
}

/// Fila del incidente vista por `/attempt`: intentos REALES (columna
/// `attempts`, la incrementa el propio `/attempt`), estado actual (para
/// registrar la transicion de auditoria con su `from_state` verdadero) y la
/// ultima huella de parche (senal anti-loop). `default` en todos: ni una
/// columna ausente ni una fila parcial rompen el parseo. La columna
/// `last_fingerprint` es NULL hasta el primer `/result`: el SELECT la envuelve
/// en COALESCE porque `default` cubre campos ausentes, no valores `null`.
#[derive(Debug, Deserialize)]
struct IncidentRow {
    #[serde(default)]
    attempts: i64,
    #[serde(default)]
    state: String,
    #[serde(default)]
    last_fingerprint: String,
}

/// Estado previo del incidente en `/ingest` (una sola query): intentos desde
/// la columna `attempts` (antes se usaba `COUNT(*) WHERE id = ?`, que es
/// siempre 0 o 1 porque `id` es PK: no media ni intentos ni eventos) y la
/// ultima huella de parche (senal anti-loop de "misma huella repetida").
/// Misma regla de COALESCE que `IncidentRow`.
#[derive(Debug, Deserialize)]
struct IncidentPriorRow {
    #[serde(default)]
    attempts: i64,
    #[serde(default)]
    last_fingerprint: String,
}

/// Estado actual del incidente, usado como `from_state` real en las
/// transiciones de auditoria (antes venia hardcodeado a `repairing` incluso
/// cuando el incidente estaba en `queued`).
#[derive(Debug, Deserialize)]
struct StateRow {
    #[serde(default)]
    state: String,
}

impl IncidentState {
    fn count(&self, query: &str, bindings: Vec<SqlStorageValue>) -> Result<i64> {
        let row: CountRow = self.sql.exec(query, bindings)?.one()?;
        Ok(row.count)
    }

    /// Lee CERO o UNA fila. `cursor.one()` lanza con cero filas, asi que una
    /// busqueda que puede no encontrar nada (idempotencia, incidente previo,
    /// guard de correlacion) usa `to_array` y toma el primer elemento.
    fn one_opt<T>(&self, query: &str, bindings: Vec<SqlStorageValue>) -> Result<Option<T>>
    where
        T: for<'a> Deserialize<'a>,
    {
        Ok(self
            .sql
            .exec(query, bindings)?
            .to_array::<T>()?
            .into_iter()
            .next())
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
                SqlStorageValue::from(response),
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

    /// POST /ingest: retencion -> idempotencia (con TTL) -> anti-loop (4
    /// señales reales) -> quota -> alta.
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

        // 0. Retencion: sin ella el DO creceria sin bound (1 GB por objeto).
        //    idempotency: TTL del contrato (CONTRACT.md §5, 24 h).
        //    eventos: solo alimentan ventanas de 15 min (anti-loop), 1 h
        //    (cooldown) y 1 dia (daily_budget); 7 dias es margen amplio.
        //    `transitions` es auditoria: no se purga.
        let ttl_ms = IDEMPOTENCY_TTL_SECONDS as i64 * 1000;
        let retention_ms = 7 * 86_400_000;
        exec_write(
            &self.sql,
            "DELETE FROM idempotency WHERE created_at < ?",
            vec![SqlStorageValue::from(now - ttl_ms)],
        )?;
        for table in ["signature_events", "fingerprints", "repair_events"] {
            exec_write(
                &self.sql,
                &format!("DELETE FROM {table} WHERE created_at < ?"),
                vec![SqlStorageValue::from(now - retention_ms)],
            )?;
        }

        // 1. Idempotencia: misma entrega -> misma respuesta sin reprocesar.
        //    Con TTL: una decision guardada expira a las 24 h (CONTRACT.md §5)
        //    y la entrega se reprocesa; un bloqueo eterno no es idempotencia.
        let existing: Option<IdemRow> = self.one_opt(
            "SELECT response, created_at FROM idempotency WHERE key = ?",
            vec![SqlStorageValue::from(q.idem_key.as_str())],
        )?;
        if let Some(row) = existing {
            if row.created_at + ttl_ms > now {
                return Response::ok(row.response);
            }
        }

        // 2. Estado previo del incidente (una sola query): intentos REALES y
        //    ultima huella de parche. Antes el "intentos" venia de COUNT(*)
        //    sobre la PK (0 o 1: jamas bloqueaba) y las senales de huella y
        //    verificacion estaban hardcodeadas a 0 (config muerta).
        let prior: Option<IncidentPriorRow> = self.one_opt(
            "SELECT attempts, COALESCE(last_fingerprint, '') AS last_fingerprint FROM incidents WHERE id = ?",
            vec![SqlStorageValue::from(q.incident_id.as_str())],
        )?;
        let same_fingerprint_recent = match &prior {
            Some(p) if !p.last_fingerprint.is_empty() => self.count(
                "SELECT COUNT(*) AS count FROM fingerprints WHERE fingerprint = ? AND created_at > ?",
                vec![
                    SqlStorageValue::from(p.last_fingerprint.as_str()),
                    SqlStorageValue::from(now - window_ms),
                ],
            )? as u32,
            _ => 0,
        };

        // 3. Anti-loop: las 4 senales, todas reales, dentro de la ventana.
        let loop_signals = LoopSignals {
            // Este incidente ya tuvo N transiciones en la ventana (reaparicion
            // activa). Antes contaba filas de `incidents` (PK: tope 1) y el
            // limite (default 3) era inalcanzable.
            same_incident_recent: self.count(
                "SELECT COUNT(*) AS count FROM transitions WHERE incident_id = ? AND created_at > ?",
                vec![
                    SqlStorageValue::from(q.incident_id.as_str()),
                    SqlStorageValue::from(now - window_ms),
                ],
            )? as u32,
            same_signature_recent: self.count(
                "SELECT COUNT(*) AS count FROM signature_events WHERE signature = ? AND created_at > ?",
                vec![
                    SqlStorageValue::from(q.signature.as_str()),
                    SqlStorageValue::from(now - window_ms),
                ],
            )? as u32,
            // La huella de ESTA entrega no existe todavia (el pipeline corre
            // despues): se mide la ultima huella del incidente. Si el mismo
            // parche ya se propuso N veces en la ventana, es un bucle.
            same_fingerprint_recent,
            same_failing_verification_recent: self.count(
                "SELECT COUNT(*) AS count FROM verification WHERE incident_id = ? AND status IN ('blocked', 'poison') AND created_at > ?",
                vec![
                    SqlStorageValue::from(q.incident_id.as_str()),
                    SqlStorageValue::from(now - window_ms),
                ],
            )? as u32,
        };
        let anti_cfg = AntiLoopConfig {
            max_same_incident: q
                .max_same_incident
                .parse()
                .unwrap_or(anti_defaults.max_same_incident),
            max_same_signature: q
                .max_same_signature
                .parse()
                .unwrap_or(anti_defaults.max_same_signature),
            max_same_fingerprint: q
                .max_same_fingerprint
                .parse()
                .unwrap_or(anti_defaults.max_same_fingerprint),
            max_same_failing_verification: q
                .max_same_failing_verification
                .parse()
                .unwrap_or(anti_defaults.max_same_failing_verification),
            window_seconds: q
                .window_seconds
                .parse()
                .unwrap_or(anti_defaults.window_seconds),
        };
        if let anti_loop::LoopVerdict::Blocked(reason) =
            anti_loop::evaluate(&anti_cfg, &loop_signals)
        {
            let resp = self.blocked_response(
                "blocked_anti_loop",
                reason,
                &q.incident_id,
                &q.correlation_id,
            );
            self.store_idempotency(&q.idem_key, &resp, now)?;
            return Response::ok(resp);
        }

        // 4. Quota (estado contable de este repositorio).
        let cooldown_ms = q
            .cooldown_seconds
            .parse::<i64>()
            .unwrap_or(defaults.cooldown_seconds)
            * 1000;
        let day = (now / 86_400_000).to_string();
        let usage = QuotaUsage {
            // Intentos REALES del incidente (columna `attempts`, la incrementa
            // /attempt): un incidente que ya agoto intentos no vuelve a
            // encolar aunque llegue con un delivery_id nuevo.
            attempts_of_incident: prior.map(|p| p.attempts as u32).unwrap_or(0),
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
            max_repairs_per_repo: q
                .max_repairs_per_repo
                .parse()
                .unwrap_or(defaults.max_repairs_per_repo),
            max_open_repairs: q
                .max_open_repairs
                .parse()
                .unwrap_or(defaults.max_open_repairs),
            cooldown_seconds: q
                .cooldown_seconds
                .parse()
                .unwrap_or(defaults.cooldown_seconds),
            daily_budget: q.daily_budget.parse().unwrap_or(defaults.daily_budget),
        };
        if let QuotaVerdict::Blocked(reason) = quota_cfg.evaluate(&usage) {
            let resp =
                self.blocked_response("blocked_quota", reason, &q.incident_id, &q.correlation_id);
            self.store_idempotency(&q.idem_key, &resp, now)?;
            return Response::ok(resp);
        }

        // 5. Alta (upsert) + auditoria.
        exec_write(
            &self.sql,
            "INSERT INTO incidents (id, repository, signature, state, attempts, correlation_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, 0, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET state = excluded.state, correlation_id = excluded.correlation_id, updated_at = excluded.updated_at",
            vec![
                SqlStorageValue::from(q.incident_id.as_str()),
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

        // La fila debe existir con ESTE correlation_id. Si una entrega nueva
        // del mismo incidente reemplazo la correlacion (upsert de /ingest),
        // este task es obsoleto: se deniega en vez de leer attempts=0
        // invisiblemente y seguir ejecutando el pipeline para después
        // sobrescribir la verification ajena por incident_id.
        let row: Option<IncidentRow> = self.one_opt(
            "SELECT attempts, state, COALESCE(last_fingerprint, '') AS last_fingerprint FROM incidents WHERE correlation_id = ?",
            vec![SqlStorageValue::from(q.correlation_id.as_str())],
        )?;
        let Some(row) = row else {
            return Response::ok(
                serde_json::json!({
                    "allowed": false,
                    "reason": "correlation_stale_or_missing",
                    "correlation_id": q.correlation_id,
                })
                .to_string(),
            );
        };
        let from_state = if row.state.is_empty() {
            String::from(STATE_QUEUED)
        } else {
            row.state
        };
        let attempts = row.attempts + 1;
        let max_attempts = q
            .max_attempts_per_incident
            .parse()
            .unwrap_or(defaults.max_attempts_per_incident) as i64;
        if attempts > max_attempts {
            // Hard stop: registrar el bloqueo y responder denegado.
            exec_write(
                &self.sql,
                "UPDATE incidents SET state = ?, updated_at = ? WHERE correlation_id = ?",
                vec![
                    SqlStorageValue::from(STATE_BLOCKED),
                    SqlStorageValue::from(now),
                    SqlStorageValue::from(q.correlation_id.as_str()),
                ],
            )?;
            self.insert_transition(
                &q.incident_id,
                &from_state,
                STATE_BLOCKED,
                &q.correlation_id,
                now,
            )?;
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

        // Anti-loop completo en la puerta: la huella de ESTE intento no existe
        // todavia (el pipeline corre despues), asi que se mide la ultima
        // huella registrada del incidente y las verificaciones fallidas.
        let window_ms = q
            .window_seconds
            .parse::<i64>()
            .unwrap_or(anti_defaults.window_seconds)
            * 1000;
        let same_signature_recent = self.count(
            "SELECT COUNT(*) AS count FROM signature_events WHERE signature = ? AND created_at > ?",
            vec![
                SqlStorageValue::from(q.signature.as_str()),
                SqlStorageValue::from(now - window_ms),
            ],
        )? as u32;
        let same_fingerprint_recent = if row.last_fingerprint.is_empty() {
            0
        } else {
            self.count(
                "SELECT COUNT(*) AS count FROM fingerprints WHERE fingerprint = ? AND created_at > ?",
                vec![
                    SqlStorageValue::from(row.last_fingerprint.as_str()),
                    SqlStorageValue::from(now - window_ms),
                ],
            )? as u32
        };
        let same_failing_verification_recent = self.count(
            "SELECT COUNT(*) AS count FROM verification WHERE incident_id = ? AND status IN ('blocked', 'poison') AND created_at > ?",
            vec![
                SqlStorageValue::from(q.incident_id.as_str()),
                SqlStorageValue::from(now - window_ms),
            ],
        )? as u32;
        let anti_cfg = AntiLoopConfig {
            max_same_signature: q
                .max_same_signature
                .parse()
                .unwrap_or(anti_defaults.max_same_signature),
            max_same_fingerprint: q
                .max_same_fingerprint
                .parse()
                .unwrap_or(anti_defaults.max_same_fingerprint),
            max_same_failing_verification: q
                .max_same_failing_verification
                .parse()
                .unwrap_or(anti_defaults.max_same_failing_verification),
            ..anti_defaults
        };
        let signals = LoopSignals {
            same_signature_recent,
            same_fingerprint_recent,
            same_failing_verification_recent,
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
            self.insert_transition(
                &q.incident_id,
                &from_state,
                STATE_BLOCKED,
                &q.correlation_id,
                now,
            )?;
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
        self.insert_transition(
            &q.incident_id,
            &from_state,
            STATE_REPAIRING,
            &q.correlation_id,
            now,
        )?;
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
        let final_state = if q.decision == "allow" {
            STATE_DONE
        } else {
            STATE_BLOCKED
        };
        // Mismo guard que /attempt: si esta correlacion fue reemplazada por
        // una entrega nueva del incidente, el resultado es de un task
        // obsoleto y NO debe sobrescribir el estado ni la verification.
        let guard: Option<StateRow> = self.one_opt(
            "SELECT state FROM incidents WHERE correlation_id = ?",
            vec![SqlStorageValue::from(q.correlation_id.as_str())],
        )?;
        let Some(guard) = guard else {
            return Response::ok(
                serde_json::json!({
                    "ok": false,
                    "skipped": "correlation_stale_or_missing",
                    "correlation_id": q.correlation_id,
                })
                .to_string(),
            );
        };
        exec_write(
            &self.sql,
            "UPDATE incidents SET state = ?, last_fingerprint = ?, last_reason = ?, updated_at = ? WHERE correlation_id = ?",
            vec![
                SqlStorageValue::from(final_state),
                SqlStorageValue::from(q.fingerprint.as_str()),
                SqlStorageValue::from(q.reason.as_str()),
                SqlStorageValue::from(now),
                SqlStorageValue::from(q.correlation_id.as_str()),
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
        let from_state = if guard.state.is_empty() {
            String::from(STATE_QUEUED)
        } else {
            guard.state
        };
        self.insert_transition(
            &q.incident_id,
            &from_state,
            final_state,
            &q.correlation_id,
            now,
        )?;
        Response::ok(serde_json::json!({ "ok": true }).to_string())
    }

    /// POST /poison: mensaje caido en DLQ.
    fn poison(&self, req: &Request) -> Result<Response> {
        let q: PoisonQuery = req.query()?;
        let now = now_ms();
        // Mismo guard: un veneno de una correlacion reemplazada no debe
        // marcar dead_letter a la entrega vigente del incidente.
        let guard: Option<StateRow> = self.one_opt(
            "SELECT state FROM incidents WHERE correlation_id = ?",
            vec![SqlStorageValue::from(q.correlation_id.as_str())],
        )?;
        let Some(guard) = guard else {
            return Response::ok(
                serde_json::json!({
                    "ok": false,
                    "skipped": "correlation_stale_or_missing",
                    "correlation_id": q.correlation_id,
                })
                .to_string(),
            );
        };
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
        let from_state = if guard.state.is_empty() {
            String::from(STATE_QUEUED)
        } else {
            guard.state
        };
        self.insert_transition(
            &q.incident_id,
            &from_state,
            STATE_DEAD_LETTER,
            &q.correlation_id,
            now,
        )?;
        Response::ok(serde_json::json!({ "ok": true }).to_string())
    }

    /// GET /state: observabilidad del incidente.
    fn state(&self, req: &Request) -> Result<Response> {
        let q: StateQuery = req.query()?;
        let where_clause = if q.correlation_id.is_empty() {
            "id = ?"
        } else {
            "correlation_id = ?"
        };
        let value = if q.correlation_id.is_empty() {
            q.incident_id.clone()
        } else {
            q.correlation_id.clone()
        };
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
    // Necesario para el TTL del contrato (CONTRACT.md §5): una respuesta
    // guardada expira a las IDEMPOTENCY_TTL_SECONDS.
    #[serde(default)]
    created_at: i64,
}

#[derive(Debug, Deserialize)]
struct RepoRow {
    repository: String,
}
