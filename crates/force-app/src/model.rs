use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use chrono::{Days, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone};
use force_core::{Session, Stats, Store, Tag, TagKind};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Pixels, PromptButton, PromptLevel, Size,
    Subscription, Window, px, size,
};
use gpui_component::input::{InputEvent, InputState};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Page {
    Focus,
    Records,
    Settings,
}

pub(crate) struct ForceView {
    pub(crate) store: Option<Store>,
    pub(crate) data_dir: PathBuf,
    pub(crate) storage_failed: bool,
    pub(crate) tags: Vec<Tag>,
    pub(crate) active: Vec<Session>,
    pub(crate) history: Vec<Session>,
    pub(crate) completed: Option<Session>,
    pub(crate) today: Stats,
    pub(crate) record_stats: Stats,
    pub(crate) week: Vec<(NaiveDate, i64)>,
    pub(crate) selected_tag: i64,
    pub(crate) selected_session: Option<i64>,
    pub(crate) record_date: NaiveDate,
    pub(crate) goal: u32,
    pub(crate) page: Page,
    pub(crate) compact: bool,
    pub(crate) pinned: bool,
    pub(crate) normal_size: Size<Pixels>,
    pub(crate) task_input: Entity<InputState>,
    pub(crate) tag_input: Entity<InputState>,
    pub(crate) goal_input: Entity<InputState>,
    pub(crate) record_search: Entity<InputState>,
    pub(crate) new_tag_kind: TagKind,
    pub(crate) editing_tag: Option<i64>,
    pub(crate) toast: Option<(String, Instant)>,
    pub(crate) export_busy: bool,
    pub(crate) now: i64,
    pub(crate) timer_minutes: u32,
    pub(crate) countdown: bool,
    pub(crate) focus_handle: FocusHandle,
    data_version: i64,
    _subscriptions: Vec<Subscription>,
}

