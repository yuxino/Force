type Name = 'timer' | 'calendar' | 'chart' | 'settings' | 'close' | 'pause' | 'stop' | 'plus' | 'refresh' | 'left' | 'right' | 'star' | 'down' | 'coffee' | 'chevron';
const paths: Record<Name, React.ReactNode> = {
  timer: <><circle cx="12" cy="14" r="8"/><path d="M12 10v4l3 2M9 2h6M12 2v4M18 6l2-2"/></>,
  calendar: <><rect x="3" y="5" width="18" height="16" rx="3"/><path d="M3 10h18M8 3v4M16 3v4"/></>,
  chart: <><path d="M4 3v17h17M8 15v-4M13 15V7M18 15V4"/></>,
  settings: <><path d="M4 6h16M4 12h16M4 18h16"/><circle cx="9" cy="6" r="2"/><circle cx="15" cy="12" r="2"/><circle cx="9" cy="18" r="2"/></>,
  close: <path d="m6 6 12 12M18 6 6 18"/>, pause: <><path d="M8 5v14M16 5v14"/></>,
  stop: <rect x="6" y="6" width="12" height="12" rx="2"/>, plus: <path d="M12 5v14M5 12h14"/>,
  refresh: <><path d="M20 7a8 8 0 1 0 0 10M20 3v5h-5"/></>, left: <path d="m14 5-7 7 7 7"/>, right: <path d="m10 5 7 7-7 7"/>,
  star: <path d="m12 3 2.7 5.5 6.1.9-4.4 4.3 1 6.1-5.4-2.9-5.4 2.9 1-6.1L3.2 9.4l6.1-.9L12 3Z"/>,
  down: <path d="m3 6 6 6 4-4 8 10M15 18h6v-6"/>,
  coffee: <><path d="M4 8h13v8a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5V8ZM17 9h2a3 3 0 0 1 0 6h-2M7 2v3M12 2v3"/></>,
  chevron: <path d="m7 10 5 5 5-5"/>,
};
export function Icon({ name, size = 16 }: { name: Name; size?: number }) {
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}
