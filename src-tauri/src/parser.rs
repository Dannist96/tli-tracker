//! Turns raw `UE_game.log` lines into typed game events.
//!
//! The game prints network messages as a header line followed by `+key [value]`
//! tree lines, and inventory changes as `ItemChange@ ProtoName=X start` ...
//! `end` blocks. This module assembles those blocks and classifies them.

use regex::Regex;
use serde_json::{Map, Value};
use std::sync::LazyLock;

/// Base id that the game reports for an internal placeholder; never real loot.
const IGNORED_BASE_ID: &str = "100200";
/// Flame Elementium, the currency every price is expressed in.
pub const CURRENCY_ID: &str = "100300";

static TS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[(\d{4})\.(\d{2})\.(\d{2})-(\d{2})\.(\d{2})\.(\d{2}):(\d{1,3})\]").unwrap());
static NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)SoundTriggerTimestamps|GameplayLog|E_Divide|Warning|PushMgr@RecvLogicMsg").unwrap()
});
static MSG_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\[[^\]]+\]\[[^\]]+\][a-zA-Z]+:\sDisplay:\s\[Game\]\s-+.*(SendMessage|PushMessage|RecvMessage|RecvPushMsg|XchgReceive)\sSTT-+([^-]+)-+SynId\s=\s(\d*)",
    )
    .unwrap()
});
static MSG_IGNORED_ACTIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)BroadcastChat|SyncArea|Spv3Info|SpGuideInfo|SpCardInfo|SpTalentTreeInfo|SavePlayerClientData|S11GetGamePlayData|ReturnClientStoredData|MainCityFrameDataApi|SetClientStoredData|UpdateSpeedUpPointLag|Ping|GetRollNotice|ActiveFlag|HeartBeat|GetServerTime|GlobalSpeedUpPointList",
    )
    .unwrap()
});
static MSG_END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(SendMessage|PushMessage|RecvMessage|RecvPushMsg)\sEnd").unwrap());
static ITEM_BLOCK_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[a-zA-Z]+:\sDisplay:\s\[Game\]\sItemChange@\sProtoName=([^\s]+)\sstart").unwrap()
});
static ITEM_BLOCK_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[a-zA-Z]+:\sDisplay:\s\[Game\]\sItemChange@\sProtoName=([^\s]+)\send").unwrap()
});
static ITEM_RESET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)ItemChange@\sReset\sPageId=(\d+)").unwrap());
static FAST_ENTER_LEVEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[a-zA-Z]+:\sDisplay:\s\[Game\]\sUGameMgr::FastEnterLevel\(\)").unwrap());
static JOIN_FIGHT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[a-zA-Z]+:\sDisplay:\s\[Game\]\sSwitchBattleAreaUtil:_JoinFightSuccess\(\)").unwrap()
});
static GAME_LOG_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[[^\]]+\].*GameLog.*").unwrap());
static ITEM_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\[([^\]]+)\]\[[^\]]+\][a-zA-Z]+:\sDisplay:\s\[Game\]\sItemChange@\s(Delete|Update|Add)\sId=(\d+_[^\s]+)\s(?:BagNum=)?([^\s]+|)\s?in\sPageId=(-?\d+)\sSlotId=(\d+)",
    )
    .unwrap()
});
static TREE_VALUE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\S+).*\[([^\]]*)\]").unwrap());

