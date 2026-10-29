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
const CAT_CLICK_DURATION: Duration = Duration::from_millis(480);
const DISCLOSURE_DURATION: Duration = Duration::from_millis(160);

struct TimerFace {
    title: String,
    title_help: String,
    status: String,
    seconds: i64,
    running: bool,
    paused: bool,
    completed: bool,
    repeatable: bool,
    failed: bool,
    invalid_duration: bool,
    session: Option<i64>,
    progress: Option<f32>,
    ready: bool,
    idle: bool,
}

impl TimerFace {
    fn show_paused_mark(&self) -> bool {
        self.paused && !self.failed
    }

    fn dock_progress(&self) -> Option<f32> {
        self.progress.filter(|_| !self.failed && !self.paused)
    }

    fn compact_display(&self) -> (String, String) {
        if self.failed {
            ("重试".to_owned(), String::new())
        } else if self.invalid_duration {
            ("设置时长".to_owned(), String::new())
        } else if self.idle {
            ("开始".to_owned(), String::new())
        } else {
            let detail = if self.completed {
                "已保存"
            } else if self.paused {
                "已暂停"
            } else if self.running {
                if self.progress.is_some() {
                    "剩余"
                } else {
                    "正计时"
                }
            } else {
                "待开始"
            };
            (compact_clock(self.seconds), detail.to_owned())
        }
    }
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
        let resolved_title = model.resolved_task_title(cx);
        let has_task_draft = model.selected_task.is_some() || model.new_task_has_draft(cx);
        let invalid_duration = !model.storage_failed
            && current.is_none()
            && !completed
            && has_task_draft
            && model.timer_duration_error(cx).is_some();
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
            activity_task_label(&session.tag_name, &session.title)
        } else if resolved_title.is_empty() {
            if has_task_draft {
                "新任务"
            } else {
                "做一件事"
            }
            .to_owned()
        } else {
            activity_task_label(
                model.selected_activity_name().unwrap_or_default(),
                &resolved_title,
            )
        };
        let paused = current.is_some_and(Session::is_paused);
        let activity = current.or(model.completed.as_ref()).map_or_else(
            || model.selected_activity_name().unwrap_or_default(),
            |session| session.tag_name.as_str(),
        );
        let activity_detail = if activity == title { "" } else { activity };
        let title_help = if activity_detail.is_empty() {
            title.clone()
        } else {
            format!("{activity} · {title}")
        };
        let status = if invalid_duration {
            "设置时长".to_owned()
        } else {
            model.toast.as_ref().filter(|_| model.compact).map_or_else(
                || {
                    if model.storage_failed {
                        "打开 Force 重试".to_owned()
                    } else if completed || current.is_some() {
                        // Time and timer state already have a stable place below
                        // the cat. Use the drawer for the work being timed.
                        activity_detail.to_owned()
                    } else if has_task_draft && model.task_tag_error().is_some() {
                        "选择活动".to_owned()
                    } else if model.task_title_error(cx).is_some() {
                        "检查说明".to_owned()
                    } else if model.task_has_unsaved_changes(cx) {
                        "修改待保存".to_owned()
                    } else if model.countdown {
                        "倒计时".to_owned()
                    } else {
                        "正计时".to_owned()
                    }
                },
                |(message, _)| message.clone(),
            )
        };
        TimerFace {
            idle: current.is_none() && !completed && !model.storage_failed && !has_task_draft,
            ready: model.can_start_task(cx),
            title,
            title_help,
            status,
            seconds,
            running: current.is_some_and(Session::is_running),
            paused,
            completed,
            repeatable: completed
                && model
                    .completed
                    .as_ref()
                    .is_some_and(|session| session.tag_kind == TagKind::Focus),
            failed: model.storage_failed,
            invalid_duration,
            session: current.map(|s| s.id),
            progress: current.and_then(|s| {
                s.target_seconds.map(|target| {
                    (1. - s.elapsed_at(model.now) as f32 / target.max(1) as f32).clamp(0., 1.)
                })
            }),
        }
    }

    fn drawer_controls_ready(&self) -> bool {
        self.expanded
            && self.drawer_visible
            && self.disclosure_task.is_none()
            && self.drawer_painted_serial == Some(self.disclosure_serial)
    }

    pub(crate) fn toggle_expanded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let old_target = if self.expanded { 1. } else { 0. };
        let current = if self.disclosure_task.is_some() {
            disclosure_reveal(
                self.disclosure_from,
                old_target,
                self.disclosure_started,
                Instant::now(),
            )
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
            cx.background_executor().timer(DISCLOSURE_DURATION).await;
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
        // A new gesture interrupts the old blink; a drag cannot replay it.
        self.face_click_started = None;
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
        self.face_click_serial = self.face_click_serial.wrapping_add(1);
        self.face_click_started = Some(Instant::now());
        cx.notify();
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
        let (readout, detail) = face.compact_display();
        // The image and eye patch share one scale. Keep all typography below
        // the paws so neither the coat nor the timer becomes a text backdrop.
        let scale = 62. / 76.;
        let blink = div()
            .absolute()
            .left(px(13. * scale))
            .top(px(24. * scale))
            .w(px(34. * scale))
            .h(px(16. * scale))
            .overflow_hidden()
            .child(
                img(crate::embedded_image_source("force-cat-blink.png"))
                    .absolute()
                    .left(px(-13. * scale))
                    .top(px(-24. * scale))
                    .w(px(62.))
                    .h(px(112. * scale)),
            );
        let blink = if cat_click_active(self.face_click_started, Instant::now()) {
            blink
                .with_animation(
                    ElementId::NamedInteger("cat-blink".into(), self.face_click_serial),
                    Animation::new(CAT_CLICK_DURATION),
                    |patch, progress| patch.opacity(cat_blink_opacity(progress)),
                )
                .into_any_element()
        } else {
            blink.opacity(0.).into_any_element()
        };
        let readout_size = if readout.chars().count() > 8 {
            11.5
        } else {
            13.
        };
        let has_detail = !detail.is_empty();
        div()
            .id("cat-clock-face")
            .debug_selector(|| "strip-face".into())
            .absolute()
            .left(px(6.))
            .top(px(4.))
            .w(px(76.))
            .h(px(122.))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(Self::begin_face_press))
            .on_click(cx.listener(|this, _, window, cx| this.face_clicked(window, cx)))
            .child(
                div()
                    .debug_selector(|| "strip-artwork".into())
                    .absolute()
                    .left(px(7.))
                    .top_0()
                    .w(px(62.))
                    .h(px(112. * scale))
                    .child(img(crate::embedded_image_source("force-cat-clock.png")).size_full())
                    .child(blink),
            )
            .child(
                div()
                    .debug_selector(|| "strip-readout-plate".into())
                    .absolute()
                    .left_0()
                    .top(px(92.))
                    .w(px(76.))
                    .h(px(30.))
                    .rounded(px(8.))
                    .bg(rgb(0xf7f7f7))
                    .border_1()
                    .border_color(rgb(0xd5d5d5))
                    .child(
                        div()
                            .debug_selector(|| "strip-time".into())
                            .absolute()
                            .left(px(3.))
                            .top(px(if has_detail { 0. } else { 6. }))
                            .w(px(68.))
                            .h(px(17.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .font(timer_font(cx))
                            .text_size(px(readout_size))
                            .line_height(px(17.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(INK))
                            .child(
                                div()
                                    .debug_selector(|| "strip-time-text".into())
                                    .flex_none()
                                    .child(readout),
                            ),
                    )
                    .when(has_detail, |plate| {
                        plate.child(
                            div()
                                .debug_selector(|| "strip-detail".into())
                                .absolute()
                                .left(px(3.))
                                .top(px(17.))
                                .w(px(68.))
                                .h(px(12.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(4.))
                                .text_size(px(11.))
                                .line_height(px(12.))
                                .text_color(rgb(0x575757))
                                .when(face.show_paused_mark(), |detail| {
                                    detail.child(
                                        div()
                                            .debug_selector(|| "strip-paused-mark".into())
                                            .w(px(5.))
                                            .h(px(6.))
                                            .flex_none()
                                            .flex()
                                            .justify_between()
                                            .child(div().w(px(1.5)).h_full().bg(rgb(0x575757)))
                                            .child(div().w(px(1.5)).h_full().bg(rgb(0x575757))),
                                    )
                                })
                                .child(
                                    div()
                                        .debug_selector(|| "strip-detail-text".into())
                                        .flex_none()
                                        .child(detail),
                                ),
                        )
                    }),
            )
    }

    fn drawer(&self, face: &TimerFace, cx: &mut Context<Self>) -> Div {
        let primary_label = if face.failed {
            "打开 Force"
        } else if face.repeatable {
            "再计一次"
        } else if face.completed {
            "查看详情"
        } else if face.running {
            "暂停"
        } else if face.session.is_some() {
            "继续"
        } else if face.ready {
            "开始"
        } else {
            "查看任务"
        };
        let primary_icon = if face.failed
            || (face.completed && !face.repeatable)
            || (!face.completed && !face.running && face.session.is_none() && !face.ready)
        {
            Icon::new(IconName::ArrowRight)
        } else {
            action_icon(if face.running { "pause" } else { "play" })
        };
        let label_width = if face.failed {
            56.
        } else if face.completed || (!face.ready && face.session.is_none()) {
            44.
        } else {
            22.
        };
        let session_id = if face.failed { None } else { face.session };
        let opens_task = face.failed || (!face.completed && face.session.is_none() && !face.ready);
        let title_help = face.title_help.clone();
        div()
            .debug_selector(|| "strip-drawer".into())
            .absolute()
            .left(px(64.))
            .top(px(42.))
            .w(px(172.))
            .h(px(88.))
            .child(
                // The content begins beyond the complete cat hit area. Three
                // short rows share one axis, irrespective of the timer state.
                div()
                    .debug_selector(|| "strip-content".into())
                    .absolute()
                    .left(px(28.))
                    .top(px(12.))
                    .w(px(128.))
                    .h(px(68.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .id("cat-task-details")
                            .debug_selector(|| "strip-open".into())
                            .w_full()
                            .h(px(18.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .rounded(px(3.))
                            .hover(|title| title.bg(rgb(0x292929)))
                            .cursor_pointer()
                            .tooltip(move |window, cx| {
                                gpui_component::tooltip::Tooltip::new(title_help.clone())
                                    .build(window, cx)
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.drawer_controls_ready() {
                                    return;
                                }
                                this.dispatch(cx, move |model, window, cx| {
                                    if let Some(id) = session_id {
                                        model.show_manager(window, cx);
                                        model.select_session(id, window, cx);
                                    } else {
                                        model.show_strip_task(window, cx);
                                    }
                                });
                            }))
                            .child(
                                single_line(face.title.clone(), 12.)
                                    .debug_selector(|| "strip-title-text".into())
                                    .w(px(112.))
                                    .flex_shrink_0()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xf5f5f5)),
                            )
                            .child(
                                control_icon(Icon::new(IconName::ChevronRight), 0xaaaaaa)
                                    .size(px(12.))
                                    .flex_none(),
                            ),
                    )
                    .child(
                        single_line(face.status.clone(), 11.)
                            .debug_selector(|| "strip-status".into())
                            .w(px(128.))
                            .flex_shrink_0()
                            .text_color(rgb(if face.failed { 0xe8b3b3 } else { 0xaaaaaa })),
                    )
                    .child(
                        div()
                            .debug_selector(|| "strip-controls".into())
                            .w(px(128.))
                            .h(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                button("bar-primary")
                                    .debug_selector(|| "strip-primary".into())
                                    .w(px(label_width + 21.))
                                    .h(px(24.))
                                    .px_0()
                                    .rounded(px(4.))
                                    .justify_start()
                                    .custom(
                                        cat_secondary_variant(cx).foreground(rgb(0xf5f5f5).into()),
                                    )
                                    .child(
                                        div()
                                            .w(px(label_width + 17.))
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .gap(px(5.))
                                            .child(
                                                single_line(primary_label, 11.)
                                                    .debug_selector(|| "strip-primary-label".into())
                                                    .w(px(label_width))
                                                    .flex_shrink_0()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(rgb(0xf5f5f5)),
                                            )
                                            .child(
                                                div()
                                                    .debug_selector(|| "strip-primary-icon".into())
                                                    .size(px(12.))
                                                    .flex_none()
                                                    .child(
                                                        control_icon(primary_icon, 0xd2d2d2)
                                                            .size(px(12.)),
                                                    ),
                                            ),
                                    )
                                    .on_click(cx.listener(
                                        move |this, event: &ClickEvent, _, cx| {
                                            if this.drawer_controls_ready()
                                                && event.click_count() == 1
                                            {
                                                this.dispatch(cx, move |model, window, cx| {
                                                    if opens_task {
                                                        model.show_strip_task(window, cx);
                                                    } else {
                                                        model.strip_primary_action(window, cx);
                                                    }
                                                });
                                            }
                                        },
                                    )),
                            )
                            .when(
                                face.completed && face.repeatable && !face.failed,
                                |controls| {
                                    controls.child(
                                        button("bar-details")
                                            .debug_selector(|| "strip-details".into())
                                            .w(px(44.))
                                            .h(px(24.))
                                            .px_0()
                                            .rounded(px(4.))
                                            .justify_end()
                                            .custom(cat_secondary_variant(cx))
                                            .child(
                                                single_line("详情", 11.)
                                                    .debug_selector(|| "strip-details-label".into())
                                                    .w(px(22.))
                                                    .flex_none()
                                                    .text_color(rgb(0xaaaaaa)),
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if this.drawer_controls_ready() {
                                                    this.dispatch(cx, |model, window, cx| {
                                                        model.show_strip_task(window, cx)
                                                    });
                                                }
                                            })),
                                    )
                                },
                            )
                            .children(face.session.filter(|_| !face.failed).map(|id| {
                                button("bar-finish")
                                    .debug_selector(|| "strip-finish".into())
                                    .w(px(56.))
                                    .h(px(24.))
                                    .px_0()
                                    .rounded(px(4.))
                                    .justify_end()
                                    .custom(cat_secondary_variant(cx))
                                    .child(
                                        single_line("结束计时", 11.)
                                            .debug_selector(|| "strip-finish-label".into())
                                            .w(px(44.))
                                            .flex_none()
                                            .text_color(rgb(0xaaaaaa)),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.drawer_controls_ready() {
                                            this.dispatch(cx, move |model, _, cx| {
                                                model.finish(id, cx)
                                            })
                                        }
                                    }))
                            })),
                    ),
            )
    }

    fn dock_face(&self, face: &TimerFace, cx: &mut Context<Self>) -> impl IntoElement {
        let (readout, unit) = if face.failed {
            ("重试".to_owned(), String::new())
        } else if face.completed {
            ("已存".to_owned(), String::new())
        } else if face.invalid_duration {
            ("设置".to_owned(), String::new())
        } else if face.idle {
            ("开始".to_owned(), String::new())
        } else {
            compact_time(face.seconds)
        };
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
                    .debug_selector(|| "strip-dock-time".into())
                    .text_size(px(if readout.chars().count() > 2 {
                        11.
                    } else {
                        12.
                    }))
                    .line_height(px(16.))
                    .child(readout),
            )
            .when(!unit.is_empty(), |dock| {
                dock.child(
                    div()
                        .debug_selector(|| "strip-dock-unit".into())
                        .text_size(px(11.))
                        .line_height(px(12.))
                        .text_color(rgb(0xbcbcbc))
                        .child(unit),
                )
            })
            .when(face.show_paused_mark(), |dock| {
                dock.child(
                    div()
                        .debug_selector(|| "strip-dock-paused-mark".into())
                        .size(px(10.))
                        .child(Icon::default().path("force-icons/pause.svg").size(px(10.))),
                )
            })
            .children(face.dock_progress().map(|progress| {
                div()
                    .debug_selector(|| "strip-dock-progress".into())
                    .h(px(22.))
                    .w(px(2.))
                    .bg(rgb(0x535353))
                    .child(div().w_full().h(relative(progress)).bg(rgb(0xf5f5f5)))
            }))
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
                        let silhouette = div()
                            .debug_selector(|| "strip-expanded-silhouette".into())
                            .absolute()
                            .left_0()
                            .top_0()
                            .size_full()
                            .child(
                                div()
                                    .absolute()
                                    .left(px(72.))
                                    .top(px(42.))
                                    .w(px(164.))
                                    .h(px(88.))
                                    .rounded(px(14.))
                                    .border_1()
                                    .border_color(rgb(0x303030))
                                    .bg(rgb(INK)),
                            );
                        let drawer = self.drawer(&face, cx);
                        if self.disclosure_task.is_some() {
                            let started = self.disclosure_started;
                            body.child(silhouette.with_animation(
                                ElementId::NamedInteger(
                                    "cat-disclosure-silhouette".into(),
                                    self.disclosure_serial,
                                ),
                                Animation::new(DISCLOSURE_DURATION),
                                move |surface, _| {
                                    surface.opacity(disclosure_reveal(
                                        from,
                                        target,
                                        started,
                                        Instant::now(),
                                    ))
                                },
                            ))
                            .child(drawer.with_animation(
                                ElementId::NamedInteger(
                                    "cat-disclosure".into(),
                                    self.disclosure_serial,
                                ),
                                Animation::new(DISCLOSURE_DURATION),
                                move |panel, _| {
                                    let reveal =
                                        disclosure_reveal(from, target, started, Instant::now());
                                    panel.opacity(reveal).left(px(64. - 8. * (1. - reveal)))
                                },
                            ))
                        } else {
                            // The timer and rendering share the same start time.
                            // A delayed first frame jumps directly to fully open,
                            // rather than leaving a newly enabled faded control.
                            let serial = self.disclosure_serial;
                            let weak = cx.weak_entity();
                            body.child(silhouette).child(drawer).when(
                                self.expanded && self.drawer_painted_serial != Some(serial),
                                |body| {
                                    body.child(
                                        canvas(
                                            |_, _, _| (),
                                            move |_, _, _, cx| {
                                                let weak = weak.clone();
                                                cx.defer(move |cx| {
                                                    let _ = weak.update(cx, |this, _| {
                                                        if this.expanded
                                                            && this.drawer_visible
                                                            && this.disclosure_task.is_none()
                                                            && this.disclosure_serial == serial
                                                        {
                                                            this.drawer_painted_serial =
                                                                Some(serial);
                                                        }
                                                    });
                                                });
                                            },
                                        )
                                        .absolute()
                                        .size_full(),
                                    )
                                },
                            )
                        }
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

