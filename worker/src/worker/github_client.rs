//! Cliente de la API REST de GitHub desde el Worker (100% Rust/WASM).
//!
//! PASO 2 del roadmap (docs/IMPLEMENTATION_PROGRESS.md): abrir el PR de
//! reparacion usando `worker::Fetch` (workers-rs 0.8) — sin `octocrab` ni
//! dependencias JS. Secuencia minima por reparacion:
//!   1. GET  /repos/{repo}                       -> rama base
//!   2. GET  /repos/{repo}/git/ref/heads/{base}  -> sha de la base
//!   3. POST /repos/{repo}/git/refs              -> rama auto-heal/{cid}
//!   4. PUT  /repos/{repo}/contents/{path}       -> commit del parche
//!   5. POST /repos/{repo}/pulls                 -> PR abierto
//!
//! Autoridad (docs/CONTRACT.md): este cliente NUNCA aprueba, fusiona ni
//! declara CI PASS. Solo crea la rama, sube el commit acotado y abre el
//! PR; VERIFY vive en GitHub Actions, que reporta a /github/callback.
//!
//! Clasificacion de errores (fail-closed en ambos casos):
//!   - Transient (red caida, 5xx, 429): la cola reintenta con delay.
//!   - Permanent (4xx definitivo, precondicion no cumplida): el incidente
//!     se bloquea con razon; jamas retry infinito.

use serde_json::{json, Value};
use worker::{Fetch, Headers, Method, Request, RequestInit};

use crate::runtime::security::urlencode;

pub const API_BASE: &str = "https://api.github.com";
pub const TOKEN_SECRET: &str = "GITHUB_TOKEN";
const BRANCH_PREFIX: &str = "auto-heal/";
/// Acote del nombre de rama (GitHub permite 255; 63 mantiene el nombre
/// legible en la UI y deja margen al prefijo).
const BRANCH_MAX: usize = 63;

#[derive(Debug)]
pub enum GitHubError {
    /// Transitorio: reintenta la cola (max_retries=3 -> DLQ).
    Transient(worker::Error),
    /// Definitivo: fail-closed, el incidente queda bloqueado con razon.
    Permanent(String),
}

pub struct GitHubClient {
    token: String,
    repo: String,
}

impl GitHubClient {
    /// Fail-closed: sin GITHUB_TOKEN no hay cliente (el llamador bloquea).
    pub fn from_env(env: &worker::Env) -> worker::Result<Self> {
        let token = env.secret(TOKEN_SECRET)?.to_string();
        if token.is_empty() {
            return Err(worker::Error::RustError("GITHUB_TOKEN empty".to_string()));
        }
        Ok(Self {
            token,
            repo: String::new(),
        })
    }

    /// Fija el repositorio objetivo ("owner/name").
    pub fn for_repo(mut self, repo: &str) -> Self {
        self.repo = repo.to_string();
        self
    }

    fn classify(status: u16, what: &str) -> GitHubError {
        if status >= 500 || status == 429 {
            GitHubError::Transient(worker::Error::RustError(format!(
                "github {} http_{}",
                what, status
            )))
        } else {
            GitHubError::Permanent(format!("github_{}_http_{}", what, status))
        }
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        accept: &str,
        body: Option<String>,
    ) -> Result<(u16, String), GitHubError> {
        let url = format!("{}{}", API_BASE, path);
        let headers = Headers::new();
        headers
            .set("Authorization", &format!("Bearer {}", self.token))
            .map_err(GitHubError::Transient)?;
        headers
            .set("Accept", accept)
            .map_err(GitHubError::Transient)?;
        headers
            .set("User-Agent", "auto-healing-agent")
            .map_err(GitHubError::Transient)?;
        headers
            .set("X-GitHub-Api-Version", "2022-11-28")
            .map_err(GitHubError::Transient)?;
        if body.is_some() {
            headers
                .set("Content-Type", "application/json")
                .map_err(GitHubError::Transient)?;
        }

        let mut init = RequestInit::new();
        init.with_method(method).with_headers(headers);
        if let Some(b) = body {
            init.with_body(Some(worker::wasm_bindgen::JsValue::from_str(&b)));
        }

        let req = Request::new_with_init(&url, &init).map_err(GitHubError::Transient)?;
        let mut resp = Fetch::Request(req)
            .send()
            .await
            .map_err(GitHubError::Transient)?;
        let status = resp.status_code();
        let text = resp.text().await.map_err(GitHubError::Transient)?;
        Ok((status, text))
    }

