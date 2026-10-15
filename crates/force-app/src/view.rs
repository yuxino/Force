use chrono::{Days, Local, TimeZone};
use force_core::{Session, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
    switch::Switch,
    tooltip::Tooltip,
};

use crate::{
    model::{ForceView, Page},
    platform,
};

const INK: u32 = 0x202020;
const MUTED: u32 = 0x737373;
const LINE: u32 = 0xe7e7e7;
const PAPER: u32 = 0xfdfdfd;
const SURFACE: u32 = 0xf2f2f2;
const DESTRUCTIVE: u32 = 0x963f3f;

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

    fn floating_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = if self.completed.is_some() {
            None
        } else {
            self.current_session()
        };
        let active_id = current.map(|session| session.id);
        let completed = self.completed.as_ref();
        let seconds = current.map_or_else(
            || {
                completed.map_or(
                    if self.countdown {
                        i64::from(self.timer_minutes) * 60
                    } else {
                        0
                    },
                    |s| {
                        if s.target_seconds
                            .is_some_and(|target| s.elapsed_at(self.now) >= target)
                        {
                            0
                        } else {
                            s.elapsed_at(self.now)
                        }
                    },
                )
            },
            |s| {
                s.target_seconds.map_or_else(
                    || s.elapsed_at(self.now),
                    |target| (target - s.elapsed_at(self.now)).max(0),
                )
            },
        );
        let title = if self.storage_failed {
            "本地数据不可用".to_owned()
        } else if let Some(session) = current {
            session.title.clone()
        } else if let Some(session) = completed {
            session.title.clone()
        } else {
            let input = self.task_input.read(cx).value();
            if input.trim().is_empty() {
                "准备专注".to_owned()
            } else {
                input.to_string()
            }
        };
        let state = if self.storage_failed {
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
        let (symbol, help) = if self.storage_failed {
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
        let state = self
            .toast
            .as_ref()
            .filter(|_| self.compact)
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
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .w_full()
                                    .text_ellipsis()
                                    .line_clamp(1)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .id("bar-status")
                                    .text_size(px(11.))
                                    .text_color(rgb(if self.storage_failed {
                                        0x963f3f
                                    } else {
                                        MUTED
                                    }))
                                    .w_full()
                                    .text_ellipsis()
                                    .line_clamp(1)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(state_help.clone()).build(window, cx)
                                    })
                                    .child(state),
                            ),
                    ),
            )
            .child(
                primary_button("bar-primary")
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
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(id) = active_id {
                            this.toggle_pause(id, cx);
                        } else {
                            this.primary_action(window, cx);
                        }
                    })),
            )
            .children(current.map(|session| {
                let id = session.id;
                button("bar-finish")
                    .ghost()
                    .size(px(28.))
                    .icon(control_icon(action_icon("stop").size(px(14.)), INK))
                    .tooltip(shortcut("结束本段", "⇧P", "Shift P"))
                    .on_click(cx.listener(move |this, _, _, cx| this.finish(id, cx)))
            }))
            .child(
                button("bar-expand")
                    .ghost()
                    .size(px(28.))
                    .text_color(rgb(MUTED))
                    .icon(control_icon(
                        Icon::new(if self.compact != self.panel_above {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronUp
                        })
                        .size(px(14.)),
                        INK,
                    ))
                    .tooltip(shortcut(
                        if self.compact { "展开" } else { "收起" },
                        "⇧M",
                        "Shift M",
                    ))
                    .on_click(cx.listener(|this, _, window, cx| this.compact(window, cx))),
            )
    }

    fn panel_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(40.))
            .px_3()
            .flex_shrink_0()
            .border_t_1()
            .border_b_1()
            .border_color(rgb(LINE))
            .flex()
            .items_center()
            .gap_1()
            .children(
                [
                    ("focus-tab", "专注", Page::Focus),
                    ("records-tab", "记录", Page::Records),
                ]
                .into_iter()
                .map(|(id, label, page)| {
                    button(id)
                        .ghost()
                        .small()
                        .h(px(28.))
                        .px(px(10.))
                        .text_color(rgb(if self.page == page { INK } else { MUTED }))
                        .font_weight(FontWeight::MEDIUM)
                        .selected(self.page == page)
                        .child(control_label(label, INK))
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.navigate(page, window, cx)),
                        )
                }),
            )
            .child(div().flex_1())
            .child(
                button("pin-window")
                    .ghost()
                    .size(px(28.))
                    .text_color(rgb(if self.pinned { INK } else { MUTED }))
                    .icon(control_icon(
                        Icon::default().path("force-icons/pin.svg").size(px(15.)),
                        INK,
                    ))
                    .unavailable(!cfg!(any(target_os = "macos", target_os = "windows")))
                    .tooltip(if !cfg!(any(target_os = "macos", target_os = "windows")) {
                        "置顶由窗口管理器控制"
                    } else if self.pinned {
                        "已置顶 · 点击取消"
                    } else {
                        "始终置顶"
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        let next = !this.pinned;
                        if platform::set_always_on_top(window, next).is_ok() {
                            this.pinned = next;
                            cx.notify();
                        } else {
                            this.notify("无法更改置顶状态，请重试。", cx);
                        }
                    })),
            )
            .child(
                button("settings-tab")
                    .ghost()
                    .size(px(28.))
                    .icon(control_icon(
                        Icon::new(IconName::Settings2).size(px(15.)),
                        INK,
                    ))
                    .selected(self.page == Page::Settings)
                    .tooltip(shortcut("设置", ",", ","))
                    .on_click(
                        cx.listener(|this, _, window, cx| {
                            this.navigate(Page::Settings, window, cx)
                        }),
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

    fn focus_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(session) = &self.completed {
            return self.completed_panel(session, cx).into_any_element();
        }
        if !self.preparing
            && let Some(session) = self.current_session()
        {
            return self.running_panel(session, cx).into_any_element();
        }
        self.prepare_panel(cx).into_any_element()
    }

    fn prepare_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tags: Vec<_> = self
            .tags
            .iter()
            .filter(|tag| tag.kind == TagKind::Focus)
            .cloned()
            .collect();
        let selected = self.selected_tag;
        let tag_name = tags
            .iter()
            .find(|tag| tag.id == selected)
            .map_or("选择标签", |tag| tag.name.as_str())
            .to_owned();
        let view = cx.entity().downgrade();
        let recent: Vec<_> = self
            .tasks
            .iter()
            .filter(|task| task.completed_at.is_none())
            .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(field_label("这次做什么"))
                    .child(field(&self.task_input)),
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
                                    button(("duration", minutes as u64))
                                        .ghost()
                                        .small()
                                        .flex_1()
                                        .min_w_0()
                                        .text_size(px(12.))
                                        .rounded(px(5.))
                                        .child(control_label(label, INK))
                                        .when(selected, |button| {
                                            button.bg(rgb(PAPER)).font_weight(FontWeight::MEDIUM)
                                        })
                                        .text_color(rgb(if selected { INK } else { MUTED }))
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
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(field_label("标签"))
                    .child(
                        button("task-tag")
                            .small()
                            .ghost()
                            .max_w(px(220.))
                            .overflow_hidden()
                            .tooltip(tag_name.clone())
                            .child(
                                div()
                                    .min_w_0()
                                    .max_w(px(172.))
                                    .text_size(px(13.))
                                    .text_ellipsis()
                                    .line_clamp(1)
                                    .text_color(rgb(INK))
                                    .child(tag_name),
                            )
                            .child(
                                Icon::new(IconName::ChevronDown)
                                    .size(px(13.))
                                    .text_color(rgb(INK)),
                            )
                            .dropdown_menu(move |mut menu, _, _| {
                                for tag in &tags {
                                    let id = tag.id;
                                    let view = view.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(tag.name.clone())
                                            .checked(id == selected)
                                            .on_click(move |_, _, cx| {
                                                let _ = view.update(cx, |this, cx| {
                                                    this.selected_tag = id;
                                                    cx.notify();
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    ),
            )
            .child(
                primary_button("start-focus")
                    .h(px(34.))
                    .w_full()
                    .icon(control_icon(action_icon("play").size(px(14.)), 0xffffff))
                    .child(control_label(
                        if self.countdown {
                            format!("开始 {} 分钟", self.timer_minutes)
                        } else {
                            "开始自由计时".to_owned()
                        },
                        0xffffff,
                    ))
                    .unavailable(
                        !self.tags.iter().any(|tag| tag.kind == TagKind::Focus)
                            || self.storage_failed,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.start(window, cx))),
            )
            .when(!recent.is_empty(), |body| {
                body.child(
                    div()
                        .pt_3()
                        .border_t_1()
                        .border_color(rgb(LINE))
                        .child(
                            div()
                                .mb_1()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(field_label("继续之前的事"))
                                .when(self.selected_task.is_some(), |row| {
                                    row.child(
                                        button("clear-task-draft")
                                            .ghost()
                                            .xsmall()
                                            .icon(control_icon(IconName::Plus, INK))
                                            .child(control_label("新任务", INK))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.new_task(window, cx)
                                            })),
                                    )
                                }),
                        )
                        .children(recent.into_iter().map(|task| {
                            let id = task.id;
                            div()
                                .h(px(38.))
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    button(("recent-task", id as u64))
                                        .ghost()
                                        .small()
                                        .px_0()
                                        .flex_1()
                                        .min_w_0()
                                        .justify_start()
                                        .overflow_hidden()
                                        .text_size(px(12.))
                                        .child(
                                            div()
                                                .min_w_0()
                                                .flex_1()
                                                .max_w(px(140.))
                                                .text_size(px(13.))
                                                .text_ellipsis()
                                                .line_clamp(1)
                                                .text_color(rgb(INK))
                                                .child(task.title.clone()),
                                        )
                                        .tooltip(task.title.clone())
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.select_task(id, window, cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(rgb(MUTED))
                                        .child(short_time(task.total_seconds)),
                                )
                                .child(
                                    button(("recent-play", id as u64))
                                        .ghost()
                                        .size(px(28.))
                                        .icon(control_icon(action_icon("play").size(px(13.)), INK))
                                        .tooltip("继续这件事")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.select_task(id, window, cx);
                                            this.start(window, cx);
                                        })),
                                )
                                .child(
                                    button(("recent-remove", id as u64))
                                        .ghost()
                                        .size(px(28.))
                                        .icon(control_icon(
                                            Icon::new(IconName::Close).size(px(13.)),
                                            INK,
                                        ))
                                        .tooltip("从最近任务移除，保留记录")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.archive_task(id, window, cx)
                                        })),
                                )
                        })),
                )
            })
    }

    fn running_panel(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let elapsed = session.elapsed_at(self.now);
        let rest = session.tag_kind == TagKind::Rest;
        let fraction = session
            .target_seconds
            .map(|target| (elapsed as f32 / target as f32).clamp(0., 1.));
        let remaining = session
            .target_seconds
            .unwrap_or(0)
            .saturating_sub(elapsed)
            .max(0);
        let end = Local
            .timestamp_opt(self.now.saturating_add(remaining), 0)
            .single();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(field_label(if rest {
                                "放松一下"
                            } else if session.is_paused() {
                                "已暂停"
                            } else {
                                "正在专注"
                            }))
                            .child(
                                div()
                                    .max_w(px(180.))
                                    .w_full()
                                    .text_ellipsis()
                                    .line_clamp(1)
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED))
                                    .child(session.tag_name.clone()),
                            ),
                    )
                    .child(task_heading(&session.title)),
            )
            .children(fraction.map(|fraction| {
                div().h(px(3.)).w_full().rounded_full().bg(rgb(LINE)).child(
                    div()
                        .h_full()
                        .w(relative(fraction))
                        .rounded_full()
                        .bg(rgb(if session.is_paused() { 0xaaaaaa } else { INK })),
                )
            }))
            .child(
                div()
                    .flex()
                    .items_start()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(field_label(if rest { "已休息" } else { "已投入" }))
                            .child(
                                div()
                                    .font(timer_font(cx))
                                    .text_size(px(20.))
                                    .child(clock(elapsed)),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .items_end()
                            .child(field_label(if session.is_paused() {
                                "剩余时间"
                            } else if session.target_seconds.is_none() {
                                "计时方式"
                            } else {
                                "预计结束"
                            }))
                            .child(div().font(timer_font(cx)).text_size(px(20.)).child(
                                if session.target_seconds.is_none() {
                                    "自由".to_owned()
                                } else if session.is_paused() {
                                    clock(remaining)
                                } else {
                                    end.map_or(String::new(), |end| end.format("%H:%M").to_string())
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .pt_1()
                    .child(
                        button("running-next")
                            .small()
                            .flex_1()
                            .h(px(32.))
                            .child(control_label(
                                if rest {
                                    if self.return_task.is_some() {
                                        "回到专注"
                                    } else {
                                        "开始专注"
                                    }
                                } else {
                                    "休息 5 分钟"
                                },
                                INK,
                            ))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if rest {
                                    this.return_to_task(window, cx);
                                } else {
                                    this.start_break(window, cx);
                                }
                            })),
                    )
                    .child(
                        button("switch-task")
                            .ghost()
                            .small()
                            .h(px(32.))
                            .flex_1()
                            .child(control_label("换个任务", INK))
                            .on_click(cx.listener(|this, _, window, cx| this.new_task(window, cx))),
                    ),
            )
            .when(self.active.len() > 1, |body| {
                body.child(
                    div()
                        .pt_3()
                        .border_t_1()
                        .border_color(rgb(LINE))
                        .child(field_label("其他计时"))
                        .children(
                            self.active
                                .iter()
                                .filter(|other| other.id != session.id)
                                .map(|other| {
                                    let id = other.id;
                                    div()
                                        .flex()
                                        .items_center()
                                        .h(px(36.))
                                        .child(
                                            button(("other-timer", id as u64))
                                                .ghost()
                                                .small()
                                                .flex_1()
                                                .min_w_0()
                                                .justify_start()
                                                .overflow_hidden()
                                                .child(
                                                    div()
                                                        .min_w_0()
                                                        .flex_1()
                                                        .max_w(px(210.))
                                                        .text_size(px(13.))
                                                        .text_ellipsis()
                                                        .line_clamp(1)
                                                        .text_color(rgb(INK))
                                                        .child(other.title.clone()),
                                                )
                                                .tooltip(other.title.clone())
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.selected_session = Some(id);
                                                    this.completed = None;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div().text_size(px(11.)).text_color(rgb(MUTED)).child(
                                                if other.is_paused() {
                                                    "已暂停"
                                                } else {
                                                    "计时中"
                                                },
                                            ),
                                        )
                                }),
                        ),
                )
            })
    }

    fn completed_panel(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let rest = session.tag_kind == TagKind::Rest;
        let next_title = if rest {
            self.return_task
                .and_then(|id| {
                    self.tasks
                        .iter()
                        .find(|task| task.id == id && task.completed_at.is_none())
                })
                .map_or(session.title.as_str(), |task| task.title.as_str())
        } else {
            session.title.as_str()
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
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
                            .text_size(px(28.))
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
                            .h(px(34.))
                            .rounded(px(6.))
                            .child(control_label(
                                if rest {
                                    "回到专注"
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
                                .h(px(34.))
                                .rounded(px(7.))
                                .child(control_label("休息 5 分钟", INK))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.start_break(window, cx)),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .when(!rest && session.task_id.is_some(), |row| {
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
                    })
                    .child(div().flex_1())
                    .child(
                        button("different-task")
                            .ghost()
                            .small()
                            .text_color(rgb(MUTED))
                            .child(control_label("换个任务", INK))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.dismiss_completed(cx);
                                this.new_task(window, cx);
                            })),
                    ),
            )
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
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div().flex().items_center().gap_1()
                    .child(button("previous-day").ghost().size(px(28.))
                        .icon(control_icon(IconName::ChevronLeft, INK)).tooltip("前一天")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.record_date = this.record_date.checked_sub_days(Days::new(1)).unwrap_or(this.record_date);
                            this.refresh(cx);
                        })))
                    .child(button("record-date").ghost().small().flex_1()
                        .child(control_label(self.record_date.format("%m 月 %d 日").to_string(), INK)).tooltip("回到今天")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.record_date = Local::now().date_naive();
                            this.refresh(cx);
                        })))
                    .child(button("next-day").ghost().size(px(28.))
                        .icon(control_icon(IconName::ChevronRight, INK)).tooltip("后一天")
                        .unavailable(self.record_date >= Local::now().date_naive())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.record_date = this.record_date.checked_add_days(Days::new(1)).unwrap_or(this.record_date);
                            this.refresh(cx);
                        }))),
            )
            .child(
                div().flex().items_end().gap_2().py_1()
                    .child(div().flex_1().flex().flex_col().gap_1()
                        .child(field_label("专注时长"))
                        .child(div().text_size(px(24.)).line_height(relative(1.2))
                            .font(timer_font(cx)).font_weight(FontWeight::MEDIUM)
                            .child(short_time(self.record_stats.focus_seconds))))
                    .child(div().pb_1().text_size(px(12.)).text_color(rgb(MUTED))
                        .child(format!("{} 条记录", self.history.len())))
                    .child(button("record-help").ghost().size(px(28.))
                        .icon(control_icon(Icon::new(IconName::Info).size(px(14.)), MUTED))
                        .tooltip("暂停不计时，重叠的专注时间只计算一次；休息不计入专注。记录按结束日期归档。")),
            )
            .child(field(&self.record_search).prefix(Icon::new(IconName::Search).size(px(14.)).text_color(rgb(MUTED))))
            .child(
                div().flex().flex_col()
                    .children(records.iter().map(|session| {
                        let id = session.id;
                        div().py_3().border_b_1().border_color(rgb(LINE))
                            .flex().items_center().gap_2()
                            .child(div().flex_1().min_w_0()
                                .child(div().text_size(px(13.)).w_full().text_ellipsis().line_clamp(1).child(session.title.clone()))
                                .child(div().mt_1().text_size(px(11.)).text_color(rgb(MUTED)).w_full().text_ellipsis().line_clamp(1)
                                    .child(format!("{} · {}", session_times(session), session.tag_name))))
                            .child(div().font(timer_font(cx)).text_size(px(13.))
                                .child(clock(session.elapsed_at(self.now))))
                            .child(destructive_button(("delete-record", id as u64)).ghost().size(px(28.))
                                .icon(control_icon(Icon::default().path("force-icons/trash.svg").size(px(14.)), DESTRUCTIVE))
                                .tooltip("删除记录")
                                .on_click(cx.listener(move |this, _, window, cx| this.delete_record(id, window, cx))))
                    }))
                    .when(records.is_empty(), |body| body.child(
                        div().py_5().flex().flex_col().items_center().gap_2()
                            .text_size(px(12.)).text_color(rgb(MUTED))
                            .child(Icon::new(if query.is_empty() { IconName::Inbox } else { IconName::Search }).size(px(22.)))
                            .child(if query.is_empty() { "这一天还没有记录" } else { "没有匹配的记录" })
                            .when(query.is_empty() && self.record_date == Local::now().date_naive(), |body| body.child(
                                button("records-start").ghost().small()
                                    .child(control_label("开始一段专注", INK))
                                    .icon(control_icon(Icon::new(IconName::ArrowRight).size(px(13.)), INK))
                                    .on_click(cx.listener(|this, _, window, cx| this.new_task(window, cx))),
                            )),
                    )),
            )
            .child(
                div().flex().justify_end()
                    .child(button("export-records").ghost().small()
                        .icon(control_icon(Icon::new(IconName::ArrowDown).size(px(14.)), MUTED))
                        .child(control_label("导出全部记录", MUTED))
                        .unavailable(self.export_busy).loading(self.export_busy)
                        .on_click(cx.listener(|this, _, _, cx| this.export(cx)))),
            )
    }

    fn settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().text_size(px(13.)).child("到时提示音"))
                    .child(
                        Switch::new("completion-sound")
                            .small()
                            .when(
                                cfg!(any(target_os = "macos", target_os = "windows")),
                                |switch| switch.cursor_pointer(),
                            )
                            .checked(self.sound_enabled)
                            .disabled(!cfg!(any(target_os = "macos", target_os = "windows")))
                            .tooltip(if cfg!(any(target_os = "macos", target_os = "windows")) {
                                "使用系统提示音"
                            } else {
                                "此平台暂不支持提示音"
                            })
                            .on_click(cx.listener(|this, checked, _, cx| {
                                this.set_sound_enabled(*checked, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().text_size(px(13.)).child("每日目标"))
                    .child(div().w(px(64.)).child(field(&self.goal_input)))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child("分钟"),
                    )
                    .child(
                        button("save-goal")
                            .ghost()
                            .size(px(28.))
                            .icon(control_icon(IconName::Check, INK))
                            .tooltip("保存每日目标")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let Ok(minutes) =
                                    this.goal_input.read(cx).value().trim().parse::<u32>()
                                else {
                                    this.notify("请输入 1–1440 之间的分钟数。", cx);
                                    return;
                                };
                                if this.write(|store| store.set_goal_minutes(minutes), cx) {
                                    this.notify("目标已保存。", cx);
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .pt_3()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(field_label("标签"))
                    .children(self.tags.iter().map(|tag| {
                        let id = tag.id;
                        div()
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(13.))
                                    .w_full()
                                    .text_ellipsis()
                                    .line_clamp(1)
                                    .child(tag.name.clone()),
                            )
                            .child(
                                div()
                                    .mr_1()
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED))
                                    .child(if tag.kind == TagKind::Focus {
                                        "专注"
                                    } else {
                                        "休息"
                                    }),
                            )
                            .child(
                                button(("edit-tag", id as u64))
                                    .ghost()
                                    .size(px(28.))
                                    .icon(control_icon(
                                        Icon::default()
                                            .path("force-icons/pencil.svg")
                                            .size(px(14.)),
                                        INK,
                                    ))
                                    .tooltip("重命名标签")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.begin_edit_tag(id, window, cx)
                                    })),
                            )
                            .child(
                                destructive_button(("remove-tag", id as u64))
                                    .ghost()
                                    .size(px(28.))
                                    .icon(control_icon(
                                        Icon::default().path("force-icons/trash.svg").size(px(14.)),
                                        DESTRUCTIVE,
                                    ))
                                    .tooltip("移除标签")
                                    .text_color(rgb(DESTRUCTIVE))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.delete_tag(id, window, cx)
                                    })),
                            )
                    }))
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(div().flex_1().min_w_0().child(field(&self.tag_input)))
                            .when(self.editing_tag.is_none(), |row| {
                                row.child(
                                    button("new-tag-kind")
                                        .ghost()
                                        .small()
                                        .child(control_label(
                                            if self.new_tag_kind == TagKind::Focus {
                                                "专注"
                                            } else {
                                                "休息"
                                            },
                                            INK,
                                        ))
                                        .tooltip("标签类型")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.new_tag_kind =
                                                if this.new_tag_kind == TagKind::Focus {
                                                    TagKind::Rest
                                                } else {
                                                    TagKind::Focus
                                                };
                                            cx.notify();
                                        })),
                                )
                            })
                            .child(
                                button("save-tag")
                                    .ghost()
                                    .size(px(28.))
                                    .icon(control_icon(
                                        if self.editing_tag.is_some() {
                                            IconName::Check
                                        } else {
                                            IconName::Plus
                                        },
                                        INK,
                                    ))
                                    .tooltip(if self.editing_tag.is_some() {
                                        "保存标签"
                                    } else {
                                        "添加标签"
                                    })
                                    .on_click(
                                        cx.listener(|this, _, window, cx| {
                                            this.save_tag(window, cx)
                                        }),
                                    ),
                            )
                            .when(self.editing_tag.is_some(), |row| {
                                row.child(
                                    button("cancel-tag-edit")
                                        .ghost()
                                        .size(px(28.))
                                        .icon(control_icon(IconName::Close, INK))
                                        .tooltip("取消")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.cancel_tag_edit(window, cx)
                                        })),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .pt_3()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        button("data-folder")
                            .ghost()
                            .small()
                            .icon(control_icon(IconName::FolderOpen, INK))
                            .child(control_label("本地数据", INK))
                            .on_click(cx.listener(|this, _, _, cx| cx.reveal_path(&this.data_dir))),
                    )
                    .child(
                        button("export-data")
                            .ghost()
                            .small()
                            .icon(control_icon(IconName::ArrowDown, INK))
                            .child(control_label("导出 CSV", INK))
                            .unavailable(self.export_busy)
                            .loading(self.export_busy)
                            .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(format!("Force {}", env!("CARGO_PKG_VERSION"))),
                    )
                    .child(
                        button("quit-force")
                            .ghost()
                            .small()
                            .text_color(rgb(MUTED))
                            .child(control_label("退出 Force", INK))
                            .on_click(cx.listener(|_, _, _, cx| cx.quit())),
                    ),
            )
    }
}

