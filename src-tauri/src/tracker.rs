//! Map/loot state machine fed by parser events.
//!
//! Drops are measured as bag stack deltas keyed by the unique stack id, so a
//! change is counted once no matter which message reported it. A map run
//! starts on entering a new non-town area and ends when the next one starts;
//! time spent in town pauses the run.

use crate::parser::{AreaInfo, Event, ItemLine};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

const TOWN_LIKE_MAP_TYPES: [&str; 3] = ["WorldBossChallengeMap", "BossChallengeMap", "TeamTdMap"];
/// Sub-areas entered from inside a map; they continue the current run.
const SUB_MAP_TYPES: [&str; 5] = ["S12Map", "S7BossMap", "S5NightmareMystic", "S10PillageMap", "S10CampsiteMap"];
/// Memory items that count as loot when exchanged inside a map.
const EXCHANGE_LOOT_IDS: [&str; 4] = ["5140", "5142", "5143", "5144"];
/// Bag pages: 100 gear, 101 skills, 102 currency, 103 other.
const FIRST_BAG_PAGE: i32 = 100;
const CALIBRATION_PAGES: [i32; 2] = [102, 103];
/// Map cost consumed this long before entering a map belongs to it.
const COST_WINDOW_BEFORE_MS: i64 = 10_000;
const COST_WINDOW_AFTER_MS: i64 = 2_500;
const PUSH2_COST_WINDOW_MS: i64 = 1_500;
/// A pickup this large is an inventory refresh, not loot.
const MASS_REFRESH_TYPES: usize = 20;
const MASS_REFRESH_QTY: i64 = 3_000;
/// Stop the live timer if the log goes quiet this long (game closed).
const STALE_AFTER_MS: i64 = 5 * 60_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Drop {
    pub base_id: String,
    pub qty: i64,
    pub ts: i64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub estimated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapRun {
    pub id: u64,
    pub area_id: String,
    pub map_id: String,
    pub map_type: String,
    pub player_id: String,
    pub season_id: String,
    pub start: i64,
    #[serde(default)]
    pub end: Option<i64>,
    /// Active (non-town) time in ms, excluding the currently running segment.
    pub duration_ms: i64,
    #[serde(default)]
    pub sub_areas: u32,
    pub loot: Vec<Drop>,
    pub cost: Vec<Drop>,
    /// Values frozen when the run ends, so history does not drift with prices.
    #[serde(default)]
    pub value: Option<RunValue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunValue {
    pub loot: f64,
    pub cost: f64,
    pub profit: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: String,
    pub season_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    pub base_id: String,
    pub qty: i64,
    pub page: i32,
    pub time: i64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    #[default]
    Paused,
    Active,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPrice {
    pub price: f64,
    pub ts: i64,
    pub samples: usize,
}

/// Values each item for profit calculations.
pub trait Pricer {
    /// Unit value of a looted item (tax already applied) and of a consumed one.
    fn loot_value(&self, base_id: &str) -> f64;
    fn cost_value(&self, base_id: &str) -> f64;
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tracker {
    pub player: Option<Player>,
    pub inventory: HashMap<String, Stack>,
    pub baseline_trusted: bool,
    pub trusted_pages: HashSet<i32>,
    pub seen_since_invalidation: HashSet<String>,
    pub current: Option<MapRun>,
    pub status: RunStatus,
    /// Log timestamp the current active segment started at.
    pub resumed_at: Option<i64>,
    pub history: Vec<MapRun>,
    pub pending_costs: Vec<Drop>,
    pub push2_cache: Vec<Drop>,
    pub session_start: i64,
    pub local_prices: HashMap<String, LocalPrice>,
    pub next_id: u64,
    pub last_log_ts: i64,
    /// Set once any tracking event is seen; tells the UI the game log is enabled.
    pub tracking_seen: bool,
    #[serde(skip)]
    pub dirty: bool,
}

impl Tracker {
    pub fn new(now: i64) -> Self {
        Self { session_start: now, next_id: 1, ..Default::default() }
    }

    pub fn handle(&mut self, event: Event, pricer: &dyn Pricer) {
        self.dirty = true;
        let ts = event_ts(&event);
        self.last_log_ts = self.last_log_ts.max(ts);
        match event {
            Event::Loot { items, .. } => self.on_loot(items),
            Event::MapCost { ts, items } => self.on_map_cost(ts, items),
            Event::MapOpen { .. } => {}
            Event::PageReset { page, items, .. } => self.on_page_reset(page, items),
            Event::Push2 { ts, items } => self.on_push2(ts, items),
            Event::Exchange { items, .. } => {
                for (base_id, delta, ts) in self.apply(&items) {
                    if delta > 0 && self.current.is_some() && EXCHANGE_LOOT_IDS.contains(&base_id.as_str()) {
                        self.add_loot(base_id, delta, ts);
                    }
                }
            }
            Event::AuctionReceive { items, .. } | Event::OtherItemChange { items, .. } => {
                self.apply(&items);
            }
            Event::SearchPrice { ts, base_id, price, samples } => {
                self.local_prices.insert(base_id, LocalPrice { price, ts, samples });
            }
            Event::PlayerData { player_id, season_id, name, .. } => {
                self.on_player(Player { id: player_id, season_id, name }, ts, pricer)
            }
            Event::JoinFight { ts, info } => self.on_join_fight(ts, info, pricer),
        }
    }

    /// Applies an event from older log history: restores player, bag and
    /// searched prices without recording loot or touching map runs.
    pub fn absorb_replayed(&mut self, event: Event) {
        self.dirty = true;
        self.last_log_ts = self.last_log_ts.max(event_ts(&event));
        match event {
            Event::PlayerData { player_id, season_id, name, .. } => {
                if self.player.as_ref().is_none_or(|p| p.id != player_id) {
                    self.inventory.clear();
                }
                self.player = Some(Player { id: player_id, season_id, name });
                self.tracking_seen = true;
                self.invalidate_baseline();
            }
            Event::PageReset { page, items, .. } => self.on_page_reset(page, items),
            Event::SearchPrice { ts, base_id, price, samples } => {
                self.local_prices.insert(base_id, LocalPrice { price, ts, samples });
            }
            Event::Loot { items, .. }
            | Event::MapCost { items, .. }
            | Event::Push2 { items, .. }
            | Event::AuctionReceive { items, .. }
            | Event::Exchange { items, .. }
            | Event::OtherItemChange { items, .. } => {
                self.apply(&items);
            }
            Event::MapOpen { .. } | Event::JoinFight { .. } => self.tracking_seen = true,
        }
    }

    /// Applies stack changes to the bag snapshot. Returns `(baseId, delta, ts)`
    /// for every change whose previous quantity is trusted.
    fn apply(&mut self, items: &[ItemLine]) -> Vec<(String, i64, i64)> {
        self.tracking_seen = true;
        let mut out = Vec::new();
        for it in items {
            if it.page_id < FIRST_BAG_PAGE {
                continue;
            }
            let prev = self.inventory.get(&it.item_id);
            if prev.is_some_and(|p| it.time < p.time) {
                continue;
            }
            let prev_qty = prev.map_or(0, |p| p.qty);
            let trusted = self.baseline_trusted
                || self.trusted_pages.contains(&it.page_id)
                || self.seen_since_invalidation.contains(&it.item_id);
            if it.quantity <= 0 {
                self.inventory.remove(&it.item_id);
            } else {
                self.inventory.insert(
                    it.item_id.clone(),
                    Stack { base_id: it.base_id.clone(), qty: it.quantity, page: it.page_id, time: it.time },
                );
            }
            self.seen_since_invalidation.insert(it.item_id.clone());
            let delta = it.quantity.max(0) - prev_qty;
            if trusted && delta != 0 {
                out.push((it.base_id.clone(), delta, it.time));
            }
        }
        out
    }

    fn on_loot(&mut self, items: Vec<ItemLine>) {
        let types: HashSet<&str> = items.iter().map(|i| i.base_id.as_str()).collect();
        let total: i64 = items.iter().map(|i| i.quantity.max(0)).sum();
        if types.len() > MASS_REFRESH_TYPES && total > MASS_REFRESH_QTY {
            self.apply(&items);
            return;
        }
        let changes = self.apply(&items);
        let mut net: BTreeMap<&str, i64> = BTreeMap::new();
        for (base, delta, _) in &changes {
            *net.entry(base.as_str()).or_default() += delta;
        }
        if net.values().any(|d| *d < 0) {
            // A pickup cannot shrink the bag; our snapshot is out of date.
            self.invalidate_baseline();
            return;
        }
        if self.current.is_none() {
            return;
        }
        for (base_id, delta, ts) in changes {
            if delta > 0 {
                self.add_loot(base_id, delta, ts);
            }
        }
    }

    fn add_loot(&mut self, base_id: String, qty: i64, ts: i64) {
        if let Some(run) = self.current.as_mut() {
            run.loot.push(Drop { base_id, qty, ts, estimated: false });
        }
    }

    fn on_map_cost(&mut self, ts: i64, items: Vec<ItemLine>) {
        let untrusted: Vec<String> = items
            .iter()
            .filter(|i| !(self.baseline_trusted || self.trusted_pages.contains(&i.page_id) || self.seen_since_invalidation.contains(&i.item_id)))
            .map(|i| i.base_id.clone())
            .collect();
        let mut costs: Vec<Drop> = self
            .apply(&items)
            .into_iter()
            .filter(|(_, d, _)| *d < 0)
            .map(|(base_id, d, ts)| Drop { base_id, qty: -d, ts, estimated: false })
            .collect();
        // Without a trusted previous count we only know one unit was used.
        costs.extend(untrusted.into_iter().map(|base_id| Drop { base_id, qty: 1, ts, estimated: true }));
        // Costs paid in town belong to the next map; only costs right around
        // entering (while inside) go to the current one.
        let active = self.status == RunStatus::Active;
        for cost in costs {
            match self.current.as_mut() {
                Some(run)
                    if active
                        && cost.ts >= run.start - COST_WINDOW_BEFORE_MS
                        && cost.ts <= run.start + COST_WINDOW_AFTER_MS =>
                {
                    run.cost.push(cost)
                }
                _ => self.pending_costs.push(cost),
            }
        }
    }

    fn on_push2(&mut self, ts: i64, items: Vec<ItemLine>) {
        let changes = self.apply(&items);
        if self.status == RunStatus::Active {
            return;
        }
        self.push2_cache.extend(
            changes.into_iter().filter(|(_, d, _)| *d < 0).map(|(base_id, d, _)| Drop { base_id, qty: -d, ts, estimated: false }),
        );
    }

    fn on_page_reset(&mut self, page: Option<i32>, items: Vec<ItemLine>) {
        let Some(page) = page.filter(|p| *p >= FIRST_BAG_PAGE) else {
            self.apply(&items);
            return;
        };
        let listed: HashSet<&str> = items.iter().map(|i| i.item_id.as_str()).collect();
        self.inventory.retain(|id, stack| stack.page != page || listed.contains(id.as_str()));
        // A full page dump is the truth; absorb it without recording anything.
        for it in &items {
            if it.page_id < FIRST_BAG_PAGE {
                continue;
            }
            if it.quantity > 0 {
                self.inventory.insert(
                    it.item_id.clone(),
                    Stack { base_id: it.base_id.clone(), qty: it.quantity, page: it.page_id, time: it.time },
                );
            }
            self.seen_since_invalidation.insert(it.item_id.clone());
        }
        self.tracking_seen = true;
        self.trusted_pages.insert(page);
        if CALIBRATION_PAGES.iter().all(|p| self.trusted_pages.contains(p)) {
            self.baseline_trusted = true;
        }
    }

    fn on_player(&mut self, player: Player, ts: i64, pricer: &dyn Pricer) {
        let changed = self.player.as_ref().is_none_or(|p| p.id != player.id || p.season_id != player.season_id);
        if changed {
            self.finish_current(ts, pricer);
            // A new character starts a new session; so does the very first
            // login a fresh install sees, even if it is older than app start.
            if self.player.is_some() || self.history.is_empty() {
                self.session_start = ts;
            }
            self.inventory.clear();
        }
        self.tracking_seen = true;
        self.player = Some(player);
        // A fresh login may follow changes we never saw.
        self.invalidate_baseline();
    }

    fn on_join_fight(&mut self, ts: i64, info: AreaInfo, pricer: &dyn Pricer) {
        self.tracking_seen = true;
        if let (Some(p), false) = (&self.player, info.owner_player_id.is_empty()) {
            if p.id != info.owner_player_id {
                self.pause(ts);
                return;
            }
        }
        let town = info.check_type == "MainCity"
            || TOWN_LIKE_MAP_TYPES.contains(&info.map_type.as_str())
            || (info.map_type.is_empty() && info.check_type.is_empty());
        if town || info.area_id.is_empty() {
            self.pause(ts);
            return;
        }
        if let Some(run) = self.current.as_mut() {
            if run.area_id == info.area_id {
                self.resume(ts);
                return;
            }
            if SUB_MAP_TYPES.contains(&info.map_type.as_str()) {
                run.sub_areas += 1;
                self.resume(ts);
                return;
            }
        }
        self.finish_current(ts, pricer);
        self.start_run(ts, info);
    }

    fn start_run(&mut self, ts: i64, info: AreaInfo) {
        let player = self.player.clone().unwrap_or_default();
        let mut cost: Vec<Drop> = std::mem::take(&mut self.pending_costs);
        cost.extend(
            std::mem::take(&mut self.push2_cache)
                .into_iter()
                .filter(|d| d.ts >= ts - PUSH2_COST_WINDOW_MS && d.ts <= ts),
        );
        self.current = Some(MapRun {
            id: self.next_id,
            area_id: info.area_id,
            map_id: info.map_id,
            map_type: info.map_type,
            player_id: player.id,
            season_id: player.season_id,
            start: ts,
            end: None,
            duration_ms: 0,
            sub_areas: 0,
            loot: Vec::new(),
            cost,
            value: None,
        });
        self.next_id += 1;
        self.status = RunStatus::Active;
        self.resumed_at = Some(ts);
    }

    pub fn pause(&mut self, ts: i64) {
        if self.status == RunStatus::Active {
            if let (Some(run), Some(from)) = (self.current.as_mut(), self.resumed_at) {
                run.duration_ms += (ts - from).max(0);
            }
        }
        self.status = RunStatus::Paused;
        self.resumed_at = None;
        self.dirty = true;
    }

    pub fn resume(&mut self, ts: i64) {
        if self.current.is_some() && self.status == RunStatus::Paused {
            self.status = RunStatus::Active;
            self.resumed_at = Some(ts);
            self.dirty = true;
        }
    }

    /// Ends the current run (if any) and moves it to history.
    pub fn finish_current(&mut self, ts: i64, pricer: &dyn Pricer) {
        self.pause(ts);
        if let Some(mut run) = self.current.take() {
            run.end = Some(run.start.max(ts));
            run.value = Some(value_of(&run, pricer));
            self.history.push(run);
            self.dirty = true;
        }
    }

    pub fn invalidate_baseline(&mut self) {
        self.baseline_trusted = false;
        self.trusted_pages.clear();
        self.seen_since_invalidation.clear();
        self.dirty = true;
    }

    /// Active time of the current run including the running segment.
    pub fn live_duration(&self, now: i64) -> i64 {
        let Some(run) = &self.current else { return 0 };
        let running = match (self.status, self.resumed_at) {
            (RunStatus::Active, Some(from)) => {
                let until = now.min(self.last_log_ts.max(from) + STALE_AFTER_MS);
                (until - from).max(0)
            }
            _ => 0,
        };
        run.duration_ms + running
    }

    pub fn start_new_session(&mut self, now: i64, pricer: &dyn Pricer) {
        self.finish_current(now, pricer);
        self.session_start = now;
        self.dirty = true;
    }
}

pub fn value_of(run: &MapRun, pricer: &dyn Pricer) -> RunValue {
    let loot: f64 = run.loot.iter().map(|d| d.qty as f64 * pricer.loot_value(&d.base_id)).sum();
    let cost: f64 = run.cost.iter().map(|d| d.qty as f64 * pricer.cost_value(&d.base_id)).sum();
    RunValue { loot, cost, profit: loot - cost }
}

fn event_ts(e: &Event) -> i64 {
    match e {
        Event::Loot { ts, .. }
        | Event::MapCost { ts, .. }
        | Event::MapOpen { ts }
        | Event::PageReset { ts, .. }
        | Event::Push2 { ts, .. }
        | Event::AuctionReceive { ts, .. }
        | Event::Exchange { ts, .. }
        | Event::OtherItemChange { ts, .. }
        | Event::SearchPrice { ts, .. }
        | Event::PlayerData { ts, .. }
        | Event::JoinFight { ts, .. } => *ts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat;
    impl Pricer for Flat {
        fn loot_value(&self, id: &str) -> f64 {
            if id == "100300" { 1.0 } else { 10.0 }
        }
        fn cost_value(&self, _: &str) -> f64 {
            5.0
        }
    }

    fn item(id: &str, qty: i64, page: i32, time: i64) -> ItemLine {
        ItemLine { item_id: id.into(), base_id: id.split('_').next().unwrap().into(), time, quantity: qty, page_id: page }
    }

    fn area(id: &str, check: &str) -> AreaInfo {
        AreaInfo { map_type: "S4Map".into(), check_type: check.into(), area_id: id.into(), map_id: "1061100".into(), owner_player_id: "p1".into() }
    }

    fn calibrated() -> Tracker {
        let mut t = Tracker::new(0);
        t.handle(Event::PlayerData { ts: 0, player_id: "p1".into(), season_id: "s".into(), name: "n".into() }, &Flat);
        t.handle(Event::PageReset { ts: 1, page: Some(102), items: vec![item("100300_a", 100, 102, 1)] }, &Flat);
        t.handle(Event::PageReset { ts: 2, page: Some(103), items: vec![item("5011_b", 2, 103, 2), item("7001_m", 3, 103, 2)] }, &Flat);
        t
    }

    #[test]
    fn full_run_counts_loot_cost_and_time() {
        let mut t = calibrated();
        assert!(t.baseline_trusted);
        // Map opened in the hideout: one map item consumed.
        t.handle(Event::MapCost { ts: 1_000, items: vec![item("7001_m", 2, 103, 1_000)] }, &Flat);
        t.handle(Event::JoinFight { ts: 5_000, info: area("A1", "") }, &Flat);
        t.handle(Event::Loot { ts: 6_000, items: vec![item("100300_a", 130, 102, 6_000), item("5011_new", 1, 103, 6_000)] }, &Flat);
        // Back to town after 60s, then into the next map 30s later.
        t.handle(Event::JoinFight { ts: 65_000, info: area("H", "MainCity") }, &Flat);
        t.handle(Event::JoinFight { ts: 95_000, info: area("A2", "") }, &Flat);

        let run = &t.history[0];
        assert_eq!(run.duration_ms, 60_000);
        assert_eq!(run.cost, vec![Drop { base_id: "7001".into(), qty: 1, ts: 1_000, estimated: false }]);
        let looted: Vec<(&str, i64)> = run.loot.iter().map(|d| (d.base_id.as_str(), d.qty)).collect();
        assert_eq!(looted, vec![("100300", 30), ("5011", 1)]);
        assert_eq!(run.value, Some(RunValue { loot: 40.0, cost: 5.0, profit: 35.0 }));
        assert_eq!(t.current.as_ref().unwrap().area_id, "A2");
    }

    #[test]
    fn returning_to_same_area_resumes_run() {
        let mut t = calibrated();
        t.handle(Event::JoinFight { ts: 0, info: area("A1", "") }, &Flat);
        t.handle(Event::JoinFight { ts: 10_000, info: area("H", "MainCity") }, &Flat);
        t.handle(Event::JoinFight { ts: 20_000, info: area("A1", "") }, &Flat);
        assert!(t.history.is_empty());
        assert_eq!(t.status, RunStatus::Active);
        assert_eq!(t.live_duration(25_000), 15_000);
    }

    #[test]
    fn untrusted_baseline_records_nothing_until_seen() {
        let mut t = Tracker::new(0);
        t.handle(Event::JoinFight { ts: 0, info: area("A1", "") }, &Flat);
        t.handle(Event::Loot { ts: 1, items: vec![item("100300_a", 500, 102, 1)] }, &Flat);
        t.handle(Event::Loot { ts: 2, items: vec![item("100300_a", 510, 102, 2)] }, &Flat);
        let loot = &t.current.as_ref().unwrap().loot;
        assert_eq!(loot.len(), 1);
        assert_eq!(loot[0].qty, 10);
    }

    #[test]
    fn shrinking_pickup_invalidates_baseline() {
        let mut t = calibrated();
        t.handle(Event::JoinFight { ts: 0, info: area("A1", "") }, &Flat);
        t.handle(Event::Loot { ts: 1, items: vec![item("100300_a", 90, 102, 1)] }, &Flat);
        assert!(!t.baseline_trusted);
        assert!(t.current.as_ref().unwrap().loot.is_empty());
    }

    #[test]
    fn other_players_area_pauses() {
        let mut t = calibrated();
        t.handle(Event::JoinFight { ts: 0, info: area("A1", "") }, &Flat);
        let mut foreign = area("A9", "");
        foreign.owner_player_id = "p2".into();
        t.handle(Event::JoinFight { ts: 1_000, info: foreign }, &Flat);
        assert_eq!(t.status, RunStatus::Paused);
        assert_eq!(t.current.as_ref().unwrap().area_id, "A1");
    }
}
