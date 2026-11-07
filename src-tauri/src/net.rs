//! The only network code in the app: read-only GETs of public JSON from the
//! ETor price API. Nothing about the user is ever sent. Responses are size
//! capped, must be JSON, and are reduced to plain validated values by the
//! callers before anything reaches the UI.

use serde_json::Value;
use std::time::Duration;

const API_BASE: &str = "https://etor-beta.710421059.xyz/etor-api";
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .user_agent("tli-tracker")
        .build()
        .map_err(|e| e.to_string())
}

/// GETs `API_BASE + path` (path and query built only from validated values).
pub async fn get_json(path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
    debug_assert!(path.starts_with("/api/") && !path.contains(".."));
    let resp = client()?
        .get(format!("{API_BASE}{path}"))
        .query(query)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let is_json = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.to_ascii_lowercase().starts_with("application/json"));
    if !is_json {
        return Err("unexpected content type".into());
    }
    if resp.content_length().is_some_and(|len| len > MAX_BODY_BYTES as u64) {
        return Err("response too large".into());
    }
    let mut resp = resp;
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > MAX_BODY_BYTES {
            return Err("response too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "invalid JSON".to_string())
}

/// Digits only, bounded length: safe for ids and URL path segments.
pub fn is_numeric_id(s: &str, max_len: usize) -> bool {
    !s.is_empty() && s.len() <= max_len && s.bytes().all(|b| b.is_ascii_digit())
}

/// Plain display text: control and bidi-override characters removed, length capped.
pub fn clean_text(v: Option<&Value>, max_chars: usize) -> String {
    let Some(s) = v.and_then(Value::as_str) else { return String::new() };
    s.chars()
        .filter(|c| !c.is_control() && !matches!(*c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}'))
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_string()
}

/// A finite number within `[min, max]`, accepting numeric strings too.
pub fn bounded_number(v: Option<&Value>, min: f64, max: f64) -> Option<f64> {
    let n = match v? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) if s.len() <= 32 => s.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    (n.is_finite() && n >= min && n <= max).then_some(n)
}

/// Parses `2026-10-25T08:48:37.807Z` style UTC timestamps to epoch ms.
pub fn parse_iso_utc(v: Option<&Value>) -> Option<i64> {
    let s = v?.as_str()?;
    if s.len() > 40 {
        return None;
    }
    let ts = format!(
        "[{}.{}.{}-{}.{}.{}:{}]",
        s.get(0..4)?,
        s.get(5..7)?,
        s.get(8..10)?,
        s.get(11..13)?,
        s.get(14..16)?,
        s.get(17..19)?,
        s.get(20..23).filter(|ms| ms.bytes().all(|b| b.is_ascii_digit())).unwrap_or("0")
    );
    crate::parser::parse_timestamp(&ts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_is_sanitized() {
        assert_eq!(clean_text(Some(&json!("a\u{202E}b\n<c>")), 10), "ab<c>");
        assert_eq!(clean_text(Some(&json!(5)), 10), "");
        assert_eq!(clean_text(Some(&json!("abcdef")), 3), "abc");
    }

    #[test]
    fn numbers_are_bounded() {
        assert_eq!(bounded_number(Some(&json!(5)), 0.0, 10.0), Some(5.0));
        assert_eq!(bounded_number(Some(&json!("7.5")), 0.0, 10.0), Some(7.5));
        assert_eq!(bounded_number(Some(&json!(-1)), 0.0, 10.0), None);
        assert_eq!(bounded_number(Some(&json!("NaN")), 0.0, 10.0), None);
        assert_eq!(bounded_number(Some(&json!([1])), 0.0, 10.0), None);
    }

    #[test]
    fn iso_dates_parse() {
        assert_eq!(parse_iso_utc(Some(&json!("1970-01-01T00:00:01.250Z"))), Some(1250));
        assert_eq!(parse_iso_utc(Some(&json!("garbage"))), None);
    }
}
