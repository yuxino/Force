//! Small calendar transactions stay above the month instead of replacing it.
use chrono::{Datelike, Days};
use force_core::TagKind;
use gpui::{prelude::*, *};
use gpui_component::{IconName, button::ButtonVariants};

use crate::{
    agenda_model::agenda_tag_color,
    label::single_line,
    model::{ForceView, day_bounds},
    settings_view::{tag_color_rgb, tag_text_color},
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, button, control_icon, control_label, field,
        field_label, primary_button, short_time,
    },
};

const POPUP_WIDTH: f32 = 360.;
const TAG_WIDTH: f32 = POPUP_WIDTH - 38.;
const VALIDATION_HEIGHT: f32 = 36.;
const EDITOR_GAP: f32 = 12.;

impl ForceView {
    pub(crate) fn agenda_popover(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let size = window.viewport_size();
        let width = POPUP_WIDTH.min(f32::from(size.width) - 24.);
        let max_height = (f32::from(size.height) - 100.).max(240.);
        let month = self.agenda.month;
        let first = month
            .checked_sub_days(Days::new(u64::from(month.weekday().num_days_from_monday())))
            .unwrap_or(month);
        let last = month
            .checked_add_days(Days::new(31))
            .and_then(|day| day.with_day(1))
            .and_then(|day| day.pred_opt())
            .unwrap_or(month);
        let weeks = (month.weekday().num_days_from_monday() + last.day()).div_ceil(7);
        let content_width = f32::from(size.width).min(976.) - 80.;
        let cell_width = content_width / 7.;
        let cell_height = (f32::from(size.height) - 200.) / weeks as f32;
        let index = self
            .agenda
            .date
            .signed_duration_since(first)
            .num_days()
            .clamp(0, 41) as usize;
        let cell_left =
            (f32::from(size.width) - content_width) / 2. + index as f32 % 7. * cell_width;
        let right = cell_left + cell_width + 8.;
        let left = if right + width <= f32::from(size.width) - 12. {
            right
        } else {
            cell_left - width - 8.
        }
        .clamp(12., f32::from(size.width) - width - 12.);
        let top =
            (176. + (index / 7) as f32 * cell_height).clamp(80., f32::from(size.height) - 100.);
        let selected_detail = self.agenda.selected_plan.is_some()
            || self.agenda.selected_record.is_some()
            || self.agenda.selected_completion.is_some();
        let heading = if self.agenda.editor_visible {
            if self.agenda.edit_id.is_some() {
                "修改安排".to_owned()
            } else {
                "新安排".to_owned()
            }
        } else {
            self.agenda.date.format("%-m月%-d日").to_string()
        };
        let body_limit = (max_height - 142.).max(100.);
        let popup = div()
            .id("agenda-popover")
            .debug_selector(|| "agenda-popover".into())
            .w(px(width))
            .max_h(px(max_height))
            .p(px(18.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(crate::view::CONTROL_LINE))
            .bg(rgb(0xffffff))
            .occlude()
            .flex()
            .flex_col()
            .gap(px(12.))
            .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                // The tag menu is independently deferred above this popup.
                // Its clicks must not suspend the editor beneath it.
                if !this.agenda.tag_picker.read(cx).is_open() {
                    this.suspend_agenda_popup(window, cx);
                }
            }))
            .child(
                div()
                    .h(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        single_line(heading, 16.)
                            .flex_1()
                            .font_weight(FontWeight::MEDIUM),
                    )
                    .child(
                        button("agenda-popover-close")
                            .debug_selector(|| "agenda-popover-close".into())
                            .custom(crate::view::quiet_variant(cx))
                            .size(px(26.))
                            .icon(control_icon(IconName::Close, MUTED))
                            .tooltip(if self.agenda.editor_visible {
                                "关闭并保留草稿"
                            } else {
                                "关闭"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.suspend_agenda_popup(window, cx)
                            })),
                    ),
            )
            .when(self.agenda.editor_visible, |popup| {
                popup.child(
                    div()
                        .debug_selector(|| "agenda-editor".into())
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .gap(px(EDITOR_GAP))
                        .child(self.agenda_compact_editor(
                            (body_limit - VALIDATION_HEIGHT - EDITOR_GAP).max(100.),
                            cx,
                        ))
                        .child(self.agenda_validation())
                        .child(self.agenda_editor_actions(cx)),
                )
            })
            .when(!self.agenda.editor_visible, |popup| {
                popup
                    .child(
                        div()
                            .id("agenda-popup-scroll")
                            .max_h(px(body_limit))
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.agenda.inspector_scroll)
                            .when(selected_detail, |body| {
                                body.when_some(self.agenda_selected_detail(cx), |body, detail| {
                                    body.child(detail)
                                })
                            })
                            .when(!selected_detail, |body| {
                                body.child(self.agenda_day_summary(cx))
                            }),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .pt(px(12.))
                            .border_t_1()
                            .border_color(rgb(LINE))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .child(
                                button("agenda-popover-timeline")
                                    .debug_selector(|| "agenda-popover-timeline".into())
                                    .custom(crate::view::quiet_variant(cx))
                                    .h(px(34.))
                                    .child(control_label("展开时间轴", MUTED))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_agenda_timeline(window, cx)
                                    })),
                            )
                            .child(if selected_detail {
                                button("agenda-popover-day")
                                    .custom(crate::view::quiet_variant(cx))
                                    .h(px(34.))
                                    .child(control_label("当天概览", INK))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_agenda_day(this.agenda.date, window, cx)
                                    }))
                            } else {
                                crate::control_style::plan_button("agenda-add", self.storage_failed)
                                    .debug_selector(|| "agenda-add".into())
                                    .h(px(34.))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.begin_plan(None, window, cx)
                                    }))
                            }),
                    )
            });
        // Anchored already lays out absolutely. Keep the parent in the normal
        // paint pass: TagPicker and input controls defer their own menus, and
        // GPUI 0.2.2 rejects a deferred draw nested inside another deferred draw.
        anchored()
            .position(point(px(left), px(top)))
            .snap_to_window_with_margin(px(12.))
            .child(popup)
            .into_any_element()
    }

    fn agenda_compact_editor(&self, body_limit: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let linked_task = self
            .agenda
            .draft_task
            .and_then(|id| self.tasks.iter().find(|task| task.id == id));
        div()
            .debug_selector(|| "agenda-editor-fields".into())
            .id("agenda-editor-scroll")
            .max_h(px(body_limit))
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.agenda.editor_scroll)
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(editor_field("安排名称").child(
                div().debug_selector(|| "agenda-title-input".into()).child(
                    field(&self.agenda.title_input).disabled(self.agenda.draft_task.is_some()),
                ),
            ))
            .child(
                editor_field("日期").child(
                    div()
                        .debug_selector(|| "agenda-date-input".into())
                        .child(field(&self.agenda.date_input)),
                ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        editor_field("开始时间").flex_1().min_w_0().child(
                            div()
                                .debug_selector(|| "agenda-start-input".into())
                                .child(field(&self.agenda.start_input)),
                        ),
                    )
                    .child(
                        editor_field("结束时间").flex_1().min_w_0().child(
                            div()
                                .debug_selector(|| "agenda-end-input".into())
                                .child(field(&self.agenda.end_input)),
                        ),
                    ),
            )
            .child(editor_field("标签").w(px(TAG_WIDTH)).flex_shrink_0().child(
                if let Some(task) = linked_task {
                    div()
                        .h(px(40.))
                        .w_full()
                        .flex()
                        .items_center()
                        .child(single_line(task.tag_name.clone(), 14.).w(px(TAG_WIDTH)))
                        .into_any_element()
                } else {
                    div()
                        .w(px(TAG_WIDTH))
                        .h(px(40.))
                        .child(self.agenda.tag_picker.clone())
                        .into_any_element()
                },
            ))
            .child(
                button("agenda-note-toggle")
                    .debug_selector(|| "agenda-note-toggle".into())
                    .custom(crate::view::quiet_variant(cx))
                    .h(px(28.))
                    .px(px(0.))
                    .justify_start()
                    .icon(control_icon(
                        if self.agenda.note_expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        },
                        MUTED,
                    ))
                    .child(control_label("备注", MUTED))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toggle_agenda_note(window, cx);
                    })),
            )
            .when(self.agenda.note_expanded, |body| {
                body.child(
                    div()
                        .debug_selector(|| "agenda-note-input".into())
                        .child(Styled::h(
                            field(&self.agenda.note_input).h(px(76.)),
                            px(76.),
                        )),
                )
            })
    }

    fn agenda_validation(&self) -> Div {
        // Keep validation visible outside the scrolling fields. The reserved
        // height also keeps bottom-snapped inputs stationary while it changes.
        div()
            .debug_selector(|| "agenda-validation-slot".into())
            .min_h(px(VALIDATION_HEIGHT))
            .flex_shrink_0()
            .when_some(self.agenda.error.as_ref(), |slot, message| {
                slot.child(
                    div()
                        .debug_selector(|| "agenda-error".into())
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .text_color(rgb(DESTRUCTIVE))
                        .whitespace_normal()
                        .child(message.clone()),
                )
            })
    }

    fn agenda_editor_actions(&self, cx: &mut Context<Self>) -> Div {
        div()
            .debug_selector(|| "agenda-editor-actions".into())
            .flex_shrink_0()
            .pt(px(12.))
            .border_t_1()
            .border_color(rgb(LINE))
            .flex()
            .justify_end()
            .gap(px(8.))
            .child(
                button("agenda-cancel")
                    .debug_selector(|| "agenda-cancel".into())
                    .custom(crate::view::quiet_variant(cx))
                    .h(px(36.))
                    .child(control_label("取消", MUTED))
                    .on_click(cx.listener(|this, _, window, cx| this.cancel_plan_edit(window, cx))),
            )
            .child(
                primary_button("agenda-save")
                    .debug_selector(|| "agenda-save".into())
                    .h(px(36.))
                    .unavailable(self.storage_failed)
                    .child(control_label(
                        "保存",
                        if self.storage_failed { MUTED } else { 0xffffff },
                    ))
                    .on_click(cx.listener(|this, _, window, cx| this.save_plan(window, cx))),
            )
    }

    fn agenda_day_summary(&self, cx: &mut Context<Self>) -> Div {
        let (start, end) = day_bounds(self.agenda.date);
        let mut rows = Vec::new();
        for plan in &self.agenda.plans {
            let id = plan.id;
            let state = self.agenda_plan_status(plan).description(true);
            let color = self.agenda_task_color(plan.task_id);
            rows.push(
                summary_row(
                    cx,
                    format!("agenda-summary-plan-{id}"),
                    &plan.title,
                    color,
                    format!(
                        "{state} · {:02}:{:02}–{:02}:{:02}",
                        plan.start_minute / 60,
                        plan.start_minute % 60,
                        plan.end_minute / 60,
                        plan.end_minute % 60
                    ),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.select_plan(id, window, cx))),
            );
        }
        for record in &self.agenda.records {
            let id = record.id;
            let seconds = record.elapsed_between(start, end, self.now);
            rows.push(
                summary_row(
                    cx,
                    format!("agenda-summary-record-{id}"),
                    &record.title,
                    agenda_tag_color(&self.tags, Some(record.tag_id), record.tag_color.as_deref()),
                    format!(
                        "{} · {}{}",
                        self.agenda_record_status(record).description(false),
                        if record.tag_kind == TagKind::Rest {
                            "休息 · "
                        } else {
                            ""
                        },
                        if seconds > 0 {
                            short_time(seconds)
                        } else {
                            "尚未计时".to_owned()
                        }
                    ),
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.select_agenda_record(id, window, cx)
                    }),
                ),
            );
        }
        for task in self.agenda_unplanned_completions() {
            let id = task.id;
            rows.push(
                summary_row(
                    cx,
                    format!("agenda-summary-completion-{id}"),
                    &task.title,
                    agenda_tag_color(&self.tags, Some(task.tag_id), None),
                    "已完成 · 未计时".to_owned(),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_agenda_completion(id, window, cx)
                })),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .when(rows.is_empty(), |body| {
                body.child(
                    div()
                        .py(px(12.))
                        .text_size(px(14.))
                        .text_color(rgb(MUTED))
                        .child("这一天还没有安排或记录。"),
                )
            })
            .children(rows)
    }
}

fn editor_field(label: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(field_label(label))
}

fn summary_row(
    cx: &App,
    id: String,
    title: &str,
    color: Option<&str>,
    detail: String,
) -> gpui_component::button::Button {
    let selector = id.clone();
    button(SharedString::from(id))
        .debug_selector(move || selector.clone())
        .custom(crate::view::quiet_variant(cx))
        .w_full()
        .h(px(56.))
        .px(px(4.))
        .rounded(px(0.))
        .border_0()
        .border_b_1()
        .border_color(rgb(LINE))
        .tooltip(format!("{title}\n{detail}"))
        .child(
            div()
                .w(px(310.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .w(px(310.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            div()
                                .w(px(3.))
                                .h(px(14.))
                                .flex_shrink_0()
                                .rounded(px(1.))
                                .bg(rgb(color.and_then(tag_color_rgb).unwrap_or(LINE))),
                        )
                        .child(
                            single_line(title.to_owned(), 14.)
                                .w(px(301.))
                                .flex_shrink_0()
                                .text_color(rgb(color.and_then(tag_text_color).unwrap_or(INK))),
                        ),
                )
                .child(single_line(detail, 13.).w(px(310.)).text_color(rgb(MUTED))),
        )
}