impl ForceView {
    fn expanded_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let scroll = &self.panel_scroll[match self.page {
            Page::Focus => 0,
            Page::Records => 1,
            Page::Settings => 2,
        }];
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.panel_navigation(cx))
            .child(
                div()
                    .id(match self.page {
                        Page::Focus => "focus-content",
                        Page::Records => "records-content",
                        Page::Settings => "settings-content",
                    })
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(scroll)
                    .relative()
                    .px_4()
                    .py_3()
                    .when(self.storage_failed, |body| {
                        body.child(self.storage_notice(cx))
                    })
                    .when(!self.storage_failed, |body| {
                        body.child(match self.page {
                            Page::Focus => self.focus_panel(cx),
                            Page::Records => self.records_panel(cx).into_any_element(),
                            Page::Settings => self.settings_panel(cx).into_any_element(),
                        })
                    })
                    .vertical_scrollbar(scroll),
            )
            .when(
                self.page != Page::Settings && !self.storage_failed,
                |body| {
                    body.child(
                        div()
                            .h(px(30.))
                            .px_4()
                            .flex_shrink_0()
                            .border_t_1()
                            .border_color(rgb(LINE))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(format!("今日专注 {}", short_time(self.today.focus_seconds)))
                            .child(
                                button("goal-help")
                                    .ghost()
                                    .size(px(28.))
                                    .icon(control_icon(
                                        Icon::new(IconName::Info).size(px(14.)),
                                        INK,
                                    ))
                                    .tooltip(format!(
                                        "每日目标 {} · 暂停不计时，电脑睡眠继续计时。",
                                        short_time(i64::from(self.goal) * 60)
                                    )),
                            ),
                    )
                },
            )
    }
}

