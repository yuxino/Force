//! Calendar event/layout regressions on GPUI TestPlatform. Native pixels and
//! actual IME composition remain part of signed-app acceptance.

use chrono::{Datelike, Days, Local, NaiveDate};
use force_core::{PlanDraft, PlanEntry};
use gpui::{
    Bounds, Focusable as _, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point,
    TestAppContext, px, size,
};
use gpui_component::input::InputEvent;

use crate::{
    headless_ui_tests::ManagerHarness,
    model::{Page, day_bounds},
    tag_picker::TagPicked,
};

fn settle(manager: &mut ManagerHarness) {
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
}

fn day_manager(cx: &mut TestAppContext, width: f32, height: f32, page: Page) -> ManagerHarness {
    let mut manager = ManagerHarness::new_on_page(cx, width, height, page);
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        view.agenda.month_overview = false;
        cx.notify();
    });
    settle(&mut manager);
    manager
}

#[gpui::test]
fn agenda_ui_month_overview_shows_task_titles_and_overflow_inside_each_day(
    cx: &mut TestAppContext,
) {
    use crate::agenda_model::AgendaMonthItem;
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            let date = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
            view.agenda.date = date;
            view.agenda.month = date.with_day(1).unwrap();
            view.agenda.month_days.insert(
                date,
                (0..5)
                    .map(|index| AgendaMonthItem {
                        title: format!("复习操作系统与整理当天的学习记录 {index}"),
                        seconds: 1800,
                        planned: false,
                        completed: index == 0,
                        rest: false,
                        moved: false,
                    })
                    .collect(),
            );
            cx.notify();
        });
        settle(&mut manager);
        let viewport = Bounds::new(Point::default(), size(px(width), px(height)));
        let grid = manager.bounds("agenda-overview-grid");
        let day = manager.bounds("agenda-month-day-2026-08-15");
        let title = manager.bounds("agenda-month-title-2026-08-15-0");
        assert_inside(grid, viewport);
        assert_inside(day, grid);
        assert_inside(title, day);
        assert!(title.size.width >= px(80.));
        assert_inside(manager.bounds("agenda-month-more-2026-08-15"), day);
        let date_header = manager.bounds("agenda-month-date-header-2026-08-15");
        assert!(date_header.top() - day.top() <= px(10.));
        let empty_day = manager.bounds("agenda-month-day-2026-08-14");
        let empty_header = manager.bounds("agenda-month-date-header-2026-08-14");
        assert!(
            (date_header.top() - day.top() - (empty_header.top() - empty_day.top())).abs() < px(1.)
        );
        assert_inside(manager.bounds("agenda-month-day-2026-08-31"), grid);
        manager.click("agenda-month-day-2026-08-15");
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(!view.agenda.month_overview);
            assert_eq!(
                view.agenda.date,
                NaiveDate::from_ymd_opt(2026, 8, 15).unwrap()
            );
            assert!(view.active.is_empty());
        });
        manager.bounds("agenda-day-grid");
        manager.click("agenda-back-to-month");
        manager
            .view
            .read_with(&manager.cx, |view, _| assert!(view.agenda.month_overview));
    }
}

#[gpui::test]
fn agenda_ui_month_navigation_today_and_add_keep_the_correct_date(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let today = Local::now().date_naive();
    manager.click("agenda-overview-next");
    manager.click("agenda-overview-today");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.agenda.month_overview);
        assert_eq!(view.agenda.month, today.with_day(1).unwrap());
    });
    manager.click("agenda-overview-next");
    let month = manager
        .view
        .read_with(&manager.cx, |view, _| view.agenda.month);
    manager.click("agenda-overview-add");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editing);
        assert_eq!(view.agenda.date, month);
        assert_eq!(
            view.agenda.date_input.read(cx).value(),
            month.format("%Y-%m-%d").to_string()
        );
        assert!(view.active.is_empty());
    });
    assert_inside(
        manager.bounds("agenda-save"),
        manager.bounds("agenda-editor"),
    );
}

