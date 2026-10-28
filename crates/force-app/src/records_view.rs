use chrono::{Days, Local};
use force_core::{Session, TagClass, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{Icon, IconName, button::ButtonVariants, tooltip::Tooltip};

use crate::{
    label::single_line,
    model::{ForceView, day_bounds},
    view::{
        ControlState, INK, LINE, MUTED, SURFACE, button, choice_button, clock, control_icon,
        control_label, destructive_button, field, record_detail, record_timestamp, session_times,
        short_time, timer_font,
    },
};

const DURATION_WIDTH: f32 = 112.;
const ACTION_WIDTH: f32 = 80.;

impl ForceView {
    fn record_date_controls(&self, cx: &mut Context<Self>) -> Div {
        div()
            .debug_selector(|| "record-date-controls".into())
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(6.))
            .child(
                button("previous-day")
                    .debug_selector(|| "previous-day".into())
                    .custom(crate::view::quiet_variant(cx))
                    .size(px(32.))
                    .icon(control_icon(IconName::ChevronLeft, INK))
                    .tooltip("前一天")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(date) = this.record_date.checked_sub_days(Days::new(1)) {
                            this.select_record_date(date, cx);
                        }
                    })),
            )
            .child(
                div()
                    .debug_selector(|| "record-date-label".into())
                    .text_size(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.record_date.format("%Y年%-m月%-d日").to_string()),
            )
            .child(
                button("next-day")
                    .debug_selector(|| "next-day".into())
                    .custom(crate::view::quiet_variant(cx))
                    .size(px(32.))
                    .icon(control_icon(
                        IconName::ChevronRight,
                        if self.record_date >= Local::now().date_naive() {
                            MUTED
                        } else {
                            INK
                        },
                    ))
                    .tooltip("后一天")
                    .unavailable(self.record_date >= Local::now().date_naive())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(date) = this.record_date.checked_add_days(Days::new(1)) {
                            this.select_record_date(date, cx);
                        }
                    })),
            )
            .when(self.record_date != Local::now().date_naive(), |row| {
                row.child(
                    button("record-today")
                        .custom(crate::view::quiet_variant(cx))
                        .h(px(32.))
                        .px(px(10.))
                        .child(control_label("今天", INK))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.select_record_date(Local::now().date_naive(), cx)
                        })),
                )
            })
    }

    fn clear_record_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.record_search.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
    }

    pub(crate) fn records_panel(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let query = self.record_search.read(cx).value().trim().to_lowercase();
        let records: Vec<_> = self
            .history
            .iter()
            .filter(|session| {
                self.record_class_filter
                    .is_none_or(|class| session.tag_class == class)
                    && (query.is_empty()
                        || session.title.to_lowercase().contains(&query)
                        || session.tag_name.to_lowercase().contains(&query))
            })
            .collect();

        div()
            .debug_selector(|| "records-content".into())
            .w_full()
            .flex()
            .flex_col()
            .gap(px(20.))
            .when_some(self.timer_context(cx), |body, context| body.child(context))
            .child(self.activity_calendar(window, cx))
            .child(
                div()
                    .debug_selector(|| "record-day-summary".into())
                    .pt(px(20.))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(24.))
                    .child(self.record_date_controls(cx))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .flex_shrink_0()
                            .gap(px(8.))
                            .child(
                                div().debug_selector(|| "record-total".into())
                                    .font(timer_font(cx)).text_size(px(24.))
                                    .line_height(relative(1.2)).font_weight(FontWeight::MEDIUM)
                                    .text_align(TextAlign::Right)
                                    .child(short_time(self.record_stats.focus_seconds).replace(' ', "")),
                            )
                            .child(div().text_size(px(13.)).text_color(rgb(MUTED)).child("专注时长"))
                            .child(
                                choice_button("record-help", self.record_help_open, SURFACE, cx)
                                    .size(px(24.))
                                    .icon(control_icon(Icon::new(IconName::Info).size(px(13.)), MUTED))
                                    .tooltip("统计说明")
                                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                        if event.click_count() == 1 {
                                            this.record_help_open = !this.record_help_open;
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            )
            .when(self.record_help_open, |body| body.child(
                div().pl(px(12.)).border_l_2().border_color(rgb(LINE))
                    .text_size(px(13.)).text_color(rgb(MUTED)).whitespace_normal()
                    .child("专注标签的正向、负向及原有未分类记录计入专注；休息、中性和休闲不计入。暂停不计时，重叠时段只计一次，跨日按天统计。搜索和分类只筛选列表。"),
            ))
            .child(
                div().flex().flex_col()
                    .child(
                        div().debug_selector(|| "record-list-toolbar".into())
                            .pb(px(12.)).border_b_1().border_color(rgb(LINE))
                            .flex().items_center().justify_between().gap(px(24.))
                            .child(div().text_size(px(14.)).text_color(rgb(MUTED))
                                .child(if query.is_empty() && self.record_class_filter.is_none() { format!("{} 条计时记录", self.history.len()) }
                                    else { format!("匹配 {} 条计时记录", records.len()) }))
                            .child(
                                div().w(px(240.)).flex_shrink_0()
                                    .child(field(&self.record_search)
                                        .prefix(Icon::new(IconName::Search).size(px(14.)).text_color(rgb(MUTED)))
                                        .when(!query.is_empty(), |input| input.suffix(
                                            button("clear-record-search")
                                                .debug_selector(|| "clear-record-search".into())
                                                .custom(crate::view::quiet_variant(cx)).size(px(24.))
                                                .icon(control_icon(Icon::new(IconName::Close).size(px(13.)), MUTED))
                                                .tooltip("清空搜索")
                                                .on_click(cx.listener(|this, _, window, cx| this.clear_record_search(window, cx))),
                                        ))),
                            ),
                    )
                    .child(self.record_moment_section(&query, cx))
                    .child(div().debug_selector(|| "record-class-filters".into()).flex().flex_wrap().gap(px(6.)).py(px(10.))
                        .children([
                            (None, "全部", "all"),
                            (Some(TagClass::Positive), "正向", "positive"),
                            (Some(TagClass::Neutral), "中性", "neutral"),
                            (Some(TagClass::Negative), "负向", "negative"),
                            (Some(TagClass::Leisure), "休闲", "leisure"),
                            (Some(TagClass::Legacy), "未分类", "legacy"),
                        ].into_iter().enumerate().map(|(index, (class, label, key))| {
                            let selected = self.record_class_filter == class;
                            choice_button(("record-class-filter", index as u64), selected, SURFACE, cx)
                                .debug_selector(move || format!("record-filter-{key}"))
                                .h(px(32.)).px(px(10.))
                                .child(control_label(label, if selected { INK } else { MUTED }))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.record_class_filter = class;
                                    cx.notify();
                                }))
                        })))
                    .children(records.iter().map(|session| self.record_row(session, cx)))
                    .when(records.is_empty(), |body| body.child(
                        div().debug_selector(|| "record-empty-state".into()).py(px(32.))
                            .flex().items_center().gap(px(12.))
                            .text_size(px(14.)).text_color(rgb(MUTED))
                            .child(if query.is_empty() && self.record_class_filter.is_none() { "当天没有计时记录" } else { "没有匹配的记录" })
                            .when(!query.is_empty() || self.record_class_filter.is_some(), |row| row.child(
                                crate::view::outlined_button("clear-empty-record-search")
                                    .debug_selector(|| "clear-empty-record-search".into())
                                    .custom(crate::view::quiet_variant(cx)).h(px(32.)).px(px(8.))
                                    .child(control_label(if self.record_class_filter.is_some() { "清除筛选" } else { "清空搜索" }, INK))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.record_class_filter = None;
                                        this.clear_record_search(window, cx);
                                    })),
                            )),
                    )),
            )
    }

    fn record_moment_section(&self, query: &str, cx: &mut Context<Self>) -> Div {
        let moments: Vec<_> = self
            .record_moments
            .iter()
            .filter(|moment| query.is_empty() || moment.content.to_lowercase().contains(query))
            .collect();
        let no_matches = !query.is_empty() && moments.is_empty();
        div()
            .debug_selector(|| "record-moments".into())
            .py(px(12.))
            .border_b_1()
            .border_color(rgb(LINE))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .debug_selector(|| "record-moments-header".into())
                    .min_h(px(32.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("时刻"),
                            )
                            .when(!moments.is_empty(), |heading| {
                                heading.child(
                                    div()
                                        .debug_selector(|| "record-moment-count".into())
                                        .text_size(px(13.))
                                        .text_color(rgb(MUTED))
                                        .child(format!("{} 条", moments.len())),
                                )
                            }),
                    )
                    .child(
                        button("record-add-moment")
                            .debug_selector(|| "record-add-moment".into())
                            .custom(crate::view::quiet_variant(cx))
                            .h(px(32.))
                            .px(px(8.))
                            .unavailable(self.storage_failed)
                            .icon(control_icon(Icon::new(IconName::Plus).size(px(14.)), INK))
                            .child(control_label("随手记", INK))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_moment(Some(this.record_date), window, cx)
                            })),
                    ),
            )
            .children(
                moments
                    .into_iter()
                    .map(|moment| self.moment_row(moment, cx)),
            )
            .when(no_matches, |section| {
                section.child(
                    div()
                        .debug_selector(|| "record-moments-empty-search".into())
                        .py(px(4.))
                        .text_size(px(13.))
                        .text_color(rgb(MUTED))
                        .child("没有匹配的时刻"),
                )
            })
    }

    fn record_row(&self, session: &Session, cx: &mut Context<Self>) -> Div {
        let id = session.id;
        let filter_key = self
            .record_class_filter
            .map_or("all", crate::settings_view::tag_class_key);
        let expanded = self.record_details == Some(id);
        let help_title = session.title.clone();
        let (start, end) = day_bounds(self.record_date);
        let day_seconds = session.elapsed_between(start, end, self.now);
        let total_seconds = session.elapsed_at(self.now);
        let metadata = if expanded || session.title == session.tag_name {
            session_times(session)
        } else {
            format!("{} · {}", session_times(session), session.tag_name)
        };
        let status = match (
            session.tag_kind,
            session.finished_at.is_some(),
            session.is_running(),
        ) {
            (TagKind::Rest, true, _) => Some("休息"),
            (TagKind::Rest, false, true) => Some("休息中"),
            (TagKind::Rest, false, false) => Some("休息 · 已暂停"),
            _ => None,
        };

        div()
            .debug_selector(move || format!("record-row-{id}"))
            .py(px(16.))
            .border_b_1()
            .border_color(rgb(LINE))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(20.))
                    .child(
                        div()
                            .debug_selector(move || format!("record-title-column-{id}"))
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_start()
                            .gap(px(8.))
                            .child(
                                choice_button(("record-details", id as u64), expanded, SURFACE, cx)
                                    .debug_selector(move || format!("record-details-{id}"))
                                    .size(px(28.))
                                    .flex_shrink_0()
                                    .icon(control_icon(
                                        Icon::new(if expanded {
                                            IconName::ChevronDown
                                        } else {
                                            IconName::ChevronRight
                                        })
                                        .size(px(14.)),
                                        MUTED,
                                    ))
                                    .tooltip(if expanded {
                                        "收起详情"
                                    } else {
                                        "查看完整记录"
                                    })
                                    .on_click(cx.listener(
                                        move |this, event: &ClickEvent, _, cx| {
                                            if event.click_count() == 1 {
                                                this.record_details =
                                                    if this.record_details == Some(id) {
                                                        None
                                                    } else {
                                                        Some(id)
                                                    };
                                                cx.notify();
                                            }
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .pt(px(3.))
                                    .child(if expanded {
                                        div()
                                            .w_full()
                                            .whitespace_normal()
                                            .text_size(px(15.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(session.title.clone())
                                            .id(("record-title", id as u64))
                                    } else {
                                        single_line(session.title.clone(), 15.)
                                            .font_weight(FontWeight::MEDIUM)
                                            .id(("record-title", id as u64))
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(help_title.clone()).build(window, cx)
                                            })
                                    })
                                    .child(
                                        div()
                                            .debug_selector(move || {
                                                format!("record-visible-{filter_key}-{id}")
                                            })
                                            .mt(px(5.))
                                            .flex()
                                            .items_center()
                                            .gap(px(6.))
                                            .when_some(
                                                self.display_session_color(session)
                                                    .and_then(crate::settings_view::tag_color_rgb),
                                                |line, color| {
                                                    line.child(
                                                        div()
                                                            .size(px(8.))
                                                            .flex_shrink_0()
                                                            .rounded_full()
                                                            .bg(rgb(color)),
                                                    )
                                                },
                                            )
                                            .child(
                                                single_line(
                                                    format!(
                                                        "{} · {}",
                                                        metadata,
                                                        crate::settings_view::tag_class_label(
                                                            session.tag_class
                                                        )
                                                    ),
                                                    13.,
                                                )
                                                .min_w_0()
                                                .text_color(rgb(MUTED)),
                                            ),
                                    )
                                    .when_some(status, |body, status| {
                                        body.child(
                                            div()
                                                .mt(px(5.))
                                                .text_size(px(13.))
                                                .text_color(rgb(if session.is_running() {
                                                    INK
                                                } else {
                                                    MUTED
                                                }))
                                                .child(status),
                                        )
                                    })
                                    .when(session.timing_estimated, |body| {
                                        body.child(
                                            div()
                                                .debug_selector(move || {
                                                    format!("record-estimated-{id}")
                                                })
                                                .mt(px(5.))
                                                .text_size(px(12.))
                                                .text_color(rgb(MUTED))
                                                .child("时段为估算"),
                                        )
                                    })
                                    .when(
                                        session.timing_adjusted && !session.timing_estimated,
                                        |body| {
                                            body.child(
                                                div()
                                                    .debug_selector(move || {
                                                        format!("record-rounded-{id}")
                                                    })
                                                    .mt(px(5.))
                                                    .text_size(px(12.))
                                                    .text_color(rgb(MUTED))
                                                    .child("时间按秒取整"),
                                            )
                                        },
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(move || format!("record-duration-column-{id}"))
                            .w(px(DURATION_WIDTH))
                            .flex_shrink_0()
                            .pt(px(3.))
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(5.))
                            .text_align(TextAlign::Right)
                            .child(
                                div()
                                    .debug_selector(move || format!("record-day-time-{id}"))
                                    .w_full()
                                    .whitespace_normal()
                                    .font(timer_font(cx))
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("当日 {}", clock(day_seconds))),
                            )
                            .when(day_seconds != total_seconds, |body| {
                                body.child(
                                    div()
                                        .w_full()
                                        .whitespace_normal()
                                        .text_size(px(13.))
                                        .text_color(rgb(MUTED))
                                        .child(format!("整段 {}", clock(total_seconds))),
                                )
                            }),
                    )
                    .child(
                        div()
                            .debug_selector(move || format!("record-action-column-{id}"))
                            .w(px(ACTION_WIDTH))
                            .flex_shrink_0()
                            .min_h(px(32.))
                            .flex()
                            .justify_end()
                            .when(session.finished_at.is_some(), |actions| {
                                actions.child(
                                    destructive_button(("delete-record", id as u64))
                                        .custom(crate::view::quiet_variant(cx))
                                        .size(px(28.))
                                        .icon(control_icon(
                                            Icon::default()
                                                .path("force-icons/trash.svg")
                                                .size(px(14.)),
                                            MUTED,
                                        ))
                                        .tooltip("删除整段记录（包括所有日期）")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.delete_record(id, window, cx)
                                        })),
                                )
                            })
                            .when(session.finished_at.is_none(), |actions| {
                                actions.child(
                                    crate::view::outlined_button(("open-record-timer", id as u64))
                                        .custom(crate::view::quiet_variant(cx))
                                        .h(px(32.))
                                        .px(px(8.))
                                        .child(control_label("查看计时", INK))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.select_session(id, window, cx)
                                        })),
                                )
                            }),
                    ),
            )
            .when(expanded, |row| {
                row.child(
                    div()
                        .pt(px(16.))
                        .pb(px(4.))
                        .pl(px(36.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            record_detail("标签", session.tag_name.clone())
                                .debug_selector(move || format!("record-detail-tag-{id}")),
                        )
                        .child(record_detail(
                            "分类",
                            crate::settings_view::tag_class_label(session.tag_class).to_owned(),
                        ))
                        .when(session.timing_estimated, |body| {
                            body.child(record_detail("时间", "时段为估算".to_owned()))
                        })
                        .when(
                            session.timing_adjusted && !session.timing_estimated,
                            |body| body.child(record_detail("时间", "按秒取整".to_owned())),
                        )
                        .when(
                            !session.timing_estimated
                                && !session.timing_adjusted
                                && session.paused_at(self.now) > 0,
                            |body| {
                                body.child(
                                    record_detail("暂停", clock(session.paused_at(self.now)))
                                        .debug_selector(move || {
                                            format!("record-detail-pause-{id}")
                                        }),
                                )
                            },
                        )
                        .child(record_detail("开始", record_timestamp(session.started_at)))
                        .child(record_detail(
                            "结束",
                            session.finished_at.map_or_else(
                                || {
                                    if session.is_running() {
                                        "计时中".to_owned()
                                    } else {
                                        "已暂停".to_owned()
                                    }
                                },
                                record_timestamp,
                            ),
                        ))
                        .child(record_detail(
                            "方式",
                            session.target_seconds.map_or_else(
                                || "不限时".to_owned(),
                                |seconds| format!("{}倒计时", short_time(seconds)),
                            ),
                        )),
                )
            })
    }
}
