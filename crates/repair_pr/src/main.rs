//! `repair-pr` CLI — GATE -> PR bridge, version 1.
//!
//! Two modes, both fail-closed:
//!
//! - `repair-pr diff <changes.json>`
//!   Offline evidence: prints the unified diff bundle for
//!   `{"files": [{"path", "before", "after"}]}` and exits 2 (BLOCKED)
//!   when the bundle is empty. No repository, network, or token needed.
//!
//! - `repair-pr pr <request.json> --repo owner/name [--repo-dir DIR]`
//!   Real repair path: `request.json` carries the `OutboundPRRequest`
//!   (patch without diff, `has_diff_generator: false`) plus
//!   `{"files": [{"path", "after"}]}`. The CLI prepares the ephemeral
//!   branch from the base ref, reads each file's CURRENT on-disk content as
//!   the diff baseline (a caller cannot inject a fake "before"), generates
//!   the bundle with `similar`, commits, pushes, and opens the PR with
//!   `octocrab`. Requires `GITHUB_TOKEN` with the least-privilege
//!   permissions of CONTRACT.md §7.
//!
//! Exit codes: 0 = bundle/PR produced; 2 = BLOCKED or fail-closed error.
//! The CLI never declares a repair PASS: that is GitHub Actions' verdict
//! (CONTRACT.md §4).

use std::path::Path;
use std::process::Command;
use std::process::ExitCode;

use repair_pr::{attach_bundle, bundle_is_empty, client_from_env, open_pr, patch_bundle, FileChange};
use repair_types::OutboundPRRequest;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("repair-pr: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("diff") if args.len() == 2 => cmd_diff(&args[1]),
        Some("pr") if args.len() >= 3 => cmd_pr(&args[1], &args[2..]),
        _ => {
            eprintln!(
                "usage: repair-pr <diff|pr> <input.json> [--repo owner/name] [--repo-dir DIR]"
            );
            Ok(ExitCode::from(2))
        }
    }
}

#[derive(serde::Deserialize)]
struct DiffInput {
    files: Vec<FileChange>,
}

#[derive(serde::Deserialize)]
struct FileAfter {
    path: String,
    after: String,
}

#[derive(serde::Deserialize)]
struct PrInput {
    request: OutboundPRRequest,
    files: Vec<FileAfter>,
}

fn cmd_diff(input_path: &str) -> Result<ExitCode, String> {
    let input: DiffInput = read_json(input_path)?;
    let bundle = patch_bundle(&input.files);
    if bundle_is_empty(&bundle) {
        eprintln!("repair-pr: BLOCKED - empty diff bundle (CONTRACT.md §3: no invented diffs)");
        return Ok(ExitCode::from(2));
    }
    print!("{bundle}");
    Ok(ExitCode::SUCCESS)
}

fn cmd_pr(input_path: &str, rest: &[String]) -> Result<ExitCode, String> {
    let owner_repo = flag_value(rest, "--repo")?
        .ok_or_else(|| "missing --repo owner/name".to_string())?;
    let (owner, repo) = owner_repo
        .split_once('/')
        .ok_or_else(|| format!("--repo must be owner/name, got {owner_repo}"))?;
    let repo_dir = flag_value(rest, "--repo-dir")?.unwrap_or(".");

    let mut input: PrInput = read_json(input_path)?;
    let base = input.request.base_branch.clone();
    let head = input.request.head_branch.clone();

    // 1. Ephemeral branch from the base ref (fail-closed on git errors).
    prepare_branch(repo_dir, &base, &head)?;

    // 2. Real baseline: the CURRENT on-disk content of the freshly checked
    //    out base branch. The caller cannot inject a fake "before".
    let mut changes = Vec::with_capacity(input.files.len());
    for file in &input.files {
        let before = std::fs::read_to_string(Path::new(repo_dir).join(&file.path))
            .map_err(|e| format!("cannot read {} at base ref: {e}", file.path))?;
        changes.push(FileChange {
            path: file.path.clone(),
            before,
            after: file.after.clone(),
        });
    }

    // 3. Real diff (CONTRACT.md §3): empty bundle => BLOCKED, no commit, no
    //    PR. Nothing has been written to the working tree at this point.
    let bundle = patch_bundle(&changes);
    if bundle_is_empty(&bundle) {
        eprintln!("repair-pr: BLOCKED - no real change in the given files (CONTRACT.md §3)");
        return Ok(ExitCode::from(2));
    }
    attach_bundle(&mut input.request.patch, &bundle);

    // 4. Write the operator output, commit, push the ephemeral branch.
    commit_files(repo_dir, &changes, &input.request.incident_id, &head)?;

    // 5. Open the PR (octocrab). An open PR for the same head branch
    //    returns the existing one (CONTRACT.md §5).
    let octocrab = client_from_env().map_err(|e| e.to_string())?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| format!("tokio runtime: {e}"))?;
    let outcome = runtime
        .block_on(open_pr(&octocrab, owner, repo, &input.request, &bundle))
        .map_err(|e| e.to_string())?;
    if outcome.duplicate {
        println!("existing PR #{}: {}", outcome.number, outcome.url);
    } else {
        println!("PR #{}: {}", outcome.number, outcome.url);
    }
    Ok(ExitCode::SUCCESS)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let raw =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("invalid JSON in {path}: {e}"))
}

fn flag_value<'a>(args: &'a [String], name: &str) -> Result<Option<&'a str>, String> {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == name {
            return match it.next() {
                Some(value) => Ok(Some(value.as_str())),
                None => Err(format!("{name} needs a value")),
            };
        }
    }
    Ok(None)
}

fn prepare_branch(repo_dir: &str, base: &str, head: &str) -> Result<(), String> {
    run_git(
        repo_dir,
        &["fetch".to_string(), "origin".to_string(), base.to_string()],
    )?;
    run_git(
        repo_dir,
        &[
            "checkout".to_string(),
            "-B".to_string(),
            head.to_string(),
            format!("origin/{base}"),
        ],
    )
}

fn commit_files(
    repo_dir: &str,
    changes: &[FileChange],
    incident: &str,
    head: &str,
) -> Result<(), String> {
    for change in changes {
        std::fs::write(Path::new(repo_dir).join(&change.path), change.after.as_bytes())
            .map_err(|e| format!("cannot write {}: {e}", change.path))?;
    }
    let mut add = vec!["add".to_string()];
    add.extend(changes.iter().map(|c| c.path.clone()));
    run_git(repo_dir, &add)?;
    run_git(
        repo_dir,
        &[
            "-c".to_string(),
            "user.name=auto-repair".to_string(),
            "-c".to_string(),
            "user.email=auto-repair@users.noreply.github.com".to_string(),
            "commit".to_string(),
            "-m".to_string(),
            format!("repair {incident}: deterministic operator patch"),
        ],
    )?;
    run_git(
        repo_dir,
        &[
            "push".to_string(),
            "origin".to_string(),
            format!("HEAD:refs/heads/{head}"),
        ],
    )
}

fn run_git(repo_dir: &str, args: &[String]) -> Result<(), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("git {args:?}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}
