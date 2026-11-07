import { Component, inject } from '@angular/core';
import { TranslatePipe } from '../core/i18n';
import { HistoryStore, Range } from '../core/history';

@Component({
  selector: 'app-range-picker',
  imports: [TranslatePipe],
  template: `
    <div class="seg">
      @for (r of ranges; track r) {
        <button type="button" [class.active]="history.range() === r" (click)="history.range.set(r)">{{ 'range.' + r | t }}</button>
      }
    </div>
    <label class="check">
      <input type="checkbox" [checked]="history.onlyCurrentPlayer()" (change)="history.onlyCurrentPlayer.set($any($event.target).checked)" />
      {{ 'range.currentCharacter' | t }}
    </label>
  `,
  styles: `
    :host { display: flex; gap: 14px; align-items: center; }
    .check { display: flex; gap: 6px; align-items: center; color: var(--text-dim); font-size: 13px; cursor: pointer; }
  `,
})
export class RangePicker {
  readonly history = inject(HistoryStore);
  readonly ranges: Range[] = ['today', '7d', '30d', 'all'];
}
