use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::platform::{PANEL_HEIGHT, STRIP_HEIGHT};

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone};
use force_core::{Session, Stats, Store, Tag, TagKind, Task};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, PromptButton, PromptLevel, Subscription, Window,
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
    pub(crate) tasks: Vec<Task>,
    pub(crate) selected_task: Option<i64>,
    pub(crate) return_task: Option<i64>,
    return_target_seconds: Option<i64>,
    pub(crate) active: Vec<Session>,
    pub(crate) history: Vec<Session>,
    pub(crate) completed: Option<Session>,
    pub(crate) today: Stats,
    pub(crate) record_stats: Stats,
    pub(crate) selected_tag: i64,
    pub(crate) selected_session: Option<i64>,
    pub(crate) record_date: NaiveDate,
    pub(crate) goal: u32,
    pub(crate) page: Page,
    pub(crate) compact: bool,
    pub(crate) panel_above: bool,
    panel_height: f32,
    pub(crate) preparing: bool,
    pub(crate) pinned: bool,
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
    pub(crate) sound_enabled: bool,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) panel_scroll: [gpui::ScrollHandle; 3],
    data_version: i64,
    window_position: (f32, f32),
    position_changed: Option<Instant>,
    _subscriptions: Vec<Subscription>,
}

impl ForceView {
    pub(crate) fn new(data_dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let task_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("写下要做的事，回车开始"));
        let tag_input = cx.new(|cx| InputState::new(window, cx).placeholder("新标签名称"));
        let goal_input = cx.new(|cx| InputState::new(window, cx).placeholder("每日目标（分钟）"));
        let record_search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索任务或标签"));
        let subscription = cx.subscribe_in(
            &task_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { secondary: false })
                    && !this.compact
                    && this.page == Page::Focus
                    && this.completed.is_none()
                    && (this.preparing || this.active.is_empty())
                {
                    this.start(window, cx);
                }
            },
        );
        let tag_subscription = cx.subscribe_in(
            &tag_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { secondary: false })
                    && !this.compact
                    && this.page == Page::Settings
                {
                    this.save_tag(window, cx);
                }
            },
        );
        let search_subscription = cx.subscribe(&record_search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let position_subscription = cx.observe_window_bounds(window, |this, window, _| {
            let origin = window.bounds().origin;
            let offset = if this.panel_above {
                (f32::from(window.bounds().size.height) - STRIP_HEIGHT).max(0.)
            } else {
                0.
            };
            this.window_position = (f32::from(origin.x), f32::from(origin.y) + offset);
            this.position_changed = Some(Instant::now());
        });
        let quit_subscription = cx.on_app_quit(|this, cx| {
            let directory = this.data_dir.clone();
            let position = this.window_position;

            cx.background_executor().spawn(async move {
                let _ = crate::platform::persist_position(&directory, position);
            })
        });
        let mut view = Self {
            store: None,
            data_dir,
            storage_failed: false,
            tags: Vec::new(),
            tasks: Vec::new(),
            selected_task: None,
            return_task: None,
            return_target_seconds: Some(25 * 60),
            active: Vec::new(),
            history: Vec::new(),
            completed: None,
            today: Stats::default(),
            record_stats: Stats::default(),
            selected_tag: 0,
            selected_session: None,
            record_date: Local::now().date_naive(),
            goal: 300,
            page: Page::Focus,
            compact: true,
            panel_above: false,
            panel_height: STRIP_HEIGHT,
            preparing: false,
            pinned: cfg!(any(target_os = "macos", target_os = "windows")),
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
            countdown: true,
            sound_enabled: true,
            focus_handle: cx.focus_handle(),
            panel_scroll: std::array::from_fn(|_| gpui::ScrollHandle::default()),
            data_version: 0,
            window_position: (
                f32::from(window.bounds().origin.x),
                f32::from(window.bounds().origin.y),
            ),
            position_changed: None,
            _subscriptions: vec![
                subscription,
                tag_subscription,
                search_subscription,
                position_subscription,
                quit_subscription,
            ],
        };
        view.open_store(cx);
        if view
            .active
            .iter()
            .any(|session| session.tag_kind == TagKind::Rest)
            || view
                .completed
                .as_ref()
                .is_some_and(|session| session.tag_kind == TagKind::Rest)
        {
            match read_return_context(&view.data_dir) {
                Ok(Some((id, target)))
                    if view
                        .tasks
                        .iter()
                        .any(|task| task.id == id && task.completed_at.is_none()) =>
                {
                    view.return_task = Some(id);
                    view.return_target_seconds = target;
                }
                Ok(_) => {
                    view.return_task = None;
                }
                Err(_) => {
                    view.return_task = None;
                    view.notify("休息会正常结束，但无法读取要返回的任务。", cx);
                }
            }
        }
        let goal = view.goal.to_string();
        view.goal_input
            .update(cx, |input, cx| input.set_value(goal, window, cx));
        if let Some(session) = view
            .active
            .iter()
            .find(|session| Some(session.id) == view.selected_session)
            .cloned()
        {
            view.selected_task = session.task_id;
            view.selected_tag = session.tag_id;
            if session.tag_kind == TagKind::Focus {
                view.return_task = session.task_id;
                view.return_target_seconds = session.target_seconds;
            }
            view.task_input
                .update(cx, |input, cx| input.set_value(session.title, window, cx));
        }
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
                store.tasks(self.now)?,
                store.completion_sound()?,
            ))
        })();
        match snapshot {
            Ok((
                tags,
                active,
                history,
                goal,
                (today, record_stats),
                completed,
                version,
                tasks,
                sound_enabled,
            )) => {
                self.data_version = version;
                self.tags = tags;
                self.tasks = tasks;
                self.sound_enabled = sound_enabled;
                self.active = active;
                self.history = history;
                self.goal = goal;
                self.today = today;
                self.record_stats = record_stats;
                self.storage_failed = false;
                if !self.tags.iter().any(|tag| {
                    tag.id == self.selected_tag && (!self.preparing || tag.kind == TagKind::Focus)
                }) {
                    self.selected_tag = self
                        .tags
                        .iter()
                        .find(|tag| tag.kind == TagKind::Focus)
                        .map_or(0, |tag| tag.id);
                }
                if !self.preparing
                    && !self
                        .active
                        .iter()
                        .any(|session| Some(session.id) == self.selected_session)
                {
                    self.selected_session = self.active.last().map(|session| session.id);
                }
                if let Some(completed) = completed {
                    if completed.tag_kind == TagKind::Focus {
                        self.return_task = completed.task_id;
                        self.return_target_seconds = completed.target_seconds;
                    }
                    if self.preparing {
                        // Finishing the previous timer must not replace an
                        // unfinished task draft, its tag, or its chosen duration.
                        self.notify("上一段计时已结束并记录。", cx);
                    } else {
                        self.completed = Some(completed);
                        self.toast = None;
                    }
                    if self.sound_enabled {
                        crate::platform::play_completion_sound();
                    }
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
                    force_core::Error::ArchivedTag => {
                        "原标签已移除，请为这件事重新选择标签。".to_owned()
                    }
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
        if self.storage_failed {
            self.notify("请先重试打开本地数据。", cx);
            return;
        }
        let title = self.task_input.read(cx).value().trim().to_owned();
        if title.is_empty() {
            self.notify("先写下这次要做的事。", cx);
            return;
        }
        let tag = self.selected_tag;
        if !self.tags.iter().any(|item| item.id == tag) {
            self.notify("请为这件事选择一个标签。", cx);
            return;
        }
        let selected = self.selected_task;
        let target = self.countdown.then_some(i64::from(self.timer_minutes) * 60);
        let now = Local::now().timestamp();
        let mut started = None;
        let mut task_id = selected;
        let saved = self.write(
            |store| {
                let id = match selected {
                    Some(id) => {
                        store.update_task(id, &title, tag)?;
                        id
                    }
                    None => {
                        let task = store.create_task(&title, tag, now)?;
                        task_id = Some(task.id);
                        task.id
                    }
                };
                started = Some(
                    match store
                        .active_sessions()?
                        .into_iter()
                        .find(|session| session.task_id == Some(id) && session.is_running())
                    {
                        Some(session) => session,
                        None => store.start_task(id, now, target)?,
                    },
                );
                Ok(())
            },
            cx,
        );
        self.selected_task = task_id;
        if saved && let Some(session) = started {
            self.accept_started(session, window, cx);
        }
    }

    fn accept_started(&mut self, session: Session, window: &mut Window, cx: &mut Context<Self>) {
        if session.tag_kind == TagKind::Focus {
            self.return_task = session.task_id;
            self.return_target_seconds = session.target_seconds;
        }
        self.preparing = false;
        self.selected_session = Some(session.id);
        self.selected_task = session.task_id;
        self.selected_tag = session.tag_id;
        self.countdown = session.target_seconds.is_some();
        if let Some(target) = session.target_seconds {
            self.timer_minutes = u32::try_from(target / 60).unwrap_or(25);
        }
        self.task_input
            .update(cx, |input, cx| input.set_value(session.title, window, cx));
        self.completed = None;
        self.page = Page::Focus;
        self.toast = None;
        self.set_expanded(false, window, cx);
    }

    fn set_expanded(&mut self, expanded: bool, window: &mut Window, cx: &mut Context<Self>) {
        let height = if expanded {
            self.preferred_panel_height()
        } else {
            STRIP_HEIGHT
        };
        if expanded && self.compact {
            let bounds = window.bounds();
            if let Some(display) = window.display(cx) {
                let screen = display.bounds();
                let top =
                    f32::from(screen.origin.y) + if cfg!(target_os = "macos") { 28. } else { 8. };
                let bottom = f32::from(screen.origin.y + screen.size.height) - 8.;
                let bar_top = f32::from(bounds.origin.y);
                let bar_bottom = bar_top + STRIP_HEIGHT;
                self.panel_above = cfg!(any(target_os = "macos", target_os = "windows"))
                    && bottom - bar_top < height
                    && bar_bottom - top > bottom - bar_top;
            }
        }
        if crate::platform::resize_floating(window, height, self.panel_above, cx).is_err() {
            self.notify("无法调整浮窗大小，请重试。", cx);
            return;
        }
        self.compact = !expanded;
        if !expanded {
            window.focus(&self.focus_handle);
        }
        self.panel_height = height;
        cx.notify();
    }

    fn preferred_panel_height(&self) -> f32 {
        if self.page == Page::Focus
            && self.completed.is_none()
            && (self.preparing || self.active.is_empty())
        {
            let recent = self
                .tasks
                .iter()
                .filter(|task| task.completed_at.is_none())
                .take(2)
                .count();
            match recent {
                0 => 376.,
                _ => PANEL_HEIGHT,
            }
        } else if self.page == Page::Focus {
            if self.completed.is_some() { 388. } else { 376. }
        } else if self.page == Page::Records {
            416.
        } else {
            PANEL_HEIGHT
        }
    }

    pub(crate) fn sync_panel_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.compact && self.panel_height != self.preferred_panel_height() {
            self.set_expanded(true, window, cx);
        }
    }

    pub(crate) fn select_task(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(task) = self.tasks.iter().find(|task| task.id == id).cloned() else {
            self.notify("这件事已变更，请刷新后重试。", cx);
            return;
        };
        self.selected_task = Some(id);
        self.selected_tag = task.tag_id;
        self.selected_session = self
            .active
            .iter()
            .find(|session| session.task_id == Some(id))
            .map(|session| session.id);
        self.preparing = self.selected_session.is_none();
        self.completed = None;
        self.page = Page::Focus;
        self.task_input
            .update(cx, |input, cx| input.set_value(task.title, window, cx));
        self.set_expanded(true, window, cx);
    }

    pub(crate) fn new_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh(cx);
        self.panel_scroll[0].set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
        self.selected_task = None;
        self.preparing = true;
        self.selected_session = None;
        self.completed = None;
        self.page = Page::Focus;
        self.countdown = true;
        self.timer_minutes = 25;
        if !self
            .tags
            .iter()
            .any(|tag| tag.id == self.selected_tag && tag.kind == TagKind::Focus)
        {
            self.selected_tag = self
                .tags
                .iter()
                .find(|tag| tag.kind == TagKind::Focus)
                .map_or(0, |tag| tag.id);
        }
        self.task_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.set_expanded(true, window, cx);
    }

    pub(crate) fn primary_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.completed.is_some() {
            self.page = Page::Focus;
            self.set_expanded(true, window, cx);
        } else if let Some(id) = self
            .selected_session
            .filter(|id| self.active.iter().any(|session| session.id == *id))
            .or_else(|| {
                if self.compact || self.preparing {
                    self.active
                        .iter()
                        .find(|session| session.is_running())
                        .or_else(|| self.active.last())
                        .map(|session| session.id)
                } else {
                    None
                }
            })
        {
            self.toggle_pause(id, cx);
        } else if !self.task_input.read(cx).value().trim().is_empty() {
            self.start(window, cx);
        } else {
            self.new_task(window, cx);
        }
    }

    pub(crate) fn repeat_completed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(completed) = self.completed.clone() else {
            return;
        };
        let now = Local::now().timestamp();
        let mut started = None;
        if self.write(
            |store| {
                let id = match completed.task_id {
                    Some(id) => id,
                    None => {
                        store
                            .create_task(&completed.title, completed.tag_id, now)?
                            .id
                    }
                };
                started = Some(store.start_task(id, now, completed.target_seconds)?);
                Ok(())
            },
            cx,
        ) && let Some(session) = started
        {
            self.accept_started(session, window, cx);
        }
    }

    pub(crate) fn start_break(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(focus) = self
            .completed
            .as_ref()
            .filter(|session| session.tag_kind == TagKind::Focus)
            .or_else(|| {
                self.active.iter().find(|session| {
                    Some(session.id) == self.selected_session && session.tag_kind == TagKind::Focus
                })
            })
            .or_else(|| {
                self.active
                    .iter()
                    .find(|session| session.is_running() && session.tag_kind == TagKind::Focus)
            })
        {
            self.return_task = focus.task_id;
            self.return_target_seconds = focus.target_seconds;
        }
        if write_return_context(&self.data_dir, self.return_task, self.return_target_seconds)
            .is_err()
        {
            self.notify("无法保存休息后的返回任务，请重试。", cx);
            return;
        }
        let now = Local::now().timestamp();
        let mut started = None;
        if self.write(
            |store| {
                let tag = rest_tag(store)?;
                started = Some(store.start_break(tag.id, now, 300)?);
                Ok(())
            },
            cx,
        ) && let Some(session) = started
        {
            self.accept_started(session, window, cx);
        }
    }

    pub(crate) fn return_to_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.return_task.filter(|id| {
            self.tasks
                .iter()
                .any(|task| task.id == *id && task.completed_at.is_none())
        }) else {
            self.new_task(window, cx);
            return;
        };
        let now = Local::now().timestamp();
        let target = self.return_target_seconds;
        let mut started = None;
        if self.write(
            |store| {
                started = Some(store.start_task(id, now, target)?);
                Ok(())
            },
            cx,
        ) && let Some(session) = started
        {
            self.accept_started(session, window, cx);
        }
    }

    pub(crate) fn complete_current_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self
            .completed
            .as_ref()
            .and_then(|session| session.task_id)
            .or(self.selected_task);
        let Some(id) = id else {
            self.dismiss_completed(cx);
            return;
        };
        let now = Local::now().timestamp();
        if self.write(|store| store.complete_task(id, now), cx) {
            self.completed = None;
            self.selected_task = None;
            self.selected_session = None;
            self.return_task = None;
            self.new_task(window, cx);
            self.notify("任务已完成。", cx);
        }
    }

    pub(crate) fn archive_task(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .active
            .iter()
            .any(|session| session.task_id == Some(id))
        {
            self.notify("请先结束这件事的计时，再从最近任务移除。", cx);
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            "从最近任务移除？",
            Some("所有计时记录都会保留。"),
            &[PromptButton::cancel("取消"), PromptButton::new("移除")],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if let Ok(1) = answer.await {
                let _ = this.update(cx, |this, cx| {
                    if this.write(|store| store.delete_task(id), cx) {
                        if this.selected_task == Some(id) {
                            this.selected_task = None;
                        }
                        this.notify("已从最近任务移除，记录已保留。", cx);
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn set_sound_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.write(|store| store.set_completion_sound(enabled), cx);
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
        let task_id = session.task_id;
        let target = session.target_seconds;
        let now = Local::now().timestamp();
        self.write(
            |store| {
                if running {
                    store.pause(id, now)
                } else if let Some(task_id) = task_id {
                    store.start_task(task_id, now, target)
                } else {
                    store.resume(id, now)
                }
            },
            cx,
        );
    }

    pub(crate) fn finish(&mut self, id: i64, cx: &mut Context<Self>) {
        let now = Local::now().timestamp();
        let mut completed = None;
        if self.write(
            |store| {
                completed = Some(store.finish(id, now)?);
                Ok(())
            },
            cx,
        ) && let Some(session) = completed
        {
            if session.tag_kind == TagKind::Focus {
                self.return_task = session.task_id;
                self.return_target_seconds = session.target_seconds;
            }
            self.selected_session = None;
            if self.preparing {
                self.notify("上一段计时已结束并记录。", cx);
            } else {
                self.completed = Some(session);
                self.toast = None;
            }
            cx.notify();
        }
    }

    pub(crate) fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle);
        self.page = page;
        self.toast = None;
        self.refresh(cx);
    }

    pub(crate) fn compact(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.compact && self.selected_session.is_none() && self.completed.is_none() {
            self.selected_session = self
                .active
                .iter()
                .find(|session| session.is_running())
                .or_else(|| self.active.last())
                .map(|session| session.id);
        }
        self.set_expanded(self.compact, window, cx);
        self.toast = None;
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
        if self
            .position_changed
            .is_some_and(|since| since.elapsed() >= Duration::from_millis(600))
        {
            self.position_changed = None;
            let directory = self.data_dir.clone();
            let position = self.window_position;
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { crate::platform::persist_position(&directory, position) })
                    .await;
                if result.is_err() {
                    let _ = this.update(cx, |this, cx| {
                        this.notify("无法保存浮窗位置。计时记录不受影响。", cx)
                    });
                }
            })
            .detach();
        }
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
                Some(Ok((today, record_stats))) => {
                    self.today = today;
                    self.record_stats = record_stats;
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

fn write_return_context(
    directory: &Path,
    task: Option<i64>,
    target: Option<i64>,
) -> std::io::Result<()> {
    let data = format!("v1 {} {}\n", task.unwrap_or(0), target.unwrap_or(0));
    crate::platform::write_private_state(directory, "focus-context", data.as_bytes())
}

fn read_return_context(directory: &Path) -> std::io::Result<Option<(i64, Option<i64>)>> {
    let text = match fs::read_to_string(directory.join("focus-context")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid focus return context",
        )
    };
    let mut fields = text.split_whitespace();
    if fields.next() != Some("v1") {
        return Err(invalid());
    }
    let id = fields
        .next()
        .ok_or_else(invalid)?
        .parse::<i64>()
        .map_err(|_| invalid())?;
    let target = fields
        .next()
        .ok_or_else(invalid)?
        .parse::<i64>()
        .map_err(|_| invalid())?;
    if fields.next().is_some() || id < 0 || (target != 0 && !(60..=86400).contains(&target)) {
        return Err(invalid());
    }
    if id == 0 {
        return Ok(None);
    }
    Ok(Some((id, (target != 0).then_some(target))))
}

type DayStatistics = (Stats, Stats);

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
    Ok((today, record_stats))
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
    fn rest_return_context_round_trips_exact_task_and_timer_mode() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(read_return_context(directory.path()).unwrap(), None);
        write_return_context(directory.path(), Some(42), Some(2700)).unwrap();
        assert_eq!(
            read_return_context(directory.path()).unwrap(),
            Some((42, Some(2700)))
        );
        write_return_context(directory.path(), Some(7), None).unwrap();
        assert_eq!(
            read_return_context(directory.path()).unwrap(),
            Some((7, None))
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        fs::write(directory.path().join("focus-context"), "v1 42 -1").unwrap();
        assert!(read_return_context(directory.path()).is_err());
        assert_eq!(
            fs::read_to_string(directory.path().join("focus-context")).unwrap(),
            "v1 42 -1"
        );
    }

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
