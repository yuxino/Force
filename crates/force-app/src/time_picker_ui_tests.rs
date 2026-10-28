//! Event and layout acceptance for the clock; native pixels and sound are checked separately.
use gpui::{
    Bounds, Focusable as _, Modifiers, MouseButton, Pixels, Point, TestAppContext, point, px, size,
};

use crate::{headless_ui_tests::ManagerHarness, model::Page};

fn settle(manager: &mut ManagerHarness) {
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
}

fn inside(child: Bounds<Pixels>, parent: Bounds<Pixels>) {
    assert!(
        child.left() >= parent.left() - px(1.),
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.right() <= parent.right() + px(1.),
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.top() >= parent.top() - px(1.),
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.bottom() <= parent.bottom() + px(1.),
        "{child:?} outside {parent:?}"
    );
}

fn set_plan_times(manager: &mut ManagerHarness, start: &str, end: &str) {
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.agenda.start_input.update(cx, |input, cx| {
            input.set_value(start.to_owned(), window, cx)
        });
        view.agenda
            .end_input
            .update(cx, |input, cx| input.set_value(end.to_owned(), window, cx));
    });
    settle(manager);
}

fn choose_dial(manager: &mut ManagerHarness, offset: Point<Pixels>) {
    let position = manager.bounds("time-picker-dial").center() + offset;
    manager.cx.simulate_click(position, Modifiers::default());
    settle(manager);
}

#[gpui::test]
fn time_picker_ui_plan_clock_is_bounded_and_only_confirmation_changes_time(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        manager.click("agenda-overview-add");
        manager.cx.simulate_input("Keep this plan draft");
        set_plan_times(&mut manager, "09:07", "10:00");
        manager.click("agenda-start-picker");
        let panel = manager.bounds("time-picker");
        inside(
            panel,
            Bounds::new(Point::default(), size(px(width), px(height))),
        );
        for selector in [
            "time-picker-hour",
            "time-picker-minute",
            "time-picker-dial",
            "time-picker-cancel",
            "time-picker-confirm",
        ] {
            inside(manager.bounds(selector), panel);
        }
        assert!(manager.cx.debug_bounds("time-picker-midnight").is_none());
        choose_dial(&mut manager, point(px(82.), px(0.))); // 06 hours on a 24-hour face.
        manager.click("time-picker-minute");
        let center = manager.bounds("time-picker-dial").center();
        manager.cx.simulate_mouse_down(
            center + point(px(82.), px(0.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        manager.cx.simulate_mouse_move(
            center + point(px(0.), px(82.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        manager.cx.simulate_mouse_up(
            center + point(px(0.), px(82.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        settle(&mut manager);
        manager.view.read_with(&manager.cx, |view, cx| {
            assert_eq!(view.agenda.start_input.read(cx).value(), "09:07");
            assert_eq!(
                view.agenda.title_input.read(cx).value(),
                "Keep this plan draft"
            );
            assert!(view.agenda.editor_visible);
        });
        manager.click("time-picker-confirm");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert_eq!(view.agenda.start_input.read(cx).value(), "06:30");
            assert_eq!(view.agenda.end_input.read(cx).value(), "10:00");
            assert!(view.agenda.start_input.focus_handle(cx).is_focused(window));
            assert!(view.agenda.editor_visible);
            assert!(!view.time_picker.read(cx).is_open());
        });
        manager.cx.simulate_keystrokes("secondary-a");
        manager.cx.simulate_input("07:45");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert_eq!(view.agenda.start_input.read(cx).value(), "07:45")
        });
    }
}

#[gpui::test]
fn time_picker_ui_cancel_preserves_invalid_input_and_end_midnight_is_explicit(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    manager.click("agenda-overview-add");
    set_plan_times(&mut manager, "  unfinished  ", "23:30");
    manager.click("agenda-start-picker");
    choose_dial(&mut manager, point(px(0.), px(82.)));
    manager.click("time-picker-cancel");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.agenda.start_input.read(cx).value(), "  unfinished  ")
    });
    manager.click("agenda-start-picker");
    choose_dial(&mut manager, point(px(82.), px(0.)));
    manager
        .cx
        .simulate_click(point(px(4.), px(4.)), Modifiers::default());
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.time_picker.read(cx).is_open());
        assert!(view.agenda.editor_visible);
        assert!(view.agenda.popup_open);
        assert_eq!(view.agenda.start_input.read(cx).value(), "  unfinished  ");
    });
    manager.click("agenda-end-picker");
    manager.click("time-picker-midnight");
    manager.click("time-picker-confirm");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.agenda.end_input.read(cx).value(), "24:00")
    });
}

