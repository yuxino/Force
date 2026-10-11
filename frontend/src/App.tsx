import { useCallback, useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { backend, fallbackGeometry } from './bridge';
import { clock, duration, elapsed, goalTarget, kindOf, sameDay, totals } from './timing';
import type { Geometry, Kind, Log, Snapshot, WeekStats } from './types';
import { Icon } from './Icons';
import { notchShape } from './notchShape';

const empty: Snapshot = { tags: [], sessions: [], logs: [], syncedAt: '' };
const kinds: Record<Kind, string> = { positive: '正向', neutral: '中性', leisure: '休闲', negative: '负向' };
const kindIcon = { positive: 'star', neutral: 'timer', leisure: 'coffee', negative: 'down' } as const;
function storedMinutes() { try { const n = Number(localStorage.getItem('force:reminder') ?? '15'); return [0,5,15,30].includes(n) ? n : 15; } catch { return 15; } }

export function App() {
  const [state, setState] = useState<Snapshot>(empty);
  const [geometry, setGeometry] = useState<Geometry>(fallbackGeometry);
  const [expanded, setExpanded] = useState(false);
  const [view, setView] = useState<'focus'|'history'|'week'|'settings'>('focus');
  const [now, setNow] = useState(Date.now());
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const [online, setOnline] = useState(false);
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState('');
  const [kind, setKind] = useState<Kind>('neutral');
  const [minutes, setMinutes] = useState(storedMinutes);
  const [nudge, setNudge] = useState('');
  const writeRef = useRef(false);
  const readRef = useRef<Promise<void>|null>(null);
  const hoverRef = useRef<ReturnType<typeof setTimeout>|null>(null);
  const leaveRef = useRef<ReturnType<typeof setTimeout>|null>(null);
  const hoverOpened = useRef(false);
  const lastNudge = useRef(Date.now());
  const idleSince = useRef(Date.now());
  const previousCount = useRef(0);

  const refresh = useCallback(async () => {
    if (writeRef.current || readRef.current) return;
    const operation = (async () => {
      try { setState(await backend().ReadState()); setOnline(true); setError(''); }
      catch (e) { setOnline(false); setError(String(e)); }
      finally { setLoading(false); }
    })();
    readRef.current = operation;
    await operation;
    readRef.current = null;
  }, []);

  const open = useCallback(() => {
    hoverOpened.current=false;
    setExpanded(true);
    void backend().SetExpanded(true);
    void refresh();
  }, [refresh]);
  const close = useCallback(() => {
    hoverOpened.current=false;
    setExpanded(false); setAdding(false); setNudge('');
    void backend().SetExpanded(false);
  }, []);

  useEffect(() => {
    void backend().GetGeometry().then(setGeometry).catch(() => {});
    void refresh();
    const poll = setInterval(() => void refresh(), 15000);
    const tick = setInterval(() => setNow(Date.now()), 1000);
    const cleanups = [
      window.runtime?.EventsOn('force:expand', open),
      window.runtime?.EventsOn('force:collapse', close),
      window.runtime?.EventsOn('force:refresh', () => void refresh()),
      window.runtime?.EventsOn('force:geometry', g => setGeometry(g as Geometry)),
    ];
    function key(event: KeyboardEvent) { if (event.key === 'Escape') close(); }
    function visibility() { if (document.visibilityState === 'visible') void refresh(); }
    window.addEventListener('keydown', key);
    document.addEventListener('visibilitychange', visibility);
    return () => { clearInterval(poll); clearInterval(tick); cleanups.forEach(cleanup => cleanup?.()); window.removeEventListener('keydown', key); document.removeEventListener('visibilitychange', visibility); if (hoverRef.current) clearTimeout(hoverRef.current); if (leaveRef.current) clearTimeout(leaveRef.current); };
  }, [refresh, open, close]);

  useEffect(() => {
    if (previousCount.current > 0 && state.sessions.length === 0) idleSince.current = Date.now();
    previousCount.current = state.sessions.length;
  }, [state.sessions.length]);
  useEffect(() => {
    if (!minutes || !online || loading || pending || now-lastNudge.current < minutes*60000) return;
    const negative = state.sessions.find(s => kindOf(state.tags, s.tag) === 'negative' && elapsed(s, now) >= minutes*60);
    let message = '';
    if (negative) message = `「${negative.tag}」已持续 ${duration(elapsed(negative, now))}，要换件事做吗？`;
    else if (state.sessions.length === 0 && now-idleSince.current >= minutes*60000) message = `过去 ${minutes} 分钟没有记录。现在在做什么？`;
    if (message) { lastNudge.current=now; setNudge(message); setView('focus'); open(); }
  }, [now, minutes, online, loading, pending, state, open]);

  async function mutate(operation: () => Promise<Snapshot>) {
    if (writeRef.current || !online) return false;
    writeRef.current=true; setPending(true); setError('');
    try {
      if (readRef.current) await readRef.current;
      setState(await operation()); setOnline(true); setNudge(''); lastNudge.current=Date.now();
      return true;
    } catch (e) { setError(String(e)); setOnline(false); return false; }
    finally { writeRef.current=false; setPending(false); }
  }
  async function addTag(event: React.FormEvent) {
    event.preventDefault();
    const trimmed=name.trim();
    if (!trimmed) return;
    if (state.tags.some(t => t.name === trimmed)) { setError('这个标签已经存在'); return; }
    if (await mutate(() => backend().CreateTag(trimmed,kind))) { setName('');setAdding(false); }
  }

  const values = totals(state.tags,state.logs,state.sessions,now);
  const progress = Math.min(1, values.positive/goalTarget);
  const primary = state.sessions[0];
  const primaryKind = primary ? kindOf(state.tags,primary.tag) : 'neutral';
  const width = expanded ? geometry.expandedWidth : geometry.collapsedWidth;
  const height = expanded ? geometry.expandedHeight : geometry.top;
  const style = { '--gap': `${geometry.gap}px`, '--top': `${geometry.top}px`, '--compact-width': `${geometry.collapsedWidth}px`, '--panel-width': `${geometry.expandedWidth}px`, '--panel-height': `${geometry.expandedHeight}px`, width, height,
    clipPath: geometry.hasNotch ? `path("${notchShape(width,height,geometry.collapsedWidth,expanded)}")` : undefined } as CSSProperties;
  const disabled = pending || !online;

  return <main className={`force ${expanded ? 'expanded' : ''} ${geometry.hasNotch ? 'notched' : 'floating'}`} style={style}
    onPointerDown={() => { if (expanded) hoverOpened.current=false; }}
    onMouseEnter={() => { if (leaveRef.current) clearTimeout(leaveRef.current); }}
    onMouseLeave={() => {
      if (hoverRef.current) clearTimeout(hoverRef.current);
      if (leaveRef.current) clearTimeout(leaveRef.current);
      const editing=document.activeElement?.matches('input,select');
      if (expanded && hoverOpened.current && !editing && !pending) leaveRef.current=setTimeout(close,500);
    }}>
    <button className="notch-strip" aria-label="展开 Force" onClick={open} tabIndex={expanded?-1:0} aria-hidden={expanded} disabled={expanded}
      onMouseEnter={() => { if (!expanded) hoverRef.current=setTimeout(() => {open();hoverOpened.current=true;},400); }}>
      <span className={`strip-label ${primaryKind}`}><span className={`status-dot ${primary ? 'running' : ''}`}/><span>{loading ? '正在连接' : primary?.tag ?? '选一件事'}</span>{state.sessions.length>1 && <small>+{state.sessions.length-1}</small>}</span>
      <span className="camera-gap" aria-hidden="true"/>
      <span className="strip-time">{primary ? clock(elapsed(primary,now)) : 'Force'}{!online && !loading && <span className="offline-dot" title="连接中断"/>}</span>
    </button>

    <div className="panel" inert={!expanded} aria-hidden={!expanded}>
      <header className="panel-header"><div><span className="wordmark">Force<span className="wordmark-dot">.</span></span><p>把时间放在眼前。</p></div>
        <nav aria-label="面板导航">
          {(['focus','history','week','settings'] as const).map((tab,i) => <button key={tab} className={`icon-button ${view===tab?'selected':''}`} aria-label={['专注','历史记录','本周统计','设置'][i]} title={['专注','历史记录','本周统计','设置'][i]} onClick={() => {setView(tab);setAdding(false);}}><Icon name={(['timer','calendar','chart','settings'] as const)[i]}/></button>)}
        </nav>
      </header>

      <div className="panel-scroll">
        {nudge && <div className="nudge"><span>{nudge}</span><button className="icon-button" aria-label="关闭提醒" onClick={() => {setNudge('');lastNudge.current=Date.now();}}><Icon name="close"/></button></div>}
        {view==='focus' && <>
          {state.sessions.length ? <section className="sessions" aria-label="正在计时">{state.sessions.map(session => <div className={`session ${kindOf(state.tags,session.tag)}`} key={session.id}>
            <div className="session-name"><span className="status-dot running"/><span title={session.tag}>{session.tag}</span></div>
            <span className="session-clock">{clock(elapsed(session,now))}</span>
            <button className="icon-button" title="结束本段计时并保存记录" aria-label={`停止 ${session.tag}`} disabled={disabled} onClick={() => void mutate(() => backend().SetTagRunning(session.tag,false))}><Icon name="pause" size={15}/></button>
          </div>)}</section> : <section className="idle"><span className="idle-clock">00:00:00</span><p>{loading ? '正在读取 Nichijou 的记录…' : '选一个标签，开始记录这段时间。'}</p></section>}

          <section className="daily-goal" aria-label="今日目标">
            <div className="section-label"><span>今日目标</span><span className="goal-value">{duration(values.positive)} <span className="muted">/ 5 小时</span></span></div>
            <div className="progress-track" role="progressbar" aria-label="今日专注目标" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(progress*100)}><span style={{width:`${progress*100}%`}}/></div>
            <div className="goal-caption"><span>只累计正向标签 · 可并行计时</span><span>{Math.floor(progress*100)}%</span></div>
            {(values.negative>0 || values.leisure>0) && <div className="secondary-totals">{values.leisure>0 && <span><Icon name="coffee" size={12}/>休闲 {duration(values.leisure)}</span>}{values.negative>0 && <span className="negative"><Icon name="down" size={12}/>负向 {duration(values.negative)}</span>}</div>}
          </section>

          <section className="tag-section" aria-label="选择标签">
            <div className="section-label"><span>现在在做什么</span><button className="text-button" disabled={disabled} onClick={() => setAdding(!adding)}><Icon name={adding?'close':'plus'} size={13}/>{adding?'取消':'新标签'}</button></div>
            {adding && <form className="add-tag" onSubmit={addTag}><input autoFocus aria-label="标签名称" placeholder="标签名称" maxLength={60} value={name} onChange={e=>setName(e.target.value)} disabled={pending}/><select aria-label="标签类别" value={kind} onChange={e=>setKind(e.target.value as Kind)}>{Object.entries(kinds).map(([value,label])=><option key={value} value={value}>{label}</option>)}</select><button className="primary-button" disabled={disabled||!name.trim()}>添加</button></form>}
            <div className="tags">{state.tags.filter(t=>!t.archivedAt).sort((a,b)=>a.sortOrder-b.sortOrder).map(tag=> {
              const active=state.sessions.some(s=>s.tag===tag.name);
              return <button key={tag.id} title={`${tag.name} · ${kinds[tag.kind]??'中性'}${active?' · 点击结束并保存':''}`} className={`tag ${tag.kind} ${active?'active':''}`} aria-pressed={active} disabled={disabled} onClick={()=>void mutate(()=>backend().SetTagRunning(tag.name,!active))}><Icon name={active?'pause':kindIcon[tag.kind]??'timer'} size={13}/><span>{tag.name}</span></button>;
            })}</div>
            {!loading && state.tags.length===0 && <p className="empty-copy">{online?'还没有标签，添加一个开始记录。':'恢复连接后显示你的标签。'}</p>}
          </section>
        </>}
        {view==='history' && <History state={state} now={now}/>}
        {view==='week' && <Week state={state}/>}
        {view==='settings' && <section className="settings">
          <h2>让时间有迹可循。</h2>
          <div className="setting-row"><div><strong>提醒间隔</strong><p>没有计时或负向计时过久时，展开提醒。</p></div><select aria-label="提醒间隔" value={minutes} onChange={e=>{const value=Number(e.target.value);setMinutes(value);localStorage.setItem('force:reminder',String(value));lastNudge.current=Date.now();}}>{[0,5,15,30].map(n=><option value={n} key={n}>{n ? `${n} 分钟` : '关闭'}</option>)}</select></div>
          <div className="setting-row"><div><strong>与 Nichijou 共用记录</strong><p>标签、正在进行的任务和历史都来自同一服务。</p></div><span className="setting-value">已接入</span></div>
          <div className="settings-note"><p>每 15 秒同步一次，展开时立即刷新。</p><p>正向标签累计今日 5 小时目标。多个任务分别累计。记录归入开始当天，跨午夜任务与网页使用相同规则。</p><p>关闭 Force 或电脑睡眠后，服务器计时继续。结束计时请先停止任务。</p><p>数据只在连接成功后写入；断网时显示上次同步内容。</p></div>
          <button className="outline-button" onClick={()=>void backend().Quit()}>退出 Force<span className="muted">计时继续</span></button>
          <p className="version">Force 0.1.0 · Wails</p>
        </section>}
      </div>

      <footer className="panel-footer">
        {error && <div className="error" role="alert"><span>{error.replace(/^Error:\s*/,'')}</span><button className="text-button" disabled={pending} onClick={()=>void refresh()}>重试</button></div>}
        <div className="footer-row"><button className="sync-button" aria-label="刷新同步" onClick={()=>void refresh()} disabled={pending}><Icon name="refresh" size={12}/>{pending?'正在保存…':loading?'连接中…':online?'已同步 Nichijou':'连接中断'}</button>
          {view==='focus' && state.sessions.length>0 ? <button className="stop-all" disabled={disabled} onClick={()=>void mutate(()=>backend().StopAll())}><Icon name="stop" size={12}/>停止全部 <span>{state.sessions.length}</span></button> : <button className="icon-button" aria-label="收起面板" onClick={close}><Icon name="chevron" size={14}/></button>}
        </div>
      </footer>
    </div>
  </main>;
}

