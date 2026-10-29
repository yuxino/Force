//! Point-note regressions on TestPlatform; native input and rendering need separate acceptance.

use chrono::{Datelike, Local, NaiveDate};
use force_core::{Moment, TagClass};
use gpui::{Bounds, Focusable as _, KeyBinding, Pixels, Point, TestAppContext, px, size};

use crate::{
    agenda_model::AgendaItemStatus,
    headless_ui_tests::ManagerHarness,
    model::{Page, day_bounds},
};

fn settle(manager: &mut ManagerHarness) {
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
}

fn bind_editor_actions(manager: &mut ManagerHarness) {
    let handle = manager.handle;
    manager.cx.update(|_, cx| {
        cx.bind_keys([
            KeyBinding::new("escape", crate::ClosePanel, Some("Force")),
            KeyBinding::new("secondary-enter", crate::SaveMoment, None),
        ]);
        cx.on_action(move |_: &crate::ClosePanel, cx| {
            crate::with_force(handle, cx, |view, window, cx| view.close_panel(window, cx))
        });
        cx.on_action(move |_: &crate::SaveMoment, cx| {
            crate::with_force(handle, cx, |view, window, cx| {
                view.save_moment(window, cx);
            })
        });
    });
}

fn set_time(manager: &mut ManagerHarness, day: NaiveDate, time: &str) {
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.moments.date.update(cx, |input, cx| {
            input.set_value(day.format("%Y-%m-%d").to_string(), window, cx)
        });
        view.moments
            .time
            .update(cx, |input, cx| input.set_value(time.to_owned(), window, cx));
    });
    settle(manager);
}

fn seed_moment(manager: &mut ManagerHarness, day: NaiveDate, content: &str) -> Moment {
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let moment = view
            .store
            .as_mut()
            .unwrap()
            .create_moment(content, day_bounds(day).0 + 3600, Local::now().timestamp())
            .unwrap();
        view.refresh(cx);
        moment
    })
}

fn assert_inside(child: Bounds<Pixels>, parent: Bounds<Pixels>) {
    let tolerance = px(1.);
    assert!(
        child.left() >= parent.left() - tolerance,
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.right() <= parent.right() + tolerance,
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.top() >= parent.top() - tolerance,
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.bottom() <= parent.bottom() + tolerance,
        "{child:?} outside {parent:?}"
    );
}

#[gpui::test]
fn moment_ui_multiline_note_saves_as_a_point_without_starting_or_counting_time(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
    bind_editor_actions(&mut manager);
    let day = Local::now().date_naive().pred_opt().unwrap();
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        view.record_date = day;
        view.agenda.date = day;
        view.agenda.month = day.with_day(1).unwrap();
        view.refresh(cx);
    });
    settle(&mut manager);
    manager.click("global-new-moment");
    manager.cx.simulate_input("A small observation");
    manager.cx.simulate_keystrokes("enter");
    manager.cx.simulate_input("Keep the second line");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(
            view.moments.content.read(cx).value(),
            "A small observation\nKeep the second line"
        );
        assert!(view.moments.visible);
        assert!(
            view.record_moments.is_empty(),
            "ordinary Return must not save"
        );
    });
    set_time(&mut manager, day, "09:15");
    manager.cx.simulate_keystrokes("secondary-enter");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.moments.visible);
        assert!(!view.moments.active);
        assert_eq!(view.record_moments.len(), 1);
        assert_eq!(
            view.record_moments[0].content,
            "A small observation\nKeep the second line"
        );
        assert_eq!(view.agenda.day_moments.len(), 1);
        let month = &view.agenda.month_days[&day];
        assert_eq!(month.len(), 1);
        assert_eq!(month[0].status, AgendaItemStatus::Moment);
        assert_eq!(month[0].seconds, 0);
        assert!(month[0].title.starts_with("09:15 "));
        assert!(view.tasks.is_empty());
        assert!(view.active.is_empty());
        assert!(view.history.is_empty());
        assert_eq!(view.today.focus_seconds, 0);
        assert_eq!(view.record_stats.focus_seconds, 0);
        assert!(view.activity_days.iter().all(|day| day.seconds == 0));
    });
    manager.cx.simulate_keystrokes("secondary-enter");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.record_moments.len(), 1)
    });
}

