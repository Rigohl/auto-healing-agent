//! NO AUTO-EDITARSE (decision del dueno, 2026-10-08): el agente NUNCA
//! abre PRs de reparacion contra su propio repositorio. El MONITOR si
//! puede observar el repo propio (solo lectura: retencion + salud), pero
//! el camino de ESCRITURA (rama -> commit -> PR) queda cerrado para el.
//!
//! Fail-closed: repo propio => blocked_by_policy/self_edit_forbidden antes
//! de cualquier llamada a la API de GitHub. La var OWN_REPO configura el
//! repo propio (default: Rigohl/auto-healing-agent). Un agente que se
//! repara a si mismo es un bucle de auto-modificacion sin barrera.

use worker::Env;

pub const VAR_OWN_REPO: &str = "OWN_REPO";
pub const DEFAULT_OWN_REPO: &str = "Rigohl/auto-healing-agent";

pub fn own_repo(env: &Env) -> String {
    match env.var(VAR_OWN_REPO) {
        Ok(v) => {
            let s = v.to_string();
            if s.trim().is_empty() {
                DEFAULT_OWN_REPO.to_string()
            } else {
                s
            }
        }
        Err(_) => DEFAULT_OWN_REPO.to_string(),
    }
}

/// Comparacion pura (testeada): insensible a mayusculas y espacios.
pub fn is_match(own: &str, repo: &str) -> bool {
    own.trim().eq_ignore_ascii_case(repo.trim())
}

pub fn is_own_repo(env: &Env, repo: &str) -> bool {
    is_match(&own_repo(env), repo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_repo_match_is_case_and_space_insensitive() {
        assert!(is_match(
            "Rigohl/auto-healing-agent",
            "rigohl/AUTO-HEALING-AGENT"
        ));
        assert!(is_match(
            " Rigohl/auto-healing-agent ",
            "Rigohl/auto-healing-agent"
        ));
        assert!(!is_match("Rigohl/auto-healing-agent", "Rigohl/other"));
        assert!(!is_match("", ""));
    }
}
