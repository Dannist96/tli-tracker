mod net;
pub mod parser;
pub mod prices;
mod strategy;
pub mod tracker;
mod views;

use parser::{Event, Parser};
use prices::{MarketTable, PriceBook, MAX_PRICE};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
#[cfg(not(target_os = "android"))]
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_fs::FsExt;
use tracker::Tracker;

const POLL_INTERVAL: Duration = Duration::from_millis(300);
const MAX_READ_PER_TICK: u64 = 2 * 1024 * 1024;
/// On first attach to a log we replay its tail to recover player and bag state.
const REPLAY_TAIL_BYTES: u64 = 8 * 1024 * 1024;
const SAVE_INTERVAL_MS: i64 = 10_000;
/// Quiet time after which half-open blocks are treated as complete.
const IDLE_FLUSH_MS: i64 = 1_500;
const LOG_RELATIVE: &str = "steamapps/common/Torchlight Infinite/UE_game/TorchLight/Saved/Logs/UE_game.log";

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Explicit log path chosen by the user; auto-detected when empty.
    pub log_path: Option<String>,
    pub apply_tax: bool,
    pub excluded: HashSet<String>,
    pub manual_prices: HashMap<String, f64>,
    pub language: String,
    pub theme: String,
    pub overlay_opacity: f64,
    pub price_refresh_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            log_path: None,
            apply_tax: true,
            excluded: HashSet::new(),
            manual_prices: HashMap::new(),
            language: "en".into(),
            theme: "system".into(),
            overlay_opacity: 0.92,
            price_refresh_minutes: 10,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    apply_tax: Option<bool>,
    language: Option<String>,
    theme: Option<String>,
    overlay_opacity: Option<f64>,
    price_refresh_minutes: Option<u32>,
}

/// Where the tailer stopped, so a restart continues instead of replaying.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct TailPosition {
    path: String,
    offset: u64,
    signature: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SavedState {
    tracker: Tracker,
    tail: TailPosition,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStatus {
    pub path: String,
    pub found: bool,
    pub auto_detected: bool,
    pub error: String,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceStatus {
    pub refreshing: bool,
    pub error: String,
}

pub struct Core {
    pub tracker: Tracker,
    pub settings: Settings,
    pub market: MarketTable,
    pub log: LogStatus,
    pub price_status: PriceStatus,
    parser: Parser,
    tail: TailPosition,
    partial: Vec<u8>,
    last_growth: i64,
    data_dir: PathBuf,
    settings_dirty: bool,
    market_dirty: bool,
    last_save: i64,
}

impl Core {
    pub fn book(&self) -> PriceBook<'_> {
        PriceBook {
            market: &self.market,
            local: &self.tracker.local_prices,
            manual: &self.settings.manual_prices,
            excluded: &self.settings.excluded,
            apply_tax: self.settings.apply_tax,
        }
    }

    /// Runs `f` with the tracker and a pricer built from the other fields.
    fn with_tracker<R>(&mut self, f: impl FnOnce(&mut Tracker, &PriceBook) -> R) -> R {
        // Local search prices live inside the tracker, so the pricer gets a copy.
        let local = self.tracker.local_prices.clone();
        let book = PriceBook {
            market: &self.market,
            local: &local,
            manual: &self.settings.manual_prices,
            excluded: &self.settings.excluded,
            apply_tax: self.settings.apply_tax,
        };
        f(&mut self.tracker, &book)
    }

    fn handle_events(&mut self, events: Vec<Event>, replay: bool) {
        for event in events {
            if replay {
                self.tracker.absorb_replayed(event);
            } else {
                self.with_tracker(|t, book| t.handle(event, book));
            }
        }
    }

    fn save(&mut self, force: bool) {
        let now = now_ms();
        if !force && now - self.last_save < SAVE_INTERVAL_MS {
            return;
        }
        self.last_save = now;
        if self.tracker.dirty || force {
            let state = SavedState { tracker: std::mem::take(&mut self.tracker), tail: self.tail.clone() };
            write_json(&self.data_dir.join("state.json"), &state);
            self.tracker = state.tracker;
            self.tracker.dirty = false;
        }
        if self.settings_dirty || force {
            write_json(&self.data_dir.join("settings.json"), &self.settings);
            self.settings_dirty = false;
        }
        if self.market_dirty || force {
            write_json(&self.data_dir.join("market.json"), &self.market);
            self.market_dirty = false;
        }
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    let Ok(text) = serde_json::to_vec(value) else { return };
    let tmp = path.with_extension("json.tmp");
    if fs::write(&tmp, text).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}

fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub type Shared = Arc<Mutex<Core>>;

fn notify(app: &AppHandle) {
    let _ = app.emit("tracker-changed", ());
}

// ---------------------------------------------------------------------------
// Log discovery and tailing

/// Steam library roots from the default install and `libraryfolders.vdf`.
#[cfg(target_os = "windows")]
fn steam_libraries() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = ["C:/Program Files (x86)/Steam", "C:/Program Files/Steam"].iter().map(PathBuf::from).collect();
    for root in roots.clone() {
        let Ok(vdf) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) else { continue };
        for line in vdf.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("\"path\"") {
                let value = rest.trim().trim_matches('"').replace("\\\\", "\\");
                if !value.is_empty() {
                    roots.push(PathBuf::from(value));
                }
            }
        }
    }
    roots
}

