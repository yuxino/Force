use chrono::{Local, TimeZone};
use force_core::{Session, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{Icon, IconName, Sizable, button::ButtonVariants, tooltip::Tooltip};

use crate::{
    label::single_line,
    model::ForceView,
    view::{
        ControlState, DESTRUCTIVE, INK, LINE, MUTED, SURFACE, TASK_SIDEBAR_WIDTH, TASK_TEXT_WIDTH,
        action_icon, button, choice_button, clock, control_icon, control_label, field, field_label,
        primary_button, section_heading, short_time, task_heading, timer_font, underline_button,
    },
};

impl ForceView {
    fn today_overview(&self, cx: &App) -> Div {
        let fraction =
            (self.today.focus_seconds as f32 / (self.goal.max(1) as f32 * 60.)).clamp(0., 1.);
        let total = short_time(self.today.focus_seconds);
        let goal = short_time(i64::from(self.goal) * 60);
        let summary = format!("今日专注 {total}，每日目标 {goal}");
        div()
            .debug_selector(|| "focus-summary".into())
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(16.))
            .pb(px(28.))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .gap(px(32.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_baseline()
                            .gap(px(12.))
                            .child(section_heading("今日专注"))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(MUTED))
                                    .child(Local::now().format("%m 月 %d 日").to_string()),
                            ),
                    )
                    .child(
                        div()
                            .id("focus-goal")
                            .debug_selector(|| "focus-goal".into())
                            .max_w(px(300.))
                            .flex_shrink_0()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .tooltip(move |window, cx| {
                                Tooltip::new(summary.clone()).build(window, cx)
                            })
                            .child(
                                div()
                                    .debug_selector(|| "focus-day-total".into())
                                    .font(timer_font(cx))
                                    .text_size(px(22.))
                                    .line_height(relative(1.2))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(total.replace(' ', "")),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "focus-day-target".into())
                                    .font(timer_font(cx))
                                    .text_size(px(13.))
                                    .text_color(rgb(MUTED))
                                    .child(format!("/ {}", goal.replace(' ', ""))),
                            ),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "focus-day-progress".into())
                    .h(px(2.))
                    .w_full()
                    .bg(rgb(LINE))
                    .child(div().h_full().w(relative(fraction)).bg(rgb(INK))),
            )
    }

    pub(crate) fn storage_notice(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(rgb(DESTRUCTIVE))
                    .child("无法读写本地数据。原文件已保留。"),
            )
            .child(
                button("retry-storage")
                    .debug_selector(|| "retry-storage".into())
                    .child(control_label("重试", INK))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let preserve_goal_draft = this.goal_has_unsaved_changes(cx);
                        this.open_store(cx);
                        if !this.storage_failed && !preserve_goal_draft {
                            let goal = this.goal.to_string();
                            this.goal_input
                                .update(cx, |input, cx| input.set_value(goal, window, cx));
                        }
                    })),
            )
    }

    pub(crate) fn focus_panel(&self, compact: bool, cx: &mut Context<Self>) -> Div {
        // Summary and task navigation stay in place while only the operation
        // changes between preparing, running and the last round's receipt.
        div()
            .debug_selector(|| "manager-content".into())
            .flex_1()
            .min_h_0()
            .w_full()
            .max_w(px(976.))
            .mx_auto()
            .px(px(40.))
            .pt(px(32.))
            .pb(px(32.))
            .flex()
            .flex_col()
            .child(self.today_overview(cx))
            .when(self.storage_failed, |body| {
                body.child(self.storage_notice(cx))
            })
            .when(!self.storage_failed, |body| {
                body.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .gap(px(32.))
                        .child(
                            div()
                                .id("focus-operation")
                                .debug_selector(|| "focus-operation".into())
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .overflow_y_scroll()
                                .track_scroll(&self.panel_scroll[0])
                                .pr(px(4.))
                                .pb(px(8.))
                                .when(self.browsing_other_timer(), |body| {
                                    body.when_some(self.timer_context(cx), |body, context| {
                                        body.child(context.mb(px(12.)))
                                    })
                                })
                                .child(if let Some(session) = &self.completed {
                                    self.completed_panel(session, compact, cx)
                                        .into_any_element()
                                } else if !self.preparing
                                    && let Some(session) = self.current_session()
                                {
                                    self.running_panel(session, compact, cx).into_any_element()
                                } else {
                                    self.prepare_panel(cx).into_any_element()
                                }),
                        )
                        .child(self.task_list(cx)),
                )
            })
    }

    fn prepare_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tags: Vec<_> = self
            .tags
            .iter()
            .filter(|tag| tag.kind == TagKind::Focus)
            .cloned()
            .collect();
        let no_focus_tags = tags.is_empty();
        let disabled = !self.can_start_task(cx);
        let selected = self.selected_tag;
        let tag_name = tags
            .iter()
            .find(|tag| tag.id == selected)
            .map_or("选择标签", |tag| tag.name.as_str())
            .to_owned();
        div()
            .debug_selector(|| "focus-form".into())
            .w_full()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(section_heading(if self.selected_task.is_some() {
                                "任务名称"
                            } else {
                                "新建任务"
                            }))
                            .when(self.task_has_unsaved_changes(cx), |row| {
                                row.child(
                                    div()
                                        .flex()
                                        .gap(px(8.))
                                        .child(
                                            button("reset-task")
                                                .debug_selector(|| "reset-task".into())
                                                .ghost()
                                                .h(px(32.))
                                                .child(control_label("撤销", MUTED))
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.reset_task_changes(window, cx)
                                                })),
                                        )
                                        .child(
                                            button("save-task")
                                                .debug_selector(|| "save-task".into())
                                                .h(px(32.))
                                                .unavailable(!self.can_save_task(cx))
                                                .child(control_label("保存", INK))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.save_task_changes(cx);
                                                })),
                                        ),
                                )
                            }),
                    )
                    .child(
                        gpui::Styled::h(field(&self.task_input), px(44.))
                            .text_size(px(20.))
                            .line_height(px(26.)),
                    )
                    .when_some(self.task_title_error(cx), |form, message| {
                        form.child(
                            div()
                                .debug_selector(|| "task-title-error".into())
                                .text_size(px(14.))
                                .text_color(rgb(DESTRUCTIVE))
                                .child(message),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(field_label("标签"))
                    .when(no_focus_tags, |row| {
                        row.child(
                            button("add-focus-tag")
                                .debug_selector(|| "add-focus-tag".into())
                                .small()
                                .h(px(40.))
                                .icon(control_icon(IconName::Plus, INK))
                                .child(control_label("添加专注标签", INK))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_tag_settings(window, cx)
                                })),
                        )
                    })
                    .when(!no_focus_tags, |row| {
                        row.child(
                            div()
                                .id("task-tag-picker")
                                .w(px(220.))
                                .h(px(40.))
                                .tooltip(move |window, cx| {
                                    Tooltip::new(tag_name.clone()).build(window, cx)
                                })
                                .child(self.tag_picker.clone()),
                        )
                    })
                    .when_some(self.task_tag_error(), |form, message| {
                        form.child(
                            div()
                                .debug_selector(|| "task-tag-error".into())
                                .text_size(px(14.))
                                .text_color(rgb(DESTRUCTIVE))
                                .child(message),
                        )
                    }),
            )
            .child(
                div()
                    .debug_selector(|| "timer-setup".into())
                    .pt(px(16.))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .child(field_label("计时方式"))
                            .child(div().flex().gap(px(4.)).children(
                                [(true, "倒计时"), (false, "正计时")].into_iter().map(
                                    |(countdown, label)| {
                                        let selected = self.countdown == countdown;
                                        choice_button(
                                            if countdown {
                                                "duration-custom"
                                            } else {
                                                "duration-0"
                                            },
                                            selected,
                                            SURFACE,
                                            cx,
                                        )
                                        .debug_selector(move || {
                                            if countdown {
                                                "duration-custom".into()
                                            } else {
                                                "duration-0".into()
                                            }
                                        })
                                        .h(px(36.))
                                        .px(px(16.))
                                        .tooltip(if countdown {
                                            "设定时长，到时自动结束"
                                        } else {
                                            "从零开始累计，手动结束计时"
                                        })
                                        .child(control_label(
                                            label,
                                            if selected { INK } else { MUTED },
                                        ))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                if countdown {
                                                    this.choose_custom_duration(window, cx);
                                                } else {
                                                    this.choose_timer_preset(0, window, cx);
                                                }
                                            }),
                                        )
                                    },
                                ),
                            )),
                    )
                    .child(
                        div()
                            .h(px(44.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .when(self.countdown, |row| {
                                row.child(
                                    div()
                                        .debug_selector(|| "duration-input".into())
                                        .w(px(88.))
                                        .flex_shrink_0()
                                        .child(
                                            gpui::Styled::h(field(&self.timer_input), px(44.))
                                                .text_size(px(20.)),
                                        ),
                                )
                                .child(control_label("分钟", MUTED))
                                .child(div().w(px(8.)).flex_shrink_0())
                                .children(
                                    [25, 45, 60].into_iter().map(|minutes| {
                                        let selected = self
                                            .timer_input
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .parse::<u32>()
                                            .ok()
                                            == Some(minutes);
                                        underline_button(
                                            ("duration", minutes as u64),
                                            selected,
                                            true,
                                            cx,
                                        )
                                        .debug_selector(move || format!("duration-{minutes}"))
                                        .h(px(36.))
                                        .w(px(44.))
                                        .px(px(4.))
                                        .tooltip(format!("倒计时 {minutes} 分钟"))
                                        .child(control_label(
                                            minutes.to_string(),
                                            if selected { INK } else { MUTED },
                                        ))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.choose_timer_preset(minutes, window, cx);
                                            }),
                                        )
                                    }),
                                )
                            })
                            .when(!self.countdown, |row| {
                                row.child(
                                    div().font(timer_font(cx)).text_size(px(28.)).child("00:00"),
                                )
                                .child(control_label("开始累计", MUTED))
                            }),
                    )
                    .when_some(self.timer_duration_error(cx), |body, message| {
                        body.child(
                            div()
                                .debug_selector(|| "timer-duration-error".into())
                                .text_size(px(14.))
                                .text_color(rgb(DESTRUCTIVE))
                                .child(message),
                        )
                    }),
            )
            .child(
                div()
                    .debug_selector(|| "prepare-actions".into())
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        crate::control_style::start_button(
                            "start-focus",
                            if self.browsing_other_timer() {
                                "切换并开始".to_owned()
                            } else {
                                "开始专注".to_owned()
                            },
                            action_icon("play").size(px(14.)),
                            disabled,
                        )
                        .debug_selector(|| "start-focus".into())
                        .tooltip(if self.browsing_other_timer() {
                            "暂停当前专注或结束当前休息，再开始所选任务；已计时间保留"
                        } else {
                            "开始所选任务的本次计时"
                        })
                        .h(px(44.))
                        .w(px(144.))
                        .on_click(cx.listener(|this, _, window, cx| this.start(window, cx))),
                    )
                    .when(self.selected_task.is_none(), |row| {
                        row.child(
                            button("save-new-task")
                                .debug_selector(|| "save-new-task".into())
                                .h(px(44.))
                                .px(px(20.))
                                .unavailable(!self.can_save_task(cx))
                                .child(control_label("保存任务", INK))
                                .tooltip("加入任务列表，稍后再开始计时")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.save_task_changes(cx);
                                })),
                        )
                    }),
            )
            .when_some(
                self.selected_task.filter(|id| {
                    !self
                        .active
                        .iter()
                        .any(|session| session.task_id == Some(*id))
                }),
                |body, _id| {
                    body.child(
                        button("complete-task")
                            .debug_selector(|| "complete-task".into())
                            .ghost()
                            .h(px(36.))
                            .w(px(112.))
                            .child(control_label("完成任务", MUTED))
                            .tooltip("从任务列表移出，保留全部计时记录")
                            .unavailable(self.task_has_unsaved_changes(cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.complete_current_task(window, cx)
                            })),
                    )
                },
            )
    }

    fn task_list(&self, cx: &mut Context<Self>) -> Div {
        let tasks: Vec<_> = self
            .tasks
            .iter()
            .filter(|task| task.completed_at.is_none())
            .collect();
        let standalone: Vec<_> = self
            .active
            .iter()
            .filter(|session| session.task_id.is_none())
            .collect();
        div()
            .debug_selector(|| "focus-tasks".into())
            .w(px(TASK_SIDEBAR_WIDTH))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .debug_selector(|| "task-list-heading".into())
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .child(section_heading("任务"))
                            .child(
                                div()
                                    .debug_selector(|| "task-count".into())
                                    .text_size(px(13.))
                                    .text_color(rgb(MUTED))
                                    .child((tasks.len() + standalone.len()).to_string()),
                            ),
                    )
                    .when(!self.is_new_task_form(), |heading| {
                        heading.child(
                            button("new-task")
                                .debug_selector(|| "new-task".into())
                                .ghost()
                                .h(px(32.))
                                .px(px(8.))
                                .icon(control_icon(Icon::new(IconName::Plus).size(px(14.)), INK))
                                .child(control_label("新建", INK))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.new_task(window, cx)),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .id("task-list-scroll")
                    .debug_selector(|| "task-list-scroll".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .when(tasks.is_empty() && standalone.is_empty(), |list| {
                        list.child(
                            div()
                                .py(px(16.))
                                .text_size(px(14.))
                                .text_color(rgb(MUTED))
                                .child("还没有任务"),
                        )
                    })
                    .children(tasks.into_iter().map(|task| {
                        let id = task.id;
                        let selected = self.selected_task == Some(id);
                        let active = self
                            .active
                            .iter()
                            .find(|session| session.task_id == Some(id));
                        let running = active.is_some_and(Session::is_running);
                        let status = active.map_or_else(
                            || {
                                if task.total_seconds == 0 {
                                    "未开始".to_owned()
                                } else {
                                    format!("累计 {}", short_time(task.total_seconds))
                                }
                            },
                            |session| {
                                format!(
                                    "{} · 本次 {}",
                                    if session.is_running() {
                                        "计时中"
                                    } else {
                                        "已暂停"
                                    },
                                    short_time(session.elapsed_at(self.now))
                                )
                            },
                        );
                        choice_button(("task-row", id as u64), selected, SURFACE, cx)
                            .debug_selector(move || format!("task-row-{id}"))
                            .w_full()
                            .h(px(64.))
                            .px(px(12.))
                            .rounded(px(0.))
                            .border_0()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .justify_start()
                            .flex_shrink_0()
                            .tooltip(task.title.clone())
                            .child(
                                div()
                                    .debug_selector(move || format!("task-column-{id}"))
                                    .w(px(TASK_TEXT_WIDTH))
                                    .relative()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .when(selected, |column| {
                                        column.child(
                                            div()
                                                .absolute()
                                                .left(px(-12.))
                                                .top(px(4.))
                                                .bottom(px(4.))
                                                .w(px(2.))
                                                .bg(rgb(INK)),
                                        )
                                    })
                                    .child(
                                        single_line(task.title.clone(), 14.)
                                            .debug_selector(move || format!("task-title-{id}"))
                                            .font_weight(if selected {
                                                FontWeight::MEDIUM
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .text_color(rgb(INK)),
                                    )
                                    .child(
                                        single_line(status, 13.)
                                            .debug_selector(move || format!("task-status-{id}"))
                                            .text_color(rgb(if running { INK } else { MUTED }))
                                            .when(running, |line| {
                                                line.font_weight(FontWeight::MEDIUM)
                                            }),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_task(id, window, cx)
                            }))
                    }))
                    .children(standalone.into_iter().map(|session| {
                        let id = session.id;
                        let selected = self.selected_session == Some(id);
                        choice_button(("standalone-task", id as u64), selected, SURFACE, cx)
                            .w_full()
                            .h(px(64.))
                            .px(px(12.))
                            .rounded(px(0.))
                            .border_0()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .justify_start()
                            .flex_shrink_0()
                            .tooltip(session.title.clone())
                            .child(
                                div()
                                    .w(px(TASK_TEXT_WIDTH))
                                    .relative()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .when(selected, |column| {
                                        column.child(
                                            div()
                                                .absolute()
                                                .left(px(-12.))
                                                .top(px(4.))
                                                .bottom(px(4.))
                                                .w(px(2.))
                                                .bg(rgb(INK)),
                                        )
                                    })
                                    .child(
                                        single_line(session.title.clone(), 14.)
                                            .font_weight(if selected {
                                                FontWeight::MEDIUM
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .text_color(rgb(INK)),
                                    )
                                    .child(
                                        single_line(
                                            if session.is_running() {
                                                "计时中"
                                            } else {
                                                "已暂停"
                                            },
                                            13.,
                                        )
                                        .text_color(rgb(if session.is_running() {
                                            INK
                                        } else {
                                            MUTED
                                        }))
                                        .when(session.is_running(), |line| {
                                            line.font_weight(FontWeight::MEDIUM)
                                        }),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_session(id, window, cx)
                            }))
                    })),
            )
    }

    fn running_panel(
        &self,
        session: &Session,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let elapsed = session.elapsed_at(self.now);
        let rest = session.tag_kind == TagKind::Rest;
        let paused = session.is_paused();
        let remaining = session
            .target_seconds
            .unwrap_or(0)
            .saturating_sub(elapsed)
            .max(0);
        let end = Local
            .timestamp_opt(self.now.saturating_add(remaining), 0)
            .single();
        let caption = if session.target_seconds.is_none() {
            "已用时间"
        } else {
            "剩余时间"
        };
        let detail = session.target_seconds.map(|target| {
            if paused {
                format!("已用 {} / 共 {}", short_time(elapsed), short_time(target))
            } else {
                format!(
                    "预计 {} 结束",
                    end.map_or(String::new(), |end| end.format("%H:%M").to_string())
                )
            }
        });
        div()
            .debug_selector(|| "focus-running".into())
            .w_full()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(field_label(if paused {
                        "已暂停"
                    } else if rest {
                        "休息中"
                    } else {
                        "专注中"
                    }))
                    .child(task_heading(&session.title))
                    .when(session.tag_name != session.title, |body| {
                        let name = session.tag_name.clone();
                        body.child(
                            div()
                                .id("focus-session-tag")
                                .tooltip(move |window, cx| {
                                    Tooltip::new(name.clone()).build(window, cx)
                                })
                                .child(
                                    single_line(session.tag_name.clone(), 13.)
                                        .text_color(rgb(MUTED)),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .text_size(px(13.))
                            .text_color(rgb(MUTED))
                            .child(caption)
                            .when(!rest && self.editing_duration != Some(session.id), |row| {
                                let id = session.id;
                                row.child(
                                    button("edit-duration")
                                        .debug_selector(|| "edit-duration".into())
                                        .ghost()
                                        .h(px(24.))
                                        .px(px(8.))
                                        .flex_shrink_0()
                                        .child(control_label("调整时长", MUTED))
                                        .tooltip("调整本次计时的总时长，保留已用时间")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.begin_duration_edit(id, window, cx)
                                        })),
                                )
                            }),
                    )
                    .child(timer_readout(
                        if session.target_seconds.is_some() {
                            remaining
                        } else {
                            elapsed
                        },
                        compact,
                        cx,
                    ))
                    .when_some(detail, |body, detail| body.child(field_label(&detail))),
            )
            .when(!rest && self.editing_duration == Some(session.id), |body| {
                body.child(self.session_duration_control(session, cx))
            })
            .when_some(
                session.target_seconds.filter(|target| *target > 0),
                |body, target| body.child(countdown_progress(elapsed, target)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .flex_wrap()
                    .when(rest, |row| {
                        row.child(
                            primary_button("return-from-rest")
                                .debug_selector(|| "return-from-rest".into())
                                .w(px(144.))
                                .h(px(44.))
                                .child(control_label(
                                    if self.return_focus_task().is_some() {
                                        "继续专注"
                                    } else {
                                        "结束休息"
                                    },
                                    0xffffff,
                                ))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.return_to_task(window, cx)
                                })),
                        )
                    })
                    .child(
                        (if rest {
                            button("running-pause")
                        } else {
                            primary_button("running-pause")
                        })
                        .debug_selector(|| "running-pause".into())
                        .w(px(144.))
                        .h(px(44.))
                        .icon(control_icon(
                            action_icon(if paused { "play" } else { "pause" }).size(px(14.)),
                            if rest { INK } else { 0xffffff },
                        ))
                        .child(control_label(
                            if paused {
                                if self.browsing_other_timer() {
                                    "切换并继续"
                                } else {
                                    "继续"
                                }
                            } else {
                                "暂停"
                            },
                            if rest { INK } else { 0xffffff },
                        ))
                        .on_click({
                            let id = session.id;
                            cx.listener(move |this, event: &ClickEvent, _, cx| {
                                if event.click_count() == 1 {
                                    this.toggle_pause(id, cx);
                                }
                            })
                        }),
                    )
                    .when(!rest, |row| {
                        row.child(
                            button("running-finish")
                                .debug_selector(|| "running-finish".into())
                                .w(px(116.))
                                .h(px(44.))
                                .child(control_label("结束计时", INK))
                                .tooltip("保存本次计时；任务保留，可再次开始计时")
                                .on_click({
                                    let id = session.id;
                                    cx.listener(move |this, _, _, cx| this.finish(id, cx))
                                }),
                        )
                    }),
            )
            .when(!rest, |body| {
                body.child(
                    button("running-rest")
                        .debug_selector(|| "running-rest".into())
                        .w(px(124.))
                        .ghost()
                        .h(px(36.))
                        .px(px(12.))
                        .child(control_label("休息 5 分钟", MUTED))
                        .on_click(cx.listener(|this, _, window, cx| this.start_break(window, cx))),
                )
            })
    }

    fn session_duration_control(&self, session: &Session, cx: &mut Context<Self>) -> Div {
        let id = session.id;
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .when(self.editing_duration == Some(id), |body| {
                body.child(
                    div()
                        .debug_selector(|| "session-duration-editor".into())
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .child(control_label("总时长", INK))
                        .child(
                            div()
                                .debug_selector(|| "session-duration-input".into())
                                .w(px(88.))
                                .child(gpui::Styled::h(field(&self.duration_input), px(40.))),
                        )
                        .child(control_label("分钟", MUTED))
                        .child(
                            button("save-duration")
                                .debug_selector(|| "save-duration".into())
                                .h(px(40.))
                                .child(control_label("保存", INK))
                                .unavailable(!self.can_save_duration(cx))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.save_duration(window, cx)
                                })),
                        )
                        .child(
                            button("cancel-duration")
                                .debug_selector(|| "cancel-duration".into())
                                .ghost()
                                .h(px(40.))
                                .child(control_label("取消", MUTED))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_duration_edit(window, cx)
                                })),
                        ),
                )
                .when_some(self.duration_error(cx), |body, message| {
                    body.child(
                        div()
                            .debug_selector(|| "duration-error".into())
                            .text_size(px(14.))
                            .text_color(rgb(DESTRUCTIVE))
                            .child(message),
                    )
                })
            })
    }

    fn completed_panel(
        &self,
        session: &Session,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rest = session.tag_kind == TagKind::Rest;
        let return_task = self.return_focus_task();
        div()
            .debug_selector(|| "focus-completed".into())
            .w_full()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_color(rgb(MUTED))
                            .child(Icon::new(IconName::Check).size(px(14.)))
                            .child(field_label(if rest { "休息结束" } else { "已保存" })),
                    )
                    .child(task_heading(&session.title))
                    .when(session.tag_name != session.title, |body| {
                        let name = session.tag_name.clone();
                        body.child(
                            div()
                                .id("focus-session-tag")
                                .tooltip(move |window, cx| {
                                    Tooltip::new(name.clone()).build(window, cx)
                                })
                                .child(
                                    single_line(session.tag_name.clone(), 13.)
                                        .text_color(rgb(MUTED)),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(timer_readout(session.elapsed_at(self.now), compact, cx))
                    .child(field_label(if rest {
                        "休息时长"
                    } else {
                        "专注时长"
                    })),
            )
            .when_some(
                session.target_seconds.filter(|target| *target > 0),
                |body, target| body.child(countdown_progress(session.elapsed_at(self.now), target)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        primary_button("continue-task")
                            .debug_selector(|| "continue-task".into())
                            .w(px(144.))
                            .h(px(44.))
                            .rounded(px(6.))
                            .child(control_label(
                                if rest {
                                    if return_task.is_some() {
                                        "继续专注"
                                    } else {
                                        "新建任务"
                                    }
                                } else {
                                    "再计一次"
                                },
                                0xffffff,
                            ))
                            .tooltip(if rest {
                                return_task.map_or_else(
                                    || "新建一个专注任务".to_owned(),
                                    |task| format!("继续：{}", task.title),
                                )
                            } else {
                                "为同一任务开始一次新的计时，原记录保留".to_owned()
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if rest {
                                    this.return_to_task(window, cx);
                                } else {
                                    this.repeat_completed(window, cx);
                                }
                            })),
                    )
                    .when(!rest, |row| {
                        row.child(
                            button("rest-five")
                                .debug_selector(|| "rest-five".into())
                                .w(px(124.))
                                .h(px(44.))
                                .rounded(px(6.))
                                .child(control_label("休息 5 分钟", INK))
                                .tooltip("暂停专注并开始休息倒计时；已有专注时间保留")
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.start_break(window, cx)),
                                ),
                        )
                    }),
            )
            .when(!rest && session.task_id.is_some(), |body| {
                body.child(
                    button("complete-task")
                        .debug_selector(|| "complete-task".into())
                        .w(px(112.))
                        .ghost()
                        .h(px(36.))
                        .child(control_label("完成任务", MUTED))
                        .tooltip("从任务列表移出，保留全部计时记录")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.complete_current_task(window, cx)
                        })),
                )
            })
    }
}

