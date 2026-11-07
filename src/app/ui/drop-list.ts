import { Component, computed, inject, input } from '@angular/core';
import { DropLine } from '../core/api';
import { FePipe } from '../core/format';
import { TranslatePipe } from '../core/i18n';
import { Items } from '../core/items';
import { ItemIcon } from './item-icon';

/** Compact list of items with quantity and value. */
@Component({
  selector: 'app-drop-list',
  imports: [ItemIcon, FePipe, TranslatePipe],
  template: `
    @for (line of visible(); track line.baseId) {
      <div class="row" [class.unpriced]="!line.priced">
        <app-item-icon [id]="line.baseId" />
        <span class="name" [title]="items.name(line.baseId)">{{ items.name(line.baseId) }}</span>
        <span class="qty num">{{ sign() }}{{ line.qty }}@if (line.estimated) {<sup title="{{ 'drops.estimated' | t }}">*</sup>}</span>
        <span class="value num">{{ line.priced ? (line.value | fe) : '—' }}</span>
      </div>
    } @empty {
      <p class="empty">{{ emptyText() | t }}</p>
    }
    @if (hidden() > 0) {
      <p class="more">{{ 'drops.more' | t: { n: hidden() } }}</p>
    }
  `,
  styles: `
    :host { display: block; }
    .row {
      display: grid;
      grid-template-columns: 28px minmax(0, 1fr) auto 72px;
      align-items: center;
      gap: 10px;
      padding: 5px 4px;
      border-radius: 6px;
    }
    .row:hover { background: var(--hover); }
    .name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .qty { color: var(--text-dim); }
    .value { text-align: right; color: var(--fe); }
    .unpriced .value { color: var(--text-faint); }
    .empty, .more { color: var(--text-faint); font-size: 12px; padding: 8px 4px; margin: 0; }
  `,
})
export class DropList {
  readonly items = inject(Items);
  readonly lines = input.required<DropLine[]>();
  readonly limit = input(200);
  readonly sign = input('');
  readonly emptyText = input('drops.empty');
  readonly visible = computed(() => this.lines().slice(0, this.limit()));
  readonly hidden = computed(() => Math.max(0, this.lines().length - this.limit()));
}
