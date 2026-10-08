//! Derivacion determinista de los parametros del edit acotado (PASO 2).
//!
//! La NN de repair_nn_core clasifica el OPERADOR (13 salidas) pero V0 no
//! tiene cabezas para los parametros que el generador de diffs exige
//! ("dependency" y "version" en package.json). Sin este modulo, todo
//! intento con el gate en verde terminaba bloqueado con "missing_param"
//! aunque el diagnostico fuera correcto (auditoria de codigo 2026-10-05).
//!
//! Politica (docs/LLM_POLICY.md): la NN es la via principal y el LLM solo
//! propone acciones estructuradas. Aqui: extraccion textual acotada y
//! determinista del mensaje del incidente. Lo que no se puede derivar NO
//! se inventa: el intento sigue bloqueado con "missing_param"
//! (fail-closed). Los parametros existentes nunca se sobrescriben.
//!
//! Alcance deliberado: "dependency"/"version" para DependencyRepair y
//! VersionPin, y LOCALIZACION textual para los operadores de edicion
//! ("file" desde el diagnostico del compilador: rustc "--> path:line:col",
//! stack JS "(path:line:col)", webpack "in 'path'"; "from" = especificador
//! no resuelto citado por el empaquetador). "to" NO se deriva JAMAS: el
//! texto de reemplazo seria generacion libre de parches; ese parametro
//! queda para el fallback LLM (acciones estructuradas) o el intento se
//! bloquea con missing_param (fail-closed).

use repair_types::{Incident, OperatorId, RepairAction};

/// Parametros que el generador de diffs lee para dependencias.
const P_DEPENDENCY: &str = "dependency";
const P_VERSION: &str = "version";
/// Parametros que el generador de diffs lee para ediciones textuales.
const P_FILE: &str = "file";
const P_FROM: &str = "from";

/// Rellena los parametros derivables que falten en la accion. Pura: sin
/// I/O, sin reloj, sin azar (testeable en host).
pub fn ensure_params(action: &mut RepairAction, incident: &Incident) {
    match action.repair_operator {
        OperatorId::DependencyRepair | OperatorId::VersionPin => {
            let chars: Vec<char> = incident.message.chars().collect();
            if let Some(dep) = dependency_from_message(&chars) {
                action
                    .parameters
                    .entry(String::from(P_DEPENDENCY))
                    .or_insert(dep);
            }
            if let Some(ver) = version_from_message(&chars) {
                action
                    .parameters
                    .entry(String::from(P_VERSION))
                    .or_insert(ver);
            }
        }
        // Operadores de edicion textual: solo LOCALIZACION (file, from).
        // "to" jamas se deriva (generacion libre prohibida).
        OperatorId::SyntaxFix
        | OperatorId::ConfigRepair
        | OperatorId::BuildScriptFix
        | OperatorId::ImportPathFix
        | OperatorId::TypeAnnotationFix
        | OperatorId::TestRepair
        | OperatorId::SourceRepair => {
            let mut text = incident.message.clone();
            if !incident.stack_hint.is_empty() {
                text.push('\n');
                text.push_str(&incident.stack_hint);
            }
            let chars: Vec<char> = text.chars().collect();
            if let Some(file) = file_from_message(&chars) {
                action
                    .parameters
                    .entry(String::from(P_FILE))
                    .or_insert(file);
            }
            if action.repair_operator == OperatorId::ImportPathFix {
                if let Some(from) = unresolved_specifier(&chars) {
                    action
                        .parameters
                        .entry(String::from(P_FROM))
                        .or_insert(from);
                }
            }
        }
        // El resto de operadores no recibe derivacion (politica del modulo).
        _ => {}
    }
}

/// Nombre de dependencia citado entre comillas tras un patron conocido de
/// modulo no resuelto. La busqueda del patron es case-insensitive; el
/// nombre se extrae sin modificar (los nombres de paquetes son sensibles).
fn dependency_from_message(chars: &[char]) -> Option<String> {
    for needle in ["cannot find module", "can't resolve", "module not found"] {
        let start = match find_ci(chars, needle) {
            Some(i) => i + needle.chars().count(),
            None => continue,
        };
        if let Some(name) = quoted_token(chars, start) {
            if valid_dependency(&name) {
                return Some(name);
            }
        }
    }
    // "peer react@^18.0.0" (conflictos ERESOLVE): dependencia pegada a la
    // version con una arroba.
    peer_parts(chars).map(|(dep, _)| dep)
}

