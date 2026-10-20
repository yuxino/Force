use chrono::{Days, Local, TimeZone};
use force_core::{Session, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::Input,
    scroll::ScrollableElement,
    tooltip::Tooltip,
};

use crate::{
    label::single_line,
    model::{FloatingStrip, ForceView, Page},
    platform,
};

pub(crate) const INK: u32 = 0x202020;
pub(crate) const MUTED: u32 = 0x626262;
pub(crate) const LINE: u32 = 0xe7e7e7;
pub(crate) const PAPER: u32 = 0xfdfdfd;
pub(crate) const SURFACE: u32 = 0xf2f2f2;
pub(crate) const DESTRUCTIVE: u32 = 0x963f3f;

const TASK_SIDEBAR_WIDTH: f32 = 240.;
const TASK_TEXT_WIDTH: f32 = TASK_SIDEBAR_WIDTH - 26.;

impl ForceView {
    fn current_session(&self) -> Option<&Session> {
        self.active
            .iter()
            .find(|session| Some(session.id) == self.selected_session)
            .or_else(|| {
                self.preparing
                    .then(|| {
                        self.active
                            .iter()
                            .find(|session| session.is_running())
                            .or_else(|| self.active.last())
                    })
                    .flatten()
            })
    }

    fn panel_navigation(&self, cx: &mut Context<Self>) -> Div {
        div()
            .debug_selector(|| "manager-header".into())
            .h(px(68.))
            .px(px(40.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(LINE))
            .flex()
            .items_center()
            .gap(px(12.))
            .child(
                Icon::default()
                    .path("force-logo.svg")
                    .size(px(24.))
                    .text_color(rgb(INK)),
            )
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Force"),
            )
            .child(div().flex_1())
            .children(
                [
                    ("focus-tab", "专注", Page::Focus),
                    ("records-tab", "记录", Page::Records),
                ]
                .into_iter()
                .map(|(id, label, page)| {
                    button(id)
                        .debug_selector(move || id.into())
                        .ghost()
                        .h(px(36.))
                        .px(px(18.))
                        .border_0()
                        .border_b_2()
                        .border_color(if self.page == page {
                            rgb(INK)
                        } else {
                            rgba(0x00000000)
                        })
                        .rounded(px(0.))
                        .when(self.page == page, |button| {
                            button.font_weight(FontWeight::SEMIBOLD)
                        })
                        .child(
                            div()
                                .text_size(px(14.))
                                .text_color(rgb(if self.page == page { INK } else { MUTED }))
                                .child(label),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.page != page {
                                this.navigate(page, window, cx);
                            }
                        }))
                }),
            )
            .child(div().w(px(1.)).h(px(20.)).mx(px(10.)).bg(rgb(LINE)))
            .child(
                choice_button("settings-tab", self.page == Page::Settings, SURFACE, cx)
                    .debug_selector(|| "settings-tab".into())
                    .size(px(36.))
                    .icon(control_icon(
                        Icon::new(IconName::Settings).size(px(18.)),
                        MUTED,
                    ))
                    .tooltip("设置")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate(Page::Settings, window, cx);
                    })),
            )
    }

    fn today_overview(&self, cx: &App) -> Div {
        let fraction =
            (self.today.focus_seconds as f32 / (self.goal.max(1) as f32 * 60.)).clamp(0., 1.);
        div()
            .debug_selector(|| "focus-summary".into())
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .pb(px(32.))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(16.))
                    .child(
                        div()
                            .text_size(px(28.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("今天"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(MUTED))
                            .child(Local::now().format("%m 月 %d 日").to_string()),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "focus-goal".into())
                    .w(px(TASK_SIDEBAR_WIDTH))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(MUTED))
                                    .child("今日专注"),
                            )
                            .child(div().font(timer_font(cx)).text_size(px(14.)).child(format!(
                                "{} / {}",
                                short_time(self.today.focus_seconds),
                                short_time(i64::from(self.goal) * 60)
                            ))),
                    )
                    .child(
                        div().h(px(3.)).w_full().rounded_full().bg(rgb(LINE)).child(
                            div()
                                .h_full()
                                .w(relative(fraction))
                                .rounded_full()
                                .bg(rgb(INK)),
                        ),
                    ),
            )
    }

    fn storage_notice(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(control_label("重试", INK))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_store(cx);
                        if !this.storage_failed {
                            let goal = this.goal.to_string();
                            this.goal_input
                                .update(cx, |input, cx| input.set_value(goal, window, cx));
                        }
                    })),
            )
    }

    fn focus_panel(&self, cx: &mut Context<Self>) -> Div {
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
            .when_some(self.toast.as_ref(), |body, (message, _)| {
                body.child(
                    div()
                        .mb(px(20.))
                        .p(px(12.))
                        .bg(rgb(SURFACE))
                        .rounded(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(div().flex_1().text_size(px(14.)).child(message.clone()))
                        .child(
                            button("dismiss-focus-feedback")
                                .ghost()
                                .size(px(28.))
                                .icon(control_icon(IconName::Close, MUTED))
                                .tooltip("关闭提示")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.toast = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
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
                                .child(if let Some(session) = &self.completed {
                                    self.completed_panel(session, cx).into_any_element()
                                } else if !self.preparing
                                    && let Some(session) = self.current_session()
                                {
                                    self.running_panel(session, cx).into_any_element()
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
        let task_name = self.task_input.read(cx).value();
        let task_length = task_name.trim().chars().count();
        let task_valid = (1..=200).contains(&task_length);
        let no_focus_tags = tags.is_empty();
        let disabled = no_focus_tags || !task_valid || self.storage_failed;
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
            .gap(px(20.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_heading("这次专注").h(px(32.)).flex().items_center())
                    .child(gpui::Styled::h(field(&self.task_input), px(44.)))
                    .when(task_length > 200, |form| {
                        form.child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(DESTRUCTIVE))
                                .child("任务名称最多 200 字。"),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(field_label("专注时长"))
                    .child(
                        div()
                            .flex()
                            .p(px(3.))
                            .gap(px(2.))
                            .rounded(px(8.))
                            .bg(rgb(SURFACE))
                            .children(
                                [
                                    (25, "25 分钟"),
                                    (45, "45 分钟"),
                                    (60, "60 分钟"),
                                    (0, "自由"),
                                ]
                                .into_iter()
                                .map(|(minutes, label)| {
                                    let selected = if minutes == 0 {
                                        !self.countdown
                                    } else {
                                        self.countdown && self.timer_minutes == minutes
                                    };
                                    choice_button(("duration", minutes as u64), selected, PAPER, cx)
                                        .h(px(36.))
                                        .flex_1()
                                        .min_w_0()
                                        .text_size(px(12.))
                                        .rounded(px(5.))
                                        .child(control_label(
                                            label,
                                            if selected { INK } else { MUTED },
                                        ))
                                        .when(selected, |button| {
                                            button.font_weight(FontWeight::MEDIUM)
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.countdown = minutes != 0;
                                            if minutes != 0 {
                                                this.timer_minutes = minutes;
                                            }
                                            cx.notify();
                                        }))
                                }),
                            ),
                    ),
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
                    }),
            )
            .child(
                crate::control_style::start_button(
                    "start-focus",
                    if self.countdown {
                        format!("开始 {} 分钟", self.timer_minutes)
                    } else {
                        "开始自由计时".to_owned()
                    },
                    action_icon("play").size(px(14.)),
                    disabled,
                )
                .debug_selector(|| "start-focus".into())
                .h(px(44.))
                .w_full()
                .on_click(cx.listener(|this, _, window, cx| this.start(window, cx))),
            )
            .when_some(
                self.selected_task.filter(|id| {
                    !self
                        .active
                        .iter()
                        .any(|session| session.task_id == Some(*id))
                }),
                |body, id| {
                    body.child(
                        button("remove-selected-task")
                            .ghost()
                            .h(px(36.))
                            .child(control_label("移出任务列表", MUTED))
                            .tooltip("保留已经记录的专注时间")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.archive_task(id, window, cx)
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
                    .child(section_heading("任务"))
                    .child(
                        button("new-task")
                            .debug_selector(|| "new-task".into())
                            .ghost()
                            .h(px(32.))
                            .px(px(8.))
                            .icon(control_icon(Icon::new(IconName::Plus).size(px(14.)), INK))
                            .child(control_label("新建", INK))
                            .on_click(cx.listener(|this, _, window, cx| this.new_task(window, cx))),
                    ),
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
                    .gap(px(4.))
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
                        let active = self
                            .active
                            .iter()
                            .find(|session| session.task_id == Some(id));
                        let status = active.map_or_else(
                            || format!("累计 {}", short_time(task.total_seconds)),
                            |session| {
                                format!(
                                    "{} · {}",
                                    if session.is_running() {
                                        "计时中"
                                    } else {
                                        "已暂停"
                                    },
                                    short_time(session.elapsed_at(self.now))
                                )
                            },
                        );
                        choice_button(
                            ("task-row", id as u64),
                            self.selected_task == Some(id),
                            SURFACE,
                            cx,
                        )
                        .debug_selector(move || format!("task-row-{id}"))
                        .w_full()
                        .h(px(64.))
                        .px(px(12.))
                        .justify_start()
                        .flex_shrink_0()
                        .tooltip(task.title.clone())
                        .child(
                            div()
                                .debug_selector(move || format!("task-column-{id}"))
                                .w(px(TASK_TEXT_WIDTH))
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    single_line(task.title.clone(), 14.)
                                        .debug_selector(move || format!("task-title-{id}"))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(INK)),
                                )
                                .child(
                                    single_line(status, 13.)
                                        .debug_selector(move || format!("task-status-{id}"))
                                        .text_color(rgb(MUTED)),
                                ),
                        )
                        .on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.select_task(id, window, cx)
                            }),
                        )
                    }))
                    .children(standalone.into_iter().map(|session| {
                        let id = session.id;
                        choice_button(
                            ("standalone-task", id as u64),
                            self.selected_session == Some(id),
                            SURFACE,
                            cx,
                        )
                        .w_full()
                        .h(px(64.))
                        .px(px(12.))
                        .justify_start()
                        .flex_shrink_0()
                        .tooltip(session.title.clone())
                        .child(
                            div()
                                .w(px(TASK_TEXT_WIDTH))
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    single_line(session.title.clone(), 14.)
                                        .font_weight(FontWeight::MEDIUM)
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
                                    .text_color(rgb(MUTED)),
                                ),
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.select_session(id, window, cx),
                        ))
                    })),
            )
    }

    fn running_panel(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
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
            "自由计时".to_owned()
        } else if paused {
            format!(
                "已{} {}",
                if rest { "休息" } else { "专注" },
                short_time(elapsed)
            )
        } else {
            format!(
                "预计 {} 结束",
                end.map_or(String::new(), |end| end.format("%H:%M").to_string())
            )
        };
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
                    .gap(px(12.))
                    .child(field_label(if paused {
                        "已暂停"
                    } else if rest {
                        "休息中"
                    } else {
                        "正在专注"
                    }))
                    .child(task_heading(&session.title))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(MUTED))
                            .child(session.tag_name.clone()),
                    ),
            )
            .child(
                div()
                    .py(px(20.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(16.))
                    .child(
                        div()
                            .font(timer_font(cx))
                            .text_size(px(72.))
                            .line_height(relative(1.1))
                            .font_weight(FontWeight::MEDIUM)
                            .child(clock(if session.target_seconds.is_some() {
                                remaining
                            } else {
                                elapsed
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(MUTED))
                            .child(caption),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        primary_button("running-pause")
                            .debug_selector(|| "running-pause".into())
                            .flex_1()
                            .h(px(44.))
                            .icon(control_icon(
                                action_icon(if paused { "play" } else { "pause" }).size(px(14.)),
                                0xffffff,
                            ))
                            .child(control_label(
                                if paused {
                                    "继续计时"
                                } else {
                                    "暂停计时"
                                },
                                0xffffff,
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
                    .child(
                        button("running-finish")
                            .debug_selector(|| "running-finish".into())
                            .flex_1()
                            .h(px(44.))
                            .child(control_label("结束本段", INK))
                            .on_click({
                                let id = session.id;
                                cx.listener(move |this, _, _, cx| this.finish(id, cx))
                            }),
                    ),
            )
            .child(
                button("running-rest")
                    .ghost()
                    .h(px(36.))
                    .child(control_label(
                        if rest {
                            "回到专注"
                        } else {
                            "休息 5 分钟"
                        },
                        MUTED,
                    ))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if rest {
                            this.return_to_task(window, cx);
                        } else {
                            this.start_break(window, cx);
                        }
                    })),
            )
    }

    fn completed_panel(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let rest = session.tag_kind == TagKind::Rest;
        let return_task = self.return_task.and_then(|id| {
            self.tasks
                .iter()
                .find(|task| task.id == id && task.completed_at.is_none())
        });
        let next_title = if rest {
            return_task.map_or("下一段专注", |task| task.title.as_str())
        } else {
            session.title.as_str()
        };
        div()
            .debug_selector(|| "focus-completed".into())
            .w_full()
            .py(px(8.))
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(Icon::new(IconName::CircleCheck).size(px(15.)))
                    .child(if rest {
                        "休息结束，可以继续了"
                    } else {
                        "本段已记录"
                    }),
            )
            .child(task_heading(next_title))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .child(
                        div()
                            .font(timer_font(cx))
                            .text_size(px(40.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(clock(session.elapsed_at(self.now))),
                    )
                    .child(field_label(if rest {
                        "休息时长"
                    } else {
                        "专注时长"
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        primary_button("continue-task")
                            .flex_1()
                            .h(px(44.))
                            .rounded(px(6.))
                            .child(control_label(
                                if rest {
                                    if return_task.is_some() {
                                        "回到专注"
                                    } else {
                                        "开始专注"
                                    }
                                } else {
                                    "继续这件事"
                                },
                                0xffffff,
                            ))
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
                                .flex_1()
                                .h(px(44.))
                                .rounded(px(7.))
                                .child(control_label("休息 5 分钟", INK))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.start_break(window, cx)),
                                ),
                        )
                    }),
            )
            .child(div().flex().items_center().justify_between().when(
                !rest && session.task_id.is_some(),
                |row| {
                    row.child(
                        button("complete-task")
                            .ghost()
                            .small()
                            .icon(control_icon(Icon::new(IconName::Check).size(px(14.)), INK))
                            .child(control_label("完成任务", INK))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.complete_current_task(window, cx)
                            })),
                    )
                },
            ))
    }

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
                    .ghost()
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
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.record_date.format("%Y 年 %m 月 %d 日").to_string()),
            )
            .child(
                button("next-day")
                    .debug_selector(|| "next-day".into())
                    .ghost()
                    .size(px(32.))
                    .icon(control_icon(IconName::ChevronRight, INK))
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
                        .ghost()
                        .h(px(32.))
                        .px(px(10.))
                        .child(control_label("今天", INK))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.select_record_date(Local::now().date_naive(), cx)
                        })),
                )
            })
    }

    fn records_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.record_search.read(cx).value().trim().to_lowercase();
        let records: Vec<_> = self
            .history
            .iter()
            .filter(|session| {
                query.is_empty()
                    || session.title.to_lowercase().contains(&query)
                    || session.tag_name.to_lowercase().contains(&query)
            })
            .collect();
        div().debug_selector(|| "records-content".into()).w_full()
            .flex().flex_col().gap(px(20.))
            .child(self.activity_calendar(cx))
            .child(div().pt(px(24.)).border_t_1().border_color(rgb(LINE))
                .flex().items_center().gap(px(20.))
                .child(self.record_date_controls(cx))
                .child(div().flex_1())
                .child(div().w(px(240.)).flex_shrink_0()
                    .child(field(&self.record_search)
                        .prefix(Icon::new(IconName::Search).size(px(14.)).text_color(rgb(MUTED)))
                        .when(!query.is_empty(), |input| input.suffix(
                            button("clear-record-search").ghost().size(px(24.))
                                .icon(control_icon(Icon::new(IconName::Close).size(px(13.)), MUTED))
                                .tooltip("清空搜索")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.record_search.update(cx, |input, cx| {
                                        input.set_value("", window, cx);
                                        input.focus(window, cx);
                                    });
                                }))
                        )))))
            .child(div().flex().items_baseline().gap(px(12.))
                .child(field_label("当日专注"))
                .child(div().debug_selector(|| "record-total".into())
                    .font(timer_font(cx)).text_size(px(24.)).font_weight(FontWeight::MEDIUM)
                    .child(short_time(self.record_stats.focus_seconds)))
                .child(div().flex_1())
                .child(div().text_size(px(13.)).text_color(rgb(MUTED))
                    .child(if query.is_empty() { format!("{} 条记录", self.history.len()) }
                        else { format!("匹配 {} 条", records.len()) }))
                .child(choice_button("record-help", self.record_help_open, SURFACE, cx).size(px(28.))
                    .icon(control_icon(Icon::new(IconName::Info).size(px(14.)), MUTED))
                    .tooltip("统计说明")
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        if event.click_count() == 1 { this.record_help_open = !this.record_help_open; cx.notify(); }
                    }))))
            .when(self.record_help_open, |body| body.child(
                div().p(px(12.)).rounded(px(6.)).bg(rgb(SURFACE)).text_size(px(13.)).text_color(rgb(MUTED))
                    .child("活动图按当天实际专注时间计算，包含进行中的专注，重叠时间只计算一次。记录按结束日期归档；搜索只筛选列表。")
            ))
            .child(div().flex().flex_col()
                .children(records.iter().map(|session| self.record_row(session, cx)))
                .when(records.is_empty(), |body| body.child(
                    div().py(px(32.)).text_size(px(14.)).text_color(rgb(MUTED))
                        .child(if query.is_empty() { "这一天还没有已结束的计时" } else { "没有匹配的记录" })
                )))
            .child(div().flex().justify_end().child(
                button("export-records").ghost().h(px(36.))
                    .icon(control_icon(Icon::new(IconName::ArrowDown).size(px(14.)), MUTED))
                    .child(control_label("导出全部记录", MUTED))
                    .unavailable(self.export_busy).loading(self.export_busy)
                    .on_click(cx.listener(|this, _, _, cx| this.export(cx)))
            ))
    }

    fn record_row(&self, session: &Session, cx: &mut Context<Self>) -> Div {
        let id = session.id;
        let expanded = self.record_details == Some(id);
        let help_title = session.title.clone();
        div()
            .debug_selector(move || format!("record-row-{id}"))
            .py(px(12.))
            .border_b_1()
            .border_color(rgb(LINE))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        choice_button(("record-details", id as u64), expanded, SURFACE, cx)
                            .size(px(28.))
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
                            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                if event.click_count() == 1 {
                                    this.record_details = if this.record_details == Some(id) {
                                        None
                                    } else {
                                        Some(id)
                                    };
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                single_line(session.title.clone(), 14.)
                                    .id(("record-title", id as u64))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(help_title.clone()).build(window, cx)
                                    }),
                            )
                            .child(
                                single_line(
                                    format!("{} · {}", session_times(session), session.tag_name),
                                    12.,
                                )
                                .mt_1()
                                .text_color(rgb(MUTED)),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap_1()
                            .child(
                                div()
                                    .font(timer_font(cx))
                                    .text_size(px(14.))
                                    .child(clock(session.elapsed_at(self.now))),
                            )
                            .when(session.tag_kind == TagKind::Rest, |body| {
                                body.child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(MUTED))
                                        .child("休息"),
                                )
                            }),
                    )
                    .child(
                        destructive_button(("delete-record", id as u64))
                            .ghost()
                            .size(px(28.))
                            .icon(control_icon(
                                Icon::default().path("force-icons/trash.svg").size(px(14.)),
                                MUTED,
                            ))
                            .tooltip("删除记录")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.delete_record(id, window, cx)
                            })),
                    ),
            )
            .when(expanded, |row| {
                row.child(
                    div()
                        .pt(px(12.))
                        .pb(px(4.))
                        .pl(px(34.))
                        .pr(px(6.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(record_detail("任务", session.title.clone()))
                        .child(record_detail(
                            "标签",
                            format!(
                                "{} · {}",
                                session.tag_name,
                                if session.tag_kind == TagKind::Focus {
                                    "专注"
                                } else {
                                    "休息"
                                }
                            ),
                        ))
                        .child(record_detail("开始", record_timestamp(session.started_at)))
                        .child(record_detail(
                            "结束",
                            session
                                .finished_at
                                .map_or_else(|| "尚未结束".to_owned(), record_timestamp),
                        ))
                        .child(record_detail(
                            "方式",
                            session.target_seconds.map_or_else(
                                || "自由计时".to_owned(),
                                |seconds| format!("{}倒计时", short_time(seconds)),
                            ),
                        )),
                )
            })
    }

    fn management_page(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.page == Page::Focus {
            return self.focus_panel(cx).into_any_element();
        }
        let scroll = &self.panel_scroll[match self.page {
            Page::Focus => 0,
            Page::Records => 1,
            Page::Settings => 2,
        }];
        div()
            .id("manager-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(scroll)
            .relative()
            .child(
                div()
                    .debug_selector(|| "manager-content".into())
                    .w_full()
                    .max_w(px(976.))
                    .mx_auto()
                    .px(px(40.))
                    .pt(px(36.))
                    .pb(px(48.))
                    .child(page_heading(
                        match self.page {
                            Page::Focus => "专注",
                            Page::Records => "记录",
                            Page::Settings => "设置",
                        },
                        (self.page == Page::Focus)
                            .then(|| Local::now().format("%m 月 %d 日").to_string()),
                    ))
                    .when_some(self.toast.as_ref(), |body, (message, _)| {
                        body.child(
                            div()
                                .mb(px(24.))
                                .p(px(12.))
                                .rounded(px(8.))
                                .bg(rgb(SURFACE))
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .child(div().flex_1().text_size(px(13.)).child(message.clone()))
                                .child(
                                    button("dismiss-feedback")
                                        .ghost()
                                        .size(px(28.))
                                        .icon(control_icon(IconName::Close, MUTED))
                                        .tooltip("关闭提示")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toast = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    .when(self.storage_failed, |body| {
                        body.child(self.storage_notice(cx))
                    })
                    .when(!self.storage_failed, |body| {
                        body.child(match self.page {
                            Page::Focus => self.focus_panel(cx).into_any_element(),
                            Page::Records => self.records_panel(cx).into_any_element(),
                            Page::Settings => self.settings_panel(cx).into_any_element(),
                        })
                    }),
            )
            .vertical_scrollbar(scroll)
            .into_any_element()
    }
}

impl Render for ForceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_tag_picker(window, cx);
        div()
            .id("force-manager")
            .track_focus(&self.focus_handle)
            .key_context("Force")
            .size_full()
            .bg(rgb(PAPER))
            .text_color(rgb(INK))
            .text_size(px(14.))
            .flex()
            .flex_col()
            .child(self.panel_navigation(cx))
            .child(self.management_page(cx))
    }
}

impl FloatingStrip {
    fn floating_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let current = if model.completed.is_some() && !model.active.iter().any(Session::is_running)
        {
            None
        } else {
            model.strip_session()
        };
        let completed = model.completed.as_ref();
        let seconds = current.map_or_else(
            || {
                completed.map_or(
                    if model.countdown {
                        i64::from(model.timer_minutes) * 60
                    } else {
                        0
                    },
                    |s| {
                        if s.target_seconds
                            .is_some_and(|target| s.elapsed_at(model.now) >= target)
                        {
                            0
                        } else {
                            s.elapsed_at(model.now)
                        }
                    },
                )
            },
            |s| {
                s.target_seconds.map_or_else(
                    || s.elapsed_at(model.now),
                    |target| (target - s.elapsed_at(model.now)).max(0),
                )
            },
        );
        let title = if model.storage_failed {
            "本地数据不可用".to_owned()
        } else if let Some(session) = current {
            session.title.clone()
        } else if let Some(session) = completed {
            session.title.clone()
        } else {
            let input = model.task_input.read(cx).value();
            if input.trim().is_empty() {
                "准备专注".to_owned()
            } else {
                input.to_string()
            }
        };
        let state = if model.storage_failed {
            "展开重试"
        } else if let Some(session) = current {
            if session.is_paused() {
                "已暂停"
            } else if session.tag_kind == TagKind::Rest {
                "休息中"
            } else {
                "专注中"
            }
        } else if completed.is_some_and(|s| s.tag_kind == TagKind::Rest) {
            "休息结束"
        } else if completed.is_some() {
            "本段结束"
        } else {
            "未开始"
        };
        let running = current.is_some_and(Session::is_running);
        let (symbol, help) = if model.storage_failed {
            ("play", "展开并重试")
        } else if completed.is_some() && current.is_none() {
            ("play", "查看本轮记录与下一步")
        } else if running {
            ("pause", "暂停")
        } else if current.is_some() {
            ("play", "继续")
        } else {
            ("play", "开始专注")
        };
        let state = model
            .toast
            .as_ref()
            .filter(|_| model.compact)
            .map_or(state.to_owned(), |(message, _)| message.clone());
        let title_help = title.clone();
        let state_help = state.clone();
        div()
            .h(px(platform::STRIP_HEIGHT - 8.))
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(12.))
            .gap(px(3.))
            .child(
                div()
                    .id("floating-drag-area")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .cursor_move()
                    .tooltip(move |window, cx| {
                        Tooltip::new(format!("{title_help} · 拖动以移动浮窗")).build(window, cx)
                    })
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        let _ = platform::start_window_drag(window);
                    })
                    .child(
                        div()
                            .flex_shrink_0()
                            .font(timer_font(cx))
                            .text_size(px(if seconds >= 3600 { 23. } else { 27. }))
                            .line_height(relative(1.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(if current.is_some_and(Session::is_paused) {
                                MUTED
                            } else {
                                INK
                            }))
                            .child(clock(seconds)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(single_line(title, 12.).font_weight(FontWeight::MEDIUM))
                            .child(
                                single_line(state, 11.)
                                    .id("bar-status")
                                    .text_color(rgb(if model.storage_failed {
                                        0x963f3f
                                    } else {
                                        MUTED
                                    }))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(state_help.clone()).build(window, cx)
                                    }),
                            ),
                    ),
            )
            .child(
                primary_button("bar-primary")
                    .debug_selector(|| "strip-primary".into())
                    .size(px(30.))
                    .rounded_full()
                    .icon(control_icon(
                        if current.is_none() && completed.is_some() {
                            Icon::new(IconName::ArrowRight).size(px(15.))
                        } else {
                            action_icon(symbol).size(px(15.))
                        },
                        0xffffff,
                    ))
                    .tooltip(shortcut(help, "P", "P"))
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        if event.click_count() > 1 {
                            return;
                        }
                        this.dispatch(cx, |model, window, cx| {
                            model.strip_primary_action(window, cx)
                        });
                    })),
            )
            .children(current.map(|session| {
                let id = session.id;
                button("bar-finish")
                    .ghost()
                    .size(px(28.))
                    .icon(control_icon(action_icon("stop").size(px(14.)), INK))
                    .tooltip(shortcut("结束本段", "⇧P", "Shift P"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dispatch(cx, move |model, _, cx| model.finish(id, cx))
                    }))
            }))
            .child(
                button("bar-expand")
                    .debug_selector(|| "strip-open".into())
                    .ghost()
                    .size(px(28.))
                    .text_color(rgb(MUTED))
                    .icon(control_icon(
                        Icon::new(IconName::ExternalLink).size(px(14.)),
                        INK,
                    ))
                    .tooltip("打开 Force")
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        if event.click_count() == 1 {
                            this.dispatch(cx, |model, window, cx| model.show_manager(window, cx));
                        }
                    })),
            )
    }
}

