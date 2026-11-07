import { DestroyRef, Injectable, computed, inject, signal } from '@angular/core';
import { listen } from '@tauri-apps/api/event';
import { api, Snapshot } from './api';

const REFETCH_DEBOUNCE_MS = 150;
const FALLBACK_POLL_MS = 5_000;

/** Live tracker state pushed from Rust, plus a 1s clock for running timers. */
@Injectable({ providedIn: 'root' })
export class TrackerStore {
  readonly snapshot = signal<Snapshot | null>(null);
  /** Wall clock, ticking every second. */
  readonly now = signal(Date.now());
  /** Bumped on every backend change so pages can refetch their own data. */
  readonly revision = signal(0);

  /** Current run duration including the time since the last snapshot. */
  readonly currentDurationMs = computed(() => {
    const s = this.snapshot();
    if (!s?.current) return 0;
    const drift = s.status === 'active' ? Math.max(0, this.now() - s.now) : 0;
    return s.current.durationMs + drift;
  });

  readonly sessionMapTimeMs = computed(() => {
    const s = this.snapshot();
    if (!s) return 0;
    const live = s.current ? this.currentDurationMs() - s.current.durationMs : 0;
    return s.session.mapTimeMs + live;
  });

  /** Real time since the session's first map, ticking between snapshots. */
  readonly sessionWallMs = computed(() => {
    const s = this.snapshot();
    if (!s || (!s.session.wallTimeMs && !s.current)) return 0;
    return s.session.wallTimeMs + Math.max(0, this.now() - s.now);
  });

  private pending: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    const destroyRef = inject(DestroyRef);
    const clock = setInterval(() => this.now.set(Date.now()), 1000);
    const poll = setInterval(() => this.refresh(), FALLBACK_POLL_MS);
    const unlisten = listen('tracker-changed', () => this.scheduleRefresh());
    destroyRef.onDestroy(() => {
      clearInterval(clock);
      clearInterval(poll);
      void unlisten.then((off) => off());
    });
    void this.refresh();
  }

  scheduleRefresh(): void {
    if (this.pending) return;
    this.pending = setTimeout(() => {
      this.pending = null;
      void this.refresh();
    }, REFETCH_DEBOUNCE_MS);
  }

  async refresh(): Promise<void> {
    try {
      this.snapshot.set(await api.snapshot());
      this.revision.update((r) => r + 1);
    } catch (err) {
      console.error('snapshot failed', err);
    }
  }
}
