import { Component, computed, effect, inject, signal } from '@angular/core';
import { NavigationEnd, Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { filter } from 'rxjs';
import { api } from './core/api';
import { FePipe } from './core/format';
import { TranslatePipe } from './core/i18n';
import { TrackerStore } from './core/tracker.store';

interface NavItem {
  path: string;
  label: string;
  icon: string;
}

const ICONS: Record<string, string> = {
  overview: 'M3 13h8V3H3zm0 8h8v-6H3zm10 0h8V11h-8zm0-18v6h8V3z',
  bag: 'M6 7V6a6 6 0 0 1 12 0v1h2a1 1 0 0 1 1 1l-1 12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2L3 8a1 1 0 0 1 1-1zm2 0h8V6a4 4 0 0 0-8 0z',
  prices: 'M12 2 3 7v2h18V7zM5 11v7h2v-7zm4 0v7h2v-7zm4 0v7h2v-7zm4 0v7h2v-7zM3 20v2h18v-2z',
  history: 'M13 3a9 9 0 0 0-9 9H1l4 4 4-4H6a7 7 0 1 1 2.05 4.95l-1.42 1.42A9 9 0 1 0 13 3zm-1 5v5l4.25 2.52.77-1.28-3.52-2.09V8z',
  analytics: 'M4 20h16v2H2V4h2zm3-3h2v-6H7zm4 0h2V7h-2zm4 0h2v-9h-2zm4 0h2V4h-2z',
  strategies: 'M12 2l2.9 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l7.1-1.01z',
  settings: 'M19.4 13a7.6 7.6 0 0 0 0-2l2.1-1.6-2-3.5-2.5 1a7.4 7.4 0 0 0-1.7-1L15 3h-4l-.4 2.9a7.4 7.4 0 0 0-1.7 1l-2.5-1-2 3.5L6.6 11a7.6 7.6 0 0 0 0 2l-2.1 1.6 2 3.5 2.5-1a7.4 7.4 0 0 0 1.7 1L11 21h4l.4-2.9a7.4 7.4 0 0 0 1.7-1l2.5 1 2-3.5zM13 15.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7z',
  overlay: 'M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zm2 0v14h14V5zm7 2h6v5h-6z',
};

@Component({
  selector: 'app-root',
  imports: [RouterOutlet, RouterLink, RouterLinkActive, TranslatePipe, FePipe],
  template: `
    @if (isOverlay()) {
      <router-outlet />
    } @else {
      <header class="titlebar" data-tauri-drag-region>
        <div class="brand" data-tauri-drag-region>
          <img src="favicon.png" alt="" />
          <span data-tauri-drag-region>TLI Tracker</span>
        </div>
        <div class="status" data-tauri-drag-region>
          <span class="pill" [class]="statusTone()">{{ statusText() | t }}</span>
          @if (store.snapshot()?.current; as run) {
            <span class="pill neutral num">{{ run.profit | fe: true }} FE</span>
          }
        </div>
        <div class="win">
          <button type="button" (click)="minimize()" [attr.aria-label]="'common.minimize' | t">&#xE921;</button>
          <button type="button" (click)="toggleMaximize()" [attr.aria-label]="'common.maximize' | t">&#xE922;</button>
          <button type="button" class="close" (click)="close()" [attr.aria-label]="'common.close' | t">&#xE8BB;</button>
        </div>
      </header>
      <div class="body">
        <nav class="rail">
          @for (item of nav; track item.path) {
            <a [routerLink]="item.path" routerLinkActive="active">
              <svg viewBox="0 0 24 24" aria-hidden="true"><path [attr.d]="item.icon" /></svg>
              <span>{{ item.label | t }}</span>
            </a>
          }
          <span class="spacer"></span>
          <button type="button" class="rail-btn" [class.active]="overlayOpen()" (click)="toggleOverlay()">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path [attr.d]="overlayIcon" /></svg>
            <span>{{ 'nav.overlay' | t }}</span>
          </button>
        </nav>
        <main>
          <router-outlet />
        </main>
      </div>
    }
  `,
  styleUrl: './app.scss',
})
export class App {
  readonly store = inject(TrackerStore);
  private readonly router = inject(Router);
  readonly overlayIcon = ICONS['overlay'];
  readonly overlayOpen = signal(false);
  readonly route = signal('');
  readonly isOverlay = computed(() => this.route().startsWith('/overlay'));

  readonly nav: NavItem[] = ['overview', 'bag', 'prices', 'history', 'analytics', 'strategies', 'settings'].map(
    (p) => ({ path: `/${p}`, label: `nav.${p}`, icon: ICONS[p] }),
  );

  readonly statusText = computed(() => {
    const s = this.store.snapshot();
    if (!s) return 'status.starting';
    if (!s.log.found) return 'status.noLog';
    if (!s.trackingSeen) return 'status.waiting';
    if (s.current && s.status === 'active') return 'status.inMap';
    return 'status.idle';
  });

  readonly statusTone = computed(() => {
    const key = this.statusText();
    return 'pill ' + (key === 'status.inMap' ? 'good' : key === 'status.noLog' || key === 'status.waiting' ? 'warn' : 'neutral');
  });

  constructor() {
    this.router.events.pipe(filter((e) => e instanceof NavigationEnd)).subscribe((e) => {
      this.route.set((e as NavigationEnd).urlAfterRedirects);
    });
    effect(() => {
      document.documentElement.classList.toggle('overlay-mode', this.isOverlay());
    });
    effect(() => {
      const theme = this.store.snapshot()?.settings.theme ?? 'system';
      const dark = theme === 'dark' || (theme === 'system' && matchMedia('(prefers-color-scheme: dark)').matches);
      document.documentElement.dataset['theme'] = dark ? 'dark' : 'light';
    });
  }

  minimize(): void {
    void getCurrentWindow().minimize();
  }

  toggleMaximize(): void {
    void getCurrentWindow().toggleMaximize();
  }

  close(): void {
    void getCurrentWindow().close();
  }

  async toggleOverlay(): Promise<void> {
    this.overlayOpen.set(await api.toggleOverlay());
  }
}
