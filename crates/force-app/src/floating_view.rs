use std::time::{Duration, Instant};

use force_core::{Session, TagKind};
use gpui::{prelude::*, *};
use gpui_component::{
    Icon, IconName,
    button::{ButtonCustomVariant, ButtonVariants},
};

use crate::{
    label::single_line,
    model::FloatingStrip,
    platform,
    view::{INK, action_icon, button, clock, control_icon, timer_font},
};

pub(crate) const EXPANDED_WIDTH: f32 = platform::STRIP_EXPANDED_WIDTH;
pub(crate) const EXPANDED_HEIGHT: f32 = platform::STRIP_EXPANDED_HEIGHT;
pub(crate) const DOCK_WIDTH: f32 = platform::STRIP_DOCKED_WIDTH;
pub(crate) const DOCK_HEIGHT: f32 = platform::STRIP_DOCKED_HEIGHT;

struct TimerFace {
    title: String,
    status: String,
    seconds: i64,
    running: bool,
    paused: bool,
    completed: bool,
    failed: bool,
    session: Option<i64>,
    progress: Option<f32>,
    ready: bool,
    idle: bool,
}

impl FloatingStrip {
    fn timer_face(&self, cx: &App) -> TimerFace {
        let model = self.model.read(cx);
        let current = if model.completed.is_some() && !model.active.iter().any(Session::is_running)
        {
            None
        } else {
            model.strip_session()
        };
        let completed = current.is_none() && model.completed.is_some();
        let seconds = current.map_or_else(
            || {
                model.completed.as_ref().map_or(
                    if model.countdown {
                        i64::from(model.timer_minutes) * 60
                    } else {
                        0
                    },
                    |s| s.elapsed_at(model.now),
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
        } else if let Some(session) = current.or(model.completed.as_ref()) {
            session.title.clone()
        } else {
            let input = model.task_input.read(cx).value();
            if input.trim().is_empty() {
                "做一件事".to_owned()
            } else {
                input.to_string()
            }
        };
        let paused = current.is_some_and(Session::is_paused);
        let state = if model.storage_failed {
            "打开 Force 重试"
        } else if completed {
            "本轮已保存"
        } else if paused {
            "已暂停"
        } else if current.is_some_and(|s| s.tag_kind == TagKind::Rest) {
            "休息中"
        } else if current.is_some_and(|s| s.target_seconds.is_none()) {
            "已投入"
        } else if current.is_some() {
            "剩余"
        } else {
            "未开始"
        };
        let status = model.toast.as_ref().filter(|_| model.compact).map_or_else(
            || {
                if model.storage_failed || paused {
                    state.to_owned()
                } else if completed {
                    format!("本轮投入 {}", clock(seconds))
                } else if let Some(session) = current {
                    if session.tag_kind == TagKind::Rest {
                        "休息中".to_owned()
                    } else if session.target_seconds.is_some() {
                        "剩余时间".to_owned()
                    } else {
                        "正计时".to_owned()
                    }
                } else if model.countdown {
                    "倒计时 · 未开始".to_owned()
                } else {
                    "正计时 · 随时开始".to_owned()
                }
            },
            |(message, _)| message.clone(),
        );
        TimerFace {
            idle: current.is_none()
                && !completed
                && !model.storage_failed
                && model.task_input.read(cx).value().trim().is_empty(),
            ready: model.can_start_task(cx),
            title,
            status,
            seconds,
            running: current.is_some_and(Session::is_running),
            paused,
            completed,
            failed: model.storage_failed,
            session: current.map(|s| s.id),
            progress: current.and_then(|s| {
                s.target_seconds.map(|target| {
                    (1. - s.elapsed_at(model.now) as f32 / target.max(1) as f32).clamp(0., 1.)
                })
            }),
        }
    }

    pub(crate) fn toggle_expanded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let old_target = if self.expanded { 1. } else { 0. };
        let current = if self.disclosure_task.is_some() {
            let elapsed = (self.disclosure_started.elapsed().as_secs_f32() / 0.16).min(1.);
            self.disclosure_from + (old_target - self.disclosure_from) * ease_in_out(elapsed)
        } else {
            old_target
        };
        self.disclosure_task = None;
        self.disclosure_serial = self.disclosure_serial.wrapping_add(1);
        let serial = self.disclosure_serial;
        self.disclosure_from = current;
        self.disclosure_started = Instant::now();
        self.expanded = !self.expanded;
        self.drawer_visible = true;
        // Grow the native window first; shrink only after the closing fade has
        // finished, so neither direction is clipped by an immediate resize.
        if self.expanded {
            self.reshape(window, cx);
        }
        self.disclosure_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(160))
                .await;
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.disclosure_serial == serial {
                        this.drawer_visible = this.expanded;
                        this.disclosure_from = if this.expanded { 1. } else { 0. };
                        this.disclosure_task = None;
                        this.reshape(window, cx);
                    }
                });
            });
        }));
        cx.notify();
    }

    fn reshape(&self, window: &mut Window, cx: &mut Context<Self>) {
        let dimensions = if self.expanded {
            size(px(EXPANDED_WIDTH), px(EXPANDED_HEIGHT))
        } else if self.docked.is_some() {
            size(px(DOCK_WIDTH), px(DOCK_HEIGHT))
        } else {
            size(px(platform::STRIP_WIDTH), px(platform::STRIP_HEIGHT))
        };
        if let Err(error) = platform::reshape_floating(window, dimensions, self.docked, cx) {
            eprintln!("Force could not reshape the floating timer: {error}");
        }
        cx.notify();
    }

    fn clear_face_press(&mut self) {
        self.drag_start = None;
        #[cfg(target_os = "macos")]
        {
            self.drag_event = None;
        }
    }

    fn begin_face_press(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Dropping an earlier task cancels its release polling. The serial also
        // prevents a queued completion from settling a newer drag.
        self.drag_task = None;
        self.drag_serial = self.drag_serial.wrapping_add(1);
        self.clear_face_press();
        self.drag_start = Some(event.position);
        self.suppress_click = false;
        #[cfg(target_os = "macos")]
        {
            // TestPlatform has no native event, but ordinary click-to-expand
            // must still work there. Only a real drag requires this event.
            self.drag_event = platform::capture_window_drag(_window).ok();
        }
        cx.notify();
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn settle_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_face_press();
        self.disclosure_task = None;
        self.drawer_visible = false;
        self.docked = platform::floating_dock_edge(window);
        self.expanded = false;
        self.reshape(window, cx);
    }

    fn face_clicked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_face_press();
        if std::mem::take(&mut self.suppress_click) {
            return;
        }
        if self.timer_face(cx).idle {
            self.dispatch(cx, |model, window, cx| model.new_task(window, cx));
        } else {
            self.toggle_expanded(window, cx);
        }
    }

    pub(crate) fn model_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.timer_face(cx).idle && self.drawer_visible {
            self.disclosure_task = None;
            self.expanded = false;
            self.drawer_visible = false;
            self.reshape(window, cx);
        }
        cx.notify();
    }

    fn move_face(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(start) = self.drag_start else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.clear_face_press();
            return;
        }
        let delta = event.position - start;
        if f32::from(delta.x).hypot(f32::from(delta.y)) < 4. {
            return;
        }
        self.drag_start = None;
        self.suppress_click = true;
        #[cfg(target_os = "macos")]
        let started = self
            .drag_event
            .take()
            .ok_or_else(|| anyhow::anyhow!("The initiating mouse-down event is unavailable"))
            .and_then(|event| platform::start_window_drag(window, event));
        #[cfg(not(target_os = "macos"))]
        let started = platform::start_window_drag(window);
        if let Err(error) = started {
            eprintln!("Force could not start moving its floating clock: {error}");
            // A gesture beyond the drag threshold must never become a click.
            return;
        }
        // Wayland/X11 movement is asynchronous; retain the current free shape
        // while the compositor owns movement and do not infer its release.
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let serial = self.drag_serial;
            self.drag_task = Some(cx.spawn_in(window, async move |this, cx| {
                #[cfg(target_os = "macos")]
                {
                    // AppKit explicitly permits omitting mouseUp. Query the
                    // button state instead of snapping during the native drag.
                    let deadline = std::time::Instant::now() + Duration::from_secs(60);
                    while platform::window_drag_button_pressed() {
                        if std::time::Instant::now() >= deadline {
                            return;
                        }
                        cx.background_executor()
                            .timer(Duration::from_millis(16))
                            .await;
                    }
                }
                // Windows' SendMessage move loop returns after release. In both
                // cases allow the final native bounds callback to be dispatched.
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let _ = cx.update(|window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if this.drag_serial == serial {
                            this.settle_drag(window, cx);
                        }
                    });
                });
            }));
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = cx;
    }

    fn cat_face(&self, face: &TimerFace, cx: &mut Context<Self>) -> impl IntoElement {
        let (minutes, detail) = compact_time(face.seconds);
        div()
            .id("cat-clock-face")
            .debug_selector(|| "strip-face".into())
            .absolute()
            .left(px(6.))
            .top(px(if self.drag_start.is_some() { 6. } else { 4. }))
            .w(px(76.))
            .h(px(116.))
            .cursor_pointer()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.face_hovered = *hovered;
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::begin_face_press))
            .on_click(cx.listener(|this, _, window, cx| this.face_clicked(window, cx)))
            .child(
                div()
                    .absolute()
                    .left(px(-0.5))
                    .top(px(-0.5))
                    .w(px(77.))
                    .h(px(if self.drawer_visible { 54. } else { 113. }))
                    .overflow_hidden()
                    .child(
                        Icon::default()
                            .path("force-cat-clock.svg")
                            .absolute()
                            .left_0()
                            .top(px(-0.5))
                            .w(px(77.))
                            .h(px(113.))
                            .text_color(rgb(0xb9b9b9)),
                    ),
            )
            .child(
                Icon::default()
                    .path("force-cat-clock.svg")
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(76.))
                    .h(px(112.))
                    .text_color(rgb(if self.drag_start.is_some() {
                        0x353535
                    } else if self.face_hovered {
                        0x242424
                    } else {
                        INK
                    })),
            )
            .child(
                Icon::default()
                    .path("force-cat-eyes.svg")
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(76.))
                    .h(px(112.))
                    .text_color(rgb(0xf5f5f5)),
            )
            .child(
                div()
                    .debug_selector(|| "strip-time".into())
                    .absolute()
                    .top(px(66.))
                    .left(px(19.))
                    .w(px(43.))
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .font(timer_font(cx))
                    .text_size(px(if minutes.len() > 3 {
                        16.
                    } else if minutes.len() > 2 {
                        18.
                    } else {
                        27.
                    }))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if face.paused { 0xb3b3b3 } else { 0xf5f5f5 }))
                    .child(if face.failed {
                        "!".to_owned()
                    } else if face.completed {
                        "✓".to_owned()
                    } else if face.idle {
                        "+".to_owned()
                    } else {
                        minutes
                    }),
            )
            .child(
                div()
                    .absolute()
                    .top(px(96.))
                    .left(px(23.))
                    .w(px(38.))
                    .h(px(14.))
                    .text_center()
                    .text_size(px(10.))
                    .text_color(rgb(0xcccccc))
                    .child(if face.failed {
                        "重试".to_owned()
                    } else if face.completed {
                        "完成".to_owned()
                    } else if face.paused {
                        "暂停".to_owned()
                    } else if face.idle {
                        "新任务".to_owned()
                    } else {
                        detail
                    }),
            )
    }

    fn drawer(&self, face: &TimerFace, cx: &mut Context<Self>) -> Div {
        let primary_label = if face.failed {
            "重试"
        } else if face.completed {
            "查看本轮"
        } else if face.running {
            "暂停"
        } else if face.session.is_some() {
            "继续"
        } else if face.ready {
            "开始"
        } else {
            "新任务"
        };
        let primary_icon = if face.failed {
            Icon::new(IconName::Redo)
        } else if face.completed {
            Icon::new(IconName::ArrowRight)
        } else if !face.running && face.session.is_none() && !face.ready {
            Icon::new(IconName::Plus)
        } else {
            action_icon(if face.running { "pause" } else { "play" })
        };
        let session_id = face.session;
        div()
            .absolute()
            .left(px(14.))
            .top(px(58.))
            .w(px(222.))
            .h(px(86.))
            .rounded(px(20.))
            .bg(rgb(INK))
            .border_1()
            .border_color(rgb(0x383838))
            .child(
                div()
                    .absolute()
                    .left(px(80.))
                    .top(px(12.))
                    .w(px(122.))
                    .h(px(20.))
                    .id("cat-task-details")
                    .debug_selector(|| "strip-open".into())
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.expanded {
                            return;
                        }
                        this.dispatch(cx, move |model, window, cx| {
                            if let Some(id) = session_id {
                                model.show_manager(window, cx);
                                model.select_session(id, window, cx);
                            } else if model.completed.is_some() || model.storage_failed {
                                model.strip_primary_action(window, cx);
                            } else {
                                model.new_task(window, cx);
                            }
                        });
                    }))
                    .child(
                        single_line(face.title.clone(), 13.)
                            .debug_selector(|| "strip-title-text".into())
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xf5f5f5)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(px(80.))
                    .top(px(35.))
                    .w(px(122.))
                    .h(px(15.))
                    .child(
                        single_line(face.status.clone(), 10.)
                            .debug_selector(|| "strip-status".into())
                            .text_color(rgb(if face.failed { 0xe8b3b3 } else { 0xa6a6a6 })),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "strip-controls".into())
                    .absolute()
                    .left(px(80.))
                    .top(px(54.))
                    .h(px(26.))
                    .w(px(128.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        button("bar-primary")
                            .debug_selector(|| "strip-primary".into())
                            .w(px(82.))
                            .h(px(26.))
                            .rounded(px(8.))
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(rgb(0xf2f2f2).into())
                                    .foreground(rgb(INK).into())
                                    .border(rgba(0x00000000).into())
                                    .hover(rgb(0xffffff).into())
                                    .active(rgb(0xd8d8d8).into()),
                            )
                            .icon(control_icon(primary_icon.size(px(11.)), INK))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(INK))
                                    .child(primary_label),
                            )
                            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                if this.expanded && event.click_count() == 1 {
                                    this.dispatch(cx, |model, window, cx| {
                                        model.strip_primary_action(window, cx)
                                    });
                                }
                            })),
                    )
                    .children(face.session.map(|id| {
                        button("bar-finish")
                            .debug_selector(|| "strip-finish".into())
                            .w(px(36.))
                            .h(px(26.))
                            .rounded(px(7.))
                            .custom(cat_secondary_variant(cx))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(0xd2d2d2))
                                    .child("结束"),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.expanded {
                                    this.dispatch(cx, move |model, _, cx| model.finish(id, cx))
                                }
                            }))
                    })),
            )
    }

    fn dock_face(&self, face: &TimerFace, cx: &mut Context<Self>) -> impl IntoElement {
        let (minutes, _) = compact_time(face.seconds);
        div()
            .id("cat-clock-dock")
            .debug_selector(|| "strip-dock".into())
            .size_full()
            .rounded(px(12.))
            .bg(rgb(INK))
            .border_1()
            .border_color(rgb(0x777777))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(7.))
            .cursor_pointer()
            .text_color(rgb(0xf5f5f5))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::begin_face_press))
            .on_click(cx.listener(|this, _, window, cx| this.face_clicked(window, cx)))
            .child(
                div()
                    .font(timer_font(cx))
                    .text_size(px(if minutes.len() > 2 { 10. } else { 15. }))
                    .child(if face.failed {
                        "!".to_owned()
                    } else if face.completed {
                        "✓".to_owned()
                    } else if face.idle {
                        "+".to_owned()
                    } else {
                        minutes
                    }),
            )
            .child(if face.paused {
                Icon::default()
                    .path("force-icons/pause.svg")
                    .size(px(10.))
                    .into_any_element()
            } else {
                div()
                    .h(px(22.))
                    .w(px(2.))
                    .bg(rgb(0x535353))
                    .child(
                        div()
                            .w_full()
                            .h(relative(face.progress.unwrap_or(1.)))
                            .bg(rgb(0xf5f5f5)),
                    )
                    .into_any_element()
            })
    }
}

