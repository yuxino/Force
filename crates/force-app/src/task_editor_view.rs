//! New task entry uses the same editor chrome as notes and arrangements.
use force_core::TagKind;
use gpui::{prelude::*, *};
use gpui_component::{IconName, button::ButtonVariants};

use crate::{
    editor_dialog,
    model::{ForceView, task_title_error_for},
    view::{
        ControlState, DESTRUCTIVE, INK, MUTED, SURFACE, button, control_icon, control_label, field,
        field_label, outlined_button, selection_button,
    },
};

impl ForceView {
    pub(crate) fn task_dialog(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let no_activities = !self.tags.iter().any(|tag| tag.kind == TagKind::Focus);
        let save_enabled = self.can_save_task_dialog(cx);
        let start_enabled = self.can_start_task_dialog(cx);
        let switching = self.active.iter().any(|session| session.is_running());
        let panel = editor_dialog::panel("task-editor", window)
            .track_focus(&self.task_editor.focus)
            .key_context("TaskEditor")
            .on_action(cx.listener(
                |this, _: &crate::task_editor_model::NextField, window, cx| {
                    this.cycle_task_dialog_focus(false, window, cx);
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::task_editor_model::PreviousField, window, cx| {
                    this.cycle_task_dialog_focus(true, window, cx);
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_component::input::IndentInline, window, cx| {
                    this.cycle_task_dialog_focus(false, window, cx);
                    cx.stop_propagation();
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_component::input::OutdentInline, window, cx| {
                    this.cycle_task_dialog_focus(true, window, cx);
                    cx.stop_propagation();
                },
            ))
            .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                if !this.task_editor.tag_picker.read(cx).is_open() {
                    this.suspend_task_dialog(window, cx);
                }
            }))
            .child(editor_dialog::heading(
                "新任务",
                button("close-task-dialog")
                    .debug_selector(|| "close-task-dialog".into())
                    .custom(crate::view::quiet_variant(cx))
                    .size(px(26.))
                    .icon(control_icon(IconName::Close, MUTED))
                    .tooltip("关闭并保留草稿")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.suspend_task_dialog(window, cx)),
                    ),
            ))
            .child(
                editor_dialog::fields("task-editor-fields", &self.task_editor.scroll)
                    .child(task_field("活动").child(if no_activities {
                        outlined_button("task-dialog-add-activity")
                            .debug_selector(|| "task-dialog-add-activity".into())
                            .h(px(40.))
                            .w_full()
                            .child(control_label("添加活动", INK))
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.open_tag_settings(window, cx)
                                }),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .h(px(40.))
                            .w_full()
                            .child(self.task_editor.tag_picker.clone())
                            .into_any_element()
                    }))
                    .child(
                        task_field("说明（可选）")
                            .child(
                                div()
                                    .debug_selector(|| "task-editor-description".into())
                                    .child(field(&self.task_editor.title_input)),
                            )
                            .when_some(
                                task_title_error_for(
                                    self.task_editor.title_input.read(cx).value().as_ref(),
                                    false,
                                ),
                                |body, error| {
                                    body.child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(rgb(DESTRUCTIVE))
                                            .child(error),
                                    )
                                },
                            ),
                    )
                    .child(
                        task_field("计时方式").child(
                            div()
                                .debug_selector(|| "task-editor-mode".into())
                                .w(px(220.))
                                .h(px(40.))
                                .p(px(2.))
                                .rounded(px(6.))
                                .bg(rgb(SURFACE))
                                .flex()
                                .children([(false, "正计时"), (true, "倒计时")].into_iter().map(
                                    |(countdown, label)| {
                                        let selected = self.task_editor.countdown == countdown;
                                        let id = if countdown {
                                            "task-dialog-duration-custom"
                                        } else {
                                            "task-dialog-duration-0"
                                        };
                                        selection_button(id, selected, cx)
                                            .debug_selector(move || id.into())
                                            .h(px(36.))
                                            .flex_1()
                                            .px(px(8.))
                                            .child(control_label(
                                                label,
                                                if selected { INK } else { MUTED },
                                            ))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                if countdown {
                                                    this.task_editor.countdown = true;
                                                    this.task_editor.error = None;
                                                    this.task_editor
                                                        .timer_input
                                                        .update(cx, |input, cx| {
                                                            input.focus(window, cx)
                                                        });
                                                    cx.notify();
                                                } else {
                                                    this.choose_task_dialog_duration(
                                                        None, window, cx,
                                                    );
                                                }
                                            }))
                                    },
                                )),
                        ),
                    )
                    .when(self.task_editor.countdown, |body| {
                        body.child(
                            task_field("本轮时长")
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .debug_selector(|| "task-editor-duration".into())
                                                .w(px(96.))
                                                .flex_shrink_0()
                                                .child(field(&self.task_editor.timer_input)),
                                        )
                                        .child(control_label("分钟", MUTED).flex_shrink_0())
                                        .child(div().ml(px(8.)).flex().gap(px(6.)).children(
                                            [25_u32, 45, 60].into_iter().map(|minutes| {
                                                let selected = self
                                                    .task_editor
                                                    .timer_input
                                                    .read(cx)
                                                    .value()
                                                    .trim()
                                                    .parse::<u32>()
                                                    .ok()
                                                    == Some(minutes);
                                                selection_button(
                                                    ("task-dialog-duration", u64::from(minutes)),
                                                    selected,
                                                    cx,
                                                )
                                                .debug_selector(move || {
                                                    format!("task-dialog-duration-{minutes}")
                                                })
                                                .h(px(32.))
                                                .w(px(44.))
                                                .px(px(4.))
                                                .child(control_label(
                                                    minutes.to_string(),
                                                    if selected { INK } else { MUTED },
                                                ))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.choose_task_dialog_duration(
                                                            Some(minutes),
                                                            window,
                                                            cx,
                                                        )
                                                    },
                                                ))
                                            }),
                                        )),
                                )
                                .when_some(self.task_dialog_duration_error(cx), |body, error| {
                                    body.child(
                                        div()
                                            .debug_selector(|| "task-editor-duration-error".into())
                                            .text_size(px(13.))
                                            .text_color(rgb(DESTRUCTIVE))
                                            .child(error),
                                    )
                                }),
                        )
                    }),
            )
            .child(editor_dialog::validation(
                "task-editor-validation",
                self.task_editor.error.as_deref(),
            ))
            .child(
                editor_dialog::footer("task-editor-actions")
                    .child(
                        button("cancel-task-dialog")
                            .debug_selector(|| "cancel-task-dialog".into())
                            .custom(crate::view::quiet_variant(cx))
                            .h(px(36.))
                            .child(control_label("取消", MUTED))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel_task_dialog(window, cx)
                            })),
                    )
                    .child(
                        outlined_button("save-task-dialog")
                            .debug_selector(|| "save-task-dialog".into())
                            .h(px(36.))
                            .unavailable(!save_enabled)
                            .child(control_label(
                                "保存任务",
                                if save_enabled { INK } else { MUTED },
                            ))
                            .tooltip("保存任务，稍后再开始计时（Enter）")
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.save_task_dialog(window, cx)
                                }),
                            ),
                    )
                    .child(
                        crate::control_style::start_button(
                            "start-task-dialog",
                            if switching {
                                "切换并开始"
                            } else {
                                "开始计时"
                            },
                            crate::view::action_icon("play").size(px(14.)),
                            !start_enabled,
                        )
                        .debug_selector(|| "start-task-dialog".into())
                        .h(px(36.))
                        .tooltip(if switching {
                            "暂停当前计时或结束当前休息，再开始新任务；已计时间保留"
                        } else {
                            "保存新任务并开始本次计时"
                        })
                        .on_click(
                            cx.listener(|this, _, window, cx| this.start_task_dialog(window, cx)),
                        ),
                    ),
            );
        editor_dialog::overlay("task-editor-overlay", window, panel)
    }
}

fn task_field(label: &'static str) -> Div {
    div()
        .w_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(field_label(label))
}
