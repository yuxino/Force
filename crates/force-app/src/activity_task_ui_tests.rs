//! Activity-first task setup, using only an isolated temporary database.

use gpui::TestAppContext;

use crate::{headless_ui_tests::ManagerHarness, model::Page};

fn settle(manager: &mut ManagerHarness) {
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
}

#[gpui::test]
fn activity_task_requires_explicit_selection_even_when_a_description_is_written(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new(cx, width, height);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert!(!view.countdown);
            assert_eq!(view.selected_tag, 0);
            assert!(!view.new_task_has_draft(cx));
            view.refresh(cx);
            view.new_task(window, cx);
            assert_eq!(view.selected_tag, 0);
            assert!(!view.can_start_task(cx));
            assert!(!view.can_save_task(cx));
            view.task_input
                .update(cx, |input, cx| input.set_value("复习第四章", window, cx));
            assert!(view.task_title_error(cx).is_none());
            assert!(view.task_tag_error().is_some());
            view.start(window, cx);
            assert!(!view.save_task_changes(cx));
            assert!(view.tasks.is_empty());
            assert!(view.active.is_empty());
        });
        settle(&mut manager);
        manager.click("focus-tag-picker");
        manager.click("focus-tag-option-2");
        manager.click("save-new-task");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert_eq!(view.selected_tag, 2);
            assert_eq!(view.tasks.len(), 1);
            assert_eq!(view.tasks[0].title, "复习第四章");
            assert!(!view.task_has_unsaved_changes(cx));
            assert!(view.can_start_task(cx));
        });
        manager.click("start-focus");
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.active.len(), 1);
            assert_eq!(view.active[0].tag_id, 2);
            assert_eq!(view.active[0].title, "复习第四章");
            assert_eq!(view.active[0].target_seconds, None);
        });
    }
}

#[gpui::test]
fn activity_task_can_save_and_start_without_description_and_does_not_resurrect_after_start(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.click("focus-tag-picker");
    manager.click("focus-tag-option-1");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.task_input.read(cx).value().is_empty());
        assert_eq!(view.resolved_task_title(cx), "工作");
        assert!(view.can_start_task(cx));
        assert!(view.can_save_task(cx));
    });
    manager.click("save-new-task");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.tasks[0].title, "工作");
        assert!(view.task_input.read(cx).value().is_empty());
        assert!(!view.task_has_unsaved_changes(cx));
    });
    manager.click("start-focus");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let id = view.active[0].id;
        assert_eq!(view.active[0].target_seconds, None);
        view.toggle_pause(id, cx);
        assert!(
            view.active
                .iter()
                .find(|session| session.id == id)
                .unwrap()
                .is_paused()
        );
        view.toggle_pause(id, cx);
        assert!(
            view.active
                .iter()
                .find(|session| session.id == id)
                .unwrap()
                .is_running()
        );
        view.new_task(window, cx);
        assert_eq!(view.selected_tag, 0);
        assert!(!view.countdown);
        assert!(!view.new_task_has_draft(cx));
        view.selected_tag = 2;
        view.start(window, cx);
        assert_eq!(
            view.active
                .iter()
                .filter(|session| session.is_running())
                .count(),
            1
        );
        assert!(
            view.active
                .iter()
                .any(|session| session.title == "学习" && session.target_seconds.is_none())
        );
        view.new_task(window, cx);
        assert_eq!(
            view.selected_tag, 0,
            "starting an activity must consume its draft"
        );
        assert!(!view.countdown);
        assert!(view.task_input.read(cx).value().is_empty());
    });
    assert!(!manager.cx.has_pending_prompt());
}

