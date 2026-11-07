import { Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { api, Leaderboard, StrategyRow } from '../core/api';
import { DurationPipe, FePipe } from '../core/format';
import { I18n, TranslatePipe } from '../core/i18n';
import { Items } from '../core/items';
import { MapLabelPipe } from '../ui/map-label.pipe';

/**
 * Public strategy leaderboard. The server data is reduced to plain numbers
 * and short strings in Rust and rendered here only as text.
 */
@Component({
  selector: 'app-strategies',
  imports: [FormsModule, FePipe, DurationPipe, TranslatePipe, MapLabelPipe],
  template: `
    <header class="page-head">
      <div>
        <h1>{{ 'strategies.title' | t }}</h1>
        <p class="sub">{{ 'strategies.subtitle' | t }}</p>
      </div>
      <div class="actions">
        <select [ngModel]="stage()" (ngModelChange)="load($event)" [disabled]="!stages().length">
          @for (s of stages(); track s) { <option [value]="s">{{ stageLabel(s) }}</option> }
        </select>
        <button type="button" class="ghost" [disabled]="loading()" (click)="init()">{{ 'common.reload' | t }}</button>
      </div>
    </header>

    <section class="card toolbar">
      <input type="search" [(ngModel)]="queryValue" (ngModelChange)="query.set($event)" [placeholder]="'common.search' | t" />
      @if (board(); as b) { <span class="dim small">{{ 'strategies.count' | t: { n: b.rows.length } }}</span> }
    </section>

    @if (error()) {
      <p class="card err">{{ 'strategies.error' | t }}: {{ error() }}</p>
    } @else if (loading()) {
      <p class="card dim">{{ 'common.loading' | t }}</p>
    } @else {
      <section class="card">
        <div class="row head">
          <span>#</span><span>{{ 'strategies.strategy' | t }}</span><span>{{ 'runs.map' | t }}</span>
          <span class="r">{{ 'strategies.score' | t }}</span><span class="r">{{ 'overview.maps' | t }}</span>
          <span class="r">{{ 'strategies.perMap' | t }}</span><span class="r">{{ 'runs.perHour' | t }}</span>
          <span class="r">{{ 'overview.avgMap' | t }}</span>
        </div>
        @for (row of rows(); track row.rank + row.name) {
          <div class="row">
            <span class="dim num">{{ row.rank }}</span>
            <span class="name">
              <strong [title]="items.translateStrategy(row.name)">{{ items.translateStrategy(row.name) }}</strong>
              <small>{{ talents(row) }}@if (row.playerName) { · {{ row.playerName }} · {{ 'strategies.level' | t: { n: row.level } }} }</small>
            </span>
            <span class="dim">{{ row | mapLabel }}</span>
            <span class="r num">{{ row.score.toFixed(1) }}</span>
            <span class="r num">{{ row.maps }}</span>
            <span class="r num fe">{{ row.avgEarnings | fe }}</span>
            <span class="r num strong">{{ row.avgEfficiency * 60 | fe }}</span>
            <span class="r num dim">{{ row.avgDurationMs | duration }}</span>
          </div>
        } @empty {
          <p class="empty">{{ 'strategies.empty' | t }}</p>
        }
      </section>
    }
  `,
  styles: `
    .toolbar { display: flex; gap: 10px; align-items: center; margin-bottom: 12px; padding: 10px 12px; }
    .toolbar input { flex: 1; }
    .row { display: grid; grid-template-columns: 32px minmax(0, 3fr) minmax(0, 1.2fr) repeat(5, minmax(60px, .7fr)); gap: 10px; align-items: center; padding: 7px 8px; border-radius: 6px; }
    .row:not(.head):hover { background: var(--hover); }
    .head { font-size: 11px; text-transform: uppercase; letter-spacing: .06em; color: var(--text-faint); }
    .name { display: flex; flex-direction: column; min-width: 0; }
    .name strong { font-weight: 550; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
    .name small { color: var(--text-faint); font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .r { text-align: right; }
    .fe { color: var(--fe); }
    .strong { font-weight: 650; }
    .dim { color: var(--text-dim); }
    .small { font-size: 12px; }
    .err { color: var(--neg); }
    .empty { color: var(--text-faint); text-align: center; padding: 24px; margin: 0; }
  `,
})
export class Strategies {
  readonly items = inject(Items);
  private readonly i18n = inject(I18n);
  readonly stages = signal<string[]>([]);
  readonly stage = signal('');
  readonly board = signal<Leaderboard | null>(null);
  readonly loading = signal(false);
  readonly error = signal('');
  readonly query = signal('');
  queryValue = '';
  private seasonId = '';

  readonly rows = computed<StrategyRow[]>(() => {
    const q = this.query().trim().toLowerCase();
    const rows = this.board()?.rows ?? [];
    return q ? rows.filter((r) => this.items.translateStrategy(r.name).toLowerCase().includes(q) || r.name.includes(q)) : rows;
  });

  constructor() {
    void this.init();
  }

  async init(): Promise<void> {
    this.loading.set(true);
    this.error.set('');
    try {
      const [season, stages] = await api.strategyStages();
      this.seasonId = season;
      this.stages.set(stages);
      const stage = stages.includes(this.stage()) ? this.stage() : (stages[0] ?? '');
      if (stage) await this.load(stage);
    } catch (err) {
      this.error.set(String(err));
    } finally {
      this.loading.set(false);
    }
  }

  async load(stage: string): Promise<void> {
    this.stage.set(stage);
    this.loading.set(true);
    this.error.set('');
    try {
      this.board.set(await api.strategyBoard(this.seasonId, stage));
    } catch (err) {
      this.error.set(String(err));
    } finally {
      this.loading.set(false);
    }
  }

  stageLabel(stage: string): string {
    const m = /^(day|week)(\d+)$/.exec(stage);
    if (m) return this.i18n.t(`strategies.stage.${m[1]}`, { n: m[2] });
    return this.i18n.t(`strategies.stage.${stage}`);
  }

  talents(row: StrategyRow): string {
    return row.talents ? this.items.translateStrategy(row.talents) : this.i18n.t('strategies.noTalents');
  }
}
