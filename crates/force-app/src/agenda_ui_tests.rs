//! Calendar event/layout regressions on GPUI TestPlatform. Native pixels and
//! actual IME composition remain part of signed-app acceptance.

use chrono::{Datelike, Days, Local, NaiveDate};
use force_core::{PlanDraft, PlanEntry};
use gpui::{
    Bounds, Focusable as _, KeyBinding, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent,
    Pixels, Point, TestAppContext, px, size,
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
    use crate::agenda_model::{AgendaItemStatus, AgendaMonthItem};
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
                        tag_id: None,
                        tag_class: force_core::TagClass::Legacy,
                        tag_color: Some("#B56F67".into()),
                        status: if index == 0 {
                            AgendaItemStatus::Completed
                        } else {
                            AgendaItemStatus::Recorded
                        },
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
        assert!(title.size.width >= px(40.));
        let status = manager.bounds("agenda-month-status-2026-08-15-0");
        let indicator = manager.bounds("agenda-month-color-2026-08-15-0");
        let row = manager.bounds("agenda-month-item-2026-08-15-0");
        assert_inside(title, row);
        assert_inside(status, row);
        assert_inside(indicator, row);
        assert!(indicator.right() <= title.left());
        assert!(title.right() <= status.left());
        assert_eq!(row.size.height, px(20.));
        assert!(status.size.width >= px(24.));
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
            assert!(view.agenda.month_overview);
            assert!(view.agenda.popup_open);
            assert!(!view.agenda.editor_visible);
            assert_eq!(
                view.agenda.date,
                NaiveDate::from_ymd_opt(2026, 8, 15).unwrap()
            );
            assert!(view.active.is_empty());
        });
        assert_same_bounds(manager.bounds("agenda-overview-grid"), grid);
        assert_inside(manager.bounds("agenda-popover"), viewport);
        manager.click("agenda-popover-timeline");
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
fn agenda_ui_same_day_today_action_suspends_the_popup_without_pointer_dismissal(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let today = Local::now().date_naive();
    let target = today.succ_opt().unwrap();
    manager.click("agenda-overview-add");
    fill_plan(
        &mut manager,
        "Keep my draft when returning to today",
        target,
        "09:00",
        "10:00",
        "Retain the changed target date",
    );
    // Invoke the same Today action without a pointer event: mouse-down outside
    // the popup must not hide an early return in date selection.
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert_eq!(view.agenda.date, today);
        assert!(view.agenda.editor_visible);
        view.select_agenda_date(today, window, cx);
    });
    settle(&mut manager);
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.agenda.date, today);
        assert!(view.agenda.month_overview);
        assert!(!view.agenda.popup_open);
        assert!(!view.agenda.editor_visible);
        assert!(view.agenda_has_suspended_draft(cx));
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Keep my draft when returning to today"
        );
        assert_eq!(
            view.agenda.date_input.read(cx).value(),
            target.format("%Y-%m-%d").to_string()
        );
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .plans_for_day(target)
                .unwrap()
                .is_empty()
        );
        assert!(view.active.is_empty());
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(view.agenda.calendar_focus.is_focused(window));
        view.open_agenda_day(today, window, cx);
    });
    settle(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(view.agenda.popup_open);
        assert!(!view.agenda.editor_visible);
        view.select_agenda_date(today, window, cx);
    });
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.agenda.popup_open);
        assert!(view.agenda_has_suspended_draft(cx));
        assert_eq!(view.agenda.date, today);
    });
    manager.click("agenda-resume-draft");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editor_visible);
        assert_eq!(
            view.agenda.note_input.read(cx).value(),
            "Retain the changed target date"
        );
    });
}

#[gpui::test]
fn agenda_ui_hidden_draft_does_not_block_timeline_initial_time_requests(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        let origin = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        let next = origin.succ_opt().unwrap();
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.store
                .as_mut()
                .unwrap()
                .create_plan(
                    &PlanDraft {
                        task_id: None,
                        title: "Afternoon arrangement on the next day".to_owned(),
                        tag_id: 1,
                        day: next,
                        start_minute: 14 * 60,
                        end_minute: 15 * 60,
                        note: String::new(),
                    },
                    view.now,
                )
                .unwrap();
            view.open_agenda_day(origin, window, cx);
        });
        settle(&mut manager);
        manager.click("agenda-overview-add");
        fill_plan(
            &mut manager,
            "Keep the draft while browsing the timeline",
            origin,
            "10:00",
            "11:00",
            "Retain the timeline draft",
        );
        manager.click("agenda-popover-close");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.open_agenda_day(origin, window, cx);
        });
        settle(&mut manager);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.open_agenda_timeline(window, cx);
            assert_eq!(view.agenda.calendar_scroll_target.get(), Some(8 * 60));
        });
        settle(&mut manager);
        let viewport = Bounds::new(Point::default(), size(px(width), px(height)));
        assert_inside(manager.bounds("agenda-day-grid"), viewport);
        // TestPlatform does not dispatch platform frame callbacks, so it cannot
        // execute Window::on_next_frame or establish the resulting native scroll
        // offset. Verify the requested minute and its handoff to that callback.
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.agenda.calendar_scroll_target.get().is_none());
            assert!(view.agenda_has_suspended_draft(cx));
            assert_eq!(view.agenda.date, origin);
            assert!(!view.agenda.month_overview);
            assert!(!view.agenda.editor_visible);
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.select_agenda_date(next, window, cx);
            assert_eq!(view.agenda.calendar_scroll_target.get(), Some(13 * 60));
        });
        settle(&mut manager);
        assert_inside(manager.bounds("agenda-day-grid"), viewport);
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.agenda.calendar_scroll_target.get().is_none());
            assert!(view.agenda_has_suspended_draft(cx));
            assert_eq!(view.agenda.date, next);
            assert_eq!(
                view.agenda.title_input.read(cx).value(),
                "Keep the draft while browsing the timeline"
            );
            assert_eq!(
                view.agenda.note_input.read(cx).value(),
                "Retain the timeline draft"
            );
            assert!(
                view.store
                    .as_ref()
                    .unwrap()
                    .plans_for_day(origin)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                view.store
                    .as_ref()
                    .unwrap()
                    .plans_for_day(next)
                    .unwrap()
                    .len(),
                1
            );
            assert!(view.active.is_empty());
        });
    }
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

