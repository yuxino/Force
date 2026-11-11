use chrono::{Days, Local, NaiveDate, TimeZone};
use force_core::{LifeBoard, LifeItem, LifeKind, LifeModule, LifeOccurrence, LifeSpan};
use gpui::{prelude::*, *};
use gpui_component::{
    IconName,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
    tooltip::Tooltip,
};

use crate::{
    editor_dialog,
    label::single_line,
    life_model::{LifeEdit, NextField, PreviousField},
    model::{ForceView, Page},
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, PAPER, SURFACE, button, choice_button,
        control_icon, control_label, field, field_label, outlined_button, primary_button,
        quiet_variant, record_timestamp, section_heading, timer_font,
    },
};

const DISPLAY_MODULES: [(LifeModule, &str); 2] = [
    (LifeModule::Trend, "趋势图"),
    (LifeModule::Total, "累计统计"),
];
const SUMMARY_HEADER_HEIGHT: f32 = 32.;
const SUMMARY_ROW_HEIGHT: f32 = 40.;
const PROGRESS_WIDTH: f32 = 220.;
const ACTION_WIDTH: f32 = 154.;
const PLOT_HEIGHT: f32 = 120.;
const SPANS_SHOWN: usize = 8;

impl ForceView {
    pub(crate) fn life_add_button(&self, cx: &mut Context<Self>) -> Button {
        let unavailable = self.storage_failed || self.life.load_failed || self.life.board.is_none();
        button("life-add")
            .debug_selector(|| "life-add".into())
            .custom(quiet_variant(cx))
            .h(px(30.))
            .unavailable(unavailable)
            .when(self.life.edit.is_none(), |button| {
                button.icon(control_icon(IconName::Plus, INK))
            })
            .child(control_label(
                if self.life.edit.is_some() {
                    "继续编辑"
                } else {
                    "添加坚持事项"
                },
                INK,
            ))
            .on_click(cx.listener(|this, _, window, cx| {
                this.begin_life_editor(LifeEdit::Add, window, cx);
            }))
    }

    pub(crate) fn life_summary_height(&self) -> f32 {
        if self.life.load_failed {
            return SUMMARY_HEADER_HEIGHT;
        }
        self.life
            .board
            .as_ref()
            .filter(|board| !board.items.is_empty())
            .map_or(0., |board| {
                SUMMARY_HEADER_HEIGHT + SUMMARY_ROW_HEIGHT * board.items.len() as f32
            })
    }