#[gpui::test]
fn time_picker_ui_keyboard_stays_in_picker_and_returns_to_moment_input(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    manager.click("global-new-moment");
    manager.cx.simulate_input("The note stays unchanged");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.moments
            .time
            .update(cx, |input, cx| input.set_value("09:10", window, cx));
    });
    settle(&mut manager);
    manager.click("moment-time-picker");
    manager.click("time-picker-minute");
    manager.cx.simulate_keystrokes("right");
    for key in ["tab", "shift-tab"] {
        for _ in 0..12 {
            manager.cx.simulate_keystrokes(key);
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                assert!(
                    view.time_picker
                        .focus_handle(cx)
                        .contains_focused(window, cx)
                );
                assert!(view.moments.visible);
                assert_eq!(view.moments.time.read(cx).value(), "09:10");
            });
        }
    }
    manager.click("time-picker-confirm");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(view.moments.visible);
        assert_eq!(
            view.moments.content.read(cx).value(),
            "The note stays unchanged"
        );
        assert_eq!(view.moments.time.read(cx).value(), "09:11");
        assert!(view.moments.time.focus_handle(cx).is_focused(window));
    });
    manager.click("moment-time-picker");
    choose_dial(&mut manager, point(px(0.), px(82.)));
    manager.cx.simulate_keystrokes("escape");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.moments.visible);
        assert!(!view.time_picker.read(cx).is_open());
        assert_eq!(view.moments.time.read(cx).value(), "09:11");
    });
}

#[gpui::test]
fn time_picker_ui_does_not_overwrite_a_changed_parent_draft(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    manager.click("agenda-overview-add");
    set_plan_times(&mut manager, "09:00", "10:00");
    manager.click("agenda-start-picker");
    choose_dial(&mut manager, point(px(82.), px(0.)));
    set_plan_times(&mut manager, "11:45", "12:00");
    manager.click("time-picker-confirm");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.agenda.start_input.read(cx).value(), "11:45")
    });
}

#[gpui::test]
fn time_picker_ui_showing_manager_preserves_picker_focus_and_parent_note(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    manager.click("global-new-moment");
    manager.cx.simulate_input("A protected note");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.moments
            .time
            .update(cx, |input, cx| input.set_value("09:10", window, cx));
    });
    settle(&mut manager);
    manager.click("moment-time-picker");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.show_manager(window, cx)
    });
    settle(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(
            view.time_picker
                .focus_handle(cx)
                .contains_focused(window, cx)
        );
        assert!(view.time_picker.read(cx).is_open());
    });
    manager.cx.simulate_keystrokes("x");
    manager.cx.simulate_input("x");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.moments.content.read(cx).value(), "A protected note");
        assert_eq!(view.moments.time.read(cx).value(), "09:10");
    });
}

#[gpui::test]
fn time_picker_ui_selecting_a_task_or_session_cancels_preview_without_hidden_focus(
    cx: &mut TestAppContext,
) {
    for select_session in [false, true] {
        let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
        let session = manager.view.update_in(&mut manager.cx, |view, _, cx| {
            let session = view
                .store
                .as_mut()
                .unwrap()
                .create_and_start_task("A selected task", 1, chrono::Local::now().timestamp(), None)
                .unwrap();
            view.refresh(cx);
            session
        });
        settle(&mut manager);
        manager.click("agenda-overview-add");
        manager.cx.simulate_input("Preserve the plan draft");
        set_plan_times(&mut manager, "09:10", "10:00");
        manager.click("agenda-start-picker");
        choose_dial(&mut manager, point(px(0.), px(82.)));
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            if select_session {
                view.select_session(session.id, window, cx);
            } else {
                view.select_task(session.task_id.unwrap(), window, cx);
            }
        });
        settle(&mut manager);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert!(matches!(view.page, Page::Focus));
            assert!(!view.time_picker.read(cx).is_open());
            assert!(
                !view
                    .time_picker
                    .focus_handle(cx)
                    .contains_focused(window, cx)
            );
            assert!(!view.agenda.start_input.focus_handle(cx).is_focused(window));
            assert!(!view.agenda.editor_visible);
            assert!(view.agenda.editing);
            assert_eq!(
                view.agenda.title_input.read(cx).value(),
                "Preserve the plan draft"
            );
            assert_eq!(view.agenda.start_input.read(cx).value(), "09:10");
        });
        manager.cx.simulate_keystrokes("x");
        manager.cx.simulate_input("x");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert_eq!(view.agenda.start_input.read(cx).value(), "09:10");
            assert_eq!(
                view.agenda.title_input.read(cx).value(),
                "Preserve the plan draft"
            );
        });
    }
}
