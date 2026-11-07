import { Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { api } from '../core/api';
import { FePipe } from '../core/format';
import { TranslatePipe } from '../core/i18n';
import { CATEGORIES, Items } from '../core/items';
import { TrackerStore } from '../core/tracker.store';
import { DropList } from '../ui/drop-list';

@Component({
  selector: 'app-bag',
  imports: [FormsModule, FePipe, TranslatePipe, DropList],
  template: `
    @if (store.snapshot(); as s) {
      <header class="page-head">
        <div>
          <h1>{{ 'bag.title' | t }}</h1>
          <p class="sub">{{ 'bag.subtitle' | t }}</p>
        </div>
        <div class="actions">
          <span class="total">{{ 'bag.total' | t }} <b class="num fe">{{ total() | fe }}</b></span>
          <button type="button" class="ghost" (click)="recalibrate()">{{ 'bag.recalibrate' | t }}</button>
        </div>
      </header>

      <div class="layout">
        <aside class="card filters">
          <input type="search" [(ngModel)]="queryValue" (ngModelChange)="query.set($event)" [placeholder]="'common.search' | t" />
          <button type="button" class="cat" [class.active]="cat() === ''" (click)="cat.set('')">
            {{ 'category.all' | t }} <span>{{ s.bag.items.length }}</span>
          </button>
          @for (c of categoriesPresent(); track c.key) {
            <button type="button" class="cat" [class.active]="cat() === c.key" (click)="cat.set(c.key)">
              {{ 'category.' + c.key | t }} <span>{{ c.count }}</span>
            </button>
          }
          <p class="pages">
            {{ 'bag.pages' | t }}
            @for (p of pageIds; track p) {
              <span class="page" [class.ok]="s.trustedPages.includes(p) || s.baselineTrusted">{{ 'bag.page.' + p | t }}</span>
            }
          </p>
        </aside>
        <section class="card">
          <app-drop-list [lines]="filtered()" emptyText="bag.empty" />
        </section>
      </div>
    }
  `,
  styles: `
    .layout { display: grid; grid-template-columns: 230px minmax(0, 1fr); gap: 12px; align-items: start; }
    .filters { display: flex; flex-direction: column; gap: 2px; }
    .filters input { margin-bottom: 8px; }
    .cat { display: flex; justify-content: space-between; text-align: left; border: 0; background: none; color: var(--text-dim); padding: 7px 10px; border-radius: 6px; font: inherit; cursor: pointer; }
    .cat span { color: var(--text-faint); font-variant-numeric: tabular-nums; }
    .cat:hover { background: var(--hover); }
    .cat.active { background: var(--accent-soft); color: var(--text); }
    .total { color: var(--text-dim); }
    .fe { color: var(--fe); font-size: 18px; }
    .pages { margin: 12px 0 0; font-size: 12px; color: var(--text-faint); display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
    .page { padding: 2px 8px; border-radius: 99px; border: 1px solid var(--border); }
    .page.ok { border-color: color-mix(in srgb, var(--pos) 50%, transparent); color: var(--pos); }
  `,
})
export class Bag {
  readonly store = inject(TrackerStore);
  private readonly items = inject(Items);
  readonly pageIds = [100, 101, 102, 103];
  readonly query = signal('');
  queryValue = '';
  readonly cat = signal('');

  readonly categoriesPresent = computed(() => {
    const lines = this.store.snapshot()?.bag.items ?? [];
    const counts = new Map<string, number>();
    for (const l of lines) counts.set(this.items.category(l.baseId), (counts.get(this.items.category(l.baseId)) ?? 0) + 1);
    return CATEGORIES.filter((c) => counts.has(c)).map((key) => ({ key, count: counts.get(key)! }));
  });

  readonly filtered = computed(() => {
    const q = this.query().trim().toLowerCase();
    const cat = this.cat();
    return (this.store.snapshot()?.bag.items ?? []).filter(
      (l) => (!cat || this.items.category(l.baseId) === cat) && (!q || this.items.name(l.baseId).toLowerCase().includes(q)),
    );
  });

  readonly total = computed(() => this.filtered().reduce((sum, l) => sum + l.value, 0));

  recalibrate(): void {
    void api.recalibrateBag();
  }
}
