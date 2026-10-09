//! Paridad entre los dos generadores de unified diff (docs/DEAD_CODE_AUDIT.md).
//!
//! Riesgo documentado: `repair_pr::diff` (similar, evidencia offline) y
//! `repair_operators::diff` (runtime del worker) son dos implementaciones
//! paralelas que pueden divergir en silencio; CI no comparaba sus salidas.
//! Este test cierra esa brecha: ante las MISMAS entradas before/after, ambos
//! generadores deben producir diffs equivalentes (mismo header a/b, mismas
//! lineas eliminadas y anadidas, en el mismo orden) y ambos deben ser
//! empty-fail-closed ante contenido identico. Si alguien cambia un solo
//! lado, este test se pone rojo ANTES de que el divergence llegue a produccion.

use repair_operators::diff::FileEdit;
use repair_pr::diff::{bundle_is_empty, unified_diff_file, FileChange};

fn runtime_diff(path: &str, before: &str, after: &str) -> String {
    repair_operators::diff::unified_diff(&FileEdit {
        path: path.to_string(),
        before: before.to_string(),
        after: after.to_string(),
    })
}

fn offline_diff(path: &str, before: &str, after: &str) -> String {
    unified_diff_file(&FileChange {
        path: path.to_string(),
        before: before.to_string(),
        after: after.to_string(),
    })
}

/// Lineas de cambio (-/+) sin headers del bundle, en orden.
fn changed_lines(diff: &str) -> Vec<String> {
    diff.lines()
        .filter(|l| l.starts_with('-') || l.starts_with('+'))
        .filter(|l| !l.starts_with("---") && !l.starts_with("+++"))
        .map(|l| l.to_string())
        .collect()
}

fn assert_equivalent(path: &str, before: &str, after: &str) {
    let rt = runtime_diff(path, before, after);
    let off = offline_diff(path, before, after);

    assert!(!bundle_is_empty(&rt), "runtime diff vacio: {path}");
    assert!(!bundle_is_empty(&off), "offline diff vacio: {path}");

    // Ambos referencian el mismo archivo con headers a/ b/.
    for d in [&rt, &off] {
        assert!(d.contains(&format!("--- a/{path}")), "header a/ falta: {d}");
        assert!(d.contains(&format!("+++ b/{path}")), "header b/ falta: {d}");
    }

    // Nucleo de la paridad: mismas lineas eliminadas y anadidas, mismo orden.
    let rt_lines = changed_lines(&rt);
    let off_lines = changed_lines(&off);
    assert_eq!(
        rt_lines, off_lines,
        "los generadores de diff divergieron para {path}:\nRuntime:\n{rt}\nOffline:\n{off}"
    );
}

#[test]
fn ambos_generadores_estan_de_acuerdo_en_un_bump_de_version() {
    assert_equivalent(
        "package.json",
        "{\n  \"left\": \"1.0.0\"\n}\n",
        "{\n  \"left\": \"1.0.1\"\n}\n",
    );
}

#[test]
fn ambos_generadores_estan_de_acuerdo_en_un_fix_de_import() {
    assert_equivalent(
        "src/demo.ts",
        "import x from \"../old/path\";\n",
        "import x from \"../new/path\";\n",
    );
}

#[test]
fn ambos_generadores_estan_de_acuerdo_en_una_linea_nueva_en_medio() {
    assert_equivalent(
        "config.yaml",
        "alpha: 1\nbeta: 2\ngamma: 3\n",
        "alpha: 1\nbeta: 2\ndelta: 4\ngamma: 3\n",
    );
}

#[test]
fn contenido_identico_es_empty_fail_closed_en_ambos() {
    let path = "same.txt";
    let rt = runtime_diff(path, "same\n", "same\n");
    let off = offline_diff(path, "same\n", "same\n");
    assert!(bundle_is_empty(&rt), "runtime debia ser no-op: {rt}");
    assert!(bundle_is_empty(&off), "offline debia ser no-op: {off}");
}