/// One stack change inside an `ItemChange` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemLine {
    /// Unique stack key, `<baseId>_<uid>`.
    pub item_id: String,
    pub base_id: String,
    pub time: i64,
    /// New stack total after the change; 0 for deletes.
    pub quantity: i64,
    pub page_id: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Items picked up from the ground.
    Loot { ts: i64, items: Vec<ItemLine> },
    /// Items consumed to open or modify a map.
    MapCost { ts: i64, items: Vec<ItemLine> },
    /// The map device was opened.
    MapOpen { ts: i64 },
    /// A bag page was fully re-sent (sorting, opening the page).
    PageReset { ts: i64, page: Option<i32>, items: Vec<ItemLine> },
    /// Server-pushed inventory change outside of a pickup.
    Push2 { ts: i64, items: Vec<ItemLine> },
    /// Items received from the auction house mailbox.
    AuctionReceive { ts: i64, items: Vec<ItemLine> },
    /// NPC exchange; a few memory items count as loot.
    Exchange { ts: i64, items: Vec<ItemLine> },
    /// Any other inventory change (selling, crafting, moving).
    OtherItemChange { ts: i64, items: Vec<ItemLine> },
    /// Result of an in-game auction price search.
    SearchPrice { ts: i64, base_id: String, price: f64, samples: usize },
    PlayerData { ts: i64, player_id: String, season_id: String, name: String },
    /// Entered a new area (map, town, hideout).
    JoinFight { ts: i64, info: AreaInfo },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AreaInfo {
    pub map_type: String,
    pub check_type: String,
    pub area_id: String,
    pub map_id: String,
    pub owner_player_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BlockKind {
    Message,
    ItemChange,
    ItemReset,
    JoinFight,
}

#[derive(Debug)]
struct Block {
    kind: BlockKind,
    start: i64,
    /// `SendMessage`, `RecvMessage`, ... for messages.
    direction: String,
    /// Message action or item ProtoName.
    action: String,
    /// Page id for item reset blocks.
    page: Option<i32>,
    data: Vec<String>,
}

#[derive(Debug, Default)]
struct SearchContext {
    item_id: Option<String>,
    legendary: bool,
    legendary_any_affix: bool,
    prism: bool,
    prism_default_query: bool,
    stop: bool,
}

#[derive(Debug, Default)]
pub struct Parser {
    block: Option<Block>,
    lines_without_item: u32,
    lines_without_tree: u32,
    search: SearchContext,
    last_ts: i64,
}

impl Parser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one log line; returns the events completed by it.
    pub fn feed(&mut self, line: &str) -> Vec<Event> {
        let line = line.trim_end_matches(['\r', '\n']);
        if let Some(ts) = parse_timestamp(line) {
            self.last_ts = ts;
        }
        if NOISE.is_match(line) {
            return Vec::new();
        }

        // Explicit end markers are consumed; a block that merely ran out of
        // matching lines is closed and the current line is processed normally,
        // since it may open the next block.
        if MSG_END.is_match(line) || ITEM_BLOCK_END.is_match(line) || FAST_ENTER_LEVEL.is_match(line) {
            self.lines_without_item = 0;
            self.lines_without_tree = 0;
            return self.flush();
        }
        let mut out = Vec::new();
        if self.lines_without_item > 4 || self.lines_without_tree > 2 {
            self.lines_without_item = 0;
            self.lines_without_tree = 0;
            out = self.flush();
        }
        out.extend(self.feed_open(line));
        out
    }

    fn feed_open(&mut self, line: &str) -> Vec<Event> {

        if let Some(caps) = MSG_START.captures(line) {
            if !MSG_IGNORED_ACTIONS.is_match(&caps[2]) {
                let out = self.flush();
                self.block = Some(Block {
                    kind: BlockKind::Message,
                    start: self.last_ts,
                    direction: caps[1].to_string(),
                    action: caps[2].trim().to_string(),
                    page: None,
                    data: Vec::new(),
                });
                self.lines_without_item = 0;
                return out;
            }
        }
        if let Some(caps) = ITEM_BLOCK_START.captures(line) {
            let out = self.flush();
            self.block = Some(Block {
                kind: BlockKind::ItemChange,
                start: self.last_ts,
                direction: "ItemChange".into(),
                action: caps[1].to_string(),
                page: None,
                data: Vec::new(),
            });
            self.lines_without_item = 0;
            return out;
        }

        // Inside a reset block, the next `Reset PageId=N` starts the next page.
        if self.block.as_ref().is_some_and(|b| b.kind == BlockKind::ItemReset) && !ITEM_LINE.is_match(line) {
            if let Some(page) = ITEM_RESET.captures(line).and_then(|c| c[1].parse().ok()) {
                let out = self.flush();
                self.block = Some(reset_block(self.last_ts, page));
                self.lines_without_item = 0;
                return out;
            }
        }

        match self.block.as_mut() {
            Some(block) if !line.contains("errCode") => match block.kind {
                BlockKind::Message => {
                    if !GAME_LOG_LINE.is_match(line) {
                        block.data.push(line.to_string());
                    }
                }
                BlockKind::ItemChange => block.data.push(line.to_string()),
                BlockKind::ItemReset => {
                    if ITEM_LINE.is_match(line) {
                        self.lines_without_item = 0;
                        block.data.push(line.to_string());
                    } else {
                        self.lines_without_item += 1;
                        block.data.push(line.to_string());
                    }
                }
                BlockKind::JoinFight => {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with('+') || trimmed.starts_with('|') {
                        block.data.push(line.to_string());
                        self.lines_without_tree = 0;
                    } else {
                        self.lines_without_tree += 1;
                    }
                }
            },
            Some(_) => {}
            None => {
                if let Some(page) = ITEM_RESET.captures(line).and_then(|c| c[1].parse().ok()) {
                    self.block = Some(reset_block(self.last_ts, page));
                    self.lines_without_item = 0;
                } else if JOIN_FIGHT.is_match(line) {
                    self.block = Some(Block {
                        kind: BlockKind::JoinFight,
                        start: self.last_ts,
                        direction: String::new(),
                        action: "JoinFight".into(),
                        page: None,
                        data: Vec::new(),
                    });
                    self.lines_without_tree = 0;
                }
            }
        }
        Vec::new()
    }

    /// Called when the log has gone quiet: blocks that only end when a
    /// non-matching line arrives (area entry, page reset) are complete now.
    pub fn flush_idle(&mut self) -> Vec<Event> {
        match self.block.as_ref().map(|b| b.kind) {
            Some(BlockKind::JoinFight | BlockKind::ItemReset) => {
                self.lines_without_item = 0;
                self.lines_without_tree = 0;
                self.flush()
            }
            _ => Vec::new(),
        }
    }

    /// Completes the open block, if any.
    pub fn flush(&mut self) -> Vec<Event> {
        match self.block.take() {
            Some(block) => self.complete(block).into_iter().collect(),
            None => Vec::new(),
        }
    }

    fn complete(&mut self, b: Block) -> Option<Event> {
        let ts = b.start;
        let text = || b.data.join("\n");
        match b.kind {
            BlockKind::ItemReset => {
                return Some(Event::PageReset { ts, page: b.page, items: parse_items(&b.data) })
            }
            BlockKind::JoinFight => return Some(Event::JoinFight { ts, info: area_info(&parse_tree(&text())) }),
            BlockKind::ItemChange => {
                let items = parse_items(&b.data);
                return Some(match b.action.as_str() {
                    "Spv3Open" | "Spv3RefreshAffix" | "ClimbTowerOpen" => Event::MapCost { ts, items },
                    "PickItem" | "PickItems" => Event::Loot { ts, items },
                    "ResetItemsLayout" => {
                        let page = b.data.iter().find_map(|l| ITEM_RESET.captures(l)).and_then(|c| c[1].parse().ok());
                        Event::PageReset { ts, page, items }
                    }
                    "Push2" => Event::Push2 { ts, items },
                    "XchgReceive" => Event::AuctionReceive { ts, items },
                    "ExchangeItem" => Event::Exchange { ts, items },
                    _ => Event::OtherItemChange { ts, items },
                });
            }
            BlockKind::Message => {}
        }

        let send = b.direction.eq_ignore_ascii_case("SendMessage");
        let recv = b.direction.eq_ignore_ascii_case("RecvMessage");
        match b.action.as_str() {
            "Spv3Open" => Some(Event::MapOpen { ts }),
            "XchgSyncSearchPrice" if send => {
                let tree = parse_tree(&text());
                if let Some(id) = str_at(&tree, &["itemBaseId"]).filter(|id| is_positive_id(id)) {
                    self.search = SearchContext { item_id: Some(id), ..Default::default() };
                }
                None
            }
            "XchgSearchPrice" if send => {
                self.begin_search(&parse_tree(&text()));
                None
            }
            "XchgSearchPrice" if recv => self.finish_search(ts, &text()),
            "GetPlayerData" if recv => {
                let tree = parse_tree(&text());
                let player = tree.get("player").cloned().unwrap_or(Value::Null);
                let pick = |keys: &[&str]| keys.iter().find_map(|k| str_at(&player, &[k])).unwrap_or_default();
                let player_id = pick(&["playerId", "PlayerId"]);
                let season_id = pick(&["seasonId", "SeasonId"]);
                if player_id.is_empty() {
                    return None;
                }
                Some(Event::PlayerData { ts, player_id, season_id, name: pick(&["name", "Name", "playerName"]) })
            }
            _ => None,
        }
    }

    /// Records which item an auction search is for and whether its result is
    /// a plain market price (filtered legendary/prism searches are not).
    fn begin_search(&mut self, tree: &Value) {
        let item_id = str_at(tree, &["filters", "1", "refer"]);
        let mut ctx = SearchContext { item_id: item_id.clone(), ..Default::default() };
        let id = item_id.unwrap_or_default();
        ctx.prism = id.len() == 6 && id.starts_with("3606");
        ctx.prism_default_query = ctx.prism;
        let support_skill = id.len() == 4 && (id.starts_with("77") || id.starts_with("78") || id.starts_with("79"));
        if id == "350900" || id == "360701" {
            ctx.legendary = true;
        }
        const PRISM_DEFAULT_KEYS: [&str; 4] = ["4", "51", "14", "53"];
        if let Some(filters) = tree.get("filters").and_then(Value::as_object) {
            for filter in filters.values() {
                let key = str_at(filter, &["key"]).unwrap_or_default();
                if key == "16" {
                    ctx.legendary = true;
                }
                let Some(params) = filter.get("params").filter(|p| p.is_object()) else { continue };
                if support_skill || ctx.prism {
                    ctx.stop = true;
                }
                let min = str_at(params, &["1", "min"]);
                let min_set = str_at(params, &["1", "minSet"]).is_some_and(|s| s == "1" || s == "true");
                if key == "32" && min.as_deref() == Some("0") {
                    ctx.legendary_any_affix = true;
                }
                if !key.is_empty() && !PRISM_DEFAULT_KEYS.contains(&key.as_str()) {
                    ctx.prism_default_query = false;
                } else if key == "51" && !(min_set && min.as_deref() == Some("2")) {
                    ctx.prism_default_query = false;
                }
            }
        }
        self.search = ctx;
    }

    fn finish_search(&mut self, ts: i64, text: &str) -> Option<Event> {
        let ctx = std::mem::take(&mut self.search);
        let base_id = ctx.item_id.filter(|id| is_positive_id(id))?;
        if ctx.stop {
            return None;
        }
        let accepted = if ctx.legendary {
            ctx.legendary_any_affix
        } else {
            !ctx.prism || ctx.prism_default_query
        };
        if !accepted {
            return None;
        }
        let (price, samples) = market_price(&unit_prices(&parse_tree(text)))?;
        Some(Event::SearchPrice { ts, base_id, price, samples })
    }
}

