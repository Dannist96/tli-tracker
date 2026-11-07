import { invoke } from '@tauri-apps/api/core';

export type RunStatus = 'active' | 'paused';
export type PriceSource = 'currency' | 'manual' | 'search' | 'market' | 'none';

export interface DropLine {
  baseId: string;
  qty: number;
  unit: number;
  value: number;
  priced: boolean;
  estimated: boolean;
  lastTs: number;
}

export interface RunView {
  id: number;
  areaId: string;
  mapId: string;
  mapType: string;
  region: number | null;
  timemark: number | null;
  playerId: string;
  start: number;
  end: number | null;
  durationMs: number;
  subAreas: number;
  lootValue: number;
  costValue: number;
  profit: number;
  perHour: number;
  loot: DropLine[];
  cost: DropLine[];
}

export interface SessionView {
  start: number;
  mapCount: number;
  mapTimeMs: number;
  wallTimeMs: number;
  lootValue: number;
  costValue: number;
  profit: number;
  perHourMap: number;
  perHourWall: number;
  runs: RunView[];
  loot: DropLine[];
  cost: DropLine[];
}

export interface Settings {
  logPath: string | null;
  applyTax: boolean;
  excluded: string[];
  manualPrices: Record<string, number>;
  language: string;
  theme: 'system' | 'light' | 'dark';
  overlayOpacity: number;
  priceRefreshMinutes: number;
}

export interface Snapshot {
  now: number;
  log: { path: string; found: boolean; autoDetected: boolean; error: string };
  trackingSeen: boolean;
  lastLogTs: number;
  player: { id: string; seasonId: string; name: string } | null;
  baselineTrusted: boolean;
  trustedPages: number[];
  status: RunStatus;
  current: RunView | null;
  session: SessionView;
  bag: { value: number; items: DropLine[] };
  prices: { seasonId: string; fetchedAt: number; count: number; refreshing: boolean; error: string };
  settings: Settings;
}

export interface ResolvedPrice {
  price: number;
  source: PriceSource;
  updatedAt: number;
}

export interface PriceList {
  prices: Record<string, ResolvedPrice>;
  excluded: string[];
  localSamples: Record<string, number>;
}

export interface StrategyRow {
  rank: number;
  name: string;
  score: number;
  maps: number;
  avgEarnings: number;
  avgEfficiency: number;
  avgDurationMs: number;
  region: number | null;
  timemark: number | null;
  talents: string;
  level: number;
  playerName: string;
}

export interface Leaderboard {
  seasonId: string;
  stage: string;
  generatedAt: number;
  rows: StrategyRow[];
}

export type SettingsPatch = Partial<Pick<Settings, 'applyTax' | 'language' | 'theme' | 'overlayOpacity' | 'priceRefreshMinutes'>>;

/** Typed wrappers around the Rust commands in `src-tauri/src/lib.rs`. */
export const api = {
  snapshot: () => invoke<Snapshot>('get_snapshot'),
  history: () => invoke<RunView[]>('get_history'),
  priceList: () => invoke<PriceList>('get_price_list'),
  refreshPrices: () => invoke<void>('refresh_prices'),
  setManualPrice: (itemId: string, price: number | null) => invoke<void>('set_manual_price', { itemId, price }),
  setExcluded: (itemId: string, excluded: boolean) => invoke<void>('set_excluded', { itemId, excluded }),
  updateSettings: (patch: SettingsPatch) => invoke<void>('update_settings', { patch }),
  chooseLogFile: () => invoke<boolean>('import_log_file'),
  importLogFile: () => invoke<boolean>('import_log_file'),
  useDetectedLog: () => invoke<void>('use_detected_log'),
  newSession: () => invoke<void>('new_session'),
  endCurrentRun: () => invoke<void>('end_current_run'),
  deleteRun: (id: number) => invoke<void>('delete_run', { id }),
  recalibrateBag: () => invoke<void>('recalibrate_bag'),
  strategyStages: () => invoke<[string, string[]]>('get_strategy_stages'),
  strategyBoard: (seasonId: string, stage: string) => invoke<Leaderboard>('get_strategy_board', { seasonId, stage }),
  toggleOverlay: () => invoke<boolean>('toggle_overlay'),
};