/// Fit the complete time to its final column width; hours are never truncated.
fn timer_readout(seconds: i64, compact: bool, cx: &App) -> Div {
    let value: SharedString = clock(seconds).into();
    let font_size = if compact { 80. } else { 104. };
    let line_height = px(font_size * 1.1);
    div()
        .debug_selector(|| "focus-timer".into())
        .w_full()
        .min_w_0()
        .h(line_height)
        .font(timer_font(cx))
        .font_weight(FontWeight::MEDIUM)
        .text_size(px(font_size))
        .line_height(relative(1.1))
        .child(
            canvas(
                move |bounds, window, _| {
                    let style = window.text_style();
                    let font_size = style.font_size.to_pixels(window.rem_size());
                    shape_timer(
                        value,
                        bounds.size.width,
                        &style,
                        font_size,
                        window.text_system(),
                    )
                },
                move |bounds, line, window, cx| {
                    let _ = line.paint(bounds.origin, line_height, window, cx);
                },
            )
            .size_full(),
        )
}

/// Measure the full time before scaling, keeping every digit at any width.
pub(crate) fn shape_timer(
    value: SharedString,
    width: Pixels,
    style: &TextStyle,
    font_size: Pixels,
    text_system: &WindowTextSystem,
) -> ShapedLine {
    // Never reuse truncated runs from an earlier narrow layout. Both passes
    // start with the complete UTF-8 byte coverage of the original value.
    let shape = |font_size| {
        text_system.shape_line(value.clone(), font_size, &[style.to_run(value.len())], None)
    };
    let line = shape(font_size);
    let width = width.max(px(1.));
    if line.width > width {
        shape(font_size * (f32::from(width) / f32::from(line.width)) * 0.995)
    } else {
        line
    }
}

