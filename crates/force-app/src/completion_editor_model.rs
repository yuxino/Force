//! A task completion is a point on the calendar, separate from timer records.

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike};
use force_core::{CompletedTaskEditSnapshot, TagKind, Task};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Focusable as _, PromptButton, PromptLevel,
    Subscription, Window,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    model::{ForceView, Page},
    tag_picker::{FocusTagOption, TagPicked, TagPicker},
};

gpui::actions!(completion_editor, [NextField, PreviousField]);

#[derive(Clone, PartialEq)]
struct CompletionFields {
    activity: i64,
    description: String,
    date: String,
    time: String,
}

pub(crate) struct CompletionEditorState {
    pub(crate) visible: bool,
    pub(crate) active: bool,
    pub(crate) original: Option<Task>,
    pub(crate) activity: i64,
    pub(crate) description: Entity<InputState>,
    pub(crate) date: Entity<InputState>,
    pub(crate) time: Entity<InputState>,
    pub(crate) picker: Entity<TagPicker>,
    pub(crate) error: Option<String>,
    pub(crate) focus: FocusHandle,
    snapshot: Option<CompletedTaskEditSnapshot>,
    baseline: Option<CompletionFields>,
    generation: u64,
    options: Vec<FocusTagOption>,
    _subscriptions: Vec<Subscription>,
}

