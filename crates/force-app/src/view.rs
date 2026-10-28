use chrono::{Datelike, Local, TimeZone};
use force_core::Session;
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
    model::{ForceView, Page},
};

pub(crate) const INK: u32 = 0x141414;
pub(crate) const MUTED: u32 = 0x626262;
// Surface and interaction roles are deliberately separate: a divider should
// not look like an input border, and selection should not resemble disabled UI.
pub(crate) const LINE: u32 = 0xe8e8e8;
pub(crate) const CONTROL_LINE: u32 = 0xc9c9c9;
pub(crate) const PAPER: u32 = 0xffffff;
pub(crate) const CHROME: u32 = 0xfafafa;
pub(crate) const SURFACE: u32 = 0xf7f7f7;
pub(crate) const HOVER: u32 = 0xf2f2f2;
pub(crate) const PRESSED: u32 = 0xe7e7e7;
pub(crate) const DESTRUCTIVE: u32 = 0x963f3f;

pub(crate) const TASK_TAG_WIDTH: f32 = 168.;
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
                    outlined_button("return-current-timer")
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
            .bg(rgb(CHROME))
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
                        img(crate::embedded_image_source("force-wordmark.png"))
                            .w(px(96.))
                            .h(px(20.)),
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
                                    ("analysis-tab", "分析", Page::Analysis),
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
                                            if page == Page::Agenda || this.page != page {
                                                this.navigate(page, window, cx);
                                            }
                                        }))
                                }),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        button("global-new-moment")
                            .debug_selector(|| "global-new-moment".into())
                            .custom(quiet_variant(cx))
                            .h(px(36.))
                            .px(px(10.))
                            .mr(px(8.))
                            .flex_shrink_0()
                            .child(control_label("随手记", INK))
                            .tooltip(shortcut("随手记", "Shift+N", "Shift+N"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_moment(None, window, cx)
                            })),
                    )
                    .child(
                        primary_button("global-new-task")
                            .debug_selector(|| "global-new-task".into())
                            .h(px(36.))
                            .px(px(12.))
                            .mr(px(12.))
                            .flex_shrink_0()
                            .icon(control_icon(Icon::new(IconName::Plus).size(px(14.)), PAPER))
                            .child(control_label("新任务", PAPER))
                            .tooltip(shortcut("新任务", "N", "N"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_task(window, cx);
                            })),
                    )
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
                    .custom(crate::view::quiet_variant(cx))
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
            Page::Analysis => 3,
        }];
        div()
            .debug_selector(|| "manager-viewport".into())
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                div()
                    .id("manager-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(scroll)
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
                                        Page::Analysis => "分析",
                                    }))
                                    .when(self.page == Page::Records, |row| {
                                        row.child(
                                            button("export-records")
                                                .debug_selector(|| "export-records".into())
                                                .custom(crate::view::quiet_variant(cx))
                                                .h(px(36.))
                                                .icon(control_icon(
                                                    Icon::new(IconName::ArrowDown).size(px(14.)),
                                                    MUTED,
                                                ))
                                                .unavailable(
                                                    self.export_busy || self.storage_failed,
                                                )
                                                .child(control_label(
                                                    if self.export_busy {
                                                        "正在导出…"
                                                    } else {
                                                        "导出计时"
                                                    },
                                                    MUTED,
                                                ))
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.export(window, cx)
                                                })),
                                        )
                                    }),
                            )
                            .when(self.storage_failed, |body| {
                                body.child(self.storage_notice(cx))
                            })
                            .when(!self.storage_failed, |body| {
                                body.child(match self.page {
                                    Page::Agenda => {
                                        self.agenda_panel(window, cx).into_any_element()
                                    }
                                    Page::Focus => self.focus_panel(compact, cx).into_any_element(),
                                    Page::Records => {
                                        self.records_panel(window, cx).into_any_element()
                                    }
                                    Page::Analysis => {
                                        self.analysis_panel(window, cx).into_any_element()
                                    }
                                    Page::Settings => self
                                        .settings_panel(
                                            window.viewport_size().width >= px(920.),
                                            cx,
                                        )
                                        .into_any_element(),
                                })
                            }),
                    ),
            )
            .vertical_scrollbar(scroll)
            .into_any_element()
    }
}

impl Render for ForceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_tag_picker(window, cx);
        self.sync_record_editor_picker(window, cx);
        self.sync_completion_editor_picker(window, cx);
        self.sync_agenda_tag_picker(window, cx);
        let viewport = window.viewport_size();
        // Shape activity labels to the available task-operation column.
        self.tag_picker.update(cx, |picker, cx| {
            let width = (f32::from(viewport.width).min(976.) - 80. - TASK_SIDEBAR_WIDTH - 32. - 4.)
                .max(260.);
            picker.set_trigger_width(width, cx);
        });
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
            .when(self.moments.visible, |body| {
                body.child(self.moment_editor(window, cx))
            })
            .when(self.record_editor.visible, |body| {
                body.child(self.record_editor(window, cx))
            })
            .when(self.completion_editor.visible, |body| {
                body.child(self.completion_editor(window, cx))
            })
            .when(self.time_picker.read(cx).is_open(), |body| {
                body.child(self.time_picker.clone())
            })
    }
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
            .hover(rgb(HOVER).into())
            .active(rgb(PRESSED).into()),
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
                .hover(rgb(HOVER).into())
                .active(rgb(PRESSED).into()),
        )
        .rounded(px(0.))
        .border_0()
        .border_b_2()
}

pub(crate) fn quiet_variant(cx: &App) -> ButtonCustomVariant {
    // The library's Ghost darkens the base by a fixed amount, independently of
    // secondary_hover. Use the same hover/press roles on every quiet action.
    ButtonCustomVariant::new(cx)
        .color(rgba(0x00000000).into())
        .foreground(rgb(INK).into())
        .border(rgba(0x00000000).into())
        .hover(rgb(HOVER).into())
        .active(rgb(PRESSED).into())
}

pub(crate) fn outlined_button(id: impl Into<ElementId>) -> Button {
    // Keep the library's pointer/focus behavior, without its default shadow.
    button(id).outline().shadow(Vec::new())
}

pub(crate) fn selection_button(id: impl Into<ElementId>, selected: bool, cx: &App) -> Button {
    button(id)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(rgb(if selected { PAPER } else { SURFACE }).into())
                .foreground(rgb(if selected { INK } else { MUTED }).into())
                .border(if selected {
                    rgb(CONTROL_LINE).into()
                } else {
                    rgba(0x00000000).into()
                })
                .hover(rgb(HOVER).into())
                .active(rgb(PRESSED).into()),
        )
        .when(selected, |button| button.font_weight(FontWeight::MEDIUM))
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
        self.disabled(disabled)
            .when(disabled, |button| button.cursor_default())
    }
}

pub(crate) fn action_icon(name: &str) -> Icon {
    Icon::default().path(format!("force-icons/{name}.svg"))
}
pub(crate) fn page_heading(title: &str) -> Div {
    div()
        .text_size(px(26.))
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

pub(crate) fn shortcut(label: &str, mac: &str, other: &str) -> String {
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