#[gpui::test]
fn agenda_ui_current_calendar_tab_and_pristine_popup_return_to_month(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = day_manager(cx, width, height, Page::Agenda);
        manager.click("agenda-tab");
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.agenda.month_overview);
            assert!(!view.compact);
        });
        let grid = manager.bounds("agenda-overview-grid");
        manager.click("agenda-overview-add");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.agenda.editing);
            assert!(view.agenda.editor_visible);
            assert!(view.agenda.popup_open);
            assert!(view.agenda.month_overview);
            assert!(!view.agenda_has_unsaved_changes(cx));
        });
        assert_same_bounds(manager.bounds("agenda-overview-grid"), grid);
        manager.click("agenda-popover-close");
        assert!(!manager.cx.has_pending_prompt());
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.agenda.month_overview);
            assert!(!view.agenda.editor_visible);
            assert!(!view.agenda.popup_open);
            assert!(!view.agenda_has_suspended_draft(cx));
            assert!(!view.compact);
            assert!(view.active.is_empty());
        });
        assert_same_bounds(manager.bounds("agenda-overview-grid"), grid);
    }
}

#[gpui::test]
fn agenda_ui_popup_close_retains_changed_date_and_cancel_discards_the_same_draft(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let origin = Local::now().date_naive();
    let target = origin.succ_opt().unwrap();
    manager.click("agenda-overview-add");
    fill_plan(
        &mut manager,
        "Keep a changed-date draft",
        target,
        "10:00",
        "11:00",
        "Keep this note",
    );
    let draft_tag = manager
        .view
        .read_with(&manager.cx, |view, _| view.agenda.draft_tag);
    manager.click("agenda-popover-close");
    manager.view.update(&mut manager.cx, |view, cx| {
        // A picker/input callback queued before dismissal cannot mutate or
        // commit the retained draft after its controls have disappeared.
        view.agenda.tag_picker.update(cx, |_, cx| {
            cx.emit(TagPicked(if draft_tag == 2 { 1 } else { 2 }));
        });
        view.agenda.title_input.update(cx, |_, cx| {
            cx.emit(InputEvent::PressEnter { secondary: false });
        });
    });
    settle(&mut manager);
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editing);
        assert_eq!(view.agenda.draft_tag, draft_tag);
        assert!(!view.agenda.editor_visible);
        assert!(!view.agenda.popup_open);
        assert!(view.agenda_has_suspended_draft(cx));
        assert!(view.agenda.month_overview);
        assert!(!view.compact);
        assert_eq!(view.agenda.date, origin);
        assert_eq!(
            view.agenda.date_input.read(cx).value(),
            target.format("%Y-%m-%d").to_string()
        );
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .plans_for_day(target)
                .unwrap()
                .is_empty()
        );
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(view.agenda.calendar_focus.is_focused(window));
        assert!(
            !view
                .agenda
                .title_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    });
    manager.click("agenda-resume-draft");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editor_visible);
        assert!(view.agenda.popup_open);
        assert_eq!(view.agenda.date, origin);
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Keep a changed-date draft"
        );
        assert_eq!(
            view.agenda.date_input.read(cx).value(),
            target.format("%Y-%m-%d").to_string()
        );
        assert_eq!(view.agenda.note_input.read(cx).value(), "Keep this note");
    });
    manager.click("agenda-cancel");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.agenda.editing);
        assert!(!view.agenda.editor_visible);
        assert!(!view.agenda.popup_open);
        assert!(!view.agenda_has_suspended_draft(cx));
        assert!(view.agenda.month_overview);
        assert_eq!(view.agenda.date, origin);
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .plans_for_day(target)
                .unwrap()
                .is_empty()
        );
        assert!(view.active.is_empty());
    });
    manager.click("agenda-overview-add");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.title_input.read(cx).value().is_empty());
        assert!(view.agenda.note_input.read(cx).value().is_empty());
        assert!(!view.agenda_has_unsaved_changes(cx));
    });
}