fn reset_block(ts: i64, page: i32) -> Block {
    Block {
        kind: BlockKind::ItemReset,
        start: ts,
        direction: "ItemChange".into(),
        action: "ItemChangeReset".into(),
        page: Some(page),
        data: Vec::new(),
    }
}

fn is_positive_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 12 && id.bytes().all(|b| b.is_ascii_digit()) && id.bytes().any(|b| b != b'0')
}

/// Log timestamps are UTC wall-clock: `[2026.07.24-08.04.42:410]`.
pub fn parse_timestamp(line: &str) -> Option<i64> {
    let c = TS.captures(line)?;
    let n = |i: usize| c[i].parse::<i64>().ok();
    let (y, mo, d, h, mi, s) = (n(1)?, n(2)?, n(3)?, n(4)?, n(5)?, n(6)?);
    let ms: i64 = format!("{:0<3}", &c[7]).parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400_000 + h * 3_600_000 + mi * 60_000 + s * 1000 + ms)
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn parse_items(lines: &[String]) -> Vec<ItemLine> {
    lines
        .iter()
        .filter_map(|line| {
            let c = ITEM_LINE.captures(line.trim())?;
            let item_id = c[3].to_string();
            let base_id = item_id.split('_').next().unwrap_or_default().to_string();
            if base_id == IGNORED_BASE_ID {
                return None;
            }
            Some(ItemLine {
                time: parse_timestamp(&format!("[{}]", &c[1])).unwrap_or(0),
                quantity: c[4].parse().unwrap_or(0),
                page_id: c[5].parse().ok()?,
                item_id,
                base_id,
            })
        })
        .collect()
}

