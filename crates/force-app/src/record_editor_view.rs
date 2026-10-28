use gpui::{prelude::*, *};
use gpui_component::{IconName, button::ButtonVariants};

use crate::{
    model::ForceView,
    record_editor_model::RecordIntervalInputs,
    view::{
        CONTROL_LINE, ControlState, DESTRUCTIVE, LINE, MUTED, PAPER, button, control_icon,
        control_label, field, field_label, primary_button,
    },
};

impl ForceView {
    pub(crate) fn record_editor(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let width = (f32::from(window.viewport_size().width) - 48.).min(560.);
        let height = (f32::from(window.viewport_size().height) - 64.).min(660.);
        div()
            .id("record-editor-overlay")
            .debug_selector(|| "record-editor-overlay".into())
            .absolute()
            .inset_0()
            .size_full()
            .occlude()
            .bg(rgba(0x00000016))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("record-editor")
                    .debug_selector(|| "record-editor".into())
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
                    .w(px(width))
                    .max_h(px(height))
                    .p(px(20.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(CONTROL_LINE))
                    .bg(rgb(PAPER))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                        if !this.time_picker.read(cx).is_open()
                            && !this.record_editor.picker.read(cx).is_open()
                        {
                            this.suspend_record_editor(window, cx);
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .flex_shrink_0()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(if self.record_editor.original.is_some() {
                                        "编辑记录"
                                    } else {
                                        "补记"
                                    }),
                            )
                            .child(
                                button("close-record-editor")
                                    .debug_selector(|| "close-record-editor".into())
                                    .custom(crate::view::quiet_variant(cx))
                                    .size(px(28.))
                                    .icon(control_icon(IconName::Close, MUTED))
                                    .tooltip("关闭并保留草稿")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.suspend_record_editor(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("record-editor-scroll")
                            .min_h_0()
                            .debug_selector(|| "record-editor-scroll".into())
                            .max_h(px((height - 162.).max(220.)))
                            .overflow_y_scroll()
                            .track_scroll(&self.record_editor.scroll)
                            .pr(px(4.))
                            .flex()
                            .flex_col()
                            .gap(px(16.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(field_label("活动"))
                                    .child(
                                        div()
                                            .debug_selector(|| "record-editor-activity".into())
                                            .w_full()
                                            .h(px(40.))
                                            .flex_shrink_0()
                                            .child(self.record_editor.picker.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(field_label("说明（可选）"))
                                    .child(
                                        div()
                                            .debug_selector(|| "record-editor-description".into())
                                            .child(field(&self.record_editor.description)),
                                    ),
                            )
                            .children(self.record_editor.intervals.iter().enumerate().map(
                                |(index, interval)| {
                                    self.record_interval_fields(index, interval, cx)
                                },
                            ))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED))
                                    .whitespace_normal()
                                    .child(self.record_editor_time_hint()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .flex_shrink_0()
                            .when_some(self.record_editor.error.as_ref(), |footer, error| {
                                footer.child(
                                    div()
                                        .debug_selector(|| "record-editor-error".into())
                                        .text_size(px(13.))
                                        .text_color(rgb(DESTRUCTIVE))
                                        .whitespace_normal()
                                        .child(error.clone()),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        primary_button("save-record-editor")
                                            .debug_selector(|| "save-record-editor".into())
                                            .h(px(36.))
                                            .px(px(18.))
                                            .unavailable(self.storage_failed)
                                            .child(control_label("保存", PAPER))
                                            .tooltip(crate::view::shortcut(
                                                "保存", "Enter", "Enter",
                                            ))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.save_record_editor(window, cx);
                                            })),
                                    )
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
                                    ),
                            ),
                    ),
            )
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