#[gpui::test]
fn agenda_ui_failed_month_storage_keeps_retry_visible_and_prevents_new_plans(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        view.storage_failed = true;
        cx.notify();
    });
    settle(&mut manager);
    let viewport = Bounds::new(Point::default(), size(px(780.), px(620.)));
    assert_inside(manager.bounds("retry-storage"), viewport);
    manager.click("agenda-overview-add");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.agenda.editing);
        assert!(view.active.is_empty());
    });
    manager.click("retry-storage");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.storage_failed);
        assert!(view.agenda.month_overview);
    });
    assert_inside(manager.bounds("agenda-overview-grid"), viewport);
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

fn assert_horizontally_inside(child: Bounds<Pixels>, parent: Bounds<Pixels>) {
    assert!(
        child.left() >= parent.left() - px(1.),
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.right() <= parent.right() + px(1.),
        "{child:?} outside {parent:?}"
    );
}

fn fill_plan(
    manager: &mut ManagerHarness,
    title: &str,
    date: NaiveDate,
    start: &str,
    end: &str,
    note: &str,
) {
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        for (input, text) in [
            (&view.agenda.title_input, title.to_owned()),
            (&view.agenda.date_input, date.format("%Y-%m-%d").to_string()),
            (&view.agenda.start_input, start.to_owned()),
            (&view.agenda.end_input, end.to_owned()),
            (&view.agenda.note_input, note.to_owned()),
        ] {
            input.update(cx, |input, cx| input.set_value(text, window, cx));
        }
    });
    settle(manager);
}

fn seed_plan(manager: &mut ManagerHarness, title: &str) -> PlanEntry {
    let plan = manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let plan = view
            .store
            .as_mut()
            .unwrap()
            .create_plan(
                &PlanDraft {
                    task_id: None,
                    title: title.to_owned(),
                    tag_id: 1,
                    day: view.agenda.date,
                    start_minute: 9 * 60,
                    end_minute: 10 * 60,
                    note: "Saved note".to_owned(),
                },
                view.now - 100,
            )
            .unwrap();
        view.refresh(cx);
        plan
    });
    settle(manager);
    plan
}

fn select_first_plan(manager: &mut ManagerHarness) {
    // Use real wheel input to reach an event rather than dispatching to a
    // clipped calendar block. Native initial scroll positioning is separate.
    let event = manager.bounds("agenda-plan-1");
    let grid = manager.bounds("agenda-day-grid");
    if event.top() < grid.top() || event.bottom() > grid.bottom() {
        manager.scroll(
            "agenda-day-grid",
            f32::from(grid.center().y - event.center().y),
        );
    }
    let event = manager.bounds("agenda-plan-1");
    let grid = manager.bounds("agenda-day-grid");
    assert_inside(event, grid);
    manager.click("agenda-plan-1");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.agenda.selected_plan, Some(1));
        assert!(!view.agenda.editing);
    });
}

fn second_click(manager: &mut ManagerHarness, position: Point<Pixels>) {
    manager.cx.simulate_event(MouseDownEvent {
        position,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    manager.cx.simulate_event(MouseUpEvent {
        position,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
    });
    settle(manager);
}

#[gpui::test]
fn agenda_ui_calendar_and_editor_fit_default_and_minimum_windows(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = day_manager(cx, width, height, Page::Agenda);
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            let date = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
            view.agenda.date = date;
            view.agenda.month = date.with_day(1).unwrap();
            view.refresh_agenda(cx);
        });
        settle(&mut manager);
        let viewport = Bounds::new(Point::default(), size(px(width), px(height)));
        let content = manager.bounds("manager-content");
        let calendar = manager.bounds("agenda-content");
        let month = manager.bounds("agenda-month");
        for item in [
            content,
            calendar,
            month,
            manager.bounds("agenda-day-header"),
            manager.bounds("agenda-day-grid"),
        ] {
            assert_inside(item, viewport);
        }
        assert!(month.right() < calendar.left());
        assert_inside(manager.bounds("agenda-date-2026-10-25"), month);
        manager.click("agenda-date-2026-10-26");
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(
                view.agenda.date,
                NaiveDate::from_ymd_opt(2026, 10, 26).unwrap()
            )
        });
        manager.click("agenda-add");
        let editor = manager.bounds("agenda-editor");
        assert_inside(editor, calendar);
        for selector in [
            "agenda-title-input",
            "agenda-date-input",
            "agenda-start-input",
            "agenda-end-input",
            "agenda-note-input",
        ] {
            assert_horizontally_inside(manager.bounds(selector), editor);
        }
        assert_inside(manager.bounds("agenda-editor-actions"), editor);
        assert_inside(manager.bounds("agenda-save"), editor);
        assert_inside(manager.bounds("agenda-cancel"), editor);
        manager.click("agenda-cancel");
        manager
            .view
            .read_with(&manager.cx, |view, _| assert!(!view.agenda.editing));
    }
}

