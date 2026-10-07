//! Dashboard de observabilidad de negocio (FASE 1, docs/DASHBOARD.md).
//!
//! GET /dashboard agrega los RepairCases persistidos en REPAIR_CASES_KV
//! (clave `repair_case:{correlation_id}`, queue_consumer::persist_case) y
//! pinta HTML+SVG generado server-side en Rust: sin assets JS externos,
//! sin LLM, sin dependencias nuevas (docs/NO_LLM_POLICY.md). La
//! autorizacion fail-closed vive en el handler de lib.rs (secret
//! DASHBOARD_TOKEN, header x-dashboard-token).
//!
//! Limites conscientes del tier gratis: UNA pagina de list (MAX_KEYS) por
//! render. El dashboard es observabilidad, no un export completo. FASE 2
//! (backlog P2 / PYH-60): Workers Analytics Engine para series temporales
//! sin escanear KV.

use worker::*;

/// Binding de KV con los RepairCases (worker/wrangler.toml).
pub const KV_BINDING: &str = "REPAIR_CASES_KV";
/// Prefijo de las claves de RepairCase (queue_consumer::persist_case).
pub const KEY_PREFIX: &str = "repair_case:";
/// Maximo de claves leidas por render (una pagina de list).
pub const MAX_KEYS: u64 = 200;
/// Dias que conserva la tendencia diaria (los mas recientes).
pub const MAX_DAYS: usize = 14;

/// Estadisticas agregadas de los RepairCases.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub total: u32,
    pub verified_pass: u32,
    pub verified_fail: u32,
    pub verification_skipped: u32,
    pub with_pr: u32,
    /// Conteos por operador: conteo descendente, nombre ascendente en
    /// empates (orden total determinista).
    pub by_operator: Vec<(String, u32)>,
    /// Conteos por dia UTC (YYYY-MM-DD), orden ascendente.
    pub by_day: Vec<(String, u32)>,
}

/// Etiqueta estable del operador de un RepairCase serializado.
fn operator_label(case: &serde_json::Value) -> String {
    let op = case.get("action").and_then(|a| a.get("repair_operator"));
    if let Some(name) = op.and_then(|v| v.as_str()) {
        return name.to_string();
    }
    if let Some(id) = op.and_then(|v| v.as_u64()) {
        return format!("operator_{id}");
    }
    String::from("unknown")
}

/// Incrementa el conteo de una clave (inserta si es nueva).
fn bump(counts: &mut Vec<(String, u32)>, key: String) {
    if let Some(entry) = counts.iter_mut().find(|(k, _)| *k == key) {
        entry.1 += 1;
    } else {
        counts.push((key, 1));
    }
}

/// Agrega RepairCases (JSON de crates/repair_types::RepairCase) en Stats.
/// El orden de entrada NO altera el resultado. Los campos ausentes o
/// malformados cuentan en la categoria conservadora correspondiente: el
/// dashboard es observabilidad y nunca abre el camino a una reparacion.
pub fn aggregate(cases: &[serde_json::Value]) -> Stats {
    let mut stats = Stats::default();
    let mut operators: Vec<(String, u32)> = Vec::new();
    let mut days: Vec<(String, u32)> = Vec::new();
    for case in cases {
        stats.total += 1;
        let verification = case
            .get("verification")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        if verification.contains("pass") {
            stats.verified_pass += 1;
        } else if verification.contains("fail") {
            stats.verified_fail += 1;
        } else {
            stats.verification_skipped += 1;
        }
        let has_pr = case
            .get("pr_url")
            .and_then(|v| v.as_str())
            .is_some_and(|u| !u.is_empty());
        if has_pr {
            stats.with_pr += 1;
        }
        bump(&mut operators, operator_label(case));
        if let Some(day) = case
            .get("created_at_unix")
            .and_then(|v| v.as_u64())
            .and_then(day_bucket)
        {
            bump(&mut days, day);
        }
    }
    operators.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    days.sort_by(|a, b| a.0.cmp(&b.0));
    if days.len() > MAX_DAYS {
        let excess = days.len() - MAX_DAYS;
        days.drain(..excess);
    }
    stats.by_operator = operators;
    stats.by_day = days;
    stats
}