/// Version citada en el mensaje (peer react@^18.0.0 -> 18.0.0).
fn version_from_message(chars: &[char]) -> Option<String> {
    peer_parts(chars).and_then(|(_, ver)| ver)
}

/// Ruta citada por el diagnostico del compilador. Extraccion textual, no
/// generacion: se reconoce el formato conocido y se copia el token tal cual.
fn file_from_message(chars: &[char]) -> Option<String> {
    rustc_location(chars)
        .or_else(|| stack_location(chars))
        .or_else(|| webpack_in_location(chars))
}

/// rustc: "--> src/foo.rs:27:5". El token tras la flecha.
fn rustc_location(chars: &[char]) -> Option<String> {
    let i = find_ci(chars, "-->")? + 3;
    located_token(chars, i)
}

/// Stack JS/node: "at fn (src/foo.js:12:34)". La primera ubicacion con
/// forma de path:linea es la del error; el resto del stack se ignora.
fn stack_location(chars: &[char]) -> Option<String> {
    let mut from = 0;
    while let Some(rel) = find_ci_from(chars, "(", from) {
        if let Some(file) = located_token(chars, rel + 1) {
            return Some(file);
        }
        from = rel + 1;
    }
    None
}

/// webpack/vite: "in '/src/index.js'" (ruta citada tras el modulo no
/// resuelto; los empaquetadores citan rutas absolutas de contexto).
fn webpack_in_location(chars: &[char]) -> Option<String> {
    let start = find_ci(chars, " in ")? + " in ".chars().count();
    let token = quoted_token(chars, start)?;
    let path = token.trim_start_matches('/');
    if valid_file_path(path) {
        Some(String::from(path))
    } else {
        None
    }
}

/// Token tipo "path:linea:columna" a partir de "from". Fail-closed: exige
/// dos puntos seguidos de digitos (una hora "10:58:56" no tiene '.' ni '/'
/// y queda fuera) y una ruta relativa segura.
fn located_token(chars: &[char], from: usize) -> Option<String> {
    let mut i = from;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    let mut token = String::new();
    while i < chars.len() && !chars[i].is_whitespace() {
        token.push(chars[i]);
        i += 1;
    }
    let (path, tail) = token.split_once(':')?;
    if tail.is_empty() || !tail.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let path = path.trim_start_matches('/');
    if !valid_file_path(path) {
        return None;
    }
    Some(String::from(path))
}

/// Especificador de import no resuelto citado por el empaquetador
/// ("Can't resolve './utils'" -> "./utils"): es el texto EXACTO que el
/// parametro "from" del diff busca dentro del archivo.
fn unresolved_specifier(chars: &[char]) -> Option<String> {
    for needle in ["cannot find module", "can't resolve", "module not found"] {
        let start = match find_ci(chars, needle) {
            Some(i) => i + needle.chars().count(),
            None => continue,
        };
        if let Some(spec) = quoted_token(chars, start) {
            if valid_specifier(&spec) {
                return Some(spec);
            }
        }
    }
    None
}

/// Ruta relativa segura para el edit acotado: sin absolutos, traversal,
/// comillas, contrabarras ni espacios; debe parecer un archivo del repo.
fn valid_file_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with(['-', '.'])
        && !path.contains("..")
        && path.contains(['.', '/'])
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/'))
}

/// Especificador de import: relativo o de paquete, sin comillas ni saltos.
fn valid_specifier(spec: &str) -> bool {
    !spec.is_empty()
        && spec.len() <= 256
        && !spec.contains('\\')
        && spec.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | '@' | '~' | '#')
        })
}

/// "peer <dep>@<rango>": parte el nombre y la version (sin el prefijo de
/// rango ^ ~ > < =). Solo acepta dependencias validas.
fn peer_parts(chars: &[char]) -> Option<(String, Option<String>)> {
    let start = find_ci(chars, "peer ")? + "peer ".chars().count();
    let mut i = start;
    let mut dep = String::new();
    while i < chars.len() && is_dep_char(chars[i]) && chars[i] != '@' {
        dep.push(chars[i]);
        i += 1;
    }
    if !valid_dependency(&dep) || i >= chars.len() || chars[i] != '@' {
        return None;
    }
    i += 1;
    // Saltar el prefijo de rango (^18.0.0, ~1.2, >=2): el pin es el numero.
    while i < chars.len() && matches!(chars[i], '^' | '~' | '>' | '<' | '=' | ' ') {
        i += 1;
    }
    let mut ver = String::new();
    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
        ver.push(chars[i]);
        i += 1;
    }
    if ver.is_empty() || !ver.starts_with(|c: char| c.is_ascii_digit()) {
        return Some((dep, None));
    }
    Some((dep, Some(ver)))
}