impl ForceView {
    pub(crate) fn new(data_dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let task_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("输入这次要专注的事，回车开始"));
        let tag_input = cx.new(|cx| InputState::new(window, cx).placeholder("新标签名称"));
        let goal_input = cx.new(|cx| InputState::new(window, cx).placeholder("每日目标（分钟）"));
        let record_search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索任务或标签"));
        let subscription = cx.subscribe_in(
            &task_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { secondary: false }) {
                    this.start(window, cx);
                }
            },
        );
        let tag_subscription = cx.subscribe_in(
            &tag_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { secondary: false }) {
                    this.save_tag(window, cx);
                }
            },
        );
        let search_subscription = cx.subscribe(&record_search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let mut view = Self {
            store: None,
            data_dir,
            storage_failed: false,
            tags: Vec::new(),
            active: Vec::new(),
            history: Vec::new(),
            completed: None,
            today: Stats::default(),
            record_stats: Stats::default(),
            week: Vec::new(),
            selected_tag: 0,
            selected_session: None,
            record_date: Local::now().date_naive(),
            goal: 300,
            page: Page::Focus,
            compact: false,
            pinned: false,
            normal_size: size(px(960.), px(720.)),
            task_input,
            tag_input,
            goal_input,
            record_search,
            new_tag_kind: TagKind::Focus,
            editing_tag: None,
            toast: None,
            export_busy: false,
            now: Local::now().timestamp(),
            timer_minutes: 25,
            countdown: false,
            focus_handle: cx.focus_handle(),
            data_version: 0,
            _subscriptions: vec![subscription, tag_subscription, search_subscription],
        };
        view.open_store(cx);
        let goal = view.goal.to_string();
        view.goal_input
            .update(cx, |input, cx| input.set_value(goal, window, cx));
        window.focus(&view.focus_handle);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        view
    }

    pub(crate) fn open_store(&mut self, cx: &mut Context<Self>) {
        match Store::open(self.data_dir.join("force.sqlite3")) {
            Ok(store) => {
                self.store = Some(store);
                self.storage_failed = false;
                self.refresh(cx);
            }
            Err(_) => {
                self.storage_failed = true;
                self.notify("无法打开本地数据。原文件已保留，请重试。", cx);
            }
        }
    }

    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.now = Local::now().timestamp();
        let Some(store) = self.store.as_mut() else {
            cx.notify();
            return;
        };
        let snapshot = (|| {
            let version = store.data_version()?;
            let mut active = store.active_sessions()?;
            let expired: Vec<_> = active
                .iter()
                .filter(|session| {
                    session
                        .target_seconds
                        .is_some_and(|target| session.elapsed_at(self.now) >= target)
                })
                .map(|session| session.id)
                .collect();
            // Resolve restored or just-paused timers before any statistics query,
            // so delayed ticks and computer sleep never inflate the first frame.
            let mut completed = None;
            for id in &expired {
                let session = match store.finish_exact_target(*id, self.now) {
                    Ok(session) => session,
                    Err(force_core::Error::InvalidTransition | force_core::Error::NotFound) => {
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                if completed.as_ref().is_none_or(|previous: &Session| {
                    (previous.finished_at, previous.id) < (session.finished_at, session.id)
                }) {
                    completed = Some(session);
                }
            }
            if !expired.is_empty() {
                active = store.active_sessions()?;
            }
            let (start, end) = day_bounds(self.record_date);
            Ok::<_, force_core::Error>((
                store.tags()?,
                active,
                store.sessions_between(start, end)?,
                store.goal_minutes()?,
                load_stats(store, self.record_date, self.now)?,
                completed,
                version,
            ))
        })();
        match snapshot {
            Ok((tags, active, history, goal, (today, record_stats, week), completed, version)) => {
                self.data_version = version;
                self.tags = tags;
                self.active = active;
                self.history = history;
                self.goal = goal;
                self.today = today;
                self.record_stats = record_stats;
                self.week = week;
                self.storage_failed = false;
                if !self.tags.iter().any(|tag| tag.id == self.selected_tag) {
                    self.selected_tag = self.tags.first().map_or(0, |tag| tag.id);
                }
                if !self
                    .active
                    .iter()
                    .any(|session| Some(session.id) == self.selected_session)
                {
                    self.selected_session = self.active.last().map(|session| session.id);
                }
                if let Some(completed) = completed {
                    self.completed = Some(completed);
                    self.toast = None;
                }
                cx.notify();
            }
            Err(_) => {
                self.storage_failed = true;
                self.notify("更新本地数据失败。已有内容已保留，请重试。", cx);
            }
        }
    }

    pub(crate) fn notify(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.toast = Some((message.into(), Instant::now()));
        cx.notify();
    }

    pub(crate) fn write<T>(
        &mut self,
        operation: impl FnOnce(&mut Store) -> force_core::Result<T>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(store) = self.store.as_mut() else {
            self.notify("本地数据尚未打开，请先重试。", cx);
            return false;
        };
        match operation(store) {
            Ok(_) => {
                self.refresh(cx);
                true
            }
            Err(error) => {
                let stale = matches!(
                    error,
                    force_core::Error::InvalidTransition | force_core::Error::NotFound
                );
                let message = match error {
                    force_core::Error::Validation(message) => {
                        validation_message(message).to_owned()
                    }
                    force_core::Error::NotFound => "记录已变更，请刷新后重试。".to_owned(),
                    force_core::Error::InvalidTransition => {
                        "当前状态无法执行此操作，请检查系统时间后重试。".to_owned()
                    }
                    _ => {
                        self.storage_failed = true;
                        "保存失败。现有记录已保留，请重试。".to_owned()
                    }
                };
                if stale {
                    self.refresh(cx);
                }
                self.notify(message, cx);
                false
            }
        }
    }

    pub(crate) fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.task_input.read(cx).value().to_string();
        let tag = self.selected_tag;
        let now = Local::now().timestamp();
        if !self.tags.iter().any(|item| item.id == tag) {
            self.notify("请先选择或添加一个标签。", cx);
            return;
        }
        let target = self
            .countdown
            .then_some(i64::from(self.timer_minutes).saturating_mul(60));
        let mut started = None;
        let saved = self.write(
            |store| {
                started = Some(match target {
                    Some(seconds) => store.start_timed(&title, tag, now, seconds)?,
                    None => store.start(&title, tag, now)?,
                });
                Ok(())
            },
            cx,
        );
        if saved {
            self.completed = None;
            self.selected_session = started.map(|session| session.id);
            self.task_input
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.page = Page::Focus;
            self.toast = None;
            cx.notify();
        }
    }

    pub(crate) fn repeat_completed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(completed) = self.completed.clone() else {
            return;
        };
        if !self.tags.iter().any(|tag| tag.id == completed.tag_id) {
            self.notify("原来的标签已删除，请选择标签开始新的计时。", cx);
            return;
        }
        let now = Local::now().timestamp();
        let mut started = None;
        if self.write(
            |store| {
                started = Some(match completed.target_seconds {
                    Some(target) => {
                        store.start_timed(&completed.title, completed.tag_id, now, target)?
                    }
                    None => store.start(&completed.title, completed.tag_id, now)?,
                });
                Ok(())
            },
            cx,
        ) {
            self.selected_tag = completed.tag_id;
            self.selected_session = started.map(|session| session.id);
            self.countdown = completed.target_seconds.is_some();
            if let Some(target) = completed.target_seconds {
                self.timer_minutes = u32::try_from(target / 60).unwrap_or(25);
            }
            self.completed = None;
            self.page = Page::Focus;
            self.toast = None;
            cx.notify();
        }
    }

    pub(crate) fn start_break(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let now = Local::now().timestamp();
        let mut started = None;
        if self.write(
            |store| {
                let tag = rest_tag(store)?;
                started = Some(store.start_timed("休息", tag.id, now, 300)?);
                Ok(())
            },
            cx,
        ) {
            if let Some(started) = started {
                self.selected_tag = started.tag_id;
                self.selected_session = Some(started.id);
            }
            self.completed = None;
            self.countdown = true;
            self.timer_minutes = 5;
            self.page = Page::Focus;
            self.toast = None;
            cx.notify();
        }
    }

    pub(crate) fn dismiss_completed(&mut self, cx: &mut Context<Self>) {
        self.completed = None;
        cx.notify();
    }

    pub(crate) fn begin_edit_tag(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tag) = self.tags.iter().find(|tag| tag.id == id) else {
            self.notify("这个标签已变更，请刷新后重试。", cx);
            return;
        };
        let name = tag.name.clone();
        self.new_tag_kind = tag.kind;
        self.editing_tag = Some(id);
        self.tag_input.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn save_tag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.tag_input.read(cx).value().to_string();
        let editing = self.editing_tag;
        let kind = self.new_tag_kind;
        let mut created_id = None;
        if self.write(
            |store| {
                if let Some(id) = editing {
                    store.rename_tag(id, &name)?;
                } else {
                    created_id = Some(store.create_tag(&name, kind)?.id);
                }
                Ok(())
            },
            cx,
        ) {
            if let Some(id) = created_id {
                self.selected_tag = id;
            }
            self.cancel_tag_edit(window, cx);
            self.notify(
                if editing.is_some() {
                    "标签已更新。"
                } else {
                    "标签已添加。"
                },
                cx,
            );
        }
    }

    pub(crate) fn cancel_tag_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_tag = None;
        self.new_tag_kind = TagKind::Focus;
        self.tag_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    pub(crate) fn toggle_pause(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(session) = self.active.iter().find(|session| session.id == id) else {
            self.notify("这项计时已结束，请刷新后重试。", cx);
            return;
        };
        let running = session.is_running();
        let now = Local::now().timestamp();
        self.write(
            |store| {
                if running {
                    store.pause(id, now)
                } else {
                    store.resume(id, now)
                }
            },
            cx,
        );
    }

    pub(crate) fn finish(&mut self, id: i64, cx: &mut Context<Self>) {
        let now = Local::now().timestamp();
        self.write(|store| store.finish(id, now), cx);
    }

    pub(crate) fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.toast = None;
        self.refresh(cx);
    }

    pub(crate) fn compact(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = window.viewport_size();
        if self.compact {
            if self.pinned {
                if crate::platform::set_always_on_top(window, false).is_err() {
                    self.notify("无法取消置顶，请重试。", cx);
                    return;
                }
                self.pinned = false;
            }
            self.compact = false;
            window.resize(self.normal_size);
        } else {
            self.normal_size = current;
            self.compact = true;
            window.resize(size(px(380.), px(116.)));
        }
        self.toast = None;
        cx.notify();
    }

    pub(crate) fn export(&mut self, cx: &mut Context<Self>) {
        if self.export_busy {
            return;
        }
        if self.store.is_none() || self.storage_failed {
            self.notify("请先打开本地数据再导出。", cx);
            return;
        }
        self.export_busy = true;
        let suggested = format!("force-{}.csv", Local::now().format("%Y-%m-%d"));
        let default_directory = directories::UserDirs::new()
            .map(|dirs| {
                dirs.document_dir()
                    .unwrap_or_else(|| dirs.home_dir())
                    .to_path_buf()
            })
            .unwrap_or_else(|| self.data_dir.clone());
        let path = cx.prompt_for_new_path(&default_directory, Some(&suggested));
        let database = self.data_dir.join("force.sqlite3");
        cx.notify();
        cx.spawn(async move |this, cx| {
            let selected_path = match path.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.export_busy = false;
                        cx.notify();
                    });
                    return;
                }
                _ => {
                    let _ = this.update(cx, |this, cx| {
                        this.export_busy = false;
                        this.notify("无法打开保存对话框，请重试。", cx);
                    });
                    return;
                }
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    let store = Store::open(&database)?;
                    let csv = store.csv_export(Local::now().timestamp())?;
                    write_export(&selected_path, csv.as_bytes(), &database)?;
                    Ok::<_, anyhow::Error>(())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.export_busy = false;
                this.notify(
                    if result.is_ok() {
                        "CSV 已导出。"
                    } else {
                        "导出失败，已有文件已保留，请重试。"
                    },
                    cx,
                );
            });
        })
        .detach();
    }

    pub(crate) fn delete_record(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let answer = window.prompt(
            PromptLevel::Warning,
            "删除这条记录？",
            Some("计时记录将被永久删除，统计也会更新。此操作无法撤销。"),
            &[PromptButton::cancel("取消"), PromptButton::new("删除")],
            cx,
        );
        cx.spawn(async move |this, cx| match answer.await {
            Ok(1) => {
                let _ = this.update(cx, |this, cx| {
                    if this.write(|store| store.delete_session(id), cx) {
                        this.notify("记录已删除。", cx);
                    }
                });
            }
            Ok(_) => {}
            Err(_) => {
                let _ = this.update(cx, |this, cx| this.notify("无法确认删除，请重试。", cx));
            }
        })
        .detach();
    }

    pub(crate) fn delete_tag(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let answer = window.prompt(
            PromptLevel::Warning,
            "删除这个标签？",
            Some("已有记录会保留原来的标签，之后不能再选择这个标签。"),
            &[PromptButton::cancel("取消"), PromptButton::new("删除")],
            cx,
        );
        cx.spawn(async move |this, cx| match answer.await {
            Ok(1) => {
                let _ = this.update(cx, |this, cx| {
                    if this.write(|store| store.delete_tag(id), cx) {
                        this.notify("标签已删除。", cx);
                    }
                });
            }
            Ok(_) => {}
            Err(_) => {
                let _ = this.update(cx, |this, cx| this.notify("无法确认删除，请重试。", cx));
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let previous_date = date_at(self.now);
        self.now = Local::now().timestamp();
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, since)| since.elapsed() >= Duration::from_secs(4))
        {
            self.toast = None;
        }
        if !self.storage_failed {
            match self.store.as_ref().map(Store::data_version) {
                Some(Ok(version)) if version != self.data_version => {
                    self.refresh(cx);
                    return;
                }
                Some(Err(_)) => {
                    self.storage_failed = true;
                    self.notify("读取本地数据失败，请重试。", cx);
                    return;
                }
                _ => {}
            }
        }
        let completed = self.active.iter().any(|session| {
            session
                .target_seconds
                .is_some_and(|target| session.elapsed_at(self.now) >= target)
        });
        if completed && !self.storage_failed {
            self.refresh(cx);
        } else if (self.active.iter().any(Session::is_running)
            || previous_date != date_at(self.now))
            && !self.storage_failed
        {
            let result = self
                .store
                .as_ref()
                .map(|store| load_stats(store, self.record_date, self.now));
            match result {
                Some(Ok((today, record_stats, week))) => {
                    self.today = today;
                    self.record_stats = record_stats;
                    self.week = week;
                }
                Some(Err(_)) => {
                    self.storage_failed = true;
                    self.notify("读取统计失败。已有记录已保留，请重试。", cx);
                }
                None => {}
            }
        }
        cx.notify();
    }
}

