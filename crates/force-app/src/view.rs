use chrono::{Datelike, Local, TimeZone};
use force_core::{Session, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName,
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

pub(crate) const INK: u32 = 0x141414;
pub(crate) const MUTED: u32 = 0x626262;
pub(crate) const LINE: u32 = 0xe3e3e3;
pub(crate) const PAPER: u32 = 0xfafafa;
pub(crate) const SURFACE: u32 = 0xf0f0f0;
pub(crate) const DESTRUCTIVE: u32 = 0x963f3f;

pub(crate) const TASK_SIDEBAR_WIDTH: f32 = 240.;
pub(crate) const TASK_TEXT_WIDTH: f32 = TASK_SIDEBAR_WIDTH - 26.;

impl ForceView {
    pub(crate) fn current_session(&self) -> Option<&Session> {
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

    /// Browsing and timing are independent; expose the timer that keeps running.
    pub(crate) fn timer_context(&self, cx: &mut Context<Self>) -> Option<Div> {
        let session = self.active.iter().find(|session| session.is_running())?;
        let id = session.id;
        let title = session.title.clone();
        Some(
            div()
                .debug_selector(|| "timer-context".into())
                .w_full()
                .pb(px(12.))
                .border_b_1()
                .border_color(rgb(LINE))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    single_line(format!("正在计时 · {title}"), 14.)
                        .flex_1()
                        .min_w_0()
                        .text_color(rgb(MUTED)),
                )
                .child(
                    button("return-current-timer")
                        .debug_selector(|| "return-current-timer".into())
                        .h(px(32.))
                        .flex_shrink_0()
                        .child(control_label("返回计时", INK))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_session(id, window, cx);
                        })),
                ),
        )
    }

    pub(crate) fn browsing_other_timer(&self) -> bool {
        self.active.iter().any(|session| {
            session.is_running()
                && (self.preparing
                    || self.completed.is_some()
                    || self.selected_session != Some(session.id))
        })
    }

    fn panel_navigation(&self, cx: &mut Context<Self>) -> Div {
        div()
            .debug_selector(|| "manager-header".into())
            .h(px(68.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(LINE))
            .child(
                div()
                    .h_full()
                    .w_full()
                    .max_w(px(976.))
                    .mx_auto()
                    .px(px(40.))
                    .flex()
                    .items_center()
                    .child(
                        Icon::default()
                            .path("force-wordmark.svg")
                            .w(px(114.))
                            .h(px(24.))
                            .text_color(rgb(INK)),
                    )
                    .child(
                        div()
                            .ml(px(36.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .children(
                                [
                                    ("agenda-tab", "日历", Page::Agenda),
                                    ("records-tab", "记录", Page::Records),
                                ]
                                .into_iter()
                                .map(|(id, label, page)| {
                                    underline_button(id, self.page == page, false, cx)
                                        .debug_selector(move || id.into())
                                        .h(px(36.))
                                        .px(px(16.))
                                        .when(self.page == page, |button| {
                                            button.font_weight(FontWeight::SEMIBOLD)
                                        })
                                        .child(control_label(
                                            label,
                                            if self.page == page { INK } else { MUTED },
                                        ))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if this.page != page {
                                                this.navigate(page, window, cx);
                                            }
                                        }))
                                }),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        choice_button("settings-tab", self.page == Page::Settings, SURFACE, cx)
                            .debug_selector(|| "settings-tab".into())
                            .size(px(36.))
                            .icon(control_icon(
                                Icon::new(IconName::Settings).size(px(18.)),
                                if self.page == Page::Settings {
                                    INK
                                } else {
                                    MUTED
                                },
                            ))
                            .tooltip("设置")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate(Page::Settings, window, cx)
                            })),
                    ),
            )
    }

    fn feedback_toast(&self, message: &str, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "manager-feedback".into())
            .absolute()
            .bottom(px(16.))
            .right(px(24.))
            .max_w(px(360.))
            .p(px(12.))
            .rounded(px(6.))
            .bg(rgb(SURFACE))
            .border_1()
            .border_color(rgb(LINE))
            .flex()
            .items_center()
            .gap(px(12.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(14.))
                    .whitespace_normal()
                    .child(message.to_owned()),
            )
            .child(
                button("dismiss-feedback")
                    .ghost()
                    .size(px(24.))
                    .icon(control_icon(IconName::Close, MUTED))
                    .tooltip("关闭提示")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toast = None;
                        cx.notify();
                    })),
            )
    }

    fn management_page(
        &self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.page == Page::Agenda {
            return self.agenda_panel(window, cx).into_any_element();
        }
        if self.page == Page::Focus {
            return self.focus_panel(compact, cx).into_any_element();
        }
        let scroll = &self.panel_scroll[match self.page {
            Page::Agenda | Page::Focus => 0,
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
                    .pt(px(28.))
                    .pb(px(48.))
                    .child(
                        div()
                            .debug_selector(|| "page-heading".into())
                            .mb(px(24.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(page_heading(match self.page {
                                Page::Agenda => "日历",
                                Page::Focus => "计时",
                                Page::Records => "记录",
                                Page::Settings => "设置",
                            }))
                            .when(self.page == Page::Records, |row| {
                                row.child(
                                    button("export-records")
                                        .debug_selector(|| "export-records".into())
                                        .ghost()
                                        .h(px(36.))
                                        .icon(control_icon(
                                            Icon::new(IconName::ArrowDown).size(px(14.)),
                                            MUTED,
                                        ))
                                        .unavailable(self.export_busy || self.storage_failed)
                                        .child(control_label(
                                            if self.export_busy {
                                                "正在导出…"
                                            } else {
                                                "导出全部"
                                            },
                                            MUTED,
                                        ))
                                        .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                                )
                            }),
                    )
                    .when(self.storage_failed, |body| {
                        body.child(self.storage_notice(cx))
                    })
                    .when(!self.storage_failed, |body| {
                        body.child(match self.page {
                            Page::Agenda => self.agenda_panel(window, cx).into_any_element(),
                            Page::Focus => self.focus_panel(compact, cx).into_any_element(),
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
        self.sync_agenda_tag_picker(window, cx);
        let viewport = window.viewport_size();
        let compact = viewport.width < px(920.) || viewport.height < px(700.);
        div()
            .id("force-manager")
            .track_focus(&self.focus_handle)
            .key_context("Force")
            .size_full()
            .relative()
            .bg(rgb(PAPER))
            .text_color(rgb(INK))
            .text_size(px(14.))
            .flex()
            .flex_col()
            .child(self.panel_navigation(cx))
            .child(self.management_page(compact, window, cx))
            .when_some(self.toast.as_ref(), |body, (message, _)| {
                body.child(self.feedback_toast(message, cx))
            })
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
                "安排一件事".to_owned()
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
                "计时中"
            }
        } else if completed.is_some_and(|s| s.tag_kind == TagKind::Rest) {
            "休息结束"
        } else if completed.is_some() {
            "已保存"
        } else {
            "未开始"
        };
        let running = current.is_some_and(Session::is_running);
        let (symbol, help) = if model.storage_failed {
            ("play", "展开并重试")
        } else if completed.is_some() && current.is_none() {
            ("play", "查看计时")
        } else if running {
            ("pause", "暂停计时")
        } else if current.is_some() {
            ("play", "继续计时")
        } else {
            ("play", "开始计时")
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
            .px(px(12.))
            .py(px(3.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .id("floating-title-drag")
                    .debug_selector(|| "strip-title".into())
                    .w_full()
                    .h(px(18.))
                    .cursor_move()
                    .tooltip(move |window, cx| {
                        Tooltip::new(format!("{title_help} · 拖动以移动浮窗")).build(window, cx)
                    })
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        let _ = platform::start_window_drag(window);
                    })
                    .child(
                        single_line(title, 12.)
                            .debug_selector(|| "strip-title-text".into())
                            .font_weight(FontWeight::MEDIUM),
                    ),
            )
            .child(
                div()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .id("floating-time-drag")
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .cursor_move()
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                let _ = platform::start_window_drag(window);
                            })
                            .child(
                                div()
                                    .debug_selector(|| "strip-time".into())
                                    .flex_shrink_0()
                                    .font(timer_font(cx))
                                    .text_size(px(if seconds >= 3600 { 20. } else { 24. }))
                                    .line_height(relative(1.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(if current.is_some_and(Session::is_paused) {
                                        MUTED
                                    } else {
                                        INK
                                    }))
                                    .child(strip_time(seconds)),
                            )
                            .child(
                                single_line(state, 11.)
                                    .id("bar-status")
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(rgb(if model.storage_failed {
                                        DESTRUCTIVE
                                    } else {
                                        MUTED
                                    }))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(state_help.clone()).build(window, cx)
                                    }),
                            ),
                    )
                    .child(
                        primary_button("bar-primary")
                            .debug_selector(|| "strip-primary".into())
                            .size(px(28.))
                            .rounded(px(4.))
                            .icon(control_icon(
                                if current.is_none() && completed.is_some() {
                                    Icon::new(IconName::ArrowRight).size(px(14.))
                                } else {
                                    action_icon(symbol).size(px(14.))
                                },
                                0xffffff,
                            ))
                            .tooltip(shortcut(help, "P", "P"))
                            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                if event.click_count() == 1 {
                                    this.dispatch(cx, |model, window, cx| {
                                        model.strip_primary_action(window, cx)
                                    });
                                }
                            })),
                    )
                    .child(
                        div()
                            .debug_selector(|| "strip-finish-slot".into())
                            .w(px(28.))
                            .h(px(28.))
                            .flex_shrink_0()
                            .children(current.map(|session| {
                                let id = session.id;
                                button("bar-finish")
                                    .debug_selector(|| "strip-finish".into())
                                    .ghost()
                                    .size(px(28.))
                                    .icon(control_icon(action_icon("stop").size(px(13.)), INK))
                                    .tooltip(shortcut("结束本次计时", "⇧P", "Shift P"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.dispatch(cx, move |model, _, cx| model.finish(id, cx));
                                    }))
                            })),
                    )
                    .child(
                        button("bar-expand")
                            .debug_selector(|| "strip-open".into())
                            .ghost()
                            .size(px(28.))
                            .icon(control_icon(
                                Icon::new(IconName::ExternalLink).size(px(14.)),
                                MUTED,
                            ))
                            .tooltip("打开 Force")
                            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                if event.click_count() == 1 {
                                    this.dispatch(cx, |model, window, cx| {
                                        model.show_manager(window, cx)
                                    });
                                }
                            })),
                    ),
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
                    .rounded(px(8.))
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

