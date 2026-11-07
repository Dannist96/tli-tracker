import { Component, computed, inject, signal } from '@angular/core';
import { api } from '../core/api';
import { DurationPipe, FePipe } from '../core/format';
import { TranslatePipe } from '../core/i18n';
import { TrackerStore } from '../core/tracker.store';
import { DropList } from '../ui/drop-list';
import { MapLabelPipe } from '../ui/map-label.pipe';
import { RunTable } from '../ui/run-table';
import { SetupBanner } from '../ui/setup-banner';

type DropTab = 'loot' | 'cost' | 'unpriced';

@Component({
  selector: 'app-overview',
  imports: [FePipe, DurationPipe, TranslatePipe, MapLabelPipe, DropList, RunTable, SetupBanner],
  template: `
    @if (store.snapshot(); as s) {
      <header class="page-head">
        <div>
          <h1>{{ 'overview.title' | t }}</h1>
          <p class="sub">{{ s.player?.name || ('overview.noPlayer' | t) }}</p>
        </div>
        <div class="actions">
          <button type="button" class="ghost" (click)="newSession()">{{ 'overview.newSession' | t }}</button>
        </div>
      </header>

      <app-setup-banner />

      <section class="kpis">
        <div class="kpi accent">
          <span class="label">{{ 'overview.profit' | t }}</span>
          <span class="value num" [class.neg]="s.session.profit < 0">{{ s.session.profit | fe: true }}</span>
          <span class="hint">{{ (s.settings.applyTax ? 'overview.afterTax' : 'overview.noTax') | t }}</span>
        </div>
        <div class="kpi">
          <span class="label">{{ 'overview.perHourMap' | t }}</span>
          <span class="value num">{{ perHourMap() | fe }}</span>
          <span class="hint">{{ 'overview.perHourWall' | t }}: <b class="num">{{ perHourWall() | fe }}</b></span>
        </div>
        <div class="kpi">
          <span class="label">{{ 'overview.maps' | t }}</span>
          <span class="value num">{{ s.session.mapCount }}</span>
          <span class="hint">{{ 'overview.avgMap' | t }}: <b class="num">{{ avgMapMs() | duration }}</b></span>
        </div>
        <div class="kpi">
          <span class="label">{{ 'overview.mapTime' | t }}</span>
          <span class="value num">{{ store.sessionMapTimeMs() | duration }}</span>
          <span class="hint">{{ 'overview.sessionTime' | t }}: <b class="num">{{ store.sessionWallMs() | duration }}</b></span>
        </div>
        <div class="kpi">
          <span class="label">{{ 'overview.bag' | t }}</span>
          <span class="value num fe">{{ s.bag.value | fe }}</span>
          <span class="hint">{{ (s.baselineTrusted ? 'overview.bagCalibrated' : 'overview.bagPartial') | t }}</span>
        </div>
      </section>

      <section class="card current" [class.live]="s.status === 'active'">
        @if (s.current; as run) {
          <div class="cur-main">
            <span class="dot"></span>
            <div>
              <span class="label">{{ (s.status === 'active' ? 'current.inMap' : 'current.paused') | t }}</span>
              <strong>{{ run | mapLabel }}</strong>
            </div>
          </div>
          <div class="cur-stat"><span class="label">{{ 'runs.time' | t }}</span><span class="num big">{{ store.currentDurationMs() | duration }}</span></div>
          <div class="cur-stat"><span class="label">{{ 'runs.loot' | t }}</span><span class="num big fe">{{ run.lootValue | fe }}</span></div>
          <div class="cur-stat"><span class="label">{{ 'runs.cost' | t }}</span><span class="num big dim">{{ run.costValue | fe }}</span></div>
          <div class="cur-stat"><span class="label">{{ 'runs.profit' | t }}</span><span class="num big" [class.pos]="run.profit > 0" [class.neg]="run.profit < 0">{{ run.profit | fe: true }}</span></div>
          <button type="button" class="ghost" (click)="endRun()">{{ 'current.end' | t }}</button>
        } @else {
          <p class="idle">{{ 'current.none' | t }}</p>
        }
      </section>

      <div class="grid">
        <section class="card">
          <h2>{{ 'overview.mapLog' | t }} <span class="count">{{ s.session.runs.length }}</span></h2>
          <app-run-table [runs]="s.session.runs" />
        </section>
        <section class="card">
          <div class="tabs">
            @for (tab of tabs; track tab) {
              <button type="button" [class.active]="dropTab() === tab" (click)="dropTab.set(tab)">
                {{ 'drops.tab.' + tab | t }}
              </button>
            }
          </div>
          @switch (dropTab()) {
            @case ('loot') { <app-drop-list [lines]="s.session.loot" sign="+" /> }
            @case ('cost') { <app-drop-list [lines]="s.session.cost" sign="-" emptyText="drops.noCost" /> }
            @case ('unpriced') { <app-drop-list [lines]="unpriced()" sign="+" emptyText="drops.allPriced" /> }
          }
        </section>
      </div>
    }
  `,
  styles: `
    .kpis { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 12px; margin-bottom: 12px; }
    .kpi { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 14px 16px; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
    .kpi.accent { background: linear-gradient(135deg, var(--accent-soft), var(--surface) 70%); border-color: var(--accent-border); }
    .kpi .value { font-size: 26px; font-weight: 650; letter-spacing: -.02em; }
    .kpi .hint { font-size: 12px; color: var(--text-faint); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .kpi .hint b { color: var(--text-dim); font-weight: 600; }
    .label { font-size: 11px; color: var(--text-faint); text-transform: uppercase; letter-spacing: .07em; }
    .fe { color: var(--fe); }
    .neg { color: var(--neg); }
    .pos { color: var(--pos); }
    .dim { color: var(--text-dim); }
    .current { display: flex; align-items: center; gap: 28px; margin-bottom: 12px; padding: 14px 18px; }
    .cur-main { display: flex; align-items: center; gap: 12px; flex: 1; min-width: 0; }
    .cur-main div { display: flex; flex-direction: column; min-width: 0; }
    .cur-main strong { font-size: 16px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .dot { width: 10px; height: 10px; border-radius: 50%; background: var(--text-faint); flex: none; }
    .current.live .dot { background: var(--pos); box-shadow: 0 0 0 4px color-mix(in srgb, var(--pos) 25%, transparent); animation: pulse 1.6s infinite; }
    .cur-stat { display: flex; flex-direction: column; align-items: flex-end; }
    .big { font-size: 18px; font-weight: 600; }
    .idle { margin: 0; color: var(--text-faint); }
    .grid { display: grid; grid-template-columns: minmax(0, 1.6fr) minmax(300px, 1fr); gap: 12px; align-items: start; }
    h2 { font-size: 13px; font-weight: 600; margin: 0 0 10px; display: flex; gap: 8px; align-items: center; }
    .count { color: var(--text-faint); font-weight: 500; }
    @keyframes pulse { 50% { box-shadow: 0 0 0 7px transparent; } }
    @media (max-width: 1100px) { .kpis { grid-template-columns: repeat(3, 1fr); } .grid { grid-template-columns: 1fr; } }
  `,
})
export class Overview {
  readonly store = inject(TrackerStore);
  readonly tabs: DropTab[] = ['loot', 'cost', 'unpriced'];
  readonly dropTab = signal<DropTab>('loot');

  readonly perHourMap = computed(() => {
    const s = this.store.snapshot();
    const ms = this.store.sessionMapTimeMs();
    return s && ms > 0 ? s.session.profit / (ms / 3_600_000) : 0;
  });
  readonly perHourWall = computed(() => {
    const s = this.store.snapshot();
    const ms = this.store.sessionWallMs();
    return s && ms > 0 ? s.session.profit / (ms / 3_600_000) : 0;
  });
  readonly avgMapMs = computed(() => {
    const s = this.store.snapshot();
    return s && s.session.mapCount ? this.store.sessionMapTimeMs() / s.session.mapCount : 0;
  });
  readonly unpriced = computed(() => this.store.snapshot()?.session.loot.filter((l) => !l.priced) ?? []);

  newSession(): void {
    void api.newSession();
  }

  endRun(): void {
    void api.endCurrentRun();
  }
}
