use chrono::{Datelike, Local, NaiveDate, NaiveTime, TimeZone, Timelike};
use force_core::{
    CheckInDay, Error, LifeBoard, LifeItem, LifeKind, LifeModule, LifeOccurrence, Store,
};
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, KeyBinding, PromptButton,
    PromptLevel, ScrollHandle, Subscription, Window, point, px,
};
use gpui_component::input::{InputEvent, InputState};

use crate::model::{ForceView, Page};

gpui::actions!(life_editor, [NextField, PreviousField]);

#[derive(Clone)]
pub(crate) enum LifeEdit {
    Add(Option<NaiveDate>),
    Record(LifeItem, Option<NaiveDate>),
    Edit(LifeItem),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CheckInMetric {
    #[default]
    Count,
    Duration,
}

impl LifeEdit {
    pub(crate) fn kind(&self, add_kind: LifeKind) -> LifeKind {
        match self {
            Self::Add(None) => add_kind,
            Self::Add(Some(_)) => LifeKind::CheckIn,
            Self::Record(item, _) | Self::Edit(item) => item.kind,
        }
    }

    fn initial_name(&self) -> &str {
        match self {
            Self::Edit(item) => &item.title,
            Self::Add(_) | Self::Record(_, _) => "",
        }
    }

    fn initial_fields(&self, at: LifeOccurrence) -> (String, String) {
        let (date, time) = occurrence_fields(at);
        match self {
            Self::Record(_, Some(day)) => (day.to_string(), time),
            _ => (date, time),
        }
    }
}

pub(crate) struct LifeState {
    pub(crate) board: Option<LifeBoard>,
    pub(crate) metric: CheckInMetric,
    pub(crate) load_failed: bool,
    pub(crate) editor_visible: bool,
    pub(crate) edit: Option<LifeEdit>,
    pub(crate) name: Entity<InputState>,
    pub(crate) date: Entity<InputState>,
    pub(crate) time: Entity<InputState>,
    pub(crate) minutes: Entity<InputState>,
    pub(crate) add_kind: LifeKind,
    pub(crate) day_only: bool,
    pub(crate) focus: FocusHandle,
    pub(crate) scroll: ScrollHandle,
    pub(crate) error: Option<&'static str>,
    initial_at: LifeOccurrence,
    generation: u64,
    _subscriptions: Vec<Subscription>,
}

impl LifeState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ForceView>) -> Self {
        cx.bind_keys([
            KeyBinding::new("tab", NextField, Some("LifeEditor")),
            KeyBinding::new("shift-tab", PreviousField, Some("LifeEditor")),
        ]);
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("例如 健身"));
        let date = cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD"));
        let time = cx.new(|cx| InputState::new(window, cx).placeholder("HH:MM:SS"));
        let minutes = cx.new(|cx| InputState::new(window, cx).placeholder("例如 60"));
        let subscriptions = [&name, &date, &time, &minutes]
            .into_iter()
            .map(|field| {
                cx.subscribe_in(field, window, |this, _, event, window, cx| {
                    if !this.life_editor_active() {
                        return;
                    }
                    match event {
                        InputEvent::Change => {
                            this.life.error = None;
                            cx.notify();
                        }
                        InputEvent::PressEnter { .. } => {
                            this.save_life_item(window, cx);
                        }
                        _ => {}
                    }
                })
            })
            .collect();
        Self {
            board: None,
            metric: CheckInMetric::default(),
            load_failed: false,
            editor_visible: false,
            edit: None,
            name,
            date,
            time,
            minutes,
            add_kind: LifeKind::default(),
            focus: cx.focus_handle().tab_stop(false),
            scroll: ScrollHandle::new(),
            error: None,
            initial_at: LifeOccurrence::Exact(0),
            day_only: false,
            generation: 0,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn check_ins_on(
        &self,
        day: NaiveDate,
    ) -> impl Iterator<Item = (&LifeItem, CheckInDay)> {
        self.board
            .iter()
            .filter(|_| !self.load_failed)
            .flat_map(|board| &board.items)
            .filter_map(move |item| {
                let history = item.check_in_days();
                let index = history.binary_search_by_key(&day, |entry| entry.day).ok()?;
                Some((item, history[index]))
            })
    }

    fn changed(&self, cx: &App) -> bool {
        let Some(edit) = &self.edit else {
            return false;
        };
        let (date, time) = edit.initial_fields(self.initial_at);
        self.name.read(cx).value().as_ref() != edit.initial_name()
            || self.date.read(cx).value().as_ref() != date
            || self.day_only != self.initial_at.is_day()
            || self.time.read(cx).value().as_ref() != time
            || !self.minutes.read(cx).value().is_empty()
            || (matches!(edit, LifeEdit::Add(None)) && self.add_kind != LifeKind::default())
    }
}

