import { Injectable, computed, effect, inject, signal } from '@angular/core';
import { api, RunView } from './api';
import { TrackerStore } from './tracker.store';

export type Range = 'today' | '7d' | '30d' | 'all';

const DAY_MS = 86_400_000;

/** Completed runs, reloaded only when the run count changes. */
@Injectable({ providedIn: 'root' })
export class HistoryStore {
  private readonly store = inject(TrackerStore);
  readonly runs = signal<RunView[]>([]);
  readonly error = signal('');
  readonly range = signal<Range>('7d');
  readonly onlyCurrentPlayer = signal(true);

  private readonly key = computed(() => {
    const s = this.store.snapshot();
    return s ? `${s.session.start}|${s.session.runs.length}|${s.session.runs[0]?.id ?? 0}` : '';
  });

  readonly filtered = computed(() => {
    const from = rangeStart(this.range());
    const player = this.onlyCurrentPlayer() ? this.store.snapshot()?.player?.id : undefined;
    return this.runs().filter((r) => r.start >= from && (!player || r.playerId === player));
  });

  constructor() {
    effect(() => {
      if (this.key()) void this.reload();
    });
  }

  async reload(): Promise<void> {
    try {
      this.runs.set(await api.history());
      this.error.set('');
    } catch (err) {
      this.error.set(String(err));
    }
  }
}

export function rangeStart(range: Range): number {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  switch (range) {
    case 'today':
      return today.getTime();
    case '7d':
      return today.getTime() - 6 * DAY_MS;
    case '30d':
      return today.getTime() - 29 * DAY_MS;
    default:
      return 0;
  }
}

export interface Totals {
  maps: number;
  timeMs: number;
  loot: number;
  cost: number;
  profit: number;
  perHour: number;
  avgMapMs: number;
}

export function totals(runs: RunView[]): Totals {
  const timeMs = runs.reduce((s, r) => s + r.durationMs, 0);
  const loot = runs.reduce((s, r) => s + r.lootValue, 0);
  const cost = runs.reduce((s, r) => s + r.costValue, 0);
  const profit = loot - cost;
  return {
    maps: runs.length,
    timeMs,
    loot,
    cost,
    profit,
    perHour: timeMs > 0 ? profit / (timeMs / 3_600_000) : 0,
    avgMapMs: runs.length ? timeMs / runs.length : 0,
  };
}
