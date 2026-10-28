use force_core::{TagClass, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Icon, IconName, Sizable, button::ButtonVariants, scroll::ScrollableElement,
    switch::Switch, tooltip::Tooltip,
};

use crate::{
    label::single_line,
    model::ForceView,
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, PAPER, SURFACE, button, control_icon,
        control_label, field, field_label, outlined_button, primary_button, section_heading,
    },
};

impl ForceView {
    pub(crate) fn settings_panel(&self, wide: bool, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "settings-column".into())
            .w_full()
            .flex()
            .flex_col()
            .gap(px(24.))
            .when_some(self.timer_context(cx), |body, context| body.child(context))
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_start()
                    .gap(px(32.))
                    .when(!wide, |body| body.flex_col())
                    .child(
                        div()
                            .debug_selector(|| "settings-general".into())
                            .flex()
                            .flex_col()
                            .gap(px(28.))
                            .when(wide, |column| column.w(px(280.)).flex_shrink_0())
                            .when(!wide, |column| column.w_full())
                            .child(self.settings_timing(cx))
                            .when(wide, |column| column.child(self.settings_about(cx))),
                    )
                    .child(
                        div()
                            .debug_selector(|| "settings-labels".into())
                            .min_w_0()
                            .when(wide, |column| column.flex_1())
                            .when(!wide, |column| column.w_full())
                            .child(self.settings_tags(cx)),
                    ),
            )
            .when(!wide, |body| body.child(self.settings_about(cx)))
    }

    fn settings_timing(&self, cx: &mut Context<Self>) -> Div {
        settings_section(
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
                                .unavailable(!cfg!(any(target_os = "macos", target_os = "windows")))
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
                                .unavailable(!cfg!(any(target_os = "macos", target_os = "windows")))
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
                                        .on_click(cx.listener(|this, enabled: &bool, _, cx| {
                                            this.set_sound_enabled(*enabled, cx);
                                        })),
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
                                    .tooltip(format!("撤销未保存的修改，恢复为 {} 分钟", self.goal))
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
        )
    }

    fn settings_tags(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w_full()
            .pt(px(16.))
            .border_t_1()
            .border_color(rgb(LINE))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .min_h(px(32.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(section_heading("标签"))
                    .when(
                        !self.tag_form_open || self.editing_tag.is_some(),
                        |header| {
                            header.child(
                                button("new-tag")
                                    .debug_selector(|| "new-tag".into())
                                    .custom(crate::view::quiet_variant(cx))
                                    .h(px(32.))
                                    .px(px(8.))
                                    .icon(control_icon(
                                        Icon::new(IconName::Plus).size(px(14.)),
                                        INK,
                                    ))
                                    .child(control_label("新标签", INK))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_tag_settings(window, cx)
                                    })),
                            )
                        },
                    ),
            )
            .when(self.tag_form_open, |section| {
                section.child(self.tag_editor(cx))
            })
            .child(
                // The scrollbar must stay outside the translated scroll content.
                div()
                    .debug_selector(|| "settings-tag-viewport".into())
                    .relative()
                    .child(
                        div()
                            .id("settings-tags")
                            .debug_selector(|| "settings-tags".into())
                            .max_h(px(240.))
                            .overflow_hidden()
                            .pr(px(20.))
                            .track_scroll(&self.tag_scroll)
                            // GPUI scroll containers otherwise also scroll their parents.
                            .on_scroll_wheel(cx.listener(
                                |this, event: &ScrollWheelEvent, window, cx| {
                                    let old = this.tag_scroll.offset();
                                    let delta = event.delta.pixel_delta(window.line_height()).y;
                                    let next = (old.y + delta)
                                        .clamp(-this.tag_scroll.max_offset().height, px(0.));
                                    if next != old.y {
                                        this.tag_scroll.set_offset(point(old.x, next));
                                        cx.notify();
                                        cx.stop_propagation();
                                    }
                                },
                            ))
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
                                let color = tag.color.as_deref().and_then(tag_text_color);
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
                                            .text_color(rgb(color.unwrap_or(INK)))
                                            .flex_1()
                                            .min_w_0()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(help_name.clone()).build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(rgb(color.unwrap_or(MUTED)))
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
                                            .debug_selector(move || format!("remove-tag-{id}"))
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
                            })),
                    )
                    .vertical_scrollbar(&self.tag_scroll),
            )
    }

    fn tag_editor(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let can_save = self.can_save_tag(cx);
        div()
            .id("tag-form")
            .debug_selector(|| "settings-form".into())
            .anchor_scroll(Some(self.tag_form_anchor.clone()))
            .w_full()
            .max_w(px(480.))
            .pb(px(16.))
            .border_b_1()
            .border_color(rgb(LINE))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(if self.editing_tag.is_some() {
                                "编辑标签"
                            } else {
                                "新标签"
                            }),
                    )
                    .when(self.editing_tag.is_some(), |heading| {
                        heading.child(
                            button("tag-edit-help")
                                .custom(crate::view::quiet_variant(cx))
                                .size(px(24.))
                                .icon(control_icon(Icon::new(IconName::Info).size(px(13.)), MUTED))
                                .tooltip("名称和分类影响未完成及之后的记录；日历颜色同步更新。"),
                        )
                    }),
            )
            .child(tag_form_row(
                "名称",
                div()
                    .debug_selector(|| "tag-input".into())
                    .w_full()
                    .child(gpui::Styled::h(field(&self.tag_input), px(36.))),
            ))
            .child(tag_form_row(
                "分类",
                div()
                    .debug_selector(|| "tag-class-options".into())
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .children(
                        [
                            TagClass::Positive,
                            TagClass::Neutral,
                            TagClass::Negative,
                            TagClass::Leisure,
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(index, class)| {
                            let selected = self.new_tag_class == class;
                            let foreground =
                                tag_text_color(self.tag_color_input.read(cx).value().trim())
                                    .unwrap_or(INK);
                            button(("tag-class", index as u64))
                                .debug_selector(move || {
                                    format!("tag-class-{}", tag_class_key(class))
                                })
                                .custom(
                                    crate::view::quiet_variant(cx)
                                        .border(rgb(if selected { INK } else { LINE }).into()),
                                )
                                .h(px(32.))
                                .px(px(10.))
                                .border_1()
                                .border_color(rgb(if selected { INK } else { LINE }))
                                .child(control_label(
                                    tag_class_label(class),
                                    if selected { foreground } else { MUTED },
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.new_tag_class = class;
                                    cx.notify();
                                }))
                        }),
                    )
                    .child(
                        button("tag-class-help")
                            .custom(crate::view::quiet_variant(cx))
                            .size(px(24.))
                            .icon(control_icon(Icon::new(IconName::Info).size(px(13.)), MUTED))
                            .tooltip("专注标签的正向和负向计入专注，中性和休闲保留记录。"),
                    ),
            ))
            .child(tag_form_row(
                "颜色",
                div()
                    .debug_selector(|| "tag-color-options".into())
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .children(
                        TAG_COLORS
                            .into_iter()
                            .enumerate()
                            .map(|(index, (label, value))| {
                                let selected = self
                                    .tag_color_input
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .eq_ignore_ascii_case(value);
                                button(("tag-color", index as u64))
                                    .debug_selector(move || format!("tag-color-{label}"))
                                    .custom(
                                        crate::view::quiet_variant(cx)
                                            .border(rgb(if selected { INK } else { LINE }).into()),
                                    )
                                    .size(px(28.))
                                    .border_1()
                                    .border_color(rgb(if selected { INK } else { LINE }))
                                    .tooltip(if value.is_empty() {
                                        "不设置颜色".to_owned()
                                    } else {
                                        format!("{label} {value}")
                                    })
                                    .when(value.is_empty(), |button| {
                                        button.child(control_label("无", MUTED))
                                    })
                                    .when_some(tag_color_rgb(value), |button, color| {
                                        button.child(
                                            div().size(px(12.)).rounded_full().bg(rgb(color)),
                                        )
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.tag_color_input.update(cx, |input, cx| {
                                            input.set_value(value, window, cx)
                                        });
                                        cx.notify();
                                    }))
                            }),
                    )
                    .child(
                        div()
                            .w(px(104.))
                            .child(gpui::Styled::h(field(&self.tag_color_input), px(32.))),
                    ),
            ))
            .when_some(self.tag_draft_error(cx), |form, message| {
                form.child(
                    div()
                        .debug_selector(|| "tag-draft-error".into())
                        .pl(px(48.))
                        .text_size(px(13.))
                        .text_color(rgb(DESTRUCTIVE))
                        .child(message),
                )
            })
            .child(
                div()
                    .debug_selector(|| "tag-form-actions".into())
                    .pl(px(48.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        primary_button("save-tag")
                            .debug_selector(|| "save-tag".into())
                            .h(px(36.))
                            .w(px(72.))
                            .px(px(12.))
                            .unavailable(!can_save)
                            .when(!can_save, |button| {
                                button.bg(rgb(crate::control_style::DISABLED_START_BACKGROUND))
                            })
                            .child(control_label(
                                if self.editing_tag.is_some() {
                                    "保存"
                                } else {
                                    "添加"
                                },
                                if can_save { PAPER } else { MUTED },
                            ))
                            .on_click(cx.listener(|this, _, window, cx| this.save_tag(window, cx))),
                    )
                    .child(
                        button("cancel-tag-edit")
                            .debug_selector(|| "cancel-tag-edit".into())
                            .custom(crate::view::quiet_variant(cx))
                            .h(px(36.))
                            .w(px(56.))
                            .px(px(8.))
                            .child(control_label("取消", MUTED))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.cancel_tag_edit(window, cx)),
                            ),
                    ),
            )
    }

    fn settings_about(&self, cx: &mut Context<Self>) -> Div {
        settings_section(
            "应用",
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .debug_selector(|| "settings-version".into())
                        .h(px(32.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(img(crate::embedded_image_source("force-logo.png")).size(px(20.)))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Force"),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(rgb(MUTED))
                                .child(env!("CARGO_PKG_VERSION")),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            outlined_button("check-updates")
                                .debug_selector(|| "settings-update".into())
                                .justify_start()
                                .px(px(12.))
                                .w_full()
                                .h(px(36.))
                                .unavailable(
                                    self.update_state == crate::updates::UpdateState::Checking,
                                )
                                .child(control_label(self.update_state.label(), INK))
                                .on_click(cx.listener(|this, _, _, cx| this.update_action(cx))),
                        )
                        .when_some(self.update_state.message(), |body, message| {
                            body.child(
                                div()
                                    .debug_selector(|| "settings-update-status".into())
                                    .w_full()
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED))
                                    .whitespace_normal()
                                    .child(message),
                            )
                        }),
                )
                .child(
                    outlined_button("data-folder")
                        .debug_selector(|| "settings-data-folder".into())
                        .justify_start()
                        .px(px(12.))
                        .w_full()
                        .h(px(36.))
                        .child(control_label("打开数据文件夹", INK))
                        .on_click(cx.listener(|this, _, _, cx| cx.reveal_path(&this.data_dir))),
                )
                .child(
                    div().pt(px(8.)).border_t_1().border_color(rgb(LINE)).child(
                        button("quit-force")
                            .debug_selector(|| "settings-quit".into())
                            .justify_start()
                            .px(px(12.))
                            .custom(crate::view::quiet_variant(cx))
                            .w_full()
                            .h(px(36.))
                            .child(control_label("退出 Force", MUTED))
                            .on_click(cx.listener(|_, _, _, cx| cx.quit())),
                    ),
                ),
        )
        .debug_selector(|| "settings-about".into())
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