impl ForceView {
    pub(crate) fn life_editor_active(&self) -> bool {
        self.life.editor_visible && matches!(self.page, Page::Agenda | Page::Analysis)
    }

    pub(crate) fn refresh_life(&mut self, cx: &mut Context<Self>) {
        match self.store.as_ref().map(Store::life_board) {
            Some(Ok(board)) => {
                self.life.board = Some(board);
                self.life.load_failed = false;
            }
            _ => self.life.load_failed = true,
        }
        cx.notify();
    }

    fn write_life<T>(
        &mut self,
        operation: impl FnOnce(&mut Store) -> force_core::Result<T>,
        cx: &mut Context<Self>,
    ) -> Result<T, &'static str> {
        let store = self.store.as_mut().ok_or("本地数据尚未打开，请重试。")?;
        let result = operation(store);
        if matches!(
            &result,
            Ok(_) | Err(Error::InvalidTransition | Error::NotFound)
        ) {
            self.refresh_life(cx);
        }
        result.map_err(life_error)
    }

    pub(crate) fn set_life_module(
        &mut self,
        module: LifeModule,
        visible: bool,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.write_life(|store| store.set_life_module(module, visible), cx) {
            self.notify(error, cx);
        }
    }

    pub(crate) fn select_life_chart_item(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Err(error) = self.write_life(|store| store.set_life_chart_item(id), cx) {
            self.notify(error, cx);
        }
    }

    pub(crate) fn begin_life_change(
        &mut self,
        id: i64,
        mode: impl FnOnce(LifeItem) -> LifeEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let item = self
            .life
            .board
            .as_ref()
            .and_then(|board| board.items.iter().find(|item| item.id == id))
            .cloned();
        if let Some(item) = item {
            self.begin_life_editor(mode(item), window, cx);
        }
    }

    pub(crate) fn begin_life_editor(
        &mut self,
        edit: LifeEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page == Page::Agenda {
            self.suspend_agenda_popup(window, cx);
        }
        if self.life.edit.is_some() {
            self.resume_life_editor(window, cx);
            return;
        }
        let at = match &edit {
            LifeEdit::Edit(item) => item.last_occurrence(),
            LifeEdit::Add(_) | LifeEdit::Record(_, _) => None,
        }
        .unwrap_or_else(|| LifeOccurrence::Exact(Local::now().timestamp()));
        let (date, time) = edit.initial_fields(at);
        self.life.name.update(cx, |input, cx| {
            input.set_value(edit.initial_name().to_owned(), window, cx)
        });
        self.life
            .date
            .update(cx, |input, cx| input.set_value(date, window, cx));
        self.life
            .time
            .update(cx, |input, cx| input.set_value(time, window, cx));
        self.life
            .minutes
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.life.add_kind = LifeKind::default();
        self.life.initial_at = at;
        self.life.day_only = at.is_day();
        self.life.error = None;
        self.life.generation = self.life.generation.wrapping_add(1);
        self.life.edit = Some(edit);
        self.life.scroll.set_offset(point(px(0.), px(0.)));
        self.resume_life_editor(window, cx);
    }

    pub(crate) fn resume_life_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.life.edit.is_some() {
            self.life.editor_visible = true;
            self.focus_life_editor(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn focus_life_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        let input = if matches!(self.life.edit, Some(LifeEdit::Record(_, _))) {
            &self.life.date
        } else {
            &self.life.name
        };
        input.read(cx).focus_handle(cx).focus(window);
    }

    pub(crate) fn cycle_life_focus(
        &self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let initial = window.focused(cx);
        loop {
            if backwards {
                window.focus_prev();
            } else {
                window.focus_next();
            }
            if self.life.focus.contains_focused(window, cx) {
                return;
            }
            if window.focused(cx) == initial {
                break;
            }
        }
        self.focus_life_editor(window, cx);
    }

    pub(crate) fn suspend_life_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.life.editor_visible {
            self.life.editor_visible = false;
            if !self.life.changed(cx) {
                self.life.edit = None;
                self.life.error = None;
            }
            window.focus(&self.focus_handle);
            cx.notify();
        }
    }

    pub(crate) fn cancel_life_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.life.editor_visible = false;
        self.life.edit = None;
        self.life.error = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn save_life_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.life_editor_active()
            || self.compact
            || self.storage_failed
            || self.life.load_failed
        {
            return;
        }
        let Some(edit) = self.life.edit.clone() else {
            return;
        };
        let name = self.life.name.read(cx).value();
        let date = self.life.date.read(cx).value();
        let now = Local::now();
        let check_in = edit.kind(self.life.add_kind) == LifeKind::CheckIn;
        let recorded = matches!(edit, LifeEdit::Record(_, _)) && check_in;
        let check_in_day = match &edit {
            LifeEdit::Add(day) if check_in => *day,
            _ => None,
        };
        let result = if check_in {
            match edit {
                LifeEdit::Add(_) => {
                    self.write_life(|store| store.create_life_habit(&name).map(Some), cx)
                }
                LifeEdit::Edit(item) => self.write_life(
                    |store| store.rename_life_habit(&item, &name).map(|_| None),
                    cx,
                ),
                LifeEdit::Record(item, _) => {
                    let minutes = self.life.minutes.read(cx).value();
                    let today = now.date_naive();
                    crate::moment_model::parse_daily_fields(&date, &minutes, today).and_then(
                        |(day, minutes)| {
                            self.write_life(
                                |store| {
                                    store
                                        .record_life_check_in(&item, day, minutes, today)
                                        .map(|_| None)
                                },
                                cx,
                            )
                        },
                    )
                }
            }
        } else {
            let time = self.life.time.read(cx).value();
            let now = now.timestamp();
            resolve_occurrence(&date, &time, self.life.day_only, self.life.initial_at, now)
                .and_then(|occurred_at| {
                    self.write_life(
                        |store| match edit {
                            LifeEdit::Add(_) => {
                                store.create_life_item(&name, occurred_at, now).map(Some)
                            }
                            LifeEdit::Record(item, _) => store
                                .record_life_occurrence(&item, occurred_at, now)
                                .map(|_| None),
                            LifeEdit::Edit(item) => store
                                .edit_life_item(&item, &name, occurred_at, now)
                                .map(|_| None),
                        },
                        cx,
                    )
                })
        };
        match result {
            Ok(created_id) => {
                self.cancel_life_editor(window, cx);
                if let (Some(id), Some(day)) = (created_id, check_in_day) {
                    self.begin_life_change(
                        id,
                        |item| LifeEdit::Record(item, Some(day)),
                        window,
                        cx,
                    );
                } else if let Some(id) = created_id.filter(|_| self.page == Page::Analysis) {
                    self.select_life_chart_item(id, cx);
                } else if recorded {
                    self.notify("已打卡。", cx);
                }
            }
            Err(error) => {
                self.life.error = Some(error);
                cx.notify();
            }
        }
    }

    pub(crate) fn delete_life_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(LifeEdit::Edit(item)) = &self.life.edit else {
            return;
        };
        let item = item.clone();
        let generation = self.life.generation;
        let answer = window.prompt(
            PromptLevel::Warning,
            "删除这个事项？",
            Some(match item.kind {
                LifeKind::CheckIn => "这项将从看板移除，已有按日记录会保留。",
                LifeKind::Abstain => "这项的发生记录也会删除。",
            }),
            &[PromptButton::cancel("取消"), PromptButton::new("删除")],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let choice = answer.await;
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.life.generation != generation || !this.life.editor_visible {
                        return;
                    }
                    if !this.compact && this.life_editor_active() {
                        window.activate_window();
                        this.focus_life_editor(window, cx);
                    }
                    match choice {
                        Ok(1) => match this.write_life(|store| store.delete_life_item(&item), cx) {
                            Ok(()) => this.cancel_life_editor(window, cx),
                            Err(error) => {
                                this.life.error = Some(error);
                                cx.notify();
                            }
                        },
                        Ok(_) => {}
                        Err(_) => {
                            this.life.error = Some("无法确认删除，请重试。");
                            cx.notify();
                        }
                    }
                });
            });
        })
        .detach();
    }
}