/// A ruler for actual countdown progress, including rounds ended early.
fn countdown_progress(elapsed: i64, target: i64) -> Div {
    let fraction = (elapsed.max(0) as f64 / target.max(1) as f64).clamp(0., 1.) as f32;
    div()
        .debug_selector(|| "countdown-progress".into())
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .debug_selector(|| "countdown-progress-track".into())
                .relative()
                .mx(px(1.))
                .h(px(14.))
                .flex()
                .items_center()
                .justify_between()
                .children((0..=25).map(|index| {
                    div()
                        .debug_selector(move || format!("countdown-tick-{index}"))
                        .w(px(1.))
                        .h(px(if index % 5 == 0 { 10. } else { 6. }))
                        .flex_shrink_0()
                        .bg(rgb(if index as f32 / 25. <= fraction {
                            INK
                        } else {
                            LINE
                        }))
                }))
                .child(
                    div()
                        .debug_selector(|| "countdown-progress-current".into())
                        .absolute()
                        .left(relative(fraction))
                        .ml(px(-1.))
                        .top_0()
                        .w(px(2.))
                        .h_full()
                        .bg(rgb(INK)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(13.))
                .text_color(rgb(MUTED))
                .child("0")
                .child(
                    div()
                        .debug_selector(|| "countdown-progress-goal".into())
                        .child(short_time(target)),
                ),
        )
}