/// Token entre comillas (simples o dobles) a partir de "from". None si
/// no hay comilla de apertura o de cierre.
fn quoted_token(chars: &[char], from: usize) -> Option<String> {
    let mut i = from;
    while i < chars.len() && chars[i] != '\'' && chars[i] != '"' {
        i += 1;
    }
    if i >= chars.len() {
        return None;
    }
    let quote = chars[i];
    i += 1;
    let mut token = String::new();
    while i < chars.len() && chars[i] != quote {
        token.push(chars[i]);
        i += 1;
    }
    if i >= chars.len() {
        return None; // comilla sin cerrar
    }
    Some(token)
}

/// Primera aparicion de "needle" ignorando mayusculas ASCII. Devuelve el
/// indice en chars.
fn find_ci(chars: &[char], needle: &str) -> Option<usize> {
    find_ci_from(chars, needle, 0)
}

/// Aparicion de "needle" a partir del indice "from" (para iterar el stack).
fn find_ci_from(chars: &[char], needle: &str, from: usize) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || chars.len() < n.len() || from > chars.len() - n.len() {
        return None;
    }
    (from..=chars.len() - n.len()).find(|&i| {
        chars[i..i + n.len()]
            .iter()
            .zip(&n)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

fn is_dep_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '@' | '/')
}