impl Render for FloatingStrip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let face = self.timer_face(cx);
        let move_face = cx.listener(Self::move_face);
        let release_face = cx.listener(|this, _: &MouseUpEvent, _, cx| {
            this.clear_face_press();
            cx.notify();
        });
        div()
            .debug_selector(|| "strip-root".into())
            .track_focus(&self.focus_handle)
            .key_context("Force")
            .size_full()
            .relative()
            .text_color(rgb(INK))
            .child(if self.docked.is_some() && !self.drawer_visible {
                self.dock_face(&face, cx).into_any_element()
            } else {
                div()
                    .size_full()
                    .relative()
                    .when(self.drawer_visible, |body| {
                        let from = self.disclosure_from;
                        let target = if self.expanded { 1. } else { 0. };
                        body.child(self.drawer(&face, cx).with_animation(
                            ElementId::NamedInteger(
                                "cat-disclosure".into(),
                                self.disclosure_serial,
                            ),
                            Animation::new(Duration::from_millis(160)).with_easing(ease_in_out),
                            move |panel, delta| {
                                let reveal = from + (target - from) * delta;
                                panel.opacity(reveal).left(px(14. - 12. * (1. - reveal)))
                            },
                        ))
                    })
                    .child(self.cat_face(&face, cx))
                    .into_any_element()
            })
            .child(
                // Div mouse-move handlers require a hovered hitbox. Track the
                // initiating gesture at window level so a fast movement outside
                // the cat silhouette can still cross the drag threshold.
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase.bubble() {
                                move_face(event, window, cx);
                            }
                        });
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase.bubble() {
                                release_face(event, window, cx);
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

fn cat_secondary_variant(cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(rgba(0x00000000).into())
        .foreground(rgb(0xd2d2d2).into())
        .border(rgba(0x00000000).into())
        .hover(rgb(0x303030).into())
        .active(rgb(0x454545).into())
}

fn compact_time(seconds: i64) -> (String, String) {
    let seconds = seconds.max(0);
    if seconds < 3600 {
        (
            format!("{:02}", seconds / 60),
            format!("{:02}", seconds % 60),
        )
    } else {
        let hours = seconds / 3600;
        (
            if hours > 99 {
                "99h+".to_owned()
            } else {
                format!("{hours}h")
            },
            format!("{:02}:{:02}", seconds % 3600 / 60, seconds % 60),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::compact_time;

    #[test]
    fn compact_clock_keeps_subhour_seconds_and_hour_meaning() {
        assert_eq!(compact_time(1122), ("18".into(), "42".into()));
        assert_eq!(compact_time(3607), ("1h".into(), "00:07".into()));
        assert_eq!(compact_time(-1), ("00".into(), "00".into()));
        assert_eq!(compact_time(i64::MAX).0, "99h+");
    }
}