/// Parses the game's `+a+b [value]` / `|  +c [value]` tree dump into JSON.
pub fn parse_tree(text: &str) -> Value {
    let mut root = Map::new();
    let mut last_keys: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("+errCode") {
            continue;
        }
        let mut expanded = line.to_string();
        for key in &last_keys {
            expanded = expanded.replacen('|', &format!("+{key}"), 1);
        }
        let keys: Vec<String> =
            expanded.split('+').map(str::trim).filter(|k| !k.is_empty()).map(String::from).collect();
        last_keys = keys.iter().filter(|k| !TREE_VALUE.is_match(k)).cloned().collect();

        let mut ctx = &mut root;
        for key in &keys {
            if let Some(c) = TREE_VALUE.captures(key) {
                ctx.insert(c[1].to_string(), Value::String(c[2].to_string()));
                break;
            }
            let entry = ctx.entry(key.clone()).or_insert_with(|| Value::Object(Map::new()));
            match entry {
                Value::Object(next) => ctx = next,
                _ => break,
            }
        }
    }
    Value::Object(root)
}

pub fn str_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut cur = value;
    for key in path {
        cur = cur.get(*key)?;
    }
    match cur {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

fn area_info(tree: &Value) -> AreaInfo {
    let get = |path: &[&str]| str_at(tree, path).unwrap_or_default();
    AreaInfo {
        map_type: get(&["maptype"]),
        check_type: get(&["checkType"]),
        area_id: get(&["areaId"]),
        map_id: get(&["mapId"]),
        owner_player_id: get(&["worldInitArgs", "ownerPlayerId"]),
    }
}

fn unit_prices(tree: &Value) -> Vec<f64> {
    let mut out = Vec::new();
    let Some(prices) = tree.get("prices").and_then(Value::as_object) else { return out };
    for entry in prices.values() {
        if str_at(entry, &["currency"]).as_deref() != Some(CURRENCY_ID) {
            continue;
        }
        if let Some(units) = entry.get("unitPrices").and_then(Value::as_object) {
            out.extend(
                units.values().filter_map(|v| v.as_str()?.trim().parse::<f64>().ok()).filter(|p| p.is_finite() && *p > 0.0),
            );
        }
    }
    out
}

/// Picks a representative price from auction listings: drops a lone lowball
/// on expensive items, otherwise filters IQR outliers and takes a low
/// percentile that shrinks as the sample grows.
pub fn market_price(prices: &[f64]) -> Option<(f64, usize)> {
    let mut sorted: Vec<f64> = prices.iter().copied().filter(|p| p.is_finite() && *p > 0.0).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.total_cmp(b));
    let min = sorted[0];
    let min_count = sorted.iter().filter(|p| **p == min).count();
    let chosen = if sorted.len() > 1 && min > 100.0 && min_count <= 3 && min_count < sorted.len() {
        sorted[min_count]
    } else {
        let filtered = filter_outliers(&sorted);
        let q = match sorted.len() {
            0..=5 => 0.5,
            6..=12 => 0.35,
            _ => 0.25,
        };
        percentile(&filtered, q).or_else(|| percentile(&sorted, 0.5)).unwrap_or(min)
    };
    let price = round_price(chosen);
    Some((price, sorted.iter().filter(|p| **p <= chosen).count()))
}