#[cfg(target_os = "windows")]
fn detect_log_path() -> Option<PathBuf> {
    steam_libraries().into_iter().map(|root| root.join(LOG_RELATIVE)).find(|p| p.is_file())
}

#[cfg(not(target_os = "windows"))]
fn detect_log_path() -> Option<PathBuf> { None }

fn file_signature(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut head = [0u8; 256];
    let n = f.read(&mut head).ok()?;
    Some(head[..n].iter().map(|b| format!("{b:02x}")).collect())
}

fn resolve_log_path(core: &mut Core) -> Option<PathBuf> {
    let (path, auto) = match core.settings.log_path.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => (Some(PathBuf::from(p)), false),
        None => (detect_log_path(), true),
    };
    core.log.auto_detected = auto;
    core.log.path = path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    core.log.found = path.as_ref().is_some_and(|p| p.is_file());
    path.filter(|p| p.is_file())
}

/// Reads whatever was appended since the last tick and feeds it to the parser.
fn tail_tick(core: &mut Core) -> bool {
    let Some(path) = resolve_log_path(core) else { return false };
    let path_str = path.display().to_string();
    let Ok(meta) = fs::metadata(&path) else { return false };
    let size = meta.len();
    let signature = file_signature(&path).unwrap_or_default();

    let mut replay_until = 0u64;
    if core.tail.path != path_str || core.tail.signature != signature || size < core.tail.offset {
        let same_file_restarted = core.tail.path == path_str && core.tail.signature != signature;
        core.parser = Parser::new();
        core.partial.clear();
        // A new game session writes a brand-new log: read it all as live data.
        // Otherwise (first attach) replay the tail only to recover state.
        let start = if same_file_restarted { 0 } else { size.saturating_sub(REPLAY_TAIL_BYTES) };
        if !same_file_restarted {
            replay_until = size;
        }
        core.tail = TailPosition { path: path_str, offset: start, signature };
    }
    if size == core.tail.offset {
        if now_ms() - core.last_growth < IDLE_FLUSH_MS {
            return false;
        }
        let events = core.parser.flush_idle();
        let changed = !events.is_empty();
        core.handle_events(events, false);
        return changed;
    }

    core.last_growth = now_ms();
    let mut changed = false;
    while core.tail.offset < size {
        let len = (size - core.tail.offset).min(MAX_READ_PER_TICK);
        let mut buf = vec![0u8; len as usize];
        let read = fs::File::open(&path)
            .and_then(|mut f| {
                f.seek(SeekFrom::Start(core.tail.offset))?;
                f.read(&mut buf)
            })
            .unwrap_or(0);
        if read == 0 {
            break;
        }
        buf.truncate(read);
        let chunk_end = core.tail.offset + read as u64;
        let replay = chunk_end <= replay_until;
        core.tail.offset = chunk_end;

        let mut data = std::mem::take(&mut core.partial);
        data.extend_from_slice(&buf);
        let complete = data.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        core.partial = data.split_off(complete);
        let mut events = Vec::new();
        for line in String::from_utf8_lossy(&data).lines() {
            events.extend(core.parser.feed(line));
        }
        changed |= !events.is_empty();
        core.handle_events(events, replay);
        if !replay {
            break;
        }
    }
    changed
}

fn spawn_tailer(app: AppHandle, shared: Shared) {
    std::thread::spawn(move || loop {
        let changed = {
            let mut core = shared.lock().unwrap();
            let changed = tail_tick(&mut core);
            core.save(false);
            changed
        };
        if changed {
            notify(&app);
        }
        std::thread::sleep(POLL_INTERVAL);
    });
}