#[gpui::test]
fn agenda_ui_only_overlapping_events_share_the_day_column(cx: &mut TestAppContext) {
    let mut manager = day_manager(cx, 780., 620., Page::Agenda);
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let store = view.store.as_mut().unwrap();
        for (title, start, end) in [
            ("First overlapping arrangement", 540, 600),
            ("Second overlapping arrangement", 555, 615),
            ("Isolated afternoon arrangement", 840, 900),
        ] {
            store
                .create_plan(
                    &PlanDraft {
                        task_id: None,
                        title: title.into(),
                        tag_id: 1,
                        day: view.agenda.date,
                        start_minute: start,
                        end_minute: end,
                        note: String::new(),
                    },
                    view.now,
                )
                .unwrap();
        }
        view.refresh(cx);
    });
    settle(&mut manager);
    let first = manager.bounds("agenda-plan-1");
    let second = manager.bounds("agenda-plan-2");
    let isolated = manager.bounds("agenda-plan-3");
    assert!(first.bottom() > second.top());
    assert!(first.right() < second.left());
    assert!((first.size.width - second.size.width).abs() <= px(1.));
    // The later non-overlapping event spans both earlier lanes, instead of
    // retaining the day's maximum overlap width.
    assert!((isolated.left() - first.left()).abs() <= px(1.));
    assert!((isolated.right() - second.right()).abs() <= px(1.));
    assert!(isolated.size.width > first.size.width * 1.9);
}

#[gpui::test]
fn agenda_ui_clicking_an_empty_hour_prefills_the_slot_and_saves_without_starting(
    cx: &mut TestAppContext,
) {
    let mut manager = day_manager(cx, 780., 620., Page::Agenda);
    let tomorrow = Local::now().date_naive().succ_opt().unwrap();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_agenda_date(tomorrow, window, cx)
    });
    settle(&mut manager);
    let slot = manager.bounds("agenda-hour-13");
    let grid = manager.bounds("agenda-day-grid");
    manager.scroll(
        "agenda-day-grid",
        f32::from(grid.center().y - slot.center().y),
    );
    assert_inside(
        manager.bounds("agenda-hour-13"),
        manager.bounds("agenda-day-grid"),
    );
    manager.click("agenda-hour-13");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editing);
        assert_eq!(view.agenda.date, tomorrow);
        assert_eq!(view.agenda.start_input.read(cx).value(), "13:00");
        assert_eq!(view.agenda.end_input.read(cx).value(), "14:00");
        assert!(view.agenda.title_input.read(cx).value().is_empty());
        assert!(view.active.is_empty());
    });
    // The click must hand typing to the new title input, without a separate
    // programmatic focus or replacing the prefilled times.
    manager.cx.simulate_input("Review chapter three");
    settle(&mut manager);
    manager.click("agenda-save");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.agenda.editing);
        assert_eq!(view.agenda.plans.len(), 1);
        let plan = &view.agenda.plans[0];
        assert_eq!(plan.title, "Review chapter three");
        assert_eq!(plan.day, tomorrow);
        assert_eq!(plan.start_minute, 13 * 60);
        assert_eq!(plan.end_minute, 14 * 60);
        assert!(view.active.is_empty());
        assert!(view.store.as_ref().unwrap().sessions().unwrap().is_empty());
    });
}