function History({state,now}:{state:Snapshot;now:number}) {
  const [month,setMonth]=useState(()=>new Date(new Date(now).getFullYear(),new Date(now).getMonth(),1));
  const [selected,setSelected]=useState(()=>new Date(now).getDate());
  const [logs,setLogs]=useState<Log[]>(state.logs);
  const [error,setError]=useState('');
  const [loading,setLoading]=useState(false);
  useEffect(()=>{
    let cancelled=false;
    setLoading(true);setError('');
    const end=new Date(month.getFullYear(),month.getMonth()+1,1);end.setMilliseconds(-1);
    void backend().ReadHistory(month.toISOString(),end.toISOString()).then(data=>{if(!cancelled)setLogs(data);}).catch(e=>{if(!cancelled)setError(String(e));}).finally(()=>{if(!cancelled)setLoading(false);});
    return()=>{cancelled=true;};
  },[month,state.syncedAt]);
  const date=new Date(month.getFullYear(),month.getMonth(),selected);
  const dayLogs=logs.filter(l=>sameDay(new Date(l.startTime),date));
  const dayTotals=totals(state.tags,dayLogs,[],date.getTime());
  const days=new Date(month.getFullYear(),month.getMonth()+1,0).getDate();
  const pad=(month.getDay()+6)%7;
  function changeMonth(delta:number){setMonth(new Date(month.getFullYear(),month.getMonth()+delta,1));setSelected(1);}
  return <section className="history">
    <div className="section-label month-heading"><button className="icon-button" aria-label="上个月" onClick={()=>changeMonth(-1)}><Icon name="left" size={14}/></button><h2>{month.getFullYear()} 年 {month.getMonth()+1} 月</h2><button className="icon-button" aria-label="下个月" onClick={()=>changeMonth(1)}><Icon name="right" size={14}/></button></div>
    <div className="calendar">{['一','二','三','四','五','六','日'].map(day=><span className="weekday" key={day}>{day}</span>)}{Array.from({length:pad},(_,i)=><span key={`pad-${i}`}/>)}{Array.from({length:days},(_,i)=>{
      const day=i+1; const d=new Date(month.getFullYear(),month.getMonth(),day);
      const recorded=logs.some(l=>sameDay(new Date(l.startTime),d));
      return <button key={day} className={`calendar-day ${selected===day?'selected':''} ${sameDay(d,new Date(now))?'today':''}`} onClick={()=>setSelected(day)} aria-label={`${month.getMonth()+1} 月 ${day} 日${recorded?'，有记录':''}`} aria-pressed={selected===day}>{day}{recorded&&<span/>}</button>;
    })}</div>
    <div className="day-summary"><span>{month.getMonth()+1} 月 {selected} 日</span><strong>正向 {duration(dayTotals.positive)}</strong></div>
    {loading ? <p className="empty-copy">读取记录…</p> : error ? <p className="error">{error}</p> : dayLogs.length ? <div className="history-logs">{dayLogs.map(log=><div className="history-log" key={log.id}><div><span className={kindOf(state.tags,log.tag)}>{log.tag}</span><small>{new Date(log.startTime).toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit'})} — {new Date(log.endTime).toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit'})}</small></div><span>{duration(log.duration)}</span></div>)}</div>:<p className="empty-copy">这一天还没有已完成的记录。</p>}
  </section>;
}