/// Nombre de paquete seguro para el needle textual de version_bump: al
/// excluir comillas y barra invertida, un mensaje malicioso no puede
/// inyectar JSON ni escapar la clave buscada.
fn valid_dependency(dep: &str) -> bool {
    !dep.is_empty() && !dep.starts_with(['-', '.', '/']) && dep.chars().all(is_dep_char)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

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

    fn incident(message: &str) -> Incident {
        Incident {
            message: String::from(message),
            ..Default::default()
        }
    }

    #[test]
    fn derives_dependency_from_cannot_find_module() {
        let mut a = action(OperatorId::DependencyRepair, &[]);
        ensure_params(&mut a, &incident("Error: Cannot find module 'lodash'"));
        assert_eq!(
            a.parameters.get(P_DEPENDENCY).map(String::as_str),
            Some("lodash")
        );
    }

    #[test]
    fn derives_dependency_from_webpack_cant_resolve() {
        let mut a = action(OperatorId::DependencyRepair, &[]);
        ensure_params(
            &mut a,
            &incident("Module not found: Error: Can't resolve 'axios'"),
        );
        assert_eq!(
            a.parameters.get(P_DEPENDENCY).map(String::as_str),
            Some("axios")
        );
    }

    #[test]
    fn derives_dependency_and_version_from_peer_conflict() {
        let mut a = action(OperatorId::VersionPin, &[]);
        ensure_params(
            &mut a,
            &incident("ERESOLVE unable to resolve dependency tree for peer react@^18.0.0"),
        );
        assert_eq!(
            a.parameters.get(P_DEPENDENCY).map(String::as_str),
            Some("react")
        );
        assert_eq!(
            a.parameters.get(P_VERSION).map(String::as_str),
            Some("18.0.0")
        );
    }

    #[test]
    fn derives_dependency_from_module_not_found_lowercase() {
        let mut a = action(OperatorId::DependencyRepair, &[]);
        ensure_params(&mut a, &incident("module not found: 'left-pad'"));
        assert_eq!(
            a.parameters.get(P_DEPENDENCY).map(String::as_str),
            Some("left-pad")
        );
    }

    #[test]
    fn never_overwrites_existing_params() {
        let mut a = action(
            OperatorId::DependencyRepair,
            &[(P_DEPENDENCY, "left-pad"), (P_VERSION, "1.3.0")],
        );
        ensure_params(&mut a, &incident("Cannot find module 'react@^18.0.0'"));
        assert_eq!(
            a.parameters.get(P_DEPENDENCY).map(String::as_str),
            Some("left-pad")
        );
        assert_eq!(
            a.parameters.get(P_VERSION).map(String::as_str),
            Some("1.3.0")
        );
    }

    #[test]
    fn rejects_quote_injection_in_dependency_name() {
        let mut a = action(OperatorId::DependencyRepair, &[]);
        ensure_params(
            &mut a,
            &incident("Cannot find module 'lodash\": 1.0.0, \"x'"),
        );
        assert!(!a.parameters.contains_key(P_DEPENDENCY));
    }

    #[test]
    fn no_pattern_means_no_params_fail_closed() {
        let mut a = action(OperatorId::DependencyRepair, &[]);
        ensure_params(&mut a, &incident("boom sin patron conocido"));
        assert!(a.parameters.is_empty());
    }

    #[test]
    fn syntax_fix_derives_file_from_rustc_arrow() {
        let mut a = action(OperatorId::SyntaxFix, &[]);
        ensure_params(
            &mut a,
            &incident("error: expected ',', found '}'\n --> src/main.rs:27:5"),
        );
        assert_eq!(
            a.parameters.get(P_FILE).map(String::as_str),
            Some("src/main.rs")
        );
        // "to" jamas se deriva (generacion libre prohibida).
        assert!(!a.parameters.contains_key("to"));
    }

    #[test]
    fn syntax_fix_derives_file_from_js_stack() {
        let mut a = action(OperatorId::SourceRepair, &[]);
        ensure_params(
            &mut a,
            &incident("TypeError: x is not a function\n    at foo (src/app.js:12:34)"),
        );
        assert_eq!(
            a.parameters.get(P_FILE).map(String::as_str),
            Some("src/app.js")
        );
    }

    #[test]
    fn import_path_fix_derives_file_and_from() {
        let mut a = action(OperatorId::ImportPathFix, &[]);
        ensure_params(
            &mut a,
            &incident("Module not found: Error: Can't resolve './utils' in '/src/index.js'"),
        );
        assert_eq!(
            a.parameters.get(P_FILE).map(String::as_str),
            Some("src/index.js")
        );
        assert_eq!(
            a.parameters.get(P_FROM).map(String::as_str),
            Some("./utils")
        );
    }

    #[test]
    fn stack_hint_is_searched_when_message_has_no_location() {
        let inc = Incident {
            message: String::from("compile error"),
            stack_hint: String::from("--> lib/parser.rs:88:13"),
            ..Default::default()
        };
        let mut a = action(OperatorId::TypeAnnotationFix, &[]);
        ensure_params(&mut a, &inc);
        assert_eq!(
            a.parameters.get(P_FILE).map(String::as_str),
            Some("lib/parser.rs")
        );
    }

    #[test]
    fn traversal_and_timestamps_are_rejected() {
        let mut a = action(OperatorId::SyntaxFix, &[]);
        ensure_params(&mut a, &incident(" --> ../outside.rs:1:1"));
        assert!(!a.parameters.contains_key(P_FILE));
        let mut b = action(OperatorId::SyntaxFix, &[]);
        ensure_params(&mut b, &incident("failed at 10:58:56 (retry)"));
        assert!(!b.parameters.contains_key(P_FILE));
    }

    #[test]
    fn textual_operators_get_no_free_generation() {
        // Sin localizacion en el mensaje: nada derivado (fail-closed).
        let mut a = action(OperatorId::SyntaxFix, &[]);
        ensure_params(&mut a, &incident("Cannot find module 'lodash'"));
        assert!(a.parameters.is_empty());
        // SyntaxFix no deriva "from" aunque el mensaje cite un modulo:
        // ese parametro es exclusivo de ImportPathFix.
        let mut b = action(OperatorId::SyntaxFix, &[]);
        ensure_params(
            &mut b,
            &incident("Module not found: Error: Can't resolve './x' in '/src/i.js'"),
        );
        assert!(!b.parameters.contains_key(P_FROM));
    }

    #[test]
    fn existing_file_param_is_never_overwritten() {
        let mut a = action(OperatorId::SyntaxFix, &[(P_FILE, "src/other.ts")]);
        ensure_params(&mut a, &incident(" --> src/main.rs:27:5"));
        assert_eq!(
            a.parameters.get(P_FILE).map(String::as_str),
            Some("src/other.ts")
        );
    }

    #[test]
    fn peer_without_version_derives_nothing() {
        let mut a = action(OperatorId::DependencyRepair, &[]);
        ensure_params(&mut a, &incident("peer dep"));
        // Sin arroba peer_parts exige el formato "peer <dep>@...":
        // nada derivado (fail-closed).
        assert!(a.parameters.is_empty());
    }
}
