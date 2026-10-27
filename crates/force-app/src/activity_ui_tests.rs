//! Keyboard interaction on GPUI's isolated test platform.
use crate::{
    headless_ui_tests::ManagerHarness,
    model::{Page, day_bounds},
};
use chrono::{Duration, Local};
use gpui::TestAppContext;

fn grid_is_focused(manager: &mut ManagerHarness) -> bool {
    manager.view.update_in(&mut manager.cx, |view, window, _| {
        view.activity_focus.is_focused(window)
    })
}

#[gpui::test]
fn activity_keyboard_moves_by_grid_direction_and_updates_records(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Records);
        let today = Local::now().date_naive();
        let yesterday = today - Duration::days(1);
        manager.view.update(&mut manager.cx, |view, cx| {
            let start = day_bounds(yesterday).0 + 3600;
            let store = view.store.as_mut().unwrap();
            let record = store.start("Yesterday", 1, start).unwrap();
            store.finish(record.id, start + 120).unwrap();
            view.refresh(cx);
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        manager.view.update_in(&mut manager.cx, |view, window, _| {
            window.focus(&view.focus_handle);
        });
        for _ in 0..20 {
            manager.cx.simulate_keystrokes("tab");
            manager.cx.run_until_parked();
            if grid_is_focused(&mut manager) {
                break;
            }
        }
        assert!(
            grid_is_focused(&mut manager),
            "the grid must have one Tab entry"
        );
        manager.cx.simulate_keystrokes("up");
        manager.cx.run_until_parked();
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.record_date, yesterday);
            assert_eq!(view.record_stats.focus_seconds, 120);
            assert_eq!(view.history[0].title, "Yesterday");
        });
        assert!(grid_is_focused(&mut manager));
        for (key, expected) in [
            ("left", today - Duration::days(8)),
            ("right", yesterday),
            ("down", today),
        ] {
            manager.cx.simulate_keystrokes(key);
            manager.cx.run_until_parked();
            manager.view.read_with(&manager.cx, |view, _| {
                assert_eq!(view.record_date, expected)
            });
            assert!(grid_is_focused(&mut manager));
        }
        manager.cx.simulate_keystrokes("down");
        manager.cx.simulate_keystrokes("right");
        manager.cx.simulate_keystrokes("shift-left");
        manager.cx.run_until_parked();
        manager
            .view
            .read_with(&manager.cx, |view, _| assert_eq!(view.record_date, today));
        let mouse_date = today - Duration::days(3);
        let selector: &'static str =
            Box::leak(format!("activity-day-{mouse_date}").into_boxed_str());
        manager.click(selector);
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.record_date, mouse_date)
        });
        assert!(grid_is_focused(&mut manager));
        manager.cx.simulate_keystrokes("tab");
        manager.cx.run_until_parked();
        assert!(
            !grid_is_focused(&mut manager),
            "Tab must leave the entire grid in one step"
        );
    }
}

#[gpui::test]
fn activity_keyboard_respects_earliest_day_and_anchors_outside_selection_at_today(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    let today = Local::now().date_naive();
    manager
        .view
        .update(&mut manager.cx, |view, cx| view.refresh(cx));
    manager.cx.run_until_parked();
    let first = manager
        .view
        .read_with(&manager.cx, |view, _| view.activity_days[0].date);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_record_date(first, cx);
        window.focus(&view.activity_focus);
    });
    manager.cx.run_until_parked();
    manager.cx.simulate_keystrokes("up");
    manager.cx.simulate_keystrokes("left");
    manager.cx.run_until_parked();
    manager
        .view
        .read_with(&manager.cx, |view, _| assert_eq!(view.record_date, first));
    assert!(grid_is_focused(&mut manager));
    let outside = first - Duration::days(10);
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        view.select_record_date(outside, cx)
    });
    manager.cx.run_until_parked();
    manager
        .view
        .read_with(&manager.cx, |view, _| assert_eq!(view.record_date, outside));
    manager.cx.simulate_keystrokes("left");
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.record_date, today - Duration::days(7))
    });
    assert!(grid_is_focused(&mut manager));
}