#[gpui::test]
fn agenda_ui_saving_a_dated_plan_creates_no_timer_and_return_updates_same_plan(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = day_manager(cx, width, height, Page::Agenda);
        let tomorrow = Local::now()
            .date_naive()
            .checked_add_days(Days::new(1))
            .unwrap();
        manager.click("agenda-add");
        fill_plan(
            &mut manager,
            "Write the outline",
            tomorrow,
            "09:15",
            "10:00",
            "Keep the scope small.",
        );
        manager.click("agenda-save");
        let saved = manager.view.read_with(&manager.cx, |view, _| {
            assert!(!view.agenda.editing);
            assert_eq!(view.agenda.date, tomorrow);
            assert_eq!(view.agenda.plans.len(), 1);
            assert!(view.active.is_empty());
            assert!(
                view.store
                    .as_ref()
                    .unwrap()
                    .active_sessions()
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(view.tasks.len(), 1);
            let plan = view.agenda.plans[0].clone();
            assert_eq!(plan.start_minute, 9 * 60 + 15);
            assert_eq!(plan.end_minute, 10 * 60);
            assert_eq!(plan.note, "Keep the scope small.");
            assert_eq!(view.tasks[0].id, plan.task_id);
            plan
        });
        manager.click("agenda-plan-edit");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.agenda.note_input.update(cx, |input, cx| {
                input.set_value("Saved with Return", window, cx)
            });
            view.agenda.start_input.update(cx, |_, cx| {
                cx.emit(InputEvent::PressEnter { secondary: false })
            });
        });
        settle(&mut manager);
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(!view.agenda.editing);
            assert_eq!(view.agenda.plans.len(), 1);
            assert_eq!(view.agenda.plans[0].id, saved.id);
            assert_eq!(view.agenda.plans[0].note, "Saved with Return");
            assert!(view.active.is_empty());
            assert_eq!(view.tasks.len(), 1);
        });
    }
}

#[gpui::test]
fn agenda_ui_invalid_time_preserves_the_entire_draft_and_writes_nothing(cx: &mut TestAppContext) {
    let mut manager = day_manager(cx, 780., 620., Page::Agenda);
    let date = Local::now().date_naive();
    manager.click("agenda-add");
    for (start, end) in [
        ("10:00", "09:00"),
        ("09:00", "09:00"),
        ("wrong", "10:00"),
        ("23:00", "24:01"),
    ] {
        fill_plan(
            &mut manager,
            "Keep my draft",
            date,
            start,
            end,
            "Do not discard this note.",
        );
        manager.click("agenda-save");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.agenda.editing);
            assert!(view.agenda.error.is_some());
            assert_eq!(view.agenda.title_input.read(cx).value(), "Keep my draft");
            assert_eq!(view.agenda.start_input.read(cx).value(), start);
            assert_eq!(view.agenda.end_input.read(cx).value(), end);
            assert_eq!(
                view.agenda.note_input.read(cx).value(),
                "Do not discard this note."
            );
            assert!(
                view.store
                    .as_ref()
                    .unwrap()
                    .plans_for_day(date)
                    .unwrap()
                    .is_empty()
            );
            assert!(view.tasks.is_empty());
            assert!(view.active.is_empty());
        });
        assert_inside(
            manager.bounds("agenda-save"),
            manager.bounds("agenda-editor"),
        );
        manager.bounds("agenda-error");
    }
    fill_plan(
        &mut manager,
        "Keep my draft",
        date,
        "23:00",
        "24:00",
        "Do not discard this note.",
    );
    manager.click("agenda-save");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.agenda.editing);
        assert_eq!(view.agenda.plans[0].end_minute, 1440);
    });
}

#[gpui::test]
fn agenda_ui_selection_and_date_changes_protect_edits_with_save_discard_cancel(
    cx: &mut TestAppContext,
) {
    let mut manager = day_manager(cx, 1040., 780., Page::Agenda);
    let first = seed_plan(&mut manager, "First arrangement");
    seed_plan(&mut manager, "Other arrangement");
    select_first_plan(&mut manager);
    manager.click("agenda-plan-edit");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.agenda.note_input.update(cx, |input, cx| {
            input.set_value("Keep until confirmed", window, cx)
        });
        view.select_plan(2, window, cx);
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editing);
        assert_eq!(view.agenda.edit_id, Some(first.id));
        assert_eq!(
            view.agenda.note_input.read(cx).value(),
            "Keep until confirmed"
        );
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(
            view.agenda
                .start_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        view.select_plan(2, window, cx)
    });
    manager.cx.simulate_prompt_answer("保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.agenda.editing);
        assert_eq!(view.agenda.selected_plan, Some(2));
        assert_eq!(
            view.agenda
                .plans
                .iter()
                .find(|plan| plan.id == first.id)
                .unwrap()
                .note,
            "Keep until confirmed"
        );
    });
    manager.click("agenda-plan-edit");
    let tomorrow = Local::now().date_naive().succ_opt().unwrap();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.agenda
            .end_input
            .update(cx, |input, cx| input.set_value("invalid", window, cx));
        view.select_agenda_date(tomorrow, window, cx);
    });
    manager.cx.simulate_prompt_answer("保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editing);
        assert_ne!(view.agenda.date, tomorrow);
        assert_eq!(view.agenda.end_input.read(cx).value(), "invalid");
        assert!(view.agenda.error.is_some());
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_agenda_date(tomorrow, window, cx)
    });
    manager.cx.simulate_prompt_answer("不保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.agenda.editing);
        assert_eq!(view.agenda.date, tomorrow);
        let old = view
            .store
            .as_ref()
            .unwrap()
            .plans_for_day(first.day)
            .unwrap();
        assert_eq!(
            old.iter().find(|plan| plan.id == 2).unwrap().end_minute,
            600
        );
    });
}

