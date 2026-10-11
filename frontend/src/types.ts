export type Kind = 'positive' | 'neutral' | 'negative' | 'leisure';
export interface Tag { id: string; name: string; kind: Kind; sortOrder: number; archivedAt: string | null }
export interface Session { id: string; tag: string; startTime: string }
export interface Log { id: string; tag: string; startTime: string; endTime: string; duration: number }
export interface Snapshot { tags: Tag[]; sessions: Session[]; logs: Log[]; syncedAt: string }
export interface Geometry { gap: number; top: number; collapsedWidth: number; expandedWidth: number; expandedHeight: number; hasNotch: boolean }
export interface WeekStats { weekStart: string; weekEnd: string; totalDuration: number; totalCount: number; dailyStats: { date: string; totalDuration: number; count: number }[]; tagStats: { tag: string; duration: number; count: number }[] }
export interface Backend {
  GetGeometry(): Promise<Geometry>;
  SetExpanded(expanded: boolean): Promise<void>;
  ReadState(): Promise<Snapshot>;
  SetTagRunning(tag: string, running: boolean): Promise<Snapshot>;
  StopAll(): Promise<Snapshot>;
  CreateTag(name: string, kind: Kind): Promise<Snapshot>;
  ReadHistory(start: string, end: string): Promise<Log[]>;
  ReadWeek(): Promise<WeekStats>;
  Quit(): Promise<void>;
}
declare global {
  interface Window {
    go?: { main: { App: Backend } };
    runtime?: { EventsOn(name: string, callback: (...args: unknown[]) => void): () => void };
  }
}