#[gpui::test]
fn clicking_a_day_moves_focus_from_search_to_grid_for_arrow_keys(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    manager.cx.update(|window, _| window.activate_window());
    manager.cx.run_until_parked();
    let today = Local::now().date_naive();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.refresh(cx);
        view.record_search
            .update(cx, |input, cx| input.focus(window, cx));
    });
    manager.cx.run_until_parked();
    manager.cx.simulate_input("example");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.record_search.read(cx).value(), "example");
    });
    let clicked = today - Duration::days(2);
    let selector: &'static str = Box::leak(format!("activity-day-{clicked}").into_boxed_str());
    let before = manager
        .view
        .read_with(&manager.cx, |view, _| view.panel_scroll[1].offset());
    manager.click(selector);
    assert!(
        grid_is_focused(&mut manager),
        "clicking the grid must move focus out of search"
    );
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.panel_scroll[1].offset(), before)
    });
    manager.cx.simulate_keystrokes("up");
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.record_date, clicked - Duration::days(1));
        assert_eq!(view.record_search.read(cx).value(), "example");
    });
}

#[gpui::test]
fn enter_and_space_select_the_focused_day_from_an_older_record(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    let today = Local::now().date_naive();
    manager
        .view
        .update(&mut manager.cx, |view, cx| view.refresh(cx));
    let first = manager
        .view
        .read_with(&manager.cx, |view, _| view.activity_days[0].date);
    let outside = first - Duration::days(10);
    for key in ["enter", "space"] {
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.select_record_date(outside, cx);
            window.focus(&view.activity_focus);
        });
        manager.cx.run_until_parked();
        manager.cx.simulate_keystrokes(key);
        manager.cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
        manager.cx.run_until_parked();
        manager
            .view
            .read_with(&manager.cx, |view, _| assert_eq!(view.record_date, today));
        assert!(grid_is_focused(&mut manager));
        manager.cx.simulate_keystrokes(key);
        manager.cx.run_until_parked();
        manager
            .view
            .read_with(&manager.cx, |view, _| assert_eq!(view.record_date, today));
    }
}

#[gpui::test]
fn shift_tab_to_activity_grid_reveals_it_after_scrolling_through_records(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    manager.cx.update(|window, _| window.activate_window());
    manager.cx.run_until_parked();
    manager.view.update(&mut manager.cx, |view, cx| {
        let date = Local::now().date_naive() - Duration::days(1);
        view.record_date = date;
        let start = day_bounds(date).0 + 3600;
        let store = view.store.as_mut().unwrap();
        for index in 0..30 {
            let session = store
                .start(&format!("Record {index}"), 1, start + index * 100)
                .unwrap();
            store.finish(session.id, start + index * 100 + 60).unwrap();
        }
        view.refresh(cx);
    });
    manager.cx.run_until_parked();
    manager.scroll("activity-grid", -1500.);
    let offset = manager
        .view
        .read_with(&manager.cx, |view, _| view.panel_scroll[1].offset().y);
    assert!(offset < gpui::px(0.));
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.record_search
            .update(cx, |input, cx| input.focus(window, cx));
    });
    manager.cx.run_until_parked();
    for _ in 0..10 {
        manager.cx.simulate_keystrokes("shift-tab");
        manager.cx.run_until_parked();
        if grid_is_focused(&mut manager) {
            break;
        }
    }
    assert!(grid_is_focused(&mut manager));
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let grid = manager.bounds("activity-grid");
    assert!(
        grid.top() >= gpui::px(68.) && grid.bottom() <= gpui::px(620.),
        "focused grid should be visible: {grid:?}"
    );
}