#[gpui::test]
fn agenda_ui_starting_a_plan_saves_a_focus_draft_and_resumes_the_original_round(
    cx: &mut TestAppContext,
) {
    let mut manager = day_manager(cx, 1040., 780., Page::Agenda);
    let (original_id, other_id, task_id) =
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let now = Local::now().timestamp();
            let store = view.store.as_mut().unwrap();
            let task = store.create_task("Scheduled task", 1, now - 1000).unwrap();
            let original = store.start_task(task.id, now - 900, Some(37 * 60)).unwrap();
            store.pause(original.id, now - 780).unwrap();
            let other = store
                .create_task("Other running task", 1, now - 500)
                .unwrap();
            let other = store.start_task(other.id, now - 300, None).unwrap();
            store
                .create_plan(
                    &PlanDraft {
                        task_id: Some(task.id),
                        title: task.title.clone(),
                        tag_id: 1,
                        day: view.agenda.date,
                        start_minute: 540,
                        end_minute: 600,
                        note: String::new(),
                    },
                    now - 100,
                )
                .unwrap();
            view.refresh(cx);
            view.new_task(window, cx);
            view.task_input.update(cx, |input, cx| {
                input.set_value("A separate unfinished draft", window, cx)
            });
            view.navigate(Page::Agenda, window, cx);
            view.select_plan(1, window, cx);
            (original.id, other.id, task.id)
        });
    settle(&mut manager);
    manager.click("agenda-plan-start");
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(
            view.task_input.read(cx).value(),
            "A separate unfinished draft"
        );
        assert!(
            view.active
                .iter()
                .find(|session| session.id == other_id)
                .unwrap()
                .is_running()
        );
        assert!(
            view.active
                .iter()
                .find(|session| session.id == original_id)
                .unwrap()
                .is_paused()
        );
    });
    manager.click("agenda-plan-start");
    manager.cx.simulate_prompt_answer("保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        let original = view
            .active
            .iter()
            .find(|session| session.id == original_id)
            .unwrap();
        assert!(original.is_running());
        assert_eq!(original.task_id, Some(task_id));
        assert_eq!(original.target_seconds, Some(37 * 60));
        assert_eq!(original.intervals.len(), 2);
        assert!(
            view.active
                .iter()
                .find(|session| session.id == other_id)
                .unwrap()
                .is_paused()
        );
        assert!(
            view.tasks
                .iter()
                .any(|task| task.title == "A separate unfinished draft")
        );
        assert_eq!(view.selected_session, Some(original_id));
        assert!(matches!(view.page, Page::Agenda));
    });
}