#[gpui::test]
fn agenda_ui_invalid_popup_save_preserves_context_and_allows_in_place_correction(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let date = Local::now().date_naive();
    let grid = manager.bounds("agenda-overview-grid");
    manager.click("agenda-overview-add");
    fill_plan(
        &mut manager,
        "Fix my end time",
        date,
        "10:00",
        "invalid",
        "Keep this note",
    );
    manager.click("agenda-save");
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(matches!(view.page, Page::Agenda));
        assert!(view.agenda.editing);
        assert!(view.agenda.editor_visible);
        assert!(view.agenda.popup_open);
        assert!(view.agenda.month_overview);
        assert!(!view.compact);
        assert!(view.agenda.error.is_some());
        assert_eq!(view.agenda.end_input.read(cx).value(), "invalid");
        assert_eq!(view.agenda.note_input.read(cx).value(), "Keep this note");
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .plans_for_day(date)
                .unwrap()
                .is_empty()
        );
        assert!(view.active.is_empty());
    });
    assert_same_bounds(manager.bounds("agenda-overview-grid"), grid);
    assert_inside(
        manager.bounds("agenda-error"),
        manager.bounds("agenda-popover"),
    );
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.agenda
            .end_input
            .update(cx, |input, cx| input.set_value("11:00", window, cx));
    });
    settle(&mut manager);
    manager.click("agenda-save");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.agenda.editing);
        assert!(!view.agenda.editor_visible);
        assert!(!view.agenda.popup_open);
        assert!(view.agenda.month_overview);
        let plans = view.store.as_ref().unwrap().plans_for_day(date).unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].title, "Fix my end time");
        assert_eq!(plans[0].note, "Keep this note");
        assert!(view.active.is_empty());
    });
    assert_same_bounds(manager.bounds("agenda-overview-grid"), grid);
    manager.view.update_in(&mut manager.cx, |view, window, _| {
        assert!(view.agenda.calendar_focus.is_focused(window));
    });
}

#[gpui::test]
fn agenda_ui_records_and_settings_suspend_the_calendar_draft_without_prompting(
    cx: &mut TestAppContext,
) {
    for (source, selector) in [
        (Page::Records, "records-tab"),
        (Page::Settings, "settings-tab"),
    ] {
        let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
        let date = Local::now().date_naive();
        manager.click("agenda-overview-add");
        fill_plan(
            &mut manager,
            "Retained while browsing",
            date,
            "10:00",
            "11:00",
            "Keep until resumed",
        );
        manager.click(selector);
        assert!(!manager.cx.has_pending_prompt());
        assert!(manager.page() == source);
        manager.click("agenda-tab");
        assert!(!manager.cx.has_pending_prompt());
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(matches!(view.page, Page::Agenda));
            assert!(view.agenda.month_overview);
            assert!(!view.agenda.popup_open);
            assert!(!view.agenda.editor_visible);
            assert!(view.agenda_has_suspended_draft(cx));
            assert_eq!(
                view.agenda.title_input.read(cx).value(),
                "Retained while browsing"
            );
            assert_eq!(
                view.agenda.note_input.read(cx).value(),
                "Keep until resumed"
            );
            assert!(!view.compact);
        });
        manager.click("agenda-resume-draft");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.agenda.editor_visible);
            assert!(view.agenda.popup_open);
            assert_eq!(
                view.agenda.title_input.read(cx).value(),
                "Retained while browsing"
            );
        });
        manager.click("agenda-cancel");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(!view.agenda_has_suspended_draft(cx));
            assert!(
                view.store
                    .as_ref()
                    .unwrap()
                    .plans_for_day(date)
                    .unwrap()
                    .is_empty()
            );
            assert!(view.active.is_empty());
        });
    }
}

#[gpui::test]
fn agenda_ui_close_panel_suspends_editor_then_hides_the_month_root(cx: &mut TestAppContext) {
    let mut manager = day_manager(cx, 780., 620., Page::Agenda);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.close_panel(window, cx)
    });
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.agenda.month_overview);
        assert!(!view.compact);
    });
    manager.click("agenda-overview-add");
    let date = Local::now().date_naive();
    fill_plan(
        &mut manager,
        "Escape retains my draft",
        date,
        "10:00",
        "11:00",
        "",
    );
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.close_panel(window, cx)
    });
    settle(&mut manager);
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.month_overview);
        assert!(!view.agenda.popup_open);
        assert!(!view.agenda.editor_visible);
        assert!(view.agenda_has_suspended_draft(cx));
        assert!(!view.compact);
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.close_panel(window, cx)
    });
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.compact);
        assert!(view.agenda_has_suspended_draft(cx));
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.show_manager(window, cx)
    });
    settle(&mut manager);
    manager.click("agenda-resume-draft");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editor_visible);
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Escape retains my draft"
        );
        assert!(view.active.is_empty());
    });
}

#[gpui::test]
fn agenda_ui_month_returns_to_the_running_timer_without_changing_any_round(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let (running_id, task_id, before) =
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let now = Local::now().timestamp();
            let store = view.store.as_mut().unwrap();
            let paused = store
                .create_and_start_task("Paused task", 1, now - 200, None)
                .unwrap();
            store.pause(paused.id, now - 160).unwrap();
            let running = store
                .create_and_start_task("Current running task", 1, now - 100, None)
                .unwrap();
            view.refresh(cx);
            view.select_session(paused.id, window, cx);
            view.navigate(Page::Agenda, window, cx);
            (
                running.id,
                running.task_id,
                view.store.as_ref().unwrap().sessions().unwrap(),
            )
        });
    settle(&mut manager);
    let viewport = Bounds::new(Point::default(), size(px(780.), px(620.)));
    assert_inside(manager.bounds("agenda-overview-timer"), viewport);
    manager.click("agenda-overview-timer");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(matches!(view.page, Page::Focus));
        assert_eq!(view.selected_session, Some(running_id));
        assert_eq!(view.selected_task, task_id);
        assert!(!view.compact);
        assert_eq!(view.store.as_ref().unwrap().sessions().unwrap(), before);
        assert_eq!(view.active.len(), 2);
        assert!(
            view.active
                .iter()
                .find(|session| session.id == running_id)
                .unwrap()
                .is_running()
        );
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.close_panel(window, cx)
    });
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(matches!(view.page, Page::Agenda));
        assert!(view.agenda.month_overview);
        assert!(!view.compact);
        assert_eq!(view.store.as_ref().unwrap().sessions().unwrap(), before);
    });
}