fn spawn_price_refresher(app: AppHandle, shared: Shared) {
    tauri::async_runtime::spawn(async move {
        loop {
            let (due, minutes) = {
                let core = shared.lock().unwrap();
                let minutes = core.settings.price_refresh_minutes.clamp(5, 240) as i64;
                (now_ms() - core.market.fetched_at >= minutes * 60_000, minutes)
            };
            if due {
                refresh_market(&app, &shared).await;
            }
            tokio::time::sleep(Duration::from_secs(60.min(minutes as u64 * 60))).await;
        }
    });
}

async fn refresh_market(app: &AppHandle, shared: &Shared) {
    {
        let mut core = shared.lock().unwrap();
        if core.price_status.refreshing {
            return;
        }
        core.price_status.refreshing = true;
    }
    notify(app);
    let result = prices::fetch_market(now_ms()).await;
    {
        let mut core = shared.lock().unwrap();
        core.price_status.refreshing = false;
        match result {
            Ok(table) => {
                core.market = table;
                core.market_dirty = true;
                core.price_status.error.clear();
            }
            Err(e) => core.price_status.error = e,
        }
    }
    notify(app);
}

// ---------------------------------------------------------------------------
// Commands

type Cmd<'a> = State<'a, Shared>;

#[tauri::command]
fn get_snapshot(state: Cmd) -> views::Snapshot {
    views::snapshot(&state.lock().unwrap(), now_ms())
}

#[tauri::command]
fn get_history(state: Cmd) -> Vec<views::RunView> {
    views::history(&state.lock().unwrap())
}

#[tauri::command]
fn get_price_list(state: Cmd) -> views::PriceList {
    views::price_list(&state.lock().unwrap())
}

#[tauri::command]
async fn import_log_file(app: AppHandle, state: Cmd<'_>) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog().file().set_title("Import UE_game.log or exported log ZIP").add_filter("Game log", &["log", "zip"]).pick_file(move |file| {
        let _ = tx.send(file);
    });
    let picked = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten()).await.map_err(|e| e.to_string())?;
    let Some(file) = picked else { return Ok(false) };
    // Android returns content:// URIs. The Tauri fs plugin can read those
    // handles, while std::fs cannot. Copy the result into app data so the
    // normal tailer can reopen it after the app is restarted.
    let selected = app.fs().read(file).map_err(|e| format!("cannot read log: {e}"))?;
    let bytes = if selected.starts_with(b"PK\x03\x04") {
        let mut archive = zip::ZipArchive::new(Cursor::new(selected)).map_err(|e| format!("cannot open log ZIP: {e}"))?;
        let index = (0..archive.len())
            .find(|&i| archive.by_index(i).map(|f| f.name().ends_with("UE_game.log") || f.name().ends_with(".log")).unwrap_or(false))
            .ok_or_else(|| "the ZIP does not contain a .log file".to_string())?;
        let mut entry = archive.by_index(index).map_err(|e| format!("cannot open log entry: {e}"))?;
        let mut extracted = Vec::new();
        entry.read_to_end(&mut extracted).map_err(|e| format!("cannot extract log: {e}"))?;
        extracted
    } else {
        selected
    };
    let mut core = state.lock().unwrap();
    let path = core.data_dir.join("imported-UE_game.log");
    fs::write(&path, &bytes).map_err(|e| format!("cannot store imported log: {e}"))?;
    core.parser = Parser::new();
    core.partial.clear();
    core.tail = TailPosition { path: path.display().to_string(), offset: bytes.len() as u64, signature: file_signature(&path).unwrap_or_default() };
    core.log.path = path.display().to_string();
    core.log.found = true;
    core.log.auto_detected = false;
    core.settings.log_path = Some(path.display().to_string());
    core.settings_dirty = true;
    let text = String::from_utf8_lossy(&bytes);
    let mut events = Vec::new();
    for line in text.lines() { events.extend(core.parser.feed(line)); }
    events.extend(core.parser.flush());
    core.handle_events(events, false);
    core.save(true);
    drop(core);
    notify(&app);
    Ok(true)
}

#[tauri::command]
async fn refresh_prices(app: AppHandle, state: Cmd<'_>) -> Result<(), String> {
    let shared = state.inner().clone();
    refresh_market(&app, &shared).await;
    Ok(())
}

fn valid_item_id(id: &str) -> Result<(), String> {
    if net::is_numeric_id(id, 12) { Ok(()) } else { Err("invalid item id".into()) }
}

