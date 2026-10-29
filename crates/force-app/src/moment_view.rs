use crate::{
    model::ForceView,
    view::{
        CONTROL_LINE, ControlState, DESTRUCTIVE, INK, LINE, MUTED, PAPER, button, control_icon,
        control_label, field, field_label, primary_button,
    },
};
use chrono::{Local, TimeZone};
use force_core::Moment;
use gpui::{prelude::*, *};
use gpui_component::{IconName, button::ButtonVariants};

impl ForceView {
    pub(crate) fn moment_editor(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let width = (f32::from(window.viewport_size().width) - 48.).min(440.);
        div()
            .id("moment-overlay")
            .debug_selector(|| "moment-overlay".into())
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
                    .id("moment-editor")
                    .track_focus(&self.moments.focus)
                    .key_context("MomentEditor")
                    .on_action(cx.listener(
                        |this, _: &crate::moment_model::NextField, window, cx| {
                            this.cycle_moment_focus(false, window, cx);
                        },
                    ))
                    .on_action(cx.listener(
                        |this, _: &crate::moment_model::PreviousField, window, cx| {
                            this.cycle_moment_focus(true, window, cx);
                        },
                    ))
                    .capture_action(cx.listener(
                        |this, _: &gpui_component::input::IndentInline, window, cx| {
                            this.cycle_moment_focus(false, window, cx);
                            cx.stop_propagation();
                        },
                    ))
                    .capture_action(cx.listener(
                        |this, _: &gpui_component::input::OutdentInline, window, cx| {
                            this.cycle_moment_focus(true, window, cx);
                            cx.stop_propagation();
                        },
                    ))
                    .debug_selector(|| "moment-editor".into())
                    .w(px(width))
                    .p(px(20.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(CONTROL_LINE))
                    .bg(rgb(PAPER))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                        if !this.time_picker.read(cx).is_open() {
                            this.suspend_moment(window, cx);
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(if self.moments.editing.is_some() {
                                        "编辑随记"
                                    } else {
                                        "随手记"
                                    }),
                            )
                            .child(
                                button("close-moment")
                                    .debug_selector(|| "close-moment".into())
                                    .custom(crate::view::quiet_variant(cx))
                                    .size(px(28.))
                                    .icon(control_icon(IconName::Close, MUTED))
                                    .tooltip("关闭并保留草稿")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.suspend_moment(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "moment-content".into())
                            .w_full()
                            .child(Styled::h(
                                field(&self.moments.content).h(px(120.)),
                                px(120.),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(field_label("日期"))
                                    .child(
                                        div()
                                            .debug_selector(|| "moment-date".into())
                                            .child(field(&self.moments.date)),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(148.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(field_label("时刻"))
                                    .child(
                                        div()
                                            .debug_selector(|| "moment-time".into())
                                            .flex()
                                            .items_center()
                                            .gap(px(6.))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .child(field(&self.moments.time)),
                                            )
                                            .child(
                                                crate::time_picker::trigger(
                                                    "moment-time-picker",
                                                    cx,
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    let input = this.moments.time.clone();
                                                    this.time_picker.update(cx, |picker, cx| {
                                                        picker.open_for(
                                                            input, "时刻", false, window, cx,
                                                        )
                                                    });
                                                })),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .min_h(px(20.))
                            .text_size(px(13.))
                            .text_color(rgb(if self.moments.error.is_some() {
                                DESTRUCTIVE
                            } else {
                                MUTED
                            }))
                            .debug_selector(|| "moment-message".into())
                            .child(
                                self.moments
                                    .error
                                    .clone()
                                    .unwrap_or_else(|| "只记这一刻，不计入专注时长。".to_owned()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                primary_button("save-moment")
                                    .debug_selector(|| "save-moment".into())
                                    .h(px(36.))
                                    .px(px(18.))
                                    .unavailable(self.storage_failed)
                                    .child(control_label("保存", PAPER))
                                    .tooltip(crate::view::shortcut("保存", "Enter", "Enter"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_moment(window, cx);
                                    })),
                            )
                            .child(
                                button("cancel-moment")
                                    .debug_selector(|| "cancel-moment".into())
                                    .custom(crate::view::quiet_variant(cx))
                                    .h(px(36.))
                                    .px(px(12.))
                                    .child(control_label("取消", MUTED))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.cancel_moment(window, cx)
                                    })),
                            )
                            .when_some(self.moments.editing, |row, id| {
                                row.child(div().flex_1()).child(
                                    button("delete-moment-editor")
                                        .debug_selector(|| "delete-moment-editor".into())
                                        .custom(crate::view::quiet_variant(cx))
                                        .h(px(36.))
                                        .px(px(12.))
                                        .unavailable(self.storage_failed)
                                        .child(control_label("删除", DESTRUCTIVE))
                                        .on_click(cx.listener(
                                            move |this, event: &ClickEvent, window, cx| {
                                                if event.click_count() == 1 {
                                                    this.delete_moment(id, window, cx)
                                                }
                                            },
                                        )),
                                )
                            }),
                    ),
            )
    }

    pub(crate) fn moment_row(&self, moment: &Moment, cx: &mut Context<Self>) -> Div {
        let edit = moment.clone();
        let id = moment.id;
        let time = Local
            .timestamp_opt(moment.occurred_at, 0)
            .single()
            .map(|stamp| stamp.format("%H:%M").to_string())
            .unwrap_or_default();
        div()
            .debug_selector(move || format!("moment-row-{id}"))
            .w_full()
            .min_w_0()
            .py(px(10.))
            .border_b_1()
            .border_color(rgb(LINE))
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(div().size(px(5.)).rounded_full().bg(rgb(MUTED)))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(time),
                    )
                    .child(
                        button(("edit-moment", id as u64))
                            .debug_selector(move || format!("edit-moment-{id}"))
                            .custom(crate::view::quiet_variant(cx))
                            .size(px(26.))
                            .icon(control_icon(crate::view::action_icon("pencil"), MUTED))
                            .tooltip("编辑随记")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_moment(edit.clone(), window, cx)
                            })),
                    )
                    .child(
                        button(("delete-moment", id as u64))
                            .debug_selector(move || format!("delete-moment-{id}"))
                            .custom(crate::view::quiet_variant(cx))
                            .size(px(26.))
                            .icon(control_icon(crate::view::action_icon("trash"), MUTED))
                            .tooltip("删除随记")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.delete_moment(id, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .whitespace_normal()
                    .text_size(px(14.))
                    .text_color(rgb(INK))
                    .child(moment.content.clone()),
            )
    }
}
