//! Unified diff generation from real file contents.
//!
//! `CandidatePatch` parameters are NOT a git diff (CONTRACT.md §3). The only
//! honest generator diffs the actual before/after contents of each file:
//! identical contents produce an EMPTY diff, and an empty bundle must leave
//! the pipeline BLOCKED (never padded, never invented).

use repair_types::contract::CandidatePatch;
use similar::TextDiff;

/// One file snapshot: repo-relative path plus the real contents before and
/// after the deterministic operator's edit.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileChange {
    pub path: String,
    pub before: String,
    pub after: String,
}

/// Unified diff for one file with `a/`/`b/` headers and 3 context lines.
///
/// Identical contents return an empty `String`: the caller must treat that
/// as BLOCKED, never as a no-op patch.
pub fn unified_diff_file(change: &FileChange) -> String {
    if change.before == change.after {
        return String::new();
    }
    TextDiff::from_lines(change.before.as_str(), change.after.as_str())
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{}", change.path), &format!("b/{}", change.path))
        .to_string()
}

/// Concatenated unified diffs for every changed file, in input order.
pub fn patch_bundle(changes: &[FileChange]) -> String {
    let mut bundle = String::new();
    for change in changes {
        let file_diff = unified_diff_file(change);
        if !file_diff.is_empty() {
            bundle.push_str(&file_diff);
        }
    }
    bundle
}

/// CONTRACT.md §3: a patch without a real diff is BLOCKED, never actionable.
pub fn bundle_is_empty(bundle: &str) -> bool {
    bundle.trim().is_empty()
}

/// Fill the contract-side `CandidatePatch` with the real diff bundle.
///
/// This is the exact spot where `has_diff_generator` stops being `false`:
/// the flag only flips when the generator actually produced content. An
/// empty bundle keeps the patch blocked (fail-closed).
pub fn attach_bundle(patch: &mut CandidatePatch, bundle: &str) {
    if bundle_is_empty(bundle) {
        // Blocked stays blocked: never invent content to unblock a patch.
        patch.diff = None;
        patch.has_diff_generator = false;
        return;
    }
    patch.diff = Some(bundle.to_string());
    patch.has_diff_generator = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(path: &str, before: &str, after: &str) -> FileChange {
        FileChange {
            path: path.to_string(),
            before: before.to_string(),
            after: after.to_string(),
        }
    }

    fn contract_patch() -> CandidatePatch {
        CandidatePatch {
            operator_id: 1,
            parameters: Default::default(),
            patch_summary: "dependency repair".to_string(),
            diff: None,
            has_diff_generator: false,
        }
    }

    #[test]
    fn identical_contents_produce_an_empty_diff() {
        assert_eq!(unified_diff_file(&change("a.txt", "same\n", "same\n")), "");
        assert!(bundle_is_empty(&patch_bundle(&[change(
            "a.txt", "same\n", "same\n"
        )])));
    }

    #[test]
    fn a_real_change_produces_unified_diff_hunks() {
        let diff = unified_diff_file(&change(
            "package.json",
            "{\n  \"left\": \"1.0.0\"\n}\n",
            "{\n  \"left\": \"1.0.1\"\n}\n",
        ));
        assert!(
            diff.starts_with("--- a/package.json"),
            "unexpected header: {diff}"
        );
        assert!(diff.contains("+++ b/package.json"));
        assert!(diff.contains("@@"));
        assert!(diff.contains("-  \"left\": \"1.0.0\""));
        assert!(diff.contains("+  \"left\": \"1.0.1\""));
    }

    #[test]
    fn bundle_concatenates_file_diffs_in_input_order() {
        let bundle = patch_bundle(&[
            change("one.txt", "x\n", "y\n"),
            change("two.txt", "p\n", "q\n"),
        ]);
        let first = bundle.find("--- a/one.txt").expect("one.txt header");
        let second = bundle.find("--- a/two.txt").expect("two.txt header");
        assert!(first < second);
    }

    #[test]
    fn empty_files_list_is_an_empty_bundle() {
        assert!(bundle_is_empty(&patch_bundle(&[])));
    }

    #[test]
    fn attach_bundle_only_unlocks_with_real_content() {
        let mut patch = contract_patch();
        attach_bundle(&mut patch, "");
        assert!(patch.is_blocked(), "empty bundle must stay blocked");
        assert!(patch.diff.is_none());

        attach_bundle(&mut patch, "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n");
        assert!(!patch.is_blocked(), "a real bundle must unblock the patch");
        assert!(matches!(patch.diff.as_deref(), Some(d) if !d.is_empty()));
    }
}
