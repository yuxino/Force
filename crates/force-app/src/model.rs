use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::tag_picker::{FocusTagOption, TagPicked, TagPicker};

use chrono::{Datelike, Days, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone};
use force_core::{MAX_GOAL_MINUTES, Session, Stats, Store, Tag, TagKind, Task};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, PromptButton, PromptLevel, Subscription, Window,
    WindowHandle,
};
use gpui_component::{
    Root,
    input::{InputEvent, InputState},
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Page {
    Focus,
    Records,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActivityDay {
    pub(crate) date: NaiveDate,
    pub(crate) seconds: i64,
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
    pub(crate) activity_days: Vec<ActivityDay>,
    pub(crate) selected_tag: i64,
    pub(crate) tag_picker: Entity<TagPicker>,
    tag_picker_options: Vec<FocusTagOption>,
    pub(crate) selected_session: Option<i64>,
    pub(crate) record_date: NaiveDate,
    pub(crate) goal: u32,
    pub(crate) page: Page,
    pub(crate) compact: bool,
    pub(crate) manager_window: Option<WindowHandle<Root>>,
    pub(crate) strip_window: Option<WindowHandle<Root>>,
    pub(crate) preparing: bool,
    pub(crate) pinned: bool,
    pub(crate) task_input: Entity<InputState>,
    pub(crate) tag_input: Entity<InputState>,
    pub(crate) goal_input: Entity<InputState>,
    pub(crate) record_search: Entity<InputState>,
    pub(crate) record_help_open: bool,
    pub(crate) record_details: Option<i64>,
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
    pub(crate) tag_scroll: gpui::ScrollHandle,
    pub(crate) tag_form_anchor: gpui::ScrollAnchor,
    data_version: i64,
    window_position: (f32, f32),
    strip_position_initialized: bool,
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
        let tag_picker = cx.new(TagPicker::new);
        let tag_picker_subscription =
            cx.subscribe(&tag_picker, |this, _, event: &TagPicked, cx| {
                let id = event.0;
                if this
                    .tags
                    .iter()
                    .any(|tag| tag.id == id && tag.kind == TagKind::Focus)
                {
                    this.selected_tag = id;
                    cx.notify();
                } else {
                    this.notify("这个专注标签已变更，请重新选择。", cx);
                }
            });
        let subscription = cx.subscribe_in(
            &task_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
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
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                } else if matches!(event, InputEvent::PressEnter { secondary: false })
                    && !this.compact
                    && this.page == Page::Settings
                {
                    this.save_tag(window, cx);
                }
            },
        );
        let goal_subscription =
            cx.subscribe_in(&goal_input, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                } else if matches!(event, InputEvent::PressEnter { secondary: false })
                    && !this.compact
                    && this.page == Page::Settings
                {
                    this.save_goal(cx);
                }
            });
        let search_subscription =
            cx.subscribe(&record_search, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.record_details = None;
                    this.panel_scroll[1].set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
                    cx.notify();
                }
            });
        let quit_subscription = cx.on_app_quit(|this, cx| {
            let directory = this.data_dir.clone();
            let position = this
                .strip_position_initialized
                .then_some(this.window_position);

            cx.background_executor().spawn(async move {
                if let Some(position) = position {
                    let _ = crate::platform::persist_position(&directory, position);
                }
            })
        });
        let panel_scroll: [gpui::ScrollHandle; 3] = std::array::from_fn(|_| Default::default());
        let tag_form_anchor = gpui::ScrollAnchor::for_handle(panel_scroll[2].clone());
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
            activity_days: Vec::new(),
            selected_tag: 0,
            tag_picker,
            tag_picker_options: Vec::new(),
            selected_session: None,
            record_date: Local::now().date_naive(),
            goal: 300,
            page: Page::Focus,
            compact: true,
            manager_window: None,
            strip_window: None,
            preparing: false,
            pinned: cfg!(any(target_os = "macos", target_os = "windows")),
            task_input,
            tag_input,
            goal_input,
            record_search,
            record_help_open: false,
            record_details: None,
            new_tag_kind: TagKind::Focus,
            editing_tag: None,
            toast: None,
            export_busy: false,
            now: Local::now().timestamp(),
            timer_minutes: 25,
            countdown: true,
            sound_enabled: true,
            focus_handle: cx.focus_handle(),
            panel_scroll,
            tag_scroll: gpui::ScrollHandle::default(),
            tag_form_anchor,
            data_version: 0,
            window_position: (0., 0.),
            strip_position_initialized: false,
            position_changed: None,
            _subscriptions: vec![
                subscription,
                tag_subscription,
                goal_subscription,
                search_subscription,
                tag_picker_subscription,
                quit_subscription,
            ],
        };
        match read_selected_session(&view.data_dir) {
            Ok(selected) => view.selected_session = selected,
            Err(_) => view.notify("无法读取上次选择的计时，将显示可用计时。", cx),
        }
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
            .completed
            .as_ref()
            .or_else(|| {
                view.active
                    .iter()
                    .find(|session| Some(session.id) == view.selected_session)
            })
            .cloned()
        {
            view.selected_task = session.task_id;
            view.selected_tag = session.tag_id;
            view.countdown = session.target_seconds.is_some();
            if let Some(target) = session.target_seconds {
                view.timer_minutes = u32::try_from(target / 60).unwrap_or(25);
            }
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

    pub(crate) fn sync_tag_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let options: Vec<_> = self
            .tags
            .iter()
            .filter(|tag| tag.kind == TagKind::Focus)
            .map(|tag| FocusTagOption {
                id: tag.id,
                name: tag.name.clone(),
            })
            .collect();
        let selected = options
            .iter()
            .find(|option| option.id == self.selected_tag)
            .map(|option| option.id);
        let enabled = self.page == Page::Focus
            && !self.compact
            && !self.storage_failed
            && self.completed.is_none()
            && (self.preparing || self.active.is_empty());
        let options_changed = options != self.tag_picker_options;
        let picker = self.tag_picker.read(cx);
        let selection_changed = picker.selected_value() != selected;
        let enabled_changed = picker.is_enabled() != enabled;
        if !options_changed && !selection_changed && !enabled_changed {
            return;
        }
        self.tag_picker.update(cx, |picker, cx| {
            picker.synchronize(options.clone(), selected, enabled, window, cx);
        });
        self.tag_picker_options = options;
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
                if self.page == Page::Records {
                    load_activity_days(store, date_at(self.now), self.now)?
                } else {
                    Vec::new()
                },
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
                activity_days,
            )) => {
                self.data_version = version;
                self.tags = tags;
                self.tasks = tasks;
                self.sound_enabled = sound_enabled;
                self.active = active;
                self.history = history;
                if self
                    .record_details
                    .is_some_and(|id| !self.history.iter().any(|record| record.id == id))
                {
                    self.record_details = None;
                }
                self.goal = goal;
                self.today = today;
                self.record_stats = record_stats;
                self.activity_days = activity_days;
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
                    self.selected_session =
                        preferred_session(&self.active).map(|session| session.id);
                }
                if let Some(completed) = completed {
                    if completed.tag_kind == TagKind::Focus {
                        self.return_task = completed.task_id;
                        self.return_target_seconds = completed.target_seconds;
                    }
                    if self.preparing || self.active.iter().any(Session::is_running) {
                        // Completion must not hide a running timer or replace
                        // an unfinished task draft and its chosen duration.
                        self.notify("上一段计时已结束并记录。", cx);
                    } else {
                        self.reset_focus_scroll();
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

    pub(crate) fn select_record_date(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        if date > Local::now().date_naive() || date == self.record_date {
            return;
        }
        self.record_date = date;
        self.record_details = None;
        // Selecting a heatmap cell keeps the activity grid at the same place.
        self.refresh(cx);
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

    fn apply_session_selection(&mut self, session: &Session) {
        self.selected_session = Some(session.id);
        self.selected_task = session.task_id;
        self.selected_tag = session.tag_id;
        self.countdown = session.target_seconds.is_some();
        if let Some(target) = session.target_seconds {
            self.timer_minutes = u32::try_from(target / 60).unwrap_or(25);
        }
    }

    fn reset_focus_scroll(&mut self) {
        self.panel_scroll[0].set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
    }

    fn remember_return_context(&mut self, cx: &mut Context<Self>) {
        if write_return_context(&self.data_dir, self.return_task, self.return_target_seconds)
            .is_err()
        {
            self.notify("计时已保存，但无法保存休息后的返回任务。", cx);
        }
    }

    fn accept_started(&mut self, session: Session, window: &mut Window, cx: &mut Context<Self>) {
        if session.tag_kind == TagKind::Focus {
            self.return_task = session.task_id;
            self.return_target_seconds = session.target_seconds;
        }
        self.preparing = false;
        self.apply_session_selection(&session);
        self.reset_focus_scroll();
        self.task_input.update(cx, |input, cx| {
            input.set_value(session.title.clone(), window, cx)
        });
        self.completed = None;
        self.page = Page::Focus;
        self.toast = None;
        self.remember_selected_session(cx);
        if session.tag_kind == TagKind::Focus {
            self.remember_return_context(cx);
        }
        // Starting or resuming a timer preserves whichever windows the user
        // currently has open. Closing management remains an explicit action.
        cx.notify();
    }

    pub(crate) fn remember_selected_session(&mut self, cx: &mut Context<Self>) {
        let data = format!("v1 {}\n", self.selected_session.unwrap_or(0));
        if crate::platform::write_private_state(&self.data_dir, "selected-session", data.as_bytes())
            .is_err()
        {
            self.notify("无法保存当前计时的选择。计时记录不受影响。", cx);
        }
    }

    fn set_expanded(&mut self, expanded: bool, window: &mut Window, cx: &mut Context<Self>) {
        if expanded {
            self.show_manager(window, cx);
        } else {
            self.hide_manager(window, cx);
        }
    }

    pub(crate) fn show_manager(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.compact = false;
        // A model created in TestAppContext has no attached native windows.
        if self.manager_window.is_some() && crate::platform::show_manager(window, cx).is_err() {
            self.notify("无法打开管理窗口，请重试。", cx);
        }
        cx.notify();
    }

    pub(crate) fn hide_manager(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tag_picker
            .update(cx, |picker, cx| picker.dismiss(window, false, cx));
        window.focus(&self.focus_handle);
        self.compact = true;
        if self.manager_window.is_some() && crate::platform::hide_manager(window, cx).is_err() {
            self.notify("无法隐藏管理窗口，请重试。", cx);
        }
        cx.notify();
    }

    pub(crate) fn toggle_strip_pin(&mut self, cx: &mut Context<Self>) {
        let Some(strip) = self.strip_window else {
            return;
        };
        let pinned = !self.pinned;
        let model = cx.entity().downgrade();
        cx.defer(move |cx| {
            let result = strip.update(cx, |_, window, _| {
                crate::platform::set_always_on_top(window, pinned)
            });
            let _ = model.update(cx, |this, cx| {
                if result.is_ok_and(|result| result.is_ok()) {
                    this.pinned = pinned;
                    cx.notify();
                } else {
                    this.notify("无法更改计时条置顶状态，请重试。", cx);
                }
            });
        });
    }

    pub(crate) fn select_session(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.active.iter().find(|session| session.id == id).cloned() else {
            self.notify("这项计时已结束，请刷新后重试。", cx);
            return;
        };
        self.preparing = false;
        self.completed = None;
        self.page = Page::Focus;
        self.apply_session_selection(&session);
        self.reset_focus_scroll();
        self.task_input
            .update(cx, |input, cx| input.set_value(session.title, window, cx));
        self.remember_selected_session(cx);
        cx.notify();
    }

    pub(crate) fn select_task(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(task) = self.tasks.iter().find(|task| task.id == id).cloned() else {
            self.notify("这件事已变更，请刷新后重试。", cx);
            return;
        };
        self.selected_task = Some(id);
        self.selected_tag = task.tag_id;
        if let Some(session) = self
            .active
            .iter()
            .find(|session| session.task_id == Some(id))
            .cloned()
        {
            self.apply_session_selection(&session);
            self.preparing = false;
        } else {
            self.selected_session = None;
            self.preparing = true;
        }
        self.reset_focus_scroll();
        self.completed = None;
        self.page = Page::Focus;
        self.task_input
            .update(cx, |input, cx| input.set_value(task.title, window, cx));
        self.remember_selected_session(cx);
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

    pub(crate) fn strip_session(&self) -> Option<&Session> {
        preferred_strip_session(&self.active, self.selected_session, self.now)
    }

    pub(crate) fn strip_primary_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.storage_failed {
            self.page = Page::Focus;
            self.show_manager(window, cx);
            return;
        }
        // A completion receipt may coexist with other paused rounds. Its play
        // action opens the receipt rather than resuming an unseen paused timer.
        if self.completed.is_some() && running_session(&self.active, self.now).is_none() {
            self.primary_action(window, cx);
        } else if let Some(id) = self.strip_session().map(|session| session.id) {
            self.toggle_pause(id, cx);
        } else {
            self.primary_action(window, cx);
        }
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

    pub(crate) fn goal_draft(&self, cx: &gpui::App) -> Option<u32> {
        parse_goal_draft(self.goal_input.read(cx).value().as_ref())
    }

    pub(crate) fn can_save_goal(&self, cx: &gpui::App) -> bool {
        goal_draft_has_changes(self.goal_input.read(cx).value().as_ref(), self.goal)
    }

    pub(crate) fn goal_has_unsaved_changes(&self, cx: &gpui::App) -> bool {
        goal_draft_has_unsaved_changes(self.goal_input.read(cx).value().as_ref(), self.goal)
    }

    pub(crate) fn reset_goal_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let goal = self.goal.to_string();
        self.goal_input.update(cx, |input, cx| {
            input.set_value(goal, window, cx);
            input.focus(window, cx);
        });
        self.toast = None;
        cx.notify();
    }

    pub(crate) fn save_goal(&mut self, cx: &mut Context<Self>) {
        let Some(minutes) = self.goal_draft(cx) else {
            self.notify("请输入 1–1440 之间的分钟数。", cx);
            return;
        };
        if self.can_save_goal(cx) && self.write(|store| store.set_goal_minutes(minutes), cx) {
            self.notify("目标已保存。", cx);
        }
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
        self.tag_form_anchor.scroll_to(window, cx);
        cx.notify();
    }

    pub(crate) fn tag_draft_error(&self, cx: &gpui::App) -> Option<&'static str> {
        tag_draft_error_for(
            self.tag_input.read(cx).value().as_ref(),
            self.editing_tag,
            &self.tags,
        )
    }

    pub(crate) fn can_save_tag(&self, cx: &gpui::App) -> bool {
        if self.tag_draft_error(cx).is_some() {
            return false;
        }
        let original = match self.editing_tag {
            Some(id) => match self.tags.iter().find(|tag| tag.id == id) {
                Some(tag) => Some(tag.name.as_str()),
                None => return false,
            },
            None => None,
        };
        tag_draft_has_changes(self.tag_input.read(cx).value().as_ref(), original)
    }

    pub(crate) fn save_tag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.tag_input.read(cx).value().to_string();
        if let Some(message) = self.tag_draft_error(cx) {
            self.notify(message, cx);
            return;
        }
        if !self.can_save_tag(cx) {
            if name.trim().is_empty() {
                self.notify("请输入 1–40 字的标签名称。", cx);
            }
            return;
        }
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
            if kind == TagKind::Focus
                && let Some(id) = created_id
            {
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
        self.tag_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn open_tag_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Page::Settings, window, cx);
        if self.editing_tag.is_some() {
            self.cancel_tag_edit(window, cx);
        }
        self.new_tag_kind = TagKind::Focus;
        // Let the settings form lay out before seeking its scroll anchor.
        let anchor = self.tag_form_anchor.clone();
        window.on_next_frame(move |window, cx| {
            anchor.scroll_to(window, cx);
            window.request_animation_frame();
        });
        self.tag_input
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub(crate) fn toggle_pause(&mut self, id: i64, cx: &mut Context<Self>) {
        let now = Local::now().timestamp();
        let mut outcome = None;
        if self.write(
            |store| {
                outcome = Some(toggle_session(store, id, now)?);
                Ok(())
            },
            cx,
        ) && let Some(outcome) = outcome
        {
            let remember_return = outcome.return_context.is_some();
            if let Some((task, target)) = outcome.return_context {
                self.return_task = task;
                self.return_target_seconds = target;
            }
            // The strip may control the previous timer while a new task draft
            // is open. Keep that draft and its chosen tag/duration intact.
            if !self.preparing
                && self
                    .active
                    .iter()
                    .any(|session| session.id == outcome.session.id)
            {
                self.apply_session_selection(&outcome.session);
                self.completed = None;
                if outcome.session.is_running() {
                    self.reset_focus_scroll();
                }
                self.remember_selected_session(cx);
            }
            if remember_return {
                self.remember_return_context(cx);
            }
            cx.notify();
        }
    }

    pub(crate) fn finish(&mut self, id: i64, cx: &mut Context<Self>) {
        let now = Local::now().timestamp();
        let mut outcome = None;
        if self.write(
            |store| {
                outcome = Some(finish_session(store, id, now)?);
                Ok(())
            },
            cx,
        ) && let Some(outcome) = outcome
        {
            let foreground = outcome
                .foreground
                .filter(|session| {
                    self.active
                        .iter()
                        .any(|active| active.id == session.id && active.is_running())
                })
                .or_else(|| running_session(&self.active, self.now).cloned());
            let return_context = foreground
                .as_ref()
                .filter(|s| s.tag_kind == TagKind::Focus)
                .or_else(|| {
                    (outcome.finished.tag_kind == TagKind::Focus).then_some(&outcome.finished)
                })
                .map(|session| (session.task_id, session.target_seconds));
            if let Some((task, target)) = return_context {
                self.return_task = task;
                self.return_target_seconds = target;
            }
            if self.preparing {
                self.selected_session = None;
                self.notify("上一段计时已结束并记录。", cx);
            } else {
                self.reset_focus_scroll();
                if let Some(session) = foreground {
                    self.apply_session_selection(&session);
                    self.completed = None;
                    self.notify("上一段计时已结束并记录。", cx);
                } else {
                    self.selected_session = None;
                    self.completed = Some(outcome.finished);
                    self.toast = None;
                }
            }
            self.remember_selected_session(cx);
            if return_context.is_some() {
                self.remember_return_context(cx);
            }
            cx.notify();
        }
    }

    pub(crate) fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle);
        self.page = page;
        self.toast = None;
        self.refresh(cx);
        self.show_manager(window, cx);
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
        cx.spawn_in(window, async move |this, cx| match answer.await {
            Ok(1) => {
                let _ = cx.update(|window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if this.write(|store| store.delete_tag(id), cx) {
                            if this.editing_tag == Some(id) {
                                this.cancel_tag_edit(window, cx);
                            }
                            this.notify("标签已删除。", cx);
                        }
                    });
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
        if (completed || previous_date != date_at(self.now)) && !self.storage_failed {
            self.refresh(cx);
        } else if self.active.iter().any(Session::is_running) && !self.storage_failed {
            let result = self
                .store
                .as_ref()
                .map(|store| load_stats(store, self.record_date, self.now));
            match result {
                Some(Ok((today, record_stats))) => {
                    self.today = today;
                    self.record_stats = record_stats;
                    let date = date_at(self.now);
                    if let Some(day) = self.activity_days.iter_mut().find(|day| day.date == date) {
                        day.seconds = today.focus_seconds;
                    }
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

pub(crate) struct FloatingStrip {
    pub(crate) model: Entity<ForceView>,
    pub(crate) manager: WindowHandle<Root>,
    pub(crate) focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl FloatingStrip {
    pub(crate) fn new(
        model: Entity<ForceView>,
        manager: WindowHandle<Root>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let bounds = window.bounds();
        model.update(cx, |this, _| {
            this.window_position = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
            this.strip_position_initialized = true;
        });
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        let position = cx.observe_window_bounds(window, |this, window, cx| {
            let origin = window.bounds().origin;
            this.model.update(cx, |model, _| {
                model.window_position = (f32::from(origin.x), f32::from(origin.y));
                model.position_changed = Some(Instant::now());
            });
        });
        Self {
            model,
            manager,
            focus_handle: cx.focus_handle(),
            _subscriptions: vec![observe, position],
        }
    }

    pub(crate) fn dispatch(
        &self,
        cx: &mut Context<Self>,
        operation: impl FnOnce(&mut ForceView, &mut Window, &mut Context<ForceView>) + 'static,
    ) {
        let manager = self.manager;
        let model = self.model.clone();
        // InputState belongs to the manager. Never handle its actions using
        // the strip's Window, and wait until the dispatching borrow has ended.
        cx.defer(move |cx| {
            let _ = manager.update(cx, |_, window, cx| {
                model.update(cx, |model, cx| operation(model, window, cx));
            });
        });
    }
}

struct TimerToggleOutcome {
    session: Session,
    return_context: Option<(Option<i64>, Option<i64>)>,
}

fn toggle_session(store: &mut Store, id: i64, now: i64) -> force_core::Result<TimerToggleOutcome> {
    let active = store.active_sessions()?;
    let session = active
        .iter()
        .find(|session| session.id == id)
        .ok_or(force_core::Error::NotFound)?;
    let return_context = if session.tag_kind == TagKind::Focus {
        Some((session.task_id, session.target_seconds))
    } else if session.is_paused() {
        active
            .iter()
            .filter(|focus| {
                focus.tag_kind == TagKind::Focus
                    && focus.is_running()
                    && focus
                        .target_seconds
                        .is_none_or(|target| focus.elapsed_at(now) < target)
            })
            .max_by_key(|focus| (focus.started_at, focus.id))
            .map(|focus| (focus.task_id, focus.target_seconds))
    } else {
        None
    };
    let changed = if session.is_running() {
        store.pause(id, now)?
    } else if session.tag_kind == TagKind::Rest {
        store.resume_break(id, now)?
    } else if let Some(task) = session.task_id {
        store.start_task(task, now, session.target_seconds)?
    } else {
        store.resume_focus(id, now)?
    };
    Ok(TimerToggleOutcome {
        session: changed,
        return_context,
    })
}

struct SessionFinishOutcome {
    finished: Session,
    foreground: Option<Session>,
}

fn finish_session(
    store: &mut Store,
    id: i64,
    now: i64,
) -> force_core::Result<SessionFinishOutcome> {
    let finished = store.finish(id, now)?;
    let active = store.active_sessions()?;
    let foreground = running_session(&active, now).cloned();
    Ok(SessionFinishOutcome {
        finished,
        foreground,
    })
}

fn running_session(active: &[Session], now: i64) -> Option<&Session> {
    active
        .iter()
        .filter(|session| {
            session.is_running()
                && session
                    .target_seconds
                    .is_none_or(|target| session.elapsed_at(now) < target)
        })
        .max_by_key(|session| (session.started_at, session.id))
}

fn preferred_strip_session(
    active: &[Session],
    selected: Option<i64>,
    now: i64,
) -> Option<&Session> {
    running_session(active, now)
        .or_else(|| active.iter().find(|session| Some(session.id) == selected))
        .or_else(|| active.last())
}

// A newer paused round must never hide the timer that is still running after
// restart. Use timestamp/id ordering rather than depending on query order.
fn preferred_session(active: &[Session]) -> Option<&Session> {
    active
        .iter()
        .max_by_key(|session| (session.is_running(), session.started_at, session.id))
}

fn read_selected_session(directory: &Path) -> std::io::Result<Option<i64>> {
    let text = match fs::read_to_string(directory.join("selected-session")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let fields: Vec<_> = text.split_whitespace().collect();
    if let ["v1", id] = fields.as_slice()
        && let Ok(id) = id.parse::<i64>()
        && id >= 0
    {
        return Ok((id != 0).then_some(id));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Invalid selected session",
    ))
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

fn parse_goal_draft(value: &str) -> Option<u32> {
    value
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|minutes| (1..=MAX_GOAL_MINUTES).contains(minutes))
}

fn goal_draft_has_changes(value: &str, saved: u32) -> bool {
    parse_goal_draft(value).is_some_and(|minutes| minutes != saved)
}

fn goal_draft_has_unsaved_changes(value: &str, saved: u32) -> bool {
    parse_goal_draft(value) != Some(saved)
}

fn tag_draft_error_for(value: &str, editing: Option<i64>, tags: &[Tag]) -> Option<&'static str> {
    if editing.is_some_and(|id| !tags.iter().any(|tag| tag.id == id)) {
        return Some("这个标签已被移除，请取消后重新选择。");
    }
    let name = value.trim();
    if name.contains('\0') {
        return Some("标签名称包含无效字符。");
    }
    if name.chars().count() > 40 {
        return Some("标签名称最多 40 字。");
    }
    if !name.is_empty()
        && tags
            .iter()
            .any(|tag| Some(tag.id) != editing && tag.name.eq_ignore_ascii_case(name))
    {
        return Some("这个标签名称已存在，请换一个。");
    }
    None
}

fn tag_draft_has_changes(value: &str, original: Option<&str>) -> bool {
    let name = value.trim();
    !name.is_empty() && name.chars().count() <= 40 && !name.contains('\0') && original != Some(name)
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

fn activity_dates(today: NaiveDate) -> Vec<NaiveDate> {
    let days_back = u64::from(today.weekday().num_days_from_monday()) + 52 * 7;
    let start = today
        .checked_sub_days(Days::new(days_back))
        .unwrap_or(today);
    (0..53 * 7)
        .filter_map(|offset| start.checked_add_days(Days::new(offset)))
        .collect()
}

fn load_activity_days(
    store: &Store,
    today: NaiveDate,
    now: i64,
) -> force_core::Result<Vec<ActivityDay>> {
    let dates = activity_dates(today);
    let ranges: Vec<_> = dates.iter().copied().map(day_bounds).collect();
    let totals = store.focus_totals_for_ranges(&ranges, now)?;
    Ok(dates
        .into_iter()
        .zip(totals)
        .map(|(date, seconds)| ActivityDay { date, seconds })
        .collect())
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
    fn legacy_focus_toggle_ends_rest_and_pauses_other_focus_without_replacing_the_round() {
        for target in [None, Some(1500)] {
            let directory = tempfile::tempdir().unwrap();
            let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
            let focus = match target {
                Some(target) => store.start_timed("Legacy focus", 1, 100, target).unwrap(),
                None => store.start("Legacy focus", 1, 100).unwrap(),
            };
            toggle_session(&mut store, focus.id, 110).unwrap();
            let rest = store.start_break(4, 120, 300).unwrap();
            let resumed = toggle_session(&mut store, focus.id, 130).unwrap();
            assert_eq!(resumed.session.id, focus.id);
            assert_eq!(resumed.session.task_id, None);
            assert_eq!(resumed.session.title, focus.title);
            assert_eq!(resumed.session.target_seconds, target);
            assert_eq!(resumed.return_context, Some((None, target)));
            assert_eq!(store.active_sessions().unwrap().len(), 1);
            assert_eq!(
                store
                    .sessions()
                    .unwrap()
                    .iter()
                    .find(|session| session.id == rest.id)
                    .unwrap()
                    .finished_at,
                Some(130)
            );

            let other_task = store.create_task("Other focus", 1, 140).unwrap();
            let other = store.start_task(other_task.id, 140, None).unwrap();
            let resumed = toggle_session(&mut store, focus.id, 150).unwrap();
            assert_eq!(resumed.session.id, focus.id);
            assert_eq!(resumed.session.elapsed_at(160), 30);
            assert_eq!(resumed.session.target_seconds, target);
            let active = store.active_sessions().unwrap();
            assert_eq!(
                active.iter().filter(|session| session.is_running()).count(),
                1
            );
            assert!(
                active
                    .iter()
                    .find(|session| session.id == other.id)
                    .unwrap()
                    .is_paused()
            );
            assert_eq!(store.stats(0, 200, 160).unwrap().focus_seconds, 40);
        }
    }

    #[test]
    fn resuming_paused_rest_through_model_toggle_pauses_focus_and_keeps_identity() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        let task = store.create_task("A", 1, 100).unwrap();
        let focus = store.start_task(task.id, 100, Some(1500)).unwrap();
        let rest = store.start_break(4, 130, 300).unwrap();
        toggle_session(&mut store, rest.id, 140).unwrap();
        toggle_session(&mut store, focus.id, 150).unwrap();
        let resumed = toggle_session(&mut store, rest.id, 160).unwrap();
        assert_eq!(resumed.session.id, rest.id);
        assert_eq!(resumed.session.task_id, None);
        assert_eq!(resumed.session.target_seconds, Some(300));
        assert_eq!(resumed.return_context, Some((Some(task.id), Some(1500))));
        let active = store.active_sessions().unwrap();
        assert_eq!(
            active.iter().filter(|session| session.is_running()).count(),
            1
        );
        assert!(
            active
                .iter()
                .find(|session| session.id == focus.id)
                .unwrap()
                .is_paused()
        );
        assert_eq!(store.stats(0, 200, 170).unwrap().focus_seconds, 40);
        assert_eq!(resumed.session.elapsed_at(170), 20);

        // Fast explicit pause/resume must keep the same round and never let
        // the focus task restart underneath the visible break.
        toggle_session(&mut store, rest.id, 170).unwrap();
        let again = toggle_session(&mut store, rest.id, 170).unwrap();
        assert_eq!(again.session.id, rest.id);
        assert_eq!(store.stats(0, 200, 180).unwrap().focus_seconds, 40);
        assert_eq!(
            store
                .active_sessions()
                .unwrap()
                .iter()
                .filter(|s| s.is_running())
                .count(),
            1
        );
    }

    #[test]
    fn resuming_legacy_free_rest_keeps_its_mode_and_pauses_focus() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        let rest = store.start("Legacy rest", 4, 100).unwrap();
        toggle_session(&mut store, rest.id, 110).unwrap();
        let task = store.create_task("Focus", 1, 120).unwrap();
        let focus = store.start_task(task.id, 120, None).unwrap();
        let resumed = toggle_session(&mut store, rest.id, 130).unwrap();
        assert_eq!(resumed.session.id, rest.id);
        assert_eq!(resumed.session.target_seconds, None);
        assert_eq!(resumed.session.title, "Legacy rest");
        assert!(
            store
                .active_sessions()
                .unwrap()
                .iter()
                .find(|session| session.id == focus.id)
                .unwrap()
                .is_paused()
        );
    }

    #[test]
    fn finishing_a_paused_task_keeps_the_other_running_task_in_the_foreground() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        let first = store.create_task("A", 1, 100).unwrap();
        let a = store.start_task(first.id, 100, Some(1500)).unwrap();
        let second = store.create_task("B", 1, 110).unwrap();
        let b = store.start_task(second.id, 110, None).unwrap();
        let outcome = finish_session(&mut store, a.id, 120).unwrap();
        assert_eq!(outcome.finished.id, a.id);
        let foreground = outcome.foreground.unwrap();
        assert_eq!(foreground.id, b.id);
        assert_eq!(foreground.task_id, Some(second.id));
        assert_eq!(foreground.title, "B");
        assert_eq!(foreground.target_seconds, None);
        let outcome = finish_session(&mut store, b.id, 130).unwrap();
        assert!(outcome.foreground.is_none());
        assert_eq!(outcome.finished.id, b.id);
    }

    #[test]
    fn failed_rest_resume_rolls_back_any_other_timer_changes() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        let rest = store.start_timed("Rest", 4, 100, 300).unwrap();
        store.pause(rest.id, 110).unwrap();
        let task = store.create_task("Future boundary", 1, 200).unwrap();
        let focus = store.start_task(task.id, 200, None).unwrap();
        assert!(matches!(
            toggle_session(&mut store, rest.id, 190),
            Err(force_core::Error::InvalidTransition)
        ));
        let active = store.active_sessions().unwrap();
        assert!(
            active
                .iter()
                .find(|session| session.id == focus.id)
                .unwrap()
                .is_running()
        );
        assert!(
            active
                .iter()
                .find(|session| session.id == rest.id)
                .unwrap()
                .is_paused()
        );
    }

    #[test]
    fn strip_prioritizes_running_timer_over_a_browsed_paused_task() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        let a = store.create_task("A", 1, 100).unwrap();
        let a = store.start_task(a.id, 100, Some(1500)).unwrap();
        let b = store.create_task("B", 1, 110).unwrap();
        let b = store.start_task(b.id, 110, None).unwrap();
        let active = store.active_sessions().unwrap();
        assert_eq!(
            preferred_strip_session(&active, Some(a.id), 120)
                .unwrap()
                .id,
            b.id
        );

        // With no running timer, the explicit browsing selection takes priority.
        toggle_session(&mut store, b.id, 120).unwrap();
        let active = store.active_sessions().unwrap();
        assert_eq!(
            preferred_strip_session(&active, Some(a.id), 120)
                .unwrap()
                .id,
            a.id
        );
        assert_eq!(
            preferred_strip_session(&active, Some(b.id), 120)
                .unwrap()
                .id,
            b.id
        );

        toggle_session(&mut store, a.id, 130).unwrap();
        let active = store.active_sessions().unwrap();
        assert_eq!(
            preferred_strip_session(&active, Some(b.id), 140)
                .unwrap()
                .id,
            a.id
        );
    }

    #[test]
    fn strip_falls_back_safely_when_the_browsed_session_is_stale() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        let task = store.create_task("Paused", 1, 100).unwrap();
        let paused = store.start_task(task.id, 100, None).unwrap();
        toggle_session(&mut store, paused.id, 110).unwrap();
        let active = store.active_sessions().unwrap();
        assert_eq!(
            preferred_strip_session(&active, Some(i64::MAX), 120)
                .unwrap()
                .id,
            paused.id
        );
        assert_eq!(
            preferred_strip_session(&active, None, 120).unwrap().id,
            active.last().unwrap().id
        );
        assert!(preferred_strip_session(&[], Some(paused.id), 120).is_none());
    }

    #[test]
    fn activity_calendar_covers_53_monday_to_sunday_weeks() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let dates = activity_dates(today);
        assert_eq!(dates.len(), 53 * 7);
        assert_eq!(dates[0], NaiveDate::from_ymd_opt(2025, 10, 20).unwrap());
        assert_eq!(dates[364], NaiveDate::from_ymd_opt(2026, 10, 19).unwrap());
        assert_eq!(
            *dates.last().unwrap(),
            NaiveDate::from_ymd_opt(2026, 10, 25).unwrap()
        );
        assert!(
            dates
                .windows(2)
                .all(|pair| pair[0].succ_opt() == Some(pair[1]))
        );
    }

    #[test]
    fn activity_days_use_local_midnight_and_zero_future_cells() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let (midnight, _) = day_bounds(today);
        store
            .start("Across local midnight", 1, midnight - 30)
            .unwrap();
        let days = load_activity_days(&store, today, midnight + 20).unwrap();
        assert_eq!(
            days.iter()
                .find(|day| day.date == today.pred_opt().unwrap())
                .unwrap()
                .seconds,
            30
        );
        assert_eq!(
            days.iter().find(|day| day.date == today).unwrap().seconds,
            20
        );
        assert!(
            days.iter()
                .filter(|day| day.date > today)
                .all(|day| day.seconds == 0)
        );
    }

    #[test]
    fn goal_drafts_require_a_changed_value_within_daily_bounds() {
        assert_eq!(parse_goal_draft(" 1 "), Some(1));
        assert_eq!(parse_goal_draft("1440"), Some(MAX_GOAL_MINUTES));
        for invalid in ["", "0", "1441", "-1", "2.5", "abc", "4294967296"] {
            assert_eq!(parse_goal_draft(invalid), None, "{invalid}");
            assert!(!goal_draft_has_changes(invalid, 300), "{invalid}");
        }
        assert!(!goal_draft_has_changes(" 0300 ", 300));
        assert!(goal_draft_has_changes("301", 300));
    }

    #[test]
    fn goal_reset_is_available_for_changed_or_invalid_drafts() {
        for draft in ["", "0", "1441", "invalid", "301"] {
            assert!(goal_draft_has_unsaved_changes(draft, 300), "{draft}");
        }
        for draft in ["300", " 300 ", "0300"] {
            assert!(!goal_draft_has_unsaved_changes(draft, 300), "{draft}");
        }
    }

    #[test]
    fn tag_drafts_reject_blank_long_and_unchanged_names() {
        assert!(!tag_draft_has_changes(" ", None));
        assert!(!tag_draft_has_changes(&"字".repeat(41), None));
        assert!(tag_draft_has_changes(&"字".repeat(40), None));
        assert!(!tag_draft_has_changes("  阅读  ", Some("阅读")));
        assert!(tag_draft_has_changes("写作", Some("阅读")));
        assert!(tag_draft_has_changes("READING", Some("Reading")));
    }

    #[test]
    fn tag_draft_uniqueness_matches_sqlite_across_kinds_and_allows_self_rename() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        let reading = store.create_tag("Reading", TagKind::Focus).unwrap();
        store.create_tag("Break", TagKind::Rest).unwrap();
        for name in ["reading", " READING ", "break"] {
            assert!(tag_draft_error_for(name, None, &store.tags().unwrap()).is_some());
            assert!(matches!(
                store.create_tag(name, TagKind::Focus),
                Err(force_core::Error::Validation(_))
            ));
        }
        assert_eq!(
            tag_draft_error_for(" READING ", Some(reading.id), &store.tags().unwrap()),
            None
        );
        assert_eq!(
            store.rename_tag(reading.id, "READING").unwrap().name,
            "READING"
        );

        let archived = store.create_tag("Archived", TagKind::Focus).unwrap();
        store.delete_tag(archived.id).unwrap();
        assert_eq!(
            tag_draft_error_for("archived", None, &store.tags().unwrap()),
            None
        );
        store.create_tag("archived", TagKind::Rest).unwrap();

        // SQLite's built-in NOCASE folds ASCII, not arbitrary Unicode letters.
        store.create_tag("Ä", TagKind::Focus).unwrap();
        assert_eq!(tag_draft_error_for("ä", None, &store.tags().unwrap()), None);
        store.create_tag("ä", TagKind::Rest).unwrap();
    }

    #[test]
    fn tag_draft_errors_reject_nul_and_orphaned_edits_without_flagging_empty_new_input() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
        assert_eq!(tag_draft_error_for(" ", None, &store.tags().unwrap()), None);
        assert!(tag_draft_error_for("name\0suffix", None, &store.tags().unwrap()).is_some());
        assert!(!tag_draft_has_changes("name\0suffix", None));
        assert!(matches!(
            store.create_tag("name\0suffix", TagKind::Focus),
            Err(force_core::Error::Validation(_))
        ));
        let tag = store.create_tag("Removed", TagKind::Focus).unwrap();
        store.delete_tag(tag.id).unwrap();
        assert!(tag_draft_error_for("Renamed", Some(tag.id), &store.tags().unwrap()).is_some());
        assert!(tag_draft_error_for("Renamed", Some(i64::MAX), &store.tags().unwrap()).is_some());
        assert!(matches!(
            store.rename_tag(tag.id, "Renamed"),
            Err(force_core::Error::NotFound)
        ));
    }

    #[test]
    fn selected_timer_state_survives_restart_and_rejects_invalid_files() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(read_selected_session(directory.path()).unwrap(), None);
        crate::platform::write_private_state(directory.path(), "selected-session", b"v1 42\n")
            .unwrap();
        assert_eq!(read_selected_session(directory.path()).unwrap(), Some(42));
        crate::platform::write_private_state(directory.path(), "selected-session", b"v1 0\n")
            .unwrap();
        assert_eq!(read_selected_session(directory.path()).unwrap(), None);
        for invalid in ["v1 -1", "v2 42", "v1 42 extra", "", "v1 invalid"] {
            fs::write(directory.path().join("selected-session"), invalid).unwrap();
            assert!(read_selected_session(directory.path()).is_err());
            assert_eq!(
                fs::read_to_string(directory.path().join("selected-session")).unwrap(),
                invalid
            );
        }
    }

    #[test]
    fn restart_prefers_running_timer_over_newer_paused_rounds() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("force.sqlite3");
        let mut store = Store::open(&path).unwrap();
        let first = store.create_task("Earlier task", 1, 100).unwrap();
        let running = store.start_task(first.id, 100, None).unwrap();
        let second = store.create_task("Newer paused task", 1, 200).unwrap();
        let paused = store.start_task(second.id, 200, Some(2700)).unwrap();
        store.pause(paused.id, 210).unwrap();
        store.start_task(first.id, 220, None).unwrap();
        drop(store);

        let store = Store::open(&path).unwrap();
        let mut active = store.active_sessions().unwrap();
        assert_eq!(active.last().unwrap().id, paused.id);
        assert_eq!(preferred_session(&active).unwrap().id, running.id);
        active.reverse();
        assert_eq!(preferred_session(&active).unwrap().id, running.id);
        drop(store);

        let mut store = Store::open(&path).unwrap();
        store.pause(running.id, 230).unwrap();
        let active = store.active_sessions().unwrap();
        assert_eq!(preferred_session(&active).unwrap().id, paused.id);
        assert!(preferred_session(&[]).is_none());
    }

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
