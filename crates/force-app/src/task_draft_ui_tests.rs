//! Tests for temporarily preserving a new task setup with no title.
use crate::headless_ui_tests::ManagerHarness;
use gpui::TestAppContext;

#[gpui::test]
fn untitled_invalid_timer_survives_switching_to_another_task(cx: &mut TestAppContext) {
    for invalid in ["0", "", "minutes"] {
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        let task_id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let task = view
                .store
                .as_mut()
                .unwrap()
                .create_task("Existing", 1, view.now)
                .unwrap();
            view.refresh(cx);
            view.timer_input
                .update(cx, |input, cx| input.set_value(invalid, window, cx));
            task.id
        });
        manager.cx.run_until_parked();
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.select_task(task_id, window, cx);
            assert_eq!(view.selected_task, Some(task_id));
            view.new_task(window, cx);
        });
        assert!(!manager.cx.has_pending_prompt());
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.is_new_task_form());
            assert!(view.task_input.read(cx).value().is_empty());
            assert_eq!(view.timer_input.read(cx).value(), invalid);
            assert!(view.tasks.iter().all(|task| !task.title.is_empty()));
        });
    }
}

#[gpui::test]
fn untitled_changed_tag_survives_switching_to_a_different_task(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let task_id = manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Existing", 1, view.now)
            .unwrap();
        view.refresh(cx);
        assert_eq!(view.selected_tag, 1);
        view.selected_tag = 2;
        task.id
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_task(task_id, window, cx);
        assert_eq!(view.selected_tag, 1);
        view.new_task(window, cx);
    });
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.is_new_task_form());
        assert_eq!(view.selected_tag, 2);
    });
}

#[gpui::test]
fn untitled_countup_draft_survives_switching_to_countdown_session(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let session_id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let session = view
            .store
            .as_mut()
            .unwrap()
            .create_and_start_task("Running", 1, view.now, Some(1500))
            .unwrap();
        view.refresh(cx);
        view.timer_input
            .update(cx, |input, cx| input.set_value("45", window, cx));
        view.countdown = false;
        session.id
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_session(session_id, window, cx);
        assert!(view.countdown);
        view.new_task(window, cx);
    });
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.is_new_task_form());
        assert!(!view.countdown);
        assert_eq!(view.timer_input.read(cx).value(), "45");
        assert_eq!(view.active.len(), 1);
    });
}

#[gpui::test]
fn valid_timer_is_preserved_when_returning_to_the_untitled_draft(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let task_id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Existing", 1, view.now)
            .unwrap();
        view.refresh(cx);
        view.timer_input
            .update(cx, |input, cx| input.set_value("37", window, cx));
        task.id
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_task(task_id, window, cx);
        assert_eq!(view.timer_input.read(cx).value(), "37");
        view.new_task(window, cx);
    });
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.is_new_task_form());
        assert_eq!(view.timer_input.read(cx).value(), "37");
    });
}

#[gpui::test]
fn removed_tag_requires_reselection_even_after_refresh(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let task_id = manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Existing", 1, view.now)
            .unwrap();
        view.refresh(cx);
        view.selected_tag = 2;
        task.id
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_task(task_id, window, cx);
        view.store.as_mut().unwrap().delete_tag(2).unwrap();
        view.refresh(cx);
        view.new_task(window, cx);
        view.task_input.update(cx, |input, cx| {
            input.set_value("Named after restoration", window, cx)
        });
        view.refresh(cx);
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.is_new_task_form());
        assert_eq!(view.selected_tag, 2);
        assert_eq!(view.tag_picker.read(cx).selected_value(), None);
        assert!(view.task_tag_error().is_some());
        assert!(!view.can_start_task(cx));
        assert!(!view.can_save_task(cx));
        assert!(
            view.toast
                .as_ref()
                .is_some_and(|(message, _)| message.contains("原标签已不可用"))
        );
    });
    manager.click("focus-tag-picker");
    manager.click("focus-tag-option-1");
    manager
        .view
        .update_in(&mut manager.cx, |view, _, cx| view.refresh(cx));
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.selected_tag, 1);
        assert!(view.task_tag_error().is_none());
        assert!(view.can_start_task(cx));
        assert!(view.can_save_task(cx));
    });
}

