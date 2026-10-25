//! Calendar selection and drafts are independent from the actual timer lifecycle.
use std::cell::Cell;

use chrono::{Datelike, Days, Local, Months, NaiveDate, Timelike};
use force_core::{PlanDraft, PlanEntry, Session, Stats, TagKind, Task};
use gpui::{
    AppContext as _, Context, Entity, Focusable as _, PromptButton, PromptLevel, ScrollHandle,
    Subscription, Window,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    model::{ForceView, Page, TaskDestination, day_bounds},
    tag_picker::{FocusTagOption, TagPicked, TagPicker},
};

#[derive(Clone, Eq, PartialEq)]
struct PlanInputSnapshot {
    title: String,
    day: String,
    start: String,
    end: String,
    note: String,
    task: Option<i64>,
    tag: i64,
}

pub(crate) struct AgendaState {
    pub(crate) date: NaiveDate,
    pub(crate) month: NaiveDate,
    pub(crate) plans: Vec<PlanEntry>,
    pub(crate) planned_days: Vec<NaiveDate>,
    pub(crate) actual_days: Vec<NaiveDate>,
    pub(crate) records: Vec<Session>,
    pub(crate) completed_tasks: Vec<Task>,
    pub(crate) stats: Stats,
    pub(crate) selected_plan: Option<i64>,
    pub(crate) selected_record: Option<i64>,
    pub(crate) selected_completion: Option<i64>,
    pub(crate) editing: bool,
    pub(crate) edit_id: Option<i64>,
    pub(crate) draft_task: Option<i64>,
    pub(crate) draft_tag: i64,
    pub(crate) title_input: Entity<InputState>,
    pub(crate) date_input: Entity<InputState>,
    pub(crate) start_input: Entity<InputState>,
    pub(crate) end_input: Entity<InputState>,
    pub(crate) note_input: Entity<InputState>,
    pub(crate) tag_picker: Entity<TagPicker>,
    pub(crate) error: Option<String>,
    pub(crate) calendar_scroll: ScrollHandle,
    pub(crate) inspector_scroll: ScrollHandle,
    pub(crate) editor_scroll: ScrollHandle,
    pub(crate) calendar_scroll_target: Cell<Option<u16>>,
    baseline: Option<PlanInputSnapshot>,
    original_plan: Option<PlanEntry>,
    generation: u64,
    tag_options: Vec<FocusTagOption>,
    _subscriptions: Vec<Subscription>,
}

impl AgendaState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ForceView>) -> Self {
        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("这段时间做什么？"));
        let date_input = cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD"));
        let start_input = cx.new(|cx| InputState::new(window, cx).placeholder("09:00"));
        let end_input = cx.new(|cx| InputState::new(window, cx).placeholder("10:00"));
        let note_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("备注（可选）")
        });
        let tag_picker = cx.new(TagPicker::new);
        let mut subscriptions = Vec::new();
        for input in [&title_input, &date_input, &start_input, &end_input] {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.agenda.error = None;
                        cx.notify();
                    } else if matches!(event, InputEvent::PressEnter { secondary: false })
                        && this.page == Page::Agenda
                        && this.agenda.editing
                        && !this.compact
                    {
                        this.save_plan(window, cx);
                    }
                },
            ));
        }
        subscriptions.push(
            cx.subscribe(&note_input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.agenda.error = None;
                    cx.notify();
                }
            }),
        );
        subscriptions.push(cx.subscribe(&tag_picker, |this, _, event: &TagPicked, cx| {
            if this.agenda.draft_task.is_none()
                && this
                    .tags
                    .iter()
                    .any(|tag| tag.id == event.0 && tag.kind == TagKind::Focus)
            {
                this.agenda.draft_tag = event.0;
                this.agenda.error = None;
                cx.notify();
            }
        }));
        let date = Local::now().date_naive();
        Self {
            date,
            month: date.with_day(1).unwrap_or(date),
            plans: Vec::new(),
            planned_days: Vec::new(),
            actual_days: Vec::new(),
            records: Vec::new(),
            completed_tasks: Vec::new(),
            stats: Stats::default(),
            selected_plan: None,
            selected_record: None,
            selected_completion: None,
            editing: false,
            edit_id: None,
            draft_task: None,
            draft_tag: 0,
            title_input,
            date_input,
            start_input,
            end_input,
            note_input,
            tag_picker,
            error: None,
            calendar_scroll: ScrollHandle::default(),
            inspector_scroll: ScrollHandle::default(),
            editor_scroll: ScrollHandle::default(),
            baseline: None,
            original_plan: None,
            calendar_scroll_target: Cell::new(Some(
                Local::now().hour().saturating_sub(1) as u16 * 60,
            )),
            generation: 0,
            tag_options: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    fn snapshot(&self, cx: &gpui::App) -> PlanInputSnapshot {
        PlanInputSnapshot {
            title: self.title_input.read(cx).value().trim().to_owned(),
            day: self.date_input.read(cx).value().trim().to_owned(),
            start: self.start_input.read(cx).value().trim().to_owned(),
            end: self.end_input.read(cx).value().trim().to_owned(),
            note: self.note_input.read(cx).value().trim().to_owned(),
            task: self.draft_task,
            tag: self.draft_tag,
        }
    }
}

