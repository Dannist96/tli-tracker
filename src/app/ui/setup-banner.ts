import { Component, computed, inject } from '@angular/core';
import { RouterLink } from '@angular/router';
import { TranslatePipe } from '../core/i18n';
import { TrackerStore } from '../core/tracker.store';

/** Explains what is missing before tracking can work. */
@Component({
  selector: 'app-setup-banner',
  imports: [TranslatePipe, RouterLink],
  template: `
    @switch (issue()) {
      @case ('noLog') {
        <div class="banner warn">
          <strong>{{ 'setup.noLog.title' | t }}</strong>
          <span>{{ 'setup.noLog.body' | t }}</span>
          <a routerLink="/settings">{{ 'setup.openSettings' | t }}</a>
        </div>
      }
      @case ('notEnabled') {
        <div class="banner warn">
          <strong>{{ 'setup.notEnabled.title' | t }}</strong>
          <ol>
            <li>{{ 'setup.notEnabled.step1' | t }}</li>
            <li>{{ 'setup.notEnabled.step2' | t }}</li>
            <li>{{ 'setup.notEnabled.step3' | t }}</li>
          </ol>
        </div>
      }
      @case ('calibrate') {
        <div class="banner info">
          <strong>{{ 'setup.calibrate.title' | t }}</strong>
          <span>{{ 'setup.calibrate.body' | t }}</span>
        </div>
      }
    }
  `,
  styles: `
    .banner { display: flex; flex-wrap: wrap; gap: 6px 12px; align-items: baseline; padding: 12px 16px; border-radius: var(--radius); margin-bottom: 12px; border: 1px solid; }
    .warn { background: var(--warn-soft); border-color: var(--warn-border); }
    .info { background: var(--info-soft); border-color: var(--info-border); }
    strong { font-weight: 600; }
    span { color: var(--text-dim); }
    ol { margin: 4px 0 0; padding-left: 20px; color: var(--text-dim); flex-basis: 100%; }
    a { color: var(--accent); }
  `,
})
export class SetupBanner {
  private readonly store = inject(TrackerStore);

  readonly issue = computed(() => {
    const s = this.store.snapshot();
    if (!s) return null;
    if (!s.log.found) return 'noLog';
    if (!s.trackingSeen) return 'notEnabled';
    if (!s.baselineTrusted) return 'calibrate';
    return null;
  });
}
