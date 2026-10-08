//! Investigacion web en paralelo (PART5, decision del dueno 2026-10-08:
//! "que investigue en internet a la par").
//!
//! Mientras el gate/LLM deciden la reparacion, este modulo consulta fuentes
//! PUBLICAS y gratuitas para dar contexto REAL a la propuesta:
//!   - registry.npmjs.org/{dep}/latest -> version vigente de la dependencia
//!     citada por el error (accionable para VERSION_PIN/DEPENDENCY_REPAIR).
//!   - doc.rust-lang.org/error_codes -> explicacion oficial del codigo E0xxx.
//!
//! Politica (docs/LLM_POLICY.md, PART5): la investigacion NUNCA repara sola
//! ni abre el gate. Es CONTEXTO para el LLM (prompt acotado) y para el
//! auditor (KV). Fail-open: sin red o deshabilitada (RESEARCH_ENABLED=false)
//! el reporte llega vacio y la reparacion sigue su camino normal. Solo
//! HTTPS GET a dominios de la lista blanca; sin API keys (endpoints
//! publicos, plan Always Free).

use serde::Serialize;
use worker::{console_warn, Env, Fetch, Headers, Method, Request, RequestInit};

use repair_types::Incident;

/// Var para deshabilitar la investigacion (default: habilitada; el
/// comportamiento previo --sin contexto-- se recupera poniendola "false").
pub const VAR_ENABLED: &str = "RESEARCH_ENABLED";

/// Prefijo de las claves de evidencia de investigacion (REPAIR_CASES_KV).
pub const RESEARCH_PREFIX: &str = "research:";

/// Lista blanca: solo estos dominios, sin redirects arbitrarios.
const NPM_REGISTRY: &str = "https://registry.npmjs.org";
const RUSTC_DOCS: &str = "https://doc.rust-lang.org/error_codes";

/// Reporte de investigacion: datos, no prosa libre.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ResearchReport {
    /// Dependencia citada por el incidente (si se pudo derivar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency: Option<String>,
    /// Version "latest" del registry (senal viva, no cache del repo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm_latest: Option<String>,
    /// URL de la explicacion oficial del codigo de error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_docs: Option<String>,
}

impl ResearchReport {
    pub fn is_empty(&self) -> bool {
        self.dependency.is_none() && self.npm_latest.is_none() && self.error_docs.is_none()
    }

    /// Render acotado para el prompt del LLM (solo datos, sin prosa libre).
    pub fn render(&self) -> String {
        let mut out = String::new();
        if let Some(dep) = &self.dependency {
            out.push_str(&format!("- dependency: {dep}\n"));
        }
        if let Some(v) = &self.npm_latest {
            out.push_str(&format!("- npm latest: {v}\n"));
        }
        if let Some(u) = &self.error_docs {
            out.push_str(&format!("- error docs: {u}\n"));
        }
        out
    }
}

/// Investigacion completa del incidente. Fail-open por diseno: cualquier
/// fallo (red, dominio, parseo) deja el campo en None.
#[worker::send]
pub async fn research(env: &Env, incident: &Incident) -> ResearchReport {
    let mut report = ResearchReport::default();
    if env.var(VAR_ENABLED).map(|v| v.to_string()).ok().as_deref() == Some("false") {
        console_warn!("web research disabled: RESEARCH_ENABLED == false");
        return report;
    }
    if let Some(url) = rustc_error_docs_url(&incident.error_code) {
        report.error_docs = Some(url);
    }
    if let Some(dep) = dependency_hint(incident) {
        if let Some(latest) = fetch_npm_latest(&dep).await {
            report.npm_latest = Some(latest);
        }
        report.dependency = Some(dep);
    }
    report
}

/// Evidencia de investigacion en REPAIR_CASES_KV (auditoria). Best-effort:
/// un fallo de KV no afecta la reparacion.
#[worker::send]
pub async fn persist(env: &Env, correlation_id: &str, report: &ResearchReport) {
    if report.is_empty() {
        return;
    }
    let Ok(serialized) = serde_json::to_string(report) else {
        return;
    };
    let Ok(kv) = env.kv(crate::runtime::queue_consumer::REPAIR_CASES_KV) else {
        console_warn!("REPAIR_CASES_KV unavailable: research evidence skipped");
        return;
    };
    let key = format!("{RESEARCH_PREFIX}{correlation_id}");
    match kv.put(&key, serialized) {
        Ok(builder) => {
            if let Err(e) = builder.execute().await {
                console_warn!("research kv put failed: {e}");
            }
        }
        Err(e) => console_warn!("research kv builder failed: {e}"),
    }
}

/// URL del registry npm para la version vigente. None si la dependencia no
/// es un nombre de paquete seguro (fail-closed: nada de URLs inyectadas).
pub fn npm_registry_url(dep: &str) -> Option<String> {
    if !valid_package_name(dep) {
        return None;
    }
    Some(format!("{NPM_REGISTRY}/{dep}/latest"))
}

/// Explicacion oficial de rustc: solo codigos E + 4 digitos. El URL se
/// construye, no se recibe: jamas se hace echo de URLs del incidente.
pub fn rustc_error_docs_url(error_code: &str) -> Option<String> {
    let bytes = error_code.as_bytes();
    if bytes.len() == 5
        && bytes[0] == b'E'
        && bytes[1..].iter().all(|b| b.is_ascii_digit())
    {
        Some(format!("{RUSTC_DOCS}/{error_code}.html"))
    } else {
        None
    }
}

