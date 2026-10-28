//! Completed timer records keep their individual intervals when edited.

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use force_core::{Interval, RecordEditSnapshot, RecordIntervalEdit, Session, TagKind};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Focusable as _, PromptButton, PromptLevel,
    ScrollHandle, Subscription, Window,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    model::{ForceView, day_bounds},
    tag_picker::{FocusTagOption, TagPicked, TagPicker},
};

gpui::actions!(record_editor, [NextField, PreviousField]);

#[derive(Clone, PartialEq)]
struct PointFields {
    date: String,
    time: String,
}

#[derive(Clone, PartialEq)]
struct IntervalFields {
    start: PointFields,
    end: PointFields,
}

#[derive(Clone, PartialEq)]
struct RecordFields {
    activity: i64,
    description: String,
    intervals: Vec<IntervalFields>,
}

pub(crate) struct RecordIntervalInputs {
    pub(crate) key: u64,
    pub(crate) start_date: Entity<InputState>,
    pub(crate) start_time: Entity<InputState>,
    pub(crate) end_date: Entity<InputState>,
    pub(crate) end_time: Entity<InputState>,
    original_start: Option<(PointFields, i64)>,
    original_end: Option<(PointFields, i64)>,
    _subscriptions: Vec<Subscription>,
}

impl RecordIntervalInputs {
    fn new(
        key: u64,
        start: i64,
        end: i64,
        preserve: bool,
        window: &mut Window,
        cx: &mut Context<ForceView>,
    ) -> Self {
        let start_fields = point_fields(start);
        let end_fields = point_fields(end);
        let start_date = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(start_fields.date.clone())
                .placeholder("YYYY-MM-DD")
        });
        let start_time = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(start_fields.time.clone())
                .placeholder("HH:MM")
        });
        let end_date = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(end_fields.date.clone())
                .placeholder("YYYY-MM-DD")
        });
        let end_time = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(end_fields.time.clone())
                .placeholder("HH:MM")
        });
        let subscriptions = [&start_date, &start_time, &end_date, &end_time]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.record_editor.error = None;
                        cx.notify();
                    }
                })
            })
            .collect();
        Self {
            key,
            start_date,
            start_time,
            end_date,
            end_time,
            original_start: preserve.then_some((start_fields, start)),
            original_end: preserve.then_some((end_fields, end)),
            _subscriptions: subscriptions,
        }
    }
    fn fields(&self, cx: &gpui::App) -> IntervalFields {
        IntervalFields {
            start: PointFields {
                date: self.start_date.read(cx).value().to_string(),
                time: self.start_time.read(cx).value().to_string(),
            },
            end: PointFields {
                date: self.end_date.read(cx).value().to_string(),
                time: self.end_time.read(cx).value().to_string(),
            },
        }
    }
}

pub(crate) struct RecordEditorState {
    pub(crate) visible: bool,
    pub(crate) active: bool,
    pub(crate) original: Option<Session>,
    snapshot: Option<RecordEditSnapshot>,
    pub(crate) activity: i64,
    pub(crate) description: Entity<InputState>,
    pub(crate) picker: Entity<TagPicker>,
    pub(crate) intervals: Vec<RecordIntervalInputs>,
    pub(crate) error: Option<String>,
    pub(crate) focus: FocusHandle,
    pub(crate) scroll: ScrollHandle,
    baseline: Option<RecordFields>,
    generation: u64,
    next_interval: u64,
    options: Vec<FocusTagOption>,
    _subscriptions: Vec<Subscription>,
}

impl RecordEditorState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ForceView>) -> Self {
        cx.bind_keys([
            gpui::KeyBinding::new("tab", NextField, Some("RecordEditor")),
            gpui::KeyBinding::new("shift-tab", PreviousField, Some("RecordEditor")),
        ]);
        let description = cx.new(|cx| InputState::new(window, cx).placeholder("补充说明（可选）"));
        let picker = cx.new(TagPicker::new);
        let input_subscription = cx.subscribe(&description, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.record_editor.error = None;
                cx.notify();
            }
        });
        let picker_subscription = cx.subscribe(&picker, |this, _, event: &TagPicked, cx| {
            if this
                .record_editor
                .options
                .iter()
                .any(|option| option.id == event.0)
            {
                this.record_editor.activity = event.0;
                this.record_editor.error = None;
                cx.notify();
            }
        });
        Self {
            visible: false,
            active: false,
            original: None,
            snapshot: None,
            activity: 0,
            description,
            picker,
            intervals: Vec::new(),
            error: None,
            focus: cx.focus_handle().tab_stop(false),
            scroll: ScrollHandle::default(),
            baseline: None,
            generation: 0,
            next_interval: 0,
            options: Vec::new(),
            _subscriptions: vec![input_subscription, picker_subscription],
        }
    }
    fn fields(&self, cx: &gpui::App) -> RecordFields {
        RecordFields {
            activity: self.activity,
            description: self.description.read(cx).value().to_string(),
            intervals: self
                .intervals
                .iter()
                .map(|interval| interval.fields(cx))
                .collect(),
        }
    }
    fn changed(&self, cx: &gpui::App) -> bool {
        self.active
            && self
                .baseline
                .as_ref()
                .is_some_and(|baseline| *baseline != self.fields(cx))
    }
}