#[gpui::test]
fn agenda_ui_checking_and_postponing_arrangements_never_rewrites_actual_time(
    cx: &mut TestAppContext,
) {
    let mut manager = day_manager(cx, 780., 620., Page::Agenda);
    let plan = seed_plan(&mut manager, "A timed arrangement");
    let original = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let original = view
            .store
            .as_mut()
            .unwrap()
            .start_task(plan.task_id, view.now - 30, None)
            .unwrap();
        view.refresh(cx);
        view.select_plan(plan.id, window, cx);
        original
    });
    settle(&mut manager);
    manager.click("agenda-plan-done");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.agenda_plan_is_done(&view.agenda.plans[0]));
        assert_eq!(view.active, vec![original.clone()]);
        assert_eq!(
            view.tasks
                .iter()
                .find(|task| task.id == plan.task_id)
                .unwrap()
                .completed_at,
            None
        );
    });
    manager.click("agenda-plan-done");
    let before = manager.view.read_with(&manager.cx, |view, _| {
        view.store.as_ref().unwrap().sessions().unwrap()
    });
    manager.click("agenda-plan-tomorrow");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.agenda.plans[0].moved_to.is_some());
        let next = view
            .store
            .as_ref()
            .unwrap()
            .plans_for_day(plan.day.succ_opt().unwrap())
            .unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].task_id, plan.task_id);
        assert_eq!(next[0].completed_at, None);
        assert_eq!(view.store.as_ref().unwrap().sessions().unwrap(), before);
        assert!(view.active[0].is_running());
    });
}

#[gpui::test]
fn agenda_ui_actual_work_and_untimed_completions_appear_automatically_without_duplicates(
    cx: &mut TestAppContext,
) {
    let mut manager = day_manager(cx, 1040., 780., Page::Agenda);
    let yesterday = Local::now().date_naive().pred_opt().unwrap();
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        view.agenda.date = yesterday;
        view.agenda.month = yesterday.with_day(1).unwrap();
        // Actual work happens six hours after its planned start. The
        // calendar must preserve both the intended slot and actual clock time.
        let base = day_bounds(yesterday).0 + 15 * 3600;
        let store = view.store.as_mut().unwrap();
        let plan = store
            .create_plan(
                &PlanDraft {
                    task_id: None,
                    title: "Planned work".into(),
                    tag_id: 1,
                    day: yesterday,
                    start_minute: 540,
                    end_minute: 600,
                    note: String::new(),
                },
                base - 100,
            )
            .unwrap();
        store
            .create_plan(
                &PlanDraft {
                    task_id: Some(plan.task_id),
                    title: "Another occurrence".into(),
                    tag_id: 1,
                    day: yesterday,
                    start_minute: 660,
                    end_minute: 720,
                    note: String::new(),
                },
                base - 100,
            )
            .unwrap();
        let planned = store.start_task(plan.task_id, base, None).unwrap();
        store.finish(planned.id, base + 120).unwrap();
        store.complete_task(plan.task_id, base + 130).unwrap();
        let free = store
            .create_and_start_task("Unplanned actual work", 1, base + 200, None)
            .unwrap();
        store.finish(free.id, base + 260).unwrap();
        let untimed = store
            .create_task("Done without a timer", 1, base - 100)
            .unwrap();
        store.complete_task(untimed.id, base + 300).unwrap();
        view.refresh(cx);
    });
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.agenda.records.len(), 2);
        assert_eq!(view.agenda.completed_tasks.len(), 2);
        assert_eq!(view.agenda_plan_actuals(1).len(), 1);
        assert!(view.agenda_plan_actuals(2).is_empty());
        assert!(
            view.agenda
                .plans
                .iter()
                .all(|plan| view.agenda_plan_is_done(plan))
        );
        let completions = view.agenda_unplanned_completions();
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].title, "Done without a timer");
        assert_eq!(view.agenda.stats.focus_seconds, 180);
    });
    // These are present in a fresh view tree; no removed-selector cache is used.
    let planned = manager.bounds("agenda-plan-1");
    let actual = manager.bounds("agenda-record-1");
    assert!((actual.top() - planned.top() - px(6. * 60.)).abs() <= px(1.));
    manager.bounds("agenda-plan-2");
    manager.bounds("agenda-record-2");
    manager.bounds("agenda-completion-3");
}

#[gpui::test]
fn agenda_ui_global_task_completion_cannot_offer_to_undo_an_occurrence(cx: &mut TestAppContext) {
    let mut manager = day_manager(cx, 1040., 780., Page::Agenda);
    let plan = seed_plan(&mut manager, "Already completed task");
    let completed_at = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let now = Local::now().timestamp();
        let store = view.store.as_mut().unwrap();
        store.set_plan_completed(plan.id, true, now - 50).unwrap();
        store.complete_task(plan.task_id, now - 40).unwrap();
        view.refresh(cx);
        view.select_plan(plan.id, window, cx);
        view.toast = None;
        view.agenda.plans[0].completed_at
    });
    settle(&mut manager);
    manager.click("agenda-plan-done");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.agenda.plans[0].completed_at, completed_at);
        assert!(
            view.agenda
                .completed_tasks
                .iter()
                .any(|task| task.id == plan.task_id)
        );
        assert!(view.toast.is_none());
        assert!(view.active.is_empty());
    });
}