fn occurrence_fields(occurrence: LifeOccurrence) -> (String, String) {
    match occurrence {
        LifeOccurrence::Exact(at) => time_fields(at),
        LifeOccurrence::Day { date, .. } => (date.to_string(), String::new()),
    }
}

fn time_fields(at: i64) -> (String, String) {
    Local.timestamp_opt(at, 0).single().map_or_else(
        || (String::new(), String::new()),
        |time| {
            (
                time.format("%Y-%m-%d").to_string(),
                time.format("%H:%M:%S").to_string(),
            )
        },
    )
}

fn resolve_occurrence(
    date: &str,
    time: &str,
    day_only: bool,
    initial_at: LifeOccurrence,
    now: i64,
) -> Result<LifeOccurrence, &'static str> {
    let (initial_date, initial_time) = occurrence_fields(initial_at);
    if date == initial_date
        && day_only == initial_at.is_day()
        && (day_only || time == initial_time)
        && !date.is_empty()
    {
        return Ok(initial_at);
    }
    let date = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d")
        .map_err(|_| "日期格式为 YYYY-MM-DD。")?;
    if !(1..=9999).contains(&date.year()) {
        return Err("日期年份需在 1–9999 之间。");
    }
    if day_only {
        let (earliest, end) = crate::model::day_bounds(date);
        if earliest >= end {
            return Err("这个本地日期不存在，请调整日期。");
        }
        return Ok(LifeOccurrence::Day {
            date,
            earliest,
            latest: end.min(now),
        });
    }
    let time = NaiveTime::parse_from_str(time.trim(), "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(time.trim(), "%H:%M"))
        .map_err(|_| "时间格式为 HH:MM 或 HH:MM:SS。")?;
    if time.nanosecond() >= 1_000_000_000 {
        return Err("秒数须在 0–59 之间。");
    }
    Local
        .from_local_datetime(&date.and_time(time))
        .single()
        .map(|at| LifeOccurrence::Exact(at.timestamp()))
        .ok_or("这个本地时间不存在或有歧义，请调整时间。")
}

fn life_error(error: Error) -> &'static str {
    match error {
        Error::Validation("Enter an item name of 1–100 characters on one line") => {
            "事项名称须为 1–100 字，并写在一行内。"
        }
        Error::Validation("Choose an occurrence time at or before now") => {
            "发生时间须在现在或之前。"
        }
        Error::Validation("Choose a time after the last occurrence") => "发生时间须晚于上次记录。",
        Error::Validation("Choose a time after the previous occurrence") => {
            "上次时间须晚于它的前一次记录。"
        }
        Error::Validation("This date already has a date-only occurrence") => {
            "该日期已有一笔仅日期记录。"
        }
        Error::Validation("A habit with this name already exists") => "已有同名的打卡事项。",
        Error::InvalidTransition | Error::NotFound => "事项已变更，请取消后重新打开。",
        _ => "保存失败。已有记录已保留，请重试。",
    }
}
