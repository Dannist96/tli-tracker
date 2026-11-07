import { Component, computed, inject } from '@angular/core';
import { api } from '../core/api';
import { DurationPipe, FePipe } from '../core/format';
import { HistoryStore, totals } from '../core/history';
import { TranslatePipe } from '../core/i18n';
import { RangePicker } from '../ui/range-picker';
import { RunTable } from '../ui/run-table';

@Component({
  selector: 'app-history',
  imports: [FePipe, DurationPipe, TranslatePipe, RunTable, RangePicker],
  template: `
    <header class="page-head">
      <div>
        <h1>{{ 'history.title' | t }}</h1>
        <p class="sub">{{ 'history.subtitle' | t }}</p>
      </div>
      <app-range-picker />
    </header>

    <section class="kpis">
      <div class="kpi"><span class="label">{{ 'runs.profit' | t }}</span><span class="value num" [class.neg]="sum().profit < 0">{{ sum().profit | fe: true }}</span></div>
      <div class="kpi"><span class="label">{{ 'overview.perHourMap' | t }}</span><span class="value num">{{ sum().perHour | fe }}</span></div>
      <div class="kpi"><span class="label">{{ 'overview.maps' | t }}</span><span class="value num">{{ sum().maps }}</span></div>
      <div class="kpi"><span class="label">{{ 'overview.mapTime' | t }}</span><span class="value num">{{ sum().timeMs | duration }}</span></div>
      <div class="kpi"><span class="label">{{ 'overview.avgMap' | t }}</span><span class="value num">{{ sum().avgMapMs | duration }}</span></div>
    </section>

    @if (history.error()) { <p class="card err">{{ history.error() }}</p> }
    <section class="card">
      <app-run-table [runs]="history.filtered()" [showDate]="true" [deletable]="true" (remove)="remove($event)" />
    </section>
  `,
  styles: `
    .kpis { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 12px; margin-bottom: 12px; }
    .kpi { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 12px 16px; display: flex; flex-direction: column; gap: 4px; }
    .kpi .value { font-size: 22px; font-weight: 650; }
    .label { font-size: 11px; color: var(--text-faint); text-transform: uppercase; letter-spacing: .07em; }
    .neg { color: var(--neg); }
    .err { color: var(--neg); }
  `,
})
export class History {
  readonly history = inject(HistoryStore);
  readonly sum = computed(() => totals(this.history.filtered()));

  constructor() {
    void this.history.reload();
  }

  remove(id: number): void {
    void api.deleteRun(id).then(() => this.history.reload());
  }
}