    async fn request_json(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<(u16, Value), GitHubError> {
        let (status, text) = self
            .request(method, path, "application/vnd.github+json", body)
            .await?;
        let value = serde_json::from_str(&text).unwrap_or(Value::Null);
        Ok((status, value))
    }

    /// Contenido crudo de un archivo en la rama base. None = 404.
    pub async fn get_file(&self, path: &str) -> Result<Option<String>, GitHubError> {
        let p = format!("/repos/{}/contents/{}", self.repo, path);
        let (status, text) = self
            .request(Method::Get, &p, "application/vnd.github.raw", None)
            .await?;
        match status {
            200 => Ok(Some(text)),
            404 => Ok(None),
            s => Err(Self::classify(s, "get_file")),
        }
    }

    /// sha del blob actual (necesario para el PUT del Contents API). None = 404.
    ///
    /// `reference` acota la lectura a la rama de reparacion: en un reintento
    /// tras un put_file exitoso, el sha de la rama BASE esta obsoleto y el PUT
    /// devuelve 409 (sha_mismatch) clasificado como Permanent. El sha de la
    /// RAMA siempre es el vigente: primer intento = sha copiado de la base,
    /// reintento = sha del commit ya subido (mismo contenido, commit idempotente).
    pub async fn get_file_sha(
        &self,
        path: &str,
        reference: &str,
    ) -> Result<Option<String>, GitHubError> {
        let p = format!("/repos/{}/contents/{}?ref={}", self.repo, path, urlencode(reference));
        let (status, value) = self.request_json(Method::Get, &p, None).await?;
        match status {
            200 => Ok(value.get("sha").and_then(|s| s.as_str()).map(String::from)),
            404 => Ok(None),
            s => Err(Self::classify(s, "get_file_sha")),
        }
    }

    pub async fn default_branch(&self) -> Result<String, GitHubError> {
        let (status, value) = self
            .request_json(Method::Get, &format!("/repos/{}", self.repo), None)
            .await?;
        if status != 200 {
            return Err(Self::classify(status, "default_branch"));
        }
        Ok(value
            .get("default_branch")
            .and_then(|b| b.as_str())
            .unwrap_or("main")
            .to_string())
    }

    pub async fn head_sha(&self, branch: &str) -> Result<String, GitHubError> {
        let p = format!("/repos/{}/git/ref/heads/{}", self.repo, branch);
        let (status, value) = self.request_json(Method::Get, &p, None).await?;
        if status != 200 {
            return Err(Self::classify(status, "head_sha"));
        }
        value
            .pointer("/object/sha")
            .and_then(|s| s.as_str())
            .map(String::from)
            .ok_or_else(|| GitHubError::Permanent("head_sha_missing".to_string()))
    }

    /// Rama de reparacion. Idempotente: 422 (ya existe) NO es error.
    pub async fn create_branch(&self, name: &str, from_sha: &str) -> Result<(), GitHubError> {
        let p = format!("/repos/{}/git/refs", self.repo);
        let body = json!({ "ref": format!("refs/heads/{}", name), "sha": from_sha }).to_string();
        let (status, _) = self.request_json(Method::Post, &p, Some(body)).await?;
        match status {
            201 => Ok(()),
            422 => Ok(()), // ya existe: idempotente
            s => Err(Self::classify(s, "create_branch")),
        }
    }

    /// Commit de UN archivo via Contents API (base64 del contenido nuevo).
    pub async fn put_file(
        &self,
        branch: &str,
        path: &str,
        blob_sha: Option<&str>,
        content_b64: &str,
        message: &str,
    ) -> Result<(), GitHubError> {
        let p = format!("/repos/{}/contents/{}", self.repo, path);
        let body = json!({
            "message": message,
            "content": content_b64,
            "branch": branch,
            "sha": blob_sha,
        })
        .to_string();
        let (status, _) = self.request_json(Method::Put, &p, Some(body)).await?;
        match status {
            200 | 201 => Ok(()),
            s => Err(Self::classify(s, "put_file")),
        }
    }

    /// Abre el PR y devuelve su html_url. Nunca lo aprueba ni fusiona.
    pub async fn create_pull_request(
        &self,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, GitHubError> {
        let p = format!("/repos/{}/pulls", self.repo);
        let body = json!({
            "title": title,
            "head": head,
            "base": base,
            "body": body,
            "maintainer_can_modify": false,
        })
        .to_string();
        let (status, value) = self.request_json(Method::Post, &p, Some(body)).await?;
        match status {
            201 => value
                .get("html_url")
                .and_then(|u| u.as_str())
                .map(String::from)
                .ok_or_else(|| GitHubError::Permanent("pr_url_missing".to_string())),
            // Idempotencia en reintentos: 422 con el PR ya abierto (put_file
            // exitoso + create_pull_request fallido y reintentado). Se
            // recupera el html_url del PR existente en vez de clasificar el
            // 422 como Permanent y bloquear el incidente por nada.
            422 => {
                let head = format!("{}:{}", self.repo, head);
                let (list_status, list) = self
                    .request_json(
                        Method::Get,
                        &format!("/repos/{}/pulls?head={}&state=open", self.repo, urlencode(&head)),
                        None,
                    )
                    .await?;
                if list_status == 200 {
                    let url = list
                        .as_array()
                        .and_then(|prs| prs.first())
                        .and_then(|pr| pr.get("html_url"))
                        .and_then(|u| u.as_str());
                    if let Some(url) = url {
                        return Ok(url.to_string());
                    }
                }
                Err(GitHubError::Permanent("pr_already_open".to_string()))
            }
            s => Err(Self::classify(s, "create_pull_request")),
        }
    }

    /// Orquesta la reparacion completa: rama -> commit -> PR.
    pub async fn open_repair_pr(
        &self,
        correlation_id: &str,
        file_path: &str,
        new_content: &str,
        title: &str,
        pr_body: &str,
        commit_message: &str,
    ) -> Result<String, GitHubError> {
        let branch = branch_name(correlation_id);
        let base = self.default_branch().await?;
        let sha = self.head_sha(&base).await?;
        self.create_branch(&branch, &sha).await?;
        // sha desde la RAMA de reparacion (no la base): reintentos tras un
        // put_file exitoso leen el sha vigente y no fallan con 409.
        let blob_sha = self.get_file_sha(file_path, &branch).await?;
        self.put_file(
            &branch,
            file_path,
            blob_sha.as_deref(),
            &base64_std(new_content.as_bytes()),
            commit_message,
        )
        .await?;
        self.create_pull_request(&branch, &base, title, pr_body)
            .await
    }
}

/// auto-heal/{correlation_id}, sanitizado y acotado a BRANCH_MAX.
fn branch_name(correlation_id: &str) -> String {
    let sanitized: String = correlation_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let mut name = format!("{}{}", BRANCH_PREFIX, sanitized);
    name.truncate(BRANCH_MAX);
    name
}

/// Base64 estandar (con padding) sin dependencias: el Contents API exige
/// contenido base64 y no hay atob/btoa en el crate worker.
fn base64_std(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_name_is_prefixed_sanitized_and_bounded() {
        assert_eq!(branch_name("inc1-a1b2c3"), "auto-heal/inc1-a1b2c3");
        assert_eq!(branch_name("a b/c"), "auto-heal/a-b-c");
        let long = "x".repeat(200);
        assert!(branch_name(&long).len() <= BRANCH_MAX);
    }

    #[test]
    fn base64_std_matches_rfc4648_vectors() {
        assert_eq!(base64_std(b""), "");
        assert_eq!(base64_std(b"f"), "Zg==");
        assert_eq!(base64_std(b"fo"), "Zm8=");
        assert_eq!(base64_std(b"foo"), "Zm9v");
        assert_eq!(base64_std(b"foobar"), "Zm9vYmFy");
    }
}
