//! Calendar selection and drafts are independent from the actual timer lifecycle.
use std::{cell::Cell, collections::BTreeMap};

use chrono::{Datelike, Days, Local, Months, NaiveDate, TimeZone, Timelike};
use force_core::{Moment, PlanDraft, PlanEntry, Session, Stats, TagKind, Task};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Focusable as _, Pixels, Point, PromptButton,
    PromptLevel, ScrollHandle, Subscription, Window,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    model::{ForceView, Page, TaskDestination, day_bounds},
    tag_picker::{FocusTagOption, TagPicked, TagPicker},
};

gpui::actions!(agenda_editor, [NextField, PreviousField]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlanField {
    Title,
    Date,
    Start,
    End,
    Tag,
    Note,
}

#[derive(Clone, Copy, Debug)]
struct PlanError {
    field: PlanField,
    message: &'static str,
}

impl PlanError {
    fn new(field: PlanField, message: &'static str) -> Self {
        Self { field, message }
    }
}

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

/// A display state derived from existing task, occurrence and timer data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgendaItemStatus {
    Running,
    Paused,
    Completed,
    Recorded,
    Pending,
    Moved,
    Moment,
}

impl AgendaItemStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Running => "计时",
            Self::Paused => "暂停",
            Self::Completed => "完成",
            Self::Recorded => "记录",
            Self::Pending => "待办",
            Self::Moved => "顺延",
            Self::Moment => "随记",
        }
    }

    pub(crate) fn description(self, planned: bool) -> &'static str {
        match self {
            Self::Running => "计时中",
            Self::Paused => "已暂停",
            Self::Completed if planned => "本次安排已完成",
            Self::Completed => "任务已完成",
            Self::Recorded => "已记录",
            Self::Pending => "待办安排",
            Self::Moved => "已顺延，原安排保留",
            Self::Moment => "时刻记录",
        }
    }
}

fn record_status(record: &Session, day: NaiveDate, now: i64) -> AgendaItemStatus {
    let (start, end) = day_bounds(day);
    if now >= start && now < end {
        if record.is_running() {
            return AgendaItemStatus::Running;
        }
        if record.is_paused() {
            return AgendaItemStatus::Paused;
        }
    }
    AgendaItemStatus::Recorded
}

/// One real task (or unassociated legacy round) shown directly in a month day.
/// Planned duration is never treated as actual investment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgendaMonthItem {
    pub(crate) title: String,
    pub(crate) tag_id: Option<i64>,
    pub(crate) tag_color: Option<String>,
    pub(crate) tag_class: force_core::TagClass,
    pub(crate) status: AgendaItemStatus,
    pub(crate) seconds: i64,
    pub(crate) planned: bool,
    pub(crate) completed: bool,
    pub(crate) rest: bool,
    pub(crate) moved: bool,
}

pub(crate) struct AgendaState {
    pub(crate) date: NaiveDate,
    pub(crate) month: NaiveDate,
    pub(crate) month_overview: bool,
    /// Day details or a centered editor above the unchanged month calendar.
    pub(crate) popup_open: bool,
    pub(crate) calendar_focus: FocusHandle,
    pub(crate) editor_focus: FocusHandle,
    /// `editing` retains a draft even while this editor is suspended.
    pub(crate) editor_visible: bool,
    pub(crate) note_expanded: bool,
    pub(crate) month_days: BTreeMap<NaiveDate, Vec<AgendaMonthItem>>,
    pub(crate) plans: Vec<PlanEntry>,
    pub(crate) planned_days: Vec<NaiveDate>,
    pub(crate) actual_days: Vec<NaiveDate>,
    pub(crate) records: Vec<Session>,
    pub(crate) day_moments: Vec<Moment>,
    pub(crate) month_moments: Vec<Moment>,
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
    note_return_scroll: Option<Point<Pixels>>,
    note_return_focus: Option<FocusHandle>,
    resume_note_focused: bool,
    pub(crate) calendar_scroll_target: Cell<Option<u16>>,
    month_records: Vec<Session>,
    month_completions: Vec<Task>,
    month_plans: Vec<PlanEntry>,
    baseline: Option<PlanInputSnapshot>,
    original_plan: Option<PlanEntry>,
    /// Stable editor origin; changing the target date never changes draft identity.
    draft_origin_date: Option<NaiveDate>,
    generation: u64,
    tag_options: Vec<FocusTagOption>,
    _subscriptions: Vec<Subscription>,
}