impl ForceView {
    pub(crate) fn begin_record_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.record_editor.active {
            let now = Local::now().timestamp();
            let (_, day_end) = day_bounds(self.record_date);
            let end = if day_end <= now { day_end - 60 } else { now };
            self.load_record_editor(None, vec![(end - 1800, end)], window, cx);
        }
        self.reveal_record_editor(window, cx);
    }

    pub(crate) fn edit_record(
        &mut self,
        record: Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if record.finished_at.is_none()
            || record
                .intervals
                .iter()
                .any(|interval| interval.end.is_none())
        {
            self.notify("请先结束这段计时，再修改记录。", cx);
            return;
        }
        if self.record_editor.active
            && self
                .record_editor
                .original
                .as_ref()
                .is_some_and(|original| original.id == record.id)
        {
            self.reveal_record_editor(window, cx);
            return;
        }
        if self.record_editor.changed(cx) {
            self.reveal_record_editor(window, cx);
            let generation = self.record_editor.generation;
            let fields = self.record_editor.fields(cx);
            let answer = window.prompt(
                PromptLevel::Info,
                "保存记录修改？",
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
                            if this.record_editor.generation != generation
                                || this.record_editor.fields(cx) != fields
                            {
                                return;
                            }
                            if choice == 0 {
                                this.focus_record_editor(window, cx);
                                return;
                            }
                            if choice == 2 && !this.save_record_editor(window, cx) {
                                return;
                            }
                            if choice == 1 || choice == 2 {
                                this.record_editor.active = false;
                                this.edit_record(record, window, cx);
                            }
                        });
                    });
                }
            })
            .detach();
            return;
        }
        let snapshot = match self
            .store
            .as_ref()
            .map(|store| store.record_edit_snapshot(record.id))
        {
            Some(Ok(snapshot)) => snapshot,
            _ => {
                self.notify("这条记录已变更，请刷新后重试。", cx);
                return;
            }
        };
        let intervals = snapshot
            .session
            .intervals
            .iter()
            .filter_map(|interval| interval.end.map(|end| (interval.start, end)))
            .collect();
        self.load_record_editor(Some(snapshot), intervals, window, cx);
        self.reveal_record_editor(window, cx);
    }

    fn load_record_editor(
        &mut self,
        snapshot: Option<RecordEditSnapshot>,
        intervals: Vec<(i64, i64)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let original = snapshot.as_ref().map(|snapshot| snapshot.session.clone());
        let description = original
            .as_ref()
            .filter(|record| record.title != record.tag_name)
            .map_or_else(String::new, |record| record.title.clone());
        self.record_editor.activity = original.as_ref().map_or(0, |record| record.tag_id);
        self.record_editor.generation = self.record_editor.generation.wrapping_add(1);
        self.record_editor.active = true;
        self.record_editor.error = None;
        self.record_editor
            .description
            .update(cx, |input, cx| input.set_value(description, window, cx));
        self.record_editor.intervals = intervals
            .into_iter()
            .map(|(start, end)| {
                self.record_editor.next_interval = self.record_editor.next_interval.wrapping_add(1);
                RecordIntervalInputs::new(
                    self.record_editor.next_interval,
                    start,
                    end,
                    original.is_some(),
                    window,
                    cx,
                )
            })
            .collect();
        self.record_editor.original = original;
        self.record_editor.snapshot = snapshot;
        self.record_editor.baseline = Some(self.record_editor.fields(cx));
        self.record_editor
            .scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
    }

    fn reveal_record_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_completion_editor(window, cx);
        self.suspend_moment(window, cx);
        self.suspend_agenda_popup(window, cx);
        self.tag_picker
            .update(cx, |picker, cx| picker.dismiss(window, false, cx));
        self.record_editor.visible = true;
        self.sync_record_editor_picker(window, cx);
        self.show_manager(window, cx);
        self.focus_record_editor(window, cx);
        cx.notify();
    }

    pub(crate) fn focus_record_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.record_editor.activity == 0 {
            self.record_editor
                .picker
                .read(cx)
                .focus_handle(cx)
                .focus(window);
        } else {
            self.record_editor
                .description
                .read(cx)
                .focus_handle(cx)
                .focus(window);
        }
    }

    pub(crate) fn cycle_record_editor_focus(
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
            if self.record_editor.focus.contains_focused(window, cx) {
                return;
            }
            if window.focused(cx) == first {
                break;
            }
        }
        self.focus_record_editor(window, cx);
    }

    pub(crate) fn sync_record_editor_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let original = self.record_editor.original.as_ref();
        let mut options: Vec<_> = self
            .tags
            .iter()
            .filter(|tag| {
                tag.kind == TagKind::Focus || original.is_some_and(|record| record.tag_id == tag.id)
            })
            .map(|tag| FocusTagOption {
                id: tag.id,
                name: tag.name.clone(),
                color: self.display_tag_color(tag).map(str::to_owned),
            })
            .collect();
        if let Some(record) = original
            && !options.iter().any(|option| option.id == record.tag_id)
        {
            options.push(FocusTagOption {
                id: record.tag_id,
                name: format!("{}（已归档）", record.tag_name),
                color: self.display_session_color(record).map(str::to_owned),
            });
        }
        let selected = options
            .iter()
            .find(|option| option.id == self.record_editor.activity)
            .map(|option| option.id);
        let enabled = self.record_editor.visible && !self.compact && !self.storage_failed;
        let picker = self.record_editor.picker.read(cx);
        if options != self.record_editor.options
            || picker.selected_value() != selected
            || picker.is_enabled() != enabled
        {
            self.record_editor.picker.update(cx, |picker, cx| {
                picker.synchronize(options.clone(), selected, enabled, window, cx)
            });
            self.record_editor.options = options;
        }
        let width = (f32::from(window.viewport_size().width) - 88.).min(520.);
        self.record_editor
            .picker
            .update(cx, |picker, cx| picker.set_trigger_width(width, cx));
    }

    pub(crate) fn suspend_record_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.time_picker
            .update(cx, |picker, cx| picker.dismiss(window, cx));
        self.record_editor
            .picker
            .update(cx, |picker, cx| picker.dismiss(window, false, cx));
        if self.record_editor.visible {
            self.record_editor.visible = false;
            window.focus(&self.focus_handle);
            cx.notify();
        }
    }

    pub(crate) fn cancel_record_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_record_editor(window, cx);
        self.record_editor.active = false;
        self.record_editor.original = None;
        self.record_editor.snapshot = None;
        self.record_editor.baseline = None;
        self.record_editor.error = None;
        self.record_editor.intervals.clear();
        self.record_editor.generation = self.record_editor.generation.wrapping_add(1);
        cx.notify();
    }

    pub(crate) fn record_editor_time_hint(&self) -> &'static str {
        if self.record_editor.original.is_none() {
            "结束时间就是任务完成时间。"
        } else if self
            .record_editor
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.task_completion_follows_record())
        {
            "结束时间同步为任务完成时间，暂停间隔保留。"
        } else if self
            .record_editor
            .original
            .as_ref()
            .is_some_and(|record| record.task_id.is_some())
        {
            "暂停间隔保留；任务单独设置的完成时间保持不变。"
        } else {
            "每段时间分别保留，段与段之间仍是暂停。"
        }
    }

    pub(crate) fn save_record_editor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.record_editor.visible
            || !self.record_editor.active
            || self.compact
            || self.time_picker.read(cx).is_open()
            || self.record_editor.picker.read(cx).is_open()
        {
            return false;
        }
        if self.record_editor.activity == 0 {
            self.record_editor.error = Some("请选择活动。".to_owned());
            cx.notify();
            return false;
        }
        let description = self
            .record_editor
            .description
            .read(cx)
            .value()
            .trim()
            .to_owned();
        if description.chars().count() > 200 || description.contains('\0') {
            self.record_editor.error = Some("说明最多 200 字，不能包含无效字符。".to_owned());
            cx.notify();
            return false;
        }
        let intervals = match self.record_editor_intervals(cx) {
            Ok(intervals) => intervals
                .into_iter()
                .map(|interval| RecordIntervalEdit {
                    start: interval.start,
                    end: interval.end.expect("validated end"),
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                self.record_editor.error = Some(error);
                cx.notify();
                return false;
            }
        };
        let now = Local::now().timestamp();
        let activity = self.record_editor.activity;
        let Some(store) = self.store.as_mut() else {
            self.record_editor.error = Some("本地数据尚未打开，请重试。".to_owned());
            cx.notify();
            return false;
        };
        let result = match self.record_editor.snapshot.as_ref() {
            Some(snapshot) => {
                store.update_completed_record(snapshot, &description, activity, &intervals, now)
            }
            None => store.create_completed_record(&description, activity, &intervals, now),
        };
        match result {
            Ok(record) => {
                if self
                    .completed
                    .as_ref()
                    .is_some_and(|completed| completed.id == record.id)
                {
                    self.completed = Some(record.clone());
                }
                let (start, end) = day_bounds(self.record_date);
                if (self.record_editor.original.is_none()
                    || record.elapsed_between(start, end, now) == 0)
                    && let Some(date) = Local
                        .timestamp_opt(record.started_at, 0)
                        .single()
                        .map(|time| time.date_naive())
                {
                    self.record_date = date;
                }
                self.refresh(cx);
                self.cancel_record_editor(window, cx);
                if self.page == crate::model::Page::Agenda
                    && !self
                        .agenda
                        .records
                        .iter()
                        .any(|entry| entry.id == record.id)
                    && let Some(date) = Local
                        .timestamp_opt(record.started_at, 0)
                        .single()
                        .map(|time| time.date_naive())
                {
                    self.select_agenda_date(date, window, cx);
                    self.select_agenda_record(record.id, window, cx);
                }
                self.notify("记录已保存。", cx);
                true
            }
            Err(error) => {
                self.record_editor.error = Some(match error {
                    force_core::Error::NotFound | force_core::Error::InvalidTransition => {
                        "记录已变更或仍在计时。请取消本次编辑，再重新打开。".to_owned()
                    }
                    force_core::Error::ArchivedTag => "这个活动已归档，请重新选择。".to_owned(),
                    force_core::Error::Validation(message) => match message {
                        "This record changed while editing; reopen it before saving" => {
                            "记录已在别处修改，当前草稿仍保留。请取消本次编辑，再重新打开。"
                                .to_owned()
                        }
                        "Task completion must remain after its other rounds; edit the final round instead" => {
                            "完成时间不能早于该任务的其他计时，请修改最后一段记录。".to_owned()
                        }
                        _ => "时间与现有任务记录冲突，请检查各段时间。".to_owned(),
                    },
                    _ => "保存失败，草稿和原记录均已保留，请重试。".to_owned(),
                });
                cx.notify();
                false
            }
        }
    }

    fn record_editor_intervals(&self, cx: &gpui::App) -> Result<Vec<Interval>, String> {
        let now = Local::now().timestamp();
        let mut intervals = Vec::new();
        for (index, input) in self.record_editor.intervals.iter().enumerate() {
            let fields = input.fields(cx);
            let start = parse_point(&fields.start, input.original_start.as_ref())
                .map_err(|message| format!("第 {} 段开始：{message}", index + 1))?;
            let end = parse_point(&fields.end, input.original_end.as_ref())
                .map_err(|message| format!("第 {} 段结束：{message}", index + 1))?;
            let preserved_zero = end == start
                && input
                    .original_start
                    .as_ref()
                    .is_some_and(|(_, timestamp)| *timestamp == start)
                && input
                    .original_end
                    .as_ref()
                    .is_some_and(|(_, timestamp)| *timestamp == end);
            if end < start || (end == start && !preserved_zero) {
                return Err(format!("第 {} 段的结束时间需晚于开始。", index + 1));
            }
            if end > now {
                return Err("记录时间不能在未来。".to_owned());
            }
            if intervals
                .last()
                .is_some_and(|previous: &Interval| previous.end.unwrap_or(start) > start)
            {
                return Err("各段时间需按先后排列，不能重叠。".to_owned());
            }
            intervals.push(Interval {
                start,
                end: Some(end),
            });
        }
        if intervals.is_empty() {
            return Err("请至少保留一段时间。".to_owned());
        }
        Ok(intervals)
    }
}

fn point_fields(timestamp: i64) -> PointFields {
    let local = Local
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Local::now);
    PointFields {
        date: local.format("%Y-%m-%d").to_string(),
        time: local.format("%H:%M").to_string(),
    }
}

fn parse_point(
    fields: &PointFields,
    original: Option<&(PointFields, i64)>,
) -> Result<i64, &'static str> {
    if let Some((baseline, timestamp)) = original
        && baseline == fields
    {
        return Ok(*timestamp);
    }
    let date = NaiveDate::parse_from_str(fields.date.trim(), "%Y-%m-%d")
        .map_err(|_| "日期格式为 YYYY-MM-DD。")?;
    use chrono::Datelike as _;
    if !(1..=9999).contains(&date.year()) {
        return Err("日期年份需在 1–9999 之间。");
    }
    let time = NaiveTime::parse_from_str(fields.time.trim(), "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(fields.time.trim(), "%H:%M:%S"))
        .map_err(|_| "时间格式为 HH:MM。")?;
    Local
        .from_local_datetime(&NaiveDateTime::new(date, time))
        .single()
        .map(|stamp| stamp.timestamp())
        .ok_or("这个本地时间不存在或有歧义。")
}
