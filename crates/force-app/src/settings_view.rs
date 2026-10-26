use force_core::TagKind;
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Icon, Sizable, button::ButtonVariants, scroll::ScrollableElement, switch::Switch,
    tooltip::Tooltip,
};

use crate::{
    label::single_line,
    model::ForceView,
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, SURFACE, button, control_icon, control_label,
        field, field_label, outlined_button, section_heading,
    },
};

impl ForceView {
    pub(crate) fn settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "settings-column".into())
            .w_full()
            .max_w(px(560.))
            .flex()
            .flex_col()
            .gap(px(20.))
            .when_some(self.timer_context(cx), |body, context| {
                body.child(context.pb_0().border_b_0())
            })
            .child(settings_section(
                "计时",
                div()
                    .debug_selector(|| "settings-timing".into())
                    .flex()
                    .flex_col()
                    .child(
                        settings_row()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .child(setting_label("猫钟始终置顶"))
                            .child(
                                button("pin-strip")
                                    .debug_selector(|| "settings-pin".into())
                                    .custom(crate::view::quiet_variant(cx))
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
                                            "猫钟置顶已开启"
                                        } else {
                                            "猫钟置顶已关闭"
                                        },
                                    )
                                    .child(
                                        Switch::new("pin-indicator")
                                            .small()
                                            .checked(self.pinned)
                                            .disabled(!cfg!(any(
                                                target_os = "macos",
                                                target_os = "windows"
                                            )))
                                            // Switch stops mouse-down propagation, so its
                                            // native surface and the outer keyboard button
                                            // each activate once through their own path.
                                            .on_click(cx.listener(|this, _: &bool, _, cx| {
                                                this.toggle_strip_pin(cx);
                                            })),
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
                            .child(setting_label("倒计时结束时响铃"))
                            .child(
                                button("completion-sound")
                                    .debug_selector(|| "settings-sound".into())
                                    .custom(crate::view::quiet_variant(cx))
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
                                            .checked(self.sound_enabled)
                                            .disabled(!cfg!(any(
                                                target_os = "macos",
                                                target_os = "windows"
                                            )))
                                            .on_click(cx.listener(
                                                |this, enabled: &bool, _, cx| {
                                                    this.set_sound_enabled(*enabled, cx);
                                                },
                                            )),
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
                            .child(setting_label("每日专注目标"))
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
                            ),
                    )
                    .when(self.goal_has_unsaved_changes(cx), |body| {
                        body.child(
                            div()
                                .debug_selector(|| "goal-actions".into())
                                .w_full()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .pb(px(8.))
                                .child(
                                    outlined_button("save-goal")
                                        .debug_selector(|| "save-goal".into())
                                        .h(px(40.))
                                        .w(px(72.))
                                        .px(px(12.))
                                        .child(control_label(
                                            "保存",
                                            if self.can_save_goal(cx) { INK } else { MUTED },
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
                                        .custom(crate::view::quiet_variant(cx))
                                        .h(px(40.))
                                        .w(px(56.))
                                        .px(px(8.))
                                        .child(control_label("撤销", MUTED))
                                        .tooltip(format!(
                                            "撤销未保存的修改，恢复为 {} 分钟",
                                            self.goal
                                        ))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.reset_goal_draft(window, cx)
                                        })),
                                ),
                        )
                    })
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
                                        .h(px(40.))
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
                                let editing = self.editing_tag == Some(id);
                                div()
                                    .debug_selector(move || {
                                        if editing {
                                            format!("editing-tag-row-{id}")
                                        } else {
                                            format!("tag-row-{id}")
                                        }
                                    })
                                    .h(px(40.))
                                    .flex_shrink_0()
                                    .when(editing, |row| row.bg(rgb(SURFACE)))
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .border_b_1()
                                    .border_color(rgb(LINE))
                                    .child(
                                        single_line(tag.name.clone(), 14.)
                                            .id(("tag-name", id as u64))
                                            .flex_1()
                                            .min_w_0()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(help_name.clone()).build(window, cx)
                                            }),
                                    )
                                    .when(tag.kind == TagKind::Rest && tag.name != "休息", |row| {
                                        row.child(
                                            div()
                                                .w(px(32.))
                                                .flex_shrink_0()
                                                .text_size(px(14.))
                                                .text_color(rgb(MUTED))
                                                .child("休息"),
                                        )
                                    })
                                    .child(
                                        button(("edit-tag", id as u64))
                                            .debug_selector(move || format!("edit-tag-{id}"))
                                            .custom(crate::view::quiet_variant(cx))
                                            .size(px(32.))
                                            .icon(control_icon(
                                                Icon::default()
                                                    .path("force-icons/pencil.svg")
                                                    .size(px(15.)),
                                                if editing { INK } else { MUTED },
                                            ))
                                            .tooltip("重命名标签")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.begin_edit_tag(id, window, cx)
                                            })),
                                    )
                                    .child(
                                        button(("remove-tag", id as u64))
                                            .custom(crate::view::quiet_variant(cx))
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
                            .pt(px(4.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .when(self.editing_tag.is_some(), |form| {
                                form.child(field_label("重命名").text_size(px(14.)))
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
                                    .child(
                                        outlined_button("save-tag")
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
                                                if self.can_save_tag(cx) { INK } else { MUTED },
                                            ))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                if this.editing_tag.is_none() {
                                                    this.new_tag_kind = TagKind::Focus;
                                                }
                                                this.save_tag(window, cx)
                                            })),
                                    )
                                    .when(self.editing_tag.is_some(), |row| {
                                        row.child(
                                            button("cancel-tag-edit")
                                                .debug_selector(|| "cancel-tag-edit".into())
                                                .custom(crate::view::quiet_variant(cx))
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
                                        .debug_selector(|| "tag-draft-error".into())
                                        .text_size(px(14.))
                                        .text_color(rgb(DESTRUCTIVE))
                                        .child(message),
                                )
                            }),
                    ),
            ))
            .child(
                div()
                    .pt(px(16.))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .flex_col()
                            .items_start()
                            .gap(px(4.))
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
                                    )
                                    .child(
                                        button("check-updates")
                                            .debug_selector(|| "settings-update".into())
                                            .custom(crate::view::quiet_variant(cx))
                                            .h(px(36.))
                                            .unavailable(
                                                self.update_state
                                                    == crate::updates::UpdateState::Checking,
                                            )
                                            .child(control_label(self.update_state.label(), MUTED))
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    this.update_action(cx)
                                                }),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        button("data-folder")
                                            .custom(crate::view::quiet_variant(cx))
                                            .h(px(36.))
                                            .child(control_label("打开数据文件夹", MUTED))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                cx.reveal_path(&this.data_dir)
                                            })),
                                    )
                                    .child(
                                        button("quit-force")
                                            .custom(crate::view::quiet_variant(cx))
                                            .h(px(36.))
                                            .child(control_label("退出 Force", MUTED))
                                            .on_click(cx.listener(|_, _, _, cx| cx.quit())),
                                    ),
                            ),
                    )
                    .when_some(self.update_state.message(), |footer, message| {
                        footer.child(
                            div()
                                .debug_selector(|| "settings-update-status".into())
                                .text_size(px(12.))
                                .text_color(rgb(MUTED))
                                .child(message),
                        )
                    }),
            )
    }
}

fn settings_section(title: &'static str, content: impl IntoElement) -> Div {
    div()
        .w_full()
        .pt(px(16.))
        .border_t_1()
        .border_color(rgb(LINE))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(section_heading(title))
        .child(div().w_full().min_w_0().child(content))
}

fn settings_row() -> Div {
    div()
        .min_h(px(48.))
        .py(px(4.))
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
