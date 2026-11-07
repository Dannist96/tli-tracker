import { Pipe, PipeTransform, inject } from '@angular/core';
import { I18n } from '../core/i18n';

/** "K7-0 · Blistering Lava Sea" from a run's region/timemark indices. */
@Pipe({ name: 'mapLabel', pure: false })
export class MapLabelPipe implements PipeTransform {
  private readonly i18n = inject(I18n);

  transform(run: { region: number | null; timemark: number | null; mapType?: string } | null | undefined): string {
    if (!run) return '';
    const parts: string[] = [];
    if (run.timemark !== null && run.timemark !== 13) parts.push(this.i18n.t(`timemark.${run.timemark}`));
    if (run.region !== null) parts.push(this.i18n.t(`region.${run.region}`));
    return parts.length ? parts.join(' · ') : this.i18n.t('map.unknown');
  }
}