#[gpui::test]
fn clicking_partially_visible_grid_does_not_scroll_it(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    manager.cx.update(|window, _| window.activate_window());
    manager.cx.run_until_parked();
    manager.view.update(&mut manager.cx, |view, cx| {
        let today = Local::now().date_naive();
        let date = today
            - Duration::days(i64::from(
                chrono::Datelike::weekday(&today).num_days_from_sunday(),
            ));
        view.record_date = date;
        let start = day_bounds(date).0 + 3600;
        let store = view.store.as_mut().unwrap();
        for index in 0..30 {
            let session = store
                .start(&format!("Record {index}"), 1, start + index * 100)
                .unwrap();
            store.finish(session.id, start + index * 100 + 60).unwrap();
        }
        view.refresh(cx);
    });
    manager.cx.run_until_parked();
    manager.scroll("activity-grid", -165.);
    let grid = manager.bounds("activity-grid");
    assert!(grid.top() < gpui::px(68.) && grid.bottom() > gpui::px(68.));
    let before = manager
        .view
        .read_with(&manager.cx, |view, _| view.panel_scroll[1].offset());
    let date = manager.view.read_with(&manager.cx, |view, _| {
        view.activity_days
            .iter()
            .rev()
            .find(|day| {
                day.date <= Local::now().date_naive()
                    && chrono::Datelike::weekday(&day.date) == chrono::Weekday::Sun
            })
            .unwrap()
            .date
    });
    let selector: &'static str = Box::leak(format!("activity-day-{date}").into_boxed_str());
    manager.click(selector);
    assert!(grid_is_focused(&mut manager));
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(
            !view.activity_pointer_focus,
            "the real focus callback should consume mouse focus"
        );
        assert_eq!(view.panel_scroll[1].offset(), before)
    });
}

#[gpui::test]
fn returning_to_window_does_not_scroll_the_previously_focused_grid(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    manager.cx.update(|window, _| window.activate_window());
    manager.cx.run_until_parked();
    manager.view.update(&mut manager.cx, |view, cx| {
        let date = Local::now().date_naive() - Duration::days(1);
        view.record_date = date;
        let start = day_bounds(date).0 + 3600;
        let store = view.store.as_mut().unwrap();
        for index in 0..30 {
            let session = store
                .start(&format!("Record {index}"), 1, start + index * 100)
                .unwrap();
            store.finish(session.id, start + index * 100 + 60).unwrap();
        }
        view.refresh(cx);
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, _| {
        window.focus(&view.activity_focus)
    });
    manager.cx.run_until_parked();
    manager.scroll("activity-grid", -1000.);
    let before = manager
        .view
        .read_with(&manager.cx, |view, _| view.panel_scroll[1].offset());
    assert!(before.y < gpui::px(0.));
    manager.cx.deactivate_window();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.activate_window());
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(
            !view.activity_reactivating,
            "focus callback should consume reactivation"
        );
        assert_eq!(view.panel_scroll[1].offset(), before)
    });
}

#[gpui::test]
fn arrow_reveals_grid_after_mouse_scroll_without_scrolling_on_wheel(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    manager.cx.update(|window, _| window.activate_window());
    manager.cx.run_until_parked();
    let selected = Local::now().date_naive() - Duration::days(1);
    let previous = selected - Duration::days(1);
    manager.view.update(&mut manager.cx, |view, cx| {
        view.record_date = selected;
        let store = view.store.as_mut().unwrap();
        for date in [selected, previous] {
            let start = day_bounds(date).0 + 3600;
            for index in 0..30 {
                let session = store
                    .start(&format!("Record {date} {index}"), 1, start + index * 100)
                    .unwrap();
                store.finish(session.id, start + index * 100 + 60).unwrap();
            }
        }
        view.refresh(cx);
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, _| {
        window.focus(&view.activity_focus)
    });
    manager.cx.run_until_parked();
    manager.scroll("activity-grid", -1000.);
    assert!(grid_is_focused(&mut manager));
    let offset = manager
        .view
        .read_with(&manager.cx, |view, _| view.panel_scroll[1].offset().y);
    assert!(offset < gpui::px(0.));
    assert!(manager.bounds("activity-grid").bottom() < gpui::px(68.));
    manager.cx.simulate_keystrokes("up");
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.record_date, previous)
    });
    assert!(grid_is_focused(&mut manager));
    let grid = manager.bounds("activity-grid");
    assert!(
        grid.top() >= gpui::px(68.) && grid.bottom() <= gpui::px(620.),
        "keyboard navigation must reveal the grid: {grid:?}"
    );
}