#[gpui::test]
fn agenda_ui_popup_editor_stays_bounded_over_a_six_week_month(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        let day = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            view.agenda.date = day;
            view.agenda.month = day.with_day(1).unwrap();
            for index in 0..5 {
                view.store
                    .as_mut()
                    .unwrap()
                    .create_plan(
                        &PlanDraft {
                            task_id: None,
                            title: format!("复习操作系统、完成章节练习并整理长期项目笔记 {index}"),
                            tag_id: 1,
                            day,
                            start_minute: 540 + index * 30,
                            end_minute: 570 + index * 30,
                            note: String::new(),
                        },
                        view.now,
                    )
                    .unwrap();
            }
            view.refresh_agenda(cx);
        });
        settle(&mut manager);
        let viewport = Bounds::new(Point::default(), size(px(width), px(height)));
        let before = manager.bounds("agenda-overview-grid");
        manager.click("agenda-month-day-2026-08-15");
        manager.click("agenda-summary-plan-1");
        let detail = manager.bounds("agenda-plan-detail");
        let title = manager.bounds("agenda-plan-title");
        assert_inside(title, detail);
        assert!(title.size.width >= px(240.));
        assert!(title.size.height >= px(17.));
        manager.click("agenda-plan-edit");
        let popup = manager.bounds("agenda-popover");
        assert_inside(popup, viewport);
        assert_same_bounds(manager.bounds("agenda-overview-grid"), before);
        assert_inside(manager.bounds("agenda-editor"), popup);
        assert_inside(manager.bounds("agenda-title-input"), popup);
        assert!(manager.bounds("agenda-title-input").size.width >= px(280.));
        assert_same_bounds(
            Bounds::new(
                manager.bounds("agenda-date-input").origin,
                manager.bounds("agenda-title-input").size,
            ),
            manager.bounds("agenda-date-input"),
        );
        assert_inside(manager.bounds("agenda-save"), popup);
        assert_inside(manager.bounds("agenda-cancel"), popup);
        let collapsed_actions = manager.bounds("agenda-editor-actions");
        let note_toggle = manager.bounds("agenda-note-toggle");
        let validation = manager.bounds("agenda-validation-slot");
        assert!(collapsed_actions.top() - note_toggle.bottom() - validation.size.height <= px(36.));
        manager.click("agenda-note-toggle");
        assert_inside(
            manager.bounds("agenda-save"),
            manager.bounds("agenda-popover"),
        );
        assert_inside(
            manager.bounds("agenda-cancel"),
            manager.bounds("agenda-popover"),
        );
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.agenda.note_expanded);
            assert!(view.agenda.month_overview);
            assert!(view.agenda.popup_open);
            assert!(view.agenda.editor_visible);
        });
        manager.click("agenda-cancel");
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(!view.agenda.popup_open);
            assert!(!view.agenda.editor_visible);
            assert!(view.agenda.month_overview);
            assert_eq!(view.agenda.date, day);
            assert_eq!(
                view.store
                    .as_ref()
                    .unwrap()
                    .plans_for_day(day)
                    .unwrap()
                    .len(),
                5
            );
            assert!(view.active.is_empty());
        });
        assert_same_bounds(manager.bounds("agenda-overview-grid"), before);
    }
}

#[gpui::test]
fn agenda_ui_outside_click_suspends_and_same_day_summary_keeps_the_draft(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let date = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        view.agenda.date = date;
        view.agenda.month = date.with_day(1).unwrap();
        view.refresh_agenda(cx);
    });
    settle(&mut manager);
    manager.click("agenda-overview-add");
    fill_plan(
        &mut manager,
        "Keep outside-click draft",
        date,
        "09:00",
        "09:30",
        "Retained note",
    );
    manager
        .cx
        .simulate_click(Point::new(px(20.), px(600.)), Modifiers::default());
    settle(&mut manager);
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.agenda.popup_open);
        assert!(!view.agenda.editor_visible);
        assert!(view.agenda_has_suspended_draft(cx));
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Keep outside-click draft"
        );
    });
    manager.click("agenda-resume-draft");
    manager.click("agenda-month-day-2026-08-31");
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.popup_open);
        assert!(!view.agenda.editor_visible);
        assert!(view.agenda_has_suspended_draft(cx));
        assert!(view.agenda.month_overview);
        assert_eq!(
            view.agenda.date,
            NaiveDate::from_ymd_opt(2026, 8, 31).unwrap()
        );
    });
    manager.click("agenda-popover-close");
    manager.click("agenda-resume-draft");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.agenda.note_input.read(cx).value(), "Retained note");
        assert_eq!(view.agenda.date, date);
        assert!(view.agenda.editor_visible);
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .plans_for_day(date)
                .unwrap()
                .is_empty()
        );
    });
}