impl Render for FloatingStrip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let progress = model
            .strip_session()
            .filter(|session| model.completed.is_none() || session.is_running())
            .and_then(|session| {
                session.target_seconds.map(|target| {
                    (session.elapsed_at(model.now) as f32 / target as f32).clamp(0., 1.)
                })
            });
        div()
            .debug_selector(|| "strip-root".into())
            .track_focus(&self.focus_handle)
            .key_context("Force")
            .size_full()
            .p(px(4.))
            .text_color(rgb(INK))
            .child(
                div()
                    .size_full()
                    .relative()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(rgb(0xd9d9d9))
                    .bg(rgb(PAPER))
                    .shadow(vec![BoxShadow {
                        color: rgba(0x0000000d).into(),
                        offset: point(px(0.), px(1.)),
                        blur_radius: px(4.),
                        spread_radius: px(0.),
                    }])
                    .overflow_hidden()
                    .child(self.floating_bar(cx))
                    .children(progress.map(|fraction| {
                        div()
                            .absolute()
                            .bottom_0()
                            .left_3()
                            .right_3()
                            .h(px(2.))
                            .bg(rgb(LINE))
                            .child(div().h_full().w(relative(fraction)).bg(rgb(MUTED)))
                    })),
            )
    }
}

pub(crate) fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id)
        .text_size(px(14.))
        .rounded(px(8.))
        .cursor_pointer()
}

