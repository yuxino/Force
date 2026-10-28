use crate::{
    model::ForceView,
    settings_view::{TAG_COLORS, tag_class_key, tag_class_label, tag_color_rgb, tag_text_color},
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, PAPER, button, control_icon, control_label,
        field, primary_button, section_heading,
    },
};
use force_core::TagClass;
use gpui::{prelude::*, *};
use gpui_component::{Icon, IconName, button::ButtonVariants};

impl ForceView {
    pub(crate) fn settings_classifications(&self, cx: &mut Context<Self>) -> Div {
        div()
            .debug_selector(|| "settings-classifications".into())
            .w_full()
            .pt(px(16.))
            .border_t_1()
            .border_color(rgb(LINE))
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(section_heading("分类"))
                    .child(
                        button("class-color-help")
                            .custom(crate::view::quiet_variant(cx))
                            .size(px(24.))
                            .icon(control_icon(Icon::new(IconName::Info).size(px(13.)), MUTED))
                            .tooltip(
                                "统一设置分类颜色，活动默认跟随。已单独设置颜色的活动保留原色。",
                            ),
                    ),
            )
            .children(
                [
                    TagClass::Positive,
                    TagClass::Neutral,
                    TagClass::Negative,
                    TagClass::Leisure,
                ]
                .into_iter()
                .map(|class| {
                    let key = tag_class_key(class);
                    let color = self.classification.colors.color(class);
                    let foreground = color.and_then(tag_text_color).unwrap_or(INK);
                    let editing = self.classification.editing == Some(class);
                    div()
                        .debug_selector(move || format!("class-row-{key}"))
                        .w_full()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .h(px(36.))
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .text_color(rgb(foreground))
                                        .child(tag_class_label(class)),
                                )
                                .child(div().flex_1())
                                .child(
                                    button(SharedString::from(format!("edit-class-color-{key}")))
                                        .debug_selector(move || format!("edit-class-color-{key}"))
                                        .custom(crate::view::quiet_variant(cx))
                                        .h(px(30.))
                                        .px(px(8.))
                                        .unavailable(self.storage_failed)
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(7.))
                                                .child(div().size(px(9.)).rounded_full().bg(rgb(
                                                    color.and_then(tag_color_rgb).unwrap_or(MUTED),
                                                )))
                                                .child(control_label(
                                                    if color.is_some() {
                                                        "颜色"
                                                    } else {
                                                        "默认"
                                                    },
                                                    MUTED,
                                                )),
                                        )
                                        .tooltip("设置分类颜色")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.begin_class_color_edit(class, window, cx)
                                        })),
                                ),
                        )
                        .when(editing, |row| row.child(self.class_color_editor(cx)))
                }),
            )
    }

    fn class_color_editor(&self, cx: &mut Context<Self>) -> Div {
        let draft = self.classification.input.read(cx).value();
        div()
            .debug_selector(|| "class-color-editor".into())
            .w_full()
            .pt(px(6.))
            .pb(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .children(
                        TAG_COLORS
                            .into_iter()
                            .enumerate()
                            .map(|(index, (label, value))| {
                                let selected = draft.trim().eq_ignore_ascii_case(value);
                                button(("class-color-option", index as u64))
                                    .debug_selector(move || format!("class-color-option-{index}"))
                                    .custom(
                                        crate::view::quiet_variant(cx)
                                            .border(rgb(if selected { INK } else { LINE }).into()),
                                    )
                                    .h(px(28.))
                                    .w(px(if value.is_empty() { 48. } else { 28. }))
                                    .border_1()
                                    .border_color(rgb(if selected { INK } else { LINE }))
                                    .tooltip(if value.is_empty() {
                                        "使用默认文字颜色".to_owned()
                                    } else {
                                        format!("{label} {value}")
                                    })
                                    .when(value.is_empty(), |button| {
                                        button.child(control_label("默认", MUTED))
                                    })
                                    .when_some(tag_color_rgb(value), |button, color| {
                                        button.child(
                                            div().size(px(12.)).rounded_full().bg(rgb(color)),
                                        )
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.classification.input.update(cx, |input, cx| {
                                            input.set_value(value, window, cx)
                                        });
                                        cx.notify();
                                    }))
                            }),
                    )
                    .child(
                        div()
                            .w(px(104.))
                            .debug_selector(|| "class-color-input".into())
                            .child(gpui::Styled::h(field(&self.classification.input), px(32.))),
                    ),
            )
            .when_some(self.classification.error.as_ref(), |editor, message| {
                editor.child(
                    div()
                        .debug_selector(|| "class-color-error".into())
                        .text_size(px(13.))
                        .text_color(rgb(DESTRUCTIVE))
                        .child(message.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        primary_button("save-class-color")
                            .debug_selector(|| "save-class-color".into())
                            .h(px(30.))
                            .px(px(12.))
                            .unavailable(self.storage_failed)
                            .child(control_label("保存", PAPER))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_class_color_edit(window, cx);
                            })),
                    )
                    .child(
                        button("cancel-class-color")
                            .debug_selector(|| "cancel-class-color".into())
                            .custom(crate::view::quiet_variant(cx))
                            .h(px(30.))
                            .px(px(10.))
                            .child(control_label("取消", MUTED))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel_class_color_edit(window, cx)
                            })),
                    ),
            )
    }
}