#[gpui::test]
fn agenda_ui_tag_menu_selects_without_dismissing_the_editor(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let handle = manager.handle;
    manager.cx.update(|_, cx| {
        cx.bind_keys([KeyBinding::new("escape", crate::ClosePanel, Some("Force"))]);
        cx.on_action(move |_: &crate::ClosePanel, cx| {
            crate::with_force(handle, cx, |view, window, cx| view.close_panel(window, cx))
        });
    });
    let date = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        view.agenda.date = date;
        view.agenda.month = date.with_day(1).unwrap();
        view.refresh_agenda(cx);
    });
    settle(&mut manager);
    manager.click("agenda-overview-add");
    fill_plan(
        &mut manager,
        "Keep the draft while choosing a tag",
        date,
        "09:00",
        "09:45",
        "Keep the note too",
    );
    manager.click("focus-tag-picker");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.tag_picker.read(cx).is_open());
        assert!(view.agenda.editor_visible);
        assert!(view.agenda.popup_open);
    });
    let option = manager.bounds("focus-tag-option-2");
    let option_point = Point::new(option.right() - px(4.), option.center().y);
    assert_inside(
        option,
        Bounds::new(Point::default(), size(px(780.), px(620.))),
    );
    manager
        .cx
        .simulate_click(option_point, Modifiers::default());
    settle(&mut manager);
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.agenda.draft_tag, 2);
        assert!(view.agenda.editor_visible);
        assert!(view.agenda.popup_open);
        assert!(view.agenda.month_overview);
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Keep the draft while choosing a tag"
        );
        assert_eq!(view.agenda.start_input.read(cx).value(), "09:00");
        assert_eq!(view.agenda.end_input.read(cx).value(), "09:45");
        assert_eq!(view.agenda.note_input.read(cx).value(), "Keep the note too");
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .plans_for_day(date)
                .unwrap()
                .is_empty()
        );
        assert!(view.active.is_empty());
    });
    manager.click("focus-tag-picker");
    manager.cx.simulate_keystrokes("escape");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.agenda.tag_picker.read(cx).is_open());
        assert!(view.agenda.editor_visible);
        assert!(view.agenda.popup_open);
        assert_eq!(view.agenda.draft_tag, 2);
    });
    manager.cx.simulate_keystrokes("escape");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.agenda.editor_visible);
        assert!(!view.agenda.popup_open);
        assert!(view.agenda_has_suspended_draft(cx));
        assert_eq!(view.agenda.draft_tag, 2);
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Keep the draft while choosing a tag"
        );
        assert!(view.active.is_empty());
    });
}

#[gpui::test]
fn agenda_ui_actual_receipt_and_untimed_completion_are_read_only_month_context(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
    let date = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
    let before = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let base = day_bounds(date).0 + 9 * 3600;
        let store = view.store.as_mut().unwrap();
        let session = store
            .create_and_start_task("Actual work with a long complete title", 1, base, None)
            .unwrap();
        store.finish(session.id, base + 45 * 60).unwrap();
        let task = store
            .create_task("Completed without a timer", 1, base)
            .unwrap();
        store.complete_task(task.id, base + 60 * 60).unwrap();
        view.refresh(cx);
        view.open_agenda_day(date, window, cx);
        view.store.as_ref().unwrap().sessions().unwrap()
    });
    settle(&mut manager);
    let grid = manager.bounds("agenda-overview-grid");
    manager.click("agenda-summary-record-1");
    let record_detail = manager.bounds("agenda-record-detail");
    let record_title = manager.bounds("agenda-record-title");
    assert_inside(record_title, record_detail);
    assert!(record_title.size.width >= px(240.));
    assert!(record_title.size.height >= px(17.));
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.agenda.popup_open);
        assert!(!view.agenda.editor_visible);
        assert!(!view.agenda.editing);
        assert_eq!(view.agenda.selected_record, Some(1));
        assert_eq!(view.store.as_ref().unwrap().sessions().unwrap(), before);
        assert!(view.active.is_empty());
    });
    assert_same_bounds(manager.bounds("agenda-overview-grid"), grid);
    manager.click("agenda-popover-close");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.open_agenda_day(date, window, cx)
    });
    settle(&mut manager);
    manager.click("agenda-summary-completion-2");
    let completion_detail = manager.bounds("agenda-completion-detail");
    let completion_title = manager.bounds("agenda-completion-title");
    assert_inside(completion_title, completion_detail);
    assert!(completion_title.size.width >= px(240.));
    assert!(completion_title.size.height >= px(17.));
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.agenda.popup_open);
        assert!(!view.agenda.editor_visible);
        assert!(!view.agenda.editing);
        assert_eq!(view.agenda.selected_completion, Some(2));
        assert_eq!(view.store.as_ref().unwrap().sessions().unwrap(), before);
        assert!(view.active.is_empty());
    });
}

