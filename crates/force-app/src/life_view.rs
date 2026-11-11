use chrono::{Local, TimeZone};
use force_core::{LifeBoard, LifeItem, LifeModule, LifeSpan};
use gpui::{prelude::*, *};
use gpui_component::{
    IconName,
    button::ButtonVariants,
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenuItem},
    tooltip::Tooltip,
};

use crate::{
    editor_dialog,
    label::single_line,
    life_model::{LifeEdit, NextField, PreviousField},
    model::ForceView,
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, PAPER, SURFACE, button, control_icon,
        control_label, field, field_label, outlined_button, primary_button, quiet_variant,
        record_timestamp, section_heading, timer_font,
    },
};

const MODULES: [(LifeModule, &str, &str); 3] = [
    (LifeModule::Table, "记录表", "life-toggle-table"),
    (LifeModule::Trend, "间隔趋势", "life-toggle-trend"),
    (LifeModule::Total, "累计天数", "life-toggle-total"),
];
const DATE_WIDTH: f32 = 162.;
const ELAPSED_WIDTH: f32 = 126.;
const ACTION_WIDTH: f32 = 140.;
const PLOT_HEIGHT: f32 = 120.;
const SPANS_SHOWN: usize = 8;

impl ForceView {
    pub(crate) fn life_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let unavailable = self.storage_failed || self.life.load_failed || self.life.board.is_none();
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                outlined_button("life-customize")
                    .debug_selector(|| "life-customize".into())
                    .h(px(36.))
                    .unavailable(unavailable)
                    .icon(control_icon(IconName::Settings2, INK))
                    .child(control_label(
                        if self.life.customizing {
                            "完成"
                        } else {
                            "自定义"
                        },
                        INK,
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.life.customizing = !this.life.customizing;
                        cx.notify();
                    })),
            )
            .child(
                primary_button("life-add")
                    .debug_selector(|| "life-add".into())
                    .h(px(36.))
                    .unavailable(unavailable)
                    .when(self.life.edit.is_none(), |button| {
                        button.icon(control_icon(IconName::Plus, PAPER))
                    })
                    .child(control_label(
                        if self.life.edit.is_some() {
                            "继续编辑"
                        } else {
                            "添加事项"
                        },
                        PAPER,
                    ))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.begin_life_editor(LifeEdit::Add, window, cx);
                    })),
            )
    }

    pub(crate) fn life_panel(&self, cx: &mut Context<Self>) -> Div {
        let panel = div()
            .debug_selector(|| "life-dashboard".into())
            .w_full()
            .flex()
            .flex_col()
            .gap(px(28.))
            .when_some(self.timer_context(cx), |panel, context| {
                panel.child(context)
            });
        let Some(board) = self.life.board.as_ref().filter(|_| !self.life.load_failed) else {
            return panel.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child("读取生活看板失败。")
                    .child(
                        outlined_button("life-retry")
                            .h(px(32.))
                            .child(control_label("重试", INK))
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_life(cx))),
                    ),
            );
        };
        let panel = panel.when(self.life.customizing, |panel| {
            panel.child(
                div()
                    .debug_selector(|| "life-module-picker".into())
                    .flex()
                    .items_center()
                    .gap(px(24.))
                    .px(px(16.))
                    .py(px(12.))
                    .rounded(px(6.))
                    .bg(rgb(SURFACE))
                    .child(field_label("显示模块"))
                    .children(MODULES.map(|(module, label, id)| {
                        div().debug_selector(move || id.into()).child(
                            Checkbox::new(id)
                                .checked(board.shows(module))
                                .label(label)
                                .on_click(cx.listener(move |this, visible: &bool, _, cx| {
                                    this.set_life_module(module, *visible, cx);
                                })),
                        )
                    })),
            )
        });
        if board.items.is_empty() {
            return panel.child(life_empty("添加事项后，就能查看距离上次发生过了多久。"));
        }
        let table = board.shows(LifeModule::Table);
        let trend = board.shows(LifeModule::Trend);
        let total = board.shows(LifeModule::Total);
        if !table && !trend && !total {
            return panel.child(life_empty("当前未显示模块。可在「自定义」中选择。"));
        }
        panel
            .when(table, |panel| panel.child(self.life_table(board, cx)))
            .when(trend || total, |panel| {
                panel.child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(32.))
                        .when(trend, |row| row.child(self.life_trend(board, cx)))
                        .when(total, |row| row.child(self.life_total(board, trend, cx))),
                )
            })
    }

    fn life_table(&self, board: &LifeBoard, cx: &mut Context<Self>) -> Div {
        div()
            .debug_selector(|| "life-table".into())
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(section_heading("记录表"))
            .child(
                div()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(
                        table_row()
                            .h(px(34.))
                            .text_size(px(13.))
                            .text_color(rgb(MUTED))
                            .child(div().flex_1().child("事项"))
                            .child(div().w(px(DATE_WIDTH)).child("上次发生"))
                            .child(div().w(px(ELAPSED_WIDTH)).child("多久没做"))
                            .child(div().w(px(ACTION_WIDTH))),
                    )
                    .children(board.items.iter().map(|item| self.life_row(item, cx))),
            )
    }

    fn life_row(&self, item: &LifeItem, cx: &mut Context<Self>) -> Div {
        let id = item.id;
        let elapsed = item.elapsed_at(self.now);
        table_row()
            .debug_selector(move || format!("life-row-{id}"))
            .min_h(px(51.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(life_name(item, "life-table-name")),
            )
            .child(
                div()
                    .w(px(DATE_WIDTH))
                    .flex_shrink_0()
                    .text_color(rgb(MUTED))
                    .text_size(px(13.))
                    .child(record_timestamp(item.last_occurred_at())),
            )
            .child(
                single_line(life_duration(elapsed, false), 14.)
                    .id(("life-elapsed", id as u64))
                    .w(px(ELAPSED_WIDTH))
                    .flex_shrink_0()
                    .font_weight(FontWeight::MEDIUM)
                    .tooltip(move |window, cx| {
                        Tooltip::new(life_duration(elapsed, true)).build(window, cx)
                    }),
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
                            .child(control_label("记一次", INK))
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
            .unwrap_or(&board.items[0]);
        let spans = item.recent_spans(self.now, SPANS_SHOWN);
        let peak = spans.iter().map(|span| span.seconds).max().unwrap_or(0);
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
        div()
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
                    .child(section_heading("间隔趋势").flex_shrink_0())
                    .child(life_chart_picker(board, item, cx)),
            )
            .child(
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
                                .children(spans.iter().copied().map(|span| life_bar(span, peak))),
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
                                            Local.timestamp_opt(at, 0).single().map_or_else(
                                                || "—".to_owned(),
                                                |time| time.format("%-m/%-d").to_string(),
                                            )
                                        },
                                    );
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
                    .child(section_heading("累计天数"))
                    .child(
                        button("life-total-help")
                            .custom(quiet_variant(cx))
                            .size(px(24.))
                            .icon(control_icon(IconName::Info, MUTED))
                            .tooltip("每段满 24 小时计 1 天，累计各段及当前已满的天数。"),
                    ),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .children(board.items.iter().map(|item| {
                        table_row()
                            .min_h(px(48.))
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
                                    .items_baseline()
                                    .gap(px(5.))
                                    .child(
                                        div()
                                            .font(timer_font(cx))
                                            .text_size(px(22.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(item.cumulative_days(self.now).to_string()),
                                    )
                                    .child(
                                        div().text_size(px(13.)).text_color(rgb(MUTED)).child("天"),
                                    ),
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
        let title = match edit {
            LifeEdit::Add => "添加事项".to_owned(),
            LifeEdit::Edit(_) => "编辑事项".to_owned(),
            LifeEdit::Record(item) => format!("记一次 · {}", item.title),
        };
        let close = button("life-close")
            .debug_selector(|| "life-close".into())
            .custom(quiet_variant(cx))
            .size(px(28.))
            .icon(control_icon(IconName::Close, MUTED))
            .tooltip("关闭并保留草稿")
            .on_click(cx.listener(|this, _, window, cx| this.suspend_life_editor(window, cx)));
        let fields = editor_dialog::fields("life-editor-fields", &self.life.scroll)
            .when(!matches!(edit, LifeEdit::Record(_)), |fields| {
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
            .child(
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
                            .child(field_label(if matches!(edit, LifeEdit::Record(_)) {
                                "发生日期"
                            } else {
                                "上次发生日期"
                            }))
                            .child(field(&self.life.date)),
                    )
                    .child(
                        div()
                            .debug_selector(|| "life-time".into())
                            .w(px(148.))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(field_label("时间"))
                            .child(field(&self.life.time)),
                    ),
            );
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

fn life_empty(message: &'static str) -> Div {
    div()
        .debug_selector(|| "life-empty".into())
        .border_t_1()
        .border_color(rgb(LINE))
        .pt(px(20.))
        .text_color(rgb(MUTED))
        .child(message)
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

fn life_bar(span: LifeSpan, peak: i64) -> Stateful<Div> {
    let ratio = span.seconds as f32 / peak.max(1) as f32;
    let height = if span.seconds == 0 {
        0.
    } else {
        (ratio * (PLOT_HEIGHT - 2.)).max(2.)
    };
    let current = span.end.is_none();
    let from = record_timestamp(span.start);
    let until = span.end.map_or_else(|| "当前".to_owned(), record_timestamp);
    let detail = format!("{from} 至 {until} · {}", life_duration(span.seconds, true));
    div()
        .id(("life-chart-span", span.start as u64))
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
                .bg(rgb(if current { INK } else { 0xa8a8a8 })),
        )
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
