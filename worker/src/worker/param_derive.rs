//! Derivacion determinista de los parametros del edit acotado (PASO 2).
//!
//! La NN de repair_nn_core clasifica el OPERADOR (13 salidas) pero V0 no
//! tiene cabezas para los parametros que el generador de diffs exige
//! ("dependency" y "version" en package.json). Sin este modulo, todo
//! intento con el gate en verde terminaba bloqueado con "missing_param"
//! aunque el diagnostico fuera correcto (auditoria de codigo 2026-10-05).
//!
//! Politica (docs/NO_LLM_POLICY.md): sin LLM ni heuristica generosa. Solo
//! extraccion textual acotada y determinista del mensaje del incidente.
//! Lo que no se puede derivar NO se inventa: el intento sigue bloqueado
//! con "missing_param" (fail-closed). Los parametros existentes nunca se
//! sobrescriben.
//!
//! Alcance deliberado: solo "dependency" y "version" para
//! DependencyRepair/VersionPin. Los operadores de edicion textual
//! ("file"/"from"/"to") NO se derivan: fabricarlos a partir de un mensaje
//! seria generacion libre de parches, exactamente lo que la politica
//! prohibe; esos intentos quedan bloqueados hasta que exista una fuente
//! real de localizacion (stack parseado, LSP, etc.).

use repair_types::{Incident, OperatorId, RepairAction};

/// Parametros que el generador de diffs lee para dependencias.
const P_DEPENDENCY: &str = "dependency";
const P_VERSION: &str = "version";

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
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || chars.len() < n.len() {
        return None;
    }
    (0..=chars.len() - n.len()).find(|&i| {
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
        assert_eq!(a.parameters.get(P_VERSION).map(String::as_str), Some("18.0.0"));
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
        assert_eq!(a.parameters.get(P_VERSION).map(String::as_str), Some("1.3.0"));
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
    fn textual_operators_get_no_derivation() {
        let mut a = action(OperatorId::SyntaxFix, &[]);
        ensure_params(&mut a, &incident("Cannot find module 'lodash'"));
        assert!(a.parameters.is_empty());
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