pub(crate) fn tag_class_key(class: TagClass) -> &'static str {
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

// Preserve the chosen hue while keeping small text legible on white and on
// neutral selected, hovered and pressed surfaces. Swatches retain the exact saved color.
pub(crate) fn tag_text_color(value: &str) -> Option<u32> {
    let color = tag_color_rgb(value)?;
    let mut channels = [(color >> 16) & 255, (color >> 8) & 255, color & 255];
    while tag_luminance(channels) > 0.138 {
        channels = channels.map(|channel| channel.saturating_sub((channel / 16).max(1)));
    }
    Some((channels[0] << 16) | (channels[1] << 8) | channels[2])
}

fn tag_luminance(channels: [u32; 3]) -> f64 {
    let linear = channels.map(|channel| {
        let value = channel as f64 / 255.;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    });
    linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
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

fn tag_form_row(label: &'static str, content: impl IntoElement) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .child(
            field_label(label)
                .w(px(36.))
                .h(px(32.))
                .flex_shrink_0()
                .flex()
                .items_center(),
        )
        .child(div().flex_1().min_w_0().child(content))
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

#[cfg(test)]
mod tests {
    use super::{TAG_COLORS, tag_luminance, tag_text_color};
    use crate::view::{HOVER, PAPER, PRESSED, SURFACE};

    #[test]
    fn tag_text_colors_keep_light_custom_colors_readable() {
        for value in TAG_COLORS
            .iter()
            .map(|(_, value)| *value)
            .chain(["#FFFFFF", "#FFFF00", "#00FF00", "#AABBCC"])
        {
            let Some(color) = tag_text_color(value) else {
                continue;
            };
            let foreground = tag_luminance([(color >> 16) & 255, (color >> 8) & 255, color & 255]);
            for background in [PAPER, SURFACE, HOVER, PRESSED] {
                let surface = tag_luminance([
                    (background >> 16) & 255,
                    (background >> 8) & 255,
                    background & 255,
                ]);
                assert!(
                    (surface + 0.05) / (foreground + 0.05) >= 4.5,
                    "{value} on {background:x}"
                );
            }
        }
        assert_eq!(tag_text_color("#123456"), Some(0x123456));
        assert_eq!(tag_text_color(""), None);
        assert_eq!(tag_text_color("#GGGGGG"), None);
    }
}