type DayStatistics = (Stats, Stats, Vec<(NaiveDate, i64)>);

fn rest_tag(store: &mut Store) -> force_core::Result<Tag> {
    let tags = store.tags()?;
    if let Some(tag) = tags.iter().find(|tag| tag.kind == TagKind::Rest) {
        return Ok(tag.clone());
    }
    let mut name = "休息".to_owned();
    let mut suffix = 2;
    while tags.iter().any(|tag| tag.name == name) {
        name = format!("休息 {suffix}");
        suffix += 1;
    }
    store.create_tag(&name, TagKind::Rest)
}

fn validation_message(message: &str) -> &'static str {
    match message {
        "Enter a task title of 1–200 characters" => "请输入 1–200 字的任务名称。",
        "Enter a tag name of 1–40 characters" => "请输入 1–40 字的标签名称。",
        "A tag with that name already exists" => "这个标签名称已存在，请换一个。",
        "Daily goal must be between 1 and 1440 minutes" => "每日目标应在 1–1440 分钟之间。",
        "Timer duration must be between 1 minute and 24 hours" => {
            "倒计时时长应在 1–1440 分钟之间。"
        }
        _ => "输入内容无效，请检查后重试。",
    }
}

fn load_stats(
    store: &Store,
    record_date: NaiveDate,
    now: i64,
) -> force_core::Result<DayStatistics> {
    let today_date = date_at(now);
    let (start, end) = day_bounds(today_date);
    let today = store.stats(start, end, now)?;
    let (start, end) = day_bounds(record_date);
    let record_stats = store.stats(start, end, now)?;
    let mut week = Vec::with_capacity(7);
    for ago in (0..7).rev() {
        let date = today_date
            .checked_sub_days(Days::new(ago))
            .unwrap_or(today_date);
        let (start, end) = day_bounds(date);
        week.push((date, store.stats(start, end, now)?.focus_seconds));
    }
    Ok((today, record_stats, week))
}

