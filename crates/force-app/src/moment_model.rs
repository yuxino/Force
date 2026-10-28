//! Point-in-time notes have an independent draft and never create timer intervals.
use crate::model::ForceView;
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use force_core::Moment;
use gpui::Focusable as _;
use gpui::{AppContext as _, Context, Entity, PromptButton, PromptLevel, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

gpui::actions!(moment_editor, [NextField, PreviousField]);

#[derive(Clone, PartialEq)]
struct MomentDraft {
    content: String,
    date: String,
    time: String,
}

pub(crate) struct MomentState {
    pub(crate) focus: gpui::FocusHandle,
    pub(crate) visible: bool,
    pub(crate) active: bool,
    pub(crate) editing: Option<i64>,
    pub(crate) content: Entity<InputState>,
    pub(crate) date: Entity<InputState>,
    pub(crate) time: Entity<InputState>,
    pub(crate) error: Option<String>,
    baseline: Option<MomentDraft>,
    source_timestamp: Option<i64>,
    generation: u64,
    _subscriptions: Vec<Subscription>,
}

impl MomentState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ForceView>) -> Self {
        cx.bind_keys([
            gpui::KeyBinding::new("tab", NextField, Some("MomentEditor")),
            gpui::KeyBinding::new("shift-tab", PreviousField, Some("MomentEditor")),
        ]);
        let content = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(4)
                .placeholder("这一刻，想记些什么？")
        });
        let date = cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD"));
        let time = cx.new(|cx| InputState::new(window, cx).placeholder("HH:MM"));
        let mut subscriptions = Vec::new();
        for input in [&content, &date, &time] {
            subscriptions.push(cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.moments.error = None;
                    cx.notify();
                }
            }));
        }
        Self {
            focus: cx.focus_handle().tab_stop(false),
            visible: false,
            active: false,
            editing: None,
            content,
            date,
            time,
            error: None,
            baseline: None,
            source_timestamp: None,
            generation: 0,
            _subscriptions: subscriptions,
        }
    }
    fn draft(&self, cx: &gpui::App) -> MomentDraft {
        MomentDraft {
            content: self.content.read(cx).value().to_string(),
            date: self.date.read(cx).value().to_string(),
            time: self.time.read(cx).value().to_string(),
        }
    }
    fn changed(&self, cx: &gpui::App) -> bool {
        self.active
            && self
                .baseline
                .as_ref()
                .is_some_and(|baseline| *baseline != self.draft(cx))
    }
}

impl ForceView {
    pub(crate) fn cycle_moment_focus(
        &self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let first = window.focused(cx);
        loop {
            if backwards {
                window.focus_prev();
            } else {
                window.focus_next();
            }
            if self.moments.focus.contains_focused(window, cx) {
                return;
            }
            if window.focused(cx) == first {
                break;
            }
        }
        self.moments.content.read(cx).focus_handle(cx).focus(window);
    }

    pub(crate) fn begin_moment(
        &mut self,
        date: Option<NaiveDate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.moments.active {
            let now = Local::now();
            let day = date.unwrap_or(now.date_naive());
            self.load_moment_draft(
                None,
                MomentDraft {
                    content: String::new(),
                    date: day.format("%Y-%m-%d").to_string(),
                    time: now.format("%H:%M").to_string(),
                },
                (day == now.date_naive()).then_some(now.timestamp()),
                window,
                cx,
            );
        }
        self.reveal_moment(window, cx);
    }