#[derive(Clone, Copy)]
enum AgendaDestination {
    Date(NaiveDate),
    Month(NaiveDate),
    New(Option<i64>),
    Plan(i64),
    Edit(i64),
    Record(i64),
    Completion(i64),
    Start(i64),
}

impl ForceView {
    pub(crate) fn refresh_agenda(&mut self, cx: &mut Context<Self>) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let (start, end) = day_bounds(self.agenda.date);
        let visible_dates = agenda_visible_dates(self.agenda.month);
        let range_first = visible_dates.first().copied().unwrap_or(self.agenda.month);
        let range_last = visible_dates.last().copied().unwrap_or(self.agenda.month);
        let range_end = range_last.succ_opt().unwrap_or(range_last);
        let visible_start = day_bounds(range_first).0;
        let visible_end = day_bounds(range_last).1;
        let snapshot = (|| -> force_core::Result<_> {
            let mut records = store.sessions_overlapping(start, end, self.now)?;
            merge_active_records(&mut records, &self.active, &[self.agenda.date], self.now);
            // One bounded visible-month batch, never one query per cell. The
            // temporary records are reduced to dates before entering UI state.
            let mut month_records =
                store.sessions_overlapping(visible_start, visible_end, self.now)?;
            merge_active_records(&mut month_records, &self.active, &visible_dates, self.now);
            let month_completions =
                store.completed_tasks_between(visible_start, visible_end, self.now)?;
            let actual_days =
                agenda_actual_dates(&visible_dates, &month_records, &month_completions, self.now);
            Ok((
                store.plans_for_day(self.agenda.date)?,
                store.plan_days_between(range_first, range_end)?,
                actual_days,
                records,
                store.completed_tasks_between(start, end, self.now)?,
                store.stats(start, end, self.now)?,
            ))
        })();
        match snapshot {
            Ok((plans, planned_days, actual_days, records, completed_tasks, stats)) => {
                self.agenda.plans = plans;
                self.agenda.planned_days = planned_days;
                self.agenda.actual_days = actual_days;
                self.agenda.records = records;
                self.agenda.completed_tasks = completed_tasks;
                self.agenda.stats = stats;
                if self
                    .agenda
                    .selected_plan
                    .is_some_and(|id| !self.agenda.plans.iter().any(|plan| plan.id == id))
                {
                    self.agenda.selected_plan = None;
                }
                if self
                    .agenda
                    .selected_record
                    .is_some_and(|id| !self.agenda.records.iter().any(|record| record.id == id))
                {
                    self.agenda.selected_record = None;
                }
                if self
                    .agenda
                    .selected_completion
                    .is_some_and(|id| !self.agenda.completed_tasks.iter().any(|task| task.id == id))
                {
                    self.agenda.selected_completion = None;
                }
            }
            Err(_) => {
                self.storage_failed = true;
                self.notify("读取日历失败。已有内容已保留，请重试。", cx);
            }
        }
        cx.notify();
    }

    /// Existing open intervals already expand against `self.now`; a timer tick
    /// needs no new calendar/history query or stable plan reload.
    pub(crate) fn refresh_agenda_actuals(&mut self, cx: &mut Context<Self>) {
        let today = Local::now().date_naive();
        if self.agenda.date == today {
            self.agenda.stats = self.today;
        }
        merge_active_records(
            &mut self.agenda.records,
            &self.active,
            &[self.agenda.date],
            self.now,
        );
        if agenda_visible_dates(self.agenda.month).contains(&today)
            && !self.agenda.actual_days.contains(&today)
            && !agenda_actual_dates(&[today], &self.active, &[], self.now).is_empty()
        {
            self.agenda.actual_days.push(today);
            self.agenda.actual_days.sort_unstable();
        }
        cx.notify();
    }

    pub(crate) fn sync_agenda_tag_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            .find(|tag| tag.id == self.agenda.draft_tag)
            .map(|tag| tag.id);
        let enabled = self.page == Page::Agenda
            && self.agenda.editing
            && self.agenda.draft_task.is_none()
            && !self.compact
            && !self.storage_failed;
        let picker = self.agenda.tag_picker.read(cx);
        if self.agenda.tag_options != options
            || picker.selected_value() != selected
            || picker.is_enabled() != enabled
        {
            self.agenda.tag_options = options.clone();
            self.agenda.tag_picker.update(cx, |picker, cx| {
                picker.synchronize(options, selected, enabled, window, cx)
            });
        }
    }

    pub(crate) fn agenda_has_unsaved_changes(&self, cx: &gpui::App) -> bool {
        self.agenda.editing && self.agenda.baseline.as_ref() != Some(&self.agenda.snapshot(cx))
    }

    fn confirm_agenda_changes(
        &mut self,
        destination: AgendaDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.agenda_has_unsaved_changes(cx) {
            return false;
        }
        let generation = self.agenda.generation;
        let return_focus = window.focused(cx).filter(|_| {
            self.agenda.draft_task.is_none()
                || !self
                    .agenda
                    .title_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
        });
        let answer = window.prompt(
            PromptLevel::Info,
            "保存安排修改？",
            None,
            &[
                PromptButton::cancel("取消"),
                PromptButton::new("不保存"),
                PromptButton::new("保存"),
            ],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(choice) = answer.await {
                let _ = cx.update(|window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if !this.agenda.editing || this.agenda.generation != generation {
                            return;
                        }
                        if choice == 0 {
                            if let Some(focus) = &return_focus {
                                window.focus(focus);
                            } else {
                                this.focus_agenda_editor(window, cx);
                            }
                            return;
                        }
                        if choice == 2 {
                            this.save_plan(window, cx);
                            if this.agenda.editing {
                                return;
                            }
                        } else {
                            this.cancel_plan_edit(window, cx);
                        }
                        match destination {
                            AgendaDestination::Date(day) => {
                                this.select_agenda_date(day, window, cx)
                            }
                            AgendaDestination::Month(month) => this.select_agenda_month(month, cx),
                            AgendaDestination::New(task) => this.begin_plan(task, window, cx),
                            AgendaDestination::Plan(id) => this.select_plan(id, window, cx),
                            AgendaDestination::Edit(id) => this.edit_plan(id, window, cx),
                            AgendaDestination::Record(id) => {
                                this.select_agenda_record(id, window, cx)
                            }
                            AgendaDestination::Completion(id) => {
                                this.select_agenda_completion(id, window, cx)
                            }
                            AgendaDestination::Start(id) => this.start_plan_timer(id, window, cx),
                        }
                    });
                });
            }
        })
        .detach();
        true
    }

    pub(crate) fn select_agenda_date(
        &mut self,
        day: NaiveDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !(1..=9999).contains(&day.year()) || day == self.agenda.date {
            return;
        }
        if self.confirm_agenda_changes(AgendaDestination::Date(day), window, cx) {
            return;
        }
        self.agenda.date = day;
        self.agenda
            .calendar_scroll_target
            .set(Some(if day == Local::now().date_naive() {
                Local::now().hour().saturating_sub(1) as u16 * 60
            } else {
                8 * 60
            }));
        self.agenda.month = day.with_day(1).unwrap_or(day);
        self.clear_agenda_selection();
        self.agenda.editing = false;
        self.agenda
            .calendar_scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
        self.toast = None;
        self.refresh_agenda(cx);
        if day != Local::now().date_naive()
            && let Some(first) = self
                .agenda
                .plans
                .iter()
                .filter(|plan| plan.moved_to.is_none())
                .min_by_key(|plan| plan.start_minute)
        {
            self.agenda
                .calendar_scroll_target
                .set(Some(first.start_minute.saturating_sub(60)));
        }
    }

    pub(crate) fn change_agenda_month(
        &mut self,
        delta: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let month = if delta >= 0 {
            self.agenda
                .month
                .checked_add_months(Months::new(delta.unsigned_abs()))
        } else {
            self.agenda
                .month
                .checked_sub_months(Months::new(delta.unsigned_abs()))
        };
        let Some(month) = month.filter(|month| (1..=9999).contains(&month.year())) else {
            return;
        };
        if self.confirm_agenda_changes(AgendaDestination::Month(month), window, cx) {
            return;
        }
        self.select_agenda_month(month, cx);
    }

    fn select_agenda_month(&mut self, month: NaiveDate, cx: &mut Context<Self>) {
        self.agenda.month = month;
        self.agenda.editing = false;
        self.refresh_agenda(cx);
    }

    pub(crate) fn begin_plan_at(&mut self, hour: u16, window: &mut Window, cx: &mut Context<Self>) {
        if self.agenda.editing || hour > 23 {
            return;
        }
        self.begin_plan(None, window, cx);
        if self.agenda.editing {
            self.agenda.start_input.update(cx, |input, cx| {
                input.set_value(format!("{hour:02}:00"), window, cx)
            });
            self.agenda.end_input.update(cx, |input, cx| {
                input.set_value(format!("{:02}:00", hour + 1), window, cx)
            });
            self.agenda.baseline = Some(self.agenda.snapshot(cx));
        }
    }

    pub(crate) fn begin_plan(
        &mut self,
        task_id: Option<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.agenda.editing && self.agenda.edit_id.is_none() && self.agenda.draft_task == task_id
        {
            self.focus_agenda_editor(window, cx);
            return;
        }
        if self.confirm_agenda_changes(AgendaDestination::New(task_id), window, cx) {
            return;
        }
        let task = task_id
            .and_then(|id| {
                self.tasks.iter().find(|task| {
                    task.id == id && task.completed_at.is_none() && task.tag_kind == TagKind::Focus
                })
            })
            .cloned();
        if task_id.is_some() && task.is_none() {
            self.notify("这件事已完成或已移除，请重新选择。", cx);
            return;
        }
        let tag = task
            .as_ref()
            .map(|task| task.tag_id)
            .or_else(|| {
                self.tags
                    .iter()
                    .find(|tag| tag.id == self.selected_tag && tag.kind == TagKind::Focus)
                    .map(|tag| tag.id)
            })
            .or_else(|| {
                self.tags
                    .iter()
                    .find(|tag| tag.kind == TagKind::Focus)
                    .map(|tag| tag.id)
            })
            .unwrap_or(0);
        let current = Local::now();
        let start = if self.agenda.date == current.date_naive() {
            ((current.hour() * 60 + current.minute()).div_ceil(15) * 15).min(23 * 60) as u16
        } else {
            9 * 60
        };
        self.agenda.editing = true;
        self.agenda.edit_id = None;
        self.agenda.original_plan = None;
        self.agenda.draft_task = task_id;
        self.agenda.draft_tag = tag;
        self.agenda.error = None;
        self.agenda.generation = self.agenda.generation.wrapping_add(1);
        self.clear_agenda_selection();
        let snapshot = PlanInputSnapshot {
            title: task
                .as_ref()
                .map_or_else(String::new, |task| task.title.clone()),
            day: self.agenda.date.format("%Y-%m-%d").to_string(),
            start: agenda_clock(start),
            end: agenda_clock(start + 30),
            note: String::new(),
            task: task_id,
            tag,
        };
        self.set_plan_inputs(&snapshot, window, cx);
        self.agenda.baseline = Some(self.agenda.snapshot(cx));
        self.agenda
            .editor_scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
        self.focus_agenda_editor(window, cx);
        cx.notify();
    }

    pub(crate) fn edit_plan(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if self.agenda.editing && self.agenda.edit_id == Some(id) {
            return;
        }
        if self.confirm_agenda_changes(AgendaDestination::Edit(id), window, cx) {
            return;
        }
        let Some(plan) = self
            .agenda
            .plans
            .iter()
            .find(|plan| plan.id == id && plan.moved_to.is_none())
            .cloned()
        else {
            return;
        };
        self.agenda.editing = true;
        self.agenda.edit_id = Some(id);
        self.agenda.original_plan = Some(plan.clone());
        self.agenda.draft_task = Some(plan.task_id);
        self.agenda.draft_tag = self
            .tasks
            .iter()
            .find(|task| task.id == plan.task_id)
            .map_or(0, |task| task.tag_id);
        self.agenda.error = None;
        self.agenda.generation = self.agenda.generation.wrapping_add(1);
        let snapshot = PlanInputSnapshot {
            title: plan.title.clone(),
            day: plan.day.format("%Y-%m-%d").to_string(),
            start: agenda_clock(plan.start_minute),
            end: agenda_clock(plan.end_minute),
            note: plan.note.clone(),
            task: Some(plan.task_id),
            tag: self.agenda.draft_tag,
        };
        self.set_plan_inputs(&snapshot, window, cx);
        self.agenda.baseline = Some(self.agenda.snapshot(cx));
        self.agenda
            .editor_scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
        self.agenda
            .start_input
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    fn focus_agenda_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        let input = if self.agenda.draft_task.is_some() {
            &self.agenda.start_input
        } else {
            &self.agenda.title_input
        };
        input.update(cx, |input, cx| input.focus(window, cx));
    }

    fn set_plan_inputs(
        &mut self,
        snapshot: &PlanInputSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (input, value) in [
            (&self.agenda.title_input, snapshot.title.clone()),
            (&self.agenda.date_input, snapshot.day.clone()),
            (&self.agenda.start_input, snapshot.start.clone()),
            (&self.agenda.end_input, snapshot.end.clone()),
            (&self.agenda.note_input, snapshot.note.clone()),
        ] {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
    }

    pub(crate) fn agenda_draft_error(&self, cx: &gpui::App) -> Option<String> {
        let draft = self.agenda.snapshot(cx);
        validate_plan_fields(&draft)
            .err()
            .map(str::to_owned)
            .or_else(|| {
                (draft.task.is_none()
                    && !self
                        .tags
                        .iter()
                        .any(|tag| tag.id == draft.tag && tag.kind == TagKind::Focus))
                .then(|| "请先在设置中添加可用的计时标签。".to_owned())
            })
    }

    pub(crate) fn save_plan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.agenda.editing {
            return;
        }
        if let Some(error) = self.agenda_draft_error(cx) {
            self.agenda.error = Some(error);
            cx.notify();
            return;
        }
        let snapshot = self.agenda.snapshot(cx);
        let Ok((day, start_minute, end_minute)) = validate_plan_fields(&snapshot) else {
            return;
        };
        let draft = PlanDraft {
            task_id: snapshot.task,
            title: snapshot.title,
            tag_id: snapshot.tag,
            day,
            start_minute,
            end_minute,
            note: snapshot.note,
        };
        let edit_id = self.agenda.edit_id;
        let original = self.agenda.original_plan.clone();
        let mut conflict = false;
        let now = Local::now().timestamp();
        let mut saved = None;
        if self.write(
            |store| {
                saved = if let Some(id) = edit_id {
                    let Some(original) = &original else {
                        return Err(force_core::Error::InvalidTransition);
                    };
                    let updated = store.update_plan_if_unchanged(id, &draft, original)?;
                    conflict = updated.is_none();
                    updated
                } else {
                    Some(store.create_plan(&draft, now)?)
                };
                Ok(())
            },
            cx,
        ) && let Some(plan) = saved
        {
            self.agenda.editing = false;
            self.agenda.baseline = None;
            self.agenda.edit_id = None;
            self.agenda.error = None;
            self.agenda.date = plan.day;
            self.agenda.month = plan.day.with_day(1).unwrap_or(plan.day);
            self.clear_agenda_selection();
            self.agenda.selected_plan = Some(plan.id);
            self.agenda
                .calendar_scroll_target
                .set(Some(plan.start_minute.saturating_sub(60)));
            self.agenda
                .tag_picker
                .update(cx, |picker, cx| picker.dismiss(window, false, cx));
            self.refresh_agenda(cx);
            self.notify("安排已保存。", cx);
        } else if conflict {
            self.agenda.error = Some("这项安排已在别处更改，请取消后重新打开。".to_owned());
            cx.notify();
        }
    }

    pub(crate) fn cancel_plan_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.agenda.editing = false;
        self.agenda.edit_id = None;
        self.agenda.baseline = None;
        self.agenda.original_plan = None;
        self.agenda.error = None;
        self.agenda.generation = self.agenda.generation.wrapping_add(1);
        self.agenda
            .tag_picker
            .update(cx, |picker, cx| picker.dismiss(window, false, cx));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn clear_agenda_selection(&mut self) {
        self.agenda.selected_plan = None;
        self.agenda.selected_record = None;
        self.agenda.selected_completion = None;
    }

    pub(crate) fn select_plan(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirm_agenda_changes(AgendaDestination::Plan(id), window, cx) {
            return;
        }
        if !self.agenda.plans.iter().any(|plan| plan.id == id) {
            return;
        }
        self.agenda.editing = false;
        self.clear_agenda_selection();
        self.agenda.selected_plan = Some(id);
        if let Some(plan) = self.agenda.plans.iter().find(|plan| plan.id == id) {
            self.agenda
                .calendar_scroll_target
                .set(Some(plan.start_minute.saturating_sub(60)));
        }
        self.agenda.error = None;
        cx.notify();
    }

    pub(crate) fn select_agenda_record(
        &mut self,
        id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.confirm_agenda_changes(AgendaDestination::Record(id), window, cx) {
            return;
        }
        if !self.agenda.records.iter().any(|record| record.id == id) {
            return;
        }
        self.agenda.editing = false;
        self.clear_agenda_selection();
        self.agenda.selected_record = Some(id);
        self.agenda.error = None;
        cx.notify();
    }

    pub(crate) fn select_agenda_completion(
        &mut self,
        id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.confirm_agenda_changes(AgendaDestination::Completion(id), window, cx) {
            return;
        }
        if !self.agenda.completed_tasks.iter().any(|task| task.id == id) {
            return;
        }
        self.agenda.editing = false;
        self.clear_agenda_selection();
        self.agenda.selected_completion = Some(id);
        self.agenda.error = None;
        cx.notify();
    }

    pub(crate) fn agenda_plan_is_done(&self, plan: &PlanEntry) -> bool {
        plan.completed_at.is_some()
            || self
                .agenda
                .completed_tasks
                .iter()
                .any(|task| task.id == plan.task_id)
    }

    pub(crate) fn toggle_plan_done(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(plan) = self.agenda.plans.iter().find(|plan| plan.id == id).cloned() else {
            return;
        };
        if plan.moved_to.is_some() {
            return;
        }
        if self
            .agenda
            .completed_tasks
            .iter()
            .any(|task| task.id == plan.task_id)
        {
            self.notify("对应任务已完成，日历会保留这项实际记录。", cx);
            return;
        }
        self.write(
            |store| {
                store.set_plan_completed(id, plan.completed_at.is_none(), Local::now().timestamp())
            },
            cx,
        );
    }

    pub(crate) fn move_plan_tomorrow(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(plan) = self.agenda.plans.iter().find(|plan| plan.id == id).cloned() else {
            return;
        };
        if self.agenda_plan_is_done(&plan) {
            self.notify("已完成的安排会保留在原日期。", cx);
            return;
        }
        let Some(day) = plan
            .day
            .checked_add_days(Days::new(1))
            .filter(|day| day.year() <= 9999)
        else {
            return;
        };
        if self.write(
            |store| store.postpone_plan(id, day, Local::now().timestamp()),
            cx,
        ) {
            self.notify("已移到明天，今天保留顺延记录。", cx);
        }
    }

    pub(crate) fn delete_plan(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if !self.agenda.plans.iter().any(|plan| plan.id == id) {
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            "删除这次安排？",
            Some("任务和实际计时记录会保留。"),
            &[PromptButton::cancel("取消"), PromptButton::new("删除")],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(1) = answer.await {
                let _ = cx.update(|_, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if this.write(|store| store.delete_plan(id), cx) {
                            if this.agenda.selected_plan == Some(id) {
                                this.agenda.selected_plan = None;
                            }
                            this.notify("安排已删除，实际记录保留。", cx);
                        }
                    });
                });
            }
        })
        .detach();
    }

    pub(crate) fn start_plan_timer(
        &mut self,
        id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.confirm_agenda_changes(AgendaDestination::Start(id), window, cx) {
            return;
        }
        if self.confirm_task_changes(TaskDestination::AgendaPlan(id), window, cx) {
            return;
        }
        let Some(plan) = self.agenda.plans.iter().find(|plan| plan.id == id).cloned() else {
            return;
        };
        if plan.day != Local::now().date_naive() {
            self.notify("请切换到今天的安排后开始计时。", cx);
            return;
        }
        if plan.moved_to.is_some() || self.agenda_plan_is_done(&plan) {
            self.notify("请先恢复这次安排，再开始计时。", cx);
            return;
        }
        if self
            .tasks
            .iter()
            .find(|task| task.id == plan.task_id)
            .is_none_or(|task| task.completed_at.is_some())
        {
            self.notify("对应任务已完成或已移除，请添加新的安排。", cx);
            return;
        }
        self.select_task(plan.task_id, window, cx);
        if self.selected_task != Some(plan.task_id) || self.task_has_unsaved_changes(cx) {
            return;
        }
        if let Some(session) = self
            .active
            .iter()
            .find(|session| session.task_id == Some(plan.task_id))
            .cloned()
        {
            if session.is_paused() {
                self.toggle_pause(session.id, cx);
            }
            self.select_session(session.id, window, cx);
        } else {
            self.choose_timer_preset(u32::from(plan.end_minute - plan.start_minute), window, cx);
            self.start(window, cx);
        }
        self.page = Page::Agenda;
        self.refresh_agenda(cx);
        cx.notify();
    }

    /// Attribute one day's task total only once even if the task has several
    /// arrangements. Actual interval timestamps remain available in Records.
    pub(crate) fn agenda_plan_actuals(&self, plan_id: i64) -> Vec<&Session> {
        let Some(plan) = self
            .agenda
            .plans
            .iter()
            .find(|plan| plan.id == plan_id && plan.moved_to.is_none())
        else {
            return Vec::new();
        };
        if self
            .agenda
            .plans
            .iter()
            .filter(|candidate| candidate.task_id == plan.task_id && candidate.moved_to.is_none())
            .min_by_key(|candidate| (candidate.start_minute, candidate.id))
            .is_none_or(|first| first.id != plan_id)
        {
            return Vec::new();
        }
        self.agenda
            .records
            .iter()
            .filter(|record| record.task_id == Some(plan.task_id))
            .collect()
    }

    pub(crate) fn agenda_unplanned_completions(&self) -> Vec<&Task> {
        self.agenda
            .completed_tasks
            .iter()
            .filter(|task| {
                !self
                    .agenda
                    .plans
                    .iter()
                    .any(|plan| plan.task_id == task.id && plan.moved_to.is_none())
                    && !self
                        .agenda
                        .records
                        .iter()
                        .any(|record| record.task_id == Some(task.id))
            })
            .collect()
    }
}

