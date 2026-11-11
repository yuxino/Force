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
    life_model::{CheckInMetric, LifeEdit, NextField, PreviousField},
    model::{ForceView, Page},
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, PAPER, SURFACE, button, choice_button,
        control_icon, control_label, field, field_label, outlined_button, primary_button,
        quiet_variant, record_timestamp, section_heading, timer_font,
    },
};

const DISPLAY_MODULES: [(LifeModule, &str); 3] = [
    (LifeModule::Total, "累计统计"),
    (LifeModule::Trend, "趋势图"),
    (LifeModule::Table, "按日明细"),
];
const PLOT_HEIGHT: f32 = 120.;
const SPANS_SHOWN: usize = 8;

impl ForceView {
    fn life_add_button(&self, cx: &mut Context<Self>) -> Button {
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
                this.begin_life_editor(LifeEdit::Add(None), window, cx);
            }))
    }

    pub(crate) fn life_analysis(&self, cx: &mut Context<Self>) -> Div {
        let panel = div()
            .debug_selector(|| "life-analysis".into())
            .w_full()
            .flex()
            .flex_col()
            .gap(px(24.))
            .when_some(self.timer_context(cx), |panel, context| {
                panel.child(context)
            });
        let Some(board) = self.life.board.as_ref().filter(|_| !self.life.load_failed) else {
            return panel.child(self.life_load_error(cx));
        };
        let Some(item) = board
            .items
            .iter()
            .find(|item| Some(item.id) == board.chart_item)
            .or_else(|| {
                board
                    .items
                    .iter()
                    .find(|item| item.kind == LifeKind::CheckIn)
            })
            .or_else(|| board.items.first())
        else {
            return panel.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(control_label("还没有坚持事项。", MUTED))
                    .child(self.life_add_button(cx)),
            );
        };
        let id = item.id;
        let check_in = item.kind == LifeKind::CheckIn;
        let mut options = DISPLAY_MODULES
            .map(|(module, label)| (module, label, board.shows(module)))
            .to_vec();
        options.retain(|(module, _, _)| check_in || *module != LifeModule::Table);
        let any_visible = options.iter().any(|(_, _, visible)| *visible);
        let model = cx.entity().downgrade();
        let draft = self.life.edit.is_some();
        panel
            .debug_selector(move || format!("life-analysis-item-{id}"))
            .child(
                div()
                    .debug_selector(|| "life-analysis-toolbar".into())
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(field_label("事项").flex_shrink_0())
                    .child(life_chart_picker(board, item, cx))
                    .child(div().flex_1())
                    .child(
                        button("life-display")
                            .debug_selector(|| "life-display".into())
                            .custom(quiet_variant(cx))
                            .h(px(32.))
                            .flex_shrink_0()
                            .icon(control_icon(IconName::Settings2, MUTED))
                            .child(control_label("显示", MUTED))
                            .dropdown_menu_with_anchor(Corner::TopRight, move |menu, _, _| {
                                options.iter().fold(
                                    menu.min_w(px(144.)),
                                    |menu, &(module, label, visible)| {
                                        let model = model.clone();
                                        menu.item(
                                            PopupMenuItem::new(label).checked(visible).on_click(
                                                move |_, _, cx| {
                                                    let _ = model.update(cx, |this, cx| {
                                                        this.set_life_module(module, !visible, cx)
                                                    });
                                                },
                                            ),
                                        )
                                    },
                                )
                            }),
                    )
                    .when(!check_in, |toolbar| {
                        toolbar.child(
                            button("life-occurrence")
                                .debug_selector(|| "life-occurrence".into())
                                .custom(quiet_variant(cx))
                                .h(px(32.))
                                .flex_shrink_0()
                                .unavailable(draft)
                                .child(control_label("记录发生", INK))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.begin_life_change(
                                        id,
                                        |item| LifeEdit::Record(item, None),
                                        window,
                                        cx,
                                    );
                                })),
                        )
                    })
                    .child(
                        button("life-edit")
                            .debug_selector(|| "life-edit".into())
                            .custom(quiet_variant(cx))
                            .h(px(32.))
                            .flex_shrink_0()
                            .unavailable(draft)
                            .child(control_label("编辑", MUTED))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.begin_life_change(id, LifeEdit::Edit, window, cx);
                            })),
                    )
                    .child(self.life_add_button(cx).flex_shrink_0()),
            )
            .when(!any_visible, |panel| {
                panel.child(control_label("在「显示」中选择要看的内容。", MUTED))
            })
            .when(board.shows(LifeModule::Total), |panel| {
                panel.child(self.life_total(item, cx))
            })
            .when(board.shows(LifeModule::Trend), |panel| {
                panel.child(self.life_trend(item, cx))
            })
            .when(check_in && board.shows(LifeModule::Table), |panel| {
                panel.child(self.life_analysis_log(item, cx))
            })
    }

    fn life_load_error(&self, cx: &mut Context<Self>) -> Div {
        div()
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
            )
    }

    pub(crate) fn life_day_picker(&self, day: NaiveDate, cx: &mut Context<Self>) -> Div {
        let Some(board) = self.life.board.as_ref().filter(|_| !self.life.load_failed) else {
            return self.life_load_error(cx);
        };
        div()
            .debug_selector(|| "agenda-check-in-picker".into())
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(section_heading("选一个事项"))
            .children(
                board
                    .items
                    .iter()
                    .filter(|item| item.kind == LifeKind::CheckIn)
                    .map(|item| {
                        let id = item.id;
                        button(("agenda-check-in-item", id as u64))
                            .debug_selector(move || format!("agenda-check-in-item-{id}"))
                            .custom(quiet_variant(cx))
                            .h(px(34.))
                            .w_full()
                            .justify_start()
                            .child(single_line(item.title.clone(), 14.).w(px(260.)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.begin_life_change(
                                    id,
                                    |item| LifeEdit::Record(item, Some(day)),
                                    window,
                                    cx,
                                );
                            }))
                    }),
            )
            .child(
                button("agenda-check-in-add")
                    .debug_selector(|| "agenda-check-in-add".into())
                    .custom(quiet_variant(cx))
                    .h(px(34.))
                    .w_full()
                    .justify_start()
                    .icon(control_icon(IconName::Plus, MUTED))
                    .child(control_label("添加打卡事项", INK))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_life_editor(LifeEdit::Add(Some(day)), window, cx);
                    })),
            )
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

    fn life_trend(&self, item: &LifeItem, cx: &mut Context<Self>) -> Div {
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
                    .when(check_in, |header| {
                        header.child(
                            div().flex().gap(px(4.)).children(
                                [
                                    (CheckInMetric::Count, "次数", "life-metric-count"),
                                    (CheckInMetric::Duration, "时长", "life-metric-duration"),
                                ]
                                .into_iter()
                                .map(|(metric, label, id)| {
                                    let selected = self.life.metric == metric;
                                    choice_button(id, selected, SURFACE, cx)
                                        .debug_selector(move || id.into())
                                        .h(px(30.))
                                        .px(px(12.))
                                        .child(control_label(
                                            label,
                                            if selected { INK } else { MUTED },
                                        ))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.life.metric = metric;
                                            cx.notify();
                                        }))
                                }),
                            ),
                        )
                    }),
            );
        if check_in {
            return panel.child(check_in_plot(
                item,
                Local::now().date_naive(),
                self.life.metric,
            ));
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

    fn life_total(&self, item: &LifeItem, cx: &App) -> Div {
        let values = if let Some(last) = item.last_occurrence() {
            let streak = if last.is_day() {
                estimated_interval(last, None, self.now)
            } else {
                life_duration(item.elapsed_at(self.now), false)
            };
            let prefix = if item.has_date_only_entries() {
                "至少 "
            } else {
                ""
            };
            vec![
                ("当前坚持", streak),
                (
                    "累计天数",
                    format!("{prefix}{} 天", item.cumulative_days(self.now)),
                ),
                ("上次发生", occurrence_timestamp(last)),
            ]
        } else {
            let summary = item.check_in_summary(Local::now().date_naive());
            vec![
                ("连续坚持", format!("{} 天", summary.streak_days)),
                ("累计打卡", format!("{} 次", summary.count)),
                ("累计天数", format!("{} 天", summary.total_days)),
                (
                    "累计时长",
                    summary
                        .minutes
                        .map(check_in_duration)
                        .unwrap_or_else(|| "—".to_owned()),
                ),
            ]
        };
        div()
            .debug_selector(|| "life-total".into())
            .flex()
            .gap(px(16.))
            .py(px(14.))
            .border_t_1()
            .border_b_1()
            .border_color(rgb(LINE))
            .children(values.into_iter().map(|(label, value)| {
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(field_label(label))
                    .child(
                        single_line(value, 18.)
                            .font(timer_font(cx))
                            .font_weight(FontWeight::MEDIUM),
                    )
            }))
    }

    fn life_analysis_log(&self, item: &LifeItem, cx: &mut Context<Self>) -> Div {
        let today = Local::now().date_naive();
        let days: Vec<_> = item
            .check_in_days()
            .iter()
            .rev()
            .copied()
            .filter(|entry| (0..7).contains(&today.signed_duration_since(entry.day).num_days()))
            .collect();
        div()
            .debug_selector(|| "life-analysis-log".into())
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(section_heading("近 7 天明细"))
            .when(days.is_empty(), |body| {
                body.child(control_label("这段时间没有打卡。", MUTED))
            })
            .child(div().children(days.into_iter().map(|entry| {
                let day = entry.day;
                let duration = entry.minutes.map(check_in_duration);
                table_row()
                    .min_h(px(38.))
                    .child(
                        div()
                            .flex_1()
                            .child(control_label(day.format("%Y/%m/%d").to_string(), INK)),
                    )
                    .child(control_label(format!("{} 次", entry.count), MUTED))
                    .child(div().w(px(132.)).child(control_label(
                        duration.unwrap_or_else(|| "—".to_owned()),
                        MUTED,
                    )))
                    .child(
                        button(SharedString::from(format!("life-log-day-{day}")))
                            .custom(quiet_variant(cx))
                            .h(px(30.))
                            .child(control_label("查看记录", MUTED))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_record_date(day, cx);
                                this.navigate(Page::Records, window, cx);
                            })),
                    )
            })))
    }

    pub(crate) fn life_editor(
        &self,
        edit: &LifeEdit,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let unavailable = self.storage_failed || self.life.load_failed;
        let check_in = edit.kind(self.life.add_kind) == LifeKind::CheckIn;
        let recording = matches!(edit, LifeEdit::Record(_, _));
        let title = match edit {
            LifeEdit::Add(_) => "添加事项".to_owned(),
            LifeEdit::Edit(_) => "编辑事项".to_owned(),
            LifeEdit::Record(item, _) => format!(
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
            .when(matches!(edit, LifeEdit::Add(None)), |fields| {
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
            .when(matches!(edit, LifeEdit::Add(_)) && check_in, |fields| {
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

fn check_in_plot(item: &LifeItem, today: NaiveDate, metric: CheckInMetric) -> Div {
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
    let value = |count: u64, minutes: Option<u64>| match metric {
        CheckInMetric::Count => count,
        CheckInMetric::Duration => minutes.unwrap_or_default(),
    };
    let peak = days
        .iter()
        .map(|(_, count, minutes)| value(*count, *minutes))
        .max()
        .unwrap_or(1)
        .max(1);
    div()
        .flex()
        .gap(px(10.))
        .child(
            div()
                .w(px(84.))
                .flex_shrink_0()
                .h(px(PLOT_HEIGHT))
                .flex()
                .flex_col()
                .justify_between()
                .text_right()
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .child(match metric {
                    CheckInMetric::Count => format!("{peak} 次"),
                    CheckInMetric::Duration => check_in_duration(peak),
                })
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
                                    if *count > 0 && minutes.is_none() {
                                        detail.push_str(" · 未记时长");
                                    }
                                    let height = (value(*count, *minutes) as f32 / peak as f32)
                                        * (PLOT_HEIGHT - 2.);
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

pub(crate) fn check_in_duration(minutes: u64) -> String {
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