    pub(crate) fn life_summary(&self, cx: &mut Context<Self>) -> Option<Div> {
        if self.life.load_failed {
            return Some(
                div()
                    .h(px(SUMMARY_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(control_label("无法读取坚持事项。", MUTED))
                    .child(
                        button("life-retry")
                            .custom(quiet_variant(cx))
                            .h(px(30.))
                            .child(control_label("重试", INK))
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_life(cx))),
                    ),
            );
        }
        let board = self
            .life
            .board
            .as_ref()
            .filter(|board| !board.items.is_empty())?;
        let options = DISPLAY_MODULES.map(|(module, label)| (module, label, board.shows(module)));
        let model = cx.entity().downgrade();
        Some(
            div()
                .debug_selector(|| "life-table".into())
                .w_full()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(SUMMARY_HEADER_HEIGHT))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(section_heading("坚持"))
                        .child(div().flex_1())
                        .child(self.life_add_button(cx))
                        .child(
                            button("life-display")
                                .debug_selector(|| "life-display".into())
                                .custom(quiet_variant(cx))
                                .h(px(30.))
                                .icon(control_icon(IconName::Settings2, MUTED))
                                .child(control_label("显示", MUTED))
                                .dropdown_menu_with_anchor(Corner::TopRight, move |menu, _, _| {
                                    options.into_iter().fold(
                                        menu.min_w(px(144.)),
                                        |menu, (module, label, visible)| {
                                            let model = model.clone();
                                            menu.item(
                                                PopupMenuItem::new(label)
                                                    .checked(visible)
                                                    .on_click(move |_, _, cx| {
                                                        let _ = model.update(cx, |this, cx| {
                                                            this.set_life_module(
                                                                module, !visible, cx,
                                                            )
                                                        });
                                                    }),
                                            )
                                        },
                                    )
                                }),
                        ),
                )
                .children(board.items.iter().map(|item| self.life_row(item, cx))),
        )
    }

    pub(crate) fn life_statistics(&self, cx: &mut Context<Self>) -> Option<Div> {
        let board = self
            .life
            .board
            .as_ref()
            .filter(|board| !self.life.load_failed && !board.items.is_empty())?;
        let trend = board.shows(LifeModule::Trend);
        let total = board.shows(LifeModule::Total);
        (trend || total).then(|| {
            div()
                .w_full()
                .flex_shrink_0()
                .pt(px(20.))
                .border_t_1()
                .border_color(rgb(LINE))
                .flex()
                .items_start()
                .gap(px(32.))
                .when(trend, |row| row.child(self.life_trend(board, cx)))
                .when(total, |row| row.child(self.life_total(board, trend, cx)))
        })
    }

    pub(crate) fn life_day_records(&self, day: NaiveDate, cx: &mut Context<Self>) -> Option<Div> {
        let mut check_ins = self.life.check_ins_on(day).peekable();
        check_ins.peek()?;
        Some(
            div()
                .debug_selector(|| "agenda-day-checkins".into())
                .pt(px(6.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(section_heading("打卡"))
                        .child(
                            button("agenda-check-in-records")
                                .debug_selector(|| "agenda-check-in-records".into())
                                .custom(quiet_variant(cx))
                                .h(px(28.))
                                .child(control_label("查看记录", MUTED))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_record_date(day, cx);
                                    this.navigate(Page::Records, window, cx);
                                })),
                        ),
                )
                .children(check_ins.map(|(item, entry)| {
                    let id = item.id;
                    let detail = match entry.minutes {
                        Some(minutes) => {
                            format!("{} 次 · {}", entry.count, check_in_duration(minutes))
                        }
                        None => format!("{} 次", entry.count),
                    };
                    table_row()
                        .debug_selector(move || format!("agenda-day-checkin-{id}"))
                        .min_h(px(34.))
                        .child(life_name(item, "life-day-name").flex_1().min_w_0())
                        .child(control_label(detail, MUTED).flex_shrink_0())
                })),
        )
    }

    fn life_row(&self, item: &LifeItem, cx: &mut Context<Self>) -> Div {
        let id = item.id;
        let check_in = item.kind == LifeKind::CheckIn;
        let (label, detail) = if let Some(last) = item.last_occurrence() {
            let elapsed = item.elapsed_at(self.now);
            let label = if last.is_day() {
                estimated_interval(last, None, self.now)
            } else if elapsed < 86_400 {
                "未满 1 天".to_owned()
            } else {
                format!("{} 天", elapsed / 86_400)
            };
            let detail = format!("上次发生：{}", occurrence_timestamp(last));
            (label, detail)
        } else {
            let summary = item.check_in_summary(Local::now().date_naive());
            (
                format!(
                    "连续 {} 天 · 打卡 {} 次",
                    summary.streak_days, summary.count
                ),
                "今天还没打卡时，连续天数保留到今天结束。".to_owned(),
            )
        };
        table_row()
            .debug_selector(move || format!("life-row-{id}"))
            .h(px(SUMMARY_ROW_HEIGHT))
            .flex_shrink_0()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(life_name(item, "life-table-name").min_w_0())
                    .when(!check_in, |row| {
                        row.child(field_label("坚持不做").flex_shrink_0())
                    }),
            )
            .child(
                single_line(label, 14.)
                    .id(("life-streak", id as u64))
                    .w(px(PROGRESS_WIDTH))
                    .flex_shrink_0()
                    .font_weight(FontWeight::MEDIUM)
                    .tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx)),
            )
            .child(
                div()
                    .w(px(ACTION_WIDTH))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .gap(px(6.))
                    .child(
                        outlined_button(("life-record", id as u64))
                            .debug_selector(move || format!("life-record-{id}"))
                            .h(px(32.))
                            .child(control_label(
                                if check_in { "打卡" } else { "记录发生" },
                                INK,
                            ))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.begin_life_change(id, LifeEdit::Record, window, cx);
                            })),
                    )
                    .child(
                        button(("life-edit", id as u64))
                            .debug_selector(move || format!("life-edit-{id}"))
                            .custom(quiet_variant(cx))
                            .h(px(32.))
                            .child(control_label("编辑", MUTED))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.begin_life_change(id, LifeEdit::Edit, window, cx);
                            })),
                    ),
            )
    }

    fn life_trend(&self, board: &LifeBoard, cx: &mut Context<Self>) -> Div {
        let item = board
            .items
            .iter()
            .find(|item| Some(item.id) == board.chart_item)
            .or_else(|| {
                board
                    .items
                    .iter()
                    .find(|item| item.kind == LifeKind::CheckIn)
            })
            .unwrap_or(&board.items[0]);
        let check_in = item.kind == LifeKind::CheckIn;
        let panel = div()
            .debug_selector(|| "life-trend".into())
            .min_w_0()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        section_heading(if check_in {
                            "近 7 天打卡"
                        } else {
                            "间隔趋势"
                        })
                        .flex_shrink_0(),
                    )
                    .child(life_chart_picker(board, item, cx)),
            );
        if check_in {
            return panel.child(check_in_plot(item, Local::now().date_naive()));
        }
        let spans = item.recent_spans(self.now, SPANS_SHOWN);
        let peak = spans
            .iter()
            .map(|span| chart_seconds(*span, self.now))
            .max()
            .unwrap_or(0);
        let peak = if spans.iter().any(|span| span.is_estimated()) {
            ((peak as u64).div_ceil(86_400).max(1) * 86_400) as i64
        } else {
            peak
        };
        let axis = div()
            .w(px(88.))
            .flex_shrink_0()
            .h(px(PLOT_HEIGHT))
            .flex()
            .flex_col()
            .justify_between()
            .text_size(px(12.))
            .text_color(rgb(MUTED))
            .text_right()
            .child(life_duration(peak, false))
            .child("0");
        panel.child(
            div().flex().gap(px(10.)).child(axis).child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .debug_selector(|| "life-trend-plot".into())
                            .h(px(PLOT_HEIGHT))
                            .border_b_1()
                            .border_t_1()
                            .border_color(rgb(LINE))
                            .flex()
                            .children(
                                spans
                                    .iter()
                                    .copied()
                                    .enumerate()
                                    .map(|(index, span)| life_bar(index, span, peak, self.now)),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(6.))
                            .flex()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .children(spans.iter().map(|span| {
                                let label = span.end.map_or_else(
                                    || "当前".to_owned(),
                                    |at| {
                                        occurrence_date(at).map_or_else(
                                            || "—".to_owned(),
                                            |time| time.format("%-m/%-d").to_string(),
                                        )
                                    },
                                );
                                let label = if span.is_estimated() {
                                    format!("约 · {label}")
                                } else {
                                    label
                                };
                                div().flex_1().min_w_0().text_center().child(label)
                            })),
                    ),
            ),
        )
    }

    fn life_total(&self, board: &LifeBoard, beside_trend: bool, cx: &App) -> Div {
        div()
            .debug_selector(|| "life-total".into())
            .w(px(if beside_trend { 218. } else { 360. }))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(section_heading("累计"))
                    .child(
                        button("life-total-help")
                            .custom(quiet_variant(cx))
                            .size(px(24.))
                            .icon(control_icon(IconName::Info, MUTED))
                            .tooltip("打卡天数按日期去重。戒除事项按各段能确认的满 24 小时累计。"),
                    ),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .children(board.items.iter().map(|item| {
                        let (total, duration) = if item.kind == LifeKind::CheckIn {
                            let summary = item.check_in_summary(Local::now().date_naive());
                            (
                                format!("{} 天 · {} 次", summary.total_days, summary.count),
                                summary.minutes.map(check_in_duration),
                            )
                        } else {
                            let prefix = if item.has_date_only_entries() {
                                "至少 "
                            } else {
                                ""
                            };
                            (
                                format!("{prefix}{} 天", item.cumulative_days(self.now)),
                                None,
                            )
                        };
                        table_row()
                            .min_h(px(52.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(life_name(item, "life-total-name")),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .items_end()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .font(timer_font(cx))
                                            .text_size(px(15.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(total),
                                    )
                                    .when_some(duration, |row, duration| {
                                        row.child(
                                            div()
                                                .text_size(px(13.))
                                                .text_color(rgb(MUTED))
                                                .child(duration),
                                        )
                                    }),
                            )
                    })),
            )
    }

    pub(crate) fn life_editor(
        &self,
        edit: &LifeEdit,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let unavailable = self.storage_failed || self.life.load_failed;
        let check_in = edit.kind(self.life.add_kind) == LifeKind::CheckIn;
        let recording = matches!(edit, LifeEdit::Record(_));
        let title = match edit {
            LifeEdit::Add => "添加事项".to_owned(),
            LifeEdit::Edit(_) => "编辑事项".to_owned(),
            LifeEdit::Record(item) => format!(
                "{} · {}",
                if check_in { "打卡" } else { "记录发生" },
                item.title
            ),
        };
        let close = button("life-close")
            .debug_selector(|| "life-close".into())
            .custom(quiet_variant(cx))
            .size(px(28.))
            .icon(control_icon(IconName::Close, MUTED))
            .tooltip("关闭并保留草稿")
            .on_click(cx.listener(|this, _, window, cx| this.suspend_life_editor(window, cx)));
        let fields = editor_dialog::fields("life-editor-fields", &self.life.scroll)
            .when(!recording, |fields| {
                fields.child(
                    div()
                        .debug_selector(|| "life-name".into())
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(field_label("事项名称"))
                        .child(field(&self.life.name)),
                )
            })
            .when(matches!(edit, LifeEdit::Add), |fields| {
                fields.child(
                    div().flex().gap(px(6.)).children(
                        [
                            (LifeKind::CheckIn, "坚持做", "life-kind-check-in"),
                            (LifeKind::Abstain, "坚持不做", "life-kind-abstain"),
                        ]
                        .into_iter()
                        .map(|(kind, label, id)| {
                            let selected = self.life.add_kind == kind;
                            choice_button(id, selected, SURFACE, cx)
                                .debug_selector(move || id.into())
                                .h(px(32.))
                                .px(px(10.))
                                .child(control_label(label, if selected { INK } else { MUTED }))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.life.add_kind = kind;
                                    this.life.error = None;
                                    cx.notify();
                                }))
                        }),
                    ),
                )
            })
            .when(matches!(edit, LifeEdit::Add) && check_in, |fields| {
                fields.child(field_label("同名的按日记录会计入这项打卡。"))
            })
            .when(!check_in, |fields| {
                fields.child(
                    div().flex().gap(px(6.)).children(
                        [
                            (false, "日期和时间", "life-exact"),
                            (true, "只记日期", "life-day-only"),
                        ]
                        .into_iter()
                        .map(|(day_only, label, id)| {
                            choice_button(id, self.life.day_only == day_only, SURFACE, cx)
                                .debug_selector(move || id.into())
                                .h(px(32.))
                                .px(px(10.))
                                .child(control_label(
                                    label,
                                    if self.life.day_only == day_only {
                                        INK
                                    } else {
                                        MUTED
                                    },
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.life.day_only = day_only;
                                    this.life.error = None;
                                    cx.notify();
                                }))
                        }),
                    ),
                )
            })
            .when(!check_in || recording, |fields| {
                fields.child(
                    div()
                        .mt(px(4.))
                        .flex()
                        .gap(px(12.))
                        .child(
                            div()
                                .debug_selector(|| "life-date".into())
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(field_label(if check_in {
                                    "日期"
                                } else if recording {
                                    "发生日期"
                                } else {
                                    "上次发生日期"
                                }))
                                .child(field(&self.life.date)),
                        )
                        .when(!check_in && !self.life.day_only, |row| {
                            row.child(
                                div()
                                    .debug_selector(|| "life-time".into())
                                    .w(px(148.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(field_label("时间"))
                                    .child(field(&self.life.time)),
                            )
                        })
                        .when(check_in, |row| {
                            row.child(
                                div()
                                    .debug_selector(|| "life-check-in-duration".into())
                                    .w(px(168.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(field_label("时长（分钟，可选）"))
                                    .child(field(&self.life.minutes)),
                            )
                        }),
                )
            });
        let footer = editor_dialog::footer("life-editor-actions")
            .when(matches!(edit, LifeEdit::Edit(_)), |footer| {
                footer
                    .child(
                        button("life-delete")
                            .debug_selector(|| "life-delete".into())
                            .custom(quiet_variant(cx))
                            .h(px(36.))
                            .unavailable(unavailable)
                            .child(control_label("删除事项", DESTRUCTIVE))
                            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                                if event.click_count() == 1 {
                                    this.delete_life_item(window, cx);
                                }
                            })),
                    )
                    .child(div().flex_1())
            })
            .child(
                button("life-cancel")
                    .debug_selector(|| "life-cancel".into())
                    .custom(quiet_variant(cx))
                    .h(px(36.))
                    .child(control_label("取消", MUTED))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.cancel_life_editor(window, cx)),
                    ),
            )
            .child(
                primary_button("life-save")
                    .debug_selector(|| "life-save".into())
                    .h(px(36.))
                    .unavailable(unavailable)
                    .child(control_label("保存", PAPER))
                    .on_click(cx.listener(|this, _, window, cx| this.save_life_item(window, cx))),
            );
        let panel = editor_dialog::panel("life-editor", window)
            .track_focus(&self.life.focus)
            .key_context("LifeEditor")
            .on_action(cx.listener(|this, _: &NextField, window, cx| {
                this.cycle_life_focus(false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &PreviousField, window, cx| {
                this.cycle_life_focus(true, window, cx);
            }))
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                if event.position.y >= px(68.) {
                    this.suspend_life_editor(window, cx);
                }
            }))
            .child(editor_dialog::heading(title, close))
            .child(fields)
            .child(editor_dialog::validation(
                "life-editor-message",
                self.life.error,
            ))
            .child(footer);
        editor_dialog::overlay("life-editor-overlay", window, panel)
    }
}

