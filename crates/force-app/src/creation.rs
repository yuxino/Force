//! One creation surface; existing editors retain their own persistence and drafts.
use chrono::{Local, NaiveDate};
use force_core::TagKind;
use gpui::{prelude::*, *};

use crate::{
    model::{ForceView, Page},
    view::{INK, MUTED, SURFACE, control_label, selection_button},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CreationKind {
    Task,
    Plan,
    Record,
    Moment,
}

pub(crate) struct CreationSeed {
    kind: CreationKind,
    activity: i64,
    title: String,
    date: Option<NaiveDate>,
}

impl ForceView {
    fn visible_creation(&self) -> Option<CreationKind> {
        if self.task_editor.visible {
            Some(CreationKind::Task)
        } else if self.agenda.editor_visible && self.agenda.edit_id.is_none() {
            Some(CreationKind::Plan)
        } else if self.record_editor.visible && self.record_editor.original.is_none() {
            Some(CreationKind::Record)
        } else if self.moments.visible && !self.moments.is_editing() {
            Some(CreationKind::Moment)
        } else {
            None
        }
    }

    pub(crate) fn open_creation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_creation_seed();
        self.show_manager(window, cx);
        match self.visible_creation() {
            Some(CreationKind::Task) => self.focus_task_dialog(window, cx),
            Some(CreationKind::Plan) => self.focus_agenda_editor(window, cx),
            Some(CreationKind::Record) => self.focus_record_editor(window, cx),
            Some(CreationKind::Moment) => self
                .moments
                .content
                .update(cx, |input, cx| input.focus(window, cx)),
            None => self.open_task_dialog(window, cx),
        }
    }

    pub(crate) fn creation_tabs(&self, selected: CreationKind, cx: &mut Context<Self>) -> Div {
        div()
            .debug_selector(|| "creation-types".into())
            .h(px(38.))
            .w_full()
            .flex_shrink_0()
            .p(px(2.))
            .rounded(px(6.))
            .bg(rgb(SURFACE))
            .flex()
            .gap(px(2.))
            .children(
                [
                    (
                        CreationKind::Task,
                        "任务",
                        "creation-type-task",
                        "保存待做任务，或开始计时",
                    ),
                    (
                        CreationKind::Plan,
                        "安排",
                        "creation-type-plan",
                        "安排日期和时间，不计入实际时长",
                    ),
                    (
                        CreationKind::Record,
                        "补记",
                        "creation-type-record",
                        "补记已经做过的任务和时段",
                    ),
                    (
                        CreationKind::Moment,
                        "随手记",
                        "creation-type-moment",
                        "记录一个时刻，不计入实际时长",
                    ),
                ]
                .into_iter()
                .map(|(kind, label, id, help)| {
                    selection_button(id, selected == kind, cx)
                        .debug_selector(move || id.into())
                        .h(px(34.))
                        .flex_1()
                        .child(control_label(
                            label,
                            if selected == kind { INK } else { MUTED },
                        ))
                        .tooltip(help)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.switch_creation(kind, window, cx)
                        }))
                }),
            )
    }

    pub(crate) fn switch_creation(
        &mut self,
        kind: CreationKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source = self.visible_creation();
        if source == Some(kind) {
            return;
        }
        let (activity, title, date) = match source {
            Some(CreationKind::Task) => (
                self.task_editor.selected_tag,
                self.task_editor.title_input.read(cx).value().to_string(),
                None,
            ),
            Some(CreationKind::Plan) => (
                self.agenda.draft_tag,
                self.agenda.title_input.read(cx).value().to_string(),
                parse_date(self.agenda.date_input.read(cx).value().as_ref()),
            ),
            Some(CreationKind::Record) => (
                self.record_editor.activity,
                self.record_editor.description.read(cx).value().to_string(),
                self.record_editor
                    .intervals
                    .first()
                    .and_then(|i| parse_date(i.start_date.read(cx).value().as_ref())),
            ),
            Some(CreationKind::Moment) => {
                let content = self.moments.content.read(cx).value().to_string();
                let title =
                    if content.chars().count() <= 200 && !content.chars().any(char::is_control) {
                        content
                    } else {
                        String::new()
                    };
                (
                    0,
                    title,
                    parse_date(self.moments.date.read(cx).value().as_ref()),
                )
            }
            None => (0, String::new(), None),
        };
        let date = date.or_else(|| (self.page == Page::Records).then_some(self.record_date));
        self.creation_seed = Some(CreationSeed {
            kind,
            activity,
            title,
            date,
        });
        match kind {
            CreationKind::Task => self.open_task_dialog(window, cx),
            CreationKind::Plan => {
                if self.agenda.editing && self.agenda.edit_id.is_none() {
                    self.clear_creation_seed();
                    self.resume_agenda_draft(window, cx);
                } else {
                    self.begin_today_plan(window, cx);
                }
            }
            CreationKind::Record => {
                if !self.record_editor.active {
                    self.record_date = date
                        .unwrap_or_else(|| {
                            if self.page == Page::Records {
                                self.record_date
                            } else {
                                Local::now().date_naive()
                            }
                        })
                        .min(Local::now().date_naive());
                }
                self.begin_record_entry(window, cx);
            }
            CreationKind::Moment => self.begin_moment(date, window, cx),
        }
        cx.notify();
    }

    pub(crate) fn creation_date(&self, kind: CreationKind) -> Option<NaiveDate> {
        self.creation_seed
            .as_ref()
            .filter(|seed| seed.kind == kind)
            .and_then(|seed| seed.date)
    }

    pub(crate) fn clear_creation_seed(&mut self) {
        self.creation_seed = None;
    }

    /// Called only when a destination is new; restored drafts are never overwritten.
    pub(crate) fn seed_creation(
        &mut self,
        kind: CreationKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .creation_seed
            .as_ref()
            .is_none_or(|seed| seed.kind != kind)
        {
            return;
        }
        let seed = self.creation_seed.take().expect("matching creation seed");
        let activity = self
            .tags
            .iter()
            .find(|tag| tag.id == seed.activity && tag.kind == TagKind::Focus);
        let tag = activity.map(|tag| tag.id);
        let title = if seed.title.trim().is_empty() {
            activity.map(|tag| tag.name.clone()).unwrap_or_default()
        } else {
            seed.title.clone()
        };
        match kind {
            CreationKind::Task => {
                if self.task_editor.selected_tag != 0
                    || !self.task_editor.title_input.read(cx).value().is_empty()
                    || self.task_editor.countdown
                {
                    return;
                }
                if let Some(tag) = tag {
                    self.task_editor.selected_tag = tag;
                }
                self.task_editor
                    .title_input
                    .update(cx, |input, cx| input.set_value(seed.title, window, cx));
            }
            CreationKind::Plan => {
                if self.agenda.draft_task.is_some() {
                    return;
                }
                if let Some(tag) = tag {
                    self.agenda.draft_tag = tag;
                }
                self.agenda
                    .title_input
                    .update(cx, |input, cx| input.set_value(title, window, cx));
                if let Some(date) = seed.date {
                    self.agenda.date_input.update(cx, |input, cx| {
                        input.set_value(date.format("%Y-%m-%d").to_string(), window, cx)
                    });
                }
            }
            CreationKind::Record => {
                if let Some(tag) = tag {
                    self.record_editor.activity = tag;
                }
                self.record_editor
                    .description
                    .update(cx, |input, cx| input.set_value(seed.title, window, cx));
            }
            CreationKind::Moment => {
                self.moments
                    .content
                    .update(cx, |input, cx| input.set_value(title, window, cx));
            }
        }
    }
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()
}