/// The month picker has six Monday-to-Sunday rows, including adjacent dates.
/// Clamp the query at the supported storage years; the final exclusive bound
/// may be 10000-01-01, which the core range API accepts as a sentinel.
fn agenda_visible_dates(month: NaiveDate) -> Vec<NaiveDate> {
    let first = month
        .checked_sub_days(Days::new(u64::from(month.weekday().num_days_from_monday())))
        .unwrap_or(month);
    (0..42)
        .filter_map(|offset| first.checked_add_days(Days::new(offset)))
        .filter(|day| (1..=9999).contains(&day.year()))
        .collect()
}

fn record_touches_day(record: &Session, day: NaiveDate, now: i64) -> bool {
    let (start, end) = day_bounds(day);
    let contains = |stamp| stamp >= start && stamp < end && stamp <= now;
    record.elapsed_between(start, end, now) > 0
        || contains(record.started_at)
        || record.finished_at.is_some_and(contains)
        || record.intervals.iter().any(|interval| {
            contains(interval.start)
                || interval.end.is_none()
                    && interval.start <= now
                    && interval.start < end
                    && now >= start
                    && now < end
        })
}

fn merge_active_records(
    records: &mut Vec<Session>,
    active: &[Session],
    dates: &[NaiveDate],
    now: i64,
) {
    for record in active {
        if !records.iter().any(|existing| existing.id == record.id)
            && dates
                .iter()
                .any(|day| record_touches_day(record, *day, now))
        {
            records.push(record.clone());
        }
    }
}

