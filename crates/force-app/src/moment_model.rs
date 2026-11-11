//! Point-in-time notes have an independent draft and never create timer intervals.
use crate::model::{ForceView, Page};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use force_core::{DailyEntry, Moment};
use gpui::Focusable as _;
use gpui::{AppContext as _, Context, Entity, PromptButton, PromptLevel, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

gpui::actions!(moment_editor, [NextField, PreviousField]);

#[derive(Clone, Default, PartialEq)]
struct MomentDraft {
    content: String,
    date: String,
    time: String,
    day_only: bool,
    minutes: String,
}

enum SavedNote {
    Timed(Moment),
    Daily(DailyEntry),
}

pub(crate) struct MomentState {
    pub(crate) focus: gpui::FocusHandle,
    pub(crate) visible: bool,
    pub(crate) active: bool,
    pub(crate) editing: Option<i64>,
    pub(crate) daily_original: Option<DailyEntry>,
    pub(crate) day_only: bool,
    pub(crate) content: Entity<InputState>,
    pub(crate) date: Entity<InputState>,
    pub(crate) time: Entity<InputState>,
    pub(crate) minutes: Entity<InputState>,
    pub(crate) error: Option<String>,
    pub(crate) editor_scroll: gpui::ScrollHandle,
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
        let minutes = cx.new(|cx| InputState::new(window, cx).placeholder("例如 60"));
        let mut subscriptions = Vec::new();
        for input in [&content, &date, &time, &minutes] {
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
            daily_original: None,
            day_only: false,
            content,
            date,
            time,
            minutes,
            error: None,
            editor_scroll: gpui::ScrollHandle::default(),
            baseline: None,
            source_timestamp: None,
            generation: 0,
            _subscriptions: subscriptions,
        }
    }
    pub(crate) fn is_editing(&self) -> bool {
        self.editing.is_some() || self.daily_original.is_some()
    }

    fn draft(&self, cx: &gpui::App) -> MomentDraft {
        MomentDraft {
            content: self.content.read(cx).value().to_string(),
            date: self.date.read(cx).value().to_string(),
            time: self.time.read(cx).value().to_string(),
            day_only: self.day_only,
            minutes: self.minutes.read(cx).value().to_string(),
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
        // A new note must never silently resume editing an existing note.
        if self.moments.active && self.moments.is_editing() {
            if self.moments.changed(cx) {
                self.reveal_moment(window, cx);
                let generation = self.moments.generation;
                let draft = self.moments.draft(cx);
                let answer = window.prompt(
                    PromptLevel::Info,
                    "保存当前记录？",
                    None,
                    &[
                        PromptButton::cancel("取消"),
                        PromptButton::new("不保存"),
                        PromptButton::new("保存"),
                    ],
                    cx,
                );
                cx.spawn_in(window, async move |this, cx| {
                    let choice = answer.await;
                    let _ = cx.update(|window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            if this.moments.generation != generation
                                || this.moments.draft(cx) != draft
                            {
                                return;
                            }
                            if !this.compact {
                                window.activate_window();
                                this.moments.content.read(cx).focus_handle(cx).focus(window);
                            }
                            if let Ok(choice @ (1 | 2)) = choice {
                                if choice == 2 && !this.save_moment(window, cx) {
                                    this.clear_creation_seed();
                                    return;
                                }
                                this.moments.active = false;
                                this.begin_moment(date, window, cx);
                            } else {
                                this.clear_creation_seed();
                            }
                        });
                    });
                })
                .detach();
                return;
            }
            self.moments.active = false;
        }
        if !self.moments.active {
            let now = Local::now();
            let day = date.unwrap_or(now.date_naive());
            self.load_moment_draft(
                None,
                MomentDraft {
                    content: String::new(),
                    date: day.format("%Y-%m-%d").to_string(),
                    time: now.format("%H:%M").to_string(),
                    ..MomentDraft::default()
                },
                (day == now.date_naive()).then_some(now.timestamp()),
                window,
                cx,
            );
            self.seed_creation(crate::creation::CreationKind::Moment, window, cx);
        } else {
            // Resuming a draft must not retain fields intended for a new note.
            self.clear_creation_seed();
        }
        self.reveal_moment(window, cx);
    }

    pub(crate) fn edit_moment(
        &mut self,
        moment: Moment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_saved_note(SavedNote::Timed(moment), window, cx);
    }

    pub(crate) fn edit_daily_entry(
        &mut self,
        entry: DailyEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_saved_note(SavedNote::Daily(entry), window, cx);
    }

    fn edit_saved_note(&mut self, note: SavedNote, window: &mut Window, cx: &mut Context<Self>) {
        let same_entry = match &note {
            SavedNote::Timed(moment) => self.moments.editing == Some(moment.id),
            SavedNote::Daily(entry) => self
                .moments
                .daily_original
                .as_ref()
                .is_some_and(|original| original.id == entry.id),
        };
        if self.moments.active && same_entry {
            self.reveal_moment(window, cx);
            return;
        }
        if self.moments.changed(cx) {
            self.reveal_moment(window, cx);
            let generation = self.moments.generation;
            let draft = self.moments.draft(cx);
            let answer = window.prompt(
                PromptLevel::Info,
                "保存当前记录？",
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
                            this.edit_saved_note(note, window, cx);
                        });
                    });
                }
            })
            .detach();
            return;
        }
        match note {
            SavedNote::Timed(moment) => {
                let Some(stamp) = Local.timestamp_opt(moment.occurred_at, 0).single() else {
                    return;
                };
                self.load_moment_draft(
                    Some(moment.id),
                    MomentDraft {
                        content: moment.content,
                        date: stamp.format("%Y-%m-%d").to_string(),
                        time: stamp.format("%H:%M").to_string(),
                        ..MomentDraft::default()
                    },
                    Some(moment.occurred_at),
                    window,
                    cx,
                );
            }
            SavedNote::Daily(entry) => {
                self.load_moment_draft(
                    None,
                    MomentDraft {
                        content: entry.content.clone(),
                        date: entry.day.to_string(),
                        day_only: true,
                        minutes: entry
                            .duration_minutes
                            .map_or_else(String::new, |minutes| minutes.to_string()),
                        ..MomentDraft::default()
                    },
                    None,
                    window,
                    cx,
                );
                self.moments.daily_original = Some(entry);
            }
        }
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
            day_only,
            minutes,
        } = draft;
        self.moments.generation = self.moments.generation.wrapping_add(1);
        self.moments.active = true;
        self.moments.editing = id;
        self.moments.daily_original = None;
        self.moments.day_only = day_only;
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
        self.moments
            .minutes
            .update(cx, |input, cx| input.set_value(minutes, window, cx));
        self.moments.baseline = Some(self.moments.draft(cx));
    }

    fn reveal_moment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspend_life_editor(window, cx);
        self.suspend_task_dialog(window, cx);
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
        self.moments.daily_original = None;
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
            || self.storage_failed
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
        if draft.day_only {
            if let Some(error) = error {
                self.moments.error = Some(error.to_owned());
                cx.notify();
                return false;
            }
            return self.save_daily_note(&draft, window, cx);
        }
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

    fn save_daily_note(
        &mut self,
        draft: &MomentDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let today = Local::now().date_naive();
        let parsed = parse_daily_fields(&draft.date, &draft.minutes, today);
        let (day, minutes) = match parsed {
            Ok(fields) => fields,
            Err(error) => {
                self.moments.error = Some(error.to_owned());
                cx.notify();
                return false;
            }
        };
        let original = self.moments.daily_original.clone();
        if !self.write(
            |store| match &original {
                Some(entry) => store.update_daily_entry(entry, day, &draft.content, minutes, today),
                None => store.create_daily_entry(day, &draft.content, minutes, today),
            },
            cx,
        ) {
            return false;
        }
        self.cancel_moment(window, cx);
        self.select_record_date(day, cx);
        if self.page != Page::Records {
            self.navigate(Page::Records, window, cx);
        }
        self.notify("按日记录已保存。", cx);
        true
    }

    pub(crate) fn delete_daily_entry(
        &mut self,
        entry: DailyEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = self.moments.generation;
        let return_focus = window.focused(cx);
        let answer = window.prompt(
            PromptLevel::Warning,
            "删除这条按日记录？",
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
                        if let Some(focus) = &return_focus {
                            focus.focus(window);
                        }
                    }
                    if matches!(choice, Ok(1))
                        && this.write(|store| store.delete_daily_entry(&entry), cx)
                    {
                        if this.moments.generation == generation {
                            this.cancel_moment(window, cx);
                        }
                        this.notify("按日记录已删除。", cx);
                    }
                });
            });
        })
        .detach();
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
    let date = parse_note_date(date)?;
    let time = NaiveTime::parse_from_str(time.trim(), "%H:%M").map_err(|_| "时间格式为 HH:MM。")?;
    Local
        .from_local_datetime(&NaiveDateTime::new(date, time))
        .single()
        .map(|stamp| stamp.timestamp())
        .ok_or("这个本地时间不存在或有歧义，请调整时间。")
}

