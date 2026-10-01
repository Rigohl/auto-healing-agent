//! Anti-loop: detecta repeticiones del mismo incidente, la misma firma de
//! fallo, el mismo repositorio, la misma huella de parche o la misma
//! verificacion fallida, y produce BLOCKED al exceder los limites.
//!
//! Un bucle de auto-reparacion (incidente que reaparece, el operador vuelve a
//! proponer lo mismo, CI vuelve a fallar igual) es el fallo mas caro de un
//! agente auto-reparador: cada vuelta consume quota, colas y confianza. El
//! corte es fail-closed y por ventana de tiempo. Decisiones puras + tests.

use worker::Env;

pub const VAR_MAX_SAME_INCIDENT: &str = "ANTI_LOOP_MAX_SAME_INCIDENT";
pub const VAR_MAX_SAME_SIGNATURE: &str = "ANTI_LOOP_MAX_SAME_SIGNATURE";
pub const VAR_MAX_SAME_FINGERPRINT: &str = "ANTI_LOOP_MAX_SAME_FINGERPRINT";
pub const VAR_MAX_SAME_FAILING_VERIFICATION: &str = "ANTI_LOOP_MAX_SAME_FAILING_VERIFICATION";
pub const VAR_WINDOW_SECONDS: &str = "ANTI_LOOP_WINDOW_SECONDS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AntiLoopConfig {
    pub max_same_incident: u32,
    pub max_same_signature: u32,
    pub max_same_fingerprint: u32,
    pub max_same_failing_verification: u32,
    pub window_seconds: i64,
}

impl Default for AntiLoopConfig {
    fn default() -> Self {
        Self {
            max_same_incident: 3,
            max_same_signature: 5,
            max_same_fingerprint: 2,
            max_same_failing_verification: 3,
            window_seconds: 900,
        }
    }
}

impl AntiLoopConfig {
    pub fn from_pairs(pairs: &[(String, String)]) -> Self {
        let mut cfg = Self::default();
        for (key, value) in pairs {
            match key.as_str() {
                VAR_MAX_SAME_INCIDENT => {
                    cfg.max_same_incident = value.parse().unwrap_or(cfg.max_same_incident);
                }
                VAR_MAX_SAME_SIGNATURE => {
                    cfg.max_same_signature = value.parse().unwrap_or(cfg.max_same_signature);
                }
                VAR_MAX_SAME_FINGERPRINT => {
 
                   cfg.max_same_fingerprint = value.parse().unwrap_or(cfg.max_same_fingerprint);
                }
                VAR_MAX_SAME_FAILING_VERIFICATION => {
                    cfg.max_same_failing_verification = value
                        .parse()
                        .unwrap_or(cfg.max_same_failing_verification);
                }
                VAR_WINDOW_SECONDS => {
                    cfg.window_seconds = value.parse().unwrap_or(cfg.window_seconds);
                }
                _ => {}
            }
        }
        cfg
    }

    pub fn from_env(env: &Env) -> Self {
        let keys = [
            VAR_MAX_SAME_INCIDENT,
            VAR_MAX_SAME_SIGNATURE,
            VAR_MAX_SAME_FINGERPRINT,
            VAR_MAX_SAME_FAILING_VERIFICATION,
            VAR_WINDOW_SECONDS,
        ];
        let mut pairs = Vec::new();
        for key in keys {
            if let Ok(var) = env.var(key) {
                pairs.push((key.to_string(), var.to_string()));
            }
        }
        Self::from_pairs(&pairs)
    }
}

/// Senales observadas dentro de la ventana (las cuenta el Durable Object).
#[derive(Debug, Clone, Copy, Default)]
pub struct LoopSignals {
    pub same_incident_recent: u32,
    pub same_signature_recent: u32,
    pub same_fingerprint_recent: u32,
    pub same_failing_verification_recent: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopVerdict {
    Allowed,
    Blocked(&'static str),
}

pub fn evaluate(config: &AntiLoopConfig, signals: &LoopSignals) -> LoopVerdict {
    if signals.same_incident_recent >= config.max_same_incident {
        return LoopVerdict::Blocked("anti_loop_same_incident");
    }
    if signals.same_signature_recent >= config.max_same_signature {
        return LoopVerdict::Blocked("anti_loop_same_signature");
    }
    if signals.same_fingerprint_recent >= config.max_same_fingerprint {
        return LoopVerdict::Blocked("anti_loop_same_fingerprint");
    }
    if signals.same_failing
_verification_recent >= config.max_same_failing_verification {
        return LoopVerdict::Blocked("anti_loop_same_failing_verification");
    }
    LoopVerdict::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> AntiLoopConfig {
        AntiLoopConfig {
            max_same_incident: 3,
            max_same_signature: 2,
            max_same_fingerprint: 1,
            max_same_failing_verification: 2,
            window_seconds: 900,
        }
    }

    fn none() -> LoopSignals {
        LoopSignals::default()
    }

    #[test]
    fn allows_without_signals() {
        assert_eq!(evaluate(&cfg(), &none()), LoopVerdict::Allowed);
    }

    #[test]
    fn blocks_each_signal_independently() {
        assert_eq!(
            evaluate(&cfg(), &LoopSignals { same_incident_recent: 3, ..none() }),
            LoopVerdict::Blocked("anti_loop_same_incident")
        );
        assert_eq!(
            evaluate(&cfg(), &LoopSignals { same_signature_recent: 2, ..none() }),
            LoopVerdict::Blocked("anti_loop_same_signature")
        );
        assert_eq!(
            evaluate(&cfg(), &LoopSignals { same_fingerprint_recent: 1, ..none() }),
            LoopVerdict::Blocked("anti_loop_same_fingerprint")
        );
        assert_eq!(
            evaluate(
                &cfg(),
                &LoopSignals { same_failing_verification_recent: 2, ..none() }
            ),
            LoopVerdict::Blocked("anti_loop_same_failing_verification")
        );
    }

    #[test]
    fn below_limits_is_allowed() {
        let signals = LoopSignals {
            same_incident_recent: 2,
            same_signature_recent: 1,
            same_fingerprint_recent: 0,
            same_failing_verification_recent: 1,
        };
        assert_eq!(evaluate(&cfg(), &signals), LoopVerdict::Allowed);
    }
}
