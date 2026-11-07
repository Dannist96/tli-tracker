import { Injectable, inject } from '@angular/core';
import ITEMS from '../../data/items.json';
import CN_NAMES from '../../data/cn_names.json';
import { I18n } from './i18n';

interface ItemRecord {
  n: string;
  c: string;
  i: string;
}

const items = ITEMS as Record<string, ItemRecord>;
const cnNames = CN_NAMES as Record<string, string>;
/** Longest names first so that "X (Artifact)" wins over "X". */
const cnByLength = Object.keys(cnNames).sort((a, b) => b.length - a.length);

export const CATEGORIES = [
  'currency', 'compass', 'beacon', 'memory', 'legendary', 'skill', 'destiny',
  'tartarus', 'divinity', 'prism', 'organ', 'other',
] as const;

@Injectable({ providedIn: 'root' })
export class Items {
  private readonly i18n = inject(I18n);

  name(id: string): string {
    return items[id]?.n ?? this.i18n.t('items.unknown', { id });
  }

  category(id: string): string {
    return items[id]?.c ?? 'other';
  }

  icon(id: string): string | null {
    const file = items[id]?.i;
    return file ? `icons/${file}` : null;
  }

  known(id: string): boolean {
    return id in items;
  }

  allIds(): string[] {
    return Object.keys(items);
  }

  /**
   * Translates a server-generated Chinese strategy name: item names via the
   * bundled item list, other words via the locale's `terms` table. Anything
   * unknown stays as-is. The result is always rendered as plain text.
   */
  translateStrategy(text: string): string {
    if (!/[㐀-鿿]/.test(text)) return text;
    const terms = this.i18n.table('terms');
    let out = text;
    for (const cn of cnByLength) {
      if (out.includes(cn)) out = out.split(cn).join(this.name(cnNames[cn]));
    }
    const termKeys = Object.keys(terms).sort((a, b) => b.length - a.length);
    for (const cn of termKeys) {
      if (out.includes(cn)) out = out.split(cn).join(terms[cn]);
    }
    return out
      .replace(/等(\d+)个/g, (_, n: string) => this.i18n.t('strategies.moreMechanics', { n }))
      .replace(/等(\d+)种/g, (_, n: string) => this.i18n.t('strategies.moreItems', { n }))
      .replace(/、/g, ', ')
      .replace(/（/g, ' (')
      .replace(/）/g, ')')
      .replace(/\s{2,}/g, ' ')
      .trim();
  }
}