fn agenda_actual_dates(
    dates: &[NaiveDate],
    records: &[Session],
    completed: &[Task],
    now: i64,
) -> Vec<NaiveDate> {
    dates
        .iter()
        .copied()
        .filter(|day| {
            let (start, end) = day_bounds(*day);
            records
                .iter()
                .any(|record| record_touches_day(record, *day, now))
                || completed.iter().any(|task| {
                    task.completed_at
                        .is_some_and(|stamp| stamp >= start && stamp < end && stamp <= now)
                })
        })
        .collect()
}

pub(crate) fn agenda_clock(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

fn parse_plan_time(value: &str, allow_midnight_end: bool) -> Option<u16> {
    let (hour, minute) = value.trim().split_once(':')?;
    if hour.is_empty()
        || hour.len() > 2
        || minute.len() != 2
        || !hour
            .bytes()
            .chain(minute.bytes())
            .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let hour = hour.parse::<u16>().ok()?;
    let minute = minute.parse::<u16>().ok()?;
    if minute >= 60 || hour > 24 || hour == 24 && (!allow_midnight_end || minute != 0) {
        return None;
    }
    Some(hour * 60 + minute)
}

fn validate_plan_fields(draft: &PlanInputSnapshot) -> Result<(NaiveDate, u16, u16), &'static str> {
    if draft.title.is_empty()
        || draft.title.chars().count() > 200
        || draft.title.chars().any(char::is_control)
    {
        return Err("请输入 1–200 字的事情名称。");
    }
    let day = NaiveDate::parse_from_str(&draft.day, "%Y-%m-%d")
        .ok()
        .filter(|day| (1..=9999).contains(&day.year()))
        .ok_or("日期格式应为 YYYY-MM-DD。");
    let start = parse_plan_time(&draft.start, false)
        .ok_or("开始时间格式应为 09:00，范围为 00:00–23:59。")?;
    let end = parse_plan_time(&draft.end, true).ok_or("结束时间格式应为 10:00，最晚为 24:00。")?;
    if end <= start {
        return Err("结束时间须晚于开始时间。");
    }
    if draft.note.chars().count() > 2000
        || draft
            .note
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err("备注最多 2000 字，且不能包含无效字符。");
    }
    Ok((day?, start, end))
}

