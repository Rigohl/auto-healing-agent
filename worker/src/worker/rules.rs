//! REGLAS DECLARATIVAS del pipeline (P1 del roadmap "Cloudflare + Rust",
//! patron blueprint de pingoo: expresion compilada + accion enum tipada).
//!
//! Por que sin el crate wirefilter-engine (0.6.1, cloudflare/wirefilter):
//! su propio engine/Cargo.toml declara que getrandom NO tiene fuente de
//! aleatoriedad en wasm32-unknown y exige --features getrandom/wasm_js
//! (ademas de deps pesadas: backtrace, regex-automata, wildcard), y NINGUN
//! job de CI compila el worker WASM real (wasm.yml compila solo
//! repair_nn_wasm): adoptarlo seria un riesgo de deploy no verificable.
//! Este modulo implementa el subconjunto necesario (Scheme -> AST -> IR en
//! miniatura, estilo wirefilter) con cero dependencias nuevas. Reevaluar
//! cuando CI pueda compilar el worker a wasm32. Fuentes consultadas
//! 2026-10-05: cloudflare/wirefilter engine/Cargo.toml y README, blueprint
//! de pingoo (pagina Notion "Cloudflare + Rust - Links").
//!
//! Reglas del modulo:
//! - Fail-closed: configuracion invalida => Err => el consumidor BLOQUEA
//!   (nunca deja pasar por defecto). Sin var REPAIR_RULES o "[]" = sin
//!   reglas (no-op).
//! - Las reglas NUNCA permiten: solo block (restringe, fail-closed
//!   adicional al gate) u observe (solo log). El gate determinista
//!   (repair_operators::gate) sigue mandando y VERIFY sigue en GitHub
//!   Actions (reglas de autoridad de PART3).
//! - Campos y tipos validados en COMPILE time contra el esquema fijo:
//!   eval no puede fallar por tipos (y aun asi el path es fail-closed).
//! - Sin unwrap() en ningun path (mismo estandar que fetch).

use repair_types::Incident;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Var de configuracion: JSON array de {id, expression, action}.
pub const VAR_REPAIR_RULES: &str = "REPAIR_RULES";

/// Campos del esquema fijo (tipos) evaluables por las expresiones.
pub const F_REPO: &str = "repo";
pub const F_SIGNATURE: &str = "signature";
pub const F_ERROR_CODE: &str = "error_code";
pub const F_ERROR_STEP: &str = "error_step";
pub const F_SOURCE: &str = "source";
pub const F_OPERATOR: &str = "operator";
pub const F_CONFIDENCE: &str = "confidence";
pub const F_RISK: &str = "risk";
pub const F_ATTEMPTS: &str = "attempts";

/// Accion de una regla. Enum tipada (blueprint pingoo): sin texto libre y
/// SIN accion allow: las reglas solo pueden restringir u observar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    /// La coincidencia solo se reporta (observabilidad): nunca cambia la
    /// decision del gate.
    Observe,
    /// La coincidencia bloquea el intento (fail-closed adicional).
    Block,
}

/// Regla tal como viene en la configuracion JSON.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub expression: String,
    pub action: RuleAction,
}

/// Valor tipado del contexto de evaluacion.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Num(f64),
    Int(i64),
    Bool(bool),
}

/// Tipo de un campo del esquema (para validar en compile time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Str,
    Num,
    Int,
    Bool,
}

/// Esquema fijo: los unicos campos permitidos en expresiones y su tipo.
fn schema_kind(name: &str) -> Option<Kind> {
    match name {
        F_REPO | F_SIGNATURE | F_ERROR_CODE | F_ERROR_STEP | F_SOURCE | F_OPERATOR => {
            Some(Kind::Str)
        }
        F_CONFIDENCE | F_RISK => Some(Kind::Num),
        F_ATTEMPTS => Some(Kind::Int),
        _ => None,
    }
}

/// Operando de una comparacion: campo del esquema o literal.
#[derive(Debug, Clone)]
enum Operand {
    Field(String),
    Str(String),
    Num(f64),
    Int(i64),
    Bool(bool),
}