#[gpui::test]
fn activity_task_old_titles_and_undo_compare_the_resolved_title(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let (same, distinct) = manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let store = view.store.as_mut().unwrap();
        let same = store.create_task("工作", 1, view.now).unwrap();
        let distinct = store.create_task("旧项目的独立标题", 1, view.now).unwrap();
        view.refresh(cx);
        (same.id, distinct.id)
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_task(same, window, cx);
        assert!(view.task_input.read(cx).value().is_empty());
        assert_eq!(view.resolved_task_title(cx), "工作");
        assert!(!view.task_has_unsaved_changes(cx));
        view.selected_tag = 2;
        assert!(view.task_has_unsaved_changes(cx));
        assert!(!view.can_start_task(cx));
        view.reset_task_changes(window, cx);
        assert_eq!(view.selected_tag, 1);
        assert!(view.task_input.read(cx).value().is_empty());
        assert!(!view.task_has_unsaved_changes(cx));
        view.select_task(distinct, window, cx);
        assert_eq!(view.task_input.read(cx).value(), "旧项目的独立标题");
        assert!(!view.task_has_unsaved_changes(cx));
        view.task_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        assert_eq!(view.resolved_task_title(cx), "工作");
        assert!(view.task_has_unsaved_changes(cx));
        view.reset_task_changes(window, cx);
        assert_eq!(view.task_input.read(cx).value(), "旧项目的独立标题");
        assert!(!view.task_has_unsaved_changes(cx));
        view.task_input.update(cx, |input, cx| {
            input.set_value("字".repeat(201), window, cx)
        });
        assert!(view.task_title_error(cx).is_some());
        assert!(!view.can_save_task(cx));
        view.reset_task_changes(window, cx);
    });
    assert!(!manager.cx.has_pending_prompt());
}

#[gpui::test]
fn activity_task_countdown_and_selection_survive_navigation_close_and_switch(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let session_id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let store = view.store.as_mut().unwrap();
        let session = store
            .create_and_start_task("原倒计时", 1, view.now, Some(1500))
            .unwrap();
        store.pause(session.id, view.now).unwrap();
        view.refresh(cx);
        view.selected_tag = 2;
        view.choose_timer_preset(37, window, cx);
        view.navigate(Page::Records, window, cx);
        view.new_task(window, cx);
        view.hide_manager(window, cx);
        view.show_strip_task(window, cx);
        assert_eq!(view.selected_tag, 2);
        assert!(view.countdown);
        assert_eq!(view.timer_input.read(cx).value(), "37");
        session.id
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_session(session_id, window, cx);
        assert_eq!(view.selected_tag, 1);
        assert!(view.countdown);
        assert_eq!(view.active[0].target_seconds, Some(1500));
        view.new_task(window, cx);
        assert_eq!(view.selected_tag, 2);
        assert!(view.countdown);
        assert_eq!(view.timer_input.read(cx).value(), "37");
        assert!(view.task_input.read(cx).value().is_empty());
        view.start(window, cx);
        assert!(
            view.active
                .iter()
                .any(|session| session.tag_id == 2 && session.target_seconds == Some(2220))
        );
        view.new_task(window, cx);
        assert_eq!(view.selected_tag, 0);
        assert!(!view.countdown);
    });
    assert!(!manager.cx.has_pending_prompt());
}

#[gpui::test]
fn activity_task_description_without_selection_is_protected_by_switch_prompt(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let existing = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Existing", 1, view.now)
            .unwrap();
        view.refresh(cx);
        view.task_input.update(cx, |input, cx| {
            input.set_value("先写说明，稍后选活动", window, cx)
        });
        view.choose_timer_preset(37, window, cx);
        view.select_task(task.id, window, cx);
        task.id
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.selected_tag, 0);
        assert!(view.selected_task.is_none());
        assert_eq!(view.task_input.read(cx).value(), "先写说明，稍后选活动");
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_task(existing, window, cx)
    });
    manager.cx.simulate_prompt_answer("保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.selected_task.is_none());
        assert_eq!(view.tasks.len(), 1);
        assert_eq!(view.task_input.read(cx).value(), "先写说明，稍后选活动");
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_task(existing, window, cx)
    });
    manager.cx.simulate_prompt_answer("不保存");
    settle(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert_eq!(view.selected_task, Some(existing));
        view.new_task(window, cx);
        assert_eq!(view.selected_tag, 0);
        assert!(!view.countdown);
        assert!(view.task_input.read(cx).value().is_empty());
    });
}