impl CompletionEditorState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ForceView>) -> Self {
        cx.bind_keys([
            gpui::KeyBinding::new("tab", NextField, Some("CompletionEditor")),
            gpui::KeyBinding::new("shift-tab", PreviousField, Some("CompletionEditor")),
        ]);
        let description = cx.new(|cx| InputState::new(window, cx).placeholder("补充说明（可选）"));
        let date = cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD"));
        let time = cx.new(|cx| InputState::new(window, cx).placeholder("HH:MM"));
        let picker = cx.new(TagPicker::new);
        let mut subscriptions: Vec<_> = [&description, &date, &time]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.completion_editor.error = None;
                        cx.notify();
                    }
                })
            })
            .collect();
        subscriptions.push(cx.subscribe(&picker, |this, _, event: &TagPicked, cx| {
            if this
                .completion_editor
                .options
                .iter()
                .any(|option| option.id == event.0)
            {
                this.completion_editor.activity = event.0;
                this.completion_editor.error = None;
                cx.notify();
            }
        }));
        Self {
            visible: false,
            active: false,
            original: None,
            activity: 0,
            description,
            date,
            time,
            picker,
            error: None,
            focus: cx.focus_handle().tab_stop(false),
            snapshot: None,
            baseline: None,
            generation: 0,
            options: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    fn fields(&self, cx: &gpui::App) -> CompletionFields {
        CompletionFields {
            activity: self.activity,
            description: self.description.read(cx).value().to_string(),
            date: self.date.read(cx).value().to_string(),
            time: self.time.read(cx).value().to_string(),
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
    pub(crate) fn edit_completion(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if self.completion_editor.active
            && self
                .completion_editor
                .original
                .as_ref()
                .is_some_and(|task| task.id == id)
        {
            self.reveal_completion_editor(window, cx);
            return;
        }
        if self.completion_editor.changed(cx) {
            // A newly revealed editor may not have a painted focus ancestry
            // yet. Capture its current field before reveal resets focus; a
            // suspended editor instead starts at its description field.
            let prior_focus = self
                .completion_editor
                .visible
                .then(|| window.focused(cx))
                .flatten();
            self.reveal_completion_editor(window, cx);
            let generation = self.completion_editor.generation;
            let fields = self.completion_editor.fields(cx);
            let answer = window.prompt(
                PromptLevel::Info,
                "保存完成事项的修改？",
                None,
                &[
                    PromptButton::cancel("取消"),
                    PromptButton::new("不保存"),
                    PromptButton::new("保存"),
                ],
                cx,
            );
            cx.spawn_in(window, async move |this, cx| {
                let choice = answer.await;
                let _ = cx.update(|window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if !this.compact {
                            window.activate_window();
                        }
                        if this.completion_editor.generation != generation
                            || this.completion_editor.fields(cx) != fields
                            || this.compact
                        {
                            return;
                        }
                        if let Some(focus) = prior_focus.as_ref() {
                            focus.focus(window);
                        } else {
                            this.focus_completion_editor(window, cx);
                        }
                        let Ok(choice) = choice else {
                            this.notify("未能完成确认，当前草稿已保留。", cx);
                            return;
                        };
                        if choice == 1 || (choice == 2 && this.save_completion_editor(window, cx)) {
                            this.cancel_completion_editor(window, cx);
                            this.edit_completion(id, window, cx);
                        }
                    });
                });
            })
            .detach();
            return;
        }
        let snapshot = match self
            .store
            .as_ref()
            .map(|store| store.completed_task_edit_snapshot(id))
        {
            Some(Ok(snapshot)) => snapshot,
            _ => {
                self.notify("完成事项已变更，请刷新后重试。", cx);
                return;
            }
        };
        let original = snapshot.task.clone();
        let Some(stamp) = original
            .completed_at
            .and_then(|value| Local.timestamp_opt(value, 0).single())
        else {
            self.notify("完成时间无效，原事项已保留。", cx);
            return;
        };
        let description = if original.title == original.tag_name {
            String::new()
        } else {
            original.title.clone()
        };
        self.completion_editor
            .description
            .update(cx, |input, cx| input.set_value(description, window, cx));
        self.completion_editor.date.update(cx, |input, cx| {
            input.set_value(stamp.format("%Y-%m-%d").to_string(), window, cx)
        });
        self.completion_editor.time.update(cx, |input, cx| {
            input.set_value(stamp.format("%H:%M").to_string(), window, cx)
        });
        self.completion_editor.activity = original.tag_id;
        self.completion_editor.original = Some(original);
        self.completion_editor.snapshot = Some(snapshot);
        self.completion_editor.active = true;
        self.completion_editor.error = None;
        self.completion_editor.generation = self.completion_editor.generation.wrapping_add(1);
        self.completion_editor.baseline = Some(self.completion_editor.fields(cx));
        self.reveal_completion_editor(window, cx);
    }

    fn reveal_completion_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_record_editor(window, cx);
        self.suspend_moment(window, cx);
        self.suspend_agenda_popup(window, cx);
        self.tag_picker
            .update(cx, |picker, cx| picker.dismiss(window, false, cx));
        self.completion_editor.visible = true;
        self.show_manager(window, cx);
        self.sync_completion_editor_picker(window, cx);
        self.focus_completion_editor(window, cx);
        cx.notify();
    }

    pub(crate) fn focus_completion_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.completion_editor
            .description
            .read(cx)
            .focus_handle(cx)
            .focus(window);
    }

    pub(crate) fn cycle_completion_editor_focus(
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
            if self.completion_editor.focus.contains_focused(window, cx) {
                return;
            }
            if window.focused(cx) == first {
                break;
            }
        }
        self.focus_completion_editor(window, cx);
    }

    pub(crate) fn sync_completion_editor_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let original = self.completion_editor.original.as_ref();
        let mut options: Vec<_> = self
            .tags
            .iter()
            .filter(|tag| {
                tag.kind == TagKind::Focus || original.is_some_and(|task| task.tag_id == tag.id)
            })
            .map(|tag| FocusTagOption {
                id: tag.id,
                name: tag.name.clone(),
                color: self.display_tag_color(tag).map(str::to_owned),
            })
            .collect();
        if let Some(task) = original
            && !options.iter().any(|option| option.id == task.tag_id)
        {
            options.push(FocusTagOption {
                id: task.tag_id,
                name: format!("{}（已归档）", task.tag_name),
                color: None,
            });
        }
        let selected = options
            .iter()
            .find(|option| option.id == self.completion_editor.activity)
            .map(|option| option.id);
        let enabled = self.completion_editor.visible && !self.compact && !self.storage_failed;
        let picker = self.completion_editor.picker.read(cx);
        if options != self.completion_editor.options
            || picker.selected_value() != selected
            || picker.is_enabled() != enabled
        {
            self.completion_editor.picker.update(cx, |picker, cx| {
                picker.synchronize(options.clone(), selected, enabled, window, cx)
            });
            self.completion_editor.options = options;
        }
        let width = (f32::from(window.viewport_size().width) - 88.).min(480.);
        self.completion_editor
            .picker
            .update(cx, |picker, cx| picker.set_trigger_width(width, cx));
    }

    pub(crate) fn suspend_completion_editor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.completion_editor.visible {
            self.time_picker
                .update(cx, |picker, cx| picker.dismiss(window, cx));
            self.completion_editor
                .picker
                .update(cx, |picker, cx| picker.dismiss(window, false, cx));
            self.completion_editor.visible = false;
            window.focus(&self.focus_handle);
            cx.notify();
        }
    }

    pub(crate) fn cancel_completion_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_completion_editor(window, cx);
        self.completion_editor.active = false;
        self.completion_editor.original = None;
        self.completion_editor.snapshot = None;
        self.completion_editor.baseline = None;
        self.completion_editor.error = None;
        self.completion_editor.generation = self.completion_editor.generation.wrapping_add(1);
        cx.notify();
    }

    pub(crate) fn save_completion_editor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.completion_editor.visible
            || !self.completion_editor.active
            || self.compact
            || self.time_picker.read(cx).is_open()
            || self.completion_editor.picker.read(cx).is_open()
        {
            return false;
        }
        let fields = self.completion_editor.fields(cx);
        let error = if fields.activity == 0 {
            Some("请选择活动。")
        } else if fields.description.trim().chars().count() > 200
            || fields.description.contains('\0')
        {
            Some("说明最多 200 字，不能包含无效字符。")
        } else {
            None
        };
        let stamp = completion_timestamp(
            &fields,
            self.completion_editor.baseline.as_ref(),
            self.completion_editor
                .original
                .as_ref()
                .and_then(|task| task.completed_at),
        );
        if let Some(error) = error.or_else(|| stamp.as_ref().err().copied()) {
            self.completion_editor.error = Some(error.to_owned());
            cx.notify();
            return false;
        }
        let Some(snapshot) = self.completion_editor.snapshot.as_ref() else {
            return false;
        };
        let original_day = snapshot.task.completed_at.and_then(|stamp| {
            Local
                .timestamp_opt(stamp, 0)
                .single()
                .map(|stamp| stamp.date_naive())
        });
        let Some(store) = self.store.as_mut() else {
            self.completion_editor.error = Some("本地数据尚未打开，请重试。".to_owned());
            cx.notify();
            return false;
        };
        let result = store.update_completed_task(
            snapshot,
            &fields.description,
            fields.activity,
            stamp.expect("validated timestamp"),
            Local::now().timestamp(),
        );
        match result {
            Ok(saved) => {
                self.refresh(cx);
                self.cancel_completion_editor(window, cx);
                if self.page == Page::Agenda
                    && let Some(stamp) = saved
                        .completed_at
                        .and_then(|stamp| Local.timestamp_opt(stamp, 0).single())
                    && Some(stamp.date_naive()) != original_day
                {
                    self.select_agenda_date(stamp.date_naive(), window, cx);
                    // Calendar combines each task with its arrangement or actual
                    // work. Select that visible item rather than a hidden marker.
                    let plan = self
                        .agenda
                        .plans
                        .iter()
                        .filter(|plan| plan.task_id == saved.id && plan.moved_to.is_none())
                        .min_by_key(|plan| (plan.start_minute, plan.id))
                        .map(|plan| plan.id);
                    let record = self
                        .agenda
                        .records
                        .iter()
                        .filter(|record| record.task_id == Some(saved.id))
                        .max_by_key(|record| (record.finished_at, record.id))
                        .map(|record| record.id);
                    if let Some(id) = plan {
                        self.select_plan(id, window, cx);
                    } else if let Some(id) = record {
                        self.select_agenda_record(id, window, cx);
                    } else {
                        self.select_agenda_completion(saved.id, window, cx);
                        let minute = (stamp.hour() * 60 + stamp.minute()) as u16;
                        self.agenda
                            .calendar_scroll_target
                            .set(Some(minute.saturating_sub(60)));
                    }
                }
                self.notify("完成事项已保存。", cx);
                true
            }
            Err(error) => {
                self.completion_editor.error = Some(completion_error(error).to_owned());
                cx.notify();
                false
            }
        }
    }

    pub(crate) fn delete_completion(
        &mut self,
        id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let snapshot = if let Some(snapshot) = self.completion_editor.snapshot.as_ref()
            && self.completion_editor.active
            && snapshot.task.id == id
        {
            snapshot.clone()
        } else {
            match self
                .store
                .as_ref()
                .map(|store| store.completed_task_edit_snapshot(id))
            {
                Some(Ok(snapshot)) => snapshot,
                _ => {
                    self.notify("完成事项已变更，请刷新后重试。", cx);
                    return;
                }
            }
        };
        let generation = self.completion_editor.generation;
        let fields = self.completion_editor.fields(cx);
        let prior_focus = window.focused(cx);
        let answer = window.prompt(
            PromptLevel::Warning,
            "删除完成事项？",
            Some("此事项和关联安排将从日历移除，已有计时记录和时长保留。"),
            &[
                PromptButton::cancel("取消"),
                PromptButton::new("删除完成事项"),
            ],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let answer = answer.await;
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if !this.compact {
                        window.activate_window();
                    }
                    if this.completion_editor.generation != generation
                        || this.completion_editor.fields(cx) != fields
                    {
                        return;
                    }
                    if let Some(focus) = prior_focus.as_ref()
                        && !this.compact
                    {
                        focus.focus(window);
                    }
                    let Ok(choice) = answer else {
                        this.notify("未能完成确认，原事项已保留。", cx);
                        return;
                    };
                    if choice != 1 {
                        return;
                    }
                    let Some(store) = this.store.as_mut() else {
                        this.notify("本地数据尚未打开，原事项已保留。", cx);
                        return;
                    };
                    match store.delete_completed_task(&snapshot) {
                        Ok(()) => {
                            if this
                                .completion_editor
                                .original
                                .as_ref()
                                .is_some_and(|task| task.id == id)
                            {
                                this.cancel_completion_editor(window, cx);
                            }
                            this.refresh(cx);
                            this.notify("完成事项已删除，计时记录保留。", cx);
                        }
                        Err(error) => this.notify(completion_error(error), cx),
                    }
                });
            });
        })
        .detach();
    }
}