/// Convierte unix-ms en "YYYY-MM-DD" (UTC). None con 0: un caso sin
/// timestamp no inventa fecha (un 1970-01-01 seria una mentira visual).
pub fn day_bucket(unix_ms: u64) -> Option<String> {
    if unix_ms == 0 {
        return None;
    }
    let days = (unix_ms / 86_400_000) as i64;
    let (y, m, d) = civil_from_days(days);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// civil-from-days (Howard Hinnant): dias desde 1970-01-01 a (anio, mes,
/// dia) sin crates de fecha. Aritmetica i64, valida para fechas utiles.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// Escapa texto para incrustarlo en HTML (anti-XSS: los valores vienen
/// de KV, no todos controlados por el pipeline).
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Barras horizontales SVG deterministas: ancho proporcional al maximo,
/// etiquetas escapadas (anti-XSS). Color unico: el color no codifica dato.
pub fn svg_bars(rows: &[(String, u32)], width: u32, bar_height: u32) -> String {
    let max = u64::from(rows.iter().map(|(_, v)| *v).max().unwrap_or(1).max(1));
    let height = rows.len() as u32 * bar_height;
    let label_w = 170u32;
    let value_w = 40u32;
    let track_w = u64::from(width.saturating_sub(label_w + value_w + 8));
    let mut svg = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" role=\"img\" width=\"");
    svg.push_str(&width.to_string());
    svg.push_str("\" height=\"");
    svg.push_str(&height.to_string());
    svg.push_str("\">");
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = i as u32 * bar_height;
        let bar_w = track_w * u64::from(*value) / max;
        svg.push_str("<text x=\"0\" y=\"");
        svg.push_str(&(y + bar_height - 5).to_string());
        svg.push_str("\" font-size=\"12\" fill=\"#e6e6e6\">");
        svg.push_str(&escape_html(label));
        svg.push_str("</text><rect x=\"");
        svg.push_str(&label_w.to_string());
        svg.push_str("\" y=\"");
        svg.push_str(&(y + 4).to_string());
        svg.push_str("\" width=\"");
        svg.push_str(&bar_w.to_string());
        svg.push_str("\" height=\"");
        svg.push_str(&bar_height.saturating_sub(8).to_string());
        svg.push_str("\" fill=\"#4f8ef7\"/><text x=\"");
        svg.push_str(&width.saturating_sub(value_w).to_string());
        svg.push_str("\" y=\"");
        svg.push_str(&(y + bar_height - 5).to_string());
        svg.push_str("\" font-size=\"12\" fill=\"#e6e6e6\">");
        svg.push_str(&value.to_string());
        svg.push_str("</text>");
    }
    svg.push_str("</svg>");
    svg
}

fn push_card(html: &mut String, label: &str, value: u32) {
    html.push_str("<div class=\"card\">");
    html.push_str(&escape_html(label));
    html.push_str("<b>");
    html.push_str(&value.to_string());
    html.push_str("</b></div>");
}

/// HTML completo del dashboard: CSS inline, sin assets externos. Los
/// valores de KV pasan por escape_html (labels) o son enteros.
pub fn render_html(stats: &Stats, truncated: bool) -> String {
    let mut html = String::from("<!DOCTYPE html><html lang=\"en\"><head>");
    html.push_str("<meta charset=\"utf-8\">");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    html.push_str("<title>Auto-Healing Agent &mdash; Dashboard</title>");
    html.push_str("<style>");
    html.push_str("body{font-family:system-ui,sans-serif;margin:24px;background:#0f1117;");
    html.push_str("color:#e6e6e6;}h1{font-size:20px;}h2{font-size:15px;margin-bottom:4px;}");
    html.push_str(".cards{display:flex;gap:12px;flex-wrap:wrap;margin:16px 0;}");
    html.push_str(".card{background:#1a1d29;border:1px solid #333;border-radius:8px;");
    html.push_str("padding:10px 16px;font-size:13px;}");
    html.push_str(".card b{display:block;font-size:22px;margin-top:4px;}");
    html.push_str(".note{color:#9a9a9a;font-size:12px;margin-top:16px;}");
    html.push_str("</style></head><body>");
    html.push_str("<h1>Auto-Healing Agent &mdash; Repair Dashboard</h1>");
    html.push_str("<div class=\"cards\">");
    push_card(&mut html, "RepairCases", stats.total);
    push_card(&mut html, "VERIFY pass", stats.verified_pass);
    push_card(&mut html, "VERIFY fail", stats.verified_fail);
    push_card(&mut html, "Pending verify", stats.verification_skipped);
    push_card(&mut html, "PRs opened", stats.with_pr);
    html.push_str("</div>");
    html.push_str("<h2>By operator</h2>");
    html.push_str(&svg_bars(&stats.by_operator, 640, 24));
    html.push_str("<h2>Daily trend (UTC)</h2>");
    html.push_str(&svg_bars(&stats.by_day, 640, 24));
    html.push_str("<p class=\"note\">Data: REPAIR_CASES_KV (repair_case:*), single list page");
    if truncated {
        html.push_str(", more keys exist than shown (pagination limit)");
    }
    html.push_str(". VERIFY authority = GitHub Actions. No LLM in the pipeline.</p>");
    html.push_str("</body></html>");
    html
}

