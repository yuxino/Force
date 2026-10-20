use force_core::TagKind;
use gpui::{prelude::*, *};
use gpui_component::{
    Icon, IconName, Sizable, button::ButtonVariants, scroll::ScrollableElement, switch::Switch,
    tooltip::Tooltip,
};

use crate::{
    label::single_line,
    model::ForceView,
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, PAPER, SURFACE, button, choice_button,
        control_icon, control_label, field, field_label, section_heading,
    },
};

impl ForceView {
    pub(crate) fn settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(36.))
            .child(settings_section(
                "计时偏好",
                div()
                    .debug_selector(|| "settings-timing".into())
                    .flex()
                    .flex_col()
                    .child(
                        settings_row()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .child(setting_label("计时条置顶"))
                            .child(
                                button("pin-strip")
                                    .debug_selector(|| "settings-pin".into())
                                    .ghost()
                                    .h(px(40.))
                                    .px(px(8.))
                                    .unavailable(!cfg!(any(
                                        target_os = "macos",
                                        target_os = "windows"
                                    )))
                                    .tooltip(
                                        if !cfg!(any(target_os = "macos", target_os = "windows")) {
                                            "置顶由窗口管理器控制"
                                        } else if self.pinned {
                                            "计时条置顶已开启"
                                        } else {
                                            "计时条置顶已关闭"
                                        },
                                    )
                                    .child(
                                        Switch::new("pin-indicator").small().checked(self.pinned),
                                    )
                                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                        if event.click_count() == 1 {
                                            this.toggle_strip_pin(cx);
                                        }
                                    })),
                            ),
                    )
                    .child(
                        settings_row()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .child(setting_label("到时提示音"))
                            .child(
                                button("completion-sound")
                                    .ghost()
                                    .h(px(40.))
                                    .px(px(8.))
                                    .unavailable(!cfg!(any(
                                        target_os = "macos",
                                        target_os = "windows"
                                    )))
                                    .tooltip(
                                        if !cfg!(any(target_os = "macos", target_os = "windows")) {
                                            "此平台暂不支持提示音"
                                        } else if self.sound_enabled {
                                            "到时提示音已开启"
                                        } else {
                                            "到时提示音已关闭"
                                        },
                                    )
                                    .child(
                                        Switch::new("sound-indicator")
                                            .small()
                                            .checked(self.sound_enabled),
                                    )
                                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                        if event.click_count() == 1 {
                                            this.set_sound_enabled(!this.sound_enabled, cx);
                                        }
                                    })),
                            ),
                    )
                    .child(
                        settings_row()
                            .child(setting_label("每日目标"))
                            .child(
                                div()
                                    .debug_selector(|| "goal-input".into())
                                    .w(px(88.))
                                    .flex_shrink_0()
                                    .child(gpui::Styled::h(field(&self.goal_input), px(40.))),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(MUTED))
                                    .child("分钟"),
                            )
                            .child(
                                button("save-goal")
                                    .debug_selector(|| "save-goal".into())
                                    .h(px(40.))
                                    .w(px(72.))
                                    .px(px(12.))
                                    .child(control_label(
                                        if self.goal_draft(cx) == Some(self.goal) {
                                            "已保存"
                                        } else {
                                            "保存"
                                        },
                                        INK,
                                    ))
                                    .unavailable(!self.can_save_goal(cx))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_goal(cx);
                                        this.goal_input
                                            .update(cx, |input, cx| input.focus(window, cx));
                                    })),
                            )
                            .child(
                                button("reset-goal")
                                    .debug_selector(|| "reset-goal".into())
                                    .ghost()
                                    .h(px(40.))
                                    .w(px(56.))
                                    .px(px(8.))
                                    .child(control_label("还原", MUTED))
                                    .tooltip(format!(
                                        "还原为已保存的每日目标（{} 分钟）",
                                        self.goal
                                    ))
                                    .unavailable(!self.goal_has_unsaved_changes(cx))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.reset_goal_draft(window, cx)
                                    })),
                            ),
                    )
                    .when(self.goal_draft(cx).is_none(), |body| {
                        body.child(
                            div()
                                .pb(px(8.))
                                .text_size(px(14.))
                                .text_color(rgb(DESTRUCTIVE))
                                .child("请输入 1–1440 之间的整数分钟数。"),
                        )
                    }),
            ))
            .child(settings_section(
                "标签",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .id("settings-tags")
                            .debug_selector(|| "settings-tags".into())
                            .max_h(px(240.))
                            .overflow_y_scroll()
                            .track_scroll(&self.tag_scroll)
                            .relative()
                            .when(self.tags.is_empty(), |list| {
                                list.child(
                                    div()
                                        .h(px(44.))
                                        .flex()
                                        .items_center()
                                        .text_size(px(14.))
                                        .text_color(rgb(MUTED))
                                        .child("暂无标签"),
                                )
                            })
                            .children(self.tags.iter().map(|tag| {
                                let id = tag.id;
                                let help_name = tag.name.clone();
                                div()
                                    .h(px(44.))
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .border_b_1()
                                    .border_color(rgb(LINE))
                                    .child(
                                        single_line(tag.name.clone(), 14.)
                                            .id(("tag-name", id as u64))
                                            .flex_1()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(help_name.clone()).build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .w(px(32.))
                                            .flex_shrink_0()
                                            .text_size(px(14.))
                                            .text_color(rgb(MUTED))
                                            .child(if tag.kind == TagKind::Focus {
                                                "专注"
                                            } else {
                                                "休息"
                                            }),
                                    )
                                    .child(
                                        button(("edit-tag", id as u64))
                                            .ghost()
                                            .size(px(32.))
                                            .icon(control_icon(
                                                Icon::default()
                                                    .path("force-icons/pencil.svg")
                                                    .size(px(15.)),
                                                MUTED,
                                            ))
                                            .tooltip("重命名标签")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.begin_edit_tag(id, window, cx)
                                            })),
                                    )
                                    .child(
                                        button(("remove-tag", id as u64))
                                            .ghost()
                                            .size(px(32.))
                                            .icon(control_icon(
                                                Icon::default()
                                                    .path("force-icons/trash.svg")
                                                    .size(px(15.)),
                                                MUTED,
                                            ))
                                            .tooltip("移除标签")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.delete_tag(id, window, cx)
                                            })),
                                    )
                            }))
                            .vertical_scrollbar(&self.tag_scroll),
                    )
                    .child(
                        div()
                            .id("tag-form")
                            .debug_selector(|| "settings-form".into())
                            .anchor_scroll(Some(self.tag_form_anchor.clone()))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .when(self.editing_tag.is_some(), |form| {
                                form.child(field_label("重命名标签").text_size(px(14.)))
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .debug_selector(|| "tag-input".into())
                                            .flex_1()
                                            .min_w_0()
                                            .child(gpui::Styled::h(
                                                field(&self.tag_input),
                                                px(40.),
                                            )),
                                    )
                                    .when(self.editing_tag.is_none(), |row| {
                                        row.child(
                                            div()
                                                .flex()
                                                .flex_shrink_0()
                                                .p(px(3.))
                                                .rounded(px(7.))
                                                .bg(rgb(SURFACE))
                                                .children(
                                                    [
                                                        (TagKind::Focus, "专注"),
                                                        (TagKind::Rest, "休息"),
                                                    ]
                                                    .into_iter()
                                                    .map(|(kind, label)| {
                                                        choice_button(
                                                            if kind == TagKind::Focus {
                                                                "tag-kind-focus"
                                                            } else {
                                                                "tag-kind-rest"
                                                            },
                                                            self.new_tag_kind == kind,
                                                            PAPER,
                                                            cx,
                                                        )
                                                        .h(px(34.))
                                                        .px(px(12.))
                                                        .child(control_label(
                                                            label,
                                                            if self.new_tag_kind == kind {
                                                                INK
                                                            } else {
                                                                MUTED
                                                            },
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.new_tag_kind = kind;
                                                                cx.notify();
                                                            },
                                                        ))
                                                    }),
                                                ),
                                        )
                                    })
                                    .child(
                                        button("save-tag")
                                            .debug_selector(|| "save-tag".into())
                                            .h(px(40.))
                                            .w(px(64.))
                                            .px(px(12.))
                                            .unavailable(!self.can_save_tag(cx))
                                            .child(control_label(
                                                if self.editing_tag.is_some() {
                                                    "保存"
                                                } else {
                                                    "添加"
                                                },
                                                INK,
                                            ))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.save_tag(window, cx)
                                            })),
                                    )
                                    .when(self.editing_tag.is_some(), |row| {
                                        row.child(
                                            button("cancel-tag-edit")
                                                .ghost()
                                                .h(px(40.))
                                                .w(px(56.))
                                                .px(px(8.))
                                                .child(control_label("取消", MUTED))
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.cancel_tag_edit(window, cx)
                                                })),
                                        )
                                    }),
                            )
                            .when_some(self.tag_draft_error(cx), |form, message| {
                                form.child(
                                    div()
                                        .text_size(px(14.))
                                        .text_color(rgb(DESTRUCTIVE))
                                        .child(message),
                                )
                            }),
                    ),
            ))
            .child(settings_section(
                "本地数据",
                div().flex().items_center().gap(px(12.)).child(
                    button("data-folder")
                        .h(px(40.))
                        .px(px(14.))
                        .icon(control_icon(IconName::FolderOpen, INK))
                        .child(control_label("数据文件夹", INK))
                        .on_click(cx.listener(|this, _, _, cx| cx.reveal_path(&this.data_dir))),
                ),
            ))
            .child(
                div()
                    .pt(px(24.))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                Icon::default()
                                    .path("force-logo.svg")
                                    .size(px(20.))
                                    .text_color(rgb(INK)),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(MUTED))
                                    .child(format!("Force {}", env!("CARGO_PKG_VERSION"))),
                            ),
                    )
                    .child(
                        button("quit-force")
                            .ghost()
                            .h(px(36.))
                            .child(control_label("退出 Force", MUTED))
                            .on_click(cx.listener(|_, _, _, cx| cx.quit())),
                    ),
            )
    }
}

fn settings_section(title: &'static str, content: impl IntoElement) -> Div {
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(32.))
        .child(
            div()
                .w(px(160.))
                .flex_shrink_0()
                .pt(px(10.))
                .child(section_heading(title)),
        )
        .child(div().flex_1().min_w_0().child(content))
}

fn settings_row() -> Div {
    div()
        .min_h(px(64.))
        .py(px(10.))
        .flex()
        .items_center()
        .gap(px(10.))
}

fn setting_label(title: &'static str) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .text_size(px(14.))
        .text_color(rgb(INK))
        .child(title)
}
