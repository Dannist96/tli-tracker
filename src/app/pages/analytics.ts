import { Component, computed, inject, signal } from '@angular/core';
import { DropLine, RunView } from '../core/api';
import { dayKey, DurationPipe, FePipe } from '../core/format';
import { HistoryStore, totals, Totals } from '../core/history';
import { I18n, TranslatePipe } from '../core/i18n';
import { DropList } from '../ui/drop-list';
import { MapLabelPipe } from '../ui/map-label.pipe';
import { RangePicker } from '../ui/range-picker';

type SortKey = 'perHour' | 'profit' | 'maps' | 'avgProfit' | 'avgMapMs';

interface Group extends Totals {
  key: string;
  sample: RunView;
  avgProfit: number;
  avgCost: number;
}

/** Strategy analysis built only from the user's own runs. */
@Component({
  selector: 'app-analytics',
  imports: [FePipe, DurationPipe, TranslatePipe, MapLabelPipe, DropList, RangePicker],
  template: `
    <header class="page-head">
      <div>
        <h1>{{ 'analytics.title' | t }}</h1>
        <p class="sub">{{ 'analytics.subtitle' | t }}</p>
      </div>
      <app-range-picker />
    </header>

    <section class="card">
      <h2>{{ 'analytics.daily' | t }}</h2>
      <div class="bars">
        @for (d of days(); track d.key) {
          <div class="bar" [title]="d.key + ': ' + (d.profit | fe) + ' · ' + d.maps + ' ' + ('analytics.mapsShort' | t)">
            <span class="fill" [class.neg]="d.profit < 0" [style.height.%]="d.height"></span>
            <small>{{ d.label }}</small>
          </div>
        } @empty {
          <p class="empty">{{ 'runs.empty' | t }}</p>
        }
      </div>
    </section>

    <section class="card">
      <h2>{{ 'analytics.byMap' | t }}</h2>
      <div class="row head">
        <span>{{ 'runs.map' | t }}</span>
        @for (col of columns; track col.key) {
          <button type="button" class="r sort" [class.active]="sort() === col.key" (click)="sort.set(col.key)">{{ col.label | t }}</button>
        }
        <span class="r">{{ 'analytics.avgCost' | t }}</span>
      </div>
      @for (g of groups(); track g.key) {
        <div class="row">
          <span class="map">{{ g.sample | mapLabel }}</span>
          <span class="r num">{{ g.maps }}</span>
          <span class="r num">{{ g.avgMapMs | duration }}</span>
          <span class="r num" [class.neg]="g.avgProfit < 0">{{ g.avgProfit | fe }}</span>
          <span class="r num strong">{{ g.perHour | fe }}</span>
          <span class="r num" [class.neg]="g.profit < 0">{{ g.profit | fe }}</span>
          <span class="r num dim">{{ g.avgCost | fe }}</span>
        </div>
      } @empty {
        <p class="empty">{{ 'runs.empty' | t }}</p>
      }
    </section>

    <div class="two">
      <section class="card">
        <h2>{{ 'analytics.topLoot' | t }}</h2>
        <app-drop-list [lines]="topLoot()" [limit]="25" sign="+" />
      </section>
      <section class="card">
        <h2>{{ 'analytics.costs' | t }}</h2>
        <app-drop-list [lines]="costs()" [limit]="25" sign="-" emptyText="drops.noCost" />
      </section>
    </div>
  `,
  styles: `
    h2 { font-size: 13px; font-weight: 600; margin: 0 0 12px; }
    .card { margin-bottom: 12px; }
    .bars { display: flex; align-items: flex-end; gap: 4px; height: 140px; }
    .bar { flex: 1; min-width: 6px; height: 100%; display: flex; flex-direction: column; justify-content: flex-end; align-items: center; gap: 4px; }
    .fill { width: 100%; max-width: 40px; background: linear-gradient(to top, var(--accent), color-mix(in srgb, var(--accent) 55%, var(--fe))); border-radius: 4px 4px 0 0; min-height: 2px; }
    .fill.neg { background: var(--neg); }
    .bar small { font-size: 10px; color: var(--text-faint); white-space: nowrap; }
    .row { display: grid; grid-template-columns: minmax(160px, 1.8fr) repeat(6, minmax(70px, 1fr)); gap: 8px; align-items: center; padding: 7px 8px; border-radius: 6px; }
    .row:not(.head):hover { background: var(--hover); }
    .head { font-size: 11px; text-transform: uppercase; letter-spacing: .06em; color: var(--text-faint); }
    .sort { border: 0; background: none; color: inherit; font: inherit; text-transform: inherit; letter-spacing: inherit; cursor: pointer; padding: 0; }
    .sort.active { color: var(--accent); }
    .r { text-align: right; }
    .strong { font-weight: 650; }
    .dim { color: var(--text-dim); }
    .neg { color: var(--neg); }
    .map { font-weight: 550; }
    .two { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
    .empty { color: var(--text-faint); text-align: center; padding: 20px; margin: 0; width: 100%; }
  `,
})
export class Analytics {
  private readonly history = inject(HistoryStore);
  private readonly i18n = inject(I18n);
  readonly sort = signal<SortKey>('perHour');
  readonly columns: { key: SortKey; label: string }[] = [
    { key: 'maps', label: 'overview.maps' },
    { key: 'avgMapMs', label: 'overview.avgMap' },
    { key: 'avgProfit', label: 'analytics.avgProfit' },
    { key: 'perHour', label: 'runs.perHour' },
    { key: 'profit', label: 'analytics.total' },
  ];

  readonly groups = computed<Group[]>(() => {
    const byKey = new Map<string, RunView[]>();
    for (const r of this.history.filtered()) {
      const key = `${r.timemark ?? '?'}|${r.region ?? '?'}`;
      byKey.set(key, [...(byKey.get(key) ?? []), r]);
    }
    const sortKey = this.sort();
    return [...byKey.entries()]
      .map(([key, runs]) => {
        const t = totals(runs);
        return { ...t, key, sample: runs[0], avgProfit: t.profit / t.maps, avgCost: t.cost / t.maps };
      })
      .sort((a, b) => b[sortKey] - a[sortKey]);
  });

  readonly days = computed(() => {
    const byDay = new Map<string, { profit: number; maps: number }>();
    for (const r of this.history.filtered()) {
      const k = dayKey(r.start);
      const d = byDay.get(k) ?? { profit: 0, maps: 0 };
      d.profit += r.profit;
      d.maps += 1;
      byDay.set(k, d);
    }
    const max = Math.max(1, ...[...byDay.values()].map((d) => Math.abs(d.profit)));
    return [...byDay.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, d]) => ({ key, ...d, label: key.slice(5), height: (Math.abs(d.profit) / max) * 100 }));
  });

  readonly topLoot = computed(() => merge(this.history.filtered().flatMap((r) => r.loot)));
  readonly costs = computed(() => merge(this.history.filtered().flatMap((r) => r.cost)));

  constructor() {
    void this.history.reload();
  }
}

function merge(lines: DropLine[]): DropLine[] {
  const byId = new Map<string, DropLine>();
  for (const l of lines) {
    const cur = byId.get(l.baseId);
    byId.set(l.baseId, cur ? { ...cur, qty: cur.qty + l.qty, value: cur.value + l.value, lastTs: Math.max(cur.lastTs, l.lastTs) } : { ...l });
  }
  return [...byId.values()].sort((a, b) => b.value - a.value);
}