fn parse_note_date(value: &str) -> Result<NaiveDate, &'static str> {
    let day = NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| "日期格式为 YYYY-MM-DD。")?;
    if !(1..=9999).contains(&day.year()) {
        return Err("日期年份需在 1–9999 之间。");
    }
    Ok(day)
}

pub(crate) fn parse_daily_fields(
    date: &str,
    minutes: &str,
    today: NaiveDate,
) -> Result<(NaiveDate, Option<u16>), &'static str> {
    let day = parse_note_date(date)?;
    if day > today {
        return Err("记录日期不能晚于今天。");
    }
    let minutes = minutes.trim();
    let duration = if minutes.is_empty() {
        None
    } else {
        Some(
            minutes
                .parse::<u16>()
                .ok()
                .filter(|minutes| (1..=1440).contains(minutes))
                .ok_or("请输入 1–1440 之间的分钟数，或留空。")?,
        )
    };
    Ok((day, duration))
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
            ..MomentDraft::default()
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
            ..MomentDraft::default()
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

#[cfg(all(test, feature = "ui-test-support"))]
mod creation_tests {
    use super::*;
    use crate::{headless_ui_tests::ManagerHarness, model::Page};
    use gpui::TestAppContext;

    fn edit_saved_note(manager: &mut ManagerHarness) -> Moment {
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let now = Local::now().timestamp();
            let moment = view
                .store
                .as_mut()
                .unwrap()
                .create_moment("Saved note", now, now)
                .unwrap();
            view.refresh(cx);
            view.edit_moment(moment.clone(), window, cx);
            moment
        })
    }

    fn settle(manager: &mut ManagerHarness) {
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
    }

    #[gpui::test]
    fn new_note_leaves_unchanged_existing_note_intact(cx: &mut TestAppContext) {
        let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
        let moment = edit_saved_note(&mut manager);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.begin_moment(None, window, cx);
        });
        settle(&mut manager);
        assert!(!manager.cx.has_pending_prompt());
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.moments.visible);
            assert!(view.moments.editing.is_none());
            assert!(view.moments.content.read(cx).value().is_empty());
            assert_eq!(view.record_moments, vec![moment]);
        });
    }

    #[gpui::test]
    fn new_note_protects_an_edited_existing_note(cx: &mut TestAppContext) {
        for answer in ["取消", "不保存", "保存"] {
            let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
            let moment = edit_saved_note(&mut manager);
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                view.moments
                    .content
                    .update(cx, |input, cx| input.set_value("Edited note", window, cx));
                view.suspend_moment(window, cx);
                view.begin_moment(None, window, cx);
            });
            settle(&mut manager);
            assert!(manager.cx.has_pending_prompt());
            manager.cx.simulate_prompt_answer(answer);
            settle(&mut manager);
            manager.view.read_with(&manager.cx, |view, cx| {
                assert!(view.moments.visible);
                assert_eq!(view.record_moments.len(), 1);
                assert_eq!(view.record_moments[0].id, moment.id);
                assert_eq!(
                    view.record_moments[0].content,
                    if answer == "保存" {
                        "Edited note"
                    } else {
                        "Saved note"
                    }
                );
                if answer == "取消" {
                    assert_eq!(view.moments.editing, Some(moment.id));
                    assert_eq!(view.moments.content.read(cx).value(), "Edited note");
                } else {
                    assert!(view.moments.editing.is_none());
                    assert!(view.moments.content.read(cx).value().is_empty());
                }
            });
        }
    }

    #[gpui::test]
    fn new_note_keeps_invalid_existing_edits_when_save_is_rejected(cx: &mut TestAppContext) {
        let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
        let moment = edit_saved_note(&mut manager);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.moments
                .date
                .update(cx, |input, cx| input.set_value("2026-02-30", window, cx));
            view.begin_moment(None, window, cx);
        });
        settle(&mut manager);
        manager.cx.simulate_prompt_answer("保存");
        settle(&mut manager);
        manager.view.read_with(&manager.cx, |view, cx| {
            assert_eq!(view.moments.editing, Some(moment.id));
            assert_eq!(view.moments.date.read(cx).value(), "2026-02-30");
            assert!(view.moments.error.is_some());
            assert_eq!(view.record_moments, vec![moment]);
        });
    }

    #[gpui::test]
    fn outdated_new_note_confirmation_cannot_replace_a_newer_draft(cx: &mut TestAppContext) {
        let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
        edit_saved_note(&mut manager);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.moments
                .content
                .update(cx, |input, cx| input.set_value("Edited note", window, cx));
            view.begin_moment(None, window, cx);
        });
        settle(&mut manager);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.cancel_moment(window, cx);
            view.begin_moment(None, window, cx);
            view.moments
                .content
                .update(cx, |input, cx| input.set_value("Newer note", window, cx));
        });
        manager.cx.simulate_prompt_answer("不保存");
        settle(&mut manager);
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.moments.editing.is_none());
            assert_eq!(view.moments.content.read(cx).value(), "Newer note");
            assert_eq!(view.record_moments.len(), 1);
            assert_eq!(view.record_moments[0].content, "Saved note");
        });
    }
}
