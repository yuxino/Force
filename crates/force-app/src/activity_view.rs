use chrono::{Datelike, Duration, Local, TimeZone};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable,
    button::{ButtonCustomVariant, ButtonVariants},
    tooltip::Tooltip,
};

use crate::{
    model::ForceView,
    view::{INK, MUTED, PAPER, button, section_heading},
};

const EMPTY: u32 = 0xf0f0f0;
const LEVELS: [u32; 4] = [0xc9c9c9, 0x989898, 0x626262, 0x242424];
const FUTURE: u32 = 0xf8f8f8;

impl ForceView {
    pub(crate) fn activity_calendar(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let today = Local
            .timestamp_opt(self.now, 0)
            .single()
            .map_or_else(|| Local::now().date_naive(), |time| time.date_naive());
        let week_count = self.activity_days.len().div_ceil(7);
        let selected_visible = self
            .activity_days
            .iter()
            .any(|day| day.date == self.record_date && day.date <= today);
        let focus_date = if selected_visible {
            self.record_date
        } else {
            today
        };
        let grid_focused = self.activity_focus.is_focused(window);
        let activity_bounds = self.activity_bounds.clone();

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
                            .track_focus(&self.activity_focus)
                            .on_key_down(cx.listener(
                                move |this, event: &KeyDownEvent, window, cx| {
                                    if event.keystroke.modifiers.modified() {
                                        return;
                                    }
                                    let days = match event.keystroke.key.as_str() {
                                        "left" => -7,
                                        "right" => 7,
                                        "up" => -1,
                                        "down" => 1,
                                        "enter" | "space" => 0,
                                        _ => return,
                                    };
                                    window.prevent_default();
                                    cx.stop_propagation();
                                    if let Some(date) = this.activity_hovered_day {
                                        this.activity_invalidated_tooltip_day = Some(date);
                                        this.activity_tooltip_generation =
                                            this.activity_tooltip_generation.wrapping_add(1);
                                    }
                                    this.reveal_activity_grid(cx);
                                    let today = Local::now().date_naive();
                                    let anchor = if this.activity_days.iter().any(|day| {
                                        day.date == this.record_date && day.date <= today
                                    }) {
                                        this.record_date
                                    } else {
                                        today
                                    };
                                    if let Some(date) =
                                        anchor.checked_add_signed(Duration::days(days))
                                        && date <= today
                                        && this.activity_days.iter().any(|day| day.date == date)
                                    {
                                        this.select_record_date(date, cx);
                                    }
                                },
                            ))
                            .w_full()
                            .relative()
                            .flex()
                            .gap(px(2.))
                            .child(
                                canvas(
                                    move |bounds, _, _| activity_bounds.set(bounds),
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .size_full(),
                            )
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
                                            let generation = if self
                                                .activity_invalidated_tooltip_day
                                                == Some(date)
                                            {
                                                self.activity_tooltip_generation
                                            } else {
                                                0
                                            };
                                            #[cfg(all(test, feature = "ui-test-support"))]
                                            let tooltip_probe = self.activity_tooltip_probe.clone();
                                            let tooltip =
                                                move |window: &mut Window, cx: &mut App| {
                                                    let text = label.clone();
                                                    #[cfg(all(test, feature = "ui-test-support"))]
                                                    let tooltip_probe = tooltip_probe.clone();
                                                    Tooltip::element(move |_, _| {
                                                        #[cfg(all(
                                                            test,
                                                            feature = "ui-test-support"
                                                        ))]
                                                        tooltip_probe
                                                            .replace(Some((date, text.clone())));
                                                        div().child(text.clone())
                                                    })
                                                    .build(window, cx)
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
                                                .child({
                                                    let mut cell = button(ElementId::NamedInteger(
                                                        SharedString::from(format!(
                                                            "activity-day-{date}"
                                                        )),
                                                        generation,
                                                    ))
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
                                                    .tab_stop(false)
                                                    .disabled(future)
                                                    .when(future, |button| {
                                                        button
                                                            .bg(rgb(FUTURE))
                                                            .border_color(rgb(0xeeeeee))
                                                            .cursor_default()
                                                    })
                                                    .when(
                                                        selected
                                                            || (grid_focused && date == focus_date),
                                                        |button| {
                                                            button.shadow(vec![BoxShadow {
                                                                color: rgb(INK).into(),
                                                                offset: point(px(0.), px(0.)),
                                                                blur_radius: px(0.),
                                                                spread_radius: px(
                                                                    if grid_focused
                                                                        && date == focus_date
                                                                    {
                                                                        2.
                                                                    } else {
                                                                        1.
                                                                    },
                                                                ),
                                                            }])
                                                        },
                                                    )
                                                    .on_mouse_move(cx.listener(
                                                        move |this, _: &MouseMoveEvent, _, _| {
                                                            this.activity_hovered_day = Some(date);
                                                        },
                                                    ))
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            if !this
                                                                .activity_focus
                                                                .is_focused(window)
                                                            {
                                                                this.activity_pointer_focus = true;
                                                                window.focus(&this.activity_focus);
                                                            }
                                                            this.select_record_date(date, cx);
                                                        },
                                                    ));
                                                    cell.interactivity().tooltip(tooltip);
                                                    cell
                                                })
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
