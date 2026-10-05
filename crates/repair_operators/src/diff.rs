//! Deterministic unified-diff generation for repair operators.
//!
//! Step 1 of the WASM auto-repair path (IMPLEMENTATION_PROGRESS.md): turn an
//! allowlisted RepairAction into a unified diff string, fully inside the
//! Rust/WASM boundary. No free-form codegen: every operator maps to one
//! bounded, deterministic edit over content the caller supplies. The worker
//! (queue_consumer) owns all I/O: it fetches file contents and pushes the
//! diff to GitHub; this crate never performs network or filesystem I/O.
//!
//! Policy invariants (CONTRACT.md / NO_LLM_POLICY.md):
//! - Advisory operators (NoOp, Unknown, EnvVarRepair, CacheClear) and
//!   LockfileRefresh (no deterministic in-WASM renderer) never emit a diff.
//! - Edits are bounded: files above MAX_FILE_BYTES are refused.
//! - DependencyRepair / VersionPin never cross a major-version boundary.
//! - Output is a single-hunk unified diff, deterministic (no timestamps).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use repair_types::{Incident, OperatorId, RepairAction};

/// Bounded edits only: refuse files above this size (256 KiB).
pub const MAX_FILE_BYTES: usize = 256 * 1024;
/// Context lines around the changed region in the unified diff.
pub const CONTEXT_LINES: usize = 3;