fn strip_time(seconds: i64) -> AnyElement {
    let value: SharedString = clock(seconds).into();
    if value.len() <= 10 {
        return value.into_any_element();
    }
    // Preserve every hour digit even for extreme stored values without painting
    // over the neighboring status and actions. Ordinary times keep natural width.
    canvas(
        move |bounds, window, _| {
            let style = window.text_style();
            crate::focus_view::shape_timer(
                value,
                bounds.size.width,
                &style,
                px(20.),
                window.text_system(),
            )
        },
        |bounds, line, window, cx| {
            let _ = line.paint(bounds.origin, px(20.), window, cx);
        },
    )
    .w(px(150.))
    .h(px(20.))
    .into_any_element()
}

pub(crate) fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id)
        .text_size(px(14.))
        .rounded(px(6.))
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

// Ghost variants clear their border on hover/press. Keep the selected underline
// in the variant so it remains visible immediately after clicking a choice.
pub(crate) fn underline_button(
    id: impl Into<ElementId>,
    selected: bool,
    baseline: bool,
    cx: &App,
) -> Button {
    button(id)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(rgba(0x00000000).into())
                .foreground(rgb(if selected { INK } else { MUTED }).into())
                .border(if selected {
                    rgb(INK).into()
                } else if baseline {
                    rgb(LINE).into()
                } else {
                    rgba(0x00000000).into()
                })
                .hover(rgb(SURFACE).into())
                .active(rgb(0xe9e9e9).into()),
        )
        .rounded(px(0.))
        .border_0()
        .border_b_2()
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
        .rounded(px(6.))
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