fn assert_same_bounds(actual: Bounds<Pixels>, expected: Bounds<Pixels>) {
    for (actual, expected) in [
        (actual.left(), expected.left()),
        (actual.top(), expected.top()),
        (actual.size.width, expected.size.width),
        (actual.size.height, expected.size.height),
    ] {
        assert!(
            (actual - expected).abs() <= px(1.),
            "{actual:?} != {expected:?}"
        );
    }
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
        let popup = manager.bounds("agenda-popover");
        assert_inside(popup, viewport);
        assert_inside(editor, popup);
        assert!(popup.size.width >= px(320.) && popup.size.width <= px(380.));
        assert_same_bounds(
            Bounds::new(
                manager.bounds("agenda-date-input").origin,
                manager.bounds("agenda-title-input").size,
            ),
            manager.bounds("agenda-date-input"),
        );
        manager.click("agenda-note-toggle");
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
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.edit_plan(saved.id, window, cx)
        });
        settle(&mut manager);
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
fn agenda_ui_read_only_navigation_retains_drafts_and_different_editors_require_a_choice(
    cx: &mut TestAppContext,
) {
    let mut manager = day_manager(cx, 1040., 780., Page::Agenda);
    let first = seed_plan(&mut manager, "First arrangement");
    let second = seed_plan(&mut manager, "Other arrangement");
    select_first_plan(&mut manager);
    manager.click("agenda-plan-edit");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.agenda.note_input.update(cx, |input, cx| {
            input.set_value("Keep until confirmed", window, cx)
        });
        view.select_plan(second.id, window, cx);
    });
    settle(&mut manager);
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.agenda.editing);
        assert!(!view.agenda.editor_visible);
        assert!(view.agenda_has_suspended_draft(cx));
        assert_eq!(view.agenda.edit_id, Some(first.id));
        assert_eq!(view.agenda.selected_plan, Some(second.id));
    });
    manager.click("agenda-plan-edit");
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.agenda.editor_visible);
        assert_eq!(view.agenda.edit_id, Some(first.id));
        assert_eq!(view.agenda.selected_plan, Some(second.id));
        assert_eq!(
            view.agenda.note_input.read(cx).value(),
            "Keep until confirmed"
        );
    });
    manager.click("agenda-plan-edit");
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.agenda.editor_visible);
        assert_eq!(view.agenda.edit_id, Some(second.id));
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .plans_for_day(first.day)
                .unwrap()
                .iter()
                .find(|plan| plan.id == first.id)
                .unwrap()
                .note,
            "Keep until confirmed"
        );
    });
    let tomorrow = first.day.succ_opt().unwrap();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.agenda
            .end_input
            .update(cx, |input, cx| input.set_value("invalid", window, cx));
        view.select_agenda_date(tomorrow, window, cx);
    });
    settle(&mut manager);
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.agenda.date, tomorrow);
        assert!(!view.agenda.editor_visible);
        assert!(view.agenda_has_suspended_draft(cx));
        assert_eq!(view.agenda.end_input.read(cx).value(), "invalid");
    });
    manager.click("agenda-resume-draft");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.agenda.date, first.day);
        assert_eq!(view.agenda.edit_id, Some(second.id));
        assert_eq!(view.agenda.end_input.read(cx).value(), "invalid");
    });
    manager.click("agenda-save");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.agenda.editor_visible);
        assert!(view.agenda.error.is_some());
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .plans_for_day(first.day)
                .unwrap()
                .iter()
                .find(|plan| plan.id == second.id)
                .unwrap()
                .end_minute,
            600
        );
    });
    manager.click("agenda-cancel");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_plan(first.id, window, cx);
        view.agenda.note_input.update(cx, |input, cx| {
            input.set_value("Discard this edit", window, cx)
        });
        view.edit_plan(second.id, window, cx);
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("不保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.agenda.editor_visible);
        assert_eq!(view.agenda.edit_id, Some(second.id));
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .plans_for_day(first.day)
                .unwrap()
                .iter()
                .find(|plan| plan.id == first.id)
                .unwrap()
                .note,
            "Keep until confirmed"
        );
        assert!(view.active.is_empty());
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
            view.selected_tag = 1;
            view.navigate(Page::Agenda, window, cx);
            view.agenda.month_overview = false;
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
    manager.click("agenda-overview-add");
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
    assert!(!manager.cx.has_pending_prompt());
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(matches!(view.page, Page::Agenda));
        assert!(!view.agenda.popup_open);
        assert!(!view.agenda.editor_visible);
        assert!(view.agenda_has_suspended_draft(cx));
    });
    manager.click("agenda-resume-draft");
    manager.click("agenda-save");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(matches!(view.page, Page::Agenda));
        assert!(view.agenda.error.is_some());
        assert!(view.agenda.editing);
        assert!(view.agenda.editor_visible);
        assert!(view.agenda.popup_open);
        assert!(view.agenda.month_overview);
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

#[gpui::test]
fn agenda_ui_bottom_editor_fields_do_not_jump_when_validation_changes(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        let date = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            view.agenda.month = date.with_day(1).unwrap();
            view.agenda.date = date;
            cx.notify();
        });
        settle(&mut manager);
        manager.click("agenda-overview-add");
        fill_plan(
            &mut manager,
            "Keep the field still",
            date,
            "10:00",
            "invalid",
            "",
        );
        let title = manager.bounds("agenda-title-input");
        let end = manager.bounds("agenda-end-input");
        let popup = manager.bounds("agenda-popover");
        manager.click("agenda-save");
        manager
            .view
            .read_with(&manager.cx, |view, _| assert!(view.agenda.error.is_some()));
        assert_same_bounds(manager.bounds("agenda-title-input"), title);
        assert_same_bounds(manager.bounds("agenda-end-input"), end);
        assert_same_bounds(manager.bounds("agenda-popover"), popup);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.agenda
                .end_input
                .update(cx, |input, cx| input.set_value("11:00", window, cx));
        });
        settle(&mut manager);
        manager
            .view
            .read_with(&manager.cx, |view, _| assert!(view.agenda.error.is_none()));
        assert_same_bounds(manager.bounds("agenda-title-input"), title);
        assert_same_bounds(manager.bounds("agenda-end-input"), end);
        assert_inside(
            manager.bounds("agenda-save"),
            Bounds::new(Point::default(), size(px(width), px(height))),
        );
    }
}