impl AgendaState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ForceView>) -> Self {
        cx.bind_keys([
            gpui::KeyBinding::new("tab", NextField, Some("AgendaEditor")),
            gpui::KeyBinding::new("shift-tab", PreviousField, Some("AgendaEditor")),
        ]);
        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("输入安排名称"));
        let date_input = cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD"));
        let start_input = cx.new(|cx| InputState::new(window, cx).placeholder("09:00"));
        let end_input = cx.new(|cx| InputState::new(window, cx).placeholder("10:00"));
        let note_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("备注（可选）")
        });
        let tag_picker = cx.new(|cx| {
            let mut picker = TagPicker::new(cx);
            picker.set_trigger_width(322., cx);
            picker
        });
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
                        && this.agenda.editor_visible
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
            if this.page == Page::Agenda
                && this.agenda.editing
                && this.agenda.editor_visible
                && !this.compact
                && !this.storage_failed
                && this.agenda_activity_editable()
                && this.agenda_activity_allowed(event.0)
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
            month_overview: true,
            popup_open: false,
            calendar_focus: cx.focus_handle().tab_stop(false),
            editor_focus: cx.focus_handle().tab_stop(false),
            editor_visible: false,
            note_expanded: false,
            month_days: BTreeMap::new(),
            plans: Vec::new(),
            planned_days: Vec::new(),
            actual_days: Vec::new(),
            records: Vec::new(),
            day_moments: Vec::new(),
            month_moments: Vec::new(),
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
            note_return_scroll: None,
            note_return_focus: None,
            resume_note_focused: false,
            month_records: Vec::new(),
            month_completions: Vec::new(),
            month_plans: Vec::new(),
            baseline: None,
            original_plan: None,
            draft_origin_date: None,
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
    Today,
    New { task: Option<i64>, day: NaiveDate },
    NewAt { hour: u16, day: NaiveDate },
    Edit { id: i64, day: NaiveDate },
    Start { id: i64, day: NaiveDate },
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
            // Keep one bounded visible-month snapshot for both the overview
            // and query-free live timer updates; never query per calendar cell.
            let mut month_records =
                store.sessions_overlapping(visible_start, visible_end, self.now)?;
            merge_active_records(&mut month_records, &self.active, &visible_dates, self.now);
            let month_completions =
                store.completed_tasks_between(visible_start, visible_end, self.now)?;
            let month_moments = store.moments_between(visible_start, visible_end)?;
            let mut actual_days =
                agenda_actual_dates(&visible_dates, &month_records, &month_completions, self.now);
            actual_days.extend(month_moments.iter().filter_map(moment_date));
            actual_days.sort_unstable();
            actual_days.dedup();
            let month_plans = store.plans_between(range_first, range_end)?;
            let mut planned_days: Vec<_> = month_plans.iter().map(|plan| plan.day).collect();
            planned_days.sort_unstable();
            planned_days.dedup();
            Ok((
                store.plans_for_day(self.agenda.date)?,
                planned_days,
                actual_days,
                records,
                store.completed_tasks_between(start, end, self.now)?,
                store.stats(start, end, self.now)?,
                month_records,
                month_completions,
                month_plans,
                store.moments_between(start, end)?,
                month_moments,
            ))
        })();
        match snapshot {
            Ok((
                plans,
                planned_days,
                actual_days,
                records,
                completed_tasks,
                stats,
                month_records,
                month_completions,
                month_plans,
                day_moments,
                month_moments,
            )) => {
                self.agenda.month_days = agenda_month_days(
                    &visible_dates,
                    &month_records,
                    &month_completions,
                    &month_plans,
                    &self.tasks,
                    &self.active,
                    self.now,
                );
                merge_month_moments(&mut self.agenda.month_days, &month_moments);
                self.agenda.day_moments = day_moments;
                self.agenda.month_moments = month_moments;
                self.agenda.month_records = month_records;
                self.agenda.month_completions = month_completions;
                self.agenda.month_plans = month_plans;
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
        let visible_dates = agenda_visible_dates(self.agenda.month);
        merge_active_records(
            &mut self.agenda.month_records,
            &self.active,
            &visible_dates,
            self.now,
        );
        self.agenda.month_days = agenda_month_days(
            &visible_dates,
            &self.agenda.month_records,
            &self.agenda.month_completions,
            &self.agenda.month_plans,
            &self.tasks,
            &self.active,
            self.now,
        );
        merge_month_moments(&mut self.agenda.month_days, &self.agenda.month_moments);
        if visible_dates.contains(&today)
            && !self.agenda.actual_days.contains(&today)
            && !agenda_actual_dates(&[today], &self.active, &[], self.now).is_empty()
        {
            self.agenda.actual_days.push(today);
            self.agenda.actual_days.sort_unstable();
        }
        cx.notify();
    }

    pub(crate) fn sync_agenda_tag_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut options: Vec<_> = self
            .tags
            .iter()
            .filter(|tag| tag.kind == TagKind::Focus)
            .map(|tag| FocusTagOption {
                id: tag.id,
                name: tag.name.clone(),
                color: self.display_tag_color(tag).map(str::to_owned),
            })
            .collect();
        // An archived original activity still describes this task. Keep it
        // visible while allowing the user to replace it with an available one.
        if let Some(task) = self
            .agenda
            .draft_task
            .and_then(|id| self.tasks.iter().find(|task| task.id == id))
            && !options.iter().any(|option| option.id == task.tag_id)
        {
            options.push(FocusTagOption {
                id: task.tag_id,
                name: task.tag_name.clone(),
                color: None,
            });
        }
        self.agenda.tag_picker.update(cx, |picker, cx| {
            picker.set_trigger_width(crate::editor_dialog::content_width(window), cx);
        });
        let selected = options
            .iter()
            .find(|tag| tag.id == self.agenda.draft_tag)
            .map(|tag| tag.id);
        let enabled = self.page == Page::Agenda
            && self.agenda.editing
            && self.agenda.editor_visible
            && self.agenda_activity_editable()
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
                            if this.page != Page::Agenda {
                                window.focus(&this.focus_handle);
                            } else if !this.agenda.editor_visible {
                                window.focus(&this.agenda.calendar_focus);
                            } else if let Some(focus) = &return_focus {
                                window.focus(focus);
                            } else {
                                this.focus_agenda_editor(window, cx);
                            }
                            return;
                        }
                        if choice == 2 {
                            this.save_plan(window, cx);
                            if this.agenda.editing {
                                // A failed save must reveal the retained draft
                                // and its validation/storage error, even if the
                                // requested action came from another page.
                                this.resume_agenda_draft(window, cx);
                                return;
                            }
                        } else {
                            this.cancel_plan_edit(window, cx);
                        }
                        match destination {
                            AgendaDestination::Today => this.begin_today_plan(window, cx),
                            AgendaDestination::New { task, day } => {
                                this.select_agenda_date(day, window, cx);
                                this.begin_plan(task, window, cx);
                            }
                            AgendaDestination::NewAt { hour, day } => {
                                this.select_agenda_date(day, window, cx);
                                this.begin_plan_at(hour, window, cx);
                            }
                            AgendaDestination::Edit { id, day } => {
                                this.select_agenda_date(day, window, cx);
                                this.edit_plan(id, window, cx);
                            }
                            AgendaDestination::Start { id, day } => {
                                this.select_agenda_date(day, window, cx);
                                this.start_plan_timer(id, window, cx);
                            }
                        }
                    });
                });
            }
        })
        .detach();
        true
    }

    /// Closing a contextual view never commits a plan or loses a changed draft.
    pub(crate) fn suspend_agenda_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.time_picker
            .update(cx, |picker, cx| picker.dismiss(window, cx));
        if self.agenda.editing && !self.agenda_has_unsaved_changes(cx) {
            self.cancel_plan_edit(window, cx);
            return;
        }
        if self.agenda.editing && self.agenda.editor_visible {
            self.agenda.resume_note_focused = self.agenda.note_expanded
                && self
                    .agenda
                    .note_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window);
        }
        self.agenda.popup_open = false;
        self.agenda.editor_visible = false;
        self.agenda
            .tag_picker
            .update(cx, |picker, cx| picker.dismiss(window, false, cx));
        window.focus(&self.agenda.calendar_focus);
        cx.notify();
    }

    pub(crate) fn agenda_has_suspended_draft(&self, cx: &gpui::App) -> bool {
        self.agenda.editing && !self.agenda.editor_visible && self.agenda_has_unsaved_changes(cx)
    }

    pub(crate) fn resume_agenda_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_task_dialog(window, cx);
        self.suspend_moment(window, cx);
        self.suspend_record_editor(window, cx);
        self.suspend_completion_editor(window, cx);
        if !self.agenda.editing {
            return;
        }
        let origin = self.agenda.draft_origin_date.unwrap_or(self.agenda.date);
        self.agenda.date = origin;
        self.agenda.month = origin.with_day(1).unwrap_or(origin);
        self.agenda.month_overview = true;
        self.agenda.popup_open = true;
        self.agenda.editor_visible = true;
        self.page = Page::Agenda;
        self.clear_agenda_selection();
        self.refresh_agenda(cx);
        if self.agenda.note_expanded && self.agenda.resume_note_focused {
            self.agenda.editor_scroll.scroll_to_bottom();
            self.agenda
                .note_input
                .update(cx, |input, cx| input.focus(window, cx));
        } else {
            self.agenda
                .editor_scroll
                .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
            self.focus_agenda_editor(window, cx);
        }
        self.agenda.resume_note_focused = false;
        cx.notify();
    }

    /// Month selection opens a small summary rather than replacing the calendar.
    pub(crate) fn open_agenda_day(
        &mut self,
        day: NaiveDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !(1..=9999).contains(&day.year()) {
            return;
        }
        self.suspend_agenda_popup(window, cx);
        self.agenda.month_overview = true;
        self.select_agenda_date(day, window, cx);
        self.clear_agenda_selection();
        self.agenda.popup_open = true;
        self.agenda.editor_visible = false;
        window.focus(&self.agenda.calendar_focus);
        cx.notify();
    }

    pub(crate) fn open_agenda_timeline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_agenda_popup(window, cx);
        self.agenda.month_overview = false;
        self.agenda.popup_open = false;
        self.page = Page::Agenda;
        window.focus(&self.focus_handle);
        self.agenda.calendar_scroll_target.set(Some(
            if self.agenda.date == Local::now().date_naive() {
                Local::now().hour().saturating_sub(1) as u16 * 60
            } else {
                8 * 60
            },
        ));
        cx.notify();
    }

    /// Every calendar-home entry suspends an edit instead of discarding it.
    pub(crate) fn prepare_agenda_overview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.suspend_agenda_popup(window, cx);
        self.agenda.month_overview = true;
        self.clear_agenda_selection();
        true
    }

    pub(crate) fn select_agenda_date(
        &mut self,
        day: NaiveDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !(1..=9999).contains(&day.year()) {
            return;
        }
        self.suspend_agenda_popup(window, cx);
        if day == self.agenda.date {
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
        if !self.agenda.month_overview || !agenda_visible_dates(self.agenda.month).contains(&day) {
            self.agenda.month = day.with_day(1).unwrap_or(day);
        }
        self.clear_agenda_selection();
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
        self.suspend_agenda_popup(window, cx);
        self.select_agenda_month(month, cx);
    }

    fn select_agenda_month(&mut self, month: NaiveDate, cx: &mut Context<Self>) {
        self.agenda.month = month;
        self.agenda.popup_open = false;
        self.agenda.editor_visible = false;
        self.refresh_agenda(cx);
    }

    pub(crate) fn begin_plan_at(&mut self, hour: u16, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_record_editor(window, cx);
        self.suspend_completion_editor(window, cx);
        if self.agenda.editor_visible || hour > 23 {
            return;
        }
        if self.confirm_agenda_changes(
            AgendaDestination::NewAt {
                hour,
                day: self.agenda.date,
            },
            window,
            cx,
        ) {
            return;
        }
        if self.agenda.editing {
            self.cancel_plan_edit(window, cx);
        }
        self.begin_plan(None, window, cx);
        if self.agenda.editor_visible {
            self.agenda.start_input.update(cx, |input, cx| {
                input.set_value(format!("{hour:02}:00"), window, cx)
            });
            self.agenda.end_input.update(cx, |input, cx| {
                input.set_value(format!("{:02}:00", hour + 1), window, cx)
            });
            self.agenda.baseline = Some(self.agenda.snapshot(cx));
        }
    }

    /// The month toolbar is a general entry point, independent of the day
    /// last inspected. Explicit day/timeline actions still call begin_plan.
    pub(crate) fn begin_today_plan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let today = Local::now().date_naive();
        if self.agenda.editing
            && self.agenda.edit_id.is_none()
            && self.agenda.draft_task.is_none()
            && self.agenda.draft_origin_date == Some(today)
        {
            self.resume_agenda_draft(window, cx);
            return;
        }
        if self.confirm_agenda_changes(AgendaDestination::Today, window, cx) {
            return;
        }
        self.select_agenda_date(today, window, cx);
        self.select_agenda_month(today.with_day(1).unwrap_or(today), cx);
        self.begin_plan(None, window, cx);
    }

    pub(crate) fn begin_plan(
        &mut self,
        task_id: Option<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.suspend_task_dialog(window, cx);
        self.suspend_moment(window, cx);
        self.suspend_record_editor(window, cx);
        self.suspend_completion_editor(window, cx);
        if self.agenda.editing
            && self.agenda.edit_id.is_none()
            && self.agenda.draft_task == task_id
            && self.agenda.draft_origin_date == Some(self.agenda.date)
        {
            self.resume_agenda_draft(window, cx);
            return;
        }
        if self.confirm_agenda_changes(
            AgendaDestination::New {
                task: task_id,
                day: self.agenda.date,
            },
            window,
            cx,
        ) {
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
        self.agenda.editor_visible = true;
        self.agenda.popup_open = true;
        self.agenda.month_overview = true;
        self.agenda.note_expanded = false;
        self.agenda.note_return_scroll = None;
        self.agenda.note_return_focus = None;
        self.agenda.resume_note_focused = false;
        self.agenda.draft_origin_date = Some(self.agenda.date);
        self.page = Page::Agenda;
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
        self.suspend_task_dialog(window, cx);
        self.suspend_moment(window, cx);
        self.suspend_record_editor(window, cx);
        self.suspend_completion_editor(window, cx);
        if self.agenda.editing && self.agenda.edit_id == Some(id) {
            self.resume_agenda_draft(window, cx);
            return;
        }
        if self.confirm_agenda_changes(
            AgendaDestination::Edit {
                id,
                day: self.agenda.date,
            },
            window,
            cx,
        ) {
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
        self.agenda.editor_visible = true;
        self.agenda.popup_open = true;
        self.agenda.month_overview = true;
        self.agenda.note_expanded = !plan.note.is_empty();
        self.agenda.note_return_scroll = None;
        self.agenda.note_return_focus = None;
        self.agenda.resume_note_focused = false;
        self.agenda.draft_origin_date = Some(self.agenda.date);
        self.page = Page::Agenda;
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

    pub(crate) fn cycle_agenda_editor_focus(
        &self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let first = window.focused(cx);
        loop {
            if backwards {
                window.focus_prev();
            } else {
                window.focus_next();
            }
            if self.agenda.editor_focus.contains_focused(window, cx) {
                return;
            }
            if window.focused(cx) == first {
                break;
            }
        }
        self.focus_agenda_editor(window, cx);
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

    pub(crate) fn agenda_activity_editable(&self) -> bool {
        self.agenda.draft_task.is_none_or(|id| {
            self.tasks
                .iter()
                .any(|task| task.id == id && task.completed_at.is_none())
        })
    }

    fn agenda_activity_allowed(&self, tag_id: i64) -> bool {
        self.tags
            .iter()
            .any(|tag| tag.id == tag_id && tag.kind == TagKind::Focus)
            || self
                .agenda
                .draft_task
                .and_then(|id| self.tasks.iter().find(|task| task.id == id))
                .is_some_and(|task| task.tag_id == tag_id && task.tag_kind == TagKind::Focus)
    }

    fn agenda_draft_error(&self, cx: &gpui::App) -> Option<PlanError> {
        let draft = self.agenda.snapshot(cx);
        validate_plan_fields(&draft).err().or_else(|| {
            (!self.agenda_activity_allowed(draft.tag))
                .then(|| PlanError::new(PlanField::Tag, "请先在设置中添加可用的活动。"))
        })
    }

    pub(crate) fn toggle_agenda_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.agenda.note_expanded {
            self.agenda.note_expanded = false;
            if let Some(offset) = self.agenda.note_return_scroll.take() {
                self.agenda.editor_scroll.set_offset(offset);
            }
            let note_focused = self
                .agenda
                .note_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);
            if note_focused {
                if let Some(focus) = self.agenda.note_return_focus.take() {
                    focus.focus(window);
                } else {
                    window.focus(&self.agenda.calendar_focus);
                }
            } else {
                self.agenda.note_return_focus = None;
            }
        } else {
            self.agenda.note_return_scroll = Some(self.agenda.editor_scroll.offset());
            self.agenda.note_return_focus = window.focused(cx);
            self.agenda.note_expanded = true;
            // Unlike scroll_to_item, scroll_to_bottom uses the new viewport and
            // content size in the same prepaint, even when the popup moves.
            self.agenda.editor_scroll.scroll_to_bottom();
            self.agenda
                .note_input
                .update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    fn reveal_agenda_error(
        &mut self,
        field: PlanField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (item, input) = match field {
            PlanField::Title => (0, Some(&self.agenda.title_input)),
            PlanField::Date => (1, Some(&self.agenda.date_input)),
            PlanField::Start => (2, Some(&self.agenda.start_input)),
            PlanField::End => (2, Some(&self.agenda.end_input)),
            PlanField::Tag => (3, None),
            PlanField::Note => {
                if !self.agenda.note_expanded {
                    self.agenda.note_return_scroll = Some(self.agenda.editor_scroll.offset());
                    self.agenda.note_return_focus = window.focused(cx);
                    self.agenda.note_expanded = true;
                }
                self.agenda.editor_scroll.scroll_to_bottom();
                (5, Some(&self.agenda.note_input))
            }
        };
        if field != PlanField::Note {
            self.agenda.editor_scroll.scroll_to_item(item);
        }
        if let Some(input) = input {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
    }

    pub(crate) fn save_plan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.agenda.editing {
            return;
        }
        if let Some(error) = self.agenda_draft_error(cx) {
            self.agenda.error = Some(error.message.to_owned());
            self.reveal_agenda_error(error.field, window, cx);
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
        let expected_tag = self.agenda.baseline.as_ref().map(|snapshot| snapshot.tag);
        // A pristine setup for this same task should follow the committed
        // activity; a user-edited task draft remains independent and intact.
        let sync_task_setup = self.preparing
            && draft.task_id.is_some()
            && self.selected_task == draft.task_id
            && !self.task_has_unsaved_changes(cx);
        let mut conflict = false;
        let now = Local::now().timestamp();
        let mut saved = None;
        if self.write(
            |store| {
                saved = if let Some(id) = edit_id {
                    let Some(original) = &original else {
                        return Err(force_core::Error::InvalidTransition);
                    };
                    let expected_tag = expected_tag.ok_or(force_core::Error::InvalidTransition)?;
                    store.update_plan_with_activity_if_unchanged(
                        id,
                        &draft,
                        original,
                        expected_tag,
                    )?
                } else {
                    store.create_plan_with_activity(&draft, now, expected_tag)?
                };
                conflict = saved.is_none();
                Ok(())
            },
            cx,
        ) && let Some(plan) = saved
        {
            if sync_task_setup {
                self.reset_task_changes(window, cx);
            }
            self.agenda.editing = false;
            self.agenda.editor_visible = false;
            self.agenda.popup_open = false;
            self.agenda.month_overview = true;
            self.agenda.note_expanded = false;
            self.agenda.draft_origin_date = None;
            self.agenda.original_plan = None;
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
            window.focus(&self.agenda.calendar_focus);
            self.notify("安排已保存。", cx);
        } else if conflict {
            self.agenda.error = Some("这项安排或活动已在别处更改，请取消后重新打开。".to_owned());
            cx.notify();
        }
    }

    pub(crate) fn cancel_plan_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.time_picker
            .update(cx, |picker, cx| picker.dismiss(window, cx));
        self.agenda.editing = false;
        self.agenda.editor_visible = false;
        self.agenda.popup_open = false;
        self.agenda.note_expanded = false;
        self.agenda.draft_origin_date = None;
        self.agenda.edit_id = None;
        self.agenda.baseline = None;
        self.agenda.original_plan = None;
        self.agenda.error = None;
        self.agenda.generation = self.agenda.generation.wrapping_add(1);
        self.agenda
            .tag_picker
            .update(cx, |picker, cx| picker.dismiss(window, false, cx));
        window.focus(&self.agenda.calendar_focus);
        cx.notify();
    }

    fn clear_agenda_selection(&mut self) {
        self.agenda.selected_plan = None;
        self.agenda.selected_record = None;
        self.agenda.selected_completion = None;
    }

    pub(crate) fn select_plan(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if !self.agenda.plans.iter().any(|plan| plan.id == id) {
            return;
        }
        self.suspend_agenda_popup(window, cx);
        self.clear_agenda_selection();
        self.agenda.popup_open = self.agenda.month_overview;
        self.agenda.selected_plan = Some(id);
        if let Some(plan) = self.agenda.plans.iter().find(|plan| plan.id == id) {
            self.agenda
                .calendar_scroll_target
                .set(Some(plan.start_minute.saturating_sub(60)));
        }
        if !self.agenda.editing {
            self.agenda.error = None;
        }
        cx.notify();
    }

    pub(crate) fn select_agenda_record(
        &mut self,
        id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.agenda.records.iter().any(|record| record.id == id) {
            return;
        }
        self.suspend_agenda_popup(window, cx);
        self.clear_agenda_selection();
        self.agenda.popup_open = self.agenda.month_overview;
        self.agenda.selected_record = Some(id);
        if !self.agenda.editing {
            self.agenda.error = None;
        }
        cx.notify();
    }

    pub(crate) fn select_agenda_completion(
        &mut self,
        id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.agenda.completed_tasks.iter().any(|task| task.id == id) {
            return;
        }
        self.suspend_agenda_popup(window, cx);
        self.clear_agenda_selection();
        self.agenda.popup_open = self.agenda.month_overview;
        self.agenda.selected_completion = Some(id);
        if !self.agenda.editing {
            self.agenda.error = None;
        }
        cx.notify();
    }

    pub(crate) fn agenda_task_tag_id(&self, task_id: i64) -> Option<i64> {
        self.tasks
            .iter()
            .chain(&self.agenda.completed_tasks)
            .find(|task| task.id == task_id)
            .map(|task| task.tag_id)
            .or_else(|| {
                self.agenda
                    .records
                    .iter()
                    .chain(&self.active)
                    .find(|record| record.task_id == Some(task_id))
                    .map(|record| record.tag_id)
            })
    }

    pub(crate) fn agenda_task_color(&self, task_id: i64) -> Option<&str> {
        let record = self
            .agenda
            .records
            .iter()
            .chain(&self.active)
            .find(|record| record.task_id == Some(task_id));
        self.display_color(
            self.agenda_task_tag_id(task_id),
            record.map_or(force_core::TagClass::Legacy, |record| record.tag_class),
            record.and_then(|record| record.tag_color.as_deref()),
        )
    }

    pub(crate) fn agenda_record_status(&self, record: &Session) -> AgendaItemStatus {
        if record
            .task_id
            .is_some_and(|id| self.agenda.completed_tasks.iter().any(|task| task.id == id))
        {
            AgendaItemStatus::Completed
        } else {
            record_status(record, self.agenda.date, self.now)
        }
    }

    pub(crate) fn agenda_plan_status(&self, plan: &PlanEntry) -> AgendaItemStatus {
        if plan.moved_to.is_some() {
            AgendaItemStatus::Moved
        } else if self.agenda_plan_is_done(plan) {
            AgendaItemStatus::Completed
        } else {
            self.active
                .iter()
                .find(|record| record.task_id == Some(plan.task_id))
                .map(|record| record_status(record, self.agenda.date, self.now))
                .filter(|status| {
                    matches!(status, AgendaItemStatus::Running | AgendaItemStatus::Paused)
                })
                .unwrap_or(AgendaItemStatus::Pending)
        }
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
        if self.confirm_agenda_changes(
            AgendaDestination::Start {
                id,
                day: self.agenda.date,
            },
            window,
            cx,
        ) {
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

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum MonthItemIdentity {
    Task(i64),
    Session(i64),
    MovedPlan(i64),
}

fn agenda_month_days(
    dates: &[NaiveDate],
    records: &[Session],
    completed: &[Task],
    plans: &[PlanEntry],
    tasks: &[Task],
    active: &[Session],
    now: i64,
) -> BTreeMap<NaiveDate, Vec<AgendaMonthItem>> {
    dates
        .iter()
        .map(|day| {
            let (start, end) = day_bounds(*day);
            let mut items = BTreeMap::<MonthItemIdentity, AgendaMonthItem>::new();
            for record in records
                .iter()
                .filter(|record| record_touches_day(record, *day, now))
            {
                let identity = record
                    .task_id
                    .map(MonthItemIdentity::Task)
                    .unwrap_or(MonthItemIdentity::Session(record.id));
                let item = items.entry(identity).or_insert_with(|| AgendaMonthItem {
                    title: record.title.clone(),
                    tag_id: Some(record.tag_id),
                    tag_color: record.tag_color.clone(),
                    tag_class: record.tag_class,
                    status: AgendaItemStatus::Recorded,
                    seconds: 0,
                    planned: false,
                    completed: false,
                    rest: record.tag_kind == TagKind::Rest,
                    moved: false,
                });
                let status = record_status(record, *day, now);
                if status == AgendaItemStatus::Running
                    || status == AgendaItemStatus::Paused
                        && item.status != AgendaItemStatus::Running
                {
                    item.status = status;
                }
                item.seconds = item
                    .seconds
                    .saturating_add(record.elapsed_between(start, end, now));
            }
            for task in completed.iter().filter(|task| {
                task.completed_at
                    .is_some_and(|stamp| stamp >= start && stamp < end && stamp <= now)
            }) {
                let item = items
                    .entry(MonthItemIdentity::Task(task.id))
                    .or_insert_with(|| AgendaMonthItem {
                        title: task.title.clone(),
                        tag_id: Some(task.tag_id),
                        tag_color: None,
                        tag_class: force_core::TagClass::Legacy,
                        status: AgendaItemStatus::Completed,
                        seconds: 0,
                        planned: false,
                        completed: false,
                        rest: task.tag_kind == TagKind::Rest,
                        moved: false,
                    });
                item.completed = true;
                item.status = AgendaItemStatus::Completed;
            }
            for plan in plans.iter().filter(|plan| plan.day == *day) {
                let moved = plan.moved_to.is_some();
                let identity = if moved {
                    MonthItemIdentity::MovedPlan(plan.id)
                } else {
                    MonthItemIdentity::Task(plan.task_id)
                };
                items
                    .entry(identity)
                    .and_modify(|item| {
                        // An occurrence check never completes an actual task.
                        // A combined plan stays pending until every occurrence
                        // is complete; moved markers retain their own identity.
                        if item.planned {
                            item.completed &= plan.completed_at.is_some();
                        }
                    })
                    .or_insert_with(|| AgendaMonthItem {
                        title: plan.title.clone(),
                        tag_id: tasks
                            .iter()
                            .chain(completed)
                            .find(|task| task.id == plan.task_id)
                            .map(|task| task.tag_id)
                            .or_else(|| {
                                records
                                    .iter()
                                    .chain(active)
                                    .find(|record| record.task_id == Some(plan.task_id))
                                    .map(|record| record.tag_id)
                            }),
                        tag_class: records
                            .iter()
                            .chain(active)
                            .find(|record| record.task_id == Some(plan.task_id))
                            .map_or(force_core::TagClass::Legacy, |record| record.tag_class),
                        tag_color: records
                            .iter()
                            .chain(active)
                            .find(|record| record.task_id == Some(plan.task_id))
                            .and_then(|record| record.tag_color.clone()),
                        status: records
                            .iter()
                            .chain(active)
                            .filter(|record| record.task_id == Some(plan.task_id))
                            .map(|record| record_status(record, *day, now))
                            .find(|status| {
                                matches!(
                                    status,
                                    AgendaItemStatus::Running | AgendaItemStatus::Paused
                                )
                            })
                            .unwrap_or(AgendaItemStatus::Pending),
                        seconds: 0,
                        planned: true,
                        completed: plan.completed_at.is_some(),
                        rest: false,
                        moved,
                    });
            }
            let mut items: Vec<_> = items.into_values().collect();
            for item in &mut items {
                if item.moved {
                    item.status = AgendaItemStatus::Moved;
                } else if item.planned && item.completed {
                    item.status = AgendaItemStatus::Completed;
                }
            }
            // Actual work remains first even when a task has no timer minutes.
            // Within it, the most substantial work is immediately visible.
            items.sort_by(|left, right| {
                (left.planned, left.moved)
                    .cmp(&(right.planned, right.moved))
                    .then_with(|| right.seconds.cmp(&left.seconds))
                    .then_with(|| left.title.cmp(&right.title))
            });
            (*day, items)
        })
        .collect()
}

fn moment_date(moment: &Moment) -> Option<NaiveDate> {
    Local
        .timestamp_opt(moment.occurred_at, 0)
        .single()
        .map(|time| time.date_naive())
}

/// Point notes occupy a row, never a duration or an invented timer session.
fn merge_month_moments(days: &mut BTreeMap<NaiveDate, Vec<AgendaMonthItem>>, moments: &[Moment]) {
    for moment in moments {
        let Some(time) = Local.timestamp_opt(moment.occurred_at, 0).single() else {
            continue;
        };
        let Some(items) = days.get_mut(&time.date_naive()) else {
            continue;
        };
        let title = moment.content.lines().next().unwrap_or_default();
        let item = AgendaMonthItem {
            title: format!("{} {title}", time.format("%H:%M")),
            tag_id: None,
            tag_color: None,
            tag_class: force_core::TagClass::Legacy,
            status: AgendaItemStatus::Moment,
            seconds: 0,
            planned: false,
            completed: false,
            rest: false,
            moved: false,
        };
        let index = items
            .iter()
            .position(|item| item.planned)
            .unwrap_or(items.len());
        items.insert(index, item);
    }
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

fn validate_plan_fields(draft: &PlanInputSnapshot) -> Result<(NaiveDate, u16, u16), PlanError> {
    if draft.title.is_empty()
        || draft.title.chars().count() > 200
        || draft.title.chars().any(char::is_control)
    {
        return Err(PlanError::new(
            PlanField::Title,
            "请输入 1–200 字的安排名称。",
        ));
    }
    let day = NaiveDate::parse_from_str(&draft.day, "%Y-%m-%d")
        .ok()
        .filter(|day| (1..=9999).contains(&day.year()))
        .ok_or(PlanError::new(PlanField::Date, "日期格式应为 YYYY-MM-DD。"))?;
    let start = parse_plan_time(&draft.start, false).ok_or(PlanError::new(
        PlanField::Start,
        "开始时间格式应为 09:00，范围为 00:00–23:59。",
    ))?;
    let end = parse_plan_time(&draft.end, true).ok_or(PlanError::new(
        PlanField::End,
        "结束时间格式应为 10:00，最晚为 24:00。",
    ))?;
    if end <= start {
        return Err(PlanError::new(PlanField::End, "结束时间须晚于开始时间。"));
    }
    if draft.note.chars().count() > 2000
        || draft
            .note
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(PlanError::new(
            PlanField::Note,
            "备注最多 2000 字，且不能包含无效字符。",
        ));
    }
    Ok((day, start, end))
}

#[cfg(test)]
mod tests {
    use super::{
        AgendaItemStatus, PlanInputSnapshot, agenda_actual_dates, agenda_clock, agenda_month_days,
        agenda_visible_dates, merge_active_records, merge_month_moments, parse_plan_time,
        record_status, validate_plan_fields,
    };
    use crate::model::day_bounds;
    use chrono::{Datelike, Days, NaiveDate};
    use force_core::{Interval, PlanEntry, Session, TagKind, Task};
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
            tag_class: force_core::TagClass::Legacy,
            tag_color: None,
            timing_estimated: false,
            timing_adjusted: false,
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
            tag_class: force_core::TagClass::Legacy,
            tag_color: None,
            timing_estimated: false,
            timing_adjusted: false,
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
            tag_class: force_core::TagClass::Legacy,
            tag_color: None,
            timing_estimated: false,
            timing_adjusted: false,
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

    fn month_record(id: i64, task: Option<i64>, start: i64, end: Option<i64>) -> Session {
        Session {
            id,
            task_id: task,
            title: "Same visible title".into(),
            tag_id: 1,
            tag_name: "Work".into(),
            tag_kind: TagKind::Focus,
            tag_class: force_core::TagClass::Legacy,
            tag_color: None,
            timing_estimated: false,
            timing_adjusted: false,
            started_at: start,
            finished_at: end,
            target_seconds: None,
            intervals: vec![Interval { start, end }],
        }
    }

    fn month_plan(id: i64, task_id: i64, day: NaiveDate) -> PlanEntry {
        PlanEntry {
            id,
            task_id,
            title: "Scheduled title".into(),
            day,
            start_minute: 540,
            end_minute: 600,
            note: String::new(),
            completed_at: None,
            moved_to: None,
            created_at: day_bounds(day).0,
        }
    }

    #[test]
    fn month_overview_clips_actual_intervals_and_does_not_turn_paused_days_into_work() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let previous = day.pred_opt().unwrap();
        let next = day.succ_opt().unwrap();
        let start = day_bounds(day).0;
        let end = day_bounds(day).1;
        let across = month_record(1, Some(1), start - 120, Some(start + 180));
        let mut paused = month_record(2, Some(2), start - 1000, Some(end + 240));
        paused.intervals = vec![
            Interval {
                start: start - 1000,
                end: Some(start - 900),
            },
            Interval {
                start: end + 120,
                end: Some(end + 240),
            },
        ];
        let days = agenda_month_days(
            &[previous, day, next],
            &[across, paused],
            &[],
            &[],
            &[],
            &[],
            end + 300,
        );
        assert_eq!(
            days[&previous].iter().map(|item| item.seconds).sum::<i64>(),
            220
        );
        assert_eq!(days[&day].len(), 1);
        assert_eq!(days[&day][0].seconds, 180);
        assert_eq!(days[&next].len(), 1);
        assert_eq!(days[&next][0].seconds, 120);
    }

    #[test]
    fn month_overview_combines_task_rounds_and_plans_but_keeps_legacy_and_rest_identity() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let start = day_bounds(day).0;
        let mut rest = month_record(5, None, start + 3000, Some(start + 3060));
        rest.tag_kind = TagKind::Rest;
        let records = [
            month_record(1, Some(1), start, Some(start + 120)),
            month_record(2, Some(1), start + 300, Some(start + 480)),
            month_record(3, None, start + 600, Some(start + 660)),
            month_record(4, None, start + 900, Some(start + 960)),
            rest,
        ];
        let completed = Task {
            id: 1,
            title: "Completed renamed task".into(),
            tag_id: 1,
            tag_name: "Work".into(),
            tag_kind: TagKind::Focus,
            created_at: start,
            completed_at: Some(start + 3600),
            total_seconds: 9000,
            session_count: 2,
        };
        let plans = [month_plan(1, 1, day), month_plan(2, 1, day)];
        let days = agenda_month_days(
            &[day],
            &records,
            &[completed],
            &plans,
            &[],
            &[],
            start + 4000,
        );
        let items = &days[&day];
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].seconds, 300);
        assert_eq!(items[0].title, "Same visible title");
        assert!(items[0].completed);
        assert!(!items[0].planned);
        assert_eq!(items.iter().filter(|item| item.rest).count(), 1);
        assert_eq!(items.iter().map(|item| item.seconds).sum::<i64>(), 480);
    }

    #[test]
    fn month_overview_shows_untimed_completion_and_moved_plan_without_inventing_minutes() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let start = day_bounds(day).0;
        let completed = Task {
            id: 1,
            title: "Untimed completed task".into(),
            tag_id: 1,
            tag_name: "Work".into(),
            tag_kind: TagKind::Focus,
            created_at: start,
            completed_at: Some(start + 60),
            total_seconds: 9000,
            session_count: 2,
        };
        let mut moved = month_plan(1, 1, day);
        moved.moved_to = Some(2);
        let plans = [moved, month_plan(3, 3, day)];
        let days = agenda_month_days(&[day], &[], &[completed], &plans, &[], &[], start + 120);
        let items = &days[&day];
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|item| item.seconds == 0));
        assert_eq!(items[0].title, "Untimed completed task");
        assert!(items[0].completed);
        assert!(!items[0].planned);
        assert!(items[1].planned);
        assert!(!items[1].moved);
        assert!(items[2].moved);
    }

    #[test]
    fn month_overview_open_interval_updates_from_the_same_snapshot_across_midnight() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let start = day_bounds(day).0;
        let records = [month_record(1, Some(1), start - 60, None)];
        let at_midnight = agenda_month_days(&[day], &records, &[], &[], &[], &[], start);
        assert_eq!(at_midnight[&day].len(), 1);
        assert_eq!(at_midnight[&day][0].seconds, 0);
        let later = agenda_month_days(&[day], &records, &[], &[], &[], &[], start + 90);
        assert_eq!(later[&day][0].seconds, 90);
        assert!(!later[&day][0].planned);
    }

    #[test]
    fn month_overview_occurrence_completion_keeps_other_plans_and_actual_tasks_unfinished() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let start = day_bounds(day).0;
        let mut done = month_plan(1, 1, day);
        done.completed_at = Some(start + 60);
        let mut pending = month_plan(2, 1, day);
        let mut moved = month_plan(3, 1, day);
        moved.completed_at = Some(start + 60);
        moved.moved_to = Some(4);
        let days = agenda_month_days(
            &[day],
            &[],
            &[],
            &[done.clone(), pending.clone(), moved.clone()],
            &[],
            &[],
            start + 300,
        );
        assert_eq!(days[&day].len(), 2);
        assert!(days[&day][0].planned);
        assert!(!days[&day][0].completed);
        assert!(days[&day][1].moved);
        assert!(days[&day][1].completed);

        pending.completed_at = Some(start + 120);
        let all_done = agenda_month_days(
            &[day],
            &[],
            &[],
            &[done.clone(), pending],
            &[],
            &[],
            start + 300,
        );
        assert!(all_done[&day][0].completed);

        let actual = agenda_month_days(
            &[day],
            &[month_record(1, Some(1), start, Some(start + 120))],
            &[],
            &[done, moved],
            &[],
            &[],
            start + 300,
        );
        assert!(!actual[&day][0].planned);
        assert!(!actual[&day][0].completed);
        assert!(actual[&day][1].moved);
        assert!(actual[&day][1].completed);
    }

    #[test]
    fn calendar_status_distinguishes_finished_rounds_and_today_clock_without_rewriting_history() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let previous = day.pred_opt().unwrap();
        let now = day_bounds(day).0 + 600;
        let mut running = month_record(1, Some(1), now - 900, None);
        running.tag_color = Some("#B56F67".into());
        let mut paused = running.clone();
        paused.id = 2;
        paused.task_id = Some(2);
        paused.intervals[0].end = Some(now - 60);
        let finished = month_record(3, Some(3), now - 300, Some(now - 30));
        assert_eq!(
            record_status(&finished, day, now),
            AgendaItemStatus::Recorded
        );
        assert_eq!(
            record_status(&running, previous, now),
            AgendaItemStatus::Recorded
        );
        assert_eq!(
            record_status(&paused, previous, now),
            AgendaItemStatus::Recorded
        );
        let days = agenda_month_days(
            &[previous, day],
            &[running, paused, finished],
            &[],
            &[],
            &[],
            &[],
            now,
        );
        let items = &days[&day];
        assert!(
            items
                .iter()
                .any(|item| item.status == AgendaItemStatus::Running)
        );
        assert!(
            items
                .iter()
                .any(|item| item.status == AgendaItemStatus::Paused)
        );
        assert!(
            items
                .iter()
                .any(|item| item.status == AgendaItemStatus::Recorded)
        );
        assert!(items.iter().all(|item| !item.completed));
        let running = items
            .iter()
            .find(|item| item.status == AgendaItemStatus::Running)
            .unwrap();
        assert_eq!(running.tag_id, Some(1));
        assert_eq!(running.tag_color.as_deref(), Some("#B56F67"));
        assert!(
            days[&previous]
                .iter()
                .all(|item| item.status == AgendaItemStatus::Recorded)
        );
    }

    #[test]
    fn month_plan_color_uses_existing_task_tag_and_completion_stays_occurrence_specific() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let start = day_bounds(day).0;
        let task = Task {
            id: 1,
            title: "Task".into(),
            tag_id: 42,
            tag_name: "Work".into(),
            tag_kind: TagKind::Focus,
            created_at: start,
            completed_at: None,
            total_seconds: 0,
            session_count: 0,
        };
        let mut done = month_plan(1, 1, day);
        done.completed_at = Some(start + 60);
        let mut moved = month_plan(2, 1, day);
        moved.moved_to = Some(3);
        let days = agenda_month_days(&[day], &[], &[], &[done, moved], &[task], &[], start + 120);
        assert!(days[&day].iter().all(|item| item.tag_id == Some(42)));
        assert_eq!(days[&day][0].status, AgendaItemStatus::Completed);
        assert_eq!(days[&day][0].status.description(true), "本次安排已完成");
        assert_eq!(days[&day][1].status, AgendaItemStatus::Moved);
        assert!(
            days[&day]
                .iter()
                .all(|item| item.planned && item.seconds == 0)
        );
    }

    #[test]
    fn moment_month_rows_use_local_points_without_adding_minutes_or_completions() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let next = day.succ_opt().unwrap();
        let start = day_bounds(day).0;
        let next_start = day_bounds(next).0;
        let mut days = agenda_month_days(&[day, next], &[], &[], &[], &[], &[], next_start + 60);
        let moments = [
            force_core::Moment {
                id: 1,
                content: "写下一点想法\n第二行".into(),
                occurred_at: start,
                created_at: next_start,
                updated_at: next_start,
            },
            force_core::Moment {
                id: 2,
                content: "午夜记录".into(),
                occurred_at: next_start,
                created_at: next_start,
                updated_at: next_start,
            },
        ];
        merge_month_moments(&mut days, &moments);
        assert_eq!(days[&day].len(), 1);
        assert_eq!(days[&next].len(), 1);
        assert_eq!(days[&day][0].title, "00:00 写下一点想法");
        assert_eq!(days[&next][0].title, "00:00 午夜记录");
        assert!(
            days.values()
                .flatten()
                .all(|item| item.status == AgendaItemStatus::Moment
                    && item.seconds == 0
                    && !item.completed
                    && !item.planned)
        );
    }

    #[test]
    fn month_plans_use_old_paused_clock_without_adding_historical_intervals_or_minutes() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let previous = day.pred_opt().unwrap();
        let old_start = day_bounds(day.checked_sub_days(Days::new(50)).unwrap()).0;
        let now = day_bounds(day).0 + 600;
        let mut paused = month_record(1, Some(1), old_start, None);
        paused.intervals[0].end = Some(old_start + 120);
        paused.tag_color = Some("#688BA5".into());
        let days = agenda_month_days(
            &[previous, day],
            &[],
            &[],
            &[month_plan(1, 1, previous), month_plan(2, 1, day)],
            &[],
            &[paused],
            now,
        );
        assert_eq!(days[&day].len(), 1);
        assert_eq!(days[&day][0].status, AgendaItemStatus::Paused);
        assert_eq!(days[&previous][0].status, AgendaItemStatus::Pending);
        assert_eq!(days[&day][0].tag_id, Some(1));
        assert_eq!(days[&day][0].tag_color.as_deref(), Some("#688BA5"));
        assert!(
            days.values()
                .flatten()
                .all(|item| item.planned && item.seconds == 0)
        );
    }
}
