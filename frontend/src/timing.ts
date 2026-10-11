import type { Kind, Log, Session, Tag } from './types.ts';

// Ported from Nichijou FocusWidget: absolute start timestamps, not tick accumulation.
export function elapsed(session: Session, now: number): number {
  const start = Date.parse(session.startTime);
  return Number.isFinite(start) ? Math.max(0, Math.floor((now - start) / 1000)) : 0;
}
export function clock(seconds: number): string {
  const value = Math.max(0, Math.floor(seconds));
  return [Math.floor(value / 3600), Math.floor((value % 3600) / 60), value % 60].map(n => String(n).padStart(2, '0')).join(':');
}
export function duration(seconds: number): string {
  const value = Math.max(0, Math.floor(seconds));
  const h = Math.floor(value / 3600), m = Math.floor((value % 3600) / 60);
  return h > 0 ? `${h} 小时 ${m} 分` : `${m} 分`;
}
export function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}
export function kindOf(tags: Tag[], name: string): Kind {
  const kind = tags.find(tag => tag.name === name)?.kind;
  return kind === 'positive' || kind === 'negative' || kind === 'leisure' ? kind : 'neutral';
}
export const goalTarget = 5 * 3600;
export function totals(tags: Tag[], logs: Log[], sessions: Session[], now: number): Record<Kind, number> {
  const result: Record<Kind, number> = { positive: 0, neutral: 0, negative: 0, leisure: 0 };
  const today = new Date(now);
  for (const log of logs) {
    if (sameDay(new Date(log.startTime), today)) result[kindOf(tags, log.tag)] += Math.max(0, log.duration);
  }
  // Match the website: parallel tags accumulate independently, running sessions include their full elapsed time.
  for (const session of sessions) result[kindOf(tags, session.tag)] += elapsed(session, now);
  return result;
}
