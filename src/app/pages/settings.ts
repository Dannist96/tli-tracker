import { Component, inject } from '@angular/core';
import { api, SettingsPatch } from '../core/api';
import { I18n, TranslatePipe } from '../core/i18n';
import { TrackerStore } from '../core/tracker.store';

@Component({
  selector: 'app-settings',
  imports: [TranslatePipe],
  template: `
    @if (store.snapshot(); as s) {
      <header class="page-head">
        <div>
          <h1>{{ 'settings.title' | t }}</h1>
        </div>
      </header>

      <section class="card group">
        <h2>{{ 'settings.log.title' | t }}</h2>
        <div class="field">
          <span class="path" [class.missing]="!s.log.found">{{ s.log.path || ('settings.log.notFound' | t) }}</span>
          <span class="tag">{{ (s.log.autoDetected ? 'settings.log.auto' : 'settings.log.manual') | t }}</span>
        </div>
        <div class="buttons">
          <button type="button" class="ghost" (click)="choose()">{{ 'settings.log.choose' | t }}</button>
          @if (!s.log.autoDetected) {
            <button type="button" class="ghost" (click)="useDetected()">{{ 'settings.log.useAuto' | t }}</button>
          }
        </div>
        <p class="help">{{ 'settings.log.help' | t }}</p>
      </section>

      <section class="card group">
        <h2>{{ 'settings.tracking.title' | t }}</h2>
        <label class="toggle">
          <input type="checkbox" [checked]="s.settings.applyTax" (change)="patch({ applyTax: $any($event.target).checked })" />
          <span>
            {{ 'settings.tracking.tax' | t }}
            <small>{{ 'settings.tracking.taxHelp' | t }}</small>
          </span>
        </label>
        <label class="line">
          {{ 'settings.tracking.refresh' | t }}
          <select [value]="s.settings.priceRefreshMinutes" (change)="patch({ priceRefreshMinutes: +$any($event.target).value })">
            @for (m of refreshOptions; track m) { <option [value]="m" [selected]="m === s.settings.priceRefreshMinutes">{{ 'settings.tracking.minutes' | t: { n: m } }}</option> }
          </select>
        </label>
      </section>

      <section class="card group overlay-setting">
        <h2>{{ 'settings.appearance.title' | t }}</h2>
        <label class="line">
          {{ 'settings.appearance.theme' | t }}
          <select (change)="patch({ theme: $any($event.target).value })">
            @for (th of themes; track th) { <option [value]="th" [selected]="th === s.settings.theme">{{ 'settings.appearance.' + th | t }}</option> }
          </select>
        </label>
        <label class="line">
          {{ 'settings.appearance.language' | t }}
          <select (change)="setLanguage($any($event.target).value)">
            @for (l of i18n.languages(); track l.code) { <option [value]="l.code" [selected]="l.code === s.settings.language">{{ l.name }}</option> }
          </select>
        </label>
        <label class="line">
          {{ 'settings.appearance.overlayOpacity' | t }}
          <input type="range" min="0.3" max="1" step="0.05" [value]="s.settings.overlayOpacity" (change)="patch({ overlayOpacity: +$any($event.target).value })" />
        </label>
      </section>

      <section class="card group">
        <h2>{{ 'settings.privacy.title' | t }}</h2>
        <p class="help">{{ 'settings.privacy.body' | t }}</p>
      </section>
    }
  `,
  styles: `
    .group { margin-bottom: 12px; max-width: 760px; display: flex; flex-direction: column; gap: 12px; }
    h2 { font-size: 13px; font-weight: 600; margin: 0; }
    .field { display: flex; gap: 10px; align-items: center; }
    .path { font-family: var(--mono); font-size: 12px; color: var(--text-dim); word-break: break-all; }
    .path.missing { color: var(--neg); }
    .tag { font-size: 11px; padding: 2px 8px; border-radius: 99px; background: var(--hover); color: var(--text-faint); white-space: nowrap; }
    .buttons { display: flex; gap: 8px; }
    .help { margin: 0; color: var(--text-faint); font-size: 12px; line-height: 1.5; }
    .toggle { display: flex; gap: 10px; align-items: flex-start; cursor: pointer; }
    .toggle span { display: flex; flex-direction: column; }
    .toggle small { color: var(--text-faint); font-size: 12px; }
    .line { display: flex; justify-content: space-between; align-items: center; gap: 16px; color: var(--text-dim); }
    .line select, .line input { width: 220px; }
  `,
})
export class SettingsPage {
  readonly store = inject(TrackerStore);
  readonly i18n = inject(I18n);
  readonly themes = ['system', 'dark', 'light'] as const;
  readonly refreshOptions = [5, 10, 15, 30, 60, 120];

  patch(p: SettingsPatch): void {
    void api.updateSettings(p);
  }

  setLanguage(code: string): void {
    void this.i18n.use(code);
    this.patch({ language: code });
  }

  choose(): void {
    void api.importLogFile();
  }

  useDetected(): void {
    void api.useDetectedLog();
  }
}
