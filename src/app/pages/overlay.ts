import { Component, computed, inject } from '@angular/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { DurationPipe, FePipe } from '../core/format';
import { I18n, TranslatePipe } from '../core/i18n';
import { Items } from '../core/items';
import { TrackerStore } from '../core/tracker.store';
import { ItemIcon } from '../ui/item-icon';

const RECENT_DROPS = 4;

/** Compact always-on-top HUD shown over the game. */
@Component({
  selector: 'app-overlay',
  imports: [FePipe, DurationPipe, TranslatePipe, ItemIcon],
  template: `
    @if (store.snapshot(); as s) {
      <div class="hud" [style.--alpha]="s.settings.overlayOpacity" [class.live]="s.status === 'active'" data-tauri-drag-region>
        <header data-tauri-drag-region>
          @if (s.current; as run) {
            @if (timemark(); as tm) { <span class="badge" data-tauri-drag-region>{{ tm }}</span> }
            <span class="region" data-tauri-drag-region>{{ region() }}</span>
          } @else {
            <span class="region dim" data-tauri-drag-region>{{ 'overlay.waiting' | t }}</span>
          }
          <span class="state" data-tauri-drag-region>
            <i></i>{{ (s.status === 'active' ? 'overlay.live' : 'overlay.paused') | t }}
          </span>
          <button type="button" class="close" (click)="close()" [title]="'common.close' | t">✕</button>
        </header>

        <section class="main" data-tauri-drag-region>
          <div class="stat" data-tauri-drag-region>
            <small>{{ 'runs.time' | t }}</small>
            <b class="num">{{ store.currentDurationMs() | duration }}</b>
          </div>
          <div class="stat right" data-tauri-drag-region>
            <small>{{ 'overlay.mapProfit' | t }}</small>
            <b class="num" [class.pos]="runProfit() > 0" [class.neg]="runProfit() < 0">{{ runProfit() | fe: true }}</b>
            <em class="num">{{ runPerHour() | fe }}/h</em>
          </div>
        </section>

        @if (recent().length) {
          <ul class="drops" data-tauri-drag-region>
            @for (d of recent(); track d.baseId) {
              <li [title]="items.name(d.baseId)">
                <app-item-icon [id]="d.baseId" />
                <span class="num">×{{ d.qty }}</span>
                <span class="num fe">{{ d.value | fe }}</span>
              </li>
            }
          </ul>
        }

        <footer data-tauri-drag-region>
          <span><small>{{ 'overlay.session' | t }}</small><b class="num fe">{{ s.session.profit | fe: true }}</b></span>
          <span><small>{{ 'overlay.perHour' | t }}</small><b class="num">{{ sessionPerHour() | fe }}</b></span>
          <span><small>{{ 'overview.maps' | t }}</small><b class="num">{{ s.session.mapCount }}</b></span>
        </footer>
      </div>
    }
  `,
  styles: `
    :host { display: block; height: 100vh; padding: 6px; }
    .hud {
      --ember: #ff7a3d;
      position: relative;
      height: 100%;
      display: flex;
      flex-direction: column;
      gap: 8px;
      padding: 10px 12px 10px 14px;
      border-radius: 12px;
      background:
        radial-gradient(120% 90% at 0% 0%, rgb(255 122 61 / 10%), transparent 55%),
        rgb(14 13 16 / var(--alpha, .92));
      border: 1px solid rgb(255 255 255 / 9%);
      box-shadow: 0 6px 24px rgb(0 0 0 / 35%), inset 0 1px 0 rgb(255 255 255 / 5%);
      color: #f3f1ee;
      font-size: 12px;
      user-select: none;
      cursor: grab;
      overflow: hidden;
    }
    .hud::before {
      content: '';
      position: absolute;
      left: 0; top: 12px; bottom: 12px;
      width: 3px;
      border-radius: 0 3px 3px 0;
      background: #57525c;
      transition: background .3s;
    }
    .hud.live::before { background: var(--ember); box-shadow: 0 0 10px rgb(255 122 61 / 60%); }

    header { display: flex; align-items: center; gap: 7px; min-height: 18px; }
    .badge {
      font-size: 10.5px; font-weight: 700; letter-spacing: .03em;
      padding: 1px 6px; border-radius: 5px;
      color: #1a1210; background: var(--ember);
    }
    .region { flex: 1; min-width: 0; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: #e6e1db; }
    .dim { color: #8d8790; font-weight: 500; }
    .state { display: flex; align-items: center; gap: 5px; font-size: 10.5px; color: #8d8790; text-transform: uppercase; letter-spacing: .07em; }
    .state i { width: 6px; height: 6px; border-radius: 50%; background: #6b6670; }
    .live .state { color: #3ecf8e; }
    .live .state i { background: #3ecf8e; box-shadow: 0 0 0 3px rgb(62 207 142 / 20%); }
    .close {
      border: 0; background: rgb(255 255 255 / 6%); color: #a59f98;
      width: 18px; height: 18px; border-radius: 5px; font-size: 10px; line-height: 1;
      cursor: pointer; opacity: 0; transition: opacity .15s;
    }
    .hud:hover .close { opacity: 1; }
    .close:hover { background: #c42b1c; color: #fff; }

    .main { display: grid; grid-template-columns: 1fr 1fr; align-items: end; }
    .stat { display: flex; flex-direction: column; gap: 1px; }
    .stat.right { align-items: flex-end; }
    small { font-size: 9.5px; text-transform: uppercase; letter-spacing: .09em; color: #8d8790; }
    .stat b { font-size: 26px; font-weight: 700; line-height: 1.05; letter-spacing: -.02em; }
    .stat em { font-style: normal; font-size: 11px; color: #a59f98; }
    .pos { color: #4fe0a0; }
    .neg { color: #ff6b6b; }
    .fe { color: #f5c35b; }

    .drops {
      list-style: none; margin: 0; padding: 6px 0 0;
      display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 6px;
      border-top: 1px solid rgb(255 255 255 / 6%);
    }
    .drops li { display: flex; align-items: center; gap: 4px; min-width: 0; --icon-size: 20px; --icon-bg: rgb(255 255 255 / 5%); }
    .drops li span { font-size: 11px; white-space: nowrap; }
    .drops li .fe { margin-left: auto; font-size: 10.5px; opacity: .9; }

    footer {
      margin-top: auto;
      display: grid; grid-template-columns: repeat(3, auto); justify-content: space-between;
      padding-top: 7px; border-top: 1px solid rgb(255 255 255 / 6%);
    }
    footer span { display: flex; flex-direction: column; }
    footer span:nth-child(2) { align-items: center; }
    footer span:last-child { align-items: flex-end; }
    footer b { font-size: 13px; font-weight: 650; }
  `,
})
export class Overlay {
  readonly store = inject(TrackerStore);
  readonly items = inject(Items);
  private readonly i18n = inject(I18n);

  readonly timemark = computed(() => {
    const t = this.store.snapshot()?.current?.timemark;
    return t === null || t === undefined || t === 13 ? '' : this.i18n.t(`timemark.${t}`);
  });
  readonly region = computed(() => {
    const r = this.store.snapshot()?.current?.region;
    return r === null || r === undefined ? this.i18n.t('map.unknown') : this.i18n.t(`region.${r}`);
  });
  readonly runProfit = computed(() => this.store.snapshot()?.current?.profit ?? 0);
  readonly runPerHour = computed(() => {
    const ms = this.store.currentDurationMs();
    return ms > 0 ? this.runProfit() / (ms / 3_600_000) : 0;
  });
  readonly sessionPerHour = computed(() => {
    const s = this.store.snapshot();
    const ms = this.store.sessionMapTimeMs();
    return s && ms > 0 ? s.session.profit / (ms / 3_600_000) : 0;
  });
  readonly recent = computed(() =>
    [...(this.store.snapshot()?.current?.loot ?? [])].sort((a, b) => b.lastTs - a.lastTs).slice(0, RECENT_DROPS),
  );

  close(): void {
    void getCurrentWindow().close();
  }
}
