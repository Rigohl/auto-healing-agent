//! Feature extraction: Incident → fixed 16-dim FeatureVector.
//!
//! Pure, deterministic, no allocations beyond the input strings.
//! Designed for edge / WASM (no_std + alloc).

#![cfg_attr(not(feature = "std"), no_std)]

use repair_types::{FeatureVector, Incident};

/// Feature indices (documented contract with the model).
///
///  0  error_code_hash          (normalized [0,1])
///  1  error_step_hash
///  2  command_hash
///  3  language_hint            (one-hot-ish)
///  4  framework_hint
///  5  attempts_norm            (attempts / 10, capped)
///  6  message_len_norm
///  7  has_syntax_token
///  8  has_dependency_token
///  9  has_config_token
/// 10  has_build_token
/// 11  has_env_token
/// 12  has_type_token
/// 13  has_import_token
/// 14  source_is_vercel
/// 15  source_is_linear
pub fn extract(incident: &Incident) -> FeatureVector {
    let mut v = FeatureVector::zeros();

    v.values[0] = hash01(&incident.error_code);
    v.values[1] = hash01(&incident.error_step);
    v.values[2] = hash01(&incident.command);
    v.values[3] = language_score(&incident.language_hint);
    v.values[4] = framework_score(&incident.framework_hint);
    v.values[5] = (incident.attempts as f32 / 10.0).min(1.0);
    v.values[6] = (incident.message.len() as f32 / 2000.0).min(1.0);

    let msg = incident.message.to_lowercase();
    let code = incident.error_code.to_lowercase();
    let step = incident.error_step.to_lowercase();
    let combined = alloc::format!("{} {} {}", msg, code, step);

    v.values[7] = contains_any(&combined, &["syntax", "parse", "unexpected token", "ts(", "eslint"]);
    v.values[8] = contains_any(&combined, &["depend", "module not found", "cannot find module", "npm err", "yarn", "pnpm"]);
    v.values[9] = contains_any(&combined, &["config", "tsconfig", "next.config", "vercel.json", "env"]);
    v.values[10] = contains_any(&combined, &["build", "compile", "webpack", "vite", "turbo"]);
    v.values[11] = contains_any(&combined, &["env", "environment", "process.env", "missing env"]);
    v.values[12] = contains_any(&combined, &["type", "typescript", "cannot assign", "property does not exist"]);
    v.values[13] = contains_any(&combined, &["import", "export", "require(", "cannot resolve"]);

    let src = incident.source.to_lowercase();
    v.values[14] = if src.contains("vercel") { 1.0 } else { 0.0 };
    v.values[15] = if src.contains("linear") { 1.0 } else { 0.0 };

    v
}

fn hash01(s: &str) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    (h as f32) / (u32::MAX as f32)
}

fn language_score(hint: &str) -> f32 {
    let h = hint.to_lowercase();
    if h.contains("typescript") || h.contains("ts") {
        0.9
    } else if h.contains("javascript") || h.contains("js") {
        0.7
    } else if h.contains("rust") {
        0.5
    } else if h.contains("python") {
        0.3
    } else {
        0.1
    }
}

fn framework_score(hint: &str) -> f32 {
    let h = hint.to_lowercase();
    if h.contains("next") {
        0.95
    } else if h.contains("react") {
        0.8
    } else if h.contains("vue") || h.contains("nuxt") {
        0.6
    } else if h.contains("svelte") {
        0.5
    } else {
        0.2
    }
}

fn contains_any(hay: &str, needles: &[&str]) -> f32 {
    for n in needles {
        if hay.contains(n) {
            return 1.0;
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use repair_types::Incident;

    #[test]
    fn extracts_syntax_features() {
        let mut inc = Incident::default();
        inc.error_code = "syntax_error".into();
        inc.error_step = "buildStep".into();
        inc.command = "npm run vercel-build".into();
        inc.message = "Unexpected token".into();
        inc.source = "vercel".into();
        inc.attempts = 3;

        let fv = extract(&inc);
        assert!(fv.values[7] > 0.5, "syntax token should fire");
        assert!(fv.values[14] > 0.5, "vercel source");
        assert!((fv.values[5] - 0.3).abs() < 0.01);
    }
}
