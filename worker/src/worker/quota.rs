//! Quotas del runtime: max_attempts_per_incident, max_repairs_per_repo,
//! max_open_repairs, cooldown y daily_budget.
//!
//! Separacion config/estado exigida por PART3: los valores configurables
//! viven en [vars] de wrangler.toml y se leen aqui; el estado contable
//! (cuantos intentos/open/repairs lleva cada repo) vive en el Durable Object.
//! Las funciones de decision son puras y estan testeadas.

use worker::Env;

pub const VAR_MAX_ATTEMPTS_PER_INCIDENT: &str = "QUOTA_MAX_ATTEMPTS_PER_INCIDENT";
pub const VAR_MAX_REPAIRS_PER_REPO: &str = "QUOTA_MAX_REPAIRS_PER_REPO";
pub const VAR_MAX_OPEN_REPAIRS: &str = "QUOTA_MAX_OPEN_REPAIRS";
pub const VAR_COOLDOWN_SECONDS: &str = "QUOTA_COOLDOWN_SECONDS";
pub const VAR_DAILY_BUDGET: &str = "QUOTA_DAILY_BUDGET";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaConfig {
    pub max_attempts_per_incident: u32,
    pub max_repairs_per_repo: u32,
    pub max_open_repairs: u32,
    pub cooldown_seconds: i64,
    pub daily_budget: u32,
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            max_attempts_per_incident: 3,
            max_repairs_per_repo: 10,
            max_open_repairs: 3,
            cooldown_seconds: 3600,
            daily_budget: 50,
        }
    }
}

impl QuotaConfig {
    /// Construye la configuracion desde pares clave/valor (vars). Las
    /// claves desconocidas se ignoran; las ausentes o no numericas usan el
    /// default (fail-safe, nunca fail-open).
    pub fn from_pairs(pairs: &[(String, String)]) -> Self {
        let mut cfg = Self::default();
        for (key, value) in pairs {
            match key.as_str() {
                VAR_MAX_ATTEMPTS_PER_INCIDENT => {
                    cfg.max_attempts_per_incident =
                        value.parse().unwrap_or(cfg.max_attempts_per_incident);
                }
                VAR_MAX_REPAIRS_PER_REPO => {
                    cfg.max_repairs_per_repo = value.parse().unwrap_or(cfg.max_repairs_per_rep
o);
                }
                VAR_MAX_OPEN_REPAIRS => {
                    cfg.max_open_repairs = value.parse().unwrap_or(cfg.max_open_repairs);
                }
                VAR_COOLDOWN_SECONDS => {
                    cfg.cooldown_seconds = value.parse().unwrap_or(cfg.cooldown_seconds);
                }
                VAR_DAILY_BUDGET => {
                    cfg.daily_budget = value.parse().unwrap_or(cfg.daily_budget);
                }
                _ => {}
            }
        }
        cfg
    }

    /// Lee la configuracion de las vars del entorno del Worker.
    pub fn from_env(env: &Env) -> Self {
        let keys = [
            VAR_MAX_ATTEMPTS_PER_INCIDENT,
            VAR_MAX_REPAIRS_PER_REPO,
            VAR_MAX_OPEN_REPAIRS,
            VAR_COOLDOWN_SECONDS,
            VAR_DAILY_BUDGET,
        ];
        let mut pairs = Vec::new();
        for key in keys {
            if let Ok(var) = env.var(key) {
                pairs.push((key.to_string(), var.to_string()));
            }
        }
        Self::from_pairs(&pairs)
    }

    /// Decision pura: con este uso acumulado, se permite un intento mas?
    pub fn evaluate(&self, usage: &QuotaUsage) -> QuotaVerdict {
        if usage.attempts_of_incident >= self.max_attempts_per_incident {
            return QuotaVerdict::Blocked("max_attempts_per_incident");
        }
        if usage.repairs_in_window >= self.max_repairs_per_repo {
            return QuotaVerdict::Blocked("max_repairs_per_repo_cooldown");
        }
        if usage.open_repairs >= self.max_open_repairs {
            return QuotaVerdict::Blocked("max_open_repairs");
        }
        if usage.daily_repairs >= self.daily_budget {
            return QuotaVerdict::Blocked("daily_budget");
        }
        QuotaVerdict::Allowed
    }
}

/// Estado contable observado (lo cuenta el Durable Object).
#[derive(Debug, Clone, Copy, Default)]
pub struct QuotaUsage {
    pub attempts_of_incident: u32,
    pub repairs_in_window: u
32,
    pub open_repairs: u32,
    pub daily_repairs: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaVerdict {
    Allowed,
    Blocked(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> QuotaConfig {
        QuotaConfig {
            max_attempts_per_incident: 3,
            max_repairs_per_repo: 2,
            max_open_repairs: 1,
            cooldown_seconds: 60,
            daily_budget: 5,
        }
    }

    #[test]
    fn allows_below_all_limits() {
        let usage = QuotaUsage {
            attempts_of_incident: 2,
            repairs_in_window: 1,
            open_repairs: 0,
            daily_repairs: 4,
        };
        assert_eq!(cfg().evaluate(&usage), QuotaVerdict::Allowed);
    }

    #[test]
    fn blocks_each_limit_independently() {
        let base = QuotaUsage {
            attempts_of_incident: 0,
            repairs_in_window: 0,
            open_repairs: 0,
            daily_repairs: 0,
        };
        assert_eq!(
            cfg().evaluate(&QuotaUsage { attempts_of_incident: 3, ..base }),
            QuotaVerdict::Blocked("max_attempts_per_incident")
        );
        assert_eq!(
            cfg().evaluate(&QuotaUsage { repairs_in_window: 2, ..base }),
            QuotaVerdict::Blocked("max_repairs_per_repo_cooldown")
        );
        assert_eq!(
            cfg().evaluate(&QuotaUsage { open_repairs: 1, ..base }),
            QuotaVerdict::Blocked("max_open_repairs")
        );
        assert_eq!(
            cfg().evaluate(&QuotaUsage { daily_repairs: 5, ..base }),
            QuotaVerdict::Blocked("daily_budget")
        );
    }

    #[test]
    fn from_pairs_parses_and_falls_back() {
        let cfg = QuotaConfig::from_pairs(&[
            (VAR_MAX_ATTEMPTS_PER_INCIDENT.to_string(), "7".to_string()),
            (VAR_DAILY_BUDGET.to_string(), "no-numero".to_string()),
        ]);
        assert_eq!(cfg.max_attempts_per_incident, 7);
        assert_eq!(cfg.daily_budget, QuotaConfig::
default().daily_budget);
    }
}