// Custom variants retain hover/pressed/focus feedback even for the current
// choice. Button::selected suppresses hover and pressed styles in 0.5.1.
pub(crate) fn choice_button(
    id: impl Into<ElementId>,
    selected: bool,
    selected_surface: u32,
    cx: &App,
) -> Button {
    button(id).custom(
        ButtonCustomVariant::new(cx)
            .color(if selected {
                rgb(selected_surface).into()
            } else {
                rgba(0x00000000).into()
            })
            .foreground(rgb(if selected { INK } else { MUTED }).into())
            .border(if selected {
                rgb(LINE).into()
            } else {
                rgba(0x00000000).into()
            })
            .hover(
                rgb(if selected && selected_surface == PAPER {
                    0xf7f7f7
                } else {
                    0xe9e9e9
                })
                .into(),
            )
            .active(rgb(0xdcdcdc).into()),
    )
}

pub(crate) fn primary_button(id: impl Into<ElementId>) -> Button {
    button(id).primary().font_weight(FontWeight::MEDIUM)
}

pub(crate) fn destructive_button(id: impl Into<ElementId>) -> Button {
    button(id).text_color(rgb(MUTED))
}

pub(crate) fn field(state: &Entity<gpui_component::input::InputState>) -> Input {
    // Input::h only affects multiline editors; Styled::h sets the single-line
    // control height too. Use one geometry across task, search and settings.
    Styled::h(Input::new(state), px(40.))
        .text_size(px(14.))
        .line_height(px(20.))
        .px(px(9.))
        .py(px(5.))
        .bg(rgb(0xffffff))
        .rounded(px(8.))
}