impl Operand {
    fn kind(&self) -> Option<Kind> {
        match self {
            Operand::Field(name) => schema_kind(name),
            Operand::Str(_) => Some(Kind::Str),
            Operand::Num(_) => Some(Kind::Num),
            Operand::Int(_) => Some(Kind::Int),
            Operand::Bool(_) => Some(Kind::Bool),
        }
    }
}

/// Operadores de comparacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CmpOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

/// Token del lexer.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Num(f64),
    Int(i64),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    And,
    Or,
    Not,
    Op(CmpOp),
    In,
    Contains,
    True,
    False,
}

/// AST de una expresion compilada.
#[derive(Debug, Clone)]
enum Ast {
    Lit(bool),
    Cmp {
        left: Operand,
        op: CmpOp,
        right: Operand,
    },
    Contains {
        left: Operand,
        right: Operand,
    },
    In {
        left: Operand,
        values: Vec<Operand>,
    },
    Not(Box<Ast>),
    And(Box<Ast>, Box<Ast>),
    Or(Box<Ast>, Box<Ast>),
}

/// Lexea un numero (entero o decimal, con signo). Devuelve token y nueva
/// posicion.
fn lex_number(bytes: &[char], start: usize, rule_id: &str) -> Result<(Tok, usize), String> {
    let mut i = start;
    if bytes[i] == '-' {
        i += 1;
    }
    let mut has_dot = false;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || (bytes[i] == '.' && !has_dot)) {
        if bytes[i] == '.' {
            has_dot = true;
        }
        i += 1;
    }
    let raw: String = bytes[start..i].iter().collect();
    if has_dot {
        match raw.parse::<f64>() {
            Ok(n) => Ok((Tok::Num(n), i)),
            Err(_) => Err(format!("regla {}: numero invalido: {}", rule_id, raw)),
        }
    } else {
        match raw.parse::<i64>() {
            Ok(n) => Ok((Tok::Int(n), i)),
            Err(_) => Err(format!("regla {}: numero invalido: {}", rule_id, raw)),
        }
    }
}

/// Lexea una palabra (identificador o keyword). Devuelve token y nueva
/// posicion. Infallible: el caller ya valido el primer caracter.
fn lex_word(bytes: &[char], start: usize) -> (Tok, usize) {
    let mut i = start;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == '_') {
        i += 1;
    }
    let word: String = bytes[start..i].iter().collect();
    let tok = match word.as_str() {
        "and" => Tok::And,
        "or" => Tok::Or,
        "not" => Tok::Not,
        "in" => Tok::In,
        "contains" => Tok::Contains,
        "true" => Tok::True,
        "false" => Tok::False,
        "eq" => Tok::Op(CmpOp::Eq),
        "ne" => Tok::Op(CmpOp::Ne),
        "gt" => Tok::Op(CmpOp::Gt),
        "ge" => Tok::Op(CmpOp::Ge),
        "lt" => Tok::Op(CmpOp::Lt),
        "le" => Tok::Op(CmpOp::Le),
        _ => Tok::Ident(word),
    };
    (tok, i)
}

/// Tokeniza una expresion completa. Fail-closed: cualquier caracter
/// inesperado es error.
fn tokenize(input: &str, rule_id: &str) -> Result<Vec<Tok>, String> {
    let bytes: Vec<char> = input.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            '{' => {
                toks.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                toks.push(Tok::RBrace);
                i += 1;
            }
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            '&' => {
                if i + 1 < bytes.len() && bytes[i + 1] == '&' {
                    toks.push(Tok::And);
                    i += 2;
                } else {
                    return Err(format!("regla {}: '&' suelto (usa &&)", rule_id));
                }
            }
            '|' => {
                if i + 1 < bytes.len() && bytes[i + 1] == '|' {
                    toks.push(Tok::Or);
                    i += 2;
                } else {
                    return Err(format!("regla {}: '|' suelto (usa ||)", rule_id));
                }
            }
            '!' => {
                toks.push(Tok::Not);
                i += 1;
            }
            '"' => {
                let mut s = String::new();
                i += 1;
                while i < bytes.len() && bytes[i] != '"' {
                    s.push(bytes[i]);
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err(format!("regla {}: string sin cerrar", rule_id));
                }
                i += 1;
                toks.push(Tok::Str(s));
            }
            _ => {
                if c.is_ascii_digit() || c == '-' {
                    let (tok, next) = lex_number(&bytes, i, rule_id)?;
                    toks.push(tok);
                    i = next;
                } else if c.is_ascii_alphabetic() || c == '_' {
                    let (tok, next) = lex_word(&bytes, i);
                    toks.push(tok);
                    i = next;
                } else {
                    return Err(format!("regla {}: caracter inesperado '{}'", rule_id, c));
                }
            }
        }
    }
    Ok(toks)
}