#[gpui::test]
fn discarded_named_draft_does_not_reappear_as_an_untitled_draft(cx: &mut TestAppContext) {
    for session_destination in [false, true] {
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        let id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let store = view.store.as_mut().unwrap();
            let id = if session_destination {
                store
                    .create_and_start_task("Existing", 1, view.now, Some(1500))
                    .unwrap()
                    .id
            } else {
                store.create_task("Existing", 1, view.now).unwrap().id
            };
            view.refresh(cx);
            view.task_input
                .update(cx, |input, cx| input.set_value("Discard me", window, cx));
            view.timer_input
                .update(cx, |input, cx| input.set_value("37", window, cx));
            id
        });
        manager.cx.run_until_parked();
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            if session_destination {
                view.select_session(id, window, cx)
            } else {
                view.select_task(id, window, cx)
            }
        });
        assert!(manager.cx.has_pending_prompt());
        manager.cx.simulate_prompt_answer("不保存");
        manager.cx.run_until_parked();
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.new_task(window, cx)
        });
        manager.cx.run_until_parked();
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.is_new_task_form());
            assert!(view.task_input.read(cx).value().is_empty());
            assert_eq!(view.timer_input.read(cx).value(), "25");
            assert!(!view.tasks.iter().any(|task| task.title == "Discard me"));
        });
    }
}

#[gpui::test]
fn discarding_edits_to_another_task_keeps_the_suspended_draft(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let (first, second) = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let store = view.store.as_mut().unwrap();
        let first = store.create_task("First", 1, view.now).unwrap();
        let second = store.create_task("Second", 1, view.now).unwrap();
        view.refresh(cx);
        view.timer_input
            .update(cx, |input, cx| input.set_value("0", window, cx));
        (first.id, second.id)
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_task(first, window, cx);
        view.task_input.update(cx, |input, cx| {
            input.set_value("Temporary edit", window, cx)
        });
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_task(second, window, cx)
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("不保存");
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.new_task(window, cx)
    });
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.is_new_task_form());
        assert_eq!(view.timer_input.read(cx).value(), "0");
        assert!(view.tasks.iter().any(|task| task.title == "First"));
        assert!(!view.tasks.iter().any(|task| task.title == "Temporary edit"));
    });
}

#[gpui::test]
fn restored_draft_is_consumed_after_opening_and_starting_it(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let task_id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Existing", 1, view.now)
            .unwrap();
        view.refresh(cx);
        view.timer_input
            .update(cx, |input, cx| input.set_value("0", window, cx));
        task.id
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_task(task_id, window, cx);
        view.new_task(window, cx);
        view.hide_manager(window, cx);
        view.show_strip_task(window, cx);
        assert_eq!(view.timer_input.read(cx).value(), "0");
        view.task_input
            .update(cx, |input, cx| input.set_value("Now start", window, cx));
        view.timer_input
            .update(cx, |input, cx| input.set_value("45", window, cx));
    });
    manager.cx.run_until_parked();
    manager
        .view
        .update_in(&mut manager.cx, |view, window, cx| view.start(window, cx));
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.new_task(window, cx)
    });
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.is_new_task_form());
        assert_eq!(view.timer_input.read(cx).value(), "25");
        assert_eq!(view.active.len(), 1);
        assert_eq!(view.active[0].title, "Now start");
    });
}

#[gpui::test]
fn untitled_draft_survives_starting_a_break(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.timer_input
            .update(cx, |input, cx| input.set_value("42", window, cx));
        view.countdown = false;
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.start_global_break(window, cx);
        assert!(
            view.active
                .iter()
                .any(|session| session.tag_kind == force_core::TagKind::Rest)
        );
        view.new_task(window, cx);
    });
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.is_new_task_form());
        assert!(!view.countdown);
        assert_eq!(view.timer_input.read(cx).value(), "42");
    });
}

#[gpui::test]
fn untitled_draft_survives_starting_a_calendar_arrangement(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let plan_id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let draft = force_core::PlanDraft {
            task_id: None,
            title: "Scheduled".to_owned(),
            tag_id: 1,
            day: chrono::Local::now().date_naive(),
            start_minute: 540,
            end_minute: 570,
            note: String::new(),
        };
        let plan = view
            .store
            .as_mut()
            .unwrap()
            .create_plan(&draft, view.now)
            .unwrap();
        view.refresh(cx);
        view.timer_input
            .update(cx, |input, cx| input.set_value("0", window, cx));
        plan.id
    });
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.start_plan_timer(plan_id, window, cx);
        assert!(
            view.active
                .iter()
                .any(|session| session.title == "Scheduled")
        );
        view.new_task(window, cx);
    });
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.is_new_task_form());
        assert_eq!(view.timer_input.read(cx).value(), "0");
    });
}