// gpui-component 0.5.1 hardcodes a red hover foreground. Keep each label and
// icon explicitly colored; registering another hover handler would panic.
pub(crate) fn control_label(value: impl Into<SharedString>, foreground: u32) -> Div {
    div()
        .text_size(px(14.))
        .line_height(relative(1.2))
        .text_color(rgb(foreground))
        .child(value.into())
}

pub(crate) fn control_icon(value: impl Into<Icon>, foreground: u32) -> Icon {
    value.into().text_color(rgb(foreground))
}

pub(crate) trait ControlState {
    fn unavailable(self, disabled: bool) -> Self;
}

impl ControlState for Button {
    fn unavailable(self, disabled: bool) -> Self {
        self.disabled(disabled).when(disabled, |button| {
            button
                .bg(rgb(0xe9e9e9))
                .border_color(rgb(0xd2d2d2))
                .cursor_default()
        })
    }
}

fn action_icon(name: &str) -> Icon {
    Icon::default().path(format!("force-icons/{name}.svg"))
}
fn page_heading(title: &str, date: Option<String>) -> Div {
    div()
        .mb(px(32.))
        .flex()
        .items_baseline()
        .justify_between()
        .child(
            div()
                .text_size(px(32.))
                .line_height(relative(1.2))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .when_some(date, |row, date| {
            row.child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(date))
        })
}