impl Render for ForceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_panel_size(window, cx);
        let progress = self
            .current_session()
            .filter(|_| self.completed.is_none())
            .and_then(|s| {
                s.target_seconds
                    .map(|target| (s.elapsed_at(self.now) as f32 / target as f32).clamp(0., 1.))
            });
        div()
            .id("force")
            .track_focus(&self.focus_handle)
            .key_context("Force")
            .size_full()
            .p(px(4.))
            .text_color(rgb(INK))
            .text_size(px(13.))
            .child(
                div()
                    .size_full()
                    .relative()
                    .flex()
                    .flex_col()
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
                    .when(!self.compact && self.panel_above, |body| {
                        body.child(self.expanded_panel(cx))
                    })
                    .child(self.floating_bar(cx))
                    .when(!self.compact && !self.panel_above, |body| {
                        body.child(self.expanded_panel(cx))
                    })
                    .when(self.compact, |body| {
                        body.children(progress.map(|fraction| {
                            div()
                                .absolute()
                                .bottom_0()
                                .left_3()
                                .right_3()
                                .h(px(2.))
                                .bg(rgb(LINE))
                                .child(div().h_full().w(relative(fraction)).bg(rgb(MUTED)))
                        }))
                    })
                    .when(self.toast.is_some() && !self.compact, |body| {
                        body.child(
                            div()
                                .absolute()
                                .bottom(px(if self.panel_above {
                                    platform::STRIP_HEIGHT + 12.
                                } else {
                                    12.
                                }))
                                .left_3()
                                .right_3()
                                .p_3()
                                .rounded(px(6.))
                                .bg(rgb(INK))
                                .text_color(rgb(0xffffff))
                                .text_size(px(12.))
                                .child(
                                    self.toast
                                        .as_ref()
                                        .map(|(message, _)| message.clone())
                                        .unwrap_or_default(),
                                ),
                        )
                    }),
            )
    }
}

fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id)
        .text_size(px(13.))
        .rounded(px(6.))
        .cursor_pointer()
}

fn primary_button(id: impl Into<ElementId>) -> Button {
    button(id).primary().font_weight(FontWeight::MEDIUM)
}

fn destructive_button(id: impl Into<ElementId>) -> Button {
    button(id).text_color(rgb(DESTRUCTIVE))
}

fn field(state: &Entity<gpui_component::input::InputState>) -> Input {
    // Input::h only affects multiline editors; Styled::h sets the single-line
    // control height too. Use one geometry across task, search and settings.
    Styled::h(Input::new(state), px(32.))
        .text_size(px(13.))
        .line_height(px(20.))
        .px(px(9.))
        .py(px(5.))
        .bg(rgb(0xffffff))
        .rounded(px(6.))
}

// gpui-component 0.5.1 hardcodes a red hover foreground. Keep each label and
// icon explicitly colored; registering another hover handler would panic.
fn control_label(value: impl Into<SharedString>, foreground: u32) -> Div {
    div()
        .text_size(px(13.))
        .line_height(relative(1.2))
        .text_color(rgb(foreground))
        .child(value.into())
}

fn control_icon(value: impl Into<Icon>, foreground: u32) -> Icon {
    value.into().text_color(rgb(foreground))
}

trait ControlState {
    fn unavailable(self, disabled: bool) -> Self;
}

impl ControlState for Button {
    fn unavailable(self, disabled: bool) -> Self {
        self.disabled(disabled)
            .when(disabled, |button| button.opacity(0.4).cursor_default())
    }
}

fn action_icon(name: &str) -> Icon {
    Icon::default().path(format!("force-icons/{name}.svg"))
}
fn field_label(label: &str) -> Div {
    div()
        .text_size(px(12.))
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
    if minutes >= 60 {
        format!("{}小时{}分", minutes / 60, minutes % 60)
    } else if seconds > 0 && minutes == 0 {
        "不到1分钟".to_owned()
    } else {
        format!("{minutes}分钟")
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

fn task_heading(title: &str) -> impl IntoElement {
    let label = title.to_owned();
    let help = label.clone();
    div()
        .id("task-heading")
        .text_size(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_ellipsis()
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