#[gpui::test]
fn agenda_ui_error_remains_visible_above_actions_with_expanded_notes(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        let date = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            view.agenda.month = date.with_day(1).unwrap();
            view.agenda.date = date;
            cx.notify();
        });
        settle(&mut manager);
        manager.click("agenda-overview-add");
        manager.click("agenda-note-toggle");
        let fields = manager.bounds("agenda-editor-fields");
        assert_inside(manager.bounds("agenda-note-toggle"), fields);
        manager.click("agenda-save");
        let title = manager.bounds("agenda-title-input");
        assert_inside(title, manager.bounds("agenda-editor-fields"));
        let error = manager.bounds("agenda-error");
        let slot = manager.bounds("agenda-validation-slot");
        let actions = manager.bounds("agenda-editor-actions");
        assert_inside(error, slot);
        assert_inside(slot, manager.bounds("agenda-popover"));
        assert!(
            slot.top() >= fields.bottom(),
            "the error must not be clipped by the scrolling fields: fields={fields:?}, slot={slot:?}"
        );
        assert!(
            slot.bottom() <= actions.top(),
            "the error must not overlap the actions"
        );
        assert_same_bounds(manager.bounds("agenda-title-input"), title);
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.agenda.note_expanded);
            assert!(view.agenda.error.is_some());
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.agenda
                .title_input
                .update(cx, |input, cx| input.set_value("a", window, cx));
        });
        settle(&mut manager);
        manager
            .view
            .read_with(&manager.cx, |view, _| assert!(view.agenda.error.is_none()));
        assert_same_bounds(manager.bounds("agenda-title-input"), title);
        assert_same_bounds(manager.bounds("agenda-validation-slot"), slot);
        assert_same_bounds(manager.bounds("agenda-editor-actions"), actions);
    }
}

#[gpui::test]
fn agenda_ui_expanding_notes_reveals_input_and_retains_validation_on_scroll(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        manager.click("agenda-overview-add");
        manager.click("agenda-note-toggle");
        let fields = manager.bounds("agenda-editor-fields");
        assert_inside(manager.bounds("agenda-note-input"), fields);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert!(
                view.agenda
                    .note_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
        });
        manager.click("agenda-save");
        assert_inside(
            manager.bounds("agenda-title-input"),
            manager.bounds("agenda-editor-fields"),
        );
        assert_inside(
            manager.bounds("agenda-error"),
            manager.bounds("agenda-validation-slot"),
        );
        manager.scroll("agenda-editor-fields", -240.);
        manager
            .view
            .read_with(&manager.cx, |view, _| assert!(view.agenda.error.is_some()));
        assert_inside(
            manager.bounds("agenda-error"),
            manager.bounds("agenda-validation-slot"),
        );
        assert_inside(
            manager.bounds("agenda-validation-slot"),
            manager.bounds("agenda-popover"),
        );
    }
}

#[gpui::test]
fn agenda_ui_collapsing_notes_restores_scroll_and_prior_input_without_losing_text(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        manager.click("agenda-overview-add");
        manager.cx.simulate_input("A title to keep");
        settle(&mut manager);
        let before_scroll = manager
            .view
            .read_with(&manager.cx, |view, _| view.agenda.editor_scroll.offset());
        manager.click("agenda-note-toggle");
        manager.cx.simulate_input("A note to keep");
        settle(&mut manager);
        assert_inside(
            manager.bounds("agenda-note-input"),
            manager.bounds("agenda-editor-fields"),
        );
        manager.click("agenda-note-toggle");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert!(!view.agenda.note_expanded);
            assert_eq!(view.agenda.editor_scroll.offset(), before_scroll);
            assert_eq!(view.agenda.title_input.read(cx).value(), "A title to keep");
            assert_eq!(view.agenda.note_input.read(cx).value(), "A note to keep");
            assert!(
                view.agenda
                    .title_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
        });
        assert_inside(
            manager.bounds("agenda-title-input"),
            manager.bounds("agenda-editor-fields"),
        );
    }
}

#[gpui::test]
fn agenda_ui_invalid_field_is_revealed_above_fixed_error_after_note_scroll(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        for field in ["title", "date", "date_before_time", "start", "end", "note"] {
            let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
            manager.click("agenda-overview-add");
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                for (input, value) in [
                    (
                        &view.agenda.title_input,
                        if field == "title" { "" } else { "A title" },
                    ),
                    (
                        &view.agenda.date_input,
                        if field == "date" || field == "date_before_time" {
                            "bad date"
                        } else {
                            "2026-10-27"
                        },
                    ),
                    (
                        &view.agenda.start_input,
                        if field == "start" || field == "date_before_time" {
                            "bad start"
                        } else {
                            "09:00"
                        },
                    ),
                    (
                        &view.agenda.end_input,
                        if field == "end" { "bad end" } else { "09:30" },
                    ),
                ] {
                    input.update(cx, |input, cx| input.set_value(value, window, cx));
                }
                if field == "note" {
                    view.agenda.note_input.update(cx, |input, cx| {
                        input.set_value("x".repeat(2001), window, cx)
                    });
                }
            });
            settle(&mut manager);
            if field != "note" {
                manager.click("agenda-note-toggle");
            }
            manager.click("agenda-save");
            let selector = match field {
                "title" => "agenda-title-input",
                "date" | "date_before_time" => "agenda-date-input",
                "start" => "agenda-start-input",
                "end" => "agenda-end-input",
                _ => "agenda-note-input",
            };
            assert_inside(
                manager.bounds(selector),
                manager.bounds("agenda-editor-fields"),
            );
            assert_inside(
                manager.bounds("agenda-error"),
                manager.bounds("agenda-validation-slot"),
            );
            assert_inside(
                manager.bounds("agenda-validation-slot"),
                manager.bounds("agenda-popover"),
            );
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                let input = match field {
                    "title" => &view.agenda.title_input,
                    "date" | "date_before_time" => &view.agenda.date_input,
                    "start" => &view.agenda.start_input,
                    "end" => &view.agenda.end_input,
                    _ => &view.agenda.note_input,
                };
                assert!(
                    input.read(cx).focus_handle(cx).is_focused(window),
                    "incorrect focus for {field}"
                );
                assert!(view.agenda.error.is_some());
                if field == "note" {
                    assert!(view.agenda.note_expanded);
                }
            });
        }
    }
}

