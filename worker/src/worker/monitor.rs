//! MONITOR: evento scheduled (cron) del Worker.
//!
//! El pipeline declara MONITOR tras REPAIR/VERIFY, pero el runtime no
//! tenia cron: la retencion del DO solo corria en /ingest (un repo sin
//! incidentes nuevos nunca purgaba) y no habia observabilidad periodica
//! del registro de modelo.
//!
//! Reglas:
//! - MONITOR es OBSERVABILIDAD + retencion. Nunca repara, nunca encola
//!   y nunca toca la autoridad de VERIFY (GitHub Actions).
//! - Fail-closed contra si mismo: un error del DO o del KV en el sweep
//!   solo se loguea; el cron nunca tumba nada ni inventa estado.
//! - Sin unwrap() en el path del cron (igual que en fetch).
//!
//! Repos barridos: var MONITOR_REPOS (separados por comas). Vacia =
//! no-op con log: el DO es por repositorio (id_from_name) y NO existe
//! registro global enumerable; el registro de repos es configuracion,
//! no estado (separacion config/estado de PART3).

use worker::*;

use crate::runtime::{candidate, model};

/// Var con la lista de repositorios a barrer (separados por comas).
pub const VAR_MONITOR_REPOS: &str = "MONITOR_REPOS";

/// Parsea la lista de repos: split por comas, trim, sin vacios y sin
/// duplicados (el primero gana, orden estable). Funcion pura + testeada.
pub fn parse_repos(raw: &str) -> Vec<String> {
    let mut repos: Vec<String> = Vec::new();
    for token in raw.split(',') {
        let repo = token.trim();
        if repo.is_empty() {
            continue;
        }
        if !repos.iter().any(|r| r.as_str() == repo) {
            repos.push(repo.to_string());
        }
    }
    repos
}

/// Punto de entrada del cron (lo llama #[event(scheduled)] de lib.rs).
/// Best-effort total: cada repo se barre de forma independiente y un
/// fallo no corta los demas.
#[worker::send]
pub async fn run(env: &Env) -> Result<()> {
    let repos = match env.var(VAR_MONITOR_REPOS) {
        Ok(v) => parse_repos(&v.to_string()),
        Err(_) => Vec::new(),
    };
    if repos.is_empty() {
        console_warn!("MONITOR: {} sin configurar: no-op", VAR_MONITOR_REPOS);
        return Ok(());
    }
    for repo in repos {
        match crate::runtime::call_do(env.clone(), repo.clone(), "/sweep".to_string()).await {
            Ok(text) => console_log!("MONITOR sweep repo={} -> {}", repo, text),
            Err(e) => console_error!("MONITOR sweep repo={} failed: {}", repo, e),
        }
    }
    // Salud del registro de modelo: sin current NI stable el pipeline
    // responde blocked_no_model. El cron solo OBSERVA: nunca promociona.
    let health = model::health(env).await;
    if !health.current && !health.stable {
        console_error!(
            "MONITOR: model registry empty (current missing, stable missing): pipeline blocked_no_model"
        );
    } else {
        console_log!(
            "MONITOR: model registry current={} stable={}",
            health.current,
            health.stable
        );
    }
    // Challenger (NN_WEIGHTS): solo observabilidad del estado del
    // candidato; nunca sirve inferencia (promocion humana via
    // promote-model.yml).
    let challenger = candidate::status(env).await;
    console_log!("MONITOR: candidate weights status={}", challenger.as_str());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_repos_splits_trims_and_dedups() {
        assert_eq!(
            parse_repos(" Rigohl/auto-healing-agent , other/repo ,Rigohl/auto-healing-agent,"),
            vec![
                "Rigohl/auto-healing-agent".to_string(),
                "other/repo".to_string()
            ]
        );
    }

    #[test]
    fn parse_repos_empty_is_noop() {
        assert!(parse_repos("").is_empty());
        assert!(parse_repos(" , ,").is_empty());
    }
}
