//! GATE -> PR bridge, version 1 (docs/CONTRACT.md §3, P2 dependency).
//!
//! `repair_operators::apply()` produces a structured `CandidatePatch`
//! (allowlisted parameters, no free-form source). A valid GitHub PR requires
//! real unified diff content: NO INVENTED DIFFS. This crate is the diff
//! generator the contract demands:
//!
//! 1. `diff` diffs the real before/after file contents with `similar` and
//!    attaches the bundle to `repair_types::contract::CandidatePatch`
//!    (`has_diff_generator = true`).
//! 2. `github` opens the repair PR with `octocrab`, honoring
//!    `DuplicatePRBehavior::ReturnExisting` (CONTRACT.md §5).
//!
//! Fail-closed semantics (CONTRACT.md §3/§6): an empty or missing diff means
//! the pipeline is BLOCKED; the crate never fabricates diff content to
//! unblock it. VERIFY authority stays with GitHub Actions (§4): opening a PR
//! is never a PASS claim.

pub mod diff;
pub mod github;

pub use diff::{attach_bundle, bundle_is_empty, patch_bundle, unified_diff_file, FileChange};
pub use github::{client_from_env, open_pr, PrError, PrOutcome};
