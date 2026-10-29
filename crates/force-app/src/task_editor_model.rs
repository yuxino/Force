//! A new-task draft stays independent from the contextual task/timer workspace.
use chrono::Local;
use force_core::TagKind;
use gpui::{AppContext as _, Context, Entity, Focusable as _, ScrollHandle, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    model::{ForceView, TaskDestination, parse_goal_draft, task_title_error_for},
    tag_picker::{FocusTagOption, TagPicked, TagPicker},
};

gpui::actions!(task_editor, [NextField, PreviousField]);

pub(crate) struct TaskDialogState {
    pub(crate) focus: gpui::FocusHandle,
    pub(crate) visible: bool,
    pub(crate) title_input: Entity<InputState>,
    pub(crate) timer_input: Entity<InputState>,
    pub(crate) tag_picker: Entity<TagPicker>,
    pub(crate) selected_tag: i64,
    pub(crate) countdown: bool,
    pub(crate) timer_minutes: u32,
    pub(crate) error: Option<String>,
    pub(crate) scroll: ScrollHandle,
    pub(crate) generation: u64,
    options: Vec<FocusTagOption>,
    _subscriptions: Vec<Subscription>,
}

impl TaskDialogState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ForceView>) -> Self {
        cx.bind_keys([
            gpui::KeyBinding::new("tab", NextField, Some("TaskEditor")),
            gpui::KeyBinding::new("shift-tab", PreviousField, Some("TaskEditor")),
        ]);
        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("补充说明（可选）"));
        let timer_input = cx.new(|cx| InputState::new(window, cx).default_value("25"));
        let tag_picker = cx.new(TagPicker::new);
        let mut subscriptions = Vec::new();
        for input in [&title_input, &timer_input] {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.task_editor.error = None;
                        if let Some(minutes) =
                            parse_goal_draft(this.task_editor.timer_input.read(cx).value().as_ref())
                        {
                            this.task_editor.timer_minutes = minutes;
                        }
                        cx.notify();
                    } else if matches!(event, InputEvent::PressEnter { secondary: false })
                        && this.task_editor.visible
                        && !this.compact
                    {
                        this.save_task_dialog(window, cx);
                    }
                },
            ));
        }
        subscriptions.push(cx.subscribe(&tag_picker, |this, _, event: &TagPicked, cx| {
            if this.task_editor.visible
                && !this.compact
                && !this.storage_failed
                && this
                    .tags
                    .iter()
                    .any(|tag| tag.id == event.0 && tag.kind == TagKind::Focus)
            {
                this.task_editor.selected_tag = event.0;
                this.task_editor.error = None;
                cx.notify();
            }
        }));
        Self {
            focus: cx.focus_handle().tab_stop(false),
            visible: false,
            title_input,
            timer_input,
            tag_picker,
            selected_tag: 0,
            countdown: false,
            timer_minutes: 25,
            error: None,
            scroll: ScrollHandle::default(),
            generation: 0,
            options: Vec::new(),
            _subscriptions: subscriptions,
        }
    }
}

