//! A small local-draft clock. The target input changes only on confirmation.
use chrono::{Local, Timelike};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::InputState,
};
use std::time::{Duration, Instant};

use crate::{
    label::single_line,
    view::{
        CONTROL_LINE, HOVER, INK, LINE, MUTED, PAPER, PRESSED, SURFACE, button, control_label,
        primary_button, quiet_variant,
    },
};

actions!(time_picker, [NextField, PreviousField]);

const DIAL_SIZE: f32 = 216.;
const LABEL_RADIUS: f32 = 82.;
const TICK_INTERVAL: Duration = Duration::from_millis(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DialPart {
    Hour,
    Minute,
}

impl DialPart {
    fn steps(self) -> u16 {
        match self {
            Self::Hour => 24,
            Self::Minute => 60,
        }
    }
}

pub(crate) struct TimePicker {
    target: Option<Entity<InputState>>,
    original: String,
    title: &'static str,
    minute_of_day: u16,
    allow_midnight_end: bool,
    part: DialPart,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    dial_bounds: Option<Bounds<Pixels>>,
    dragging: bool,
    sound_enabled: bool,
    last_tick: Option<Instant>,
}

impl TimePicker {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new("tab", NextField, Some("TimePicker")),
            KeyBinding::new("shift-tab", PreviousField, Some("TimePicker")),
        ]);
        Self {
            target: None,
            original: String::new(),
            title: "时刻",
            minute_of_day: 0,
            allow_midnight_end: false,
            part: DialPart::Hour,
            focus: cx.focus_handle().tab_stop(false),
            return_focus: None,
            dial_bounds: None,
            dragging: false,
            sound_enabled: false,
            last_tick: None,
        }
    }

    pub(crate) fn open_for(
        &mut self,
        input: Entity<InputState>,
        title: &'static str,
        allow_midnight_end: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.original = input.read(cx).value().to_string();
        self.minute_of_day = parse_time(&self.original, allow_midnight_end).unwrap_or_else(|| {
            let now = Local::now();
            (now.hour() * 60 + now.minute()) as u16
        });
        self.return_focus = Some(input.focus_handle(cx));
        self.target = Some(input);
        self.title = title;
        self.allow_midnight_end = allow_midnight_end;
        self.part = DialPart::Hour;
        self.dragging = false;
        self.dial_bounds = None;
        self.last_tick = None;
        self.focus.focus(window);
        cx.notify();
    }

    pub(crate) fn is_open(&self) -> bool {
        self.target.is_some()
    }

    pub(crate) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owned_focus = self.focus.contains_focused(window, cx);
        self.target = None;
        self.dragging = false;
        self.dial_bounds = None;
        if let Some(focus) = self.return_focus.take().filter(|_| owned_focus) {
            focus.focus(window);
        }
        cx.notify();
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.target.clone() else {
            return;
        };
        // A changed/hidden parent draft must never receive an old picker result.
        if target.read(cx).value().as_ref() == self.original {
            let value = format!(
                "{:02}:{:02}",
                self.minute_of_day / 60,
                self.minute_of_day % 60
            );
            target.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        self.dismiss(window, cx);
    }

    fn select_part(&mut self, part: DialPart, cx: &mut Context<Self>) {
        self.part = part;
        self.dragging = false;
        cx.notify();
    }

    fn set_minute(&mut self, value: u16, cx: &mut Context<Self>) {
        if !self.is_open() || value > if self.allow_midnight_end { 1440 } else { 1439 } {
            return;
        }
        if value != self.minute_of_day {
            self.minute_of_day = value;
            if self.sound_enabled && crate::platform::time_tick_supported() {
                let now = Instant::now();
                if self
                    .last_tick
                    .is_none_or(|previous| now.duration_since(previous) >= TICK_INTERVAL)
                {
                    crate::platform::play_time_tick();
                    self.last_tick = Some(now);
                }
            }
            cx.notify();
        }
    }

    fn select_point(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(bounds) = self.dial_bounds else {
            return;
        };
        let offset = position - bounds.center();
        let Some(step) = dial_step(f32::from(offset.x), f32::from(offset.y), self.part.steps())
        else {
            return;
        };
        let value = match self.part {
            DialPart::Hour => step * 60 + self.minute_of_day % 60,
            DialPart::Minute => (self.minute_of_day / 60 % 24) * 60 + step,
        };
        self.set_minute(value, cx);
    }

    fn adjust(&mut self, delta: i32, cx: &mut Context<Self>) {
        let value = match self.part {
            DialPart::Hour => {
                let minute = self.minute_of_day % 60;
                let count = if self.allow_midnight_end && minute == 0 {
                    25
                } else {
                    24
                };
                let hour = (i32::from(self.minute_of_day / 60) + delta).rem_euclid(count);
                hour as u16 * 60 + minute
            }
            DialPart::Minute => {
                let count = if self.allow_midnight_end { 1441 } else { 1440 };
                (i32::from(self.minute_of_day) + delta).rem_euclid(count) as u16
            }
        };
        self.set_minute(value, cx);
    }

    fn cycle_focus(&self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        let first = window.focused(cx);
        loop {
            if backwards {
                window.focus_prev();
            } else {
                window.focus_next();
            }
            if self.focus.contains_focused(window, cx) {
                break;
            }
            if window.focused(cx) == first {
                self.focus.focus(window);
                break;
            }
        }
        cx.stop_propagation();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open()
            || event.keystroke.modifiers.platform
            || event.keystroke.modifiers.control
            || event.keystroke.modifiers.alt
        {
            return;
        }
        match event.keystroke.key.as_str() {
            "up" | "right" => self.adjust(1, cx),
            "down" | "left" => self.adjust(-1, cx),
            "escape" => self.dismiss(window, cx),
            "enter" if self.focus.is_focused(window) => self.confirm(window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn dial(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let part = self.part;
        let selected = match part {
            DialPart::Hour => self.minute_of_day / 60 % 24,
            DialPart::Minute => self.minute_of_day % 60,
        };
        let steps = part.steps();
        let major = if part == DialPart::Hour { 2 } else { 5 };
        let view = cx.entity().downgrade();
        div()
            .id("time-picker-dial")
            .debug_selector(|| "time-picker-dial".into())
            .relative()
            .size(px(DIAL_SIZE))
            .flex_shrink_0()
            .rounded_full()
            .bg(rgb(SURFACE))
            .border_1()
            .border_color(rgb(LINE))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.focus.focus(window);
                    this.dragging = true;
                    this.select_point(event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                canvas(
                    move |bounds, _, cx| {
                        let _ = view.update(cx, |this, _| this.dial_bounds = Some(bounds));
                    },
                    move |bounds, _, window, _| {
                        let center = bounds.center();
                        let radius = f32::from(bounds.size.width) / 2.;
                        for step in 0..steps {
                            let outer = polar(center, radius - 5., step, steps);
                            let inner = polar(
                                center,
                                radius - if step % major == 0 { 11. } else { 8. },
                                step,
                                steps,
                            );
                            paint_line(
                                window,
                                inner,
                                outer,
                                1.,
                                if step % major == 0 {
                                    MUTED
                                } else {
                                    CONTROL_LINE
                                },
                            );
                        }
                        let radius = if selected % major == 0 {
                            LABEL_RADIUS
                        } else {
                            LABEL_RADIUS - 20.
                        };
                        let tip = polar(center, radius, selected, steps);
                        paint_line(window, center, tip, 1.5, INK);
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .children((0..steps).filter(|step| step % major == 0).map(|step| {
                let position = polar(
                    point(px(DIAL_SIZE / 2.), px(DIAL_SIZE / 2.)),
                    LABEL_RADIUS,
                    step,
                    steps,
                );
                let active = step == selected;
                div()
                    .absolute()
                    .left(position.x - px(12.))
                    .top(position.y - px(12.))
                    .size(px(24.))
                    .rounded_full()
                    .when(active, |label| label.bg(rgb(INK)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.))
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(rgb(if active { PAPER } else { MUTED }))
                    .child(format!("{step:02}"))
            }))
            .when(selected % major != 0, |dial| {
                let tip = polar(
                    point(px(DIAL_SIZE / 2.), px(DIAL_SIZE / 2.)),
                    LABEL_RADIUS - 20.,
                    selected,
                    steps,
                );
                dial.child(
                    div()
                        .absolute()
                        .left(tip.x - px(4.))
                        .top(tip.y - px(4.))
                        .size(px(8.))
                        .rounded_full()
                        .bg(rgb(INK)),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(px(DIAL_SIZE / 2. - 3.))
                    .top(px(DIAL_SIZE / 2. - 3.))
                    .size(px(6.))
                    .rounded_full()
                    .bg(rgb(INK)),
            )
    }
}

impl Focusable for TimePicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TimePicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.is_open() {
            return div().into_any_element();
        }
        let hour = format!("{:02}", self.minute_of_day / 60);
        let minute = format!("{:02}", self.minute_of_day % 60);
        let readout = |id: &'static str, value: String, active: bool| {
            button(id)
                .debug_selector(move || id.into())
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(rgb(if active { SURFACE } else { PAPER }).into())
                        .foreground(rgb(INK).into())
                        .border(if active {
                            rgb(CONTROL_LINE).into()
                        } else {
                            transparent_black()
                        })
                        .hover(rgb(HOVER).into())
                        .active(rgb(PRESSED).into()),
                )
                .w(px(64.))
                .h(px(40.))
                .p_0()
                .child(single_line(value, 26.).w(px(36.)).text_color(rgb(INK)))
        };
        let hours = readout("time-picker-hour", hour, self.part == DialPart::Hour)
            .tooltip("小时 · 方向键调整")
            .on_click(cx.listener(|this, _, _, cx| this.select_part(DialPart::Hour, cx)));
        let minutes = readout("time-picker-minute", minute, self.part == DialPart::Minute)
            .tooltip("分钟 · 方向键调整")
            .on_click(cx.listener(|this, _, _, cx| this.select_part(DialPart::Minute, cx)));
        let panel = div()
            .id("time-picker")
            .debug_selector(|| "time-picker".into())
            .track_focus(&self.focus)
            .key_context("TimePicker")
            .on_action(
                cx.listener(|this, _: &NextField, window, cx| this.cycle_focus(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PreviousField, window, cx| {
                    this.cycle_focus(true, window, cx)
                }),
            )
            .capture_action(cx.listener(|this, _: &crate::SaveMoment, window, cx| {
                this.confirm(window, cx);
                cx.stop_propagation();
            }))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down_out(cx.listener(|_, _, window, cx| {
                // Keep the parent editor protected for the whole pointer dispatch.
                // Closing immediately lets its outside-click handler see a closed picker.
                cx.defer_in(window, |this, window, cx| this.dismiss(window, cx));
            }))
            .w(px(304.))
            .p(px(16.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(CONTROL_LINE))
            .bg(rgb(PAPER))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        single_line(self.title, 14.)
                            .w(px(128.))
                            .font_weight(FontWeight::MEDIUM),
                    )
                    .child(
                        single_line("24 小时制", 12.)
                            .w(px(64.))
                            .text_color(rgb(MUTED)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .child(hours)
                    .child(div().text_size(px(24.)).text_color(rgb(MUTED)).child(":"))
                    .child(minutes),
            )
            .child(div().flex().justify_center().child(self.dial(cx)))
            .child(
                div()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .when(self.allow_midnight_end, |row| {
                        row.child(
                            button("time-picker-midnight")
                                .debug_selector(|| "time-picker-midnight".into())
                                .custom(quiet_variant(cx))
                                .h(px(28.))
                                .px(px(6.))
                                .child(control_label(
                                    if self.minute_of_day == 1440 {
                                        "✓ 24:00"
                                    } else {
                                        "24:00"
                                    },
                                    INK,
                                ))
                                .tooltip("当天结束")
                                .on_click(cx.listener(|this, _, _, cx| this.set_minute(1440, cx))),
                        )
                    })
                    .when(!self.allow_midnight_end, |row| {
                        row.child(
                            single_line("拖动指针或按方向键", 12.)
                                .w(px(136.))
                                .text_color(rgb(MUTED)),
                        )
                    })
                    .when(crate::platform::time_tick_supported(), |row| {
                        row.child(
                            button("time-picker-sound")
                                .debug_selector(|| "time-picker-sound".into())
                                .custom(quiet_variant(cx))
                                .h(px(28.))
                                .px(px(6.))
                                .child(control_label(
                                    if self.sound_enabled {
                                        "✓ 刻度音"
                                    } else {
                                        "刻度音"
                                    },
                                    MUTED,
                                ))
                                .tooltip(if self.sound_enabled {
                                    "关闭刻度音"
                                } else {
                                    "开启刻度音"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sound_enabled = !this.sound_enabled;
                                    this.last_tick = None;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .pt(px(6.))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button("time-picker-cancel")
                            .debug_selector(|| "time-picker-cancel".into())
                            .custom(quiet_variant(cx))
                            .h(px(32.))
                            .child(control_label("取消", MUTED))
                            .on_click(cx.listener(|this, _, window, cx| this.dismiss(window, cx))),
                    )
                    .child(
                        primary_button("time-picker-confirm")
                            .debug_selector(|| "time-picker-confirm".into())
                            .h(px(32.))
                            .child(control_label("确定", PAPER))
                            .on_click(cx.listener(|this, _, window, cx| this.confirm(window, cx))),
                    ),
            );
        div()
            .id("time-picker-overlay")
            .debug_selector(|| "time-picker-overlay".into())
            .absolute()
            .inset_0()
            .size_full()
            .occlude()
            .bg(rgba(0x00000010))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.dragging {
                    if event.dragging() {
                        this.select_point(event.position, cx);
                    } else {
                        this.dragging = false;
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .child(panel)
            .into_any_element()
    }
}

/// Clock trigger uses the same quiet-button feedback as other editor controls.
pub(crate) fn trigger(id: &'static str, cx: &App) -> Button {
    button(id)
        .debug_selector(move || id.into())
        .custom(quiet_variant(cx))
        .size(px(32.))
        .p_0()
        .tooltip("用表盘选择时间")
        .child(
            div()
                .relative()
                .size(px(16.))
                .rounded_full()
                .border_1()
                .border_color(rgb(MUTED))
                .child(
                    div()
                        .absolute()
                        .left(px(7.))
                        .top(px(3.))
                        .w(px(1.))
                        .h(px(5.))
                        .bg(rgb(MUTED)),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(7.))
                        .top(px(7.))
                        .w(px(4.))
                        .h(px(1.))
                        .bg(rgb(MUTED)),
                ),
        )
}

fn parse_time(value: &str, allow_midnight_end: bool) -> Option<u16> {
    let (hour, minute) = value.trim().split_once(':')?;
    if !(1..=2).contains(&hour.len())
        || minute.len() != 2
        || !hour
            .bytes()
            .chain(minute.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let hour: u16 = hour.parse().ok()?;
    let minute: u16 = minute.parse().ok()?;
    if minute < 60 && (hour < 24 || (allow_midnight_end && hour == 24 && minute == 0)) {
        Some(hour * 60 + minute)
    } else {
        None
    }
}

fn dial_step(x: f32, y: f32, steps: u16) -> Option<u16> {
    if x.hypot(y) < 12. {
        return None;
    }
    let turn = x.atan2(-y).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    Some((turn * f32::from(steps)).round() as u16 % steps)
}

fn polar(center: Point<Pixels>, radius: f32, step: u16, steps: u16) -> Point<Pixels> {
    let angle = f32::from(step) / f32::from(steps) * std::f32::consts::TAU;
    center + point(px(angle.sin() * radius), px(-angle.cos() * radius))
}

fn paint_line(
    window: &mut Window,
    start: Point<Pixels>,
    end: Point<Pixels>,
    width: f32,
    color: u32,
) {
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(start);
    path.line_to(end);
    if let Ok(path) = path.build() {
        window.paint_path(path, rgb(color));
    }
}

#[cfg(test)]
mod tests {
    use super::{dial_step, parse_time};

    #[test]
    fn time_picker_accepts_end_midnight_only_for_end_fields() {
        assert_eq!(parse_time("9:07", false), Some(547));
        assert_eq!(parse_time(" 23:59 ", false), Some(1439));
        assert_eq!(parse_time("24:00", true), Some(1440));
        for value in [
            "24:00", "24:01", "25:00", "09:60", "9:7", "-1:00", "9:00:00",
        ] {
            assert_eq!(parse_time(value, false), None, "{value}");
        }
        assert_eq!(parse_time("24:01", true), None);
    }

    #[test]
    fn time_picker_clock_angles_snap_without_a_fake_duration() {
        for (x, y, hour, minute) in [
            (0., -100., 0, 0),
            (100., 0., 6, 15),
            (0., 100., 12, 30),
            (-100., 0., 18, 45),
        ] {
            assert_eq!(dial_step(x, y, 24), Some(hour));
            assert_eq!(dial_step(x, y, 60), Some(minute));
        }
        assert_eq!(dial_step(-1., -100., 60), Some(0));
        assert_eq!(dial_step(0., 0., 60), None);
        assert_eq!(dial_step(3., 5., 24), None);
    }
}
