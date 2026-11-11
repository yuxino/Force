use crate::{
    model::ForceView,
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, PAPER, button, control_icon, control_label,
        field, field_label, primary_button,
    },
};
use chrono::{Local, TimeZone};
use force_core::Moment;
use gpui::{prelude::*, *};
use gpui_component::{IconName, button::ButtonVariants};

impl ForceView {
    pub(crate) fn moment_editor(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let editing = self.moments.is_editing();
        let day_only = self.moments.day_only;
        let content_height = px(if day_only {
            80.
        } else if editing {
            200.
        } else {
            192.
        });
        let (precision_id, precision_label) = if day_only {
            ("moment-exact", "填具体时刻")
        } else {
            ("moment-day-only", "只记日期")
        };
        let close = button("close-moment")
            .debug_selector(|| "close-moment".into())
            .custom(crate::view::quiet_variant(cx))
            .size(px(28.))
            .icon(control_icon(IconName::Close, MUTED))
            .tooltip("关闭并保留草稿")
            .on_click(cx.listener(|this, _, window, cx| this.suspend_moment(window, cx)));
        let fields =
            crate::editor_dialog::fields("moment-editor-scroll", &self.moments.editor_scroll)
                .debug_selector(|| "moment-editor-fields".into())
                .child(
                    div()
                        .debug_selector(|| "moment-content".into())
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(field_label("内容"))
                                .when(!editing, |row| {
                                    row.child(
                                        button(precision_id)
                                            .debug_selector(move || precision_id.into())
                                            .custom(crate::view::quiet_variant(cx))
                                            .h(px(24.))
                                            .px(px(8.))
                                            .child(control_label(precision_label, MUTED))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.moments.day_only = !day_only;
                                                this.moments.error = None;
                                                cx.notify();
                                            })),
                                    )
                                }),
                        )
                        .child(Styled::h(
                            field(&self.moments.content).h(content_height),
                            content_height,
                        )),
                )
                .child(
                    div()
                        .flex_shrink_0()
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
                        .when(!day_only, |row| {
                            row.child(
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
                            )
                        })
                        .when(day_only, |row| {
                            row.child(
                                div()
                                    .debug_selector(|| "moment-duration".into())
                                    .w(px(168.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(field_label("时长（分钟，可选）"))
                                    .child(field(&self.moments.minutes)),
                            )
                        }),
                );
        let footer = crate::editor_dialog::footer("moment-editor-actions")
            .when_some(self.moments.editing, |row, id| {
                row.child(
                    button("delete-moment-editor")
                        .debug_selector(|| "delete-moment-editor".into())
                        .custom(crate::view::quiet_variant(cx))
                        .h(px(36.))
                        .px(px(12.))
                        .unavailable(self.storage_failed)
                        .child(control_label("删除", DESTRUCTIVE))
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            if event.click_count() == 1 {
                                this.delete_moment(id, window, cx);
                            }
                        })),
                )
                .child(div().flex_1())
            })
            .when_some(self.moments.daily_original.as_ref(), |row, original| {
                let original = original.clone();
                row.child(
                    button("delete-daily-entry")
                        .debug_selector(|| "delete-daily-entry".into())
                        .custom(crate::view::quiet_variant(cx))
                        .h(px(36.))
                        .px(px(12.))
                        .unavailable(self.storage_failed)
                        .child(control_label("删除", DESTRUCTIVE))
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            if event.click_count() == 1 {
                                this.delete_daily_entry(original.clone(), window, cx);
                            }
                        })),
                )
                .child(div().flex_1())
            })
            .child(
                button("cancel-moment")
                    .debug_selector(|| "cancel-moment".into())
                    .custom(crate::view::quiet_variant(cx))
                    .h(px(36.))
                    .child(control_label("取消", MUTED))
                    .on_click(cx.listener(|this, _, window, cx| this.cancel_moment(window, cx))),
            )
            .child(
                primary_button("save-moment")
                    .debug_selector(|| "save-moment".into())
                    .h(px(36.))
                    .unavailable(self.storage_failed)
                    .child(control_label("保存", PAPER))
                    .tooltip(crate::view::shortcut("保存", "Enter", "Enter"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.save_moment(window, cx);
                    })),
            );
        let panel = crate::editor_dialog::panel("moment-editor", window)
            .track_focus(&self.moments.focus)
            .key_context("MomentEditor")
            .on_action(
                cx.listener(|this, _: &crate::moment_model::NextField, window, cx| {
                    this.cycle_moment_focus(false, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::moment_model::PreviousField, window, cx| {
                    this.cycle_moment_focus(true, window, cx);
                }),
            )
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
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                if event.position.y < px(68.) {
                    return;
                }
                if !this.time_picker.read(cx).is_open() {
                    this.suspend_moment(window, cx);
                }
            }))
            .child(crate::editor_dialog::heading(
                if self.moments.daily_original.is_some() {
                    "编辑按日记录"
                } else if editing {
                    "编辑随记"
                } else {
                    "新建"
                },
                close,
            ))
            .when(!editing, |panel| {
                panel.child(self.creation_tabs(crate::creation::CreationKind::Moment, cx))
            })
            .child(fields)
            .child(crate::editor_dialog::validation(
                "moment-message",
                self.moments.error.as_deref(),
            ))
            .child(footer);
        crate::editor_dialog::overlay("moment-overlay", window, panel)
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