#[gpui::test]
fn same_frame_pointer_then_search_does_not_hide_later_keyboard_focus(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    manager.cx.update(|window, _| window.activate_window());
    manager.cx.run_until_parked();
    let today = Local::now().date_naive();
    let date = today
        - Duration::days(i64::from(
            chrono::Datelike::weekday(&today).num_days_from_sunday(),
        ));
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.record_date = date;
        let store = view.store.as_mut().unwrap();
        let start = day_bounds(date).0 + 3600;
        for index in 0..30 {
            let session = store
                .start(&format!("Record {index}"), 1, start + index * 100)
                .unwrap();
            store.finish(session.id, start + index * 100 + 60).unwrap();
        }
        view.refresh(cx);
        view.record_search
            .update(cx, |input, cx| input.focus(window, cx));
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let selector: &'static str = Box::leak(format!("activity-day-{date}").into_boxed_str());
    let bounds = manager.bounds(selector);
    manager
        .cx
        .simulate_click(bounds.center(), gpui::Modifiers::default());
    // Move focus away before the frame that dispatches focus-change notifications.
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.record_search
            .update(cx, |input, cx| input.focus(window, cx));
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.scroll("activity-grid", -1000.);
    for _ in 0..10 {
        manager.cx.simulate_keystrokes("shift-tab");
        manager.cx.run_until_parked();
        if grid_is_focused(&mut manager) {
            break;
        }
    }
    assert!(grid_is_focused(&mut manager));
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let grid = manager.bounds("activity-grid");
    assert!(
        grid.top() >= gpui::px(68.) && grid.bottom() <= gpui::px(620.),
        "grid must be revealed after later keyboard focus: {grid:?}"
    );
}

#[gpui::test]
fn keyboard_navigation_dismisses_hover_tooltip_and_mouse_move_restores_correct_date(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    manager.cx.update(|window, _| window.activate_window());
    manager.cx.run_until_parked();
    let today = Local::now().date_naive();
    let yesterday = today - Duration::days(1);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.refresh(cx);
        window.focus(&view.activity_focus);
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let tooltip = manager
        .view
        .read_with(&manager.cx, |view, _| view.activity_tooltip_probe.clone());
    let first: &'static str = Box::leak(format!("activity-day-{today}").into_boxed_str());
    let second: &'static str = Box::leak(format!("activity-day-{yesterday}").into_boxed_str());
    let first_pos = manager.bounds(first).center();
    let second_pos = manager.bounds(second).center();
    manager
        .cx
        .simulate_mouse_move(first_pos, None, gpui::Modifiers::default());
    manager
        .cx
        .executor()
        .advance_clock(std::time::Duration::from_millis(600));
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let shown = tooltip
        .borrow_mut()
        .take()
        .expect("the hovered day's tooltip must be rendered");
    assert_eq!(shown.0, today);
    assert!(
        shown
            .1
            .contains(&today.format("%Y年%-m月%-d日").to_string())
    );
    manager.cx.simulate_keystrokes("left");
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    assert!(
        tooltip.borrow_mut().take().is_none(),
        "old hover tooltip must disappear after a key"
    );
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.record_date, today - Duration::days(7))
    });
    manager.cx.simulate_mouse_move(
        first_pos + gpui::point(gpui::px(1.), gpui::px(0.)),
        None,
        gpui::Modifiers::default(),
    );
    manager
        .cx
        .executor()
        .advance_clock(std::time::Duration::from_millis(600));
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    assert_eq!(
        tooltip
            .borrow_mut()
            .take()
            .expect("moving within the same cell restores its tooltip")
            .0,
        today
    );
    manager.cx.simulate_keystrokes("left");
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    assert!(tooltip.borrow_mut().take().is_none());
    manager
        .cx
        .simulate_mouse_move(second_pos, None, gpui::Modifiers::default());
    manager
        .cx
        .executor()
        .advance_clock(std::time::Duration::from_millis(600));
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let shown = tooltip
        .borrow_mut()
        .take()
        .expect("one mouse movement must restore the tooltip");
    assert_eq!(shown.0, yesterday);
    assert!(
        shown
            .1
            .contains(&yesterday.format("%Y年%-m月%-d日").to_string())
    );
    // A pending delayed tooltip must not reappear after keyboard navigation.
    manager.cx.simulate_keystrokes("left");
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    tooltip.borrow_mut().take();
    manager
        .cx
        .simulate_mouse_move(first_pos, None, gpui::Modifiers::default());
    manager.cx.simulate_keystrokes("up");
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager
        .cx
        .executor()
        .advance_clock(std::time::Duration::from_millis(600));
    manager.cx.run_until_parked();
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    assert!(
        tooltip.borrow_mut().take().is_none(),
        "a pending tooltip must not reappear after navigation"
    );
}
