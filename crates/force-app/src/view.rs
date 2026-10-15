use chrono::{Days, Local, TimeZone};
use force_core::{Session, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
    tooltip::Tooltip,
};

use crate::{
    model::{ForceView, Page},
    platform,
};

const INK: u32 = 0x202020;
const MUTED: u32 = 0x777773;
const LINE: u32 = 0xe7e7e3;
const PAPER: u32 = 0xfdfdfc;

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
            ("pause", "暂停 · ⌘/Ctrl P")
        } else if current.is_some() {
            ("play", "继续 · ⌘/Ctrl P")
        } else {
            ("play", "开始专注")
        };
        div()
            .h(px(68.))
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_3()
            .gap_1()
            .child(
                div()
                    .id("floating-drag-area")
                    .flex_1()
                    .min_w_0()
                    .cursor_move()
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        let _ = platform::start_window_drag(window);
                    })
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(relative(1.25))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(title),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .flex()
                            .items_baseline()
                            .gap_3()
                            .child(
                                div()
                                    .font(timer_font(cx))
                                    .text_size(px(28.))
                                    .line_height(relative(1.1))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(clock(seconds)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(if self.storage_failed {
                                        0x963f3f
                                    } else {
                                        MUTED
                                    }))
                                    .child(state),
                            ),
                    ),
            )
            .child(
                Button::new("bar-primary")
                    .primary()
                    .size(px(32.))
                    .rounded_full()
                    .icon(if current.is_none() && completed.is_some() {
                        Icon::new(IconName::ArrowRight)
                    } else {
                        action_icon(symbol)
                    })
                    .tooltip(help)
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
                Button::new("bar-finish")
                    .ghost()
                    .size(px(28.))
                    .icon(action_icon("stop"))
                    .tooltip("结束本段 · ⌘/Ctrl Shift P")
                    .on_click(cx.listener(move |this, _, _, cx| this.finish(id, cx)))
            }))
            .child(
                Button::new("bar-expand")
                    .ghost()
                    .size(px(26.))
                    .icon(if self.compact {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronUp
                    })
                    .tooltip(if self.compact {
                        "展开 · ⌘/Ctrl Shift M"
                    } else {
                        "收起 · ⌘/Ctrl Shift M"
                    })
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
                    Button::new(id)
                        .ghost()
                        .small()
                        .label(label)
                        .selected(self.page == page)
                        .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
                }),
            )
            .child(div().flex_1())
            .child(
                Button::new("pin-window")
                    .ghost()
                    .size(px(28.))
                    .selected(self.pinned)
                    .icon(Icon::default().path("force-icons/pin.svg"))
                    .disabled(!cfg!(any(target_os = "macos", target_os = "windows")))
                    .tooltip(if cfg!(any(target_os = "macos", target_os = "windows")) {
                        "始终置顶"
                    } else {
                        "置顶由窗口管理器控制"
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
                Button::new("settings-tab")
                    .ghost()
                    .size(px(28.))
                    .icon(IconName::Settings2)
                    .selected(self.page == Page::Settings)
                    .tooltip("设置")
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Settings, cx))),
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
                    .text_color(rgb(0x963f3f))
                    .child("无法读写本地数据。原文件已保留。"),
            )
            .child(
                Button::new("retry-storage")
                    .label("重试")
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
        let tags = self.tags.clone();
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
            .take(4)
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(field_label("这次做什么"))
                    .child(Input::new(&self.task_input).h(px(36.)).text_size(px(14.))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(field_label("专注时长"))
                    .child(
                        div().flex().gap_1().children(
                            [
                                (25, "25 分钟"),
                                (45, "45 分钟"),
                                (60, "60 分钟"),
                                (0, "自由"),
                            ]
                            .into_iter()
                            .map(|(minutes, label)| {
                                Button::new(("duration", minutes as u64))
                                    .ghost()
                                    .small()
                                    .flex_1()
                                    .label(label)
                                    .selected(if minutes == 0 {
                                        !self.countdown
                                    } else {
                                        self.countdown && self.timer_minutes == minutes
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
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("task-tag")
                            .small()
                            .ghost()
                            .w(px(128.))
                            .max_w(px(128.))
                            .overflow_hidden()
                            .icon(IconName::ChevronDown)
                            .tooltip(tag_name.clone())
                            .child(div().min_w_0().flex_1().truncate().child(tag_name))
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
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("start-focus")
                            .primary()
                            .label(if self.countdown {
                                format!("开始 {} 分钟", self.timer_minutes)
                            } else {
                                "开始计时".to_owned()
                            })
                            .disabled(self.tags.is_empty() || self.storage_failed)
                            .on_click(cx.listener(|this, _, window, cx| this.start(window, cx))),
                    ),
            )
            .when(!recent.is_empty(), |body| {
                body.child(
                    div()
                        .pt_4()
                        .border_t_1()
                        .border_color(rgb(LINE))
                        .child(
                            div()
                                .mb_2()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(field_label("继续之前的事"))
                                .child(
                                    Button::new("clear-task-draft")
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Plus)
                                        .label("新任务")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.new_task(window, cx)
                                        })),
                                ),
                        )
                        .children(recent.into_iter().map(|task| {
                            let id = task.id;
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .h(px(42.))
                                .child(
                                    Button::new(("recent-task", id as u64))
                                        .ghost()
                                        .flex_1()
                                        .min_w_0()
                                        .justify_start()
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .min_w_0()
                                                .flex_1()
                                                .truncate()
                                                .child(task.title.clone()),
                                        )
                                        .tooltip(task.title.clone())
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.select_task(id, window, cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(MUTED))
                                        .child(short_time(task.total_seconds)),
                                )
                                .child(
                                    Button::new(("recent-play", id as u64))
                                        .ghost()
                                        .size(px(26.))
                                        .icon(action_icon("play"))
                                        .tooltip("继续这件事")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.select_task(id, window, cx);
                                            this.start(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new(("recent-remove", id as u64))
                                        .ghost()
                                        .size(px(24.))
                                        .icon(IconName::Close)
                                        .tooltip("从最近任务移除，保留计时记录")
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
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(field_label(if rest {
                        "休息时间"
                    } else {
                        "当前任务"
                    }))
                    .child(task_heading(&session.title))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(format!(
                                "{} · {}",
                                session.tag_name,
                                session.target_seconds.map_or(
                                    "自由计时".to_owned(),
                                    |target| format!("{} 分钟", target / 60)
                                )
                            )),
                    ),
            )
            .children(fraction.map(|fraction| {
                div()
                    .h(px(3.))
                    .w_full()
                    .rounded_full()
                    .bg(rgb(0xebebe8))
                    .child(
                        div()
                            .h_full()
                            .w(relative(fraction))
                            .rounded_full()
                            .bg(rgb(INK)),
                    )
            }))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(13.))
                    .child(div().text_color(rgb(MUTED)).child(if rest {
                        "已休息"
                    } else {
                        "已投入"
                    }))
                    .child(div().font(timer_font(cx)).child(clock(elapsed))),
            )
            .when(
                session.is_running() && session.target_seconds.is_some(),
                |body| {
                    let remaining = session
                        .target_seconds
                        .unwrap_or(0)
                        .saturating_sub(elapsed)
                        .max(0);
                    let end = Local
                        .timestamp_opt(self.now.saturating_add(remaining), 0)
                        .single();
                    body.child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(13.))
                            .child(div().text_color(rgb(MUTED)).child("预计结束"))
                            .child(
                                end.map_or(String::new(), |end| end.format("%H:%M").to_string()),
                            ),
                    )
                },
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .pt_2()
                    .child(
                        Button::new("panel-collapse")
                            .primary()
                            .flex_1()
                            .label("收起浮窗")
                            .on_click(cx.listener(|this, _, window, cx| this.compact(window, cx))),
                    )
                    .child(
                        Button::new("switch-task")
                            .ghost()
                            .label(if rest && self.return_task.is_some() {
                                "回到任务"
                            } else if rest {
                                "开始专注"
                            } else {
                                "切换任务"
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if rest {
                                    this.return_to_task(window, cx);
                                } else {
                                    this.new_task(window, cx);
                                }
                            })),
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
                                        .h(px(38.))
                                        .child(
                                            Button::new(("other-timer", id as u64))
                                                .ghost()
                                                .small()
                                                .flex_1()
                                                .justify_start()
                                                .overflow_hidden()
                                                .child(
                                                    div()
                                                        .min_w_0()
                                                        .flex_1()
                                                        .truncate()
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
                                            div().text_size(px(12.)).text_color(rgb(MUTED)).child(
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
                    .text_color(rgb(MUTED))
                    .text_size(px(13.))
                    .child(Icon::new(IconName::CircleCheck).size(px(16.)))
                    .child(if rest {
                        "休息结束"
                    } else {
                        "本段已记录"
                    }),
            )
            .child(task_heading(next_title))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(13.))
                    .child(div().text_color(rgb(MUTED)).child(if rest {
                        "休息时长"
                    } else {
                        "本段投入"
                    }))
                    .child(
                        div()
                            .font(timer_font(cx))
                            .child(clock(session.elapsed_at(self.now))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .pt_1()
                    .child(
                        Button::new("continue-task")
                            .primary()
                            .w_full()
                            .label(if rest {
                                "回到专注"
                            } else {
                                "继续这件事"
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if rest {
                                    this.return_to_task(window, cx);
                                } else {
                                    this.repeat_completed(window, cx);
                                }
                            })),
                    )
                    .when(!rest, |body| {
                        body.child(
                            Button::new("rest-five")
                                .w_full()
                                .label("休息 5 分钟")
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.start_break(window, cx)),
                                ),
                        )
                    })
                    .when(!rest && session.task_id.is_some(), |body| {
                        body.child(
                            Button::new("complete-task")
                                .ghost()
                                .w_full()
                                .icon(IconName::Check)
                                .label("标记任务完成")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.complete_current_task(window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("different-task")
                            .ghost()
                            .w_full()
                            .label("换一件事")
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
        div().flex().flex_col().gap_3()
            .child(div().flex().items_center().gap_1()
                .child(Button::new("previous-day").ghost().size(px(26.)).icon(IconName::ChevronLeft).tooltip("前一天")
                    .on_click(cx.listener(|this, _, _, cx| { this.record_date = this.record_date.checked_sub_days(Days::new(1)).unwrap_or(this.record_date); this.refresh(cx); })))
                .child(Button::new("record-date").ghost().small().flex_1().label(self.record_date.format("%m 月 %d 日").to_string()).tooltip("回到今天")
                    .on_click(cx.listener(|this, _, _, cx| { this.record_date = Local::now().date_naive(); this.refresh(cx); })))
                .child(Button::new("next-day").ghost().size(px(26.)).icon(IconName::ChevronRight).tooltip("后一天").disabled(self.record_date >= Local::now().date_naive())
                    .on_click(cx.listener(|this, _, _, cx| { this.record_date = this.record_date.checked_add_days(Days::new(1)).unwrap_or(this.record_date); this.refresh(cx); }))))
            .child(div().flex().items_baseline().gap_2()
                .child(div().text_size(px(25.)).font(timer_font(cx)).font_weight(FontWeight::MEDIUM).child(short_time(self.record_stats.focus_seconds)))
                .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(format!("专注 · {} 段", self.record_stats.completed_sessions)))
                .child(div().flex_1())
                .child(Button::new("record-help").ghost().size(px(24.)).icon(IconName::Info).tooltip("暂停不计时，重叠的专注时间只计算一次；休息不计入专注。记录按结束日期归档。")))
            .child(Input::new(&self.record_search).small().prefix(Icon::new(IconName::Search)))
            .child(div().flex().flex_col().children(records.iter().map(|session| {
                let id = session.id;
                div().py_3().border_b_1().border_color(rgb(LINE)).flex().items_center().gap_2()
                    .child(div().flex_1().min_w_0()
                        .child(div().text_size(px(13.)).truncate().child(session.title.clone()))
                        .child(div().mt_1().text_size(px(12.)).text_color(rgb(MUTED)).truncate().child(format!("{} · {}", session_times(session), session.tag_name))))
                    .child(div().font(timer_font(cx)).text_size(px(13.)).child(clock(session.elapsed_at(self.now))))
                    .child(Button::new(("delete-record", id as u64)).ghost().size(px(24.)).icon(Icon::default().path("force-icons/trash.svg")).tooltip("删除记录").text_color(rgb(0x963f3f))
                        .on_click(cx.listener(move |this, _, window, cx| this.delete_record(id, window, cx))))
            })).when(records.is_empty(), |body| body.child(div().py_6().text_center().text_size(px(13.)).text_color(rgb(MUTED)).child(if query.is_empty() { "这一天还没有计时记录" } else { "没有匹配的记录" }))))
            .child(Button::new("export-records").ghost().small().icon(IconName::ArrowDown).label("导出全部记录").disabled(self.export_busy).loading(self.export_busy)
                .on_click(cx.listener(|this, _, _, cx| this.export(cx))))
    }

    fn settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().text_size(px(13.)).child("到时提示音"))
                    .child(
                        Button::new("completion-sound")
                            .disabled(!cfg!(any(target_os = "macos", target_os = "windows")))
                            .tooltip("使用系统提示音")
                            .ghost()
                            .small()
                            .selected(self.sound_enabled)
                            .label(if !cfg!(any(target_os = "macos", target_os = "windows")) {
                                "暂不支持"
                            } else if self.sound_enabled {
                                "开启"
                            } else {
                                "关闭"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_sound_enabled(!this.sound_enabled, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().text_size(px(13.)).child("每日目标"))
                    .child(div().w(px(58.)).child(Input::new(&self.goal_input).small()))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child("分钟"),
                    )
                    .child(
                        Button::new("save-goal")
                            .ghost()
                            .size(px(28.))
                            .icon(IconName::Check)
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
                    .pt_4()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(field_label("标签"))
                    .children(self.tags.iter().map(|tag| {
                        let id = tag.id;
                        div()
                            .h(px(36.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(13.))
                                    .truncate()
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
                                Button::new(("edit-tag", id as u64))
                                    .ghost()
                                    .size(px(26.))
                                    .icon(Icon::default().path("force-icons/pencil.svg"))
                                    .tooltip("重命名标签")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.begin_edit_tag(id, window, cx)
                                    })),
                            )
                            .child(
                                Button::new(("remove-tag", id as u64))
                                    .ghost()
                                    .size(px(26.))
                                    .icon(Icon::default().path("force-icons/trash.svg"))
                                    .tooltip("移除标签")
                                    .text_color(rgb(0x963f3f))
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
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(Input::new(&self.tag_input).small()),
                            )
                            .when(self.editing_tag.is_none(), |row| {
                                row.child(
                                    Button::new("new-tag-kind")
                                        .ghost()
                                        .small()
                                        .label(if self.new_tag_kind == TagKind::Focus {
                                            "专注"
                                        } else {
                                            "休息"
                                        })
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
                                Button::new("save-tag")
                                    .ghost()
                                    .size(px(28.))
                                    .icon(if self.editing_tag.is_some() {
                                        IconName::Check
                                    } else {
                                        IconName::Plus
                                    })
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
                                    Button::new("cancel-tag-edit")
                                        .ghost()
                                        .size(px(26.))
                                        .icon(IconName::Close)
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
                        Button::new("data-folder")
                            .ghost()
                            .small()
                            .icon(IconName::FolderOpen)
                            .label("本地数据")
                            .on_click(cx.listener(|this, _, _, cx| cx.reveal_path(&this.data_dir))),
                    )
                    .child(
                        Button::new("export-data")
                            .ghost()
                            .small()
                            .icon(IconName::ArrowDown)
                            .label("导出 CSV")
                            .disabled(self.export_busy)
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
                        Button::new("quit-force")
                            .ghost()
                            .small()
                            .label("退出")
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
                    .p_4()
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
                            .h(px(32.))
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
                                Button::new("goal-help")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Info)
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
                    .rounded(px(14.))
                    .border_1()
                    .border_color(rgb(0xdededa))
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
                                .bg(rgb(0xefefec))
                                .child(div().h_full().w(relative(fraction)).bg(rgb(0x777773)))
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
                                .rounded_md()
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

fn action_icon(name: &str) -> Icon {
    Icon::default().path(format!("force-icons/{name}.svg"))
}
fn field_label(label: &str) -> Div {
    div()
        .text_size(px(12.))
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
        .text_size(px(17.))
        .font_weight(FontWeight::MEDIUM)
        .line_clamp(2)
        .tooltip(move |window, cx| Tooltip::new(help.clone()).build(window, cx))
        .child(label)
}