    pub(crate) fn edit_moment(
        &mut self,
        moment: Moment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.moments.active && self.moments.editing == Some(moment.id) {
            self.reveal_moment(window, cx);
            return;
        }
        if self.moments.changed(cx) {
            self.reveal_moment(window, cx);
            let generation = self.moments.generation;
            let draft = self.moments.draft(cx);
            let answer = window.prompt(
                PromptLevel::Info,
                "保存当前随记？",
                None,
                &[
                    PromptButton::cancel("取消"),
                    PromptButton::new("不保存"),
                    PromptButton::new("保存"),
                ],
                cx,
            );
            cx.spawn_in(window, async move |this, cx| {
                if let Ok(choice @ (1 | 2)) = answer.await {
                    let _ = cx.update(|window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            if this.moments.generation != generation
                                || this.moments.draft(cx) != draft
                            {
                                return;
                            }
                            if choice == 2 && !this.save_moment(window, cx) {
                                return;
                            }
                            this.moments.active = false;
                            this.edit_moment(moment, window, cx);
                        });
                    });
                }
            })
            .detach();
            return;
        }
        let Some(stamp) = Local.timestamp_opt(moment.occurred_at, 0).single() else {
            return;
        };
        self.load_moment_draft(
            Some(moment.id),
            MomentDraft {
                content: moment.content,
                date: stamp.format("%Y-%m-%d").to_string(),
                time: stamp.format("%H:%M").to_string(),
            },
            Some(moment.occurred_at),
            window,
            cx,
        );
        self.reveal_moment(window, cx);
    }

    fn load_moment_draft(
        &mut self,
        id: Option<i64>,
        draft: MomentDraft,
        source_timestamp: Option<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let MomentDraft {
            content,
            date,
            time,
        } = draft;
        self.moments.generation = self.moments.generation.wrapping_add(1);
        self.moments.active = true;
        self.moments.editing = id;
        self.moments.source_timestamp = source_timestamp;
        self.moments.error = None;
        self.moments
            .content
            .update(cx, |input, cx| input.set_value(content, window, cx));
        self.moments
            .date
            .update(cx, |input, cx| input.set_value(date, window, cx));
        self.moments
            .time
            .update(cx, |input, cx| input.set_value(time, window, cx));
        self.moments.baseline = Some(self.moments.draft(cx));
    }

    fn reveal_moment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_record_editor(window, cx);
        self.suspend_completion_editor(window, cx);
        self.time_picker
            .update(cx, |picker, cx| picker.dismiss(window, cx));
        self.suspend_agenda_popup(window, cx);
        self.tag_picker
            .update(cx, |picker, cx| picker.dismiss(window, false, cx));
        self.moments.visible = true;
        self.show_manager(window, cx);
        self.moments
            .content
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub(crate) fn suspend_moment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.time_picker
            .update(cx, |picker, cx| picker.dismiss(window, cx));
        if self.moments.visible {
            self.moments.visible = false;
            window.focus(&self.focus_handle);
            cx.notify();
        }
    }

    pub(crate) fn cancel_moment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.moments.visible = false;
        self.moments.active = false;
        self.moments.editing = None;
        self.moments.baseline = None;
        self.moments.source_timestamp = None;
        self.moments.error = None;
        self.moments.generation = self.moments.generation.wrapping_add(1);
        self.moments
            .content
            .update(cx, |input, cx| input.set_value("", window, cx));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn save_moment(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.moments.visible
            || !self.moments.active
            || self.compact
            || self.time_picker.read(cx).is_open()
        {
            return false;
        }
        let draft = self.moments.draft(cx);
        let content = draft.content.trim();
        let error = if content.is_empty() {
            Some("写点内容再保存。")
        } else if content.chars().count() > force_core::MOMENT_CONTENT_LIMIT {
            Some("内容请保持在 2000 字以内。")
        } else {
            None
        };
        let stamp = resolve_moment_time(
            &draft,
            self.moments.baseline.as_ref(),
            self.moments.source_timestamp,
        );
        if let Some(error) = error.or_else(|| stamp.as_ref().err().copied()) {
            self.moments.error = Some(error.to_owned());
            cx.notify();
            return false;
        }
        let stamp = stamp.expect("validated timestamp");
        let id = self.moments.editing;
        let now = Local::now().timestamp();
        if self.write(
            |store| match id {
                Some(id) => store.update_moment(id, content, stamp, now),
                None => store.create_moment(content, stamp, now),
            },
            cx,
        ) {
            self.cancel_moment(window, cx);
            self.notify("时刻已记录。", cx);
            true
        } else {
            false
        }
    }

    pub(crate) fn delete_moment(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let return_focus = window.focused(cx);
        let answer = window.prompt(
            PromptLevel::Warning,
            "删除这条随记？",
            None,
            &[PromptButton::cancel("取消"), PromptButton::new("删除")],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let choice = answer.await;
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if !this.compact {
                        window.activate_window();
                        if let Some(focus) = return_focus.as_ref() {
                            focus.focus(window);
                        }
                    }
                    match choice {
                        Ok(1) => {
                            if this.write(|store| store.delete_moment(id), cx) {
                                if this.moments.editing == Some(id) {
                                    this.cancel_moment(window, cx);
                                }
                                this.notify("随记已删除。", cx);
                            }
                        }
                        Ok(_) => {}
                        Err(_) => this.notify("无法确认删除，请重试。", cx),
                    }
                });
            });
        })
        .detach();
    }
}