#[gpui::test]
fn activity_task_rename_keeps_saved_optional_description_clean_until_an_explicit_edit(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.selected_tag = 1;
        assert!(view.save_task_changes(cx));
        let task_id = view.selected_task.unwrap();
        view.store
            .as_mut()
            .unwrap()
            .rename_tag(1, "项目工作")
            .unwrap();
        view.refresh(cx);
        assert!(view.task_input.read(cx).value().is_empty());
        assert_eq!(view.selected_activity_name(), Some("项目工作"));
        assert_eq!(view.resolved_task_title(cx), "工作");
        assert!(!view.task_has_unsaved_changes(cx));
        assert!(!view.can_save_task(cx));
        assert!(view.can_start_task(cx));
        assert_eq!(
            view.tasks
                .iter()
                .find(|task| task.id == task_id)
                .unwrap()
                .title,
            "工作"
        );

        // Switching an empty description resolves to that activity; switching
        // back restores the unchanged title instead of inventing an edit.
        view.selected_tag = 2;
        assert_eq!(view.resolved_task_title(cx), "学习");
        assert!(view.task_has_unsaved_changes(cx));
        assert!(!view.can_start_task(cx));
        view.selected_tag = 1;
        assert!(!view.task_has_unsaved_changes(cx));
        assert_eq!(view.resolved_task_title(cx), "工作");

        view.task_input
            .update(cx, |input, cx| input.set_value("项目说明", window, cx));
        assert!(view.save_task_changes(cx));
        assert!(!view.task_has_unsaved_changes(cx));
        view.task_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        assert_eq!(view.resolved_task_title(cx), "项目工作");
        assert!(view.task_has_unsaved_changes(cx));
        view.reset_task_changes(window, cx);
        assert_eq!(view.task_input.read(cx).value(), "项目说明");
        assert!(!view.task_has_unsaved_changes(cx));

        view.task_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        view.selected_tag = 2;
        assert!(view.save_task_changes(cx));
        assert!(view.task_input.read(cx).value().is_empty());
        assert_eq!(view.resolved_task_title(cx), "学习");
        assert!(!view.task_has_unsaved_changes(cx));
        view.start(window, cx);
        assert_eq!(view.active.len(), 1);
        assert_eq!(view.active[0].task_id, Some(task_id));
        assert_eq!(view.active[0].title, "学习");
        assert_eq!(view.active[0].tag_id, 2);
    });
    assert!(!manager.cx.has_pending_prompt());
}

#[gpui::test]
fn activity_task_historical_title_survives_renamed_activity_and_reopening(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let store = view.store.as_mut().unwrap();
        let task = store.create_task("工作", 1, view.now - 60).unwrap();
        store.complete_task(task.id, view.now - 30).unwrap();
        store.rename_tag(1, "项目工作").unwrap();
        view.refresh(cx);
        view.select_task(task.id, window, cx);
        assert!(view.task_input.read(cx).value().is_empty());
        assert_eq!(view.selected_activity_name(), Some("项目工作"));
        assert_eq!(view.resolved_task_title(cx), "工作");
        assert!(!view.task_has_unsaved_changes(cx));
        assert!(!view.can_save_task(cx));
        view.reset_task_changes(window, cx);
        assert!(view.task_input.read(cx).value().is_empty());
        assert!(!view.task_has_unsaved_changes(cx));

        // Reopening updates the activity snapshot in storage, while this
        // untouched form must keep its original optional-description meaning.
        view.store.as_mut().unwrap().reopen_task(task.id).unwrap();
        view.refresh(cx);
        assert_eq!(
            view.tasks
                .iter()
                .find(|saved| saved.id == task.id)
                .unwrap()
                .tag_name,
            "项目工作"
        );
        assert!(!view.task_has_unsaved_changes(cx));
        assert!(view.can_start_task(cx));
        view.start(window, cx);
        assert_eq!(view.active.len(), 1);
        assert_eq!(view.active[0].task_id, Some(task.id));
        assert_eq!(view.active[0].title, "工作");
        assert_eq!(view.active[0].tag_name, "项目工作");
        assert_eq!(view.active[0].target_seconds, None);
    });
    assert!(!manager.cx.has_pending_prompt());
}