#[cfg(all(test, feature = "ui-test-support"))]
mod tests {
    use super::shape_timer;
    use gpui::{SharedString, TestAppContext, TextStyle, px};

    #[gpui::test]
    fn timer_keeps_every_digit_when_fitting_and_restores_wide_layout(cx: &mut TestAppContext) {
        cx.add_empty_window().update(|window, _| {
            let style = TextStyle::default();
            for value in ["100:05:07", "12345:59:59", "2562047788015215:30:07"] {
                let value: SharedString = value.into();
                for font_size in [20., 80., 104.] {
                    let original = shape_timer(
                        value.clone(),
                        px(2_000.),
                        &style,
                        px(font_size),
                        window.text_system(),
                    );
                    for width in [800., 424., 150., 800., 2_000.] {
                        let line = shape_timer(
                            value.clone(),
                            px(width),
                            &style,
                            px(font_size),
                            window.text_system(),
                        );
                        assert_eq!(line.text, value);
                        assert_eq!(line.len(), value.len());
                        assert!(line.width <= px(width), "{value} overflows {width} pt");
                        assert!(line.font_size <= px(font_size));
                        if width >= f32::from(original.width) {
                            assert_eq!(line.font_size, original.font_size);
                            assert_eq!(line.width, original.width);
                        } else {
                            assert!(line.font_size < original.font_size);
                        }
                    }
                }
            }
            window.remove_window();
        });
    }
}
