use chrono::{Datelike, Days, Local, NaiveDate, TimeZone, Timelike};
use force_core::{PlanEntry, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{
    Icon, IconName,
    button::{ButtonCustomVariant, ButtonVariants},
    tooltip::Tooltip,
};

use crate::{
    agenda_model::AgendaItemStatus,
    label::single_line,
    model::{ForceView, Page, day_bounds},
    settings_view::{tag_color_rgb, tag_text_color},
    view::{
        CHROME, ControlState, DESTRUCTIVE, HOVER, INK, LINE, MUTED, PAPER, PRESSED, SURFACE,
        action_icon, button, choice_button, clock, control_icon, control_label, field_label,
        primary_button, section_heading, short_time, timer_font,
    },
};

const MONTH_WIDTH: f32 = 208.;
const COLUMN_GAP: f32 = 24.;
const HOUR_HEIGHT: f32 = 60.;
const HOUR_LABEL_TOP: f32 = 3.;
const HOUR_LABEL_HEIGHT: f32 = 16.;
const CURRENT_TIME_LINE_TOP: f32 = 10.;
const CURRENT_TIME_LABEL_OFFSET: f32 = -8.;
const CURRENT_TIME_LABEL_HEIGHT: f32 = 16.;
const TIME_GUTTER: f32 = 46.;
const MIN_EVENT_HEIGHT: f32 = 28.;

fn hour_label_overlaps_current_time(hour: u32, current_minute: Option<f32>) -> bool {
    let Some(current_minute) = current_minute else {
        return false;
    };
    let hour_top = hour as f32 * HOUR_HEIGHT + HOUR_LABEL_TOP;
    let current_top =
        current_minute / 60. * HOUR_HEIGHT + CURRENT_TIME_LINE_TOP + CURRENT_TIME_LABEL_OFFSET;
    current_top < hour_top + HOUR_LABEL_HEIGHT && current_top + CURRENT_TIME_LABEL_HEIGHT > hour_top
}

#[derive(Clone, Copy)]
enum CalendarTarget {
    Plan(i64),
    Record(i64),
    Completion(i64),
    Moment(i64),
}

struct CalendarBlock {
    target: CalendarTarget,
    title: String,
    detail: String,
    help: String,
    start: f32,
    end: f32,
    status: AgendaItemStatus,
    color: Option<String>,
    actual: bool,
    selected: bool,
    lane: usize,
    lane_count: usize,
}

impl CalendarBlock {
    // A point marker is centered at its timestamp. Reserve only its visible
    // hit area for collision layout; this never changes stored time or totals.
    fn visual_start(&self) -> f32 {
        if matches!(self.target, CalendarTarget::Moment(_)) {
            self.start - MIN_EVENT_HEIGHT / HOUR_HEIGHT * 30.
        } else {
            self.start
        }
    }

    fn visual_end(&self) -> f32 {
        if matches!(self.target, CalendarTarget::Moment(_)) {
            self.start + MIN_EVENT_HEIGHT / HOUR_HEIGHT * 30.
        } else {
            self.end
                .max(self.start + MIN_EVENT_HEIGHT / HOUR_HEIGHT * 60.)
        }
    }
}

impl ForceView {
    pub(crate) fn agenda_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        if !self.agenda.month_overview
            && !self.agenda.editor_visible
            && let Some(minute) = self.agenda.calendar_scroll_target.take()
        {
            let scroll = self.agenda.calendar_scroll.clone();
            window.on_next_frame(move |window, _| {
                scroll.set_offset(point(px(0.), px(-f32::from(minute) / 60. * HOUR_HEIGHT)));
                window.refresh();
            });
        }
        if self.agenda.month_overview {
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
                        body.child(self.agenda_calendar_grid(day_width, cx))
                            .when_some(self.agenda_selected_detail(cx), |body, detail| {
                                body.child(detail)
                            })
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
        let available_rows = ((cell_height - 28.) / 21.).floor().max(1.) as usize;
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
                for (index, item) in items.iter().enumerate() {
                    if index < item_limit {
                        let color = self.display_color(
                            item.tag_id,
                            item.tag_class,
                            item.tag_color.as_deref(),
                        );
                        let title_color = color.and_then(tag_text_color).unwrap_or(foreground);
                        let indicator_color = color.and_then(tag_color_rgb).unwrap_or(LINE);
                        let text_width = (cell_width - 18. - 3. - 8. - 24.).max(0.);
                        let help = format!(
                            "{}\n{}{}",
                            item.title,
                            item.status.description(item.planned),
                            if item.rest { " · 休息" } else { "" }
                        );
                        content = content.child(
                            div()
                                .id(SharedString::from(format!(
                                    "agenda-month-item-{date}-{index}"
                                )))
                                .debug_selector(move || format!("agenda-month-item-{date}-{index}"))
                                .w_full()
                                .h(px(20.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .when(!self.agenda.popup_open, |row| {
                                    row.tooltip(move |window, cx| {
                                        Tooltip::new(help.clone()).build(window, cx)
                                    })
                                })
                                .child(
                                    div()
                                        .debug_selector(move || {
                                            format!("agenda-month-color-{date}-{index}")
                                        })
                                        .w(px(3.))
                                        .h(px(12.))
                                        .flex_shrink_0()
                                        .rounded(px(1.))
                                        .bg(rgb(indicator_color))
                                        .when(item.status == AgendaItemStatus::Moment, |mark| {
                                            mark.h(px(3.)).rounded_full().bg(rgb(MUTED))
                                        }),
                                )
                                .child(
                                    single_line(item.title.clone(), 13.)
                                        .debug_selector(move || {
                                            format!("agenda-month-title-{date}-{index}")
                                        })
                                        .w(px(text_width))
                                        .flex_shrink_0()
                                        .text_color(rgb(title_color))
                                        .font_weight(if item.planned || item.moved {
                                            FontWeight::NORMAL
                                        } else {
                                            FontWeight::MEDIUM
                                        }),
                                )
                                .child(
                                    single_line(item.status.label(), 11.)
                                        .debug_selector(move || {
                                            format!("agenda-month-status-{date}-{index}")
                                        })
                                        .w(px(24.))
                                        .flex_shrink_0()
                                        .text_right()
                                        .text_color(rgb(
                                            if item.status == AgendaItemStatus::Running {
                                                INK
                                            } else {
                                                MUTED
                                            },
                                        ))
                                        .font_weight(if item.status == AgendaItemStatus::Running {
                                            FontWeight::MEDIUM
                                        } else {
                                            FontWeight::NORMAL
                                        }),
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
                                .custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(rgb(if in_month { PAPER } else { CHROME }).into())
                                        .foreground(rgb(INK).into())
                                        .border(transparent_black())
                                        .hover(rgb(HOVER).into())
                                        .active(rgb(PRESSED).into()),
                                )
                                .size_full()
                                .p_0()
                                .rounded(px(0.))
                                .overflow_hidden()
                                .child(content)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_agenda_day(date, window, cx);
                                })),
                        ),
                );
            }
            weeks.push(div().flex_1().min_h_0().w_full().flex().children(cells));
        }
        div()
            .debug_selector(|| "manager-content".into())
            .track_focus(&self.agenda.calendar_focus)
            .tab_stop(false)
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
                            .flex_shrink_0()
                            .mr(px(12.))
                            .text_size(px(28.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(month.format("%Y年%-m月").to_string()),
                    )
                    .child(
                        button("agenda-overview-previous")
                            .debug_selector(|| "agenda-overview-previous".into())
                            .custom(crate::view::quiet_variant(cx))
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
                            .custom(crate::view::quiet_variant(cx))
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
                            .custom(crate::view::quiet_variant(cx))
                            .h(px(32.))
                            .child(control_label("今天", INK))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select_agenda_date(Local::now().date_naive(), window, cx);
                                // The selected day may already be today while browsing another month.
                                this.agenda.month = Local::now().date_naive().with_day(1).unwrap();
                                this.refresh_agenda(cx);
                            })),
                    )
                    .child(div().flex_1())
                    .when(self.agenda_has_suspended_draft(cx), |header| {
                        header.child(
                            button("agenda-resume-draft")
                                .debug_selector(|| "agenda-resume-draft".into())
                                .custom(crate::view::quiet_variant(cx))
                                .h(px(32.))
                                .child(control_label("继续草稿", MUTED))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.resume_agenda_draft(window, cx);
                                })),
                        )
                    })
                    .when_some(self.strip_session(), |header, session| {
                        let id = session.id;
                        header.child(
                            button("agenda-overview-timer")
                                .debug_selector(|| "agenda-overview-timer".into())
                                .custom(crate::view::quiet_variant(cx))
                                .h(px(32.))
                                .tooltip(format!("返回计时：{}", session.title))
                                .child(control_label("返回计时", INK))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_session(id, window, cx);
                                })),
                        )
                    })
                    .when(!self.agenda.editor_visible, |header| {
                        header.child(
                            crate::control_style::plan_button(
                                "agenda-overview-add",
                                self.storage_failed,
                            )
                            .debug_selector(|| "agenda-overview-add".into())
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    if this.agenda.date.month() != this.agenda.month.month()
                                        || this.agenda.date.year() != this.agenda.month.year()
                                    {
                                        this.select_agenda_date(this.agenda.month, window, cx);
                                    }
                                    this.begin_plan(None, window, cx);
                                },
                            )),
                        )
                    }),
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
            .when(self.agenda.popup_open && !self.storage_failed, |body| {
                body.child(self.agenda_popover(window, cx))
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
                            .custom(crate::view::quiet_variant(cx))
                            .size(px(30.))
                            .icon(control_icon(IconName::ChevronLeft, INK))
                            .tooltip("上个月")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.change_agenda_month(-1, window, cx)
                            })),
                    )
                    .child(section_heading(&month.format("%Y年%-m月").to_string()))
                    .child(
                        button("agenda-month-next")
                            .debug_selector(|| "agenda-month-next".into())
                            .custom(crate::view::quiet_variant(cx))
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
                    .custom(crate::view::quiet_variant(cx))
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
                            .custom(crate::view::quiet_variant(cx))
                            .w_full()
                            .h(px(40.))
                            .px(px(6.))
                            .rounded(px(0.))
                            .border_0()
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
                                .custom(crate::view::quiet_variant(cx))
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
                        .custom(crate::view::quiet_variant(cx))
                        .overflow_hidden()
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
                                    single_line(plan.title.clone(), 14.)
                                        .debug_selector(|| "agenda-next-plan-title".into())
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
                                "专注 {}",
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
                            .custom(crate::view::quiet_variant(cx))
                            .h(px(32.))
                            .icon(control_icon(IconName::ChevronLeft, INK))
                            .child(control_label("月历", INK))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate(Page::Agenda, window, cx);
                            })),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .mr(px(8.))
                            .text_size(px(28.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if today {
                                "今天".to_owned()
                            } else {
                                date.format("%-m月%-d日").to_string()
                            }),
                    )
                    .child(
                        button("agenda-day-previous")
                            .custom(crate::view::quiet_variant(cx))
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
                            .custom(crate::view::quiet_variant(cx))
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
                    .child(div().flex_1())
                    .child(
                        crate::control_style::plan_button("agenda-add", self.storage_failed)
                            .debug_selector(|| "agenda-add".into())
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
                        date.format("%Y年%-m月%-d日"),
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
                        "{} 项安排 · 专注 {}",
                        self.agenda
                            .plans
                            .iter()
                            .filter(|plan| plan.moved_to.is_none())
                            .count(),
                        short_time(self.agenda.stats.focus_seconds)
                    )),
            )
    }

    fn agenda_calendar_grid(&self, day_width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let mut blocks = self.agenda_blocks();
        blocks.sort_by(|a, b| {
            a.visual_start()
                .total_cmp(&b.visual_start())
                .then_with(|| a.visual_end().total_cmp(&b.visual_end()))
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
                let visible_start = block.visual_start();
                if group_end > group_start && visible_start >= furthest_end {
                    break;
                }
                let visible_end = block.visual_end();
                let lane = lane_ends
                    .iter()
                    .position(|end| *end <= visible_start)
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
        let date = self.agenda.date;
        let now = Local.timestamp_opt(self.now, 0).single();
        let current_minute = now.filter(|time| time.date_naive() == date).map(|time| {
            time.hour() as f32 * 60. + time.minute() as f32 + time.second() as f32 / 60.
        });
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
                    .when(
                        !hour_label_overlaps_current_time(hour, current_minute),
                        |slot| {
                            slot.child(
                                div()
                                    .debug_selector(move || format!("agenda-hour-label-{hour}"))
                                    .absolute()
                                    .top(px(HOUR_LABEL_TOP))
                                    .left_0()
                                    .w(px(TIME_GUTTER - 8.))
                                    .h(px(HOUR_LABEL_HEIGHT))
                                    .text_size(px(12.))
                                    .line_height(px(HOUR_LABEL_HEIGHT))
                                    .text_color(rgb(MUTED))
                                    .text_align(TextAlign::Right)
                                    .child(format!("{hour:02}:00")),
                            )
                        },
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
                                .top(px(minute / 60. * HOUR_HEIGHT + CURRENT_TIME_LINE_TOP))
                                .left_0()
                                .right_0()
                                .h(px(1.))
                                .bg(rgb(INK))
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .top(px(CURRENT_TIME_LABEL_OFFSET))
                                        .w(px(TIME_GUTTER))
                                        .h(px(CURRENT_TIME_LABEL_HEIGHT))
                                        .py(px(2.))
                                        .rounded(px(3.))
                                        .text_center()
                                        .text_size(px(10.))
                                        .line_height(px(12.))
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
        if let CalendarTarget::Moment(id) = block.target {
            return self.agenda_moment_marker(id, block, canvas_width, cx);
        }
        let lane_count = block.lane_count;
        let text_width = ((canvas_width - TIME_GUTTER) / lane_count as f32 - 20.).max(100.);
        let top = block.start / 60. * HOUR_HEIGHT;
        let height = ((block.end - block.start) / 60. * HOUR_HEIGHT).max(MIN_EVENT_HEIGHT);
        let target = block.target;
        let foreground = block
            .color
            .as_deref()
            .and_then(tag_text_color)
            .unwrap_or(INK);
        let indicator = block.color.as_deref().and_then(tag_color_rgb);
        let secondary = MUTED;
        let status = block.status.description(!block.actual);
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
                    .custom(
                        crate::view::quiet_variant(cx)
                            .border(rgb(if block.selected { INK } else { LINE }).into())
                            .color(
                                rgb(if block.selected {
                                    CHROME
                                } else if block.actual
                                    || block.status == AgendaItemStatus::Completed
                                {
                                    SURFACE
                                } else {
                                    PAPER
                                })
                                .into(),
                            ),
                    )
                    .debug_selector(move || match target {
                        CalendarTarget::Plan(id) => format!("agenda-plan-{id}"),
                        CalendarTarget::Record(id) => format!("agenda-record-{id}"),
                        CalendarTarget::Completion(id) => format!("agenda-completion-{id}"),
                        CalendarTarget::Moment(id) => format!("agenda-moment-{id}"),
                    })
                    .w_full()
                    .h_full()
                    .px(px(7.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(if block.selected { INK } else { LINE }))
                    .justify_start()
                    .tooltip(format!("{}\n{status}", block.help))
                    .child(
                        div()
                            .w(px(text_width))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .w(px(text_width))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .w(px(3.))
                                            .h(px(12.))
                                            .flex_shrink_0()
                                            .rounded(px(1.))
                                            .bg(rgb(indicator.unwrap_or(LINE))),
                                    )
                                    .child(
                                        single_line(block.title.clone(), 13.)
                                            .debug_selector(move || {
                                                format!("agenda-event-title-{index}")
                                            })
                                            .w(px(text_width - 35.))
                                            .flex_shrink_0()
                                            .text_color(rgb(foreground))
                                            .font_weight(FontWeight::MEDIUM),
                                    )
                                    .child(
                                        single_line(block.status.label(), 11.)
                                            .debug_selector(move || {
                                                format!("agenda-event-status-{index}")
                                            })
                                            .w(px(24.))
                                            .flex_shrink_0()
                                            .text_right()
                                            .text_color(rgb(
                                                if block.status == AgendaItemStatus::Running {
                                                    INK
                                                } else {
                                                    secondary
                                                },
                                            ))
                                            .font_weight(
                                                if block.status == AgendaItemStatus::Running {
                                                    FontWeight::MEDIUM
                                                } else {
                                                    FontWeight::NORMAL
                                                },
                                            ),
                                    ),
                            )
                            .when(height >= 46., |column| {
                                column.child(
                                    single_line(block.detail.clone(), 12.)
                                        .w(px(text_width))
                                        .text_color(rgb(secondary)),
                                )
                            }),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        match target {
                            CalendarTarget::Plan(id) => this.select_plan(id, window, cx),
                            CalendarTarget::Record(id) => this.select_agenda_record(id, window, cx),
                            CalendarTarget::Completion(id) => {
                                this.select_agenda_completion(id, window, cx)
                            }
                            CalendarTarget::Moment(id) => {
                                if let Some(moment) = this
                                    .agenda
                                    .day_moments
                                    .iter()
                                    .find(|moment| moment.id == id)
                                    .cloned()
                                {
                                    this.edit_moment(moment, window, cx);
                                }
                            }
                        }
                    })),
            )
    }

    fn agenda_moment_marker(
        &self,
        id: i64,
        block: &CalendarBlock,
        canvas_width: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let width = ((canvas_width - TIME_GUTTER) / block.lane_count as f32 - 12.).max(100.);
        div()
            .block_mouse_except_scroll()
            .absolute()
            .left(relative(block.lane as f32 / block.lane_count as f32))
            .top(px(block.visual_start() / 60. * HOUR_HEIGHT))
            .w(relative(1. / block.lane_count as f32))
            .h(px(MIN_EVENT_HEIGHT))
            .px(px(3.))
            .child(
                button(("agenda-moment", id as u64))
                    .debug_selector(move || format!("agenda-moment-{id}"))
                    .custom(crate::view::quiet_variant(cx))
                    .w_full()
                    .h_full()
                    .px(px(3.))
                    .py_0()
                    .border_0()
                    .justify_start()
                    .tooltip(format!("{} · 随记\n{}", block.detail, block.help))
                    .child(
                        div()
                            .w(px(width))
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .child(
                                div()
                                    .size(px(6.))
                                    .rounded_full()
                                    .flex_shrink_0()
                                    .bg(rgb(INK)),
                            )
                            .child(div().w(px(8.)).h(px(1.)).flex_shrink_0().bg(rgb(LINE)))
                            .child(
                                single_line(format!("{}  {}", block.detail, block.title), 13.)
                                    .debug_selector(move || format!("agenda-moment-title-{id}"))
                                    .w(px(width - 24.))
                                    .flex_shrink_0()
                                    .text_color(rgb(INK)),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(moment) = this
                            .agenda
                            .day_moments
                            .iter()
                            .find(|moment| moment.id == id)
                            .cloned()
                        {
                            this.edit_moment(moment, window, cx);
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
            let status = self.agenda_plan_status(plan);
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
                status,
                color: self.agenda_task_color(plan.task_id).map(str::to_owned),
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
                    title: if session.tag_kind == TagKind::Rest && session.title != "休息" {
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
                    status: self.agenda_record_status(session),
                    color: self.display_session_color(session).map(str::to_owned),
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
                    status: self.agenda_record_status(session),
                    color: self.display_session_color(session).map(str::to_owned),
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
                status: AgendaItemStatus::Completed,
                color: self
                    .display_color(Some(task.tag_id), force_core::TagClass::Legacy, None)
                    .map(str::to_owned),
                actual: true,
                selected: self.agenda.selected_completion == Some(task.id),
                lane: 0,
                lane_count: 1,
            });
        }
        for moment in &self.agenda.day_moments {
            let minute = clock_minute(self.agenda.date, moment.occurred_at).min(1439.99);
            blocks.push(CalendarBlock {
                target: CalendarTarget::Moment(moment.id),
                title: moment.content.lines().next().unwrap_or_default().to_owned(),
                detail: timestamp_clock(moment.occurred_at),
                help: moment.content.clone(),
                start: minute,
                end: minute,
                status: AgendaItemStatus::Moment,
                color: None,
                actual: false,
                selected: false,
                lane: 0,
                lane_count: 1,
            });
        }
        blocks
    }

    pub(crate) fn agenda_selected_detail(&self, cx: &mut Context<Self>) -> Option<Div> {
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
                    .child(detail_title("agenda-record-title", &session.title))
                    .child(field_label(&format!(
                        "{} · 当天{} {}",
                        self.agenda_record_status(session).description(false),
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
                    .child(detail_title("agenda-completion-title", &task.title))
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
            .child(detail_title("agenda-plan-title", &plan.title))
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
                                .custom(crate::view::quiet_variant(cx))
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
                            .custom(crate::view::quiet_variant(cx))
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
                                .custom(crate::view::quiet_variant(cx))
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

fn detail_title(selector: &'static str, title: &str) -> Div {
    div()
        .debug_selector(move || selector.into())
        .flex_shrink_0()
        .text_size(px(17.))
        .text_color(rgb(INK))
        .whitespace_normal()
        .child(title.to_owned())
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

#[cfg(test)]
mod time_label_tests {
    use super::hour_label_overlaps_current_time;

    #[test]
    fn hides_only_hour_labels_overlapped_by_the_current_time_label() {
        assert!(!hour_label_overlaps_current_time(12, None));
        assert!(!hour_label_overlaps_current_time(12, Some(11. * 60. + 45.)));
        assert!(hour_label_overlaps_current_time(
            12,
            Some(11. * 60. + 45. + 1. / 60.)
        ));
        assert!(hour_label_overlaps_current_time(12, Some(11. * 60. + 52.)));
        assert!(!hour_label_overlaps_current_time(11, Some(11. * 60. + 52.)));
        assert!(hour_label_overlaps_current_time(
            12,
            Some(12. * 60. + 16. + 59. / 60.)
        ));
        assert!(!hour_label_overlaps_current_time(12, Some(12. * 60. + 17.)));
        assert!(hour_label_overlaps_current_time(0, Some(0.)));
        assert!(hour_label_overlaps_current_time(23, Some(23. * 60.)));
    }
}