fn check_in_plot(item: &LifeItem, today: NaiveDate) -> Div {
    let days: Vec<_> = (0..7)
        .rev()
        .filter_map(|age| today.checked_sub_days(Days::new(age)))
        .map(|day| {
            let history = item.check_in_days();
            let summary = history
                .binary_search_by_key(&day, |entry| entry.day)
                .ok()
                .map(|index| history[index]);
            (
                day,
                summary.map_or(0, |entry| entry.count),
                summary.and_then(|entry| entry.minutes),
            )
        })
        .collect();
    let peak = days
        .iter()
        .map(|(_, count, _)| *count)
        .max()
        .unwrap_or(1)
        .max(1);
    div()
        .flex()
        .gap(px(10.))
        .child(
            div()
                .w(px(38.))
                .flex_shrink_0()
                .h(px(PLOT_HEIGHT))
                .flex()
                .flex_col()
                .justify_between()
                .text_right()
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .child(format!("{peak} 次"))
                .child("0"),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(PLOT_HEIGHT))
                        .flex()
                        .border_b_1()
                        .border_t_1()
                        .border_color(rgb(LINE))
                        .children(
                            days.iter()
                                .enumerate()
                                .map(|(index, (day, count, minutes))| {
                                    let mut detail =
                                        format!("{} · {} 次", day.format("%Y/%m/%d"), count);
                                    if let Some(minutes) = minutes {
                                        detail.push_str(&format!(
                                            " · {}",
                                            check_in_duration(*minutes)
                                        ));
                                    }
                                    let height = (*count as f32 / peak as f32) * (PLOT_HEIGHT - 2.);
                                    div()
                                        .id(("life-check-in-bar", index as u64))
                                        .flex_1()
                                        .min_w_0()
                                        .h_full()
                                        .flex()
                                        .items_end()
                                        .justify_center()
                                        .tooltip(move |window, cx| {
                                            Tooltip::new(detail.clone()).build(window, cx)
                                        })
                                        .child(div().w(px(14.)).h(px(height)).bg(rgb(INK)))
                                }),
                        ),
                )
                .child(
                    div()
                        .mt(px(6.))
                        .flex()
                        .text_size(px(12.))
                        .text_color(rgb(MUTED))
                        .children(days.iter().map(|(day, _, _)| {
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_center()
                                .child(day.format("%-m/%-d").to_string())
                        })),
                ),
        )
}