/// Minute-only fields must not discard seconds or reinterpret a known instant
/// during a repeated local hour. Editing either field opts into strict parsing.
fn resolve_moment_time(
    draft: &MomentDraft,
    baseline: Option<&MomentDraft>,
    source_timestamp: Option<i64>,
) -> Result<i64, &'static str> {
    if let Some(stamp) = source_timestamp
        && baseline
            .is_some_and(|baseline| baseline.date == draft.date && baseline.time == draft.time)
    {
        return Ok(stamp);
    }
    parse_moment_time(&draft.date, &draft.time)
}

fn parse_moment_time(date: &str, time: &str) -> Result<i64, &'static str> {
    let date = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d")
        .map_err(|_| "日期格式为 YYYY-MM-DD。")?;
    use chrono::Datelike as _;
    if !(1..=9999).contains(&date.year()) {
        return Err("日期年份需在 1–9999 之间。");
    }
    let time = NaiveTime::parse_from_str(time.trim(), "%H:%M").map_err(|_| "时间格式为 HH:MM。")?;
    Local
        .from_local_datetime(&NaiveDateTime::new(date, time))
        .single()
        .map(|stamp| stamp.timestamp())
        .ok_or("这个本地时间不存在或有歧义，请调整时间。")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moment_dates_reject_invalid_days_and_time_ranges() {
        assert!(parse_moment_time("2026-10-28", "09:15").is_ok());
        for (day, time) in [
            ("2026-02-30", "12:00"),
            ("2026-10-28", "24:00"),
            ("2026-10-28", "12:60"),
            ("0-01-01", "12:00"),
        ] {
            assert!(parse_moment_time(day, time).is_err());
        }
    }

    #[test]
    fn unchanged_time_preserves_each_repeated_hour_instant_and_its_seconds() {
        let local_time = NaiveDate::from_ymd_opt(2026, 11, 1)
            .unwrap()
            .and_hms_opt(1, 30, 37)
            .unwrap();
        let baseline = MomentDraft {
            content: "Before".into(),
            date: "2026-11-01".into(),
            time: "01:30".into(),
        };
        let edited = MomentDraft {
            content: "After, with more detail".into(),
            ..baseline.clone()
        };
        let mut instants = Vec::new();
        // Both sides of a fall-back fold display the same date and minute. Use
        // fixed offsets so the regression is independent of the test machine.
        for hours_west in [4, 5] {
            let source = chrono::FixedOffset::west_opt(hours_west * 3600)
                .unwrap()
                .from_local_datetime(&local_time)
                .single()
                .unwrap()
                .timestamp();
            assert_eq!(
                resolve_moment_time(&edited, Some(&baseline), Some(source)),
                Ok(source)
            );
            assert_eq!(source.rem_euclid(60), 37);
            instants.push(source);
        }
        assert_eq!(instants[1] - instants[0], 3600);
    }

    #[test]
    fn changed_time_and_backdated_new_drafts_are_strictly_parsed() {
        let baseline = MomentDraft {
            content: "Note".into(),
            date: "2026-10-28".into(),
            time: "09:15".into(),
        };
        let source = parse_moment_time(&baseline.date, &baseline.time).unwrap() + 37;
        let changed = MomentDraft {
            time: "10:20".into(),
            ..baseline.clone()
        };
        let expected = parse_moment_time(&changed.date, &changed.time).unwrap();
        assert_eq!(
            resolve_moment_time(&changed, Some(&baseline), Some(source)),
            Ok(expected)
        );
        assert_eq!(expected.rem_euclid(60), 0);
        let backdated = MomentDraft {
            date: "2026-10-27".into(),
            ..baseline.clone()
        };
        assert_eq!(
            resolve_moment_time(&backdated, Some(&backdated), None),
            parse_moment_time("2026-10-27", "09:15")
        );
        for invalid in [
            MomentDraft {
                date: "2026-02-30".into(),
                ..baseline.clone()
            },
            MomentDraft {
                time: "24:00".into(),
                ..baseline.clone()
            },
        ] {
            assert!(resolve_moment_time(&invalid, Some(&baseline), Some(source)).is_err());
        }
    }
}