/// One bounded file edit: the diff is computed from before/after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEdit {
    pub path: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffError {
    /// Operator never emits a diff by policy (advisory or unrenderable).
    UnsupportedOperator(OperatorId),
    /// Missing or empty required parameter.
    MissingParam(&'static str),
    /// The caller did not supply the target file content.
    FileUnavailable(String),
    /// File exceeds MAX_FILE_BYTES: refuse, never truncate silently.
    FileTooLarge(String),
    /// The pa
ttern to replace does not exist in the file.
    PatternNotFound(&'static str),
    /// Version bump would cross a major boundary.
    MajorBump {
        dependency: String,
        from: String,
        to: String,
    },
}

/// Render edit as a single-hunk unified diff (deterministic, no timestamps).
///
/// An empty return value means "no textual change": the caller must treat
/// that as a no-op and escalate instead of opening a PR.
pub fn unified_diff(edit: &FileEdit) -> String {
    let before_lines = split_lines(&edit.before);
    let after_lines = split_lines(&edit.after);

    let max = before_lines.len().min(after_lines.len());
    let mut prefix = 0usize;
    while prefix < max && before_lines[prefix] == after_lines[prefix] {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < max - prefix
        && before_lines[before_lines.len() - 1 - suffix]
            == after_lines[after_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let old_mid = &before_lines[prefix..before_lines.len() - suffix];
    let new_mid = &after_lines[prefix..after_lines.len() - suffix];
    if old_mid.is_empty() && new_mid.is_empty() {
        return String::new(); // no textual change
    }

    let ctx_before = CONTEXT_LINES.min(prefix);
    let ctx_after = CONTEXT_LINES.min(suffix);
    let start = prefix - ctx_before; // zero-based first context line

    let old_count = ctx_before + old_mid.len() + ctx_after;
    let new_count = ctx_before + new_mid.len() + ctx_after;

    let mut out = format!("--- a/{}\n+++ b/{}\n", edit.path, edit.path);
    out.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        start + 1,
        old_count,
        start + 1,
        new_count
    ));
    for line in &before_lines[start..prefix] {
        out.push(' ');
        out.push_str(line);
        out.push('\n');
    }
    for line in old_mid {
        out.push('-');
        out.push_str(line);
        out.push('\n');
    }
    for line in new_mid {
    
    out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    for line in &after_lines[prefix..prefix + ctx_after] {
        out.push(' ');
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Deterministic bounded edit for one allowlisted RepairAction.
///
/// fetch supplies the CURRENT content of a repo-relative path (the worker
/// reads it from GitHub; this crate stays I/O-free).
pub fn generate_edit<F>(
    action: &RepairAction,
    _incident: &Incident,
    fetch: F,
) -> Result<String, DiffError>
where
    F: Fn(&str) -> Option<String>,
{
    match action.repair_operator {
        // Advisory or unrenderable: never emit a diff (policy gate).
        OperatorId::NoOp
        | OperatorId::Unknown
        | OperatorId::EnvVarRepair
        | OperatorId::CacheClear
        | OperatorId::LockfileRefresh => Err(DiffError::UnsupportedOperator(action.repair_operator)),

        OperatorId::DependencyRepair | OperatorId::VersionPin => {
            let dep = param(action, "dependency")?;
            let version = param(action, "version")?;
            let before = fetch_file(fetch, "package.json")?;
            let exact_pin = action.repair_operator == OperatorId::VersionPin;
            let after = version_bump(&before, &dep, &version, exact_pin)?;
            Ok(FileEdit {
                path: String::from("package.json"),
                before,
                after,
            })
        }

        // Bounded deterministic edit inside one allowlisted node: the
        // parameters name the file and the exact from -> to substitution.
        OperatorId::SyntaxFix
        | OperatorId::ConfigRepair
        | OperatorId::BuildScriptFix
        | OperatorId::ImportPathFix
        | OperatorId::TypeAnnotationFix
        | OperatorId::TestRepair
        | OperatorId::Sour
ceRepair => {
            let path = param(action, "file")?;
            let from = param(action, "from")?;
            let to = param(action, "to")?;
            if from == to {
                return Err(DiffError::MissingParam("from != to"));
            }
            let before = fetch_file(fetch, &path)?;
            if !before.contains(&from) {
                return Err(DiffError::PatternNotFound("from"));
            }
            let after = before.replace(&from, &to);
            Ok(FileEdit { path, before, after })
        }
    }
}

/// Unified diff for one allowlisted RepairAction.
///
/// Ok("") = textual no-op: the caller must escalate instead of opening a PR.
pub fn generate_diff<F>(
    action: &RepairAction,
    incident: &Incident,
    fetch: F,
) -> Result<String, DiffError>
where
    F: Fn(&str) -> Option<String>,
{
    generate_edit(action, incident, fetch).map(|edit| unified_diff(&edit))
}

fn fetch_file<F>(fetch: F, path: &str) -> Result<String, DiffError>
where
    F: Fn(&str) -> Option<String>,
{
    let content = fetch(path).ok_or_else(|| DiffError::FileUnavailable(path.to_string()))?;
    if content.len() > MAX_FILE_BYTES {
        return Err(DiffError::FileTooLarge(path.to_string()));
    }
    Ok(content)
}

fn param(action: &RepairAction, key: &'static str) -> Result<String, DiffError> {
    action
        .parameters
        .get(key)
        .filter(|v| !v.is_empty())
        .cloned()
        .ok_or(DiffError::MissingParam(key))
}

/// Rewrite one dependency entry of a package.json body.
///
/// Refuses major bumps (DependencyRepair stays within existing ranges;
/// VersionPin may pin within the same major only). exact_pin drops ^/~
/// and writes the bare version.
fn version_bump(
    before: &str,
    dep: &str,
    new_version: &str,
    exact_pin: bool,
) -> Result<String, DiffError> {
    let needle = format!("\"{}\": \"", dep);
    let start = before
        .find(&needle)
        .ok_or(DiffError::PatternNotFound("dependency"))?;
    let vstart = start + needle.len();
    let vend = before[vstart..]
        .find('"')
        .map(|i| vstart + i)
        .ok_or(DiffError::PatternNotFound("version"))?;
    let raw_old = &before[vstart..vend];

    let old_clean = raw_old.trim_start_matches(|c: char| !c.is_ascii_digit());
    let old_major = major_of(old_clean).ok_or(DiffError::Pat
ternNotFound("version"))?;
    let new_major = major_of(new_version).ok_or(DiffError::MissingParam("version"))?;
    if new_major > old_major {
        return Err(DiffError::MajorBump {
            dependency: dep.to_string(),
            from: raw_old.to_string(),
            to: new_version.to_string(),
        });
    }

    let prefix = if exact_pin { "" } else { range_prefix(raw_old) };
    let mut out = String::with_capacity(before.len());
    out.push_str(&before[..vstart]);
    out.push_str(prefix);
    out.push_str(new_version);
    out.push_str(&before[vend..]);
    Ok(out)
}

/// Keep the existing range marker (^ or ~) so a DependencyRepair never
/// widens the declared range semantics.
fn range_prefix(raw: &str) -> &'static str {
    match raw.chars().next() {
        Some('^') => "^",
        Some('~') => "~",
        _ => "",
    }
}

fn major_of(v: &str) -> Option<u32> {
    let digits: String = v.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn split_lines(s: &str) -> Vec<&str> {
    let mut v: Vec<&str> = s.split('\n').collect();
    // A single trailing newline is a terminator, not an extra empty line.
    if v.len() > 1 && v[v.len() - 1].is_empty() {
        v.pop();
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    fn action(op: OperatorId, params: &[(&str, &str)]) -> RepairAction {
        let mut parameters = BTreeMap::new();
        for (k, v) in params {
            parameters.insert(String::from(*k), String::from(*v));
        }
        RepairAction {
            node_id: String::from("n1"),
            repair_operator: op,
            parameters,
            confidence: 0.9,
            risk: 0.1,
        }
    }

    const PKG: &str = "{\n  \"name\": \"demo\",\n  \"dependencies\": {\n    \"left-pad\": \"^1.3.0\"\n  }\n}\n";

    fn fetch_map(files: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + '_ {
        move |path| {
            files
         
       .iter()
                .find(|(p, _)| *p == path)
                .map(|(_, c)| c.to_string())
        }
    }

    #[test]
    fn unified_diff_emits_single_hunk_with_headers() {
        let edit = FileEdit {
            path: String::from("src/a.ts"),
            before: String::from("l1\nl2\nl3\nl4\nl5\nl6\nl7\n"),
            after: String::from("l1\nl2\nX\nl4\nl5\nl6\nl7\n"),
        };
        let d = unified_diff(&edit);
        assert!(d.starts_with("--- a/src/a.ts\n+++ b/src/a.ts\n"));
        assert!(d.contains("@@ -1,7 +1,7 @@\n"));
        assert!(d.contains("-l3\n+X\n"));
    }

    #[test]
    fn unified_diff_is_empty_when_no_change() {
        let edit = FileEdit {
            path: String::from("a"),
            before: String::from("x\n"),
            after: String::from("x\n"),
        };
        assert_eq!(unified_diff(&edit), String::new());
    }

    #[test]
    fn import_path_fix_rewrites_specifier() {
        let files = [("src/main.ts", "import x from \"../old/path\";\n")];
        let a = action(
            OperatorId::ImportPathFix,
            &[
                ("file", "src/main.ts"),
                ("from", "../old/path"),
                ("to", "../new/path"),
            ],
        );
        let d = generate_diff(&a, &Incident::default(), fetch_map(&files)).unwrap();
        assert!(d.contains("-import x from \"../old/path\";"));
        assert!(d.contains("+import x from \"../new/path\";"));
    }

    #[test]
    fn dependency_repair_within_same_major_is_allowed() {
        let a = action(
            OperatorId::DependencyRepair,
            &[("dependency", "left-pad"), ("version", "1.4.2")],
        );
        let d = generate_diff(&a, &Incident::default(), fetch_map(&[("package.json", PKG)]))
            .unwrap();
        assert!(d.contains("-    \"left-pad\": \"^1.3.0\""));
        assert!(d.contains("+    \"left-pad\": \"^1.4.2\""));
    }

    #[test]
    fn version_pin_drops_range_prefix() {
        let a = actio
n(
            OperatorId::VersionPin,
            &[("dependency", "left-pad"), ("version", "1.3.0")],
        );
        let d = generate_diff(&a, &Incident::default(), fetch_map(&[("package.json", PKG)]))
            .unwrap();
        assert!(d.contains("+    \"left-pad\": \"1.3.0\""));
    }

    #[test]
    fn major_bump_is_refused() {
        let a = action(
            OperatorId::DependencyRepair,
            &[("dependency", "left-pad"), ("version", "2.0.0")],
        );
        let err = generate_diff(
            &a,
            &Incident::default(),
            fetch_map(&[("package.json", PKG)]),
        )
        .unwrap_err();
        assert!(matches!(err, DiffError::MajorBump { .. }));
    }

    #[test]
    fn advisory_operators_never_emit_diffs() {
        for op in [
            OperatorId::NoOp,
            OperatorId::Unknown,
            OperatorId::EnvVarRepair,
            OperatorId::CacheClear,
            OperatorId::LockfileRefresh,
        ] {
            let a = action(op, &[]);
            let err = generate_diff(
                &a,
                &Incident::default(),
                fetch_map(&[("package.json", PKG)]),
            )
            .unwrap_err();
            assert_eq!(err, DiffError::UnsupportedOperator(op));
        }
    }

    #[test]
    fn missing_param_and_missing_pattern_fail_closed() {
        let a = action(OperatorId::ImportPathFix, &[("file", "src/a.ts")]);
        let err = generate_diff(&a, &Incident::default(), fetch_map(&[("src/a.ts", "x\n")]))
            .unwrap_err();
        assert_eq!(err, DiffError::MissingParam("from"));

        let a = action(
            OperatorId::ImportPathFix,
            &[("file", "src/a.ts"), ("from", "nope"), ("to", "yes")],
        );
        let err = generate_diff(&a, &Incident::default(), fetch_map(&[("src/a.ts", "x\n")]))
            .unwrap_err();
        assert_eq!(err, DiffError::PatternNotFound("from"));
    }

    #[test]
    fn oversized_file_is_refused() {

        let big = "x".repeat(MAX_FILE_BYTES + 1);
        let a = action(
            OperatorId::SourceRepair,
            &[("file", "big.txt"), ("from", "x"), ("to", "y")],
        );
        let err = generate_diff(&a, &Incident::default(), fetch_map(&[("big.txt", big.as_str())]))
            .unwrap_err();
        assert!(matches!(err, DiffError::FileTooLarge(_)));
    }
}