fn completion_timestamp(
    fields: &CompletionFields,
    baseline: Option<&CompletionFields>,
    original: Option<i64>,
) -> Result<i64, &'static str> {
    if let Some(stamp) = original
        && baseline
            .is_some_and(|baseline| baseline.date == fields.date && baseline.time == fields.time)
    {
        return Ok(stamp);
    }
    let date = NaiveDate::parse_from_str(fields.date.trim(), "%Y-%m-%d")
        .map_err(|_| "日期格式为 YYYY-MM-DD。")?;
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
        .ok_or("这个本地时间不存在或有歧义，请调整时间。")
}

fn completion_error(error: force_core::Error) -> &'static str {
    match error {
        force_core::Error::NotFound | force_core::Error::InvalidTransition => {
            "完成事项已变更。草稿已保留，请取消本次编辑后重新打开。"
        }
        force_core::Error::ArchivedTag => "这个活动已归档，请重新选择。",
        force_core::Error::Validation("Task completion cannot be in the future") => {
            "完成时间不能在未来。"
        }
        force_core::Error::Validation(
            "Task completion must follow its creation and recorded work",
        ) => "完成时间不能早于事项创建时间或最后一段计时。",
        force_core::Error::Validation(
            "This completion changed while editing; reopen it before saving",
        ) => "事项或关联记录已在别处修改。草稿已保留，请取消后重新打开。",
        force_core::Error::Validation(_) => "请检查活动、说明和完成时间。",
        _ => "保存失败，草稿和原事项均已保留，请重试。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_minute_fields_keep_the_original_instant_and_seconds() {
        let baseline = CompletionFields {
            activity: 1,
            description: "Before".into(),
            date: "2026-11-01".into(),
            time: "01:30".into(),
        };
        let changed = CompletionFields {
            description: "After".into(),
            ..baseline.clone()
        };
        let local_time = NaiveDate::from_ymd_opt(2026, 11, 1)
            .unwrap()
            .and_hms_opt(1, 30, 37)
            .unwrap();
        for hours_west in [4, 5] {
            let instant = chrono::FixedOffset::west_opt(hours_west * 3600)
                .unwrap()
                .from_local_datetime(&local_time)
                .single()
                .unwrap()
                .timestamp();
            assert_eq!(
                completion_timestamp(&changed, Some(&baseline), Some(instant)),
                Ok(instant)
            );
        }
    }

    #[test]
    fn edited_completion_fields_reject_invalid_dates_and_keep_explicit_seconds() {
        let mut fields = CompletionFields {
            activity: 1,
            description: String::new(),
            date: "2026-10-28".into(),
            time: "09:30:17".into(),
        };
        assert_eq!(
            completion_timestamp(&fields, None, None)
                .unwrap()
                .rem_euclid(60),
            17
        );
        fields.date = "2026-02-30".into();
        assert!(completion_timestamp(&fields, None, None).is_err());
        fields.date = "2026-10-28".into();
        fields.time = "24:01".into();
        assert!(completion_timestamp(&fields, None, None).is_err());
    }
}
