use chrono::{Datelike, Days, Local, NaiveDate, TimeZone, Timelike};
use force_core::{PlanEntry, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{Icon, IconName, button::ButtonVariants};

use crate::{
    label::single_line,
    model::{ForceView, Page, day_bounds},
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, SURFACE, action_icon, button, choice_button,
        clock, control_icon, control_label, field, field_label, primary_button, section_heading,
        short_time, timer_font,
    },
};

const MONTH_WIDTH: f32 = 208.;
const COLUMN_GAP: f32 = 24.;
const HOUR_HEIGHT: f32 = 60.;
const TIME_GUTTER: f32 = 46.;
const MIN_EVENT_HEIGHT: f32 = 28.;

#[derive(Clone, Copy)]
enum CalendarTarget {
    Plan(i64),
    Record(i64),
    Completion(i64),
}

struct CalendarBlock {
    target: CalendarTarget,
    title: String,
    detail: String,
    help: String,
    start: f32,
    end: f32,
    done: bool,
    moved: bool,
    actual: bool,
    selected: bool,
    lane: usize,
    lane_count: usize,
}

impl ForceView {
    pub(crate) fn agenda_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        if !self.agenda.editing
            && let Some(minute) = self.agenda.calendar_scroll_target.take()
        {
            let scroll = self.agenda.calendar_scroll.clone();
            window.on_next_frame(move |window, _| {
                scroll.set_offset(point(px(0.), px(-f32::from(minute) / 60. * HOUR_HEIGHT)));
                window.refresh();
            });
        }
        if self.agenda.month_overview && !self.agenda.editing {
            return self.agenda_month_overview(window, cx);
        }
        let content_width = f32::from(window.bounds().size.width).min(976.) - 80.;
        let day_width = (content_width - MONTH_WIDTH - COLUMN_GAP).max(300.);
        div()
            .debug_selector(|| "manager-content".into())
            .flex_1()
            .min_h_0()
            .w_full()
            .max_w(px(976.))
            .mx_auto()
            .px(px(40.))
            .pt(px(24.))
            .pb(px(24.))
            .flex()
            .gap(px(COLUMN_GAP))
            .child(self.agenda_month_sidebar(cx))
            .child(
                div()
                    .debug_selector(|| "agenda-content".into())
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(self.agenda_day_header(cx))
                    .when(self.storage_failed, |body| {
                        body.child(self.storage_notice(cx))
                    })
                    .when(!self.storage_failed, |body| {
                        if self.agenda.editing {
                            body.child(self.agenda_editor(cx))
                        } else {
                            body.child(self.agenda_calendar_grid(day_width, cx))
                                .when_some(self.agenda_selected_detail(cx), |body, detail| {
                                    body.child(detail)
                                })
                        }
                    }),
            )
    }

    fn agenda_month_overview(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let month = self.agenda.month;
        let first = month
            .checked_sub_days(Days::new(u64::from(month.weekday().num_days_from_monday())))
            .unwrap_or(month);
        let last = month
            .checked_add_days(Days::new(31))
            .and_then(|day| day.with_day(1))
            .and_then(|day| day.pred_opt())
            .unwrap_or(month);
        let week_count = (month.weekday().num_days_from_monday() + last.day()).div_ceil(7);
        let width = f32::from(window.bounds().size.width).min(976.) - 80.;
        let cell_width = width / 7.;
        let cell_height = (f32::from(window.bounds().size.height) - 200.) / week_count as f32;
        let available_rows = ((cell_height - 28.) / 20.5).floor().max(1.) as usize;
        let today = Local::now().date_naive();
        let mut weeks = Vec::new();
        for week in 0..week_count {
            let mut cells = Vec::new();
            for weekday in 0..7 {
                let Some(date) = first
                    .checked_add_days(Days::new(u64::from(week * 7 + weekday)))
                    .filter(|day| (1..=9999).contains(&day.year()))
                else {
                    cells.push(div().flex_1().min_w_0().h_full());
                    continue;
                };
                let items = self
                    .agenda
                    .month_days
                    .get(&date)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let item_limit = if items.len() > available_rows {
                    available_rows.saturating_sub(1).max(1)
                } else {
                    available_rows
                };
                let invested = items
                    .iter()
                    .filter(|item| !item.planned && !item.moved && !item.rest)
                    .fold(0_i64, |total, item| total.saturating_add(item.seconds));
                let in_month = date.month() == month.month();
                let foreground = if in_month { INK } else { MUTED };
                let mut content = div()
                    .debug_selector(move || format!("agenda-month-cell-content-{date}"))
                    .w(px((cell_width - 18.).max(0.)))
                    // Button's centered content wrapper cannot resolve a 100%
                    // height. Size this inner column explicitly so dates align
                    // at the top regardless of how many entries a day has.
                    .h(px((cell_height - 2.).max(0.)))
                    .py(px(3.))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(1.))
                    .child(
                        div()
                            .debug_selector(move || format!("agenda-month-date-header-{date}"))
                            .w_full()
                            .h(px(20.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(
                                div()
                                    .size(px(20.))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(if date == today {
                                        0xffffff
                                    } else {
                                        foreground
                                    }))
                                    .when(date == today, |label| label.bg(rgb(INK)))
                                    .child(date.day().to_string()),
                            )
                            .when(invested > 0, |header| {
                                header.child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_right()
                                        .text_size(px(11.))
                                        .text_color(rgb(MUTED))
                                        .child(if invested >= 3600 {
                                            format!("{}时{}分", invested / 3600, invested / 60 % 60)
                                        } else if invested >= 60 {
                                            format!("{}分", invested / 60)
                                        } else {
                                            format!("{invested}秒")
                                        }),
                                )
                            }),
                    );
                let mut help = date.format("%Y年%-m月%-d日").to_string();
                if invested > 0 {
                    help.push_str(&format!("\n任务投入 {}", short_time(invested)));
                }
                for (index, item) in items.iter().enumerate() {
                    let detail = if item.moved {
                        "已改期".to_owned()
                    } else if item.planned {
                        if item.completed {
                            "安排已完成".to_owned()
                        } else {
                            "待做安排".to_owned()
                        }
                    } else if item.seconds > 0 {
                        format!(
                            "{}{}{}",
                            if item.rest { "休息 · " } else { "" },
                            short_time(item.seconds),
                            if item.completed { " · 完成" } else { "" }
                        )
                    } else if item.completed {
                        "已完成".to_owned()
                    } else {
                        "已开始".to_owned()
                    };
                    help.push_str(&format!("\n{} · {detail}", item.title));
                    if index < item_limit {
                        content = content.child(
                            div()
                                .debug_selector(move || format!("agenda-month-item-{date}-{index}"))
                                .w_full()
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .text_color(rgb(if item.planned || item.moved {
                                    MUTED
                                } else {
                                    foreground
                                }))
                                .child(
                                    single_line(
                                        format!(
                                            "{}{}",
                                            if item.moved {
                                                "↗ "
                                            } else if item.planned {
                                                if item.completed { "✓ " } else { "待 · " }
                                            } else if item.rest {
                                                "休息 · "
                                            } else if item.completed {
                                                "✓ "
                                            } else {
                                                ""
                                            },
                                            item.title
                                        ),
                                        13.,
                                    )
                                    .debug_selector(move || {
                                        format!("agenda-month-title-{date}-{index}")
                                    })
                                    .w(px((cell_width - 18.).max(0.)))
                                    .font_weight(
                                        if item.planned || item.moved {
                                            FontWeight::NORMAL
                                        } else {
                                            FontWeight::MEDIUM
                                        },
                                    ),
                                ),
                        );
                    }
                }
                if items.len() > item_limit {
                    content = content.child(
                        div()
                            .debug_selector(move || format!("agenda-month-more-{date}"))
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(format!("另 {} 项", items.len() - item_limit)),
                    );
                }
                cells.push(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .border_r_1()
                        .border_b_1()
                        .border_color(rgb(LINE))
                        .child(
                            button(SharedString::from(format!("agenda-month-day-{date}")))
                                .debug_selector(move || format!("agenda-month-day-{date}"))
                                .ghost()
                                .size_full()
                                .p_0()
                                .rounded(px(0.))
                                .overflow_hidden()
                                .tooltip(help)
                                .child(content)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.agenda.month_overview = false;
                                    this.select_agenda_date(date, window, cx);
                                    cx.notify();
                                })),
                        ),
                );
            }
            weeks.push(div().flex_1().min_h_0().w_full().flex().children(cells));
        }
        div()
            .debug_selector(|| "manager-content".into())
            .flex_1()
            .min_h_0()
            .w_full()
            .max_w(px(976.))
            .mx_auto()
            .px(px(40.))
            .py(px(24.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .debug_selector(|| "agenda-overview-header".into())
                    .h(px(40.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(28.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(month.format("%Y 年 %-m 月").to_string()),
                    )
                    .child(
                        button("agenda-overview-previous")
                            .debug_selector(|| "agenda-overview-previous".into())
                            .ghost()
                            .size(px(32.))
                            .icon(control_icon(IconName::ChevronLeft, INK))
                            .tooltip("上个月")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.change_agenda_month(-1, window, cx)
                            })),
                    )
                    .child(
                        button("agenda-overview-next")
                            .debug_selector(|| "agenda-overview-next".into())
                            .ghost()
                            .size(px(32.))
                            .icon(control_icon(IconName::ChevronRight, INK))
                            .tooltip("下个月")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.change_agenda_month(1, window, cx)
                            })),
                    )
                    .child(
                        button("agenda-overview-today")
                            .debug_selector(|| "agenda-overview-today".into())
                            .ghost()
                            .h(px(32.))
                            .child(control_label("今天", INK))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select_agenda_date(Local::now().date_naive(), window, cx);
                                // The selected day may already be today while browsing another month.
                                this.agenda.month = Local::now().date_naive().with_day(1).unwrap();
                                this.refresh_agenda(cx);
                            })),
                    )
                    .child(
                        primary_button("agenda-overview-add")
                            .debug_selector(|| "agenda-overview-add".into())
                            .h(px(36.))
                            .px(px(12.))
                            .unavailable(self.storage_failed)
                            .icon(control_icon(
                                IconName::Plus,
                                if self.storage_failed { MUTED } else { 0xffffff },
                            ))
                            .child(control_label(
                                "添加安排",
                                if self.storage_failed { MUTED } else { 0xffffff },
                            ))
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.agenda.date.month() != this.agenda.month.month()
                                    || this.agenda.date.year() != this.agenda.month.year()
                                {
                                    this.select_agenda_date(this.agenda.month, window, cx);
                                }
                                this.agenda.month_overview = false;
                                this.begin_plan(None, window, cx);
                            })),
                    ),
            )
            .when(self.storage_failed, |body| {
                body.child(self.storage_notice(cx))
            })
            .when(!self.storage_failed, |body| {
                body.child(
                    div()
                        .debug_selector(|| "agenda-overview-grid".into())
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .child(div().h(px(28.)).flex_shrink_0().flex().children(
                            ["周一", "周二", "周三", "周四", "周五", "周六", "周日"].map(|day| {
                                div()
                                    .flex_1()
                                    .pl(px(9.))
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED))
                                    .child(day)
                            }),
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .border_t_1()
                                .border_l_1()
                                .border_color(rgb(LINE))
                                .children(weeks),
                        ),
                )
            })
    }

    fn agenda_month_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let month = self.agenda.month;
        let offset = i64::from(month.weekday().num_days_from_monday());
        let first = month
            .checked_sub_signed(chrono::TimeDelta::days(offset))
            .unwrap_or(month);
        let selected = self.agenda.date;
        let today = Local::now().date_naive();
        let mut weeks = Vec::new();
        for week in 0..6 {
            let mut dates = Vec::new();
            for weekday in 0..7 {
                let Some(date) = first.checked_add_days(Days::new(week * 7 + weekday)) else {
                    continue;
                };
                let is_selected = date == selected;
                let foreground = if is_selected {
                    0xffffff
                } else if date.month() == month.month() {
                    INK
                } else {
                    MUTED
                };
                let marked = self.agenda.planned_days.contains(&date)
                    || self.agenda.actual_days.contains(&date);
                dates.push(
                    div().flex_1().min_w_0().h(px(28.)).child(
                        choice_button(
                            SharedString::from(format!("agenda-date-{date}")),
                            is_selected,
                            INK,
                            cx,
                        )
                        .debug_selector(move || format!("agenda-date-{date}"))
                        .size(px(28.))
                        .p_0()
                        .rounded(px(5.))
                        .when(date == today && !is_selected, |button| {
                            button.border_1().border_color(rgb(INK))
                        })
                        .tooltip(date.format("%Y年%-m月%-d日").to_string())
                        .child(
                            div()
                                .w(px(24.))
                                .h(px(28.))
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(rgb(foreground))
                                        .child(date.day().to_string()),
                                )
                                .child(
                                    div()
                                        .h(px(3.))
                                        .w(px(3.))
                                        .rounded_full()
                                        .when(marked, |dot| dot.bg(rgb(foreground))),
                                ),
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.select_agenda_date(date, window, cx);
                            },
                        )),
                    ),
                );
            }
            weeks.push(div().flex().w_full().children(dates));
        }
        let pending: Vec<_> = self
            .tasks
            .iter()
            .filter(|task| {
                task.completed_at.is_none()
                    && !self
                        .agenda
                        .plans
                        .iter()
                        .any(|plan| plan.task_id == task.id && plan.moved_to.is_none())
            })
            .collect();
        div()
            .debug_selector(|| "agenda-month".into())
            .w(px(MONTH_WIDTH))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(34.))
                    .child(
                        button("agenda-month-previous")
                            .debug_selector(|| "agenda-month-previous".into())
                            .ghost()
                            .size(px(30.))
                            .icon(control_icon(IconName::ChevronLeft, INK))
                            .tooltip("上个月")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.change_agenda_month(-1, window, cx)
                            })),
                    )
                    .child(section_heading(&month.format("%Y 年 %-m 月").to_string()))
                    .child(
                        button("agenda-month-next")
                            .debug_selector(|| "agenda-month-next".into())
                            .ghost()
                            .size(px(30.))
                            .icon(control_icon(IconName::ChevronRight, INK))
                            .tooltip("下个月")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.change_agenda_month(1, window, cx)
                            })),
                    ),
            )
            .child(div().flex().w_full().mt(px(12.)).mb(px(5.)).children(
                ["一", "二", "三", "四", "五", "六", "日"].map(|day| {
                    div()
                        .flex_1()
                        .text_center()
                        .text_size(px(12.))
                        .text_color(rgb(MUTED))
                        .child(day)
                }),
            ))
            .child(
                div()
                    .debug_selector(|| "agenda-month-grid".into())
                    .flex()
                    .flex_col()
                    .children(weeks),
            )
            .child(
                button("agenda-today")
                    .debug_selector(|| "agenda-today".into())
                    .ghost()
                    .w_full()
                    .h(px(32.))
                    .mt(px(8.))
                    .child(control_label("回到今天", MUTED))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.select_agenda_date(Local::now().date_naive(), window, cx)
                    })),
            )
            .child(
                div()
                    .id("agenda-pending-scroll")
                    .debug_selector(|| "agenda-pending-scroll".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.agenda.inspector_scroll)
                    .mt(px(20.))
                    .child(self.agenda_day_brief(cx))
                    .when(!pending.is_empty(), |list| {
                        list.child(
                            div()
                                .mt(px(20.))
                                .mb(px(8.))
                                .text_size(px(13.))
                                .text_color(rgb(MUTED))
                                .child(format!("待安排  {}", pending.len())),
                        )
                    })
                    .children(pending.iter().map(|task| {
                        let id = task.id;
                        button(("agenda-pending", id as u64))
                            .debug_selector(move || format!("agenda-pending-{id}"))
                            .ghost()
                            .w_full()
                            .h(px(40.))
                            .px(px(6.))
                            .rounded(px(0.))
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .tooltip(format!("安排：{}", task.title))
                            .child(
                                div()
                                    .w(px(MONTH_WIDTH - 14.))
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        single_line(task.title.clone(), 14.)
                                            .w(px(MONTH_WIDTH - 38.)),
                                    )
                                    .child(
                                        Icon::new(IconName::Plus)
                                            .size(px(14.))
                                            .text_color(rgb(MUTED)),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.begin_plan(Some(id), window, cx)
                            }))
                    })),
            )
    }

    fn agenda_day_brief(&self, cx: &mut Context<Self>) -> Div {
        let running = self.strip_session();
        let minute = Local::now().hour() as u16 * 60 + Local::now().minute() as u16;
        let today = self.agenda.date == Local::now().date_naive();
        let next = self
            .agenda
            .plans
            .iter()
            .filter(|plan| plan.moved_to.is_none() && !self.agenda_plan_is_done(plan))
            .filter(|plan| !today || plan.end_minute > minute)
            .filter(|plan| {
                running.is_none_or(|session| {
                    !today || !session.is_running() || session.task_id != Some(plan.task_id)
                })
            })
            .min_by_key(|plan| plan.start_minute);
        let planned = self
            .agenda
            .plans
            .iter()
            .filter(|plan| plan.moved_to.is_none())
            .count();
        let done = self
            .agenda
            .plans
            .iter()
            .filter(|plan| plan.moved_to.is_none() && self.agenda_plan_is_done(plan))
            .count();
        div()
            .debug_selector(|| "agenda-day-brief".into())
            .flex()
            .flex_col()
            .gap(px(12.))
            .when_some(running, |brief, session| {
                let id = session.id;
                brief.child(
                    div()
                        .debug_selector(|| "agenda-live-timer".into())
                        .p(px(12.))
                        .rounded(px(8.))
                        .bg(rgb(SURFACE))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(
                            if session.is_running() {
                                "正在进行"
                            } else {
                                "已暂停"
                            },
                        ))
                        .child(
                            single_line(session.title.clone(), 16.)
                                .w(px(MONTH_WIDTH - 24.))
                                .font_weight(FontWeight::MEDIUM),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .font(timer_font(cx))
                                        .text_size(px(28.))
                                        .child(clock(session.elapsed_at(self.now))),
                                )
                                .child(
                                    button("agenda-live-pause")
                                        .debug_selector(|| "agenda-live-pause".into())
                                        .size(px(30.))
                                        .icon(control_icon(
                                            action_icon(if session.is_running() {
                                                "pause"
                                            } else {
                                                "play"
                                            })
                                            .size(px(14.)),
                                            INK,
                                        ))
                                        .tooltip(if session.is_running() {
                                            "暂停计时"
                                        } else {
                                            "继续计时"
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.toggle_pause(id, cx)
                                        })),
                                ),
                        )
                        .child(
                            button("agenda-live-detail")
                                .ghost()
                                .h(px(28.))
                                .w_full()
                                .child(control_label("查看计时", MUTED))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_session(id, window, cx)
                                })),
                        ),
                )
            })
            .when_some(next, |brief, plan| {
                let id = plan.id;
                brief.child(
                    button("agenda-next-plan")
                        .debug_selector(|| "agenda-next-plan".into())
                        .ghost()
                        .w_full()
                        .h(px(70.))
                        .px(px(2.))
                        .justify_start()
                        .child(
                            div()
                                .w(px(MONTH_WIDTH - 8.))
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(
                                    format!(
                                        "{}  {}",
                                        if today && plan.start_minute <= minute {
                                            "待完成"
                                        } else {
                                            "接下来"
                                        },
                                        minute_label(plan.start_minute)
                                    ),
                                ))
                                .child(
                                    single_line(plan.title.clone(), 16.)
                                        .w(px(MONTH_WIDTH - 8.))
                                        .font_weight(FontWeight::MEDIUM),
                                ),
                        )
                        .on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.select_plan(id, window, cx)
                            }),
                        ),
                )
            })
            .child(
                div()
                    .pt(px(12.))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(if planned == 0 {
                                "还没有安排".to_owned()
                            } else {
                                format!("已完成 {done} / {planned}")
                            })
                            .child(format!(
                                "计时 {}",
                                short_time(self.agenda.stats.focus_seconds)
                            )),
                    )
                    .when(planned > 0, |brief| {
                        brief.child(
                            div().h(px(3.)).w_full().rounded_full().bg(rgb(LINE)).child(
                                div()
                                    .h_full()
                                    .w(relative(done as f32 / planned as f32))
                                    .rounded_full()
                                    .bg(rgb(INK)),
                            ),
                        )
                    }),
            )
    }

    fn agenda_day_header(&self, cx: &mut Context<Self>) -> Div {
        let date = self.agenda.date;
        let previous = date.pred_opt();
        let next = date.succ_opt();
        let today = date == Local::now().date_naive();
        div()
            .debug_selector(|| "agenda-day-header".into())
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        button("agenda-back-to-month")
                            .debug_selector(|| "agenda-back-to-month".into())
                            .ghost()
                            .h(px(32.))
                            .icon(control_icon(IconName::ChevronLeft, INK))
                            .child(control_label("月历", INK))
                            .when(!self.agenda.editing, |button| {
                                button.on_click(cx.listener(|this, _, _, cx| {
                                    this.agenda.month_overview = true;
                                    cx.notify();
                                }))
                            })
                            .unavailable(self.agenda.editing),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(28.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if today {
                                "今天".to_owned()
                            } else {
                                date.format("%-m 月 %-d 日").to_string()
                            }),
                    )
                    .child(
                        button("agenda-day-previous")
                            .ghost()
                            .size(px(30.))
                            .icon(control_icon(IconName::ChevronLeft, MUTED))
                            .tooltip("前一天")
                            .unavailable(previous.is_none())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(date) = previous {
                                    this.select_agenda_date(date, window, cx);
                                }
                            })),
                    )
                    .child(
                        button("agenda-day-next")
                            .ghost()
                            .size(px(30.))
                            .icon(control_icon(IconName::ChevronRight, MUTED))
                            .tooltip("后一天")
                            .unavailable(next.is_none())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(date) = next {
                                    this.select_agenda_date(date, window, cx);
                                }
                            })),
                    )
                    .child(
                        primary_button("agenda-add")
                            .debug_selector(|| "agenda-add".into())
                            .h(px(36.))
                            .px(px(12.))
                            .unavailable(self.storage_failed)
                            .icon(control_icon(
                                IconName::Plus,
                                if self.storage_failed { MUTED } else { 0xffffff },
                            ))
                            .child(control_label(
                                "添加安排",
                                if self.storage_failed { MUTED } else { 0xffffff },
                            ))
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.begin_plan(None, window, cx)
                                }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .gap(px(12.))
                    .text_size(px(13.))
                    .text_color(rgb(MUTED))
                    .child(format!(
                        "{}  {}",
                        date.format("%Y 年 %-m 月 %-d 日"),
                        [
                            "星期一",
                            "星期二",
                            "星期三",
                            "星期四",
                            "星期五",
                            "星期六",
                            "星期日"
                        ][date.weekday().num_days_from_monday() as usize]
                    ))
                    .child(format!(
                        "{} 项安排 · 计时 {}",
                        self.agenda
                            .plans
                            .iter()
                            .filter(|plan| plan.moved_to.is_none())
                            .count(),
                        short_time(self.agenda.stats.focus_seconds)
                    )),
            )
    }

    fn agenda_editor(&self, cx: &mut Context<Self>) -> Div {
        let linked_task = self
            .agenda
            .draft_task
            .and_then(|id| self.tasks.iter().find(|task| task.id == id));
        div()
            .debug_selector(|| "agenda-editor".into())
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(section_heading(if self.agenda.edit_id.is_some() {
                "编辑安排"
            } else {
                "添加安排"
            }))
            .child(
                div()
                    .id("agenda-editor-scroll")
                    .debug_selector(|| "agenda-editor-scroll".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.agenda.editor_scroll)
                    .pr(px(4.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        agenda_field("要做什么").child(
                            div().debug_selector(|| "agenda-title-input".into()).child(
                                field(&self.agenda.title_input)
                                    .disabled(self.agenda.draft_task.is_some()),
                            ),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(
                                agenda_field("日期").flex_1().min_w(px(120.)).child(
                                    div()
                                        .debug_selector(|| "agenda-date-input".into())
                                        .child(field(&self.agenda.date_input)),
                                ),
                            )
                            .child(
                                agenda_field("开始").w(px(88.)).flex_shrink_0().child(
                                    div()
                                        .debug_selector(|| "agenda-start-input".into())
                                        .child(field(&self.agenda.start_input)),
                                ),
                            )
                            .child(
                                agenda_field("结束").w(px(88.)).flex_shrink_0().child(
                                    div()
                                        .debug_selector(|| "agenda-end-input".into())
                                        .child(field(&self.agenda.end_input)),
                                ),
                            ),
                    )
                    .child(agenda_field("标签").child(if let Some(task) = linked_task {
                        div()
                            .h(px(40.))
                            .flex()
                            .items_center()
                            .text_size(px(14.))
                            .child(task.tag_name.clone())
                            .into_any_element()
                    } else {
                        div()
                            .w_full()
                            .max_w(px(240.))
                            .h(px(40.))
                            .child(self.agenda.tag_picker.clone())
                            .into_any_element()
                    }))
                    .child(
                        agenda_field("备注").child(
                            div()
                                .debug_selector(|| "agenda-note-input".into())
                                .child(field(&self.agenda.note_input)),
                        ),
                    ),
            )
            .when_some(self.agenda.error.as_ref(), |body, message| {
                body.child(
                    div()
                        .debug_selector(|| "agenda-error".into())
                        .text_size(px(14.))
                        .text_color(rgb(DESTRUCTIVE))
                        .whitespace_normal()
                        .child(message.clone()),
                )
            })
            .child(
                div()
                    .debug_selector(|| "agenda-editor-actions".into())
                    .flex_shrink_0()
                    .flex()
                    .gap(px(10.))
                    .child(
                        primary_button("agenda-save")
                            .debug_selector(|| "agenda-save".into())
                            .h(px(40.))
                            .w(px(112.))
                            .unavailable(self.storage_failed)
                            .child(control_label(
                                "保存安排",
                                if self.storage_failed { MUTED } else { 0xffffff },
                            ))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.save_plan(window, cx)),
                            ),
                    )
                    .child(
                        button("agenda-cancel")
                            .debug_selector(|| "agenda-cancel".into())
                            .h(px(40.))
                            .child(control_label("取消", MUTED))
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.cancel_plan_edit(window, cx)
                                }),
                            ),
                    ),
            )
    }

    fn agenda_calendar_grid(&self, day_width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let mut blocks = self.agenda_blocks();
        blocks.sort_by(|a, b| {
            a.start
                .total_cmp(&b.start)
                .then_with(|| a.end.total_cmp(&b.end))
        });
        // Width belongs to a connected overlap group. An isolated later
        // event must regain the full day width after an earlier overlap ends.
        let mut group_start = 0;
        let mut maximum_lanes = 1;
        while group_start < blocks.len() {
            let mut group_end = group_start;
            let mut furthest_end = f32::NEG_INFINITY;
            let mut lane_ends: Vec<f32> = Vec::new();
            while let Some(block) = blocks.get_mut(group_end) {
                if group_end > group_start && block.start >= furthest_end {
                    break;
                }
                let minimum_end = block.start + MIN_EVENT_HEIGHT / HOUR_HEIGHT * 60.;
                let visible_end = block.end.max(minimum_end);
                let lane = lane_ends
                    .iter()
                    .position(|end| *end <= block.start)
                    .unwrap_or(lane_ends.len());
                if lane == lane_ends.len() {
                    lane_ends.push(visible_end);
                } else {
                    lane_ends[lane] = visible_end;
                }
                block.lane = lane;
                furthest_end = furthest_end.max(visible_end);
                group_end += 1;
            }
            let lane_count = lane_ends.len();
            for block in &mut blocks[group_start..group_end] {
                block.lane_count = lane_count;
            }
            maximum_lanes = maximum_lanes.max(lane_count);
            group_start = group_end;
        }
        let canvas_width = day_width.max(TIME_GUTTER + maximum_lanes as f32 * 140.);
        let mut hours = Vec::new();
        for hour in 0..24 {
            hours.push(
                div()
                    .id(("agenda-hour", hour as usize))
                    .debug_selector(move || format!("agenda-hour-{hour}"))
                    .cursor_pointer()
                    .hover(|slot| slot.bg(rgb(SURFACE)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_plan_at(hour as u16, window, cx);
                    }))
                    .absolute()
                    .top(px(hour as f32 * HOUR_HEIGHT))
                    .left_0()
                    .right_0()
                    .h(px(HOUR_HEIGHT))
                    .child(
                        div()
                            .absolute()
                            .top(px(3.))
                            .left_0()
                            .w(px(TIME_GUTTER - 8.))
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .text_align(TextAlign::Right)
                            .child(format!("{hour:02}:00")),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(10.))
                            .left(px(TIME_GUTTER))
                            .right_0()
                            .h(px(1.))
                            .bg(rgb(LINE)),
                    ),
            );
        }
        let date = self.agenda.date;
        let now = Local.timestamp_opt(self.now, 0).single();
        let current_minute = now.filter(|time| time.date_naive() == date).map(|time| {
            time.hour() as f32 * 60. + time.minute() as f32 + time.second() as f32 / 60.
        });
        div()
            .id("agenda-day-scroll")
            .debug_selector(|| "agenda-day-grid".into())
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .overflow_x_scroll()
            .track_scroll(&self.agenda.calendar_scroll)
            .child(
                div()
                    .debug_selector(|| "agenda-time-canvas".into())
                    .relative()
                    .w(px(canvas_width))
                    .h(px(24. * HOUR_HEIGHT + MIN_EVENT_HEIGHT + 12.))
                    .children(hours)
                    .when(blocks.is_empty(), |grid| {
                        grid.child(
                            div()
                                .absolute()
                                .top(px(current_minute
                                    .map_or(8. * HOUR_HEIGHT + 18., |minute| {
                                        ((minute - 30.).max(0.) / 60.) * HOUR_HEIGHT + 18.
                                    })))
                                .left(px(TIME_GUTTER + 16.))
                                .text_size(px(14.))
                                .text_color(rgb(MUTED))
                                .child("点击时间，安排要做的事"),
                        )
                    })
                    .when_some(current_minute, |grid, minute| {
                        grid.child(
                            div()
                                .absolute()
                                .top(px(minute / 60. * HOUR_HEIGHT + 10.))
                                .left_0()
                                .right_0()
                                .h(px(1.))
                                .bg(rgb(INK))
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .top(px(-8.))
                                        .w(px(TIME_GUTTER))
                                        .py(px(2.))
                                        .rounded(px(3.))
                                        .text_center()
                                        .text_size(px(10.))
                                        .bg(rgb(INK))
                                        .text_color(rgb(0xffffff))
                                        .child(minute_label(minute as u16)),
                                ),
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .left(px(TIME_GUTTER))
                            .right_0()
                            .top(px(10.))
                            .h(px(24. * HOUR_HEIGHT + MIN_EVENT_HEIGHT))
                            .children(blocks.iter().enumerate().map(|(index, block)| {
                                self.agenda_calendar_block(block, index, canvas_width, cx)
                            })),
                    ),
            )
    }

    fn agenda_calendar_block(
        &self,
        block: &CalendarBlock,
        index: usize,
        canvas_width: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let lane_count = block.lane_count;
        let text_width = ((canvas_width - TIME_GUTTER) / lane_count as f32 - 20.).max(100.);
        let top = block.start / 60. * HOUR_HEIGHT;
        let height = ((block.end - block.start) / 60. * HOUR_HEIGHT).max(MIN_EVENT_HEIGHT);
        let target = block.target;
        let foreground = INK;
        let secondary = MUTED;
        let active = self.active.iter().find(|session| match target {
            CalendarTarget::Plan(id) => self
                .agenda
                .plans
                .iter()
                .any(|plan| plan.id == id && Some(plan.task_id) == session.task_id),
            CalendarTarget::Record(id) => session.id == id,
            CalendarTarget::Completion(_) => false,
        });
        let status = if block.moved {
            "已顺延"
        } else if active.is_some_and(|session| session.is_running()) {
            "计时中"
        } else if active.is_some_and(|session| session.is_paused()) {
            "已暂停"
        } else if block.done {
            "已完成"
        } else if block.actual {
            "已记录"
        } else {
            "计划"
        };
        div()
            // Event blocks overlap the empty-hour hitboxes. Prevent selecting
            // an event from also opening a new arrangement behind it.
            .block_mouse_except_scroll()
            .absolute()
            .left(relative(block.lane as f32 / lane_count as f32))
            .top(px(top))
            .w(relative(1. / lane_count as f32))
            .h(px(height))
            .px(px(3.))
            .child(
                button(("agenda-event", index))
                    .ghost()
                    .debug_selector(move || match target {
                        CalendarTarget::Plan(id) => format!("agenda-plan-{id}"),
                        CalendarTarget::Record(id) => format!("agenda-record-{id}"),
                        CalendarTarget::Completion(id) => format!("agenda-completion-{id}"),
                    })
                    .w_full()
                    .h_full()
                    .px(px(7.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(if block.selected { INK } else { LINE }))
                    .border_l(px(if block.selected || block.actual {
                        3.
                    } else {
                        1.
                    }))
                    .bg(rgb(if block.actual || block.done {
                        SURFACE
                    } else {
                        0xffffff
                    }))
                    .justify_start()
                    .tooltip(block.help.clone())
                    .child(
                        div()
                            .w(px(text_width))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                single_line(block.title.clone(), 13.)
                                    .w(px(text_width))
                                    .text_color(rgb(foreground))
                                    .font_weight(FontWeight::MEDIUM),
                            )
                            .when(height >= 46., |column| {
                                column.child(
                                    single_line(format!("{status} · {}", block.detail), 12.)
                                        .w(px(text_width))
                                        .text_color(rgb(secondary)),
                                )
                            }),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| match target {
                        CalendarTarget::Plan(id) => this.select_plan(id, window, cx),
                        CalendarTarget::Record(id) => this.select_agenda_record(id, window, cx),
                        CalendarTarget::Completion(id) => {
                            this.select_agenda_completion(id, window, cx)
                        }
                    })),
            )
    }

    fn agenda_blocks(&self) -> Vec<CalendarBlock> {
        let mut blocks = Vec::new();
        for plan in &self.agenda.plans {
            let actuals = self.agenda_plan_actuals(plan.id);
            let actual_seconds: i64 = actuals
                .iter()
                .map(|session| {
                    let (start, end) = day_bounds(self.agenda.date);
                    session.elapsed_between(start, end, self.now)
                })
                .sum();
            let done = self.agenda_plan_is_done(plan);
            let detail = if actual_seconds > 0 {
                format!("今日累计 {}", short_time(actual_seconds))
            } else {
                format!(
                    "{}–{}",
                    minute_label(plan.start_minute),
                    minute_label(plan.end_minute)
                )
            };
            blocks.push(CalendarBlock {
                target: CalendarTarget::Plan(plan.id),
                title: plan.title.clone(),
                detail: detail.clone(),
                help: format!(
                    "{}\n计划 {}–{}{}{}",
                    plan.title,
                    minute_label(plan.start_minute),
                    minute_label(plan.end_minute),
                    if plan.moved_to.is_some() {
                        " · 已顺延"
                    } else {
                        ""
                    },
                    if actual_seconds > 0 {
                        format!("\n任务今日累计 {}", short_time(actual_seconds))
                    } else {
                        String::new()
                    }
                ),
                start: f32::from(plan.start_minute),
                end: f32::from(plan.end_minute),
                done,
                moved: plan.moved_to.is_some(),
                actual: false,
                selected: self.agenda.selected_plan == Some(plan.id),
                lane: 0,
                lane_count: 1,
            });
        }
        let (day_start, day_end) = day_bounds(self.agenda.date);
        // Actual intervals keep their real clock position even when their
        // task also has an arrangement at a different time.
        for session in &self.agenda.records {
            for interval in &session.intervals {
                let start = interval.start.max(day_start);
                let end = interval.end.unwrap_or(self.now).min(day_end).min(self.now);
                if end <= start {
                    continue;
                }
                let left = clock_minute(self.agenda.date, start);
                let right = clock_minute(self.agenda.date, end)
                    .max(left + 0.1)
                    .min(1440.);
                blocks.push(CalendarBlock {
                    target: CalendarTarget::Record(session.id),
                    title: if session.tag_kind == TagKind::Rest {
                        format!("休息 · {}", session.title)
                    } else {
                        session.title.clone()
                    },
                    detail: format!(
                        "{} {}",
                        if session.tag_kind == TagKind::Rest {
                            "休息"
                        } else {
                            "计时"
                        },
                        short_time(end - start)
                    ),
                    help: format!(
                        "{}\n{}–{} · {}{}",
                        session.title,
                        timestamp_clock(start),
                        timestamp_clock(end),
                        if session.tag_kind == TagKind::Rest {
                            "休息 "
                        } else {
                            "计时 "
                        },
                        short_time(end - start)
                    ),
                    start: left,
                    end: right,
                    done: session.finished_at.is_some(),
                    moved: false,
                    actual: true,
                    selected: self.agenda.selected_record == Some(session.id),
                    lane: 0,
                    lane_count: 1,
                });
            }
            if session.elapsed_between(day_start, day_end, self.now) == 0 {
                let stamp = session.finished_at.unwrap_or(session.started_at);
                let minute = clock_minute(self.agenda.date, stamp).min(1439.);
                blocks.push(CalendarBlock {
                    target: CalendarTarget::Record(session.id),
                    title: session.title.clone(),
                    detail: "尚未计时".to_owned(),
                    help: format!("{} · {} · 尚未计时", session.title, timestamp_clock(stamp)),
                    start: minute,
                    end: minute + 1.,
                    done: session.finished_at.is_some(),
                    moved: false,
                    actual: true,
                    selected: self.agenda.selected_record == Some(session.id),
                    lane: 0,
                    lane_count: 1,
                });
            }
        }
        for task in self.agenda_unplanned_completions() {
            let minute =
                clock_minute(self.agenda.date, task.completed_at.unwrap_or(self.now)).min(1439.);
            blocks.push(CalendarBlock {
                target: CalendarTarget::Completion(task.id),
                title: task.title.clone(),
                detail: "已完成，未计时".to_owned(),
                help: format!("{} · 已完成，未计时", task.title),
                start: minute,
                end: minute + 1.,
                done: true,
                moved: false,
                actual: true,
                selected: self.agenda.selected_completion == Some(task.id),
                lane: 0,
                lane_count: 1,
            });
        }
        blocks
    }

    fn agenda_selected_detail(&self, cx: &mut Context<Self>) -> Option<Div> {
        if let Some(plan) = self
            .agenda
            .plans
            .iter()
            .find(|plan| Some(plan.id) == self.agenda.selected_plan)
        {
            return Some(self.agenda_plan_detail(plan, cx));
        }
        if let Some(session) = self
            .agenda
            .records
            .iter()
            .find(|record| Some(record.id) == self.agenda.selected_record)
        {
            let id = session.id;
            let date = self.agenda.date;
            let (start, end) = day_bounds(date);
            return Some(
                detail_section()
                    .debug_selector(|| "agenda-record-detail".into())
                    .child(single_line(session.title.clone(), 17.))
                    .child(field_label(&format!(
                        "{} · 当天{} {}",
                        if session.is_running() {
                            "计时中"
                        } else if session.is_paused() {
                            "已暂停"
                        } else {
                            "已记录"
                        },
                        if session.tag_kind == TagKind::Rest {
                            "休息"
                        } else {
                            "计时"
                        },
                        short_time(session.elapsed_between(start, end, self.now))
                    )))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .when(session.finished_at.is_none(), |row| {
                                row.child(
                                    primary_button("agenda-record-timer")
                                        .h(px(36.))
                                        .child(control_label("返回计时", 0xffffff))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.select_session(id, window, cx)
                                        })),
                                )
                            })
                            .child(
                                button("agenda-record-open")
                                    .h(px(36.))
                                    .child(control_label("查看记录", INK))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select_record_date(date, cx);
                                        this.navigate(Page::Records, window, cx);
                                        this.record_details = Some(id);
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        self.agenda
            .completed_tasks
            .iter()
            .find(|task| Some(task.id) == self.agenda.selected_completion)
            .map(|task| {
                detail_section()
                    .debug_selector(|| "agenda-completion-detail".into())
                    .child(single_line(task.title.clone(), 17.))
                    .child(field_label("已完成，未计时"))
            })
    }

    fn agenda_plan_detail(&self, plan: &PlanEntry, cx: &mut Context<Self>) -> Div {
        let id = plan.id;
        let done = self.agenda_plan_is_done(plan);
        let task_completed = self
            .agenda
            .completed_tasks
            .iter()
            .any(|task| task.id == plan.task_id);
        let moved = plan.moved_to.is_some();
        let task_open = self
            .tasks
            .iter()
            .any(|task| task.id == plan.task_id && task.completed_at.is_none());
        let can_start =
            !done && !moved && task_open && self.agenda.date == Local::now().date_naive();
        let actuals = self.agenda_plan_actuals(id);
        let (start, end) = day_bounds(self.agenda.date);
        let seconds: i64 = actuals
            .iter()
            .map(|record| record.elapsed_between(start, end, self.now))
            .sum();
        detail_section()
            .debug_selector(|| "agenda-plan-detail".into())
            .child(single_line(plan.title.clone(), 17.))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(rgb(MUTED))
                    .whitespace_normal()
                    .child(format!(
                        "计划 {}–{}{}{}",
                        minute_label(plan.start_minute),
                        minute_label(plan.end_minute),
                        if moved {
                            " · 已移到其他日期"
                        } else {
                            ""
                        },
                        if seconds > 0 {
                            format!(" · 任务今日累计 {}", short_time(seconds))
                        } else {
                            String::new()
                        }
                    )),
            )
            .when(!plan.note.is_empty(), |detail| {
                detail.child(
                    div()
                        .id("agenda-detail-note-scroll")
                        .max_h(px(56.))
                        .overflow_y_scroll()
                        .text_size(px(14.))
                        .whitespace_normal()
                        .child(plan.note.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .when(!moved, |row| {
                        row.child(
                            primary_button("agenda-plan-start")
                                .debug_selector(|| "agenda-plan-start".into())
                                .h(px(36.))
                                .unavailable(!can_start || self.storage_failed)
                                .child(control_label(
                                    "开始计时",
                                    if can_start && !self.storage_failed {
                                        0xffffff
                                    } else {
                                        MUTED
                                    },
                                ))
                                .tooltip(if self.agenda.date != Local::now().date_naive() {
                                    "只能开始今天的安排"
                                } else if done {
                                    "这次安排已完成"
                                } else if !task_open {
                                    "对应任务已完成，请添加新的安排"
                                } else {
                                    "开始此任务计时，已有暂停轮次会继续"
                                })
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        if event.click_count() == 1 {
                                            this.start_plan_timer(id, window, cx)
                                        }
                                    },
                                )),
                        )
                        .child(
                            button("agenda-plan-done")
                                .debug_selector(|| "agenda-plan-done".into())
                                .h(px(36.))
                                .unavailable(self.storage_failed || task_completed)
                                .child(control_label(
                                    if task_completed {
                                        "任务已完成"
                                    } else if done {
                                        "撤销完成"
                                    } else {
                                        "标记完成"
                                    },
                                    if task_completed { MUTED } else { INK },
                                ))
                                .tooltip(if task_completed {
                                    "对应任务已完成，实际记录会保留"
                                } else {
                                    "只更改这次安排的勾选状态，计时和任务记录保留"
                                })
                                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                    if event.click_count() == 1 {
                                        this.toggle_plan_done(id, cx)
                                    }
                                })),
                        )
                        .child(
                            button("agenda-plan-edit")
                                .debug_selector(|| "agenda-plan-edit".into())
                                .ghost()
                                .h(px(36.))
                                .unavailable(self.storage_failed)
                                .child(control_label("编辑", MUTED))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.edit_plan(id, window, cx)
                                })),
                        )
                    })
                    .child(
                        button("agenda-plan-delete")
                            .debug_selector(|| "agenda-plan-delete".into())
                            .ghost()
                            .size(px(36.))
                            .unavailable(self.storage_failed)
                            .icon(control_icon(
                                action_icon("trash").size(px(14.)),
                                DESTRUCTIVE,
                            ))
                            .tooltip("删除安排，任务与实际记录保留")
                            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                if event.click_count() == 1 {
                                    this.delete_plan(id, window, cx)
                                }
                            })),
                    )
                    .when(!done && !moved, |row| {
                        row.child(
                            button("agenda-plan-tomorrow")
                                .debug_selector(|| "agenda-plan-tomorrow".into())
                                .ghost()
                                .h(px(36.))
                                .unavailable(self.storage_failed)
                                .child(control_label("移到明天", MUTED))
                                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                    if event.click_count() == 1 {
                                        this.move_plan_tomorrow(id, cx)
                                    }
                                })),
                        )
                    }),
            )
    }
}

fn agenda_field(label: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(field_label(label))
}

fn detail_section() -> Div {
    div()
        .flex_shrink_0()
        .border_t_1()
        .border_color(rgb(LINE))
        .pt(px(14.))
        .flex()
        .flex_col()
        .gap(px(8.))
}

fn minute_label(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

fn timestamp_clock(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map_or_else(String::new, |time| time.format("%H:%M").to_string())
}

fn clock_minute(date: NaiveDate, timestamp: i64) -> f32 {
    let (start, end) = day_bounds(date);
    if timestamp <= start {
        return 0.;
    }
    if timestamp >= end {
        return 1440.;
    }
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map_or(0., |time| {
            time.hour() as f32 * 60. + time.minute() as f32 + time.second() as f32 / 60.
        })
}
