import type { Backend, Geometry, Kind, Log, Session, Snapshot, Tag, WeekStats } from './types';

export const fallbackGeometry: Geometry = { gap: 0, top: 22, collapsedWidth: 210, expandedWidth: 432, expandedHeight: 480, hasNotch: true };

// Local fixture server for browser QA only. Production requires the Wails native bridge.
function localBackend(): Backend {
  async function request<T>(path: string, body?: object): Promise<T> {
    const response = await fetch(`/dev-api${path}`, body ? { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) } : undefined);
    if (!response.ok) throw new Error('连接中断，请检查网络后重试');
    return response.json() as Promise<T>;
  }
  const sessions = () => request<Session[]>('/focus/current?userId=dog');
  const state = async (): Promise<Snapshot> => {
    const [tags, active, logs] = await Promise.all([request<Tag[]>('/focus/tags?userId=dog&includeArchived=true'), sessions(), request<Log[]>('/focus/logs?userId=dog')]);
    return { tags, sessions: active, logs, syncedAt: new Date().toISOString() };
  };
  return {
    GetGeometry: async () => fallbackGeometry, SetExpanded: async () => {}, ReadState: state,
    async SetTagRunning(tag: string, running: boolean) {
      await request(running ? '/focus/start' : '/focus/stop', { userId: 'dog', tag });
      return state();
    },
    async StopAll() { await request('/focus/stop', { userId: 'dog' }); return state(); },
    async CreateTag(name: string, kind: Kind) { await request('/focus/tags', { userId: 'dog', name, kind }); return state(); },
    ReadHistory: (start, end) => request<Log[]>(`/focus/logs?userId=dog&startDate=${encodeURIComponent(start)}&endDate=${encodeURIComponent(end)}`),
    ReadWeek: () => request<WeekStats>('/focus/week-stats?userId=dog'),
    Quit: async () => {},
  };
}

export function backend(): Backend {
  if (window.go?.main.App) return window.go.main.App;
  if (import.meta.env.DEV && new URLSearchParams(location.search).has('fixture')) return localBackend();
  throw new Error('请在 Force 应用中打开');
}