#[cfg(test)]
mod tests {
    use super::{
        PlanInputSnapshot, agenda_actual_dates, agenda_clock, agenda_visible_dates,
        merge_active_records, parse_plan_time, validate_plan_fields,
    };
    use crate::model::day_bounds;
    use chrono::{Datelike, Days, NaiveDate};
    use force_core::{Interval, Session, TagKind, Task};
    fn draft() -> PlanInputSnapshot {
        PlanInputSnapshot {
            title: "Read".into(),
            day: "2026-10-25".into(),
            start: "09:00".into(),
            end: "10:00".into(),
            note: String::new(),
            task: None,
            tag: 1,
        }
    }
    #[test]
    fn calendar_time_validation_accepts_end_midnight_without_wrapping_a_start() {
        assert_eq!(parse_plan_time("9:00", false), Some(540));
        assert_eq!(parse_plan_time("24:00", true), Some(1440));
        for value in ["24:00", "24:01", "9:0", "-1:30", "12:60", ""] {
            assert_eq!(parse_plan_time(value, false), None);
        }
        assert_eq!(agenda_clock(1440), "24:00");
    }
    #[test]
    fn calendar_form_rejects_invalid_dates_ranges_and_notes_without_normalizing_them() {
        assert!(validate_plan_fields(&draft()).is_ok());
        for (start, end) in [("10:00", "09:00"), ("09:00", "09:00"), ("23:59", "24:01")] {
            let mut d = draft();
            d.start = start.into();
            d.end = end.into();
            assert!(validate_plan_fields(&d).is_err());
        }
        let mut d = draft();
        d.day = "2026-02-30".into();
        assert!(validate_plan_fields(&d).is_err());
        let mut d = draft();
        d.note = "x".repeat(2001);
        assert!(validate_plan_fields(&d).is_err());
        let mut d = draft();
        d.start = "23:00".into();
        d.end = "24:00".into();
        assert!(validate_plan_fields(&d).is_ok());
    }
    #[test]
    fn month_presence_uses_real_intervals_not_a_paused_gap_and_includes_untimed_completion() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let previous = day.checked_sub_days(Days::new(1)).unwrap();
        let next = day.checked_add_days(Days::new(1)).unwrap();
        let left = day_bounds(previous).0;
        let middle = day_bounds(day).0;
        let right = day_bounds(next).0;
        let now = right + 120;
        let record = Session {
            id: 1,
            task_id: Some(1),
            title: "Paused across day".into(),
            tag_id: 1,
            tag_name: "Work".into(),
            tag_kind: TagKind::Focus,
            started_at: left + 60,
            finished_at: Some(right + 60),
            target_seconds: None,
            intervals: vec![
                Interval {
                    start: left + 60,
                    end: Some(left + 120),
                },
                Interval {
                    start: right,
                    end: Some(right + 60),
                },
            ],
        };
        let dates = vec![previous, day, next];
        assert_eq!(
            agenda_actual_dates(&dates, std::slice::from_ref(&record), &[], now),
            vec![previous, next]
        );
        let untimed = Task {
            id: 2,
            title: "Untimed".into(),
            tag_id: 1,
            tag_name: "Work".into(),
            tag_kind: TagKind::Focus,
            created_at: middle,
            completed_at: Some(middle + 30),
            total_seconds: 0,
            session_count: 0,
        };
        assert_eq!(
            agenda_actual_dates(&dates, &[record], &[untimed], now),
            dates
        );
        let future = Session {
            id: 3,
            task_id: None,
            title: "Future clock".into(),
            tag_id: 1,
            tag_name: "Work".into(),
            tag_kind: TagKind::Focus,
            started_at: right + 300,
            finished_at: Some(right + 300),
            target_seconds: None,
            intervals: vec![],
        };
        assert!(agenda_actual_dates(&[next], &[future], &[], now).is_empty());
    }

    #[test]
    fn midnight_open_interval_is_visible_without_waiting_for_another_database_query() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let midnight = day_bounds(day).0;
        let running = Session {
            id: 1,
            task_id: Some(1),
            title: "Across midnight".into(),
            tag_id: 1,
            tag_name: "Work".into(),
            tag_kind: TagKind::Focus,
            started_at: midnight - 60,
            finished_at: None,
            target_seconds: None,
            intervals: vec![Interval {
                start: midnight - 60,
                end: None,
            }],
        };
        let mut records = Vec::new();
        merge_active_records(
            &mut records,
            std::slice::from_ref(&running),
            &[day],
            midnight,
        );
        merge_active_records(
            &mut records,
            std::slice::from_ref(&running),
            &[day],
            midnight,
        );
        assert_eq!(records, vec![running]);
        assert_eq!(
            agenda_actual_dates(&[day], &records, &[], midnight),
            vec![day]
        );
    }

    #[test]
    fn visible_calendar_range_covers_overflow_dates_and_storage_year_boundaries() {
        let month = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let dates = agenda_visible_dates(month);
        assert_eq!(dates.len(), 42);
        assert!(dates[0] < month);
        assert!(dates.last().unwrap().month() > month.month());
        for month in [
            NaiveDate::from_ymd_opt(1, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(9999, 12, 1).unwrap(),
        ] {
            let dates = agenda_visible_dates(month);
            assert!(!dates.is_empty());
            assert!(dates.iter().all(|date| (1..=9999).contains(&date.year())));
            assert!(dates.len() <= 42);
        }
    }

    #[test]
    fn calendar_fields_reject_control_characters_but_preserve_multiline_notes() {
        let mut d = draft();
        d.title = "Read\nmore".into();
        assert!(validate_plan_fields(&d).is_err());
        let mut d = draft();
        d.note = "First line\nSecond\tline".into();
        assert!(validate_plan_fields(&d).is_ok());
        d.note.push('\u{1b}');
        assert!(validate_plan_fields(&d).is_err());
    }
}