#[gpui::test]
fn agenda_ui_plan_tag_selection_is_independent_from_the_focus_draft(cx: &mut TestAppContext) {
    let mut manager = day_manager(cx, 1040., 780., Page::Agenda);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.new_task(window, cx);
        view.selected_tag = 1;
        view.task_input.update(cx, |input, cx| {
            input.set_value("Keep the focus draft", window, cx)
        });
        view.navigate(Page::Agenda, window, cx);
    });
    settle(&mut manager);
    manager.click("agenda-add");
    let date = Local::now().date_naive();
    fill_plan(
        &mut manager,
        "A separate calendar plan",
        date,
        "09:00",
        "09:30",
        "",
    );
    manager.view.update(&mut manager.cx, |view, cx| {
        view.agenda
            .tag_picker
            .update(cx, |_, cx| cx.emit(TagPicked(2)));
    });
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.agenda.draft_tag, 2);
        assert_eq!(view.selected_tag, 1);
        assert_eq!(view.task_input.read(cx).value(), "Keep the focus draft");
        assert!(view.agenda.tag_picker.read(cx).is_enabled());
    });
    manager.click("agenda-save");
    manager.view.read_with(&manager.cx, |view, cx| {
        let task_id = view.agenda.plans[0].task_id;
        assert_eq!(
            view.tasks
                .iter()
                .find(|task| task.id == task_id)
                .unwrap()
                .tag_id,
            2
        );
        assert_eq!(view.task_input.read(cx).value(), "Keep the focus draft");
        assert_eq!(view.selected_tag, 1);
        assert!(view.active.is_empty());
    });
}

#[gpui::test]
fn agenda_ui_hiding_reopening_and_navigation_keep_an_unsaved_calendar_draft(
    cx: &mut TestAppContext,
) {
    let mut manager = day_manager(cx, 780., 620., Page::Agenda);
    let date = Local::now().date_naive();
    manager.click("agenda-add");
    fill_plan(
        &mut manager,
        "Unfinished calendar draft",
        date,
        "11:00",
        "invalid",
        "A note worth keeping",
    );
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.hide_manager(window, cx);
        view.show_manager(window, cx);
        view.navigate(Page::Records, window, cx);
        view.navigate(Page::Agenda, window, cx);
    });
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editing);
        assert!(view.agenda_has_unsaved_changes(cx));
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Unfinished calendar draft"
        );
        assert_eq!(view.agenda.start_input.read(cx).value(), "11:00");
        assert_eq!(view.agenda.end_input.read(cx).value(), "invalid");
        assert_eq!(
            view.agenda.note_input.read(cx).value(),
            "A note worth keeping"
        );
        assert!(view.agenda.plans.is_empty());
    });
    assert_inside(
        manager.bounds("agenda-save"),
        manager.bounds("agenda-editor"),
    );
}

#[gpui::test]
fn agenda_ui_double_click_cannot_undo_completion_or_accidentally_start_a_round(
    cx: &mut TestAppContext,
) {
    let mut manager = day_manager(cx, 1040., 780., Page::Agenda);
    seed_plan(&mut manager, "One deliberate action");
    select_first_plan(&mut manager);
    let done = manager.bounds("agenda-plan-done").center();
    manager.click("agenda-plan-done");
    let completed = manager.view.read_with(&manager.cx, |view, _| {
        view.agenda.plans[0].completed_at.unwrap()
    });
    second_click(&mut manager, done);
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.agenda.plans[0].completed_at, Some(completed));
        assert!(view.active.is_empty());
    });
    manager.click("agenda-plan-done");
    let start = manager.bounds("agenda-plan-start").center();
    second_click(&mut manager, start);
    manager
        .view
        .read_with(&manager.cx, |view, _| assert!(view.active.is_empty()));
    manager.click("agenda-plan-start");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active.len(), 1);
        assert!(view.active[0].is_running());
    });
}