/// Lee UNA pagina de RepairCases y devuelve el HTML agregado. Los
/// errores por clave (KV o JSON malformado) se ignoran: esa clave
/// cuenta como ausente y el resto del dashboard sigue. Error de
/// binding/list: Err (el handler responde 500 fail-closed).
pub async fn render(env: &Env) -> Result<Response> {
    let kv = env.kv(KV_BINDING)?;
    let list = kv
        .list()
        .limit(MAX_KEYS)
        .execute()
        .await
        .map_err(|e| Error::JsError(format!("kv_list_failed: {e}")))?;
    let mut cases: Vec<serde_json::Value> = Vec::new();
    for key in &list.keys {
        if !key.name.starts_with(KEY_PREFIX) {
            continue;
        }
        if let Ok(Some(raw)) = kv.get(&key.name).text().await {
            if let Ok(value) = serde_json::from_str(&raw) {
                cases.push(value);
            }
        }
    }
    let stats = aggregate(&cases);
    let html = render_html(&stats, !list.list_complete);
    let headers = Headers::new();
    headers.set("content-type", "text/html; charset=utf-8")?;
    Ok(Response::from_bytes(html.into_bytes())?.with_headers(headers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(operator: &str, verification: &str, pr: Option<&str>, at: u64) -> serde_json::Value {
        let mut v = serde_json::json!({
            "action": { "repair_operator": operator, "confidence": 0.9, "risk": 0.1 },
            "verification": verification,
            "created_at_unix": at,
        });
        if let Some(p) = pr {
            v["pr_url"] = serde_json::json!(p);
        }
        v
    }

    #[test]
    fn aggregate_counts_and_orders() {
        let cases = vec![
            case("DepPatch", "Skipped", Some("https://pr/1"), 1_000),
            case("DepPatch", "Pass", Some("https://pr/2"), 1_000),
            case("CacheClear", "Fail", None, 86_400_000),
        ];
        let s = aggregate(&cases);
        assert_eq!(s.total, 3);
        assert_eq!(s.verified_pass, 1);
        assert_eq!(s.verified_fail, 1);
        assert_eq!(s.verification_skipped, 1);
        assert_eq!(s.with_pr, 2);
        assert_eq!(s.by_operator[0], (String::from("DepPatch"), 2));
        assert_eq!(s.by_operator[1], (String::from("CacheClear"), 1));
        assert_eq!(s.by_day.len(), 2);
        assert_eq!(s.by_day[0].0, "1970-01-01");
        assert_eq!(s.by_day[1].0, "1970-01-02");
    }

    #[test]
    fn aggregate_tolerates_malformed() {
        let s = aggregate(&[serde_json::json!({})]);
        assert_eq!(s.total, 1);
        assert_eq!(s.verification_skipped, 1);
        assert_eq!(s.by_operator, vec![(String::from("unknown"), 1)]);
        assert!(s.by_day.is_empty());
    }

    #[test]
    fn aggregate_keeps_last_days_only() {
        let mut cases = Vec::new();
        for i in 0..30u64 {
            cases.push(case("X", "Pass", None, i * 86_400_000 + 1));
        }
        let s = aggregate(&cases);
        assert_eq!(s.by_day.len(), MAX_DAYS);
        assert_eq!(s.by_day[0].0, "1970-01-17");
    }

    #[test]
    fn day_bucket_known_dates() {
        assert_eq!(day_bucket(0), None);
        assert_eq!(day_bucket(86_400_000), Some(String::from("1970-01-02")));
        assert_eq!(
            day_bucket(1_791_331_200_000),
            Some(String::from("2026-10-07"))
        );
    }

    #[test]
    fn escape_html_neutralizes_markup() {
        assert_eq!(
            escape_html("<b>&\"'</b>"),
            "&lt;b&gt;&amp;&quot;&#39;&lt;/b&gt;"
        );
    }

    #[test]
    fn svg_bars_escapes_labels() {
        let svg = svg_bars(&[(String::from("<x>"), 3)], 640, 24);
        assert!(svg.contains("&lt;x&gt;"));
        assert!(!svg.contains("<x>"));
    }
}