fn percentile(sorted: &[f64], q: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let pos = (sorted.len() - 1) as f64 * q.clamp(0.0, 1.0);
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    let frac = pos - lo as f64;
    Some(sorted[lo] * (1.0 - frac) + sorted[hi] * frac)
}

fn filter_outliers(sorted: &[f64]) -> Vec<f64> {
    if sorted.len() < 6 {
        return sorted.to_vec();
    }
    let (Some(q1), Some(q3)) = (percentile(sorted, 0.25), percentile(sorted, 0.75)) else {
        return sorted.to_vec();
    };
    let iqr = q3 - q1;
    if !iqr.is_finite() || iqr <= 0.0 {
        return sorted.to_vec();
    }
    let kept: Vec<f64> = sorted.iter().copied().filter(|p| *p >= q1 - 1.5 * iqr && *p <= q3 + 1.5 * iqr).collect();
    if kept.is_empty() { sorted.to_vec() } else { kept }
}

fn round_price(p: f64) -> f64 {
    let digits = if p < 0.1 {
        4
    } else if p < 1.0 {
        3
    } else if p < 10.0 {
        2
    } else if p < 100.0 {
        1
    } else {
        return p.round();
    };
    let f = 10f64.powi(digits);
    (p * f).round() / f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(parser: &mut Parser, text: &str) -> Vec<Event> {
        let mut out = Vec::new();
        for line in text.lines() {
            out.extend(parser.feed(line));
        }
        out.extend(parser.flush());
        out
    }

    #[test]
    fn timestamp_is_utc_millis() {
        assert_eq!(parse_timestamp("[1970.01.01-00.00.01:5][  0]x"), Some(1500));
        assert_eq!(parse_timestamp("[2026.07.24-08.04.42:410][  0]TLGame"), Some(1_784_880_282_410));
        assert_eq!(parse_timestamp("[2026.13.24-08.04.42:410]"), None);
    }

    #[test]
    fn tree_handles_pipes_and_nesting() {
        let text = "+areaId [110_abc]\n+worldInitArgs+ownerPlayerId [814]\n|             +seed [15]\n+mapId [111000]\n+checkType [MainCity]";
        let tree = parse_tree(text);
        assert_eq!(str_at(&tree, &["areaId"]).as_deref(), Some("110_abc"));
        assert_eq!(str_at(&tree, &["worldInitArgs", "ownerPlayerId"]).as_deref(), Some("814"));
        assert_eq!(str_at(&tree, &["worldInitArgs", "seed"]).as_deref(), Some("15"));
        assert_eq!(str_at(&tree, &["checkType"]).as_deref(), Some("MainCity"));
    }

    #[test]
    fn pickup_block_becomes_loot() {
        let log = "\
[2026.10.25-10.00.00:001][  1]TLGame: Display: [Game] ItemChange@ ProtoName=PickItems start
[2026.10.25-10.00.00:002][  1]TLGame: Display: [Game] ItemChange@ Update Id=100300_aa BagNum=120 in PageId=102 SlotId=3
[2026.10.25-10.00.00:003][  1]TLGame: Display: [Game] ItemChange@ Add Id=5011_bb BagNum=2 in PageId=103 SlotId=0
[2026.10.25-10.00.00:004][  1]TLGame: Display: [Game] ItemChange@ Add Id=100200_cc BagNum=1 in PageId=103 SlotId=1
[2026.10.25-10.00.00:005][  1]TLGame: Display: [Game] ItemChange@ ProtoName=PickItems end";
        let events = feed_all(&mut Parser::new(), log);
        let Event::Loot { items, .. } = &events[0] else { panic!("{events:?}") };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].base_id, "100300");
        assert_eq!(items[0].quantity, 120);
        assert_eq!(items[1].item_id, "5011_bb");
        assert_eq!(items[1].page_id, 103);
    }

    #[test]
    fn join_fight_reads_area() {
        let log = "\
[2026.10.25-10.00.00:001][  1]TLGame: Display: [Game] SwitchBattleAreaUtil:_JoinFightSuccess()
+areaId [110_df24]
+worldInitArgs+ownerPlayerId [814971]
|             +seed [1532]
+mapId [111000]
+maptype [S4GlobalMap]
[2026.10.25-10.00.00:002][  1]TLGame: Display: [Game] a
[2026.10.25-10.00.00:003][  1]TLGame: Display: [Game] b
[2026.10.25-10.00.00:004][  1]TLGame: Display: [Game] c
[2026.10.25-10.00.00:005][  1]TLGame: Display: [Game] d";
        let events = feed_all(&mut Parser::new(), log);
        let Event::JoinFight { info, .. } = &events[0] else { panic!("{events:?}") };
        assert_eq!(info.area_id, "110_df24");
        assert_eq!(info.map_id, "111000");
        assert_eq!(info.owner_player_id, "814971");
        assert_eq!(info.map_type, "S4GlobalMap");
    }

    #[test]
    fn auction_search_yields_price() {
        let log = "\
[2026.10.25-10.00.00:001][1]GameLog: Display: [Game] ----Socket SendMessage STT----XchgSearchPrice----SynId = 11942
+filters+1+refer [5011]
[2026.10.25-10.00.00:002][1]GameLog: Display: [Game] ----Socket SendMessage End----
[2026.10.25-10.00.00:003][1]GameLog: Display: [Game] ----Socket RecvMessage STT----XchgSearchPrice----SynId = 11942
+prices+1+currency [100300]
|       | +unitPrices+1 [10]
|       | |          +2 [12]
|       | |          +3 [12.5]
[2026.10.25-10.00.00:004][1]GameLog: Display: [Game] ----Socket RecvMessage End----";
        let events = feed_all(&mut Parser::new(), log);
        assert_eq!(events, vec![Event::SearchPrice { ts: parse_timestamp("[2026.10.25-10.00.00:003]").unwrap(), base_id: "5011".into(), price: 12.0, samples: 2 }]);
    }

    #[test]
    fn price_drops_lone_lowball() {
        assert_eq!(market_price(&[150.0, 900.0, 950.0]).unwrap().0, 900.0);
        assert_eq!(market_price(&[1.0, 2.0, 3.0]).unwrap().0, 2.0);
        assert!(market_price(&[]).is_none());
    }

    #[test]
    fn reset_blocks_chain_pages() {
        let log = "\
[2026.10.25-10.00.00:001][1]TLGame: Display: [Game] ItemChange@ Reset PageId=102
[2026.10.25-10.00.00:002][1]TLGame: Display: [Game] ItemChange@ Add Id=100300_aa BagNum=50 in PageId=102 SlotId=0
[2026.10.25-10.00.00:003][1]TLGame: Display: [Game] ItemChange@ Reset PageId=103
[2026.10.25-10.00.00:004][1]TLGame: Display: [Game] ItemChange@ Add Id=5011_bb BagNum=3 in PageId=103 SlotId=0";
        let events = feed_all(&mut Parser::new(), log);
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], Event::PageReset { page: Some(102), items, .. } if items.len() == 1));
        assert!(matches!(&events[1], Event::PageReset { page: Some(103), items, .. } if items[0].quantity == 3));
    }
}