/// Parser recursive-descent: or < and < not < comparacion.
struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    rule_id: &'a str,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_expr(&mut self) -> Result<Ast, String> {
        let mut left = self.parse_and()?;
        loop {
            match self.peek() {
                Some(Tok::Or) => {
                    self.pos += 1;
                    let right = self.parse_and()?;
                    left = Ast::Or(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Ast, String> {
        let mut left = self.parse_not()?;
        loop {
            match self.peek() {
                Some(Tok::And) => {
                    self.pos += 1;
                    let right = self.parse_not()?;
                    left = Ast::And(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Ast, String> {
        match self.peek() {
            Some(Tok::Not) => {
                self.pos += 1;
                let inner = self.parse_not()?;
                Ok(Ast::Not(Box::new(inner)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_operand(&mut self) -> Result<Operand, String> {
        match self.next() {
            Some(tok) => self.operand_from(tok),
            None => Err(format!("regla {}: se esperaba un valor", self.rule_id)),
        }
    }

    fn operand_from(&self, tok: Tok) -> Result<Operand, String> {
        match tok {
            Tok::Ident(name) => {
                if schema_kind(&name).is_none() {
                    return Err(format!(
                        "regla {}: campo desconocido '{}' (esquema: repo, signature, error_code, error_step, source, operator, confidence, risk, attempts)",
                        self.rule_id, name
                    ));
                }
                Ok(Operand::Field(name))
            }
            Tok::Str(s) => Ok(Operand::Str(s)),
            Tok::Num(n) => Ok(Operand::Num(n)),
            Tok::Int(n) => Ok(Operand::Int(n)),
            Tok::True => Ok(Operand::Bool(true)),
            Tok::False => Ok(Operand::Bool(false)),
            other => Err(format!("regla {}: token inesperado {:?}", self.rule_id, other)),
        }
    }

    fn parse_primary(&mut self) -> Result<Ast, String> {
        match self.next() {
            Some(Tok::LParen) => {
                let inner = self.parse_expr()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(inner),
                    _ => Err(format!("regla {}: falta ')' de cierre", self.rule_id)),
                }
            }
            Some(Tok::True) => Ok(Ast::Lit(true)),
            Some(Tok::False) => Ok(Ast::Lit(false)),
            Some(tok) => {
                let left = self.operand_from(tok)?;
                match self.next() {
                    Some(Tok::Op(op)) => {
                        let right = self.parse_operand()?;
                        Ok(Ast::Cmp {
                            left,
                            op,
                            right,
                        })
                    }
                    Some(Tok::Contains) => {
                        let right = self.parse_operand()?;
                        Ok(Ast::Contains { left, right })
                    }
                    Some(Tok::In) => {
                        if !matches!(self.next(), Some(Tok::LBrace)) {
                            return Err(format!("regla {}: 'in' espera {{ ... }}", self.rule_id));
                        }
                        let mut values = Vec::new();
                        loop {
                            while matches!(self.peek(), Some(Tok::Comma)) {
                                self.pos += 1;
                            }
                            if matches!(self.peek(), Some(Tok::RBrace)) {
                                self.pos += 1;
                                break;
                            }
                            values.push(self.parse_operand()?);
                        }
                        if values.is_empty() {
                            return Err(format!("regla {}: 'in' con conjunto vacio", self.rule_id));
                        }
                        Ok(Ast::In { left, values })
                    }
                    None => Err(format!("regla {}: expresion incompleta", self.rule_id)),
                    Some(_) => Err(format!(
                        "regla {}: se esperaba un operador despues del valor",
                        self.rule_id
                    )),
                }
            }
            None => Err(format!("regla {}: expresion vacia", self.rule_id)),
        }
    }
}

/// Validacion de tipos en compile time (Scheme -> AST): los campos deben
/// existir y los operandos ser compatibles con el operador.
fn validate(ast: &Ast, rule_id: &str) -> Result<(), String> {
    match ast {
        Ast::Lit(_) => Ok(()),
        Ast::Not(inner) => validate(inner, rule_id),
        Ast::And(a, b) | Ast::Or(a, b) => {
            validate(a, rule_id)?;
            validate(b, rule_id)
        }
        Ast::Cmp { left, op, right } => {
            let lk = match left.kind() {
                Some(k) => k,
                None => return Err(format!("regla {}: campo desconocido en la comparacion", rule_id)),
            };
            let rk = match right.kind() {
                Some(k) => k,
                None => return Err(format!("regla {}: valor invalido en la comparacion", rule_id)),
            };
            match op {
                CmpOp::Eq | CmpOp::Ne => {
                    if lk != rk {
                        return Err(format!(
                            "regla {}: eq/ne requiere operandos del mismo tipo",
                            rule_id
                        ));
                    }
                }
                CmpOp::Gt | CmpOp::Ge | CmpOp::Lt | CmpOp::Le => {
                    if !matches!(lk, Kind::Num | Kind::Int) || lk != rk {
                        return Err(format!(
                            "regla {}: gt/ge/lt/le requiere dos numericos del mismo tipo",
                            rule_id
                        ));
                    }
                }
            }
            Ok(())
        }
        Ast::Contains { left, right } => {
            if left.kind() != Some(Kind::Str) || right.kind() != Some(Kind::Str) {
                return Err(format!("regla {}: contains requiere dos strings", rule_id));
            }
            Ok(())
        }
        Ast::In { left, values } => {
            let lk = match left.kind() {
                Some(k) => k,
                None => return Err(format!("regla {}: campo desconocido en 'in'", rule_id)),
            };
            for v in values {
                if v.kind() != Some(lk) {
                    return Err(format!(
                        "regla {}: 'in' requiere valores del tipo del campo",
                        rule_id
                    ));
                }
            }
            Ok(())
        }
    }
}

fn eval_operand(op: &Operand, ctx: &RuleContext) -> Result<Value, String> {
    match op {
        Operand::Field(name) => match ctx.get(name) {
            Some(v) => Ok(v.clone()),
            None => Err(format!("campo sin valor en el contexto: {}", name)),
        },
        Operand::Str(s) => Ok(Value::Str(s.clone())),
        Operand::Num(n) => Ok(Value::Num(*n)),
        Operand::Int(n) => Ok(Value::Int(*n)),
        Operand::Bool(b) => Ok(Value::Bool(*b)),
    }
}

/// Evalua el AST contra el contexto. Los tipos ya se validaron en compile
/// time; los errores de runtime son de contexto (fail-closed arriba).
fn eval(ast: &Ast, ctx: &RuleContext) -> Result<bool, String> {
    match ast {
        Ast::Lit(b) => Ok(*b),
        Ast::Not(inner) => Ok(!eval(inner, ctx)?),
        Ast::And(a, b) => Ok(eval(a, ctx)? && eval(b, ctx)?),
        Ast::Or(a, b) => Ok(eval(a, ctx)? || eval(b, ctx)?),
        Ast::Cmp { left, op, right } => {
            let l = eval_operand(left, ctx)?;
            let r = eval_operand(right, ctx)?;
            let res = match (&l, &r) {
                (Value::Str(a), Value::Str(b)) => match op {
                    CmpOp::Eq => a == b,
                    CmpOp::Ne => a != b,
                    _ => return Err("comparacion ordenada entre strings".to_string()),
                },
                (Value::Num(a), Value::Num(b)) => match op {
                    CmpOp::Eq => a == b,
                    CmpOp::Ne => a != b,
                    CmpOp::Gt => a > b,
                    CmpOp::Ge => a >= b,
                    CmpOp::Lt => a < b,
                    CmpOp::Le => a <= b,
                },
                (Value::Int(a), Value::Int(b)) => match op {
                    CmpOp::Eq => a == b,
                    CmpOp::Ne => a != b,
                    CmpOp::Gt => a > b,
                    CmpOp::Ge => a >= b,
                    CmpOp::Lt => a < b,
                    CmpOp::Le => a <= b,
                },
                (Value::Bool(a), Value::Bool(b)) => match op {
                    CmpOp::Eq => a == b,
                    CmpOp::Ne => a != b,
                    _ => return Err("comparacion ordenada entre bools".to_string()),
                },
                _ => return Err("tipos no comparables".to_string()),
            };
            Ok(res)
        }
        Ast::Contains { left, right } => {
            match (eval_operand(left, ctx)?, eval_operand(right, ctx)?) {
                (Value::Str(a), Value::Str(b)) => Ok(a.contains(b.as_str())),
                _ => Err("contains requiere dos strings".to_string()),
            }
        }
        Ast::In { left, values } => {
            let l = eval_operand(left, ctx)?;
            for v in values {
                if eval_operand(v, ctx)? == l {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

/// Contexto de evaluacion: valores tipados por campo del esquema.
#[derive(Debug, Default, Clone)]
pub struct RuleContext {
    values: BTreeMap<String, Value>,
}

impl RuleContext {
    pub fn new() -> Self {
        RuleContext {
            values: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, name: &str, value: Value) {
        self.values.insert(name.to_string(), value);
    }

    fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// Contexto estandar del consumidor: campos del esquema desde el
    /// incidente y la accion del gate. Funcion pura (testeable en host).
    pub fn from_incident_and_action(
        incident: &Incident,
        repo: &str,
        signature: &str,
        operator: &str,
        confidence: f32,
        risk: f32,
    ) -> RuleContext {
        let mut ctx = RuleContext::new();
        ctx.set(F_REPO, Value::Str(repo.to_string()));
        ctx.set(F_SIGNATURE, Value::Str(signature.to_string()));
        ctx.set(F_ERROR_CODE, Value::Str(incident.error_code.clone()));
        ctx.set(F_ERROR_STEP, Value::Str(incident.error_step.clone()));
        ctx.set(F_SOURCE, Value::Str(incident.source.clone()));
        ctx.set(F_OPERATOR, Value::Str(operator.to_string()));
        ctx.set(F_CONFIDENCE, Value::Num(confidence as f64));
        ctx.set(F_RISK, Value::Num(risk as f64));
        ctx.set(F_ATTEMPTS, Value::Int(incident.attempts as i64));
        ctx
    }
}

/// Ruleset compilado (inmutable tras from_json).
pub struct Ruleset {
    rules: Vec<CompiledRule>,
}

struct CompiledRule {
    id: String,
    ast: Ast,
    action: RuleAction,
}

impl Ruleset {
    /// Compila un JSON de configuracion. Fail-closed: cualquier regla
    /// invalida (JSON, sintaxis, campo desconocido, tipos) rechaza TODO
    /// el ruleset; el llamador debe bloquear en ese caso.
    pub fn from_json(raw: &str) -> Result<Ruleset, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Ruleset {
                rules: Vec::new(),
            });
        }
        let rules: Vec<Rule> = serde_json::from_str(trimmed)
            .map_err(|e| format!("REPAIR_RULES no es JSON valido: {}", e))?;
        let mut compiled = Vec::new();
        for rule in rules {
            if rule.id.trim().is_empty() {
                return Err("regla con id vacio".to_string());
            }
            let toks = tokenize(&rule.expression, &rule.id)?;
            let mut parser = Parser {
                toks: &toks,
                pos: 0,
                rule_id: &rule.id,
            };
            let ast = parser.parse_expr()?;
            if parser.pos != toks.len() {
                return Err(format!("regla {}: tokens sobrantes al final", rule.id));
            }
            validate(&ast, &rule.id)?;
            compiled.push(CompiledRule {
                id: rule.id,
                ast,
                action: rule.action,
            });
        }
        Ok(Ruleset { rules: compiled })
    }

    /// Evalua las reglas en orden. Devuelve (id de la PRIMERA regla block
    /// que coincide | None, ids de las reglas observe que coincidieron).
    pub fn evaluate(&self, ctx: &RuleContext) -> Result<(Option<String>, Vec<String>), String> {
        let mut blocked: Option<String> = None;
        let mut observed = Vec::new();
        for rule in &self.rules {
            if eval(&rule.ast, ctx)? {
                match rule.action {
                    RuleAction::Block => {
                        if blocked.is_none() {
                            blocked = Some(rule.id.clone());
                        }
                    }
                    RuleAction::Observe => observed.push(rule.id.clone()),
                }
            }
        }
        Ok((blocked, observed))
    }
}

/// Carga el ruleset desde la var de entorno. Sin var (o vacia) = sin reglas
/// (no-op); con var invalida = Err y el llamador bloquea (fail-closed).
pub fn load(env: &worker::Env) -> Result<Ruleset, String> {
    match env.var(VAR_REPAIR_RULES) {
        Ok(v) => Ruleset::from_json(&v.to_string()),
        Err(_) => Ok(Ruleset {
            rules: Vec::new(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> RuleContext {
        let mut c = RuleContext::new();
        c.set(F_REPO, Value::Str("acme/api".to_string()));
        c.set(F_SIGNATURE, Value::Str("E500|build|cargo build".to_string()));
        c.set(F_ERROR_CODE, Value::Str("E500".to_string()));
        c.set(F_ERROR_STEP, Value::Str("build".to_string()));
        c.set(F_SOURCE, Value::Str("webhook".to_string()));
        c.set(F_OPERATOR, Value::Str("pin_dependency".to_string()));
        c.set(F_CONFIDENCE, Value::Num(0.9));
        c.set(F_RISK, Value::Num(0.2));
        c.set(F_ATTEMPTS, Value::Int(2));
        c
    }

    /// Compila UNA regla block con la expresion dada y la evalua contra
    /// ctx(). Devuelve true si bloquea.
    fn blocks(expr: &str) -> bool {
        let raw = format!(
            "[{{"id":"r1","expression":{:?},"action":"block"}}]",
            expr
        );
        let rs = Ruleset::from_json(&raw).expect("regla valida");
        let (blocked, _) = rs.evaluate(&ctx()).expect("eval");
        blocked.is_some()
    }

    fn rejected(raw: &str) {
        assert!(Ruleset::from_json(raw).is_err(), "debia ser rechazado: {}", raw);
    }

    #[test]
    fn eq_and_ne_on_strings() {
        assert!(blocks("repo eq "acme/api""));
        assert!(!blocks("repo eq "other/repo""));
        assert!(blocks("repo ne "other/repo""));
    }

    #[test]
    fn numeric_comparisons() {
        assert!(blocks("confidence ge 0.9"));
        assert!(blocks("confidence gt 0.5 and risk le 0.25"));
        assert!(!blocks("risk gt 0.5"));
        assert!(blocks("attempts eq 2"));
        assert!(blocks("attempts ge 2"));
        assert!(!blocks("attempts lt 2"));
    }

    #[test]
    fn and_binds_tighter_than_or() {
        // or(and(x, y), z): falso and verdadero-or-falso => falso
        assert!(!blocks("repo eq "x" or repo eq "acme/api" and attempts eq 99"));
        // and(or(x, y), z): verdadero => verdadero
        assert!(blocks("(repo eq "x" or repo eq "acme/api") and attempts eq 2"));
    }

    #[test]
    fn not_and_symbol_operators() {
        assert!(!blocks("not (repo eq "acme/api")"));
        assert!(blocks("!(repo eq "acme/api")"));
        assert!(blocks("repo eq "acme/api" && attempts eq 2 || risk gt 1"));
    }

    #[test]
    fn contains_and_in() {
        assert!(blocks("signature contains "E500""));
        assert!(blocks("repo contains "api""));
        assert!(!blocks("repo contains "nope""));
        assert!(blocks("error_step in {build deploy}"));
        assert!(blocks("error_step in {build, deploy}"));
        assert!(!blocks("error_step in {deploy, test}"));
    }

    #[test]
    fn block_first_match_wins_and_observe_reports() {
        let raw = "[{"id":"o1","expression":"attempts ge 1","action":"observe"},{"id":"b1","expression":"confidence ge 0.5","action":"block"},{"id":"b2","expression":"risk lt 1","action":"block"}]";
        let rs = Ruleset::from_json(raw).expect("ruleset valido");
        let (blocked, observed) = rs.evaluate(&ctx()).expect("eval");
        assert_eq!(blocked.as_deref(), Some("b1"));
        assert_eq!(observed, vec!["o1".to_string()]);
    }

    #[test]
    fn empty_config_is_noop() {
        let rs = Ruleset::from_json("").expect("vacio");
        assert!(rs.evaluate(&ctx()).expect("eval").0.is_none());
        let rs = Ruleset::from_json("[]").expect("vacio");
        assert!(rs.evaluate(&ctx()).expect("eval").0.is_none());
    }

    #[test]
    fn invalid_configs_rejected_fail_closed() {
        // JSON invalido
        rejected("{no es json}");
        // campo fuera del esquema
        rejected("[{"id":"r","expression":"wat eq 1","action":"block"}]");
        // tipos mezclados: string vs numero
        rejected("[{"id":"r","expression":"repo eq 5","action":"block"}]");
        // int vs float
        rejected("[{"id":"r","expression":"attempts eq 2.0","action":"block"}]");
        // orden entre strings
        rejected("[{"id":"r","expression":"repo gt "a"","action":"block"}]");
        // sintaxis: tokens sobrantes
        rejected("[{"id":"r","expression":"repo eq "a" foo","action":"block"}]");
        // sintaxis: string sin cerrar
        rejected("[{"id":"r","expression":"repo eq "a","action":"block"}]");
        // conjunto vacio
        rejected("[{"id":"r","expression":"repo in {}","action":"block"}]");
        // id vacio
        rejected("[{"id":"","expression":"true","action":"block"}]");
        // accion inexistente (no existe allow: solo block u observe)
        rejected("[{"id":"r","expression":"true","action":"allow"}]");
        // campo desconocido en el JSON de regla
        rejected("[{"id":"r","expression":"true","action":"block","extra":1}]");
    }

    #[test]
    fn from_incident_and_action_maps_all_fields() {
        let incident = Incident {
            id: "i-1".to_string(),
            source: "ci".to_string(),
            error_code: "E1".to_string(),
            error_step: "test".to_string(),
            command: "cargo test".to_string(),
            message: "boom".to_string(),
            project: "acme/api".to_string(),
            attempts: 3,
            stack_hint: String::new(),
            language_hint: String::new(),
            framework_hint: String::new(),
            verified: false,
            status: "open".to_string(),
        };
        let c = RuleContext::from_incident_and_action(
            &incident,
            "acme/api",
            "E1|test|cargo test",
            "pin_dependency",
            0.75,
            0.3,
        );
        assert_eq!(c.get(F_ERROR_CODE), Some(&Value::Str("E1".to_string())));
        assert_eq!(c.get(F_ERROR_STEP), Some(&Value::Str("test".to_string())));
        assert_eq!(c.get(F_SOURCE), Some(&Value::Str("ci".to_string())));
        assert_eq!(c.get(F_OPERATOR), Some(&Value::Str("pin_dependency".to_string())));
        assert_eq!(c.get(F_CONFIDENCE), Some(&Value::Num(0.75)));
        assert_eq!(c.get(F_RISK), Some(&Value::Num(0.3)));
        assert_eq!(c.get(F_ATTEMPTS), Some(&Value::Int(3)));
        assert_eq!(c.get(F_REPO), Some(&Value::Str("acme/api".to_string())));
    }
}