#[tauri::command]
fn set_manual_price(app: AppHandle, state: Cmd, item_id: String, price: Option<f64>) -> Result<(), String> {
    valid_item_id(&item_id)?;
    let mut core = state.lock().unwrap();
    match price {
        Some(p) if p.is_finite() && (0.0..=MAX_PRICE).contains(&p) => {
            core.settings.manual_prices.insert(item_id, p);
        }
        Some(_) => return Err("price out of range".into()),
        None => {
            core.settings.manual_prices.remove(&item_id);
        }
    }
    core.settings_dirty = true;
    drop(core);
    notify(&app);
    Ok(())
}

#[tauri::command]
fn set_excluded(app: AppHandle, state: Cmd, item_id: String, excluded: bool) -> Result<(), String> {
    valid_item_id(&item_id)?;
    let mut core = state.lock().unwrap();
    if excluded {
        core.settings.excluded.insert(item_id);
    } else {
        core.settings.excluded.remove(&item_id);
    }
    core.settings_dirty = true;
    drop(core);
    notify(&app);
    Ok(())
}

#[tauri::command]
fn update_settings(app: AppHandle, state: Cmd, patch: SettingsPatch) -> Result<(), String> {
    let mut core = state.lock().unwrap();
    let s = &mut core.settings;
    if let Some(v) = patch.apply_tax {
        s.apply_tax = v;
    }
    if let Some(v) = patch.language.filter(|l| l.len() <= 10 && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')) {
        s.language = v;
    }
    if let Some(v) = patch.theme.filter(|t| ["system", "light", "dark"].contains(&t.as_str())) {
        s.theme = v;
    }
    if let Some(v) = patch.overlay_opacity.filter(|v| v.is_finite()) {
        s.overlay_opacity = v.clamp(0.3, 1.0);
    }
    if let Some(v) = patch.price_refresh_minutes {
        s.price_refresh_minutes = v.clamp(5, 240);
    }
    core.settings_dirty = true;
    drop(core);
    notify(&app);
    Ok(())
}

#[tauri::command]
async fn choose_log_file(app: AppHandle, state: Cmd<'_>) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Select UE_game.log")
        .add_filter("Game log", &["log"])
        .pick_file(move |file| {
            let _ = tx.send(file.and_then(|f| f.into_path().ok()));
        });
    let picked = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten()).await.map_err(|e| e.to_string())?;
    let Some(path) = picked else { return Ok(false) };
    let mut core = state.lock().unwrap();
    core.settings.log_path = Some(path.display().to_string());
    core.settings_dirty = true;
    drop(core);
    notify(&app);
    Ok(true)
}

#[tauri::command]
fn use_detected_log(app: AppHandle, state: Cmd) {
    let mut core = state.lock().unwrap();
    core.settings.log_path = None;
    core.settings_dirty = true;
    drop(core);
    notify(&app);
}

#[tauri::command]
fn new_session(app: AppHandle, state: Cmd) {
    state.lock().unwrap().with_tracker(|t, book| t.start_new_session(now_ms(), book));
    notify(&app);
}

#[tauri::command]
fn end_current_run(app: AppHandle, state: Cmd) {
    state.lock().unwrap().with_tracker(|t, book| t.finish_current(now_ms(), book));
    notify(&app);
}

#[tauri::command]
fn delete_run(app: AppHandle, state: Cmd, id: u64) {
    let mut core = state.lock().unwrap();
    core.tracker.history.retain(|r| r.id != id);
    core.tracker.dirty = true;
    drop(core);
    notify(&app);
}

#[tauri::command]
fn recalibrate_bag(app: AppHandle, state: Cmd) {
    state.lock().unwrap().tracker.invalidate_baseline();
    notify(&app);
}

#[tauri::command]
async fn get_strategy_stages() -> Result<(String, Vec<String>), String> {
    let season = strategy::current_season().await?;
    let stages = strategy::fetch_stages(&season).await?;
    Ok((season, stages))
}

#[tauri::command]
async fn get_strategy_board(season_id: String, stage: String) -> Result<strategy::Leaderboard, String> {
    strategy::fetch_leaderboard(&season_id, &stage).await
}