/// "version" del payload del registry, validado (sin inyeccion).
pub fn parse_latest_version(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let version = value.get("version")?.as_str()?;
    if valid_version(version) {
        Some(String::from(version))
    } else {
        None
    }
}

async fn fetch_npm_latest(dep: &str) -> Option<String> {
    let url = npm_registry_url(dep)?;
    let headers = Headers::new();
    headers.set("Accept", "application/json").ok()?;
    headers.set("User-Agent", "auto-healing-agent").ok()?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let req = Request::new_with_init(&url, &init).ok()?;
    let mut resp = Fetch::Request(req).send().await.ok()?;
    if resp.status_code() != 200 {
        return None;
    }
    let body = resp.text().await.ok()?;
    parse_latest_version(&body)
}

/// Dependencia citada por el incidente (token entre comillas tras un
/// patron conocido de modulo no resuelto). Solo paquetes del registry:
/// los especificadores relativos (./x, ../x) no se investigan.
fn dependency_hint(incident: &Incident) -> Option<String> {
    let text = format!("{}\n{}", incident.message, incident.stack_hint);
    let chars: Vec<char> = text.chars().collect();
    for needle in ["cannot find module", "can't resolve", "module not found"] {
        let Some(i) = find_ci(&chars, needle) else {
            continue;
        };
        let mut j = i + needle.chars().count();
        while j < chars.len() && chars[j] != '\'' && chars[j] != '"' {
            j += 1;
        }
        if j >= chars.len() {
            continue;
        }
        let quote = chars[j];
        j += 1;
        let mut token = String::new();
        while j < chars.len() && chars[j] != quote {
            token.push(chars[j]);
            j += 1;
        }
        if j < chars.len() && valid_package_name(&token) {
            return Some(token);
        }
    }
    None
}

/// Nombre de paquete npm seguro para un GET del registry: sin espacios,
/// comillas, traversal ni inyeccion de URL. Scoped ("@scope/name") admitido.
fn valid_package_name(dep: &str) -> bool {
    !dep.is_empty()
        && dep.len() <= 214
        && !dep.starts_with(['-', '.', '/'])
        && !dep.contains("..")
        && dep
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | '@'))
        // La arroba solo vale como prefijo de scope ("@scope/name").
        && dep.find('@').map(|i| i == 0).unwrap_or(true)
}

fn valid_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 64
        && v
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

/// Primera aparicion de "needle" ignorando mayusculas ASCII.
fn find_ci(chars: &[char], needle: &str) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || chars.len() < n.len() {
        return None;
    }
    (0..=chars.len() - n.len()).find(|&i| {
        chars[i..i + n.len()]
            .iter()
            .zip(&n)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_registry_url_accepts_plain_and_scoped() {
        assert_eq!(
            npm_registry_url("lodash"),
            Some("https://registry.npmjs.org/lodash/latest".to_string())
        );
        assert_eq!(
            npm_registry_url("@scope/pkg"),
            Some("https://registry.npmjs.org/@scope/pkg/latest".to_string())
        );
        assert_eq!(npm_registry_url("../evil"), None);
        assert_eq!(npm_registry_url("pkg?x=1"), None);
        assert_eq!(npm_registry_url("a b"), None);
        assert_eq!(npm_registry_url("lodash@1.2.3"), None);
    }

    #[test]
    fn parse_latest_version_reads_registry_payload() {
        let body = r#"{"name":"lodash","version":"4.17.21"}"#;
        assert_eq!(parse_latest_version(body), Some("4.17.21".to_string()));
        assert_eq!(parse_latest_version("{}"), None);
        assert_eq!(parse_latest_version("not json"), None);
        // Version maliciosa: rechazada por la validacion de caracteres.
        assert_eq!(parse_latest_version(r#"{"version":"1.0.0\"x"}"#), None);
    }

    #[test]
    fn rustc_docs_url_only_for_real_error_codes() {
        assert_eq!(
            rustc_error_docs_url("E0432"),
            Some("https://doc.rust-lang.org/error_codes/E0432.html".to_string())
        );
        assert_eq!(rustc_error_docs_url("e0432"), None);
        assert_eq!(rustc_error_docs_url("EPIC"), None);
        assert_eq!(rustc_error_docs_url("E043"), None);
        assert_eq!(rustc_error_docs_url("E04321"), None);
        assert_eq!(rustc_error_docs_url(""), None);
    }

    #[test]
    fn dependency_hint_extracts_quoted_package() {
        let incident = Incident {
            error_code: String::new(),
            message: String::from("Module not found: Error: Can't resolve 'axios'"),
            ..Default::default()
        };
        assert_eq!(dependency_hint(&incident), Some("axios".to_string()));
        // Import relativo: no es un paquete del registry, no se investiga.
        let rel = Incident {
            message: String::from("Cannot find module './utils'"),
            ..Default::default()
        };
        assert_eq!(dependency_hint(&rel), None);
    }

    #[test]
    fn render_lists_findings() {
        let mut r = ResearchReport::default();
        assert!(r.is_empty());
        r.dependency = Some(String::from("axios"));
        r.npm_latest = Some(String::from("1.7.0"));
        r.error_docs = Some(String::from(
            "https://doc.rust-lang.org/error_codes/E0432.html",
        ));
        let text = r.render();
        assert!(text.contains("axios"));
        assert!(text.contains("1.7.0"));
        assert!(text.contains("E0432.html"));
        assert!(!r.is_empty());
    }
}
