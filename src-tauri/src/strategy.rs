//! Public strategy leaderboard, reduced to plain validated fields.

use crate::net::{bounded_number, clean_text, get_json, is_numeric_id};
use serde::Serialize;
use serde_json::Value;

const MAX_ITEMS: usize = 200;
const MAX_STAGES: usize = 40;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StrategyRow {
    pub rank: u32,
    pub name: String,
    pub score: f64,
    pub maps: u32,
    pub avg_earnings: f64,
    pub avg_efficiency: f64,
    pub avg_duration_ms: f64,
    /// Region index 0-4 (see `region.*` locale keys), if known.
    pub region: Option<u8>,
    /// Timemark index 0-13, if known.
    pub timemark: Option<u8>,
    pub talents: String,
    pub level: u32,
    pub player_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leaderboard {
    pub season_id: String,
    pub stage: String,
    pub generated_at: f64,
    pub rows: Vec<StrategyRow>,
}

pub fn is_valid_stage(s: &str) -> bool {
    !s.is_empty() && s.len() <= 16 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

pub async fn current_season() -> Result<String, String> {
    let status = get_json("/api/player/season-status", &[]).await?;
    status
        .get("currentSeasonId")
        .and_then(Value::as_str)
        .filter(|s| is_numeric_id(s, 8))
        .map(String::from)
        .ok_or_else(|| "season status has no valid currentSeasonId".into())
}

pub async fn fetch_stages(season_id: &str) -> Result<Vec<String>, String> {
    let body = get_json("/api/strategy/stage-options", &[("seasonId", season_id)]).await?;
    Ok(parse_stages(&body))
}

pub fn parse_stages(body: &Value) -> Vec<String> {
    let Some(options) = body.pointer("/data/options").and_then(Value::as_array) else { return Vec::new() };
    options
        .iter()
        .take(MAX_STAGES)
        .filter_map(|o| o.get("value")?.as_str().filter(|s| is_valid_stage(s)).map(String::from))
        .collect()
}

pub async fn fetch_leaderboard(season_id: &str, stage: &str) -> Result<Leaderboard, String> {
    if !is_numeric_id(season_id, 8) || !is_valid_stage(stage) {
        return Err("invalid season or stage".into());
    }
    let body = get_json("/api/strategy/leaderboard", &[("seasonId", season_id), ("stage", stage)]).await?;
    Ok(Leaderboard {
        season_id: season_id.to_string(),
        stage: stage.to_string(),
        generated_at: bounded_number(body.pointer("/data/generatedAt"), 0.0, 1e14).unwrap_or(0.0),
        rows: parse_rows(&body),
    })
}

pub fn parse_rows(body: &Value) -> Vec<StrategyRow> {
    let Some(items) = body.pointer("/data/items").and_then(Value::as_array) else { return Vec::new() };
    items
        .iter()
        .take(MAX_ITEMS)
        .filter_map(|it| {
            let num = |k: &str, max: f64| bounded_number(it.get(k), 0.0, max);
            let small = |k: &str, max: f64| num(k, max).map(|n| n as u8);
            Some(StrategyRow {
                rank: num("rank", 100_000.0)? as u32,
                name: clean_text(it.get("name"), 160),
                score: num("score", 1e9)?,
                maps: num("count", 1e9)? as u32,
                avg_earnings: bounded_number(it.get("avgEarnings"), -1e9, 1e9).unwrap_or(0.0),
                avg_efficiency: bounded_number(it.get("avgEfficiency"), -1e9, 1e9).unwrap_or(0.0),
                avg_duration_ms: num("avgDuration", 1e10).unwrap_or(0.0),
                region: small("mapLabel", 4.0),
                timemark: small("timeLabel", 13.0),
                talents: clean_text(it.get("talentLabel"), 120),
                level: num("level", 1000.0).unwrap_or(0.0) as u32,
                player_name: clean_text(it.get("playerName"), 40),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rows_are_validated() {
        let body = json!({"data": {"items": [
            {"rank": 1, "name": "火图 狩猎×23\u{202E}", "score": 22.3, "count": 83, "avgEarnings": 1.4,
             "avgEfficiency": 3.8, "avgDuration": 21503, "mapLabel": "1", "timeLabel": "6",
             "talentLabel": "狩猎、画雪", "level": 95, "playerName": "x#5071", "userId": "525014805598670848"},
            {"rank": "bad", "score": 1, "count": 1},
            {"rank": 2, "score": "1e400", "count": 1}
        ]}});
        let rows = parse_rows(&body);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "火图 狩猎×23");
        assert_eq!(rows[0].region, Some(1));
        assert_eq!(rows[0].timemark, Some(6));
    }

    #[test]
    fn stages_are_whitelisted() {
        let body = json!({"data": {"options": [{"value": "legacy"}, {"value": "day1"}, {"value": "../x"}, {"value": "A"}]}});
        assert_eq!(parse_stages(&body), vec!["legacy", "day1"]);
    }
}