pub(crate) fn action_icon(name: &str) -> Icon {
    Icon::default().path(format!("force-icons/{name}.svg"))
}
fn page_heading(title: &str) -> Div {
    div()
        .text_size(px(32.))
        .line_height(relative(1.2))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.to_owned())
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
pub(crate) fn timer_font(cx: &App) -> Font {
    let mut font = font(cx.theme().font_family.clone());
    font.features = FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]));
    font
}
pub(crate) fn clock(seconds: i64) -> String {
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
pub(crate) fn short_time(seconds: i64) -> String {
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
pub(crate) fn session_times(session: &Session) -> String {
    let start = Local.timestamp_opt(session.started_at, 0).single();
    let end = session
        .finished_at
        .and_then(|at| Local.timestamp_opt(at, 0).single());
    let format = match (start, end) {
        (Some(a), Some(b)) if a.year() != b.year() => "%Y/%m/%d %H:%M",
        (Some(a), Some(b)) if a.date_naive() == b.date_naive() => "%H:%M",
        _ => "%m/%d %H:%M",
    };
    let from = start.map_or_else(
        || "时间不可用".to_owned(),
        |at| at.format(format).to_string(),
    );
    let to = if session.finished_at.is_none() {
        if session.is_running() {
            "现在"
        } else {
            "已暂停"
        }
        .to_owned()
    } else {
        end.map_or_else(
            || "时间不可用".to_owned(),
            |at| at.format(format).to_string(),
        )
    };
    format!("{from}–{to}")
}

pub(crate) fn record_timestamp(timestamp: i64) -> String {
    Local.timestamp_opt(timestamp, 0).single().map_or_else(
        || "时间不可用".to_owned(),
        |time| time.format("%Y/%m/%d %H:%M:%S").to_string(),
    )
}

pub(crate) fn record_detail(label: &str, value: String) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .text_size(px(13.))
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

pub(crate) fn task_heading(title: &str) -> impl IntoElement {
    let label = title.to_owned();
    let help = label.clone();
    div()
        .id("task-heading")
        .debug_selector(|| "focus-task-title".into())
        .text_size(px(28.))
        .font_weight(FontWeight::MEDIUM)
        .line_height(relative(1.35))
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

#[cfg(test)]
mod record_tests {
    use super::session_times;
    use chrono::{Local, TimeZone};

    #[test]
    fn record_time_ranges_show_dates_across_midnight_and_years() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = force_core::Store::open(directory.path().join("force.sqlite3")).unwrap();
        let at = |y, m, d, h, min| {
            Local
                .with_ymd_and_hms(y, m, d, h, min, 0)
                .earliest()
                .unwrap()
                .timestamp()
        };
        for (start, end, expected) in [
            (
                at(2026, 10, 22, 20, 52),
                at(2026, 10, 23, 1, 19),
                "10/22 20:52–10/23 01:19",
            ),
            (
                at(2026, 12, 31, 23, 50),
                at(2027, 1, 1, 0, 10),
                "2026/12/31 23:50–2027/01/01 00:10",
            ),
            (
                at(2026, 10, 22, 9, 0),
                at(2026, 10, 22, 10, 0),
                "09:00–10:00",
            ),
        ] {
            let session = store.start("Time range", 1, start).unwrap();
            let session = store.finish(session.id, end).unwrap();
            assert_eq!(session_times(&session), expected);
        }
    }
}