function Week({state}:{state:Snapshot}) {
  const [stats,setStats]=useState<WeekStats|null>(null);
  const [error,setError]=useState('');
  useEffect(()=>{let cancelled=false;void backend().ReadWeek().then(data=>{if(!cancelled){setStats(data);setError('');}}).catch(e=>{if(!cancelled)setError(String(e));});return()=>{cancelled=true;};},[state.syncedAt]);
  if(error)return <p className="error">{error}</p>;
  if(!stats)return <p className="empty-copy">读取本周统计…</p>;
  const max=Math.max(...stats.dailyStats.map(d=>d.totalDuration),1);
  return <section className="week"><span className="eyebrow">本周实际投入</span><h2 className="week-total">{duration(stats.totalDuration)}</h2><p className="muted">{stats.weekStart} — {stats.weekEnd} · 重叠时段去重</p><div className="week-bars">{stats.dailyStats.map((day,i)=><div className="week-bar" key={day.date} title={`${day.date}：${duration(day.totalDuration)}`}><span>{Math.floor(day.totalDuration/3600)}h</span><div><i style={{height:`${day.totalDuration/max*100}%`}}/></div><span>{['一','二','三','四','五','六','日'][i]}</span></div>)}</div><div className="week-tags">{stats.tagStats.map(tag=><div className="history-log" key={tag.tag}><span className={kindOf(state.tags,tag.tag)}>{tag.tag}</span><span>{duration(tag.duration)}</span></div>)}</div><p className="goal-caption">标签投入时长分别累计。</p></section>;
}