fn check_in_duration(minutes: u64) -> String {
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} 分钟"),
        (hours, 0) => format!("{hours} 小时"),
        (hours, minutes) => format!("{hours} 小时 {minutes} 分钟"),
    }
}

fn table_row() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .border_b_1()
        .border_color(rgb(LINE))
}

fn life_name(item: &LifeItem, scope: &'static str) -> Stateful<Div> {
    let title = item.title.clone();
    single_line(title.clone(), 14.)
        .id((scope, item.id as u64))
        .tooltip(move |window, cx| Tooltip::new(title.clone()).build(window, cx))
}

fn life_chart_picker(
    board: &LifeBoard,
    selected: &LifeItem,
    cx: &mut Context<ForceView>,
) -> AnyElement {
    if board.items.len() == 1 {
        return div()
            .w(px(174.))
            .flex_shrink_0()
            .child(life_name(selected, "life-chart-name"))
            .into_any_element();
    }
    let model = cx.entity().downgrade();
    let selected_id = selected.id;
    let options: Vec<_> = board
        .items
        .iter()
        .map(|item| (item.id, item.title.clone()))
        .collect();
    outlined_button("life-chart-item")
        .debug_selector(|| "life-chart-item".into())
        .h(px(32.))
        .w(px(174.))
        .child(
            single_line(selected.title.clone(), 13.)
                .w(px(120.))
                .text_color(rgb(INK)),
        )
        .child(control_icon(IconName::ChevronDown, MUTED))
        .dropdown_menu_with_anchor(Corner::TopRight, move |menu, _, _| {
            options.iter().fold(
                menu.min_w(px(174.))
                    .max_w(px(260.))
                    .max_h(px(240.))
                    .scrollable(true),
                |menu, (id, title)| {
                    let id = *id;
                    let model = model.clone();
                    menu.item(
                        PopupMenuItem::new(title.clone())
                            .checked(id == selected_id)
                            .on_click(move |_, _, cx| {
                                let _ = model
                                    .update(cx, |this, cx| this.select_life_chart_item(id, cx));
                            }),
                    )
                },
            )
        })
        .into_any_element()
}

