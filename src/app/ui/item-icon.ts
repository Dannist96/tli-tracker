import { Component, computed, inject, input } from '@angular/core';
import { Items } from '../core/items';

@Component({
  selector: 'app-item-icon',
  template: `
    @if (src(); as s) {
      <img [src]="s" alt="" loading="lazy" decoding="async" />
    } @else {
      <span class="fallback" [attr.data-cat]="category()">{{ initial() }}</span>
    }
  `,
  styles: `
    :host {
      display: inline-grid;
      place-items: center;
      width: var(--icon-size, 28px);
      height: var(--icon-size, 28px);
      border-radius: 6px;
      background: var(--icon-bg);
      flex: none;
      overflow: hidden;
    }
    img { width: 100%; height: 100%; object-fit: contain; }
    .fallback { font-size: 11px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; }
  `,
})
export class ItemIcon {
  private readonly items = inject(Items);
  readonly id = input.required<string>();
  readonly src = computed(() => this.items.icon(this.id()));
  readonly category = computed(() => this.items.category(this.id()));
  readonly initial = computed(() => this.items.name(this.id()).slice(0, 1));
}