#[gpui::test]
fn moment_ui_cat_task_reveal_suspends_note_without_losing_either_draft(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.new_task(window, cx);
        view.selected_tag = 1;
        view.task_input
            .update(cx, |input, cx| input.set_value("Prepared task", window, cx));
        view.countdown = true;
        view.timer_input
            .update(cx, |input, cx| input.set_value("0", window, cx));
        view.begin_moment(None, window, cx);
        view.moments.content.update(cx, |input, cx| {
            input.set_value("Keep this unsaved thought", window, cx)
        });
        view.moments
            .date
            .update(cx, |input, cx| input.set_value("2026-02-30", window, cx));
        view.moments
            .time
            .update(cx, |input, cx| input.set_value("09:15", window, cx));
    });
    settle(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(view.moments.visible);
        view.hide_manager(window, cx);
        view.show_strip_task(window, cx);
        assert_eq!(view.page, Page::Focus);
        assert!(!view.compact);
        assert!(!view.moments.visible);
        assert!(view.moments.active);
        assert!(
            view.timer_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert_eq!(view.task_input.read(cx).value(), "Prepared task");
        assert_eq!(view.selected_tag, 1);
        assert!(view.countdown);
        assert_eq!(view.timer_input.read(cx).value(), "0");
    });
    settle(&mut manager);
    assert!(!manager.cx.has_pending_prompt());
    manager.click("global-new-moment");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.moments.visible);
        assert!(view.moments.editing.is_none());
        assert_eq!(
            view.moments.content.read(cx).value(),
            "Keep this unsaved thought"
        );
        assert_eq!(view.moments.date.read(cx).value(), "2026-02-30");
        assert_eq!(view.moments.time.read(cx).value(), "09:15");
        assert!(view.tasks.is_empty());
        assert!(view.active.is_empty());
        assert!(view.record_moments.is_empty());
    });
}

#[gpui::test]
fn moment_ui_invalid_and_suspended_drafts_survive_navigation_and_window_reopening(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    bind_editor_actions(&mut manager);
    let day = Local::now().date_naive();
    let existing = seed_moment(&mut manager, day, "Existing point");
    manager.click("global-new-moment");
    manager.cx.simulate_input("Keep this unsaved thought");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.moments
            .date
            .update(cx, |input, cx| input.set_value("2026-02-30", window, cx));
    });
    settle(&mut manager);
    manager.click("save-moment");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.moments.visible);
        assert!(view.moments.error.is_some());
        assert_eq!(
            view.moments.content.read(cx).value(),
            "Keep this unsaved thought"
        );
        assert_eq!(view.record_moments.len(), 1);
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_moment(existing.clone(), window, cx);
    });
    settle(&mut manager);
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.moments.visible);
        assert!(
            view.moments.editing.is_none(),
            "invalid draft cannot switch targets"
        );
        assert!(view.moments.error.is_some());
    });
    manager.cx.simulate_keystrokes("escape");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.moments.visible);
        assert!(view.moments.active);
        assert!(!view.compact);
    });
    manager.cx.simulate_keystrokes("secondary-enter");
    settle(&mut manager);
    manager.click("records-tab");
    manager.click("global-new-moment");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert_eq!(
            view.moments.content.read(cx).value(),
            "Keep this unsaved thought"
        );
        assert_eq!(view.moments.date.read(cx).value(), "2026-02-30");
        assert!(
            view.moments
                .content
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        view.hide_manager(window, cx);
        view.show_manager(window, cx);
        assert!(
            view.moments
                .content
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    });
    manager.cx.simulate_input(" continued");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(
            view.moments.content.read(cx).value(),
            "Keep this unsaved thought continued"
        );
        assert_eq!(view.record_moments.len(), 1);
    });
    manager.click("cancel-moment");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.moments.active);
        assert!(view.moments.content.read(cx).value().is_empty());
    });
}

