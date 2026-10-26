use chrono::{Datelike, Local, TimeZone};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable,
    button::{ButtonCustomVariant, ButtonVariants},
};

use crate::{
    model::ForceView,
    view::{INK, MUTED, PAPER, button, section_heading},
};

const EMPTY: u32 = 0xececec;
const LEVELS: [u32; 4] = [0xc9c9c9, 0x989898, 0x626262, 0x242424];
const FUTURE: u32 = 0xf8f8f8;

impl ForceView {
    pub(crate) fn activity_calendar(&self, cx: &mut Context<Self>) -> Div {
        let today = Local
            .timestamp_opt(self.now, 0)
            .single()
            .map_or_else(|| Local::now().date_naive(), |time| time.date_naive());
        let week_count = self.activity_days.len().div_ceil(7);
        let selected_visible = self
            .activity_days
            .iter()
            .any(|day| day.date == self.record_date && day.date <= today);

        div()
            .debug_selector(|| "activity-calendar".into())
            .w_full()
            .max_w(px(896.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(section_heading("过去一年"))
                    .child(activity_legend()),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div().h(px(18.)).w_full().flex().gap(px(2.)).children(
                            self.activity_days
                                .chunks(7)
                                .enumerate()
                                .map(|(week_index, week)| {
                                    let month = week
                                        .iter()
                                        .find(|day| day.date.day() == 1 && day.date <= today)
                                        .map(|day| day.date.month());
                                    div().flex_1().min_w_0().relative().when_some(
                                        month,
                                        |column, month| {
                                            column.child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .w(px(38.))
                                                    .when(week_index + 3 >= week_count, |label| {
                                                        label.right_0().text_align(TextAlign::Right)
                                                    })
                                                    .when(week_index + 3 < week_count, |label| {
                                                        label.left_0()
                                                    })
                                                    .text_size(px(13.))
                                                    .text_color(rgb(MUTED))
                                                    .child(format!("{month}月")),
                                            )
                                        },
                                    )
                                }),
                        ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "activity-grid".into())
                            .w_full()
                            .flex()
                            .gap(px(2.))
                            .children(self.activity_days.chunks(7).enumerate().map(
                                |(week_index, week)| {
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(3.))
                                        .children(week.iter().enumerate().map(|(weekday, day)| {
                                            let date = day.date;
                                            let future = date > today;
                                            let selected = date == self.record_date && !future;
                                            let index = week_index * 7 + weekday;
                                            let fill = if future {
                                                FUTURE
                                            } else {
                                                activity_color(day.seconds)
                                            };
                                            let hover = if day.seconds >= 7_200 {
                                                0x454545
                                            } else {
                                                activity_hover_color(fill)
                                            };
                                            let label = if future {
                                                format!(
                                                    "{} · 尚未到来",
                                                    date.format("%Y年%-m月%-d日")
                                                )
                                            } else {
                                                format!(
                                                    "{} · 计时 {}",
                                                    date.format("%Y年%-m月%-d日"),
                                                    activity_duration(day.seconds)
                                                )
                                            };
                                            div()
                                                .w_full()
                                                .max_w(px(12.))
                                                .h(px(12.))
                                                .flex_shrink_0()
                                                .when(index == 0, |cell| {
                                                    cell.debug_selector(|| {
                                                        "activity-cell-first".into()
                                                    })
                                                })
                                                .when(
                                                    index + 1 == self.activity_days.len(),
                                                    |cell| {
                                                        cell.debug_selector(|| {
                                                            "activity-cell-last".into()
                                                        })
                                                    },
                                                )
                                                .child(
                                                    button(SharedString::from(format!(
                                                        "activity-day-{date}"
                                                    )))
                                                    .debug_selector(move || {
                                                        format!("activity-day-{date}")
                                                    })
                                                    .custom(
                                                        ButtonCustomVariant::new(cx)
                                                            .color(rgb(fill).into())
                                                            .foreground(rgb(INK).into())
                                                            .border(
                                                                rgb(if selected {
                                                                    PAPER
                                                                } else {
                                                                    fill
                                                                })
                                                                .into(),
                                                            )
                                                            .hover(rgb(hover).into())
                                                            .active(rgb(INK).into()),
                                                    )
                                                    .w_full()
                                                    .h(px(12.))
                                                    .min_w_0()
                                                    .p_0()
                                                    .rounded(px(2.))
                                                    .tab_stop(
                                                        selected
                                                            || (!selected_visible && date == today),
                                                    )
                                                    .disabled(future)
                                                    .when(future, |button| {
                                                        button
                                                            .bg(rgb(FUTURE))
                                                            .border_color(rgb(0xeeeeee))
                                                            .cursor_default()
                                                    })
                                                    .when(selected, |button| {
                                                        button.shadow(vec![BoxShadow {
                                                            color: rgb(INK).into(),
                                                            offset: point(px(0.), px(0.)),
                                                            blur_radius: px(0.),
                                                            spread_radius: px(1.),
                                                        }])
                                                    })
                                                    .tooltip(label)
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.select_record_date(date, cx);
                                                    })),
                                                )
                                        }))
                                },
                            )),
                    ),
            )
    }
}

fn activity_legend() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(13.))
        .text_color(rgb(MUTED))
        .child("少")
        .children(
            std::iter::once(EMPTY)
                .chain(LEVELS)
                .map(|color| div().size(px(12.)).rounded(px(2.)).bg(rgb(color))),
        )
        .child("多")
}

fn activity_color(seconds: i64) -> u32 {
    match seconds {
        ..=0 => EMPTY,
        1..=1_799 => LEVELS[0],
        1_800..=3_599 => LEVELS[1],
        3_600..=7_199 => LEVELS[2],
        _ => LEVELS[3],
    }
}

fn activity_hover_color(color: u32) -> u32 {
    let gray = (color & 0xff).saturating_sub(18);
    (gray << 16) | (gray << 8) | gray
}

fn activity_duration(seconds: i64) -> String {
    if seconds <= 0 {
        return "0 分钟".to_owned();
    }
    if seconds < 60 {
        return "不足 1 分钟".to_owned();
    }
    let minutes = seconds / 60;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} 分钟"),
        (hours, 0) => format!("{hours} 小时"),
        (hours, minutes) => format!("{hours} 小时 {minutes} 分钟"),
    }
}