fn activity_task_label(activity: &str, title: &str) -> String {
    if title.is_empty() {
        activity.to_owned()
    } else {
        title.to_owned()
    }
}

fn disclosure_reveal(from: f32, target: f32, started: Instant, now: Instant) -> f32 {
    let elapsed = (now.saturating_duration_since(started).as_secs_f32()
        / DISCLOSURE_DURATION.as_secs_f32())
    .min(1.);
    from + (target - from) * ease_in_out(elapsed)
}

fn cat_click_active(started: Option<Instant>, now: Instant) -> bool {
    started.is_some_and(|started| now.saturating_duration_since(started) < CAT_CLICK_DURATION)
}

// Fade only the eye patch in and out during the first half of the click.
fn cat_blink_opacity(progress: f32) -> f32 {
    let blink = (progress.clamp(0., 1.) / 0.48).min(1.);
    (std::f32::consts::PI * blink).sin().powi(2)
}

fn compact_clock(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds / 3_600 >= 10_000 {
        "9999时+".to_owned()
    } else {
        clock(seconds)
    }
}

fn compact_time(seconds: i64) -> (String, String) {
    let seconds = seconds.max(0);
    if seconds < 3600 {
        (format!("{:02}", seconds / 60), "分".to_owned())
    } else {
        let hours = seconds / 3600;
        (
            if hours > 99 {
                "99+".to_owned()
            } else {
                hours.to_string()
            },
            "时".to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "ui-test-support")]
    use super::compact_clock;
    use super::{
        CAT_CLICK_DURATION, DISCLOSURE_DURATION, TimerFace, cat_blink_opacity, cat_click_active,
        compact_time, disclosure_reveal,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn disclosure_opacity_uses_the_transition_start_even_if_the_first_frame_is_late() {
        let start = Instant::now();
        assert_eq!(disclosure_reveal(0., 1., start, start), 0.);
        let halfway = disclosure_reveal(0., 1., start, start + DISCLOSURE_DURATION / 2);
        assert!((halfway - 0.5).abs() < 0.001);
        assert_eq!(
            disclosure_reveal(0., 1., start, start + DISCLOSURE_DURATION),
            1.
        );
        assert_eq!(
            disclosure_reveal(0.4, 0., start, start + DISCLOSURE_DURATION),
            0.
        );
        assert_eq!(
            disclosure_reveal(0.4, 1., start, start + DISCLOSURE_DURATION * 2),
            1.
        );
    }

    #[cfg(feature = "ui-test-support")]
    #[gpui::test]
    fn drawer_controls_require_the_current_fully_painted_frame(cx: &mut gpui::TestAppContext) {
        use crate::headless_ui_tests::ManagerHarness;
        use gpui::AppContext as _;
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        let model = manager.view.clone();
        let handle = manager.handle;
        let cat = manager
            .cx
            .update(|window, cx| cx.new(|cx| super::FloatingStrip::new(model, handle, window, cx)));
        cat.update(&mut manager.cx, |cat, _| {
            cat.expanded = true;
            cat.drawer_visible = true;
            cat.disclosure_serial = 5;
            cat.drawer_painted_serial = None;
            assert!(!cat.drawer_controls_ready());
            cat.drawer_painted_serial = Some(4); // previous transition's late frame
            assert!(!cat.drawer_controls_ready());
            cat.drawer_painted_serial = Some(5);
            assert!(cat.drawer_controls_ready());
            cat.expanded = false;
            assert!(!cat.drawer_controls_ready());
        });
    }

    #[cfg(feature = "ui-test-support")]
    #[gpui::test]
    fn activity_countup_default_is_idle_until_a_real_selection_or_description(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::headless_ui_tests::ManagerHarness;
        use gpui::AppContext as _;
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        let model = manager.view.clone();
        let handle = manager.handle;
        let cat = manager
            .cx
            .update(|window, cx| cx.new(|cx| super::FloatingStrip::new(model, handle, window, cx)));
        cat.read_with(&manager.cx, |cat, cx| {
            let face = cat.timer_face(cx);
            assert!(face.idle && !face.ready);
            assert_eq!(face.compact_display().0, "开始");
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.primary_action(window, cx);
            assert_eq!(view.selected_tag, 0);
            assert!(!view.countdown);
            assert!(view.active.is_empty());
            view.task_input.update(cx, |input, cx| {
                input.set_value("未选活动的说明", window, cx)
            });
        });
        cat.read_with(&manager.cx, |cat, cx| {
            let face = cat.timer_face(cx);
            assert!(!face.idle && !face.ready);
            assert_eq!(face.status, "选择活动");
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.task_input
                .update(cx, |input, cx| input.set_value("", window, cx));
            view.selected_tag = 2;
            cx.notify();
        });
        cat.read_with(&manager.cx, |cat, cx| {
            let face = cat.timer_face(cx);
            assert!(!face.idle && face.ready);
            assert_eq!(face.title, "学习");
            assert_eq!(face.status, "正计时");
        });
    }

    #[cfg(feature = "ui-test-support")]
    #[gpui::test]
    fn saved_countup_task_reports_pending_changes_until_saved(cx: &mut gpui::TestAppContext) {
        use crate::headless_ui_tests::ManagerHarness;
        use gpui::AppContext as _;
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let task = view
                .store
                .as_mut()
                .unwrap()
                .create_task("已有说明", 1, view.now)
                .unwrap();
            view.refresh(cx);
            view.select_task(task.id, window, cx);
            view.countdown = false;
        });
        let model = manager.view.clone();
        let handle = manager.handle;
        let cat = manager
            .cx
            .update(|window, cx| cx.new(|cx| super::FloatingStrip::new(model, handle, window, cx)));
        cat.read_with(&manager.cx, |cat, cx| {
            let face = cat.timer_face(cx);
            assert!(face.ready);
            assert_eq!(face.status, "正计时");
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.task_input
                .update(cx, |input, cx| input.set_value("修改后的说明", window, cx));
            assert!(view.task_has_unsaved_changes(cx));
        });
        cat.read_with(&manager.cx, |cat, cx| {
            let face = cat.timer_face(cx);
            assert!(!face.ready && !face.running);
            assert_eq!(face.status, "修改待保存");
        });
        manager.view.update(&mut manager.cx, |view, cx| {
            view.save_task_changes(cx);
            assert!(!view.task_has_unsaved_changes(cx));
            assert_eq!(view.tasks[0].title, "修改后的说明");
            assert!(view.active.is_empty());
        });
        cat.read_with(&manager.cx, |cat, cx| {
            let face = cat.timer_face(cx);
            assert!(face.ready && !face.running);
            assert_eq!(face.status, "正计时");
        });
    }

    #[cfg(feature = "ui-test-support")]
    #[gpui::test]
    fn compact_cat_keeps_readouts_below_the_artwork_in_every_state(cx: &mut gpui::TestAppContext) {
        use crate::headless_ui_tests::ManagerHarness;
        use gpui::{
            AppContext as _, Bounds, Point, VisualTestContext, WindowBounds, WindowOptions, px,
            size,
        };
        use gpui_component::Root;

        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        let model = manager.view.clone();
        let manager_window = manager.handle;
        let strip = manager.cx.cx.update(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Point::default(),
                        size(px(88.), px(128.)),
                    ))),
                    ..Default::default()
                },
                |window, cx| {
                    let cat =
                        cx.new(|cx| super::FloatingStrip::new(model, manager_window, window, cx));
                    cx.new(|cx| Root::new(cat, window, cx))
                },
            )
            .unwrap()
        });
        let mut visual = VisualTestContext::from_window(*strip, &manager.cx.cx);
        visual.simulate_resize(size(px(88.), px(128.)));
        visual.update(|window, _| window.refresh());
        visual.run_until_parked();

        let artwork = visual.debug_bounds("strip-artwork").unwrap();
        let plate = visual.debug_bounds("strip-readout-plate").unwrap();
        let mark = visual.debug_bounds("strip-time-text").unwrap();
        assert!(visual.debug_bounds("strip-detail").is_none());
        assert!(artwork.bottom() <= plate.top());
        assert!(mark.left() >= plate.left() && mark.right() <= plate.right());
        assert!(mark.top() >= plate.top() && mark.bottom() <= plate.bottom());

        // Render prepared, long-running, paused, completed and failed
        // states with real text boxes rather than only the outer layout.
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.new_task(window, cx);
            view.task_input
                .update(cx, |input, cx| input.set_value("Example", window, cx));
        });
        for state in 0..6 {
            if state == 1 {
                manager.view.update_in(&mut manager.cx, |view, _, cx| {
                    view.store
                        .as_mut()
                        .unwrap()
                        .start("Long running task", 1, view.now - 100 * 3600 - 135)
                        .unwrap();
                    view.refresh(cx);
                });
            } else if state == 2 {
                manager.view.update_in(&mut manager.cx, |view, _, cx| {
                    let id = view.strip_session().unwrap().id;
                    view.store.as_mut().unwrap().pause(id, view.now).unwrap();
                    view.refresh(cx);
                });
            } else if state == 3 {
                manager.view.update(&mut manager.cx, |view, cx| {
                    view.active[0].intervals[0].start = view.now - 10_000 * 3600 - 135;
                    cx.notify();
                });
            } else if state == 4 {
                manager.view.update(&mut manager.cx, |view, cx| {
                    view.completed = view.strip_session().cloned();
                    view.active.clear();
                    cx.notify();
                });
            } else if state == 5 {
                manager.view.update(&mut manager.cx, |view, cx| {
                    view.storage_failed = true;
                    cx.notify();
                });
            }
            visual.update(|window, _| window.refresh());
            visual.run_until_parked();
            let root = visual.debug_bounds("strip-root").unwrap();
            let face = visual.debug_bounds("strip-face").unwrap();
            let artwork = visual.debug_bounds("strip-artwork").unwrap();
            let plate = visual.debug_bounds("strip-readout-plate").unwrap();
            let time = visual.debug_bounds("strip-time").unwrap();
            let text = visual.debug_bounds("strip-time-text").unwrap();
            assert_eq!(root.size, size(px(88.), px(128.)));
            assert!(artwork.bottom() <= plate.top());
            assert!(plate.left() >= face.left() && plate.right() <= face.right());
            assert!(plate.bottom() <= root.bottom());
            assert!(text.left() >= time.left() - px(1.) && text.right() <= time.right() + px(1.));
            assert!(text.top() >= time.top() - px(1.) && text.bottom() <= time.bottom() + px(1.));
            assert!(text.size.width > px(0.) && text.size.height > px(0.));
            if state < 5 {
                let detail = visual.debug_bounds("strip-detail-text").unwrap();
                assert!(text.bottom() <= detail.top());
                assert!(detail.left() >= plate.left() && detail.right() <= plate.right());
                assert!(detail.bottom() <= plate.bottom());
                if (2..=3).contains(&state) {
                    let pause = visual.debug_bounds("strip-paused-mark").unwrap();
                    assert!(pause.right() < detail.left());
                    assert!(pause.bottom() <= plate.bottom());
                }
            }
        }
        visual.update(|window, _| window.remove_window());
        visual.run_until_parked();
    }

    #[cfg(feature = "ui-test-support")]
    #[gpui::test]
    fn storage_error_hides_paused_marks_in_both_cat_and_dock(cx: &mut gpui::TestAppContext) {
        use crate::{headless_ui_tests::ManagerHarness, platform::DockEdge};
        use gpui::{
            AppContext as _, Bounds, Point, VisualTestContext, WindowBounds, WindowOptions, px,
            size,
        };
        use gpui_component::Root;

        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        manager.view.update(&mut manager.cx, |view, cx| {
            let session = view
                .store
                .as_mut()
                .unwrap()
                .start("Paused task", 1, view.now - 120)
                .unwrap();
            view.store
                .as_mut()
                .unwrap()
                .pause(session.id, view.now)
                .unwrap();
            view.refresh(cx);
            assert!(view.strip_session().unwrap().is_paused());
            view.storage_failed = true;
            cx.notify();
        });
        for docked in [false, true] {
            let model = manager.view.clone();
            let manager_window = manager.handle;
            let dimensions = if docked {
                size(px(28.), px(80.))
            } else {
                size(px(88.), px(128.))
            };
            let (strip, cat) = manager.cx.cx.update(|cx| {
                let mut cat = None;
                let window = cx
                    .open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                                Point::default(),
                                dimensions,
                            ))),
                            ..Default::default()
                        },
                        |window, cx| {
                            let entity = cx.new(|cx| {
                                let mut cat =
                                    super::FloatingStrip::new(model, manager_window, window, cx);
                                if docked {
                                    cat.docked = Some(DockEdge::Left);
                                }
                                cat
                            });
                            cat = Some(entity.clone());
                            cx.new(|cx| Root::new(entity, window, cx))
                        },
                    )
                    .unwrap();
                (window, cat.unwrap())
            });
            let mut visual = VisualTestContext::from_window(*strip, &manager.cx.cx);
            visual.simulate_resize(dimensions);
            visual.update(|window, _| window.refresh());
            visual.run_until_parked();
            cat.read_with(&visual, |cat, cx| {
                let face = cat.timer_face(cx);
                assert!(face.failed && face.paused);
                assert_eq!(face.compact_display(), ("重试".into(), String::new()));
                assert!(!face.show_paused_mark());
            });
            if docked {
                assert!(visual.debug_bounds("strip-dock").is_some());
                assert!(visual.debug_bounds("strip-dock-paused-mark").is_none());
            } else {
                assert!(visual.debug_bounds("strip-time-text").is_some());
                assert!(visual.debug_bounds("strip-detail-text").is_none());
                assert!(visual.debug_bounds("strip-paused-mark").is_none());
            }
            visual.update(|window, _| window.remove_window());
            visual.run_until_parked();
        }
    }

    #[cfg(feature = "ui-test-support")]
    #[gpui::test]
    fn invalid_draft_time_never_shows_a_stale_value_or_overrides_a_running_timer(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::headless_ui_tests::ManagerHarness;
        use gpui::AppContext as _;

        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.new_task(window, cx);
            view.selected_tag = 1;
            view.task_input
                .update(cx, |input, cx| input.set_value("Prepare", window, cx));
            view.hide_manager(window, cx);
        });
        let model = manager.view.clone();
        let handle = manager.handle;
        let cat = manager
            .cx
            .update(|window, cx| cx.new(|cx| super::FloatingStrip::new(model, handle, window, cx)));
        for invalid in ["0", "", "1441"] {
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                view.countdown = true;
                view.timer_input
                    .update(cx, |input, cx| input.set_value(invalid, window, cx));
            });
            manager.cx.run_until_parked();
            cat.read_with(&manager.cx, |cat, cx| {
                let face = cat.timer_face(cx);
                assert!(face.invalid_duration && !face.ready);
                assert_eq!(face.status, "设置时长");
                assert_eq!(face.compact_display(), ("设置时长".into(), String::new()));
            });
            manager.view.read_with(&manager.cx, |view, cx| {
                assert_eq!(view.timer_input.read(cx).value(), invalid);
                assert_eq!(view.task_input.read(cx).value(), "Prepare");
            });
        }
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.timer_input
                .update(cx, |input, cx| input.set_value("37", window, cx));
        });
        manager.cx.run_until_parked();
        cat.read_with(&manager.cx, |cat, cx| {
            let face = cat.timer_face(cx);
            assert!(!face.invalid_duration && face.ready);
            assert_eq!(face.compact_display(), ("37:00".into(), "待开始".into()));
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.timer_input
                .update(cx, |input, cx| input.set_value("0", window, cx));
            view.countdown = false;
        });
        manager.cx.run_until_parked();
        cat.read_with(&manager.cx, |cat, cx| {
            let face = cat.timer_face(cx);
            assert!(!face.invalid_duration && face.ready);
            assert_eq!(face.compact_display(), ("00:00".into(), "待开始".into()));
            assert_eq!(face.status, "正计时");
        });
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            view.countdown = true;
            view.store
                .as_mut()
                .unwrap()
                .create_and_start_task("Running", view.selected_tag, view.now - 60, Some(1500))
                .unwrap();
            view.refresh(cx);
        });
        manager.cx.run_until_parked();
        cat.read_with(&manager.cx, |cat, cx| {
            let face = cat.timer_face(cx);
            assert!(!face.invalid_duration && face.running);
            assert_eq!(face.title, "Running");
            assert_eq!(face.title_help, "工作 · Running");
            assert_eq!(face.status, "工作");
            let model = cat.model.read(cx);
            let session = model.strip_session().unwrap();
            assert_eq!(face.session, Some(session.id));
            assert_eq!(model.timer_input.read(cx).value(), "0");
            let remaining =
                (session.target_seconds.unwrap() - session.elapsed_at(model.now)).max(0);
            assert_eq!(face.seconds, remaining);
            assert_eq!(
                face.compact_display(),
                (compact_clock(remaining), "剩余".into())
            );
        });
    }

    #[cfg(feature = "ui-test-support")]
    #[gpui::test]
    fn invalid_cat_names_the_action_without_a_dash_or_stale_time(cx: &mut gpui::TestAppContext) {
        use crate::headless_ui_tests::ManagerHarness;
        use gpui::{
            AppContext as _, Bounds, Point, VisualTestContext, WindowBounds, WindowOptions, px,
            size,
        };
        use gpui_component::Root;

        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.new_task(window, cx);
            view.task_input
                .update(cx, |input, cx| input.set_value("Test", window, cx));
            view.countdown = true;
            view.timer_input
                .update(cx, |input, cx| input.set_value("0", window, cx));
        });
        let model = manager.view.clone();
        let manager_window = manager.handle;
        let strip = manager.cx.cx.update(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Point::default(),
                        size(px(88.), px(128.)),
                    ))),
                    ..Default::default()
                },
                |window, cx| {
                    let cat =
                        cx.new(|cx| super::FloatingStrip::new(model, manager_window, window, cx));
                    cx.new(|cx| Root::new(cat, window, cx))
                },
            )
            .unwrap()
        });
        let mut visual = VisualTestContext::from_window(*strip, &manager.cx.cx);
        visual.simulate_resize(size(px(88.), px(128.)));
        visual.update(|window, _| window.refresh());
        visual.run_until_parked();
        let artwork = visual.debug_bounds("strip-artwork").unwrap();
        let plate = visual.debug_bounds("strip-readout-plate").unwrap();
        let message = visual.debug_bounds("strip-time-text").unwrap();
        assert!(artwork.bottom() <= plate.top());
        assert!(message.left() >= plate.left() && message.right() <= plate.right());
        assert!(message.top() >= plate.top() && message.bottom() <= plate.bottom());
        assert!(message.size.width > px(0.) && message.size.height > px(0.));
        assert!(visual.debug_bounds("strip-detail").is_none());
        visual.update(|window, _| window.remove_window());
        visual.run_until_parked();
    }

    #[test]
    fn cat_click_animation_expires_before_it_can_be_remounted() {
        let start = Instant::now();
        assert!(!cat_click_active(None, start));
        assert!(cat_click_active(
            Some(start),
            start + CAT_CLICK_DURATION - Duration::from_millis(1)
        ));
        assert!(!cat_click_active(Some(start), start + CAT_CLICK_DURATION));
        assert!(!cat_click_active(
            Some(start),
            start + Duration::from_secs(10)
        ));
    }

    #[test]
    fn cat_blink_fades_in_and_out_then_stays_open() {
        for progress in [0., 0.12, 0.24, 0.36, 0.48, 0.7, 1.] {
            assert!((0. ..=1.).contains(&cat_blink_opacity(progress)));
        }
        assert!(cat_blink_opacity(0.) < 0.001);
        assert!(cat_blink_opacity(0.24) > 0.999);
        assert!(cat_blink_opacity(0.48) < 0.001);
        assert!(cat_blink_opacity(1.) < 0.001);
    }

    #[test]
    fn paused_readout_keeps_the_same_complete_clock_for_long_sessions() {
        let mut face = TimerFace {
            title: String::new(),
            title_help: String::new(),
            status: String::new(),
            seconds: 100 * 3600 + 135,
            running: false,
            paused: true,
            completed: false,
            repeatable: false,
            failed: false,
            invalid_duration: false,
            session: Some(1),
            progress: None,
            ready: false,
            idle: false,
        };
        for paused in [false, true] {
            face.paused = paused;
            face.running = !paused;
            let state = if paused { "已暂停" } else { "正计时" };
            for (seconds, expected) in [
                (3_599, "59:59"),
                (3_600, "1:00:00"),
                (99 * 3_600 + 3_599, "99:59:59"),
                (100 * 3_600 + 3_599, "100:59:59"),
                (9_999 * 3_600 + 3_599, "9999:59:59"),
                (10_000 * 3_600, "9999时+"),
            ] {
                face.seconds = seconds;
                assert_eq!(face.compact_display(), (expected.into(), state.into()));
            }
        }
        face.paused = false;
        face.running = false;
        face.completed = true;
        face.seconds = 1_245;
        assert_eq!(face.compact_display(), ("20:45".into(), "已保存".into()));
    }

    #[test]
    fn dock_progress_requires_a_countdown_and_yields_to_pause_or_error() {
        let mut face = TimerFace {
            title: String::new(),
            title_help: String::new(),
            status: String::new(),
            seconds: 0,
            running: false,
            paused: false,
            completed: false,
            repeatable: false,
            failed: false,
            invalid_duration: false,
            session: None,
            progress: None,
            ready: false,
            idle: true,
        };
        assert_eq!(face.dock_progress(), None);
        face.idle = false;
        face.running = true;
        face.session = Some(1);
        assert_eq!(face.dock_progress(), None); // Count-up has no goal.
        for progress in [0., 0.5, 1.] {
            face.progress = Some(progress);
            assert_eq!(face.dock_progress(), Some(progress));
        }
        face.paused = true;
        assert_eq!(face.dock_progress(), None);
        face.paused = false;
        face.failed = true;
        assert_eq!(face.dock_progress(), None);
    }

    #[test]
    fn compact_clock_keeps_subhour_seconds_and_hour_meaning() {
        assert_eq!(compact_time(1122), ("18".into(), "分".into()));
        assert_eq!(compact_time(3607), ("1".into(), "时".into()));
        assert_eq!(compact_time(-1), ("00".into(), "分".into()));
        assert_eq!(compact_time(i64::MAX), ("99+".into(), "时".into()));
    }
}
