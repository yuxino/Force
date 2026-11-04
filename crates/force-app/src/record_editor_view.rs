use force_core::TagKind;
use gpui::{prelude::*, *};
use gpui_component::{IconName, button::ButtonVariants};

use crate::{
    creation::CreationKind,
    editor_dialog,
    model::ForceView,
    record_editor_model::RecordIntervalInputs,
    view::{
        ControlState, INK, LINE, MUTED, PAPER, button, control_icon, control_label, field,
        field_label, outlined_button, primary_button,
    },
};

impl ForceView {
    pub(crate) fn record_editor(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let creating = self.record_editor.original.is_none();
        let no_activities = creating && !self.tags.iter().any(|tag| tag.kind == TagKind::Focus);
        let panel =
            editor_dialog::panel("record-editor", window)
                .track_focus(&self.record_editor.focus)
                .key_context("RecordEditor")
                // Single-line inputs propagate Enter. Consume ordinary Return here
                // so it cannot fall through to text input as a newline.
                .on_action(cx.listener(
                    |this, action: &gpui_component::input::Enter, window, cx| {
                        if action.secondary {
                            this.save_record_editor(window, cx);
                        }
                        cx.stop_propagation();
                    },
                ))
                .on_action(cx.listener(
                    |this, _: &crate::record_editor_model::NextField, window, cx| {
                        this.cycle_record_editor_focus(false, window, cx)
                    },
                ))
                .on_action(cx.listener(
                    |this, _: &crate::record_editor_model::PreviousField, window, cx| {
                        this.cycle_record_editor_focus(true, window, cx)
                    },
                ))
                .capture_action(cx.listener(
                    |this, _: &gpui_component::input::IndentInline, window, cx| {
                        this.cycle_record_editor_focus(false, window, cx);
                        cx.stop_propagation();
                    },
                ))
                .capture_action(cx.listener(
                    |this, _: &gpui_component::input::OutdentInline, window, cx| {
                        this.cycle_record_editor_focus(true, window, cx);
                        cx.stop_propagation();
                    },
                ))
                .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if event.position.y < px(68.) {
                        return;
                    }
                    if !this.time_picker.read(cx).is_open()
                        && !this.record_editor.picker.read(cx).is_open()
                    {
                        this.suspend_record_editor(window, cx);
                    }
                }))
                .child(editor_dialog::heading(
                    if creating { "新建" } else { "编辑记录" },
                    button("close-record-editor")
                        .debug_selector(|| "close-record-editor".into())
                        .custom(crate::view::quiet_variant(cx))
                        .size(px(26.))
                        .icon(control_icon(IconName::Close, MUTED))
                        .tooltip("关闭并保留草稿")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.suspend_record_editor(window, cx)
                        })),
                ))
                .when(creating, |panel| {
                    panel.child(self.creation_tabs(CreationKind::Record, cx))
                })
                .child(
                    editor_dialog::fields("record-editor-scroll", &self.record_editor.scroll)
                        .child(record_field("活动").child(if no_activities {
                            outlined_button("record-editor-add-activity")
                                .debug_selector(|| "record-editor-add-activity".into())
                                .h(px(40.))
                                .w_full()
                                .child(control_label("添加活动", INK))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_tag_settings(window, cx)
                                }))
                                .into_any_element()
                        } else {
                            div()
                                .debug_selector(|| "record-editor-activity".into())
                                .w_full()
                                .h(px(40.))
                                .flex_shrink_0()
                                .child(self.record_editor.picker.clone())
                                .into_any_element()
                        }))
                        .child(
                            record_field("说明（可选）").child(
                                div()
                                    .debug_selector(|| "record-editor-description".into())
                                    .child(field(&self.record_editor.description)),
                            ),
                        )
                        .children(self.record_editor.intervals.iter().enumerate().map(
                            |(index, interval)| self.record_interval_fields(index, interval, cx),
                        ))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(MUTED))
                                .whitespace_normal()
                                .child(self.record_editor_time_hint()),
                        ),
                )
                .child(editor_dialog::validation(
                    "record-editor-error",
                    self.record_editor.error.as_deref(),
                ))
                .child(
                    editor_dialog::footer("record-editor-actions")
                        .child(
                            button("cancel-record-editor")
                                .debug_selector(|| "cancel-record-editor".into())
                                .custom(crate::view::quiet_variant(cx))
                                .h(px(36.))
                                .px(px(12.))
                                .child(control_label("取消", MUTED))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_record_editor(window, cx)
                                })),
                        )
                        .child(
                            primary_button("save-record-editor")
                                .debug_selector(|| "save-record-editor".into())
                                .h(px(36.))
                                .px(px(18.))
                                .unavailable(self.storage_failed || no_activities)
                                .child(control_label("保存", PAPER))
                                .tooltip(crate::view::shortcut("保存", "Enter", "Enter"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.save_record_editor(window, cx);
                                })),
                        ),
                );
        editor_dialog::overlay("record-editor-overlay", window, panel)
    }

    fn record_interval_fields(
        &self,
        index: usize,
        interval: &RecordIntervalInputs,
        cx: &mut Context<Self>,
    ) -> Div {
        let key = interval.key;
        div()
            .debug_selector(move || format!("record-editor-interval-{index}"))
            .flex_shrink_0()
            .pt(px(12.))
            .border_t_1()
            .border_color(rgb(LINE))
            .flex()
            .flex_col()
            .gap(px(10.))
            .when(self.record_editor.intervals.len() > 1, |section| {
                section.child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(format!("时段 {}", index + 1)),
                )
            })
            .children(
                [
                    (
                        "start",
                        "开始",
                        interval.start_date.clone(),
                        interval.start_time.clone(),
                    ),
                    (
                        "end",
                        "结束",
                        interval.end_date.clone(),
                        interval.end_time.clone(),
                    ),
                ]
                .into_iter()
                .map(|(kind, label, date, time)| {
                    let target = time.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(field_label(label).w(px(32.)).flex_shrink_0())
                        .child(
                            div()
                                .debug_selector(move || {
                                    format!("record-editor-{kind}-date-{index}")
                                })
                                .flex_1()
                                .min_w_0()
                                .child(field(&date)),
                        )
                        .child(
                            div()
                                .debug_selector(move || {
                                    format!("record-editor-{kind}-time-{index}")
                                })
                                .w(px(104.))
                                .flex_shrink_0()
                                .child(field(&time)),
                        )
                        .child(
                            button((
                                if kind == "start" {
                                    "record-start-picker"
                                } else {
                                    "record-end-picker"
                                },
                                key,
                            ))
                            .debug_selector(move || format!("record-editor-{kind}-picker-{index}"))
                            .custom(crate::view::quiet_variant(cx))
                            .size(px(32.))
                            .tooltip("用表盘选择时间")
                            .child(
                                div()
                                    .relative()
                                    .size(px(16.))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(MUTED))
                                    .child(
                                        div()
                                            .absolute()
                                            .left(px(7.))
                                            .top(px(3.))
                                            .w(px(1.))
                                            .h(px(5.))
                                            .bg(rgb(MUTED)),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .left(px(7.))
                                            .top(px(7.))
                                            .w(px(4.))
                                            .h(px(1.))
                                            .bg(rgb(MUTED)),
                                    ),
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.time_picker.update(cx, |picker, cx| {
                                        picker.open_for(target.clone(), label, false, window, cx)
                                    });
                                },
                            )),
                        )
                }),
            )
    }
}

fn record_field(label: &'static str) -> Div {
    div()
        .w_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(field_label(label))
}