pub(crate) fn section_heading(title: &str) -> Div {
    div()
        .text_size(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.to_owned())
}

pub(crate) fn field_label(label: &str) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(MUTED))
        .child(label.to_owned())
}
fn timer_font(cx: &App) -> Font {
    let mut font = font(cx.theme().font_family.clone());
    font.features = FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]));
    font
}
fn clock(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
}
fn short_time(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    if minutes >= 60 && minutes % 60 == 0 {
        format!("{} 小时", minutes / 60)
    } else if minutes >= 60 {
        format!("{} 小时 {} 分", minutes / 60, minutes % 60)
    } else if seconds > 0 && minutes == 0 {
        format!("{seconds} 秒")
    } else {
        format!("{minutes} 分钟")
    }
}
fn session_times(session: &Session) -> String {
    let at = |timestamp| {
        Local
            .timestamp_opt(timestamp, 0)
            .single()
            .map_or("--:--".to_owned(), |time| time.format("%H:%M").to_string())
    };
    format!(
        "{}–{}",
        at(session.started_at),
        session.finished_at.map_or("现在".to_owned(), at)
    )
}

fn record_timestamp(timestamp: i64) -> String {
    Local.timestamp_opt(timestamp, 0).single().map_or_else(
        || "时间不可用".to_owned(),
        |time| time.format("%Y/%m/%d %H:%M:%S").to_string(),
    )
}

fn record_detail(label: &str, value: String) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .text_size(px(12.))
        .child(
            div()
                .w(px(28.))
                .flex_shrink_0()
                .text_color(rgb(MUTED))
                .child(label.to_owned()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_normal()
                .text_color(rgb(INK))
                .child(value),
        )
}

fn task_heading(title: &str) -> impl IntoElement {
    let label = title.to_owned();
    let help = label.clone();
    div()
        .id("task-heading")
        .text_size(px(24.))
        .font_weight(FontWeight::MEDIUM)
        .line_clamp(2)
        .tooltip(move |window, cx| Tooltip::new(help.clone()).build(window, cx))
        .child(label)
}

fn shortcut(label: &str, mac: &str, other: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("{label} · ⌘{mac}")
    } else {
        format!("{label} · Ctrl {other}")
    }
}
