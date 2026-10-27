use force_core::{TagClass, TagKind};
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
                                .debug_selector(|| "goal-error".into())
                                .pb(px(8.))
                                .text_right()
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
                                    .when_some(
                                        tag.color.as_deref().and_then(tag_color_rgb),
                                        |row, color| {
                                            row.child(
                                                div()
                                                    .size(px(8.))
                                                    .flex_shrink_0()
                                                    .rounded_full()
                                                    .bg(rgb(color)),
                                            )
                                        },
                                    )
                                    .child(
                                        single_line(tag.name.clone(), 14.)
                                            .id(("tag-name", id as u64))
                                            .flex_1()
                                            .min_w_0()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(help_name.clone()).build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(rgb(MUTED))
                                            .flex_shrink_0()
                                            .child(tag_class_label(tag.class)),
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
                                            .tooltip("编辑标签")
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
                            .child(
                                field_label(if self.editing_tag.is_some() {
                                    "编辑标签"
                                } else {
                                    "新标签"
                                })
                                .text_size(px(14.)),
                            )
                            .when(self.editing_tag.is_some(), |form| {
                                form.child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(MUTED))
                                        .child("修改影响未完成和之后的记录，已完成记录保持原样。"),
                                )
                            })
                            .child(
                                div().flex().items_center().gap(px(8.)).child(
                                    div()
                                        .debug_selector(|| "tag-input".into())
                                        .flex_1()
                                        .min_w_0()
                                        .child(gpui::Styled::h(field(&self.tag_input), px(40.))),
                                ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(field_label("分类"))
                                    .child(
                                        div().flex().flex_wrap().gap(px(8.)).children(
                                            [
                                                TagClass::Positive,
                                                TagClass::Neutral,
                                                TagClass::Negative,
                                                TagClass::Leisure,
                                            ]
                                            .into_iter()
                                            .enumerate()
                                            .map(
                                                |(index, class)| {
                                                    let selected = self.new_tag_class == class;
                                                    button(("tag-class", index as u64))
                                                        .debug_selector(move || {
                                                            format!(
                                                                "tag-class-{}",
                                                                tag_class_key(class)
                                                            )
                                                        })
                                                        .custom(crate::view::quiet_variant(cx))
                                                        .h(px(32.))
                                                        .px(px(12.))
                                                        .border_1()
                                                        .border_color(rgb(if selected {
                                                            INK
                                                        } else {
                                                            LINE
                                                        }))
                                                        .child(control_label(
                                                            tag_class_label(class),
                                                            if selected { INK } else { MUTED },
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.new_tag_class = class;
                                                                cx.notify();
                                                            },
                                                        ))
                                                },
                                            ),
                                        ),
                                    )
                                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(
                                        "专注标签的正向和负向计入专注，中性和休闲保留记录。",
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(field_label("颜色（可选）"))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .items_center()
                                            .gap(px(8.))
                                            .children(TAG_COLORS.into_iter().enumerate().map(
                                                |(index, (label, value))| {
                                                    let selected = self
                                                        .tag_color_input
                                                        .read(cx)
                                                        .value()
                                                        .trim()
                                                        .eq_ignore_ascii_case(value);
                                                    button(("tag-color", index as u64))
                                                        .debug_selector(move || {
                                                            format!("tag-color-{label}")
                                                        })
                                                        .custom(crate::view::quiet_variant(cx))
                                                        .size(px(32.))
                                                        .border_1()
                                                        .border_color(rgb(if selected {
                                                            INK
                                                        } else {
                                                            LINE
                                                        }))
                                                        .tooltip(if value.is_empty() {
                                                            "不设置颜色".to_owned()
                                                        } else {
                                                            format!("{label} {value}")
                                                        })
                                                        .when(value.is_empty(), |b| {
                                                            b.child(control_label("无", MUTED))
                                                        })
                                                        .when_some(
                                                            tag_color_rgb(value),
                                                            |b, color| {
                                                                b.child(
                                                                    div()
                                                                        .size(px(14.))
                                                                        .rounded_full()
                                                                        .bg(rgb(color)),
                                                                )
                                                            },
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.tag_color_input.update(
                                                                    cx,
                                                                    |input, cx| {
                                                                        input.set_value(
                                                                            value, window, cx,
                                                                        )
                                                                    },
                                                                );
                                                                cx.notify();
                                                            },
                                                        ))
                                                },
                                            ))
                                            .child(div().w(px(112.)).child(gpui::Styled::h(
                                                field(&self.tag_color_input),
                                                px(32.),
                                            ))),
                                    ),
                            )
                            .when_some(self.tag_draft_error(cx), |form, message| {
                                form.child(
                                    div()
                                        .debug_selector(|| "tag-draft-error".into())
                                        .text_size(px(14.))
                                        .text_color(rgb(DESTRUCTIVE))
                                        .child(message),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .gap(px(8.))
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
                                    })
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
                                    ),
                            ),
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

pub(crate) fn tag_class_label(class: TagClass) -> &'static str {
    match class {
        TagClass::Positive => "正向",
        TagClass::Neutral => "中性",
        TagClass::Negative => "负向",
        TagClass::Leisure => "休闲",
        TagClass::Legacy => "未分类",
    }
}

fn tag_class_key(class: TagClass) -> &'static str {
    match class {
        TagClass::Positive => "positive",
        TagClass::Neutral => "neutral",
        TagClass::Negative => "negative",
        TagClass::Leisure => "leisure",
        TagClass::Legacy => "legacy",
    }
}

pub(crate) fn tag_color_rgb(value: &str) -> Option<u32> {
    (value.len() == 7 && value.starts_with('#'))
        .then(|| u32::from_str_radix(&value[1..], 16).ok())
        .flatten()
}

const TAG_COLORS: [(&str, &str); 7] = [
    ("无", ""),
    ("灰", "#858585"),
    ("陶", "#B56F67"),
    ("琥珀", "#B58A4A"),
    ("苔", "#6C9079"),
    ("海", "#688BA5"),
    ("紫灰", "#8A7B9D"),
];

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
