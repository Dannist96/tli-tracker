import { Injectable, Pipe, PipeTransform, inject, signal } from '@angular/core';

/**
 * UI translations live in `public/locales/<code>.json`. To add a language,
 * drop a new file there and list it in `public/locales/languages.json`.
 * Keys missing from a language fall back to English.
 */
export interface LanguageInfo {
  code: string;
  name: string;
}

type Dict = Record<string, unknown>;

const FALLBACK = 'en';
const LANG_CODE = /^[a-z]{2}(-[A-Za-z]{2,4})?$/;

@Injectable({ providedIn: 'root' })
export class I18n {
  readonly lang = signal(FALLBACK);
  readonly languages = signal<LanguageInfo[]>([{ code: FALLBACK, name: 'English' }]);
  private readonly dict = signal<Dict>({});
  private fallback: Dict = {};

  async init(lang: string): Promise<void> {
    this.fallback = await this.load(FALLBACK);
    try {
      const list = (await (await fetch('locales/languages.json')).json()) as LanguageInfo[];
      this.languages.set(list.filter((l) => LANG_CODE.test(l.code)));
    } catch {
      // Keep the built-in English entry.
    }
    await this.use(lang);
  }

  async use(lang: string): Promise<void> {
    const code = LANG_CODE.test(lang) ? lang : FALLBACK;
    this.dict.set(code === FALLBACK ? this.fallback : await this.load(code));
    this.lang.set(code);
    document.documentElement.lang = code;
  }

  /** Looks up a dotted key; `{name}` placeholders are filled from `params`. */
  t(key: string, params?: Record<string, string | number>): string {
    const raw = lookup(this.dict(), key) ?? lookup(this.fallback, key);
    const text = typeof raw === 'string' ? raw : key;
    return params ? text.replace(/\{(\w+)\}/g, (m, name: string) => (name in params ? String(params[name]) : m)) : text;
  }

  /** Raw sub-tree, e.g. the strategy term dictionary. */
  table(key: string): Record<string, string> {
    const merged = { ...(lookup(this.fallback, key) as object), ...(lookup(this.dict(), key) as object) };
    return Object.fromEntries(Object.entries(merged).filter(([, v]) => typeof v === 'string')) as Record<string, string>;
  }

  private async load(code: string): Promise<Dict> {
    try {
      return (await (await fetch(`locales/${code}.json`)).json()) as Dict;
    } catch {
      return {};
    }
  }
}

function lookup(dict: Dict, key: string): unknown {
  let cur: unknown = dict;
  for (const part of key.split('.')) {
    if (cur === null || typeof cur !== 'object') return undefined;
    cur = (cur as Dict)[part];
  }
  return cur;
}

@Pipe({ name: 't', pure: false })
export class TranslatePipe implements PipeTransform {
  private readonly i18n = inject(I18n);

  transform(key: string, params?: Record<string, string | number>): string {
    return this.i18n.t(key, params);
  }
}
