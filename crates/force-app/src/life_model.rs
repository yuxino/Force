use chrono::{Datelike, Local, NaiveDate, NaiveTime, TimeZone, Timelike};
use force_core::{Error, LifeBoard, LifeItem, LifeModule, Store};
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, KeyBinding, PromptButton,
    PromptLevel, ScrollHandle, Subscription, Window, point, px,
};
use gpui_component::input::{InputEvent, InputState};

use crate::model::{ForceView, Page};

gpui::actions!(life_editor, [NextField, PreviousField]);

#[derive(Clone)]
pub(crate) enum LifeEdit {
    Add,
    Record(LifeItem),
    Edit(LifeItem),
}

impl LifeEdit {
    fn initial_name(&self) -> &str {
        match self {
            Self::Edit(item) => &item.title,
            Self::Add | Self::Record(_) => "",
        }
    }
}

pub(crate) struct LifeState {
    pub(crate) board: Option<LifeBoard>,
    pub(crate) load_failed: bool,
    pub(crate) customizing: bool,
    pub(crate) editor_visible: bool,
    pub(crate) edit: Option<LifeEdit>,
    pub(crate) name: Entity<InputState>,
    pub(crate) date: Entity<InputState>,
    pub(crate) time: Entity<InputState>,
    pub(crate) focus: FocusHandle,
    pub(crate) scroll: ScrollHandle,
    pub(crate) error: Option<&'static str>,
    initial_at: i64,
    generation: u64,
    _subscriptions: Vec<Subscription>,
}

impl LifeState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ForceView>) -> Self {
        cx.bind_keys([
            KeyBinding::new("tab", NextField, Some("LifeEditor")),
            KeyBinding::new("shift-tab", PreviousField, Some("LifeEditor")),
        ]);
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("例如 喝咖啡"));
        let date = cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD"));
        let time = cx.new(|cx| InputState::new(window, cx).placeholder("HH:MM:SS"));
        let subscriptions = [&name, &date, &time]
            .into_iter()
            .map(|field| {
                cx.subscribe_in(field, window, |this, _, event, window, cx| {
                    if this.page != Page::Life || !this.life.editor_visible {
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
            load_failed: false,
            customizing: false,
            editor_visible: false,
            edit: None,
            name,
            date,
            time,
            focus: cx.focus_handle().tab_stop(false),
            scroll: ScrollHandle::new(),
            error: None,
            initial_at: 0,
            generation: 0,
            _subscriptions: subscriptions,
        }
    }

    fn changed(&self, cx: &App) -> bool {
        let Some(edit) = &self.edit else {
            return false;
        };
        let (date, time) = time_fields(self.initial_at);
        self.name.read(cx).value().as_ref() != edit.initial_name()
            || self.date.read(cx).value().as_ref() != date
            || self.time.read(cx).value().as_ref() != time
    }
}

impl ForceView {
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
        mode: fn(LifeItem) -> LifeEdit,
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
        if self.life.edit.is_some() {
            self.resume_life_editor(window, cx);
            return;
        }
        let at = match &edit {
            LifeEdit::Edit(item) => item.last_occurred_at(),
            LifeEdit::Add | LifeEdit::Record(_) => Local::now().timestamp(),
        };
        let (date, time) = time_fields(at);
        self.life.name.update(cx, |input, cx| {
            input.set_value(edit.initial_name().to_owned(), window, cx)
        });
        self.life
            .date
            .update(cx, |input, cx| input.set_value(date, window, cx));
        self.life
            .time
            .update(cx, |input, cx| input.set_value(time, window, cx));
        self.life.initial_at = at;
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
        let input = if matches!(self.life.edit, Some(LifeEdit::Record(_))) {
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
        if self.page != Page::Life
            || self.compact
            || self.storage_failed
            || self.life.load_failed
            || !self.life.editor_visible
        {
            return;
        }
        let Some(edit) = &self.life.edit else { return };
        let name = self.life.name.read(cx).value();
        let date = self.life.date.read(cx).value();
        let time = self.life.time.read(cx).value();
        let occurred_at = match resolve_time(&date, &time, self.life.initial_at) {
            Ok(at) => at,
            Err(error) => {
                self.life.error = Some(error);
                cx.notify();
                return;
            }
        };
        let now = Local::now().timestamp();
        let edit = edit.clone();
        match self.write_life(
            |store| match edit {
                LifeEdit::Add => store.create_life_item(&name, occurred_at, now).map(|_| ()),
                LifeEdit::Record(item) => store.record_life_occurrence(&item, occurred_at, now),
                LifeEdit::Edit(item) => store.edit_life_item(&item, &name, occurred_at, now),
            },
            cx,
        ) {
            Ok(()) => self.cancel_life_editor(window, cx),
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
            Some("这项的发生记录也会删除。"),
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
                    if !this.compact && this.page == Page::Life {
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

fn resolve_time(date: &str, time: &str, initial_at: i64) -> Result<i64, &'static str> {
    let (initial_date, initial_time) = time_fields(initial_at);
    if date == initial_date && time == initial_time && !date.is_empty() {
        return Ok(initial_at);
    }
    let date = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d")
        .map_err(|_| "日期格式为 YYYY-MM-DD。")?;
    if !(1..=9999).contains(&date.year()) {
        return Err("日期年份需在 1–9999 之间。");
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
        .map(|at| at.timestamp())
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
        Error::InvalidTransition | Error::NotFound => "事项已变更，请取消后重新打开。",
        _ => "保存失败。已有记录已保留，请重试。",
    }
}
