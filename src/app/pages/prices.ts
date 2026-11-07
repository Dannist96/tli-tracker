import { Component, computed, effect, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { api, PriceList, ResolvedPrice } from '../core/api';
import { DayPipe, ClockPipe, FePipe } from '../core/format';
import { TranslatePipe } from '../core/i18n';
import { CATEGORIES, Items } from '../core/items';
import { TrackerStore } from '../core/tracker.store';
import { ItemIcon } from '../ui/item-icon';

const PAGE_SIZE = 60;

interface Row {
  id: string;
  name: string;
  category: string;
  price: ResolvedPrice | null;
  excluded: boolean;
}

@Component({
  selector: 'app-prices',
  imports: [FormsModule, FePipe, DayPipe, ClockPipe, TranslatePipe, ItemIcon],
  template: `
    @if (store.snapshot(); as s) {
      <header class="page-head">
        <div>
          <h1>{{ 'prices.title' | t }}</h1>
          <p class="sub">
            @if (s.prices.fetchedAt) {
              {{ 'prices.updated' | t: { n: s.prices.count } }} {{ s.prices.fetchedAt | day }} {{ s.prices.fetchedAt | clock }}
            } @else {
              {{ 'prices.never' | t }}
            }
            @if (s.prices.error) { <span class="err">· {{ s.prices.error }}</span> }
          </p>
        </div>
        <div class="actions">
          <button type="button" class="primary" [disabled]="s.prices.refreshing" (click)="refresh()">
            {{ (s.prices.refreshing ? 'prices.refreshing' : 'prices.refresh') | t }}
          </button>
        </div>
      </header>

      <section class="card toolbar">
        <input type="search" [(ngModel)]="queryValue" (ngModelChange)="query.set($event); page.set(0)" [placeholder]="'common.search' | t" />
        <select [ngModel]="cat()" (ngModelChange)="cat.set($event); page.set(0)">
          <option value="">{{ 'category.all' | t }}</option>
          @for (c of categories; track c) { <option [value]="c">{{ 'category.' + c | t }}</option> }
        </select>
        <select [ngModel]="filter()" (ngModelChange)="filter.set($event); page.set(0)">
          <option value="priced">{{ 'prices.filter.priced' | t }}</option>
          <option value="all">{{ 'prices.filter.all' | t }}</option>
          <option value="manual">{{ 'prices.filter.manual' | t }}</option>
          <option value="excluded">{{ 'prices.filter.excluded' | t }}</option>
        </select>
        <span class="count">{{ rows().length }}</span>
      </section>

      <section class="card table">
        <div class="row head">
          <span></span><span>{{ 'prices.item' | t }}</span><span>{{ 'prices.category' | t }}</span>
          <span class="r">{{ 'prices.price' | t }}</span><span>{{ 'prices.source' | t }}</span>
          <span>{{ 'prices.manual' | t }}</span><span class="c">{{ 'prices.counts' | t }}</span>
        </div>
        @for (row of pageRows(); track row.id) {
          <div class="row" [class.excluded]="row.excluded">
            <app-item-icon [id]="row.id" />
            <span class="name">{{ row.name }}</span>
            <span class="dim">{{ 'category.' + row.category | t }}</span>
            <span class="r num fe">{{ row.price ? (row.price.price | fe) : '—' }}</span>
            <span class="dim small">
              {{ 'prices.src.' + (row.price?.source ?? 'none') | t }}
              @if (row.price?.updatedAt) { · {{ row.price!.updatedAt | day }} }
            </span>
            <input class="manual" type="number" min="0" step="any" [placeholder]="'prices.auto' | t"
              [value]="manual()[row.id] ?? ''" (change)="setManual(row.id, $any($event.target).value)" />
            <input class="c" type="checkbox" [checked]="!row.excluded" (change)="setExcluded(row.id, !$any($event.target).checked)"
              [title]="'prices.countsHint' | t" />
          </div>
        } @empty {
          <p class="empty">{{ 'prices.empty' | t }}</p>
        }
        @if (pageCount() > 1) {
          <nav class="pager">
            <button type="button" class="ghost" [disabled]="page() === 0" (click)="page.set(page() - 1)">‹</button>
            <span class="num">{{ page() + 1 }} / {{ pageCount() }}</span>
            <button type="button" class="ghost" [disabled]="page() + 1 >= pageCount()" (click)="page.set(page() + 1)">›</button>
          </nav>
        }
      </section>
    }
  `,
  styles: `
    .toolbar { display: flex; gap: 10px; align-items: center; margin-bottom: 12px; padding: 10px 12px; }
    .toolbar input { flex: 1; }
    .count { color: var(--text-faint); font-variant-numeric: tabular-nums; }
    .row { display: grid; grid-template-columns: 28px minmax(0, 2fr) minmax(0, 1fr) 90px minmax(0, 1.2fr) 110px 56px; gap: 10px; align-items: center; padding: 5px 8px; border-radius: 6px; }
    .row:not(.head):hover { background: var(--hover); }
    .head { font-size: 11px; text-transform: uppercase; letter-spacing: .06em; color: var(--text-faint); }
    .name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .excluded .name, .excluded .fe { opacity: .45; }
    .r { text-align: right; }
    .c { justify-self: center; }
    .fe { color: var(--fe); }
    .dim { color: var(--text-dim); }
    .small { font-size: 12px; }
    .manual { width: 100%; padding: 4px 8px; }
    .err { color: var(--neg); }
    .pager { display: flex; justify-content: center; align-items: center; gap: 12px; padding-top: 10px; }
    .empty { color: var(--text-faint); text-align: center; padding: 24px; margin: 0; }
  `,
})
export class Prices {
  readonly store = inject(TrackerStore);
  private readonly items = inject(Items);
  readonly categories = CATEGORIES;
  readonly list = signal<PriceList | null>(null);
  readonly query = signal('');
  queryValue = '';
  readonly cat = signal('');
  readonly filter = signal<'priced' | 'all' | 'manual' | 'excluded'>('priced');
  readonly page = signal(0);
  readonly manual = computed(() => this.store.snapshot()?.settings.manualPrices ?? {});

  constructor() {
    effect(() => {
      this.store.revision();
      void api.priceList().then((l) => this.list.set(l));
    });
  }

  readonly rows = computed<Row[]>(() => {
    const list = this.list();
    if (!list) return [];
    const excluded = new Set(list.excluded);
    const ids = new Set([...this.items.allIds(), ...Object.keys(list.prices)]);
    const q = this.query().trim().toLowerCase();
    const cat = this.cat();
    const filter = this.filter();
    const rows: Row[] = [];
    for (const id of ids) {
      const price = list.prices[id] ?? null;
      const row: Row = { id, name: this.items.name(id), category: this.items.category(id), price, excluded: excluded.has(id) };
      if (cat && row.category !== cat) continue;
      if (q && !row.name.toLowerCase().includes(q) && id !== q) continue;
      if (filter === 'priced' && !(price && price.source !== 'none')) continue;
      if (filter === 'manual' && price?.source !== 'manual') continue;
      if (filter === 'excluded' && !row.excluded) continue;
      rows.push(row);
    }
    return rows.sort((a, b) => (b.price?.price ?? 0) - (a.price?.price ?? 0) || a.name.localeCompare(b.name));
  });

  readonly pageCount = computed(() => Math.ceil(this.rows().length / PAGE_SIZE));
  readonly pageRows = computed(() => this.rows().slice(this.page() * PAGE_SIZE, (this.page() + 1) * PAGE_SIZE));

  refresh(): void {
    void api.refreshPrices();
  }

  setManual(id: string, raw: string): void {
    const value = raw.trim() === '' ? null : Number(raw);
    if (value !== null && (!Number.isFinite(value) || value < 0)) return;
    void api.setManualPrice(id, value);
  }

  setExcluded(id: string, excluded: boolean): void {
    void api.setExcluded(id, excluded);
  }
}
