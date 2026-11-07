import { Pipe, PipeTransform } from '@angular/core';

/** Flame Elementium amounts: 1.25, 98.4, 1 284, 12.3k, 1.42M. */
export function formatFe(value: number, signed = false): string {
  if (!Number.isFinite(value)) return '—';
  const sign = signed && value > 0 ? '+' : '';
  const abs = Math.abs(value);
  let text: string;
  if (abs >= 1_000_000) text = `${(value / 1_000_000).toFixed(2)}M`;
  else if (abs >= 100_000) text = `${(value / 1000).toFixed(0)}k`;
  else if (abs >= 10_000) text = `${(value / 1000).toFixed(1)}k`;
  else if (abs >= 100) text = Math.round(value).toLocaleString('en-US');
  else if (abs >= 10) text = value.toFixed(1);
  else if (abs === 0) text = '0';
  else text = value.toFixed(abs < 0.1 ? 3 : 2);
  return sign + text;
}

/** 1:02:03 or 12:34. */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(h ? 2 : 1, '0');
  return h ? `${h}:${mm}:${String(s).padStart(2, '0')}` : `${mm}:${String(s).padStart(2, '0')}`;
}

export function formatClock(ts: number): string {
  return ts ? new Date(ts).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }) : '—';
}

export function formatDate(ts: number): string {
  return ts ? new Date(ts).toLocaleDateString('en-GB', { day: '2-digit', month: 'short', year: 'numeric' }) : '—';
}

export function dayKey(ts: number): string {
  const d = new Date(ts);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

@Pipe({ name: 'fe' })
export class FePipe implements PipeTransform {
  transform(value: number | null | undefined, signed = false): string {
    return formatFe(value ?? 0, signed);
  }
}

@Pipe({ name: 'duration' })
export class DurationPipe implements PipeTransform {
  transform(ms: number | null | undefined): string {
    return formatDuration(ms ?? 0);
  }
}

@Pipe({ name: 'clock' })
export class ClockPipe implements PipeTransform {
  transform(ts: number | null | undefined): string {
    return formatClock(ts ?? 0);
  }
}

@Pipe({ name: 'day' })
export class DayPipe implements PipeTransform {
  transform(ts: number | null | undefined): string {
    return formatDate(ts ?? 0);
  }
}