#[gpui::test]
fn agenda_ui_resumed_note_draft_reveals_the_focused_editor_field(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        manager.click("agenda-overview-add");
        manager.cx.simulate_input("A title to keep");
        settle(&mut manager);
        manager.click("agenda-note-toggle");
        manager.cx.simulate_input("A note to keep");
        settle(&mut manager);
        manager.click("agenda-popover-close");
        manager.click("agenda-resume-draft");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert_eq!(view.agenda.title_input.read(cx).value(), "A title to keep");
            assert_eq!(view.agenda.note_input.read(cx).value(), "A note to keep");
            assert!(
                view.agenda
                    .note_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
        });
        assert_inside(
            manager.bounds("agenda-note-input"),
            manager.bounds("agenda-editor-fields"),
        );
    }
}

#[gpui::test]
fn agenda_ui_moments_appear_as_points_without_creating_tasks_or_timer_totals(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        let date = Local::now().date_naive();
        let stamp = day_bounds(date).0 + 9 * 3600 + 30 * 60;
        let mut moment_id = 0;
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            moment_id = view
                .store
                .as_mut()
                .unwrap()
                .create_moment("突然想到一个小点子，先记下来\n稍后再整理", stamp, view.now)
                .unwrap()
                .id;
            view.refresh(cx);
            view.open_agenda_day(date, window, cx);
            assert_eq!(view.agenda.day_moments.len(), 1);
            assert!(view.agenda.actual_days.contains(&date));
            assert_eq!(view.agenda.stats.focus_seconds, 0);
            assert!(view.agenda.records.is_empty());
            assert!(view.tasks.is_empty());
            let items = &view.agenda.month_days[&date];
            assert_eq!(items.len(), 1);
            assert_eq!(
                items[0].status,
                crate::agenda_model::AgendaItemStatus::Moment
            );
            assert_eq!(items[0].seconds, 0);
            view.refresh_agenda_actuals(cx);
            assert_eq!(
                view.agenda.month_days[&date].len(),
                1,
                "timer ticks must not duplicate notes"
            );
        });
        settle(&mut manager);
        let viewport = Bounds::new(Point::default(), size(px(width), px(height)));
        let popup = manager.bounds("agenda-popover");
        assert_inside(popup, viewport);
        assert_inside(manager.bounds("agenda-moment-add"), popup);
        manager.click("agenda-popover-timeline");
        let marker_selector: &'static str =
            Box::leak(format!("agenda-moment-{moment_id}").into_boxed_str());
        let title_selector: &'static str =
            Box::leak(format!("agenda-moment-title-{moment_id}").into_boxed_str());
        // Like plan selection above, reach the marker with actual wheel input.
        // The platform's initial on-next-frame scroll is native acceptance;
        // merely scheduling it does not move TestPlatform's visible hit area.
        let marker = manager.bounds(marker_selector);
        let grid = manager.bounds("agenda-day-grid");
        if marker.top() < grid.top() || marker.bottom() > grid.bottom() {
            manager.scroll(
                "agenda-day-grid",
                f32::from(grid.center().y - marker.center().y),
            );
        }
        let marker = manager.bounds(marker_selector);
        let title = manager.bounds(title_selector);
        assert_eq!(marker.size.height, px(28.));
        assert_inside(title, marker);
        assert_inside(marker, manager.bounds("agenda-day-grid"));
        manager.click(marker_selector);
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.moments.visible, "a point marker must open its editor");
            assert_eq!(view.moments.editing, Some(moment_id));
        });
        assert_inside(manager.bounds("moment-editor"), viewport);
    }
}

#[gpui::test]
fn agenda_ui_timeline_scrollbars_reach_both_edges_without_moving_the_viewport(
    cx: &mut TestAppContext,
) {
    let mut manager = day_manager(cx, 780., 620., Page::Agenda);
    for index in 0..8 {
        seed_plan(&mut manager, &format!("Overlapping plan {index}"));
    }
    let viewport = manager.bounds("agenda-day-viewport");
    manager.scroll("agenda-day-grid", -120.);
    manager.cx.simulate_click(
        Point::new(viewport.right() - px(6.), viewport.bottom() - px(20.)),
        Modifiers::default(),
    );
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        let scroll = &view.agenda.calendar_scroll;
        assert!(scroll.max_offset().height > px(0.));
        assert_eq!(scroll.offset().y, -scroll.max_offset().height);
        assert!(
            !view.agenda.editing,
            "scrollbar must not open an hour editor"
        );
    });
    manager.cx.simulate_click(
        Point::new(viewport.right() - px(20.), viewport.bottom() - px(6.)),
        Modifiers::default(),
    );
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        let scroll = &view.agenda.calendar_scroll;
        assert!(scroll.max_offset().width > px(0.));
        assert_eq!(scroll.offset().x, -scroll.max_offset().width);
        assert!(!view.agenda.editing, "scrollbar must not select a plan");
    });
    assert_eq!(manager.bounds("agenda-day-viewport"), viewport);
}
