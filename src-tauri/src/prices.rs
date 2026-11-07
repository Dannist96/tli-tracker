//! Price table: public market snapshot + prices seen in the user's own
//! auction searches + manual overrides.

use crate::net::{bounded_number, get_json, is_numeric_id, parse_iso_utc};
use crate::parser::CURRENCY_ID;
use crate::tracker::{LocalPrice, Pricer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

const MAX_ROWS: usize = 20_000;
pub const MAX_PRICE: f64 = 10_000_000.0;
/// Auction house fee taken from every sale.
const SALE_TAX: f64 = 0.125;

/// Item categories from the bundled item list; organs are never priced.
static CATEGORIES: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let raw: HashMap<String, Value> = serde_json::from_str(include_str!("../../src/data/items.json")).unwrap_or_default();
    raw.into_iter().filter_map(|(id, v)| Some((id, v.get("c")?.as_str()?.to_string()))).collect()
});

pub fn category(base_id: &str) -> &str {
    CATEGORIES.get(base_id).map_or("other", String::as_str)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MarketPrice {
    pub price: f64,
    pub updated_at: i64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarketTable {
    pub season_id: String,
    pub fetched_at: i64,
    pub rows: HashMap<String, MarketPrice>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PriceSource {
    Currency,
    Manual,
    Search,
    Market,
    None,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPrice {
    pub price: f64,
    pub source: PriceSource,
    pub updated_at: i64,
}

/// Everything needed to value an item, borrowed for one computation.
pub struct PriceBook<'a> {
    pub market: &'a MarketTable,
    pub local: &'a HashMap<String, LocalPrice>,
    pub manual: &'a HashMap<String, f64>,
    pub excluded: &'a HashSet<String>,
    pub apply_tax: bool,
}

impl PriceBook<'_> {
    pub fn resolve(&self, base_id: &str) -> ResolvedPrice {
        if base_id == CURRENCY_ID {
            return ResolvedPrice { price: 1.0, source: PriceSource::Currency, updated_at: 0 };
        }
        if let Some(p) = self.manual.get(base_id) {
            return ResolvedPrice { price: *p, source: PriceSource::Manual, updated_at: 0 };
        }
        if category(base_id) == "organ" {
            return ResolvedPrice { price: 0.0, source: PriceSource::None, updated_at: 0 };
        }
        let market = self.market.rows.get(base_id);
        let local = self.local.get(base_id);
        match (market, local) {
            (Some(m), Some(l)) if l.ts > m.updated_at => {
                ResolvedPrice { price: l.price, source: PriceSource::Search, updated_at: l.ts }
            }
            (Some(m), _) => ResolvedPrice { price: m.price, source: PriceSource::Market, updated_at: m.updated_at },
            (None, Some(l)) => ResolvedPrice { price: l.price, source: PriceSource::Search, updated_at: l.ts },
            (None, None) => ResolvedPrice { price: 0.0, source: PriceSource::None, updated_at: 0 },
        }
    }
}

impl Pricer for PriceBook<'_> {
    fn loot_value(&self, base_id: &str) -> f64 {
        if self.excluded.contains(base_id) {
            return 0.0;
        }
        let p = self.resolve(base_id).price;
        if self.apply_tax && base_id != CURRENCY_ID { p * (1.0 - SALE_TAX) } else { p }
    }

    fn cost_value(&self, base_id: &str) -> f64 {
        if self.excluded.contains(base_id) { 0.0 } else { self.resolve(base_id).price }
    }
}

/// Downloads the current season's price snapshot.
pub async fn fetch_market(now: i64) -> Result<MarketTable, String> {
    let status = get_json("/api/player/season-status", &[]).await?;
    let season_id = status
        .get("currentSeasonId")
        .and_then(Value::as_str)
        .filter(|s| is_numeric_id(s, 8))
        .ok_or("season status has no valid currentSeasonId")?
        .to_string();
    let body = get_json(&format!("/api/prices-snapshot/{season_id}"), &[]).await?;
    let rows = parse_snapshot(&body);
    if rows.is_empty() {
        return Err("price snapshot was empty".into());
    }
    Ok(MarketTable { season_id, fetched_at: now, rows })
}

pub fn parse_snapshot(body: &Value) -> HashMap<String, MarketPrice> {
    let Some(rows) = body.get("data").and_then(Value::as_array) else { return HashMap::new() };
    rows.iter()
        .take(MAX_ROWS)
        .filter_map(|row| {
            let id = row.get("id")?.as_str().filter(|id| is_numeric_id(id, 12))?;
            let price = bounded_number(row.get("price"), 0.0, MAX_PRICE)?;
            let updated_at = parse_iso_utc(row.get("updatedAt")).unwrap_or(0);
            Some((id.to_string(), MarketPrice { price, updated_at }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hostile_rows_are_dropped() {
        let body = json!({"data": [
            {"id": "5011", "price": 12.5, "updatedAt": "2026-10-25T08:48:37.807Z"},
            {"id": "7744", "price": "8.006"},
            {"id": "../etc", "price": 1},
            {"id": "<script>", "price": 1},
            {"id": "5", "price": -1},
            {"id": "6", "price": 1e30},
            {"id": 10, "price": 1},
            "garbage"
        ]});
        let rows = parse_snapshot(&body);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows["5011"].price, 12.5);
        assert!(rows["5011"].updated_at > 0);
        assert_eq!(rows["7744"].updated_at, 0);
        assert!(parse_snapshot(&json!({"data": "x"})).is_empty());
    }

    #[test]
    fn newest_source_wins_and_tax_applies_to_loot() {
        let mut market = MarketTable::default();
        market.rows.insert("1".into(), MarketPrice { price: 10.0, updated_at: 100 });
        let mut local = HashMap::new();
        local.insert("1".into(), LocalPrice { price: 20.0, ts: 200, samples: 3 });
        let manual = HashMap::new();
        let excluded = HashSet::new();
        let book = PriceBook { market: &market, local: &local, manual: &manual, excluded: &excluded, apply_tax: true };
        assert_eq!(book.resolve("1").price, 20.0);
        assert_eq!(book.loot_value("1"), 17.5);
        assert_eq!(book.cost_value("1"), 20.0);
        assert_eq!(book.loot_value(CURRENCY_ID), 1.0);
    }
}