fn date_at(timestamp: i64) -> NaiveDate {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map_or_else(|| Local::now().date_naive(), |time| time.date_naive())
}

pub(crate) fn day_bounds(date: NaiveDate) -> (i64, i64) {
    let next_date = date.succ_opt().unwrap_or(date);
    (local_day_start(date), local_day_start(next_date))
}

fn local_day_start(date: NaiveDate) -> i64 {
    let mut candidate = NaiveDateTime::new(date, NaiveTime::MIN);
    loop {
        // A repeated midnight begins at its earliest instant. A skipped midnight
        // begins at the first valid local instant, including a skipped full day.
        if let Some(time) = Local.from_local_datetime(&candidate).earliest() {
            return time.timestamp();
        }
        match candidate.checked_add_signed(TimeDelta::seconds(1)) {
            Some(next) => candidate = next,
            None => return candidate.and_utc().timestamp(),
        }
    }
}

fn write_export(path: &Path, content: &[u8], database: &Path) -> std::io::Result<()> {
    let resolve = |path: &Path| {
        path.canonicalize().or_else(|_| {
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            Ok::<_, std::io::Error>(
                parent
                    .canonicalize()?
                    .join(path.file_name().unwrap_or_default()),
            )
        })
    };
    let selected = resolve(path)?;
    for suffix in ["", "-wal", "-shm"] {
        let protected = database.with_file_name(format!(
            "{}{}",
            database.file_name().unwrap_or_default().to_string_lossy(),
            suffix
        ));
        if resolve(&protected)? == selected {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "An export cannot replace the app database",
            ));
        }
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(".force-export-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(b"\xEF\xBB\xBF")?;
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_can_start_when_a_focus_tag_already_has_the_default_rest_name() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        for tag in store
            .tags()
            .unwrap()
            .into_iter()
            .filter(|tag| tag.kind == TagKind::Rest)
        {
            store.delete_tag(tag.id).unwrap();
        }
        store.create_tag("休息", TagKind::Focus).unwrap();
        let rest = rest_tag(&mut store).unwrap();
        assert_eq!(rest.kind, TagKind::Rest);
        assert_ne!(rest.name, "休息");
        let session = store.start_timed("休息", rest.id, 100, 300).unwrap();
        assert_eq!(session.tag_kind, TagKind::Rest);
        assert_eq!(rest_tag(&mut store).unwrap().id, rest.id);
    }

    #[test]
    fn exporting_replaces_csv_atomically_and_preserves_unicode() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("force.sqlite3");
        fs::write(&database, b"database").unwrap();
        let export = directory.path().join("records.csv");
        fs::write(&export, b"previous export").unwrap();
        write_export(&export, "title\r\n写作\r\n".as_bytes(), &database).unwrap();
        assert_eq!(
            fs::read(&export).unwrap(),
            "\u{feff}title\r\n写作\r\n".as_bytes()
        );
        assert_eq!(fs::read(&database).unwrap(), b"database");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn export_cannot_overwrite_the_database_or_its_journal() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("force.sqlite3");
        fs::write(&database, b"database").unwrap();
        for suffix in ["", "-wal", "-shm"] {
            let target = database.with_file_name(format!("force.sqlite3{suffix}"));
            assert!(write_export(&target, b"csv", &database).is_err());
        }
        assert_eq!(fs::read(&database).unwrap(), b"database");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