#[gpui::test]
fn moment_ui_switching_edit_targets_protects_changes_and_delete_requires_confirmation(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Records);
    let day = Local::now().date_naive();
    let previous = day.pred_opt().unwrap();
    let first = seed_moment(&mut manager, day, "First point");
    let second = seed_moment(&mut manager, day, "Second point");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_moment(first.clone(), window, cx);
        view.moments.content.update(cx, |input, cx| {
            input.set_value("Edited first point", window, cx)
        });
    });
    set_time(&mut manager, previous, "08:30");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_moment(second.clone(), window, cx)
    });
    settle(&mut manager);
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.moments.editing, Some(first.id));
        assert_eq!(view.moments.content.read(cx).value(), "Edited first point");
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_moment(second.clone(), window, cx)
    });
    settle(&mut manager);
    manager.cx.simulate_prompt_answer("保存");
    settle(&mut manager);
    let saved_first = manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.moments.editing, Some(second.id));
        let store = view.store.as_ref().unwrap();
        let (start, end) = day_bounds(previous);
        let saved = store.moments_between(start, end).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, first.id);
        assert_eq!(saved[0].content, "Edited first point");
        let (start, end) = day_bounds(day);
        assert_eq!(
            store.moments_between(start, end).unwrap(),
            vec![second.clone()]
        );
        saved[0].clone()
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.moments.content.update(cx, |input, cx| {
            input.set_value("Discard second edit", window, cx)
        });
        view.edit_moment(saved_first.clone(), window, cx);
    });
    settle(&mut manager);
    manager.cx.simulate_prompt_answer("不保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.moments.editing, Some(first.id));
        assert_eq!(view.moments.content.read(cx).value(), "Edited first point");
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.delete_moment(first.id, window, cx)
    });
    settle(&mut manager);
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        let (start, end) = day_bounds(previous);
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .moments_between(start, end)
                .unwrap()
                .len(),
            1
        );
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.delete_moment(first.id, window, cx)
    });
    settle(&mut manager);
    manager.cx.simulate_prompt_answer("删除");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        let (start, end) = day_bounds(previous);
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .moments_between(start, end)
                .unwrap()
                .is_empty()
        );
        assert!(!view.moments.active);
        assert_eq!(view.record_moments, vec![second.clone()]);
    });
}

#[gpui::test]
fn moment_ui_editor_delete_preserves_calendar_plan_draft_and_running_timer(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        let day = Local::now().date_naive().pred_opt().unwrap();
        let moment = seed_moment(&mut manager, day, "已有随记");
        let running = manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let running = view
                .store
                .as_mut()
                .unwrap()
                .create_and_start_task("保持计时", 1, view.now - 300, None)
                .unwrap();
            view.refresh(cx);
            view.open_agenda_day(day, window, cx);
            view.begin_plan(None, window, cx);
            view.agenda.title_input.update(cx, |input, cx| {
                input.set_value("尚未保存的计划", window, cx)
            });
            view.edit_moment(moment.clone(), window, cx);
            view.moments
                .content
                .update(cx, |input, cx| input.set_value("保留这次修改", window, cx));
            running
        });
        settle(&mut manager);
        let viewport = Bounds::new(Point::default(), size(px(width), px(height)));
        let editor = manager.bounds("moment-editor");
        assert_inside(editor, viewport);
        for selector in ["save-moment", "cancel-moment", "delete-moment-editor"] {
            assert_inside(manager.bounds(selector), editor);
        }
        assert!(
            manager.bounds("delete-moment-editor").right() < manager.bounds("cancel-moment").left()
        );
        assert!(manager.bounds("cancel-moment").right() < manager.bounds("save-moment").left());
        manager.click("delete-moment-editor");
        assert!(manager.cx.has_pending_prompt());
        manager.cx.simulate_prompt_answer("取消");
        settle(&mut manager);
        manager.view.read_with(&manager.cx, |view, cx| {
            assert_eq!(view.page, Page::Agenda);
            assert_eq!(view.agenda.date, day);
            assert!(view.moments.visible && view.moments.active);
            assert_eq!(view.moments.editing, Some(moment.id));
            assert_eq!(view.moments.content.read(cx).value(), "保留这次修改");
            assert_eq!(view.agenda.day_moments, vec![moment.clone()]);
            assert!(view.agenda_has_suspended_draft(cx));
        });
        // No click or programmatic refocus after the native prompt: ordinary
        // key input must still reach the existing draft, without saving it.
        manager.cx.simulate_keystrokes("x");
        settle(&mut manager);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert!(
                view.moments
                    .content
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
            assert_eq!(view.moments.content.read(cx).value(), "x保留这次修改");
            assert!(view.moments.visible && view.moments.active);
            let (start, end) = day_bounds(day);
            assert_eq!(
                view.store
                    .as_ref()
                    .unwrap()
                    .moments_between(start, end)
                    .unwrap(),
                vec![moment.clone()],
                "canceling deletion and typing must not write the draft"
            );
            assert_eq!(view.agenda.day_moments, vec![moment.clone()]);
            assert_eq!(view.agenda.title_input.read(cx).value(), "尚未保存的计划");
        });
        manager.click("delete-moment-editor");
        manager.cx.simulate_prompt_answer("删除");
        settle(&mut manager);
        manager.view.read_with(&manager.cx, |view, cx| {
            assert_eq!(view.page, Page::Agenda);
            assert_eq!(view.agenda.date, day);
            assert!(!view.moments.visible && !view.moments.active);
            assert_eq!(view.moments.editing, None);
            assert!(view.agenda.day_moments.is_empty());
            assert!(view.agenda_has_suspended_draft(cx));
            assert_eq!(view.agenda.title_input.read(cx).value(), "尚未保存的计划");
            assert_eq!(
                view.store.as_ref().unwrap().active_sessions().unwrap(),
                vec![running.clone()]
            );
            let (start, end) = day_bounds(day);
            assert!(
                view.store
                    .as_ref()
                    .unwrap()
                    .moments_between(start, end)
                    .unwrap()
                    .is_empty()
            );
        });
    }
}

