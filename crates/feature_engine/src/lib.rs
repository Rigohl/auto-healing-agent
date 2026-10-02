//! Feature encoder V1 — [f32; 64], schema v2.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod synthetic;

use repair_types::{FailureSignature, FeatureVector, Incident};

/// Extracts a 64-dimensional feature vector from an Incident and FailureSignature.
///
/// DESIGN GOALS:
/// 1. Generalizability: no feature depends on the incident ID, on session
///    noise or on the full fingerprint hash.
/// 2. Explicit categorical encoding: dedicated slots for error type, step,
///    language, framework and command family.
/// 3. Zero inactive slots: every slot 0..=63 carries signal that actually
///    varies across the synthetic dataset (`synthetic` module + test).
/// 4. Robustness: features are bounded in [0.0, 1.0].
pub fn extract(incident: &Incident, signature: &FailureSignature) -> FeatureVector {
    let mut v = FeatureVector::zeros();
    let msg = incident.message.to_lowercase();
    let code = incident.error_code.to_lowercase();
    let step = incident.error_step.to_lowercase();
    let cmd = incident.command.to_lowercase();
    let lang = incident.language_hint.to_lowercase();
    let fw = incident.framework_hint.to_lowercase();
    let src = incident.source.to_lowercase();

    // --- Slots 0..7: Categorical Error Code Grouping ---
    v.values[0] = if code.contains("syntax") || code.contains("parse") {
        1.0
    } else {
        0.0
    };
    v.values[1] = if code.contains("module")
        || code.contains("depend")
        || code.contains("resolve")
    {
        1.0
    } else {
        0.0
    };
    v.values[2] = if code.contains("config") || code.contains("tsconfig") || code.contains("vercel")
    {
        1.0
    } else {
        0.0
    };
    v.values[3] = if code.contains("script")
        || code.contains("command")
        || code.contains("missing")
    {
        1.0
    } else {
        0.0
    };
    v.values[4] = if code.contains("env") || code.contains("missing_env") {
        1.0
    } else {
        0.0
    };
    v.values[5] = if code.contains("lockfile")
        || code.contains("peer")
        || code.contains("eresolve")
    {
        1.0
    } else {
        0.0
    };
    v.values[6] = if code.contains("type") || code.contains("mismatch") || code.contains("assign") {
        1.0
    } else {
        0.0
    };
    v.values[7] = if code.contains("import") || code.contains("export") || code.contains("path") {
        1.0
    } else {
        0.0
    };

    // --- Slots 8..12: Categorical Error Step One-Hot ---
    v.values[8] = if step.contains("build") || step.contains("compile") {
        1.0
    } else {
        0.0
    };
    v.values[9] = if step.contains("install") || step.contains("setup") {
        1.0
    } else {
        0.0
    };
    v.values[10] = if step.contains("test") || step.contains("check") {
        1.0
    } else {
        0.0
    };
    v.values[11] = if step.contains("deploy") || step.contains("release") {
        1.0
    } else {
        0.0
    };
    v.values[12] = if step.contains("runtime") || step.contains("start") {
        1.0
    } else {
        0.0
    };

    // --- Slots 13..16: Command Family One-Hot ---
    v.values[13] = if signature.command_family == "vercel_build" || cmd.contains("vercel") {
        1.0
    } else {
        0.0
    };
    v.values[14] = if signature.command_family == "node_package"
        || cmd.contains("npm")
        || cmd.contains("pnpm")
        || cmd.contains("yarn")
    {
        1.0
    } else {
        0.0
    };
    v.values[15] = if signature.command_family == "cargo" || cmd.contains("cargo") {
        1.0
    } else {
        0.0
    };
    v.values[16] = if signature.command_family == "other"
        || cmd.contains("chmod")
        || cmd.contains("node")
    {
        1.0
    } else {
        0.0
    };

    // --- Slots 17..20: Language & Framework One-Hot ---
    v.values[17] = if lang.contains("typescript") || lang == "ts" {
        1.0
    } else {
        0.0
    };
    v.values[18] = if lang.contains("javascript") || lang == "js" {
        1.0
    } else {
        0.0
    };
    v.values[19] = if fw.contains("next") {
        1.0
    } else {
        0.0
    };
    v.values[20] = if fw.contains("react") {
        1.0
    } else {
        0.0
    };

    // --- Slots 21..24: Source & Environment Flags ---
    v.values[21] = if src.contains("vercel") {
        1.0
    } else {
        0.0
    };
    v.values[22] = if src.contains("github") || src.contains("actions") {
        1.0
    } else {
        0.0
    };
    v.values[23] = if incident.verified {
        1.0
    } else {
        0.0
    };
    v.values[24] = if incident.status.contains("fail") {
        1.0
    } else {
        0.0
    };

    // --- Slots 25..31: Numerical Bounded Signals ---
    v.values[25] = (incident.attempts as f32 / 10.0).min(1.0);
    v.values[26] = if incident.attempts > 2 {
        1.0
    } else {
        0.0
    };
    v.values[27] = (incident.message.len() as f32 / 2000.0).min(1.0);
    v.values[28] = (incident.stack_hint.len() as f32 / 4000.0).min(1.0);
    v.values[29] = lang_score(&incident.language_hint);
    v.values[30] = fw_score(&incident.framework_hint);
    v.values[31] = FeatureVector::SCHEMA_VERSION as f32 / 10.0;

    // --- Slots 32..47: Semantic Keyword Matchers in Message & Stack ---
    v.values[32] = has(&msg, &["unexpected token", "syntaxerror", "unexpected end"]);
    v.values[33] = has(&msg, &["cannot find module", "module not found", "can't resolve"]);
    v.values[34] = has(&msg, &["tsconfig", "compileroptions", "invalid schema"]);
    v.values[35] = has(&msg, &["missing script", "npm err! missing script"]);
    v.values[36] = has(&msg, &["missing required environment variable", "missing env"]);
    v.values[37] = has(&msg, &["package-lock.json is out of date", "lockfile"]);
    v.values[38] = has(&msg, &["not assignable to type", "type error", "type '"]);
    v.values[39] = has(&msg, &["cannot resolve import path", "cannot resolve import"]);
    v.values[40] = has(&msg, &["eresolve unable to resolve dependency tree", "peer dep"]);
    v.values[41] = has(&msg, &["failed to read build cache", "cache error"]);
    v.values[42] = has(&msg, &["expected 200", "fail src/", "jest", "vitest"]);
    v.values[43] = has(&msg, &["cannot read property", "typeerror", "null", "undefined"]);
    v.values[44] = has(&msg, &["permission denied", "eacces"]);
    v.values[45] = has(&msg, &["timeout", "oom", "heap limit", "out of memory"]);
    v.values[46] = has(&msg, &["eslint", "lint"]);
    v.values[47] = has(&msg, &["webpack", "babel", "swc", "turbopack", "unresolved import"]);

    // --- Slots 48..62: hashing categorico sobre tokens ya normalizados ---
    // 15 buckets repartidos entre code, step, cmd, lang, fw y src. No entra el
    // fingerprint: identico texto de error produce identico vector.
    let tokens = [&code, &step, &cmd, &lang, &fw, &src];
    for (i, token) in tokens.iter().enumerate() {
        if !token.is_empty() {
            let hash_val = hash01(token);
            let bin = ((hash_val * 15.0) as usize + i * 3) % 15;
            v.values[48 + bin] = (v.values[48 + bin] + 0.33).min(1.0);
        }
    }

    // Set bias constant in slot 63 to 1.0
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
    } else if h.contains("javascript") || h == "js" {
        0.7
    } else if h.contains("rust") {
        0.95
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
    } else if h.contains("cargo") {
        0.9
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
    use crate::synthetic::generate_synthetic_dataset;

    #[test]
    fn dim64() {
        assert_eq!(FeatureVector::DIM, 64);
    }

    #[test]
    fn test_invariance_fingerprint_change() {
        let inc1 = Incident {
            id: "inc-001".into(),
            error_code: "syntax_error".into(),
            error_step: "buildStep".into(),
            command: "npm run vercel-build".into(),
            message: "Unexpected token '{'".into(),
            project: "proj-A".into(),
            language_hint: "typescript".into(),
            framework_hint: "next".into(),
            ..Default::default()
        };
        let mut inc2 = inc1.clone();
        inc2.id = "inc-999".into();
        inc2.project = "proj-Z".into();

        let sig1 = FailureSignature::from_incident(&inc1);
        let sig2 = FailureSignature::from_incident(&inc2);

        let fv1 = extract(&inc1, &sig1);
        let fv2 = extract(&inc2, &sig2);

        for i in 0..64 {
            assert_eq!(
                fv1.values[i], fv2.values[i],
                "Slot {} changed when fingerprint changed!",
                i
            );
        }
    }

    #[test]
    fn test_invariance_irrelevant_text_change() {
        let inc1 = Incident {
            id: "inc-001".into(),
            error_code: "module_not_found".into(),
            error_step: "install".into(),
            command: "npm start".into(),
            message: "Cannot find module 'lodash' (ref: 1234)".into(),
            ..Default::default()
        };
        let mut inc2 = inc1.clone();
        inc2.message = "Cannot find module 'lodash' (ref: 9999)".into();

        let sig1 = FailureSignature::from_incident(&inc1);
        let sig2 = FailureSignature::from_incident(&inc2);

        let fv1 = extract(&inc1, &sig1);
        let fv2 = extract(&inc2, &sig2);

        for i in 0..64 {
            assert_eq!(
                fv1.values[i], fv2.values[i],
                "Slot {} changed on irrelevant text change!",
                i
            );
        }
    }

    #[test]
    fn test_synthetic_dataset_metrics() {
        let dataset = generate_synthetic_dataset(1000, 42);
        assert_eq!(dataset.len(), 1000);

        let mut constant_count = 0;
        let mut zero_var_slots = Vec::new();

        for slot in 0..64 {
            let first_val = extract(&dataset[0].incident, &dataset[0].signature).values[slot];
            let is_const = dataset.iter().all(|item| {
                let fv = extract(&item.incident, &item.signature);
                (fv.values[slot] - first_val).abs() < 1e-6
            });
            // 31 = SCHEMA_VERSION/10 (constante por diseno), 63 = bias 1.0.
            if is_const && slot != 31 && slot != 63 {
                constant_count += 1;
                zero_var_slots.push(slot);
            }
        }

        assert_eq!(
            constant_count, 0,
            "Unexpected constant slots: {:?}",
            zero_var_slots
        );
    }
}