/// Async on purpose: building a webview window inside a sync command
/// deadlocks WebView2 on Windows.
#[tauri::command]
#[cfg(not(target_os = "android"))]
async fn toggle_overlay(app: AppHandle) -> Result<bool, String> {
    if let Some(win) = app.get_webview_window("overlay") {
        win.close().map_err(|e| e.to_string())?;
        return Ok(false);
    }
    // A `#fragment` in WebviewUrl::App gets percent-encoded, so route via a script instead.
    WebviewWindowBuilder::new(&app, "overlay", WebviewUrl::App("index.html".into()))
        .initialization_script("if (!location.hash.startsWith('#/overlay')) location.hash = '#/overlay';")
        .title("TLI Tracker overlay")
        .inner_size(340.0, 196.0)
        .min_inner_size(280.0, 150.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
#[cfg(target_os = "android")]
async fn toggle_overlay() -> Result<bool, String> {
    // The Android activity owns the native overlay service. It requests the
    // system overlay permission on first launch and starts/resumes the
    // service when the permission is granted.
    Ok(true)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let saved: SavedState = read_json(&data_dir.join("state.json"));
            let mut tracker = saved.tracker;
            if tracker.next_id == 0 {
                tracker = Tracker { history: tracker.history, ..Tracker::new(now_ms()) };
                tracker.next_id = tracker.history.iter().map(|r| r.id + 1).max().unwrap_or(1);
            }
            // We cannot know what happened while closed; resume on the next area change.
            if tracker.status == tracker::RunStatus::Active {
                let at = tracker.last_log_ts;
                tracker.pause(at);
            }
            let core = Core {
                tracker,
                settings: read_json(&data_dir.join("settings.json")),
                market: read_json(&data_dir.join("market.json")),
                log: LogStatus::default(),
                price_status: PriceStatus::default(),
                parser: Parser::new(),
                tail: saved.tail,
                partial: Vec::new(),
                last_growth: 0,
                data_dir,
                settings_dirty: false,
                market_dirty: false,
                last_save: now_ms(),
            };
            let shared: Shared = Arc::new(Mutex::new(core));
            app.manage(shared.clone());
            spawn_tailer(app.handle().clone(), shared.clone());
            spawn_price_refresher(app.handle().clone(), shared);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_history,
            get_price_list,
            import_log_file,
            refresh_prices,
            set_manual_price,
            set_excluded,
            update_settings,
            choose_log_file,
            use_detected_log,
            new_session,
            end_current_run,
            delete_run,
            recalibrate_bag,
            get_strategy_stages,
            get_strategy_board,
            toggle_overlay,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build app")
        .run(|app, event| {
            // Closing the main window quits, even if the overlay is still open.
            if let RunEvent::WindowEvent { label, event: tauri::WindowEvent::Destroyed, .. } = &event {
                if label == "main" {
                    app.exit(0);
                }
            }
            if let RunEvent::Exit = event {
                if let Some(shared) = app.try_state::<Shared>() {
                    shared.lock().unwrap().save(true);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays a whole synthetic session (scripts/demo_log.py) through parser and tracker.
    #[test]
    fn demo_log_end_to_end() {
        let mut parser = Parser::new();
        let mut tracker = Tracker::new(0);
        let market = MarketTable::default();
        let (local, manual, excluded) = (HashMap::new(), HashMap::new(), HashSet::new());
        let book = PriceBook { market: &market, local: &local, manual: &manual, excluded: &excluded, apply_tax: true };
        for line in include_str!("../tests/fixtures/demo.log").lines() {
            for event in parser.feed(line) {
                tracker.handle(event, &book);
            }
        }
        for event in parser.flush() {
            tracker.handle(event, &book);
        }

        assert_eq!(tracker.player.as_ref().map(|p| p.name.as_str()), Some("DemoHero"));
        assert!(tracker.baseline_trusted);
        // Three maps: two finished by the next map, the last one paused in town.
        assert_eq!(tracker.history.len(), 2);
        let all: Vec<&tracker::MapRun> = tracker.history.iter().chain(tracker.current.iter()).collect();
        assert_eq!(all.len(), 3);
        for run in all {
            let cost: Vec<(&str, i64)> = run.cost.iter().map(|d| (d.base_id.as_str(), d.qty)).collect();
            assert_eq!(cost, vec![("400015", 1), ("10250", 1)], "run {}", run.id);
            let fe: i64 = run.loot.iter().filter(|d| d.base_id == "100300").map(|d| d.qty).sum();
            assert!(fe > 0, "run {} looted no FE", run.id);
            assert!(run.loot.iter().all(|d| d.qty > 0));
        }
        assert_eq!(tracker.status, tracker::RunStatus::Paused);
    }
}
