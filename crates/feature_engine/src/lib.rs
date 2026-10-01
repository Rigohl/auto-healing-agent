//! Feature encoder V0 — [f32; 64], schema v2.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use repair_types::{FailureSignature, FeatureVector, Incident};

pub fn extract(incident: &Incident, signature: &FailureSignature) -> FeatureVector {
    let mut v = FeatureVector::zeros();
    let msg = alloc::format!(
        "{} {} {} {}",
        incident.message.to_lowercase(),
        incident.error_code.to_lowercase(),
        incident.error_step.to_lowercase(),
        signature.fingerprint.to_lowercase()
    );

    v.values[0] = hash01(&incident.error_code);
    v.values[1] = hash01(&incident.error_step);
    v.values[2] = hash01(&incident.command);
    v.values[3] = lang_score(&incident.language_hint);
    v.values[4] = fw_score(&incident.framework_hint);
    v.values[5] = (incident.attempts as f32 / 10.0).min(1.0);
    v.values[6] = (incident.message.len() as f32 / 2000.0).min(1.0);
    v.values[7] = has(&msg, &["syntax", "parse", "unexpected token"]);
    v.values[8] = has(&msg, &["depend", "module not found", "cannot find module"]);
    v.values[9] = has(&msg, &["config", "tsconfig", "vercel.json"]);
    v.values[10] = has(&msg, &["build", "compile", "webpack"]);
    v.values[11] = has(&msg, &["env", "process.env", "missing env"]);
    v.values[12] = has(&msg, &["type", "typescript", "cannot assign"]);
    v.values[13] = has(&msg, &["import", "export", "cannot resolve"]);
    v.values[14] = if incident.source.to_lowercase().contains("vercel") {
        1.0
    } else {
        0.0
    };
    v.values[15] = if incident.source.to_lowercase().contains("linear") {
        1.0
    } else {
        0.0
    };
    v.values[16] = has(&msg, &["test", "jest", "vitest"]);
    v.values[17] = has(&msg, &["lint", "eslint"]);
    v.values[18] = has(&msg, &["timeout", "oom"]);
    v.values[19] = has(&msg, &["permission", "denied"]);
    v.values[20] = hash01(&signature.command_family);
    v.values[21] = hash01(&signature.fingerprint);
    v.values[22] = if incident.verified { 1.0 } else { 0.0 };
    v.values[23] = if incident.status.contains("fail") { 1.0 } else { 0.0 };
    v.values[24] = (incident.stack_hint.len() as f32 / 4000.0).min(1.0);
    v.values[25] = has(&msg, &["lockfile", "package-lock"]);
    v.values[26] = has(&msg, &["peer dep", "eresolve"]);
    v.values[27] = has(&msg, &["version", "unsupported"]);
    v.values[28] = has(&msg, &["cache"]);
    v.values[29] = has(&msg, &["syntax_error"]);
    v.values[30] = has(&msg, &["buildstep", "build_step", "build step"]);
    v.values[31] = if incident.attempts > 2 { 1.0 } else { 0.0 };

    for i in 0..16 {
        v.values[32 + i] =
            ((hash01(&alloc::format!("{}:{}", i, signature.fingerprint)) - 0.5) * 2.0).abs();
    }

    v.values[55] = v.values[5];
    v.values[60] = FeatureVector::SCHEMA_VERSION as f32 / 10.0;
    v.values[63] = 1.0;
    v
}

fn hash01(s: &str) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(16777619);
    }
    (h as f32) / (u32::MAX as f32)
}

fn lang_score(h: &str) -> f32 {
    let h = h.to_lowercase();
    if h.contains("typescript") || h == "ts" {
        0.9
    } else if h.contains("javascript") {
        0.7
    } else {
        0.1
    }
}

fn fw_score(h: &str) -> f32 {
    let h = h.to_lowercase();
    if h.contains("next") {
        0.95
    } else if h.contains("react") {
        0.8
    } else {
        0.2
    }
}

fn has(hay: &str, needles: &[&str]) -> f32 {
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

    #[test]
    fn dim64() {
        assert_eq!(FeatureVector::DIM, 64);
    }

    #[test]
    fn syntax_fixture() {
        let inc = Incident {
            error_code: "syntax_error".into(),
            error_step: "buildStep".into(),
            command: "npm run vercel-build".into(),
            message: "Unexpected token".into(),
            source: "vercel".into(),
            attempts: 3,
            ..Default::default()
        };
        let sig = FailureSignature::from_incident(&inc);
        let fv = extract(&inc, &sig);
        assert!(fv.values[7] > 0.5);
        assert!(fv.values[14] > 0.5);
        assert!(fv.values[29] > 0.5);
    }
}