impl ForceView {
    pub(crate) fn sync_task_dialog_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let options: Vec<_> = self
            .tags
            .iter()
            .filter(|tag| tag.kind == TagKind::Focus)
            .map(|tag| FocusTagOption {
                id: tag.id,
                name: tag.name.clone(),
                color: self.display_tag_color(tag).map(str::to_owned),
            })
            .collect();
        let selected = options
            .iter()
            .find(|tag| tag.id == self.task_editor.selected_tag)
            .map(|tag| tag.id);
        let enabled = self.task_editor.visible && !self.compact && !self.storage_failed;
        self.task_editor.tag_picker.update(cx, |picker, cx| {
            picker.set_trigger_width(crate::editor_dialog::content_width(window), cx);
        });
        let picker = self.task_editor.tag_picker.read(cx);
        if options != self.task_editor.options
            || selected != picker.selected_value()
            || enabled != picker.is_enabled()
        {
            self.task_editor.options = options.clone();
            self.task_editor.tag_picker.update(cx, |picker, cx| {
                picker.synchronize(options, selected, enabled, window, cx)
            });
        }
    }

    pub(crate) fn open_task_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_moment(window, cx);
        self.suspend_record_editor(window, cx);
        self.suspend_completion_editor(window, cx);
        self.suspend_agenda_popup(window, cx);
        if !self.task_editor.visible {
            self.task_editor.generation = self.task_editor.generation.wrapping_add(1);
        }
        self.task_editor.visible = true;
        self.task_editor
            .scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
        self.show_manager(window, cx);
        self.focus_task_dialog(window, cx);
        cx.notify();
    }

    pub(crate) fn focus_task_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.task_editor.selected_tag == 0
            && self.tags.iter().any(|tag| tag.kind == TagKind::Focus)
        {
            self.task_editor.tag_picker.focus_handle(cx).focus(window);
        } else {
            self.task_editor
                .title_input
                .update(cx, |input, cx| input.focus(window, cx));
        }
    }

    pub(crate) fn cycle_task_dialog_focus(
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
            if self.task_editor.focus.contains_focused(window, cx) {
                return;
            }
            if window.focused(cx) == first {
                break;
            }
        }
        self.focus_task_dialog(window, cx);
    }

    pub(crate) fn suspend_task_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.task_editor.visible {
            return;
        }
        self.task_editor.visible = false;
        self.task_editor.generation = self.task_editor.generation.wrapping_add(1);
        self.task_editor
            .tag_picker
            .update(cx, |picker, cx| picker.dismiss(window, false, cx));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn cancel_task_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_task_dialog(window, cx);
        self.task_editor.selected_tag = 0;
        self.task_editor.countdown = false;
        self.task_editor.timer_minutes = 25;
        self.task_editor.error = None;
        self.task_editor
            .title_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.task_editor
            .timer_input
            .update(cx, |input, cx| input.set_value("25", window, cx));
        cx.notify();
    }

    fn task_dialog_title(&self, cx: &gpui::App) -> String {
        let title = self.task_editor.title_input.read(cx).value();
        if title.trim().is_empty() {
            self.tags
                .iter()
                .find(|tag| tag.id == self.task_editor.selected_tag && tag.kind == TagKind::Focus)
                .map_or_else(String::new, |tag| tag.name.clone())
        } else {
            title.trim().to_owned()
        }
    }

    pub(crate) fn task_dialog_form_error(&self, cx: &gpui::App) -> Option<&'static str> {
        if !self
            .tags
            .iter()
            .any(|tag| tag.id == self.task_editor.selected_tag && tag.kind == TagKind::Focus)
        {
            Some(if self.task_editor.selected_tag == 0 {
                "请选择活动。"
            } else {
                "原活动已移除，请重新选择。"
            })
        } else {
            task_title_error_for(&self.task_dialog_title(cx), true)
        }
    }

    pub(crate) fn task_dialog_duration_error(&self, cx: &gpui::App) -> Option<&'static str> {
        (self.task_editor.countdown
            && parse_goal_draft(self.task_editor.timer_input.read(cx).value().as_ref()).is_none())
        .then_some("请输入 1–1440 之间的整数分钟数。")
    }

    pub(crate) fn can_save_task_dialog(&self, cx: &gpui::App) -> bool {
        self.task_editor.visible
            && !self.compact
            && !self.storage_failed
            && self.store.is_some()
            && self.task_dialog_form_error(cx).is_none()
    }

    pub(crate) fn can_start_task_dialog(&self, cx: &gpui::App) -> bool {
        self.can_save_task_dialog(cx) && self.task_dialog_duration_error(cx).is_none()
    }

    pub(crate) fn choose_task_dialog_duration(
        &mut self,
        minutes: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.task_editor.countdown = minutes.is_some();
        if let Some(minutes) = minutes {
            self.task_editor.timer_minutes = minutes;
            self.task_editor.timer_input.update(cx, |input, cx| {
                input.set_value(minutes.to_string(), window, cx)
            });
        }
        self.task_editor.error = None;
        cx.notify();
    }

    pub(crate) fn save_task_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.task_editor.visible || self.compact {
            return;
        }
        if let Some(error) = self.task_dialog_form_error(cx) {
            self.task_editor.error = Some(error.to_owned());
            self.focus_task_dialog(window, cx);
            cx.notify();
            return;
        }
        if self.storage_failed || self.store.is_none() {
            return;
        }
        if self.confirm_task_changes(
            TaskDestination::SaveDialog(self.task_editor.generation),
            window,
            cx,
        ) {
            return;
        }
        let title = self.task_dialog_title(cx);
        let tag = self.task_editor.selected_tag;
        let countdown = self.task_editor.countdown;
        let timer_value = self.task_editor.timer_input.read(cx).value().to_string();
        let minutes = self.task_editor.timer_minutes;
        let mut saved = None;
        if self.write(
            |store| {
                saved = Some(store.create_task(&title, tag, Local::now().timestamp())?);
                Ok(())
            },
            cx,
        ) && let Some(task) = saved
        {
            self.cancel_task_dialog(window, cx);
            self.select_created_task(task.id, window, cx);
            self.countdown = countdown;
            self.timer_minutes = minutes;
            self.timer_input
                .update(cx, |input, cx| input.set_value(timer_value, window, cx));
            self.notify("任务已保存。", cx);
        } else {
            self.task_editor.error = Some("任务保存失败。已有内容已保留，请重试。".to_owned());
            cx.notify();
        }
    }

    pub(crate) fn start_task_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.task_editor.visible || self.compact {
            return;
        }
        if let Some(error) = self
            .task_dialog_form_error(cx)
            .or_else(|| self.task_dialog_duration_error(cx))
        {
            self.task_editor.error = Some(error.to_owned());
            self.focus_task_dialog(window, cx);
            cx.notify();
            return;
        }
        if self.storage_failed || self.store.is_none() {
            return;
        }
        if self.confirm_task_changes(
            TaskDestination::StartDialog(self.task_editor.generation),
            window,
            cx,
        ) {
            return;
        }
        let title = self.task_dialog_title(cx);
        let tag = self.task_editor.selected_tag;
        let target = self.task_editor.countdown.then(|| {
            i64::from(
                parse_goal_draft(self.task_editor.timer_input.read(cx).value().as_ref())
                    .unwrap_or(25),
            ) * 60
        });
        let mut started = None;
        if self.write(
            |store| {
                started = Some(store.create_and_start_task(
                    &title,
                    tag,
                    Local::now().timestamp(),
                    target,
                )?);
                Ok(())
            },
            cx,
        ) && let Some(session) = started
        {
            self.cancel_task_dialog(window, cx);
            self.accept_started(session, window, cx);
        } else {
            self.task_editor.error = Some("无法开始计时。已有内容已保留，请重试。".to_owned());
            cx.notify();
        }
    }
}
