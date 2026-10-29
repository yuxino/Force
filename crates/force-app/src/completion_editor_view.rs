use gpui::{prelude::*, *};
use gpui_component::{IconName, button::ButtonVariants};

use crate::{
    model::ForceView,
    view::{
        CONTROL_LINE, ControlState, DESTRUCTIVE, MUTED, PAPER, button, control_icon, control_label,
        field, field_label, primary_button,
    },
};

impl ForceView {
    pub(crate) fn completion_editor(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let width = (f32::from(window.viewport_size().width) - 48.).min(520.);
        let height = (f32::from(window.viewport_size().height) - 64.).min(560.);
        div()
            .id("completion-editor-overlay")
            .debug_selector(|| "completion-editor-overlay".into())
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
                    .id("completion-editor")
                    .debug_selector(|| "completion-editor".into())
                    .track_focus(&self.completion_editor.focus)
                    .key_context("CompletionEditor")
                    .on_action(cx.listener(
                        |this, action: &gpui_component::input::Enter, window, cx| {
                            if action.secondary {
                                this.save_completion_editor(window, cx);
                            }
                            cx.stop_propagation();
                        },
                    ))
                    .on_action(cx.listener(
                        |this, _: &crate::completion_editor_model::NextField, window, cx| {
                            this.cycle_completion_editor_focus(false, window, cx)
                        },
                    ))
                    .on_action(cx.listener(
                        |this, _: &crate::completion_editor_model::PreviousField, window, cx| {
                            this.cycle_completion_editor_focus(true, window, cx)
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
                            && !this.completion_editor.picker.read(cx).is_open()
                        {
                            this.suspend_completion_editor(window, cx);
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
                                    .child("编辑完成事项"),
                            )
                            .child(
                                button("close-completion-editor")
                                    .debug_selector(|| "close-completion-editor".into())
                                    .custom(crate::view::quiet_variant(cx))
                                    .size(px(28.))
                                    .icon(control_icon(IconName::Close, MUTED))
                                    .tooltip("关闭并保留草稿")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.suspend_completion_editor(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("completion-editor-scroll")
                            .min_h_0()
                            .max_h(px((height - 162.).max(180.)))
                            .overflow_y_scroll()
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
                                            .debug_selector(|| "completion-editor-activity".into())
                                            .w_full()
                                            .h(px(40.))
                                            .flex_shrink_0()
                                            .child(self.completion_editor.picker.clone()),
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
                                            .debug_selector(|| {
                                                "completion-editor-description".into()
                                            })
                                            .child(field(&self.completion_editor.description)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(field_label("完成时间"))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.))
                                            .child(
                                                div()
                                                    .debug_selector(|| {
                                                        "completion-editor-date".into()
                                                    })
                                                    .flex_1()
                                                    .min_w_0()
                                                    .child(field(&self.completion_editor.date)),
                                            )
                                            .child(
                                                div()
                                                    .debug_selector(|| {
                                                        "completion-editor-time".into()
                                                    })
                                                    .w(px(104.))
                                                    .flex_shrink_0()
                                                    .child(field(&self.completion_editor.time)),
                                            )
                                            .child(
                                                button("completion-time-picker")
                                                    .debug_selector(|| {
                                                        "completion-time-picker".into()
                                                    })
                                                    .custom(crate::view::quiet_variant(cx))
                                                    .size(px(32.))
                                                    .tooltip("用表盘选择完成时间")
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
                                                        |this, _, window, cx| {
                                                            let target =
                                                                this.completion_editor.time.clone();
                                                            this.time_picker.update(
                                                                cx,
                                                                |picker, cx| {
                                                                    picker.open_for(
                                                                        target, "完成", false,
                                                                        window, cx,
                                                                    )
                                                                },
                                                            );
                                                        },
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED))
                                    .whitespace_normal()
                                    .child("已有计时和安排时间保持不变。"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .flex_shrink_0()
                            .when_some(self.completion_editor.error.as_ref(), |footer, error| {
                                footer.child(
                                    div()
                                        .debug_selector(|| "completion-editor-error".into())
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
                                        primary_button("save-completion-editor")
                                            .debug_selector(|| "save-completion-editor".into())
                                            .h(px(36.))
                                            .px(px(18.))
                                            .unavailable(self.storage_failed)
                                            .child(control_label("保存", PAPER))
                                            .tooltip(crate::view::shortcut(
                                                "保存", "Enter", "Enter",
                                            ))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.save_completion_editor(window, cx);
                                            })),
                                    )
                                    .child(
                                        button("cancel-completion-editor")
                                            .debug_selector(|| "cancel-completion-editor".into())
                                            .custom(crate::view::quiet_variant(cx))
                                            .h(px(36.))
                                            .px(px(12.))
                                            .child(control_label("取消", MUTED))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.cancel_completion_editor(window, cx)
                                            })),
                                    )
                                    .child(div().flex_1())
                                    .when_some(
                                        self.completion_editor.original.as_ref(),
                                        |actions, task| {
                                            let id = task.id;
                                            actions.child(
                                                button("delete-completion-editor")
                                                    .debug_selector(|| {
                                                        "delete-completion-editor".into()
                                                    })
                                                    .custom(crate::view::quiet_variant(cx))
                                                    .h(px(36.))
                                                    .px(px(8.))
                                                    .unavailable(self.storage_failed)
                                                    .child(control_label("删除完成事项", MUTED))
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.delete_completion(id, window, cx)
                                                        },
                                                    )),
                                            )
                                        },
                                    ),
                            ),
                    ),
            )
    }
}
