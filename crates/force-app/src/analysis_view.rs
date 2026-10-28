use force_core::TagClass;
use gpui::{prelude::*, *};
use gpui_component::{Icon, IconName, button::ButtonVariants};

use crate::{
    analysis_model::AnalysisPeriod,
    model::ForceView,
    view::{
        INK, LINE, MUTED, SURFACE, button, choice_button, control_icon, control_label,
        section_heading, short_time, timer_font,
    },
};

const CHART_HEIGHT: f32 = 148.;
const DATE_WIDTH: f32 = 106.;
const ROW_ACTION_WIDTH: f32 = 24.;
const CLASSES: [(TagClass, &str); 5] = [
    (TagClass::Positive, "正向"),
    (TagClass::Negative, "负向"),
    (TagClass::Neutral, "中性"),
    (TagClass::Leisure, "休闲"),
    (TagClass::Legacy, "未分类"),
];

impl ForceView {
    pub(crate) fn analysis_panel(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let content_width = f32::from(window.viewport_size().width).min(976.) - 80.;
        let range = self
            .analysis
            .days
            .first()
            .zip(self.analysis.days.last())
            .map(|(first, last)| {
                format!(
                    "{} — {}",
                    first.date.format("%Y/%m/%d"),
                    last.date.format("%m/%d")
                )
            })
            .unwrap_or_else(|| "包含今天".to_owned());
        div()
            .debug_selector(|| "analysis-content".into())
            .w_full()
            .flex()
            .flex_col()
            .gap(px(24.))
            .when_some(self.timer_context(cx), |body, context| body.child(context))
            .child(
                div()
                    .debug_selector(|| "analysis-toolbar".into())
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(div().text_size(px(14.)).text_color(rgb(MUTED)).child(range))
                    .child(
                        div().flex().gap(px(4.)).flex_shrink_0().children(
                            [
                                AnalysisPeriod::Week,
                                AnalysisPeriod::Month,
                                AnalysisPeriod::Quarter,
                            ]
                            .into_iter()
                            .map(|period| {
                                let selected = self.analysis.period == period;
                                choice_button(period.key(), selected, SURFACE, cx)
                                    .debug_selector(move || period.key().into())
                                    .h(px(32.))
                                    .px(px(12.))
                                    .child(control_label(
                                        format!("近 {} 天", period.days()),
                                        if selected { INK } else { MUTED },
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_analysis_period(period, cx)
                                    }))
                            }),
                        ),
                    ),
            )
            .when(self.analysis.error, |body| {
                body.child(
                    div()
                        .debug_selector(|| "analysis-error".into())
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .text_size(px(14.))
                        .child("读取分析失败。已有记录已保留。")
                        .child(
                            button("retry-analysis")
                                .custom(crate::view::quiet_variant(cx))
                                .h(px(32.))
                                .child(control_label("重试", INK))
                                .on_click(cx.listener(|this, _, _, cx| this.refresh_analysis(cx))),
                        ),
                )
            })
            .when(!self.analysis.error, |body| {
                body.child(self.analysis_summary(content_width, cx))
                    .child(self.analysis_chart(content_width, cx))
                    .child(self.analysis_table(content_width, cx))
            })
    }

    fn analysis_summary(&self, content_width: f32, cx: &App) -> Div {
        let column_width = (content_width - 48.) / 3.;
        div()
            .debug_selector(|| "analysis-summary".into())
            .w(px(content_width))
            .flex()
            .gap(px(24.))
            .py(px(18.))
            .border_t_1()
            .border_b_1()
            .border_color(rgb(LINE))
            .children(
                [
                    (
                        "analysis-total-focus",
                        "专注时长",
                        self.analysis.total.focus_seconds,
                    ),
                    (
                        "analysis-average-focus",
                        "日均专注",
                        self.analysis.average_focus_seconds(),
                    ),
                    (
                        "analysis-total-investment",
                        "计时投入",
                        self.analysis.total.task_seconds,
                    ),
                ]
                .into_iter()
                .map(|(id, title, seconds)| {
                    div()
                        .debug_selector(move || id.into())
                        .w(px(column_width))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(div().text_size(px(13.)).text_color(rgb(MUTED)).child(title))
                        .child(
                            div()
                                .font(timer_font(cx))
                                .text_size(px(24.))
                                .font_weight(FontWeight::MEDIUM)
                                .line_height(relative(1.2))
                                .child(short_time(seconds).replace(' ', "")),
                        )
                }),
            )
    }

    fn analysis_chart(&self, content_width: f32, cx: &mut Context<Self>) -> Div {
        let day_count = self.analysis.days.len().max(1) as f32;
        let slot_width = ((content_width - 60. - (day_count - 1.) * 2.) / day_count).max(1.);
        let peak = self
            .analysis
            .days
            .iter()
            .map(|day| day.values.focus_seconds)
            .max()
            .unwrap_or(0);
        let scale = chart_ceiling(peak);
        let labels: Vec<_> = [
            0,
            self.analysis.days.len().saturating_sub(1) / 2,
            self.analysis.days.len().saturating_sub(1),
        ]
        .into_iter()
        .filter_map(|index| self.analysis.days.get(index))
        .map(|day| day.date.format("%-m/%-d").to_string())
        .collect();
        div()
            .debug_selector(|| "analysis-trend".into())
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(section_heading("专注趋势"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child("点击查看当日记录"),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .w(px(48.))
                            .flex_shrink_0()
                            .h(px(CHART_HEIGHT))
                            .flex()
                            .flex_col()
                            .justify_between()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .text_align(TextAlign::Right)
                            .child(short_axis(scale))
                            .child(short_axis(scale / 2))
                            .child("0"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .debug_selector(|| "analysis-chart-plot".into())
                                    .relative()
                                    .w_full()
                                    .h(px(CHART_HEIGHT))
                                    .border_b_1()
                                    .border_color(rgb(LINE))
                                    .child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .top_0()
                                            .w_full()
                                            .h(px(1.))
                                            .bg(rgb(LINE)),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .top(px(CHART_HEIGHT / 2.))
                                            .w_full()
                                            .h(px(1.))
                                            .bg(rgb(LINE)),
                                    )
                                    .child(
                                        div()
                                            .relative()
                                            .size_full()
                                            .flex()
                                            .items_end()
                                            .gap(px(2.))
                                            .children(self.analysis.days.iter().map(|day| {
                                                let date = day.date;
                                                let seconds = day.values.focus_seconds;
                                                let height = CHART_HEIGHT
                                                    * (seconds as f64 / scale as f64) as f32;
                                                let tooltip = format!(
                                                    "{} · 专注 {}",
                                                    date.format("%Y年%-m月%-d日"),
                                                    table_duration(seconds)
                                                );
                                                button(SharedString::from(format!(
                                                    "analysis-bar-{date}"
                                                )))
                                                .debug_selector(move || {
                                                    format!("analysis-bar-{date}")
                                                })
                                                .custom(crate::view::quiet_variant(cx))
                                                .w(px(slot_width))
                                                .h(px(CHART_HEIGHT))
                                                .p_0()
                                                .border_0()
                                                .rounded(px(2.))
                                                // The daily rows below expose the same
                                                // destinations in the keyboard tab order.
                                                .tab_stop(false)
                                                .tooltip(format!("{tooltip} · 查看记录"))
                                                .child(
                                                    div()
                                                        .w(px(slot_width))
                                                        .h(px(CHART_HEIGHT))
                                                        .flex()
                                                        .items_end()
                                                        .justify_center()
                                                        .child(
                                                            div()
                                                                .w(px(slot_width.min(28.)))
                                                                .h(px(height))
                                                                .bg(rgb(0x363636)),
                                                        ),
                                                )
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.open_analysis_day(date, window, cx)
                                                    },
                                                ))
                                            })),
                                    )
                                    .when(peak == 0, |plot| {
                                        plot.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(px(13.))
                                                .text_color(rgb(MUTED))
                                                .child("这段时间还没有专注计时"),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED))
                                    .children(labels),
                            ),
                    ),
            )
    }

    fn analysis_table(&self, content_width: f32, cx: &mut Context<Self>) -> Div {
        let data_width = (content_width - DATE_WIDTH - ROW_ACTION_WIDTH) / 6.;
        div().debug_selector(|| "analysis-table".into()).w_full().flex().flex_col()
            .child(div().mb(px(12.)).flex().items_center().justify_between()
                .child(section_heading("每日明细"))
                .child(div().flex().items_center().gap(px(8.))
                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("时:分:秒"))
                    .child(choice_button("analysis-help", self.analysis.help_open, SURFACE, cx)
                        .debug_selector(|| "analysis-help".into()).size(px(24.))
                        .icon(control_icon(Icon::new(IconName::Info).size(px(13.)), MUTED))
                        .tooltip("统计说明")
                        .on_click(cx.listener(|this, _, _, cx| { this.analysis.help_open = !this.analysis.help_open; cx.notify(); })))),
            )
            .when(self.analysis.help_open, |table| table.child(
                div().debug_selector(|| "analysis-help-text".into()).mb(px(12.)).pl(px(12.))
                    .border_l_2().border_color(rgb(LINE)).text_size(px(13.)).text_color(rgb(MUTED)).whitespace_normal()
                    .child("专注合并正向、负向及未分类的专注时段，暂停和休息不计入。分类与计时投入按每条记录累加，重叠记录分别计入；日均包含没有计时的日期。跨日按当地日期拆分，随手记不计时。")),
            )
            .child(div().debug_selector(|| "analysis-table-header".into())
                .w(px(content_width)).h(px(36.)).flex().items_center().border_b_1().border_color(rgb(LINE))
                .text_size(px(12.)).text_color(rgb(MUTED))
                .child(div().w(px(DATE_WIDTH)).flex_shrink_0().child("日期"))
                .child(table_cell("专注", data_width).debug_selector(|| "analysis-header-focus".into()))
                .children(CLASSES.into_iter().enumerate().map(|(index, (_, label))|
                    table_cell(label, data_width).debug_selector(move || format!("analysis-header-class-{index}"))))
                .child(div().debug_selector(|| "analysis-header-arrow".into()).w(px(ROW_ACTION_WIDTH)).flex_shrink_0()))
            .children(self.analysis.days.iter().rev().map(|day| {
                let date = day.date;
                button(SharedString::from(format!("analysis-day-{date}")))
                    .debug_selector(move || format!("analysis-day-{date}"))
                    .custom(crate::view::quiet_variant(cx))
                    .w_full().h(px(38.)).p_0().rounded(px(0.))
                    .border_0().border_b_1().border_color(rgb(LINE))
                    .tooltip(format!("{} · 查看记录", date.format("%Y年%-m月%-d日")))
                    .child(div().debug_selector(move || format!("analysis-day-content-{date}"))
                        // Button content wrappers do not resolve percentage
                        // widths reliably; size the complete row to the grid.
                        .w(px(content_width)).flex_shrink_0().flex().items_center()
                        .text_size(px(12.)).font(timer_font(cx)).text_color(rgb(INK))
                        .child(div().w(px(DATE_WIDTH)).flex_shrink_0().text_color(rgb(MUTED)).child(date.format("%Y-%m-%d").to_string()))
                        .child(table_cell(table_duration(day.values.focus_seconds), data_width)
                            .debug_selector(move || format!("analysis-day-focus-{date}"))
                            .font_weight(FontWeight::MEDIUM))
                        .children(CLASSES.into_iter().enumerate().map(|(index, (class, _))| {
                            let seconds = day.values.class_seconds(class);
                            table_cell(if seconds == 0 { "—".to_owned() } else { table_duration(seconds) }, data_width)
                                .debug_selector(move || format!("analysis-day-class-{index}-{date}"))
                                .text_color(rgb(MUTED))
                        }))
                        .child(div().debug_selector(move || format!("analysis-day-arrow-{date}"))
                            .w(px(ROW_ACTION_WIDTH)).flex_shrink_0().flex().justify_end()
                            .child(control_icon(Icon::new(IconName::ChevronRight).size(px(12.)), MUTED))))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_analysis_day(date, window, cx)
                    }))
            }))
    }
}

fn table_cell(text: impl Into<SharedString>, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .text_align(TextAlign::Right)
        .child(text.into())
}

pub(crate) fn table_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn short_axis(seconds: i64) -> String {
    if seconds >= 3600 {
        let hours = seconds as f64 / 3600.;
        if seconds % 3600 == 0 {
            format!("{hours:.0} 时")
        } else {
            format!("{hours:.1} 时")
        }
    } else if seconds >= 60 {
        format!("{} 分", seconds / 60)
    } else {
        format!("{seconds} 秒")
    }
}

fn chart_ceiling(peak: i64) -> i64 {
    let unit = if peak > 3600 { 3600 } else { 600 };
    peak.max(1).saturating_add(unit - 1) / unit * unit
}