fn life_bar(index: usize, span: LifeSpan, peak: i64, now: i64) -> Stateful<Div> {
    let seconds = chart_seconds(span, now);
    let ratio = seconds as f32 / peak.max(1) as f32;
    let height = if seconds == 0 {
        0.
    } else {
        (ratio * (PLOT_HEIGHT - 2.)).max(2.)
    };
    let current = span.end.is_none();
    let from = occurrence_timestamp(span.start);
    let until = span
        .end
        .map_or_else(|| "当前".to_owned(), occurrence_timestamp);
    let label = if span.is_estimated() {
        estimated_interval(span.start, span.end, now)
    } else {
        life_duration(span.seconds, true)
    };
    let detail = format!("{from} 至 {until} · {label}");
    div()
        .id(("life-chart-span", index as u64))
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .items_end()
        .justify_center()
        .tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx))
        .child(
            div()
                .w(px(18.))
                .h(px(height))
                .bg(rgb(if span.is_estimated() {
                    0xbdbdbd
                } else if current {
                    INK
                } else {
                    0xa8a8a8
                })),
        )
}

fn occurrence_date(occurrence: LifeOccurrence) -> Option<NaiveDate> {
    match occurrence {
        LifeOccurrence::Exact(at) => Local
            .timestamp_opt(at, 0)
            .single()
            .map(|at| at.date_naive()),
        LifeOccurrence::Day { date, .. } => Some(date),
    }
}