#[gpui::test]
fn moment_ui_editor_fits_and_record_search_does_not_classify_point_notes(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Records);
        manager.click("global-new-moment");
        assert!(manager.cx.debug_bounds("delete-moment-editor").is_none());
        manager
            .cx
            .simulate_input(&"A long thought with details\n".repeat(12));
        settle(&mut manager);
        let viewport = Bounds::new(Point::default(), size(px(width), px(height)));
        let editor = manager.bounds("moment-editor");
        assert_inside(editor, viewport);
        for selector in [
            "moment-content",
            "moment-date",
            "moment-time",
            "moment-message",
            "save-moment",
            "cancel-moment",
            "close-moment",
        ] {
            assert_inside(manager.bounds(selector), editor);
        }
        let content = manager.bounds("moment-content");
        assert!(
            content.size.height >= px(120.),
            "multiple note lines must stay visible"
        );
        assert_inside(content, manager.bounds("moment-editor-fields"));
        let date = manager.bounds("moment-date");
        let time = manager.bounds("moment-time");
        assert!(content.bottom() <= date.top());
        assert!(date.right() < time.left());
        manager.click("cancel-moment");
        let today = Local::now().date_naive();
        // GPUI 0.2.2 retains debug_bounds across frames. Set the filter before
        // either note is drawn so absence is evidence from its first render.
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.record_search
                .update(cx, |input, cx| input.set_value("needle", window, cx));
            view.record_class_filter = Some(TagClass::Negative);
            cx.notify();
        });
        let wanted = seed_moment(
            &mut manager,
            today,
            "Needle: a point\nAnother line to keep readable",
        );
        let hidden = seed_moment(&mut manager, today, "Other observation");
        settle(&mut manager);
        assert!(
            manager
                .cx
                .debug_bounds(Box::leak(
                    format!("moment-row-{}", wanted.id).into_boxed_str()
                ))
                .is_some()
        );
        assert!(
            manager
                .cx
                .debug_bounds(Box::leak(
                    format!("moment-row-{}", hidden.id).into_boxed_str()
                ))
                .is_none()
        );
        let notes = manager.bounds("record-moments");
        let parent = manager.bounds("manager-content");
        assert!(notes.left() >= parent.left());
        assert!(notes.right() <= parent.right());
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.record_moments.len(), 2);
            assert_eq!(view.record_stats.focus_seconds, 0);
            assert!(view.history.is_empty());
        });
    }
}

#[gpui::test]
fn moment_modal_tab_cycle_never_reaches_background_task(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 780., 620.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.new_task(window, cx);
        view.task_input.update(cx, |input, cx| {
            input.set_value("Keep background draft", window, cx)
        });
        view.begin_moment(None, window, cx);
    });
    settle(&mut manager);
    for key in ["tab", "shift-tab"] {
        for _ in 0..12 {
            manager.cx.simulate_keystrokes(key);
            settle(&mut manager);
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                assert!(view.moments.focus.contains_focused(window, cx));
                assert_eq!(view.task_input.read(cx).value(), "Keep background draft");
                assert!(view.moments.content.read(cx).value().is_empty());
                assert!(view.tasks.is_empty());
            });
        }
    }
}
