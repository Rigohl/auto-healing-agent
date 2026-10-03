//! PR opener for the GATE -> PR bridge (CONTRACT.md §3/§5).
//!
//! Fail-closed by construction:
//! - no token => `MissingToken` (credentials are never guessed or logged);
//! - empty diff bundle => `Blocked` (CONTRACT.md §3: NO INVENTED DIFFS);
//! - a blocked `CandidatePatch` never opens a PR (`is_actionable`).
//!
//! Duplicate handling implements `DuplicatePRBehavior::ReturnExisting`: if
//! an open PR already exists for the incident's head branch, it is returned
//! instead of opening a second one. VERIFY authority stays with GitHub
//! Actions (CONTRACT.md §4): this module only opens PRs, never declares
//! PASS.

use std::fmt;

use octocrab::params::State;
use secrecy::SecretString;

use repair_types::OutboundPRRequest;

/// Outcome of an `open_pr` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrOutcome {
    pub number: u64,
    pub url: String,
    /// True when the PR already existed (`ReturnExisting`, CONTRACT.md §5).
    pub duplicate: bool,
}

/// Every way the bridge can refuse to open a PR. All of them fail closed.
#[derive(Debug)]
pub enum PrError {
    /// CONTRACT.md §3: no real diff content; pipeline status is BLOCKED.
    Blocked(String),
    /// `GITHUB_TOKEN` absent or empty: no credentials are ever guessed.
    MissingToken,
    /// The patch is blocked by policy; PRs are only for actionable patches.
    NotActionable(String),
    /// GitHub API error (network, permissions, rate limit). The retry
    /// matrix of CONTRACT.md §6 applies; this crate does not retry by
    /// itself.
    Api(octocrab::Error),
}

impl fmt::Display for PrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrError::Blocked(msg) => write!(f, "BLOCKED: {msg}"),
            PrError::MissingToken => {
                write!(f, "GITHUB_TOKEN is not set; refusing to run without credentials")
            }
            PrError::NotActionable(msg) => write!(f, "patch not actionable: {msg}"),
            PrError::Api(err) => write!(f, "GitHub API error: {err}"),
        }
    }
}

impl std::error::Error for PrError {}

impl From<octocrab::Error> for PrError {
    fn from(err: octocrab::Error) -> Self {
        PrError::Api(err)
    }
}

/// Build the API client from the `GITHUB_TOKEN` environment variable.
///
/// The token is read here and wrapped into a `SecretString`; it is never
/// printed, logged, or embedded in URLs (CONTRACT.md §7).
pub fn client_from_env() -> Result<octocrab::Octocrab, PrError> {
    let token = std::env::var("GITHUB_TOKEN").map_err(|_| PrError::MissingToken)?;
    if token.trim().is_empty() {
        return Err(PrError::MissingToken);
    }
    let octocrab = octocrab::Octocrab::builder()
        .personal_token(SecretString::new(token))
        .build()?;
    Ok(octocrab)
}

/// Open the repair PR for `request`, embedding the real diff bundle in the
/// body. See the module docs for the fail-closed order of checks.
pub async fn open_pr(
    octocrab: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    request: &OutboundPRRequest,
    diff_bundle: &str,
) -> Result<PrOutcome, PrError> {
    if diff_bundle.trim().is_empty() {
        return Err(PrError::Blocked(
            "empty diff bundle: no invented diffs, no PR (CONTRACT.md §3)".to_string(),
        ));
    }
    if !request.is_actionable() {
        return Err(PrError::NotActionable(format!(
            "incident {}: patch.is_blocked() is true",
            request.incident_id
        )));
    }
    // CONTRACT.md §5 (ReturnExisting): an open PR for the same head branch
    // is returned as-is instead of opening a duplicate.
    let existing = octocrab
        .pulls(owner, repo)
        .list()
        .state(State::Open)
        .head(format!("{}:{}", owner, request.head_branch))
        .send()
        .await?;
    if let Some(pr) = existing.items.into_iter().next() {
        return Ok(PrOutcome {
            number: pr.number,
            url: pr.html_url.to_string(),
            duplicate: true,
        });
    }
    let pr = octocrab
        .pulls(owner, repo)
        .create(&request.title, &request.head_branch, &request.base_branch)
        .body(pr_body(request, diff_bundle))
        .send()
        .await?;
    Ok(PrOutcome {
        number: pr.number,
        url: pr.html_url.to_string(),
        duplicate: false,
    })
}

/// PR body: operator summary + metadata + the real unified diff.
///
/// The body never claims PASS: it states that GitHub Actions is the verify
/// authority (CONTRACT.md §4).
fn pr_body(request: &OutboundPRRequest, diff_bundle: &str) -> String {
    let mut body = String::new();
    body.push_str("## ");
    body.push_str(&request.patch.patch_summary);
    body.push_str("\n\n");
    body.push_str(&request.body);
    body.push_str("\n\n### Unified diff (generated from real file contents)\n\n\x60\x60\x60diff\n");
    body.push_str(diff_bundle.trim_end());
    body.push_str("\n\x60\x60\x60\n\n### Metadata\n");
    body.push_str(&format!("- incident_id: {}\n", request.incident_id));
    body.push_str(&format!("- correlation_id: {}\n", request.correlation_id));
    body.push_str(&format!("- operator_id: {}\n", request.patch.operator_id));
    if !request.labels.is_empty() {
        body.push_str(&format!("- labels: {}\n", request.labels.join(", ")));
    }
    for (key, value) in &request.metadata {
        body.push_str(&format!("- {}: {}\n", key, value));
    }
    body.push_str("\nVERIFY authority: GitHub Actions. This PR is never auto-declared PASS.\n");
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use repair_types::contract::CandidatePatch;

    fn test_client() -> octocrab::Octocrab {
        octocrab::Octocrab::builder()
            .personal_token(SecretString::new("x".to_string()))
            .build()
            .expect("client construction is offline")
    }

    fn request() -> OutboundPRRequest {
        OutboundPRRequest {
            incident_id: "inc-test".to_string(),
            correlation_id: "corr-1".to_string(),
            head_branch: "auto-repair/inc-test".to_string(),
            base_branch: "main".to_string(),
            title: "[Auto-Repair] test".to_string(),
            body: "operator output".to_string(),
            patch: CandidatePatch {
                operator_id: 1,
                parameters: Default::default(),
                patch_summary: "dependency repair".to_string(),
                diff: Some("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n".to_string()),
                has_diff_generator: true,
            },
            labels: vec!["auto-repair".to_string()],
            metadata: Default::default(),
        }
    }

    #[tokio::test]
    async fn empty_bundle_is_rejected_before_any_api_call() {
        let client = test_client();
        let err = open_pr(&client, "owner", "repo", &request(), "")
            .await
            .expect_err("an empty bundle must fail closed");
        assert!(matches!(err, PrError::Blocked(_)));
    }

    #[tokio::test]
    async fn blocked_patch_is_rejected_before_any_api_call() {
        let client = test_client();
        let mut req = request();
        req.patch.diff = None;
        let err = open_pr(&client, "owner", "repo", &req, "non-empty-diff\n")
            .await
            .expect_err("a blocked patch must fail closed");
        assert!(matches!(err, PrError::NotActionable(_)));
    }

    #[test]
    fn empty_token_is_fail_closed() {
        std::env::set_var("GITHUB_TOKEN", "");
        assert!(matches!(client_from_env(), Err(PrError::MissingToken)));
        std::env::remove_var("GITHUB_TOKEN");
    }
}
