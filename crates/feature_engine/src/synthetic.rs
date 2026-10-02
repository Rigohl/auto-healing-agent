//! Dataset sintetico determinista para medir cobertura del feature encoder.
//!
//! No es entrenamiento: `model/*.json` sigue con `weights: null`. Sirve para
//! dos cosas, ambas verificables en test:
//!   1. Que ningun slot 0..=63 sea constante (senal muerta).
//!   2. Que el vector no dependa del id de incidente ni del ruido de sesion.
//!
//! Generador: LCG de 64 bits con semilla explicita, asi que la salida es
//! reproducible byte a byte y el test no depende de `rand`.

use alloc::format;
use alloc::vec::Vec;
use repair_types::{FailureSignature, Incident};

#[derive(Debug, Clone)]
pub struct SyntheticIncident {
    pub incident: Incident,
    pub signature: FailureSignature,
    pub ground_truth_operator: u8,
    pub error_category: String,
}

pub fn generate_synthetic_dataset(count: usize, seed: u64) -> Vec<SyntheticIncident> {
    let mut dataset = Vec::with_capacity(count);
    let mut state = seed;

    let error_templates = [
        // Category 0: Syntax error -> Operator 2 (SyntaxFix)
        ("syntax_error", "buildStep", "npm run build", "Unexpected token '{' at index 42", "typescript", "next", 2u8, "github", "open"),
        ("parse_error", "compile", "next build", "SyntaxError: Unexpected end of input", "javascript", "react", 2u8, "vercel", "failed"),
        // Category 1: Dependency missing -> Operator 1 (DependencyRepair)
        ("module_not_found", "install", "npm start", "Error: Cannot find module 'lodash'", "javascript", "express", 1u8, "vercel", "failed"),
        ("missing_dependency", "build", "pnpm build", "Module not found: Error: Can't resolve 'axios'", "typescript", "next", 1u8, "github", "open"),
        // Category 2: Config repair -> Operator 3 (ConfigRepair)
        ("tsconfig_error", "check", "tsc --noEmit", "CompilerOptions invalid in tsconfig.json", "typescript", "react", 3u8, "github", "failed"),
        ("vercel_config", "deploy", "vercel --prod", "Invalid vercel.json schema property 'builds'", "typescript", "next", 3u8, "vercel", "open"),
        // Category 3: Build script fix -> Operator 4 (BuildScriptFix)
        ("missing_script", "buildStep", "npm run vercel-build", "npm ERR! missing script: vercel-build", "javascript", "node", 4u8, "vercel", "failed"),
        // Category 4: Env var repair -> Operator 5 (EnvVarRepair)
        ("missing_env", "runtime", "node server.js", "Error: Missing required environment variable DATABASE_URL", "python", "django", 5u8, "vercel", "failed"),
        // Category 5: Lockfile refresh -> Operator 6 (LockfileRefresh)
        ("lockfile_outdated", "install", "npm ci", "npm ERR! package-lock.json is out of date with package.json", "javascript", "node", 6u8, "github", "open"),
        // Category 6: Type annotation -> Operator 7 (TypeAnnotationFix)
        ("type_mismatch", "compile", "tsc", "Type 'string' is not assignable to type 'number'", "typescript", "react", 7u8, "github", "failed"),
        // Category 7: Import path -> Operator 8 (ImportPathFix)
        ("import_resolution", "build", "next build", "Cannot resolve import path '../../components/Button'", "typescript", "next", 8u8, "github", "failed"),
        // Category 8: Version pin -> Operator 9 (VersionPin)
        ("peer_dep_conflict", "install", "npm install", "ERESOLVE unable to resolve dependency tree for peer react@^18.0.0", "javascript", "react", 9u8, "github", "open"),
        // Category 9: Cache clear -> Operator 10 (CacheClear)
        ("corrupt_cache", "build", "next build", "Error: Failed to read build cache in .next/cache", "typescript", "next", 10u8, "vercel", "failed"),
        // Category 10: Test repair -> Operator 11 (TestRepair)
        ("test_failed", "test", "npm test", "FAIL src/app.test.ts - Expected 200 received 500 (jest)", "typescript", "jest", 11u8, "github", "failed"),
        // Category 11: Source repair -> Operator 12 (SourceRepair)
        ("null_pointer", "runtime", "node index.js", "TypeError: Cannot read property 'map' of undefined", "typescript", "react", 12u8, "github", "failed"),
        // Category 12: Cargo build failure -> Operator 4 (BuildScriptFix) / Cargo
        ("cargo_build_err", "build", "cargo build", "error[E0432]: unresolved import `std::fs`", "rust", "cargo", 4u8, "github", "failed"),
        // Category 13: Permission denied -> Operator 3 (ConfigRepair)
        ("permission_denied", "setup", "chmod +x build.sh", "EACCES: permission denied, open '/usr/bin'", "bash", "shell", 3u8, "github", "open"),
        // Category 14: OOM / Timeout -> Operator 10 (CacheClear)
        ("timeout_oom", "build", "next build", "FATAL ERROR: Reached heap limit Allocation failed - JavaScript heap out of memory", "javascript", "next", 10u8, "vercel", "failed"),
        // Category 15: Lint failure -> Operator 7 (TypeAnnotationFix)
        ("eslint_error", "check", "eslint .", "ESLint error: 'x' is defined but never used", "typescript", "react", 7u8, "github", "failed"),
        // Category 16: Webpack bundler -> Operator 4 (BuildScriptFix)
        ("webpack_err", "build", "webpack --config webpack.config.js", "Webpack compilation failed: ModuleBuildError", "javascript", "webpack", 4u8, "github", "open"),
        // Category 17: Linear issue trigger -> Operator 1 (DependencyRepair)
        ("linear_dep_err", "buildStep", "pnpm install", "Linear issue: Dependency installation failed with code 1", "typescript", "svelte", 1u8, "linear", "failed"),
    ];

    for i in 0..count {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let idx = (state as usize) % error_templates.len();
        let template = error_templates[idx];

        let inc_id = format!("inc-{:06}", i);
        let rand_noise = (state >> 16) & 0xffff;
        let project_id = format!("proj-{}", (state % 50));

        let message = format!("{} (ref: {})", template.3, rand_noise);
        let verified = (state % 2) == 1;

        let incident = Incident {
            id: inc_id,
            source: template.7.into(),
            error_code: template.0.into(),
            error_step: template.1.into(),
            command: template.2.into(),
            message,
            project: project_id,
            attempts: ((state % 4) + 1) as u32,
            stack_hint: format!("at line {}:{}", (state % 100) + 1, (state % 80) + 1),
            language_hint: template.4.into(),
            framework_hint: template.5.into(),
            verified,
            status: template.8.into(),
        };

        let signature = FailureSignature::from_incident(&incident);

        dataset.push(SyntheticIncident {
            incident,
            signature,
            ground_truth_operator: template.6,
            error_category: template.0.into(),
        });
    }

    dataset
}
