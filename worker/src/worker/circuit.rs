//! CIRCUIT BREAKER por operador (adopcion del namespace huerfano CACHE,
//! decision del dueno 2026-10-08). Patron CLOSED -> OPEN -> HALF_OPEN
//! tomado del analisis de repos self-healing (restrok/auto-healing-agent,
//! 2026-10-08): si un operador acumula fails consecutivos VERIFICADOS por
//! Actions, el circuito abre y los siguientes intentos con ese operador se
//! bloquean fail-closed hasta que la ventana pase.
//!
//! Reglas:
//! - La senal es SIEMPRE la verificacion real de Actions (/github/callback);
//!   un blocked del gate NO cuenta como fail del operador (el gate freno el
//!   parche, CI no lo rechazo).
//! - HALF_OPEN implicito: pasada la ventana, el siguiente intento re-evalua;
//!   un PASS cierra el circuito, un FAIL lo reabre.
//! - Estado en CACHE KV, best-effort: sin KV el circuito se lee CLOSED
//!   (fail-open AQUI es seguro: el gate determinista y las cuotas siguen
//!   mandando; el breaker es una red extra, no la autoridad).
//! - Determinista: sin relojes propios mas alla del timestamp de la senal.

use serde::{Deserialize, Serialize};
use worker::{console_error, Env};

/// Binding del KV del breaker (wrangler.toml: namespace CACHE).
pub const KV_BINDING: &str = "CACHE";
pub const PREFIX: &str = "breaker";
/// Fails consecutivos verificados que abren el circuito.
pub const MAX_CONSECUTIVE_FAILS: u32 = 3;
/// Ventana OPEN en segundos; pasada la ventana el circuito re-evalua.
pub const OPEN_WINDOW_SECONDS: u64 = 3600;

/// Estado contable del operador (persistido en CACHE KV).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakerState {
    pub consecutive_fails: u32,
    /// 0 = nunca abierto; si no, el timestamp unix EN SEGUNDOS del momento
    /// en que abrio (mismo orden de magnitud que OPEN_WINDOW_SECONDS).
    pub opened_at_unix: u64,
}

/// Decision pura (testeada): el circuito esta OPEN?
pub fn state_is_open(state: &BreakerState, now_unix: u64) -> bool {
    state.consecutive_fails >= MAX_CONSECUTIVE_FAILS
        && state.opened_at_unix > 0
        && now_unix.saturating_sub(state.opened_at_unix) < OPEN_WINDOW_SECONDS
}

/// Transicion pura (testeada) tras una verificacion real de Actions.
/// PASS => CLOSED (reset total). FAIL => incrementa; al alcanzar el umbral
/// fija opened_at (solo la primera vez: la ventana cuenta desde la apertura,
/// no desde cada fail).
pub fn transition(state: &BreakerState, pass: bool, now_unix: u64) -> BreakerState {
    if pass {
        return BreakerState::default();
    }
    let fails = state.consecutive_fails.saturating_add(1);
    let opened_at = if fails >= MAX_CONSECUTIVE_FAILS && state.opened_at_unix == 0 {
        now_unix
    } else {
        state.opened_at_unix
    };
    BreakerState {
        consecutive_fails: fails,
        opened_at_unix: opened_at,
    }
}

/// Clave determinista por operador (global: el breaker mide al OPERADOR,
/// no al repo; un operador que rompe CI en cualquier repo esta roto).
pub fn key(operator: u8) -> String {
    format!("{}:{:02x}", PREFIX, operator)
}

/// Segundos unix. now_ms() devuelve MILISEGUNDOS: pasar ms donde el
/// estado y la ventana razonan en segundos hacia la ventana de 3.6s en
/// vez de 1h (bug de unidades hallado en el review del PR #114).
fn unix_seconds() -> u64 {
    (crate::runtime::now_ms() / 1000) as u64
}

/// El circuito del operador esta OPEN? Best-effort (fail-open seguro: ver
/// mod docs). Nunca bloquea el pipeline por un fallo de lectura de KV.
pub async fn is_open(env: &Env, operator: repair_types::OperatorId) -> bool {
    match read_state(env, operator as u8).await {
        Some(state) => state_is_open(&state, unix_seconds()),
        None => false,
    }
}

/// Registra la senal REAL de Actions (pass/fail). Best-effort: un fallo de
/// KV solo se loguea (el DO ya registro la decision autoritativa).
pub async fn record(env: &Env, operator: u8, pass: bool) {
    let current = read_state(env, operator).await.unwrap_or_default();
    let next = transition(&current, pass, unix_seconds());
    if let Ok(kv) = env.kv(KV_BINDING) {
        if let Ok(serialized) = serde_json::to_string(&next) {
            if let Ok(builder) = kv.put(&key(operator), serialized) {
                if let Err(e) = builder.execute().await {
                    console_error!("circuit kv put failed: {e}");
                }
            }
        }
    }
}

async fn read_state(env: &Env, operator: u8) -> Option<BreakerState> {
    let kv = env.kv(KV_BINDING).ok()?;
    let raw = kv.get(&key(operator)).text().await.ok().flatten()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_after_threshold_and_closes_after_window() {
        let mut s = BreakerState::default();
        for _ in 0..3 {
            s = transition(&s, false, 1000);
        }
        assert_eq!(s.consecutive_fails, 3);
        assert_eq!(s.opened_at_unix, 1000);
        // OPEN dentro de la ventana
        assert!(state_is_open(&s, 1000 + 60));
        // y tambien exactamente en el borde interno de la ventana
        assert!(state_is_open(&s, 1000 + OPEN_WINDOW_SECONDS - 1));
        // cerrada al pasar la ventana (HALF_OPEN implicito: re-evalua)
        assert!(!state_is_open(&s, 1000 + OPEN_WINDOW_SECONDS));
    }

    #[test]
    fn pass_resets_completely() {
        let mut s = BreakerState::default();
        s = transition(&s, false, 10);
        s = transition(&s, false, 20);
        s = transition(&s, true, 30);
        assert_eq!(s, BreakerState::default());
        assert!(!state_is_open(&s, 30));
    }

    #[test]
    fn opened_at_freezes_at_first_open_not_each_fail() {
        let mut s = BreakerState::default();
        s = transition(&s, false, 100);
        s = transition(&s, false, 200);
        s = transition(&s, false, 300); // abre aqui
        assert_eq!(s.opened_at_unix, 300);
        s = transition(&s, false, 400); // ya abierto: no mueve la ventana
        assert_eq!(s.opened_at_unix, 300);
        assert_eq!(s.consecutive_fails, 4);
    }

    #[test]
    fn below_threshold_is_not_open_even_with_stale_opened_at() {
        let s = BreakerState {
            consecutive_fails: 1,
            opened_at_unix: 500,
        };
        assert!(!state_is_open(&s, 600));
    }

    #[test]
    fn key_is_deterministic() {
        assert_eq!(key(1), "breaker:01");
        assert_eq!(key(255), "breaker:ff");
    }
}