fn occurrence_timestamp(occurrence: LifeOccurrence) -> String {
    match occurrence {
        LifeOccurrence::Exact(at) => record_timestamp(at),
        LifeOccurrence::Day { date, .. } => format!("{} · 仅日期", date.format("%Y/%m/%d")),
    }
}

fn estimate_days(start: LifeOccurrence, end: Option<LifeOccurrence>, now: i64) -> i64 {
    let start = occurrence_date(start);
    let end = occurrence_date(end.unwrap_or(LifeOccurrence::Exact(now)));
    start
        .zip(end)
        .map_or(0, |(start, end)| (end - start).num_days().max(0))
}

fn estimated_interval(start: LifeOccurrence, end: Option<LifeOccurrence>, now: i64) -> String {
    let days = estimate_days(start, end, now);
    if days == 0 {
        "同一天".to_owned()
    } else {
        format!("约 {days} 天")
    }
}

fn chart_seconds(span: LifeSpan, now: i64) -> i64 {
    if span.is_estimated() {
        estimate_days(span.start, span.end, now) * 86_400
    } else {
        span.seconds
    }
}

fn life_duration(seconds: i64, exact: bool) -> String {
    let parts = [
        (seconds / 86_400, "天"),
        (seconds / 3_600 % 24, "小时"),
        (seconds / 60 % 60, "分"),
        (seconds % 60, "秒"),
    ];
    let mut shown = parts.into_iter().filter(|(amount, _)| *amount > 0);
    let Some((amount, unit)) = shown.next() else {
        return "0 秒".to_owned();
    };
    let mut label = format!("{amount} {unit}");
    for (amount, unit) in shown.take(if exact { 3 } else { 1 }) {
        label.push_str(&format!(" {amount} {unit}"));
    }
    label
}
