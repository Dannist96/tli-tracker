import { Component, input, output, signal } from '@angular/core';
import { RunView } from '../core/api';
import { ClockPipe, DayPipe, DurationPipe, FePipe } from '../core/format';
import { TranslatePipe } from '../core/i18n';
import { DropList } from './drop-list';
import { MapLabelPipe } from './map-label.pipe';

/** Map runs with expandable loot/cost details. */
@Component({
  selector: 'app-run-table',
  imports: [FePipe, DurationPipe, ClockPipe, DayPipe, TranslatePipe, MapLabelPipe, DropList],
  template: `
    <div class="head row">
      <span>{{ 'runs.map' | t }}</span>
      <span class="r">{{ 'runs.time' | t }}</span>
      <span class="r">{{ 'runs.loot' | t }}</span>
      <span class="r">{{ 'runs.cost' | t }}</span>
      <span class="r">{{ 'runs.profit' | t }}</span>
      <span class="r">{{ 'runs.perHour' | t }}</span>
    </div>
    @for (run of runs(); track run.id) {
      <button type="button" class="row run" [class.open]="openId() === run.id" (click)="toggle(run.id)">
        <span class="map">
          <strong>{{ run | mapLabel }}</strong>
          <small>{{ showDate() ? (run.start | day) + ' ' : '' }}{{ run.start | clock }}@if (run.subAreas) { · +{{ run.subAreas }} }</small>
        </span>
        <span class="r num">{{ run.durationMs | duration }}</span>
        <span class="r num fe">{{ run.lootValue | fe }}</span>
        <span class="r num dim">{{ run.costValue ? '-' + (run.costValue | fe) : '—' }}</span>
        <span class="r num" [class.pos]="run.profit > 0" [class.neg]="run.profit < 0">{{ run.profit | fe: true }}</span>
        <span class="r num dim">{{ run.perHour | fe }}</span>
      </button>
      @if (openId() === run.id) {
        <div class="details">
          <section>
            <h4>{{ 'runs.loot' | t }}</h4>
            <app-drop-list [lines]="run.loot" sign="+" />
          </section>
          <section>
            <h4>{{ 'runs.cost' | t }}</h4>
            <app-drop-list [lines]="run.cost" sign="-" emptyText="drops.noCost" />
            @if (deletable()) {
              <button type="button" class="ghost danger" (click)="remove.emit(run.id)">{{ 'runs.delete' | t }}</button>
            }
          </section>
        </div>
      }
    } @empty {
      <p class="empty">{{ 'runs.empty' | t }}</p>
    }
  `,
  styles: `
    :host { display: block; }
    .row {
      display: grid;
      grid-template-columns: minmax(150px, 1.6fr) repeat(5, minmax(64px, 1fr));
      gap: 8px;
      align-items: center;
      padding: 8px 10px;
      width: 100%;
      text-align: left;
    }
    .head { color: var(--text-faint); font-size: 11px; text-transform: uppercase; letter-spacing: .06em; padding-top: 0; }
    .run { border: 0; background: none; color: inherit; font: inherit; border-radius: 8px; cursor: pointer; }
    .run:hover, .run.open { background: var(--hover); }
    .map { display: flex; flex-direction: column; min-width: 0; }
    .map strong { font-weight: 550; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .map small { color: var(--text-faint); font-size: 11px; }
    .r { text-align: right; }
    .fe { color: var(--fe); }
    .dim { color: var(--text-dim); }
    .pos { color: var(--pos); }
    .neg { color: var(--neg); }
    .details { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; padding: 4px 10px 14px; }
    h4 { margin: 6px 0; font-size: 11px; text-transform: uppercase; letter-spacing: .06em; color: var(--text-faint); font-weight: 600; }
    .empty { color: var(--text-faint); padding: 24px 10px; margin: 0; text-align: center; }
    .danger { margin-top: 8px; }
  `,
})
export class RunTable {
  readonly runs = input.required<RunView[]>();
  readonly showDate = input(false);
  readonly deletable = input(false);
  readonly remove = output<number>();
  readonly openId = signal<number | null>(null);

  toggle(id: number): void {
    this.openId.update((cur) => (cur === id ? null : id));
  }
}
