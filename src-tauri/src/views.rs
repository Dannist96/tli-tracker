//! Serializable views of the tracker for the UI.

use crate::prices::{PriceSource, ResolvedPrice};
use crate::tracker::{value_of, Drop, MapRun, Player, Pricer, RunStatus};
use crate::{Core, LogStatus, PriceStatus, Settings};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

/// Region index used when the map id is a Myriad/Deep Space special case.
const REGION_MYRIAD: u8 = 5;
const REGION_DEEP_SPACE: u8 = 6;
const TIMEMARK_DEEP_SPACE: u8 = 13;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub now: i64,
    pub log: LogStatus,
    pub tracking_seen: bool,
    pub last_log_ts: i64,
    pub player: Option<Player>,
    pub baseline_trusted: bool,
    pub trusted_pages: Vec<i32>,
    pub status: RunStatus,
    pub current: Option<RunView>,
    pub session: SessionView,
    pub bag: BagView,
    pub prices: PriceInfo,
    pub settings: Settings,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RunView {
    pub id: u64,
    pub area_id: String,
    pub map_id: String,
    pub map_type: String,
    pub region: Option<u8>,
    pub timemark: Option<u8>,
    pub player_id: String,
    pub start: i64,
    pub end: Option<i64>,
    pub duration_ms: i64,
    pub sub_areas: u32,
    pub loot_value: f64,
    pub cost_value: f64,
    pub profit: f64,
    pub per_hour: f64,
    pub loot: Vec<DropLine>,
    pub cost: Vec<DropLine>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DropLine {
    pub base_id: String,
    pub qty: i64,
    pub unit: f64,
    pub value: f64,
    pub priced: bool,
    pub estimated: bool,
    pub last_ts: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub start: i64,
    pub map_count: usize,
    pub map_time_ms: i64,
    pub wall_time_ms: i64,
    pub loot_value: f64,
    pub cost_value: f64,
    pub profit: f64,
    pub per_hour_map: f64,
    pub per_hour_wall: f64,
    pub runs: Vec<RunView>,
    pub loot: Vec<DropLine>,
    pub cost: Vec<DropLine>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BagView {
    pub value: f64,
    pub items: Vec<DropLine>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceInfo {
    pub season_id: String,
    pub fetched_at: i64,
    pub count: usize,
    pub refreshing: bool,
    pub error: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceList {
    pub prices: HashMap<String, ResolvedPrice>,
    pub excluded: Vec<String>,
    pub local_samples: HashMap<String, usize>,
}

/// `(timemark, region)` from a map id: `TTTRR...` where TTT-100 is the
/// timemark index and RR-10 the region index.
pub fn map_label(map_id: &str) -> (Option<u8>, Option<u8>) {
    match map_id {
        "2000000" | "2000001" => return (Some(6), Some(REGION_MYRIAD)),
        "2000002" => return (Some(9), Some(REGION_MYRIAD)),
        "2000003" => return (Some(12), Some(REGION_MYRIAD)),
        _ => {}
    }
    if map_id.len() < 5 || !map_id.bytes().all(|b| b.is_ascii_digit()) {
        return (None, None);
    }
    let t = map_id[0..3].parse::<i32>().unwrap_or(0) - 100;
    let r = map_id[3..5].parse::<i32>().unwrap_or(0) - 10;
    if !(0..=13).contains(&t) {
        return (None, None);
    }
    if t == TIMEMARK_DEEP_SPACE as i32 {
        return (Some(TIMEMARK_DEEP_SPACE), Some(REGION_DEEP_SPACE));
    }
    (Some(t as u8), (0..=4).contains(&r).then_some(r as u8))
}

fn per_hour(value: f64, ms: i64) -> f64 {
    if ms <= 0 { 0.0 } else { value / (ms as f64 / 3_600_000.0) }
}

fn aggregate(drops: &[&Drop], unit: impl Fn(&str) -> f64, priced: impl Fn(&str) -> bool) -> Vec<DropLine> {
    let mut by_id: BTreeMap<&str, (i64, bool, i64)> = BTreeMap::new();
    for d in drops {
        let e = by_id.entry(d.base_id.as_str()).or_default();
        e.0 += d.qty;
        e.1 |= d.estimated;
        e.2 = e.2.max(d.ts);
    }
    let mut lines: Vec<DropLine> = by_id
        .into_iter()
        .map(|(id, (qty, estimated, last_ts))| {
            let u = unit(id);
            DropLine { base_id: id.to_string(), qty, unit: u, value: u * qty as f64, priced: priced(id), estimated, last_ts }
        })
        .collect();
    lines.sort_by(|a, b| b.value.total_cmp(&a.value).then(b.qty.cmp(&a.qty)));
    lines
}

fn run_view(run: &MapRun, duration_ms: i64, live: bool, core: &Core) -> RunView {
    let book = core.book();
    let value = if live { value_of(run, &book) } else { run.value.clone().unwrap_or_else(|| value_of(run, &book)) };
    let (timemark, region) = map_label(&run.map_id);
    let priced = |id: &str| book.resolve(id).source != PriceSource::None;
    RunView {
        id: run.id,
        area_id: run.area_id.clone(),
        map_id: run.map_id.clone(),
        map_type: run.map_type.clone(),
        region,
        timemark,
        player_id: run.player_id.clone(),
        start: run.start,
        end: run.end,
        duration_ms,
        sub_areas: run.sub_areas,
        loot_value: value.loot,
        cost_value: value.cost,
        profit: value.profit,
        per_hour: per_hour(value.profit, duration_ms),
        loot: aggregate(&run.loot.iter().collect::<Vec<_>>(), |id| book.loot_value(id), priced),
        cost: aggregate(&run.cost.iter().collect::<Vec<_>>(), |id| book.cost_value(id), priced),
    }
}

pub fn snapshot(core: &Core, now: i64) -> Snapshot {
    let t = &core.tracker;
    let book = core.book();
    let priced = |id: &str| book.resolve(id).source != PriceSource::None;
    let current = t.current.as_ref().map(|r| run_view(r, t.live_duration(now), true, core));

    let session_runs: Vec<&MapRun> = t.history.iter().filter(|r| r.start >= t.session_start).collect();
    let mut runs: Vec<RunView> = session_runs.iter().rev().map(|r| run_view(r, r.duration_ms, false, core)).collect();
    let map_count = runs.len() + usize::from(current.is_some());
    let map_time_ms = runs.iter().map(|r| r.duration_ms).sum::<i64>() + current.as_ref().map_or(0, |c| c.duration_ms);
    let loot_value = runs.iter().map(|r| r.loot_value).sum::<f64>() + current.as_ref().map_or(0.0, |c| c.loot_value);
    let cost_value = runs.iter().map(|r| r.cost_value).sum::<f64>() + current.as_ref().map_or(0.0, |c| c.cost_value);
    let profit = loot_value - cost_value;
    let first_start = session_runs.first().map(|r| r.start).or(t.current.as_ref().map(|r| r.start));
    let wall_time_ms = first_start.map_or(0, |s| (now.min(t.last_log_ts.max(s) + 5 * 60_000) - s).max(0));

    let all_runs = session_runs.iter().copied().chain(t.current.iter());
    let (mut loot, mut cost): (Vec<&Drop>, Vec<&Drop>) = (Vec::new(), Vec::new());
    for r in all_runs {
        loot.extend(r.loot.iter());
        cost.extend(r.cost.iter());
    }
    runs.truncate(500);

    let mut bag_drops: Vec<Drop> = Vec::new();
    for stack in t.inventory.values() {
        bag_drops.push(Drop { base_id: stack.base_id.clone(), qty: stack.qty, ts: stack.time, estimated: false });
    }
    let bag_items = aggregate(&bag_drops.iter().collect::<Vec<_>>(), |id| book.cost_value(id), priced);

    let mut trusted_pages: Vec<i32> = t.trusted_pages.iter().copied().collect();
    trusted_pages.sort_unstable();
    Snapshot {
        now,
        log: core.log.clone(),
        tracking_seen: t.tracking_seen,
        last_log_ts: t.last_log_ts,
        player: t.player.clone(),
        baseline_trusted: t.baseline_trusted,
        trusted_pages,
        status: t.status,
        current,
        session: SessionView {
            start: t.session_start,
            map_count,
            map_time_ms,
            wall_time_ms,
            loot_value,
            cost_value,
            profit,
            per_hour_map: per_hour(profit, map_time_ms),
            per_hour_wall: per_hour(profit, wall_time_ms),
            runs,
            loot: aggregate(&loot, |id| book.loot_value(id), priced),
            cost: aggregate(&cost, |id| book.cost_value(id), priced),
        },
        bag: BagView { value: bag_items.iter().map(|i| i.value).sum(), items: bag_items },
        prices: price_info(core),
        settings: core.settings.clone(),
    }
}

fn price_info(core: &Core) -> PriceInfo {
    let PriceStatus { refreshing, error } = core.price_status.clone();
    PriceInfo {
        season_id: core.market.season_id.clone(),
        fetched_at: core.market.fetched_at,
        count: core.market.rows.len(),
        refreshing,
        error,
    }
}

pub fn history(core: &Core) -> Vec<RunView> {
    core.tracker.history.iter().rev().map(|r| run_view(r, r.duration_ms, false, core)).collect()
}

pub fn price_list(core: &Core) -> PriceList {
    let book = core.book();
    let ids = core
        .market
        .rows
        .keys()
        .chain(core.tracker.local_prices.keys())
        .chain(core.settings.manual_prices.keys());
    PriceList {
        prices: ids.map(|id| (id.clone(), book.resolve(id))).collect(),
        excluded: core.settings.excluded.iter().cloned().collect(),
        local_samples: core.tracker.local_prices.iter().map(|(k, v)| (k.clone(), v.samples)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_ids_decode() {
        assert_eq!(map_label("1061100"), (Some(6), Some(1)));
        assert_eq!(map_label("1001000"), (Some(0), Some(0)));
        assert_eq!(map_label("1131000"), (Some(13), Some(REGION_DEEP_SPACE)));
        assert_eq!(map_label("2000002"), (Some(9), Some(REGION_MYRIAD)));
        assert_eq!(map_label("abc"), (None, None));
    }
}
