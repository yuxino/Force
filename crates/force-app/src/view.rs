use chrono::{Days, Local, TimeZone};
use force_core::{Session, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Selectable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};

use crate::{
    NewTask, Quit, ToggleCompact,
    model::{ForceView, Page},
    platform,
};

const INK: u32 = 0x202020;
const MUTED: u32 = 0x777775;
const LINE: u32 = 0xe5e5e2;

impl ForceView {
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let nav = [
            ("focus", "专注", IconName::Frame, Page::Focus),
            ("records", "记录", IconName::ChartPie, Page::Records),
            ("settings", "设置", IconName::Settings2, Page::Settings),
        ];
        div()
            .w(px(168.))
            .h_full()
            .flex_shrink_0()
            .bg(rgb(0xf4f4f1))
            .border_r_1()
            .border_color(rgb(LINE))
            .p_5()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .mt_3()
                    .mb_10()
                    .child(
                        svg()
                            .path("force-logo.svg")
                            .size(px(28.))
                            .text_color(rgb(INK)),
                    )
                    .child(
                        div()
                            .text_size(px(23.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Force"),
                    ),
            )
            .children(nav.into_iter().map(|(id, label, icon, page)| {
                Button::new(id)
                    .ghost()
                    .justify_start()
                    .w_full()
                    .mb_2()
                    .h(px(40.))
                    .icon(icon)
                    .label(label)
                    .selected(self.page == page)
                    .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
            }))
            .child(div().flex_1())
            .child(
                Button::new("keyboard-help")
                    .ghost()
                    .small()
                    .icon(IconName::Info)
                    .label("快捷键")
                    .justify_start()
                    .mb_2()
                    .tooltip("回车开始 · ⌘/Ctrl N 新任务 · ⌘/Ctrl Shift M 小窗 · ⌘/Ctrl Q 退出"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(div().size(px(5.)).rounded_full().bg(rgb(MUTED)))
                    .child("数据只存本机"),
            )
    }

    fn section_heading(&self, title: &str, subtitle: &str) -> Div {
        div()
            .mb_2()
            .child(
                div()
                    .text_size(px(26.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_owned()),
            )
            .child(
                div()
                    .mt_2()
                    .text_size(px(14.))
                    .text_color(rgb(MUTED))
                    .child(subtitle.to_owned()),
            )
    }

    fn storage_notice(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .mb_5()
            .p_3()
            .border_1()
            .border_color(rgb(0xc1a8a8))
            .rounded_md()
            .text_color(rgb(0x9a3535))
            .child("本地数据不可用。请检查磁盘空间和文件权限，现有文件会保留。")
            .child(
                Button::new("retry-storage")
                    .ghost()
                    .small()
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
            .into_any_element()
    }

    fn completion_notice(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(session) = &self.completed else {
            return div().into_any_element();
        };
        let rest = session.tag_kind == TagKind::Rest;
        let mut actions = div().flex().flex_wrap().gap_2().justify_end();
        if rest {
            actions = actions.child(
                Button::new("back-to-focus")
                    .primary()
                    .label("开始下一项专注")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_completed(cx);
                        this.countdown = true;
                        this.timer_minutes = 25;
                        this.selected_tag = this
                            .tags
                            .iter()
                            .find(|tag| tag.kind == TagKind::Focus)
                            .map_or(0, |tag| tag.id);
                        this.task_input
                            .update(cx, |input, cx| input.focus(window, cx));
                    })),
            );
        } else {
            actions = actions
                .child(
                    Button::new("start-break")
                        .primary()
                        .label("休息 5 分钟")
                        .on_click(cx.listener(|this, _, window, cx| this.start_break(window, cx))),
                )
                .child(Button::new("repeat-completed").label("再来一轮").on_click(
                    cx.listener(|this, _, window, cx| this.repeat_completed(window, cx)),
                ));
        }
        div()
            .p_4()
            .border_1()
            .border_color(rgb(0xd7d7d2))
            .rounded_lg()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(gpui_component::Icon::new(IconName::CircleCheck))
                    .child(div().flex_1().min_w_0().truncate().child(format!(
                        "{} · {}",
                        if rest {
                            "休息结束"
                        } else {
                            "本轮已完成"
                        },
                        session.title
                    )))
                    .child(
                        Button::new("dismiss-completed")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("收起完成状态")
                            .on_click(cx.listener(|this, _, _, cx| this.dismiss_completed(cx))),
                    ),
            )
            .child(actions)
            .into_any_element()
    }

    fn focus_page(&self, narrow: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self
            .active
            .iter()
            .find(|session| Some(session.id) == self.selected_session);
        let duration = current.map_or(
            if self.countdown {
                i64::from(self.timer_minutes) * 60
            } else {
                0
            },
            |session| {
                session.target_seconds.map_or_else(
                    || session.elapsed_at(self.now),
                    |target| (target - session.elapsed_at(self.now)).max(0),
                )
            },
        );
        let title = current.map_or(
            "选个任务，留一段不被打断的时间。",
            |session| session.title.as_str(),
        );
        let state = current.map_or("准备开始", |session| {
            if session.is_running() {
                if session.tag_kind == TagKind::Rest {
                    "休息中"
                } else if session.target_seconds.is_some() {
                    "番茄钟 · 专注中"
                } else {
                    "正计时 · 专注中"
                }
            } else {
                "已暂停"
            }
        });
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(MUTED))
                            .child(Local::now().format("%Y / %m / %d").to_string()),
                    )
                    .child(
                        Button::new("compact")
                            .ghost()
                            .small()
                            .icon(IconName::Minimize)
                            .label("小窗")
                            .tooltip("切换小窗 · ⌘/Ctrl Shift M")
                            .on_click(cx.listener(|this, _, window, cx| this.compact(window, cx))),
                    ),
            )
            .children(self.completed.as_ref().map(|_| self.completion_notice(cx)))
            .child(
                div()
                    .pt_4()
                    .pb_2()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(state))
                    .child(
                        div()
                            .font(timer_font(cx))
                            .text_size(px(if narrow { 48. } else { 68. }))
                            .font_weight(FontWeight::LIGHT)
                            .child(clock(duration)),
                    )
                    .child(
                        div()
                            .w_full()
                            .max_w(px(480.))
                            .text_center()
                            .truncate()
                            .text_size(px(16.))
                            .child(title.to_owned()),
                    )
                    .children(current.map(|session| {
                        let id = session.id;
                        div()
                            .mt_2()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("hero-pause")
                                    .label(if session.is_running() {
                                        "暂停"
                                    } else {
                                        "继续"
                                    })
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| {
                                            this.toggle_pause(id, cx)
                                        }),
                                    ),
                            )
                            .child(
                                Button::new("hero-finish")
                                    .primary()
                                    .label("完成这一段")
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| this.finish(id, cx)),
                                    ),
                            )
                    })),
            )
            .child(
                div()
                    .border_1()
                    .border_color(rgb(0xdededa))
                    .rounded_lg()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(self.mode_controls(cx))
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div().flex_1().min_w_0().child(
                                    Input::new(&self.task_input)
                                        .appearance(false)
                                        .bordered(false)
                                        .text_size(px(16.)),
                                ),
                            )
                            .child(
                                Button::new("start")
                                    .primary()
                                    .label(if self.active.is_empty() {
                                        "开始专注"
                                    } else {
                                        "另开一项"
                                    })
                                    .disabled(self.tags.is_empty() || self.storage_failed)
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.start(window, cx)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(self.tags.iter().map(|tag| {
                                let id = tag.id;
                                Button::new(("tag", id as u64))
                                    .small()
                                    .ghost()
                                    .selected(self.selected_tag == id)
                                    .label(tag.name.clone())
                                    .tooltip(if tag.kind == TagKind::Focus {
                                        "计入专注目标"
                                    } else {
                                        "休息不计入专注目标"
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.selected_tag = id;
                                        cx.notify();
                                    }))
                            })),
                    )
                    .when(self.tags.is_empty() && !self.storage_failed, |view| {
                        view.child(
                            Button::new("create-first-tag")
                                .ghost()
                                .label("先添加一个标签")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.navigate(Page::Settings, cx)),
                                ),
                        )
                    }),
            )
            .children((!self.active.is_empty()).then(|| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .mb_2()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(format!("进行中 · {}", self.active.len())),
                    )
                    .children(
                        self.active
                            .iter()
                            .map(|session| self.session_row(session, false, cx)),
                    )
            }))
            .child(self.goal_view())
    }

    fn mode_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_start()
            .flex_wrap()
            .gap_2()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .mr_2()
                    .child("新任务"),
            )
            .child(
                Button::new("stopwatch-mode")
                    .ghost()
                    .small()
                    .selected(!self.countdown)
                    .label("正计时")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.countdown = false;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("pomodoro-mode")
                    .ghost()
                    .small()
                    .selected(self.countdown)
                    .label("番茄钟")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.countdown = true;
                        cx.notify();
                    })),
            )
            .when(self.countdown, |row| {
                row.child(div().w(px(1.)).h(px(18.)).mx_2().bg(rgb(LINE)))
                    .child(
                        Button::new("duration-minus")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Minus)
                            .tooltip("减少 5 分钟")
                            .disabled(self.timer_minutes <= 5)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.timer_minutes = this.timer_minutes.saturating_sub(5).max(5);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .child(format!("{} 分钟", self.timer_minutes)),
                    )
                    .child(
                        Button::new("duration-plus")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .tooltip("增加 5 分钟")
                            .disabled(self.timer_minutes >= 120)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.timer_minutes = (this.timer_minutes + 5).min(120);
                                cx.notify();
                            })),
                    )
            })
    }

    fn session_row(&self, session: &Session, history: bool, cx: &mut Context<Self>) -> AnyElement {
        let id = session.id;
        let name = if history {
            div()
                .flex_1()
                .min_w_0()
                .child(div().truncate().child(session.title.clone()))
                .child(
                    div()
                        .mt_1()
                        .text_size(px(11.))
                        .text_color(rgb(MUTED))
                        .child(session_times(session)),
                )
                .into_any_element()
        } else {
            Button::new(("select-session", id as u64))
                .ghost()
                .flex_1()
                .min_w_0()
                .justify_start()
                .label(session.title.clone())
                .tooltip(session.title.clone())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected_session = Some(id);
                    cx.notify();
                }))
                .into_any_element()
        };
        div()
            .flex()
            .items_center()
            .gap_3()
            .py_3()
            .border_b_1()
            .border_color(rgb(LINE))
            .child(div().w(px(3.)).h(px(28.)).rounded_full().bg(rgb(
                if !history && session.is_running() {
                    0x252525
                } else {
                    0xd1d1ce
                },
            )))
            .child(name)
            .child(
                div()
                    .max_w(px(70.))
                    .truncate()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(session.tag_name.clone()),
            )
            .child(
                div()
                    .w(px(74.))
                    .text_right()
                    .font(timer_font(cx))
                    .text_size(px(13.))
                    .child(clock(session.elapsed_at(self.now))),
            )
            .when(!history, |row| {
                row.child(
                    Button::new(("pause", id as u64))
                        .ghost()
                        .small()
                        .label(if session.is_running() {
                            "暂停"
                        } else {
                            "继续"
                        })
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_pause(id, cx))),
                )
                .child(
                    Button::new(("finish", id as u64))
                        .ghost()
                        .small()
                        .icon(IconName::Check)
                        .tooltip("完成这一段")
                        .on_click(cx.listener(move |this, _, _, cx| this.finish(id, cx))),
                )
            })
            .when(history, |row| {
                row.child(
                    Button::new(("delete", id as u64))
                        .ghost()
                        .small()
                        .icon(gpui_component::Icon::default().path("force-icons/trash.svg"))
                        .tooltip("删除记录")
                        .text_color(rgb(0x9a3535))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.delete_record(id, window, cx)
                        })),
                )
            })
            .into_any_element()
    }

    fn goal_view(&self) -> impl IntoElement {
        let fraction = (self.today.focus_seconds as f32 / (self.goal as f32 * 60.)).clamp(0., 1.);
        div()
            .pt_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_size(px(13.)).child("今日专注"))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(MUTED))
                            .child(format!(
                                "{} / {}",
                                short_time(self.today.focus_seconds),
                                short_time(i64::from(self.goal) * 60)
                            )),
                    ),
            )
            .child(
                div()
                    .h(px(4.))
                    .w_full()
                    .rounded_full()
                    .bg(rgb(0xe9e9e6))
                    .child(
                        div()
                            .h_full()
                            .w(relative(fraction))
                            .rounded_full()
                            .bg(rgb(0x272727)),
                    ),
            )
    }

    fn records_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
        let max = self
            .week
            .iter()
            .map(|(_, seconds)| *seconds)
            .max()
            .unwrap_or(1)
            .max(3600);
        div().flex().flex_col().gap_6()
            .child(self.section_heading("记录", "看看时间花在了哪里。"))
            .child(div().flex().flex_wrap().gap_2().justify_between().items_center()
                .child(div().flex().items_center().gap_1()
                    .child(Button::new("previous-day").ghost().small().icon(IconName::ChevronLeft).tooltip("前一天").on_click(cx.listener(|this, _, _, cx| {
                        this.record_date = this.record_date.checked_sub_days(Days::new(1)).unwrap_or(this.record_date); this.refresh(cx);
                    })))
                    .child(Button::new("today").ghost().small().label(self.record_date.format("%Y / %m / %d").to_string()).tooltip("回到今天")
                        .on_click(cx.listener(|this, _, _, cx| { this.record_date = Local::now().date_naive(); this.refresh(cx); })))
                    .child(Button::new("next-day").ghost().small().icon(IconName::ChevronRight).tooltip("后一天").disabled(self.record_date >= Local::now().date_naive()).on_click(cx.listener(|this, _, _, cx| {
                        this.record_date = this.record_date.checked_add_days(Days::new(1)).unwrap_or(this.record_date); this.refresh(cx);
                    }))))
                .child(Button::new("export-records").ghost().small().icon(IconName::ArrowDown).label("导出全部").disabled(self.export_busy).loading(self.export_busy).on_click(cx.listener(|this, _, _, cx| this.export(cx)))))
            .child(div().flex().gap_4()
                .child(metric("专注时长", short_time(self.record_stats.focus_seconds)))
                .child(metric("任务投入", short_time(self.record_stats.task_seconds)))
                .child(metric("完成段数", self.record_stats.completed_sessions.to_string()))
                .child(Button::new("stats-help").ghost().xsmall().icon(IconName::Info).tooltip("专注时长合并重叠的专注时间；任务投入分别累计所有任务，包含休息。指标对应所选日期，列表按完成日期归档。")))
            .child(div().border_b_1().border_color(rgb(LINE)).pb_5().child(div().mb_4().text_size(px(12.)).text_color(rgb(MUTED)).child("近七天 · 专注时长"))
                .child(div().h(px(125.)).flex().gap_3().items_end().children(self.week.iter().map(|(date, seconds)| {
                    let height = (*seconds as f32 / max as f32 * 82.).max(2.);
                    div().flex_1().flex().flex_col().items_center().gap_2()
                        .child(div().text_size(px(10.)).text_color(rgb(0x888886)).child(short_time(*seconds)))
                        .child(div().w(px(24.)).h(px(height)).rounded_sm().bg(rgb(if *date == Local::now().date_naive() { 0x292929 } else { 0xb9b9b4 })))
                        .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(date.format("%m/%d").to_string()))
                }))))
            .child(Input::new(&self.record_search).prefix(gpui_component::Icon::new(IconName::Search)))
            .child(div().flex().flex_col().children(records.iter().map(|session| self.session_row(session, true, cx)))
                .when(records.is_empty(), |view| view.child(div().py_8().flex().flex_col().items_center().gap_3().text_size(px(14.)).text_color(rgb(MUTED))
                    .child(if query.is_empty() { "这一天还没有完成的记录。" } else { "没有匹配的任务或标签。" })
                    .when(query.is_empty(), |view| view.child(Button::new("record-empty-start").ghost().label("开始一段专注").on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Focus, cx))))))))
    }

    fn settings_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(self.section_heading("设置", "按自己的节奏来。"))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pb_6()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(div().text_size(px(15.)).child("每日专注目标"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(78.)).child(Input::new(&self.goal_input)))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(MUTED))
                                    .child("分钟"),
                            )
                            .child(Button::new("save-goal").label("保存").on_click(cx.listener(
                                |this, _, _, cx| {
                                    let Ok(minutes) =
                                        this.goal_input.read(cx).value().trim().parse::<u32>()
                                    else {
                                        this.notify("请输入 1–1440 之间的分钟数。", cx);
                                        return;
                                    };
                                    if this.write(|store| store.set_goal_minutes(minutes), cx) {
                                        this.notify("目标已保存。", cx);
                                    }
                                },
                            ))),
                    ),
            )
            .child(div().text_size(px(15.)).child("标签"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .children(self.tags.iter().map(|tag| {
                        let id = tag.id;
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .py_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_3()
                                    .items_center()
                                    .child(div().truncate().child(tag.name.clone()))
                                    .child(
                                        div().text_size(px(12.)).text_color(rgb(0x888886)).child(
                                            if tag.kind == TagKind::Focus {
                                                "专注"
                                            } else {
                                                "休息"
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                Button::new(("edit-tag", id as u64))
                                    .ghost()
                                    .small()
                                    .icon(
                                        gpui_component::Icon::default()
                                            .path("force-icons/pencil.svg"),
                                    )
                                    .tooltip("重命名标签")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.begin_edit_tag(id, window, cx)
                                    })),
                            )
                            .child(
                                Button::new(("remove-tag", id as u64))
                                    .ghost()
                                    .small()
                                    .icon(
                                        gpui_component::Icon::default()
                                            .path("force-icons/trash.svg"),
                                    )
                                    .tooltip("移除标签")
                                    .text_color(rgb(0x9a3535))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.delete_tag(id, window, cx)
                                    })),
                            )
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().child(Input::new(&self.tag_input)))
                    .when(self.editing_tag.is_some(), |row| {
                        row.child(
                            Button::new("cancel-tag-edit")
                                .ghost()
                                .small()
                                .icon(IconName::Close)
                                .tooltip("取消重命名")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_tag_edit(window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("tag-kind")
                            .label(if self.new_tag_kind == TagKind::Focus {
                                "专注"
                            } else {
                                "休息"
                            })
                            .tooltip("切换标签类型；休息不计入目标")
                            .disabled(self.editing_tag.is_some())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.new_tag_kind = if this.new_tag_kind == TagKind::Focus {
                                    TagKind::Rest
                                } else {
                                    TagKind::Focus
                                };
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("add-tag")
                            .icon(if self.editing_tag.is_some() {
                                IconName::Check
                            } else {
                                IconName::Plus
                            })
                            .label(if self.editing_tag.is_some() {
                                "保存"
                            } else {
                                "添加"
                            })
                            .on_click(cx.listener(|this, _, window, cx| this.save_tag(window, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pt_6()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(div().text_size(px(15.)).child("本地数据"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("data-folder")
                                    .ghost()
                                    .icon(IconName::FolderOpen)
                                    .label("打开文件夹")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| {
                                            cx.reveal_path(&this.data_dir)
                                        }),
                                    ),
                            )
                            .child(
                                Button::new("export")
                                    .label("导出 CSV")
                                    .disabled(self.export_busy)
                                    .loading(self.export_busy)
                                    .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .pt_4()
                    .text_size(px(12.))
                    .text_color(rgb(0x888886))
                    .child("Force 0.1.0 · 暂停不计时，电脑睡眠继续计时。"),
            )
    }

    fn mini(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self
            .active
            .iter()
            .find(|session| Some(session.id) == self.selected_session);
        let seconds = current.map_or(0, |session| {
            session.target_seconds.map_or_else(
                || session.elapsed_at(self.now),
                |target| (target - session.elapsed_at(self.now)).max(0),
            )
        });
        let title = current.map_or(
            if self.completed.is_some() {
                "本轮已完成"
            } else {
                "准备开始"
            },
            |session| session.title.as_str(),
        );
        let mut controls = div().flex().gap_1();
        if let Some(session) = current {
            let id = session.id;
            controls = controls
                .child(
                    Button::new("mini-pause")
                        .small()
                        .ghost()
                        .label(if session.is_running() {
                            "暂停"
                        } else {
                            "继续"
                        })
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_pause(id, cx))),
                )
                .child(
                    Button::new("mini-finish")
                        .small()
                        .primary()
                        .icon(IconName::Check)
                        .tooltip("完成")
                        .on_click(cx.listener(move |this, _, _, cx| this.finish(id, cx))),
                );
        } else {
            controls = controls.child(
                Button::new("mini-start")
                    .small()
                    .ghost()
                    .label(if self.storage_failed {
                        "展开"
                    } else if self.completed.is_some() {
                        "查看"
                    } else {
                        "开始"
                    })
                    .tooltip("展开主窗口")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.compact(window, cx);
                        this.page = Page::Focus;
                        if !this.storage_failed {
                            this.task_input
                                .update(cx, |input, cx| input.focus(window, cx));
                        }
                    })),
            );
        }
        div()
            .size_full()
            .bg(rgb(0xfafaf8))
            .p_4()
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        svg()
                            .path("force-logo.svg")
                            .size(px(18.))
                            .text_color(rgb(INK)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .truncate()
                            .child(title.to_owned()),
                    )
                    .child(
                        Button::new("pin")
                            .ghost()
                            .xsmall()
                            .icon(gpui_component::Icon::default().path("force-icons/pin.svg"))
                            .selected(self.pinned)
                            .disabled(!cfg!(any(target_os = "macos", target_os = "windows")))
                            .tooltip(if cfg!(any(target_os = "macos", target_os = "windows")) {
                                "切换始终置顶"
                            } else {
                                "置顶请使用窗口管理器菜单"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                if platform::set_always_on_top(window, !this.pinned).is_ok() {
                                    this.pinned = !this.pinned;
                                    cx.notify();
                                } else {
                                    this.notify("无法设置置顶。", cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("expand")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Maximize)
                            .tooltip("展开")
                            .on_click(cx.listener(|this, _, window, cx| this.compact(window, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font(timer_font(cx))
                            .text_size(px(32.))
                            .child(clock(seconds)),
                    )
                    .child(controls),
            )
            .when(self.storage_failed, |view| {
                view.child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0x9a3535))
                        .child("数据不可用，请展开重试。"),
                )
            })
    }
}

impl Render for ForceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(700.);
        let body = if self.compact {
            self.mini(cx).into_any_element()
        } else {
            let navigation = if narrow {
                div()
                    .flex()
                    .p_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(
                        svg()
                            .path("force-logo.svg")
                            .size(px(25.))
                            .text_color(rgb(INK)),
                    )
                    .children(
                        [
                            ("narrow-focus", "专注", Page::Focus),
                            ("narrow-records", "记录", Page::Records),
                            ("narrow-settings", "设置", Page::Settings),
                        ]
                        .into_iter()
                        .map(|(id, label, page)| {
                            Button::new(id)
                                .ghost()
                                .small()
                                .label(label)
                                .selected(self.page == page)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.navigate(page, cx)),
                                )
                        }),
                    )
                    .into_any_element()
            } else {
                self.sidebar(cx).into_any_element()
            };
            div()
                .flex()
                .size_full()
                .when(narrow, |view| view.flex_col())
                .child(navigation)
                .child(
                    div()
                        .id("content")
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .h_full()
                        .overflow_y_scroll()
                        .px(if narrow { px(16.) } else { px(40.) })
                        .py_6()
                        .when(self.storage_failed, |view| {
                            view.child(self.storage_notice(cx))
                        })
                        .child(match self.page {
                            Page::Focus => self.focus_page(narrow, cx).into_any_element(),
                            Page::Records => self.records_page(cx).into_any_element(),
                            Page::Settings => self.settings_page(cx).into_any_element(),
                        }),
                )
                .into_any_element()
        };
        div()
            .id("force")
            .track_focus(&self.focus_handle)
            .key_context("Force")
            .size_full()
            .relative()
            .bg(rgb(0xfafaf8))
            .text_color(rgb(INK))
            .text_size(px(14.))
            .on_action(cx.listener(|this, _: &NewTask, window, cx| {
                if this.compact {
                    this.compact(window, cx);
                }
                this.page = Page::Focus;
                this.task_input
                    .update(cx, |input, cx| input.focus(window, cx));
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleCompact, window, cx| this.compact(window, cx)))
            .on_action(cx.listener(|_, _: &Quit, _, cx| cx.quit()))
            .child(body)
            .when(self.toast.is_some(), |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom_4()
                        .right_4()
                        .max_w(px(360.))
                        .px_4()
                        .py_3()
                        .rounded_md()
                        .bg(rgb(0x272727))
                        .text_color(rgb(0xffffff))
                        .text_size(px(13.))
                        .child(
                            self.toast
                                .as_ref()
                                .map(|(message, _)| message.clone())
                                .unwrap_or_default(),
                        ),
                )
            })
    }
}

fn metric(label: &str, value: String) -> Div {
    div()
        .flex_1()
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .child(label.to_owned()),
        )
        .child(
            div()
                .mt_2()
                .text_size(px(28.))
                .font_weight(FontWeight::LIGHT)
                .child(value),
        )
}

fn clock(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn short_time(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    if minutes >= 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

fn timer_font(cx: &App) -> Font {
    let mut value = font(cx.theme().font_family.clone());
    value.features = FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]));
    value
}

fn session_times(session: &Session) -> String {
    let Some(start) = Local.timestamp_opt(session.started_at, 0).single() else {
        return String::new();
    };
    let Some(end) = session
        .finished_at
        .and_then(|time| Local.timestamp_opt(time, 0).single())
    else {
        return String::new();
    };
    if start.date_naive() == end.date_naive() {
        format!("{} – {}", start.format("%H:%M"), end.format("%H:%M"))
    } else {
        format!(
            "{} – {}",
            start.format("%m/%d %H:%M"),
            end.format("%m/%d %H:%M")
        )
    }
}
