//! Completion markers are edited in isolated data, independently of timer work.
use chrono::{Days, Local};
use force_core::{PlanDraft, RecordIntervalEdit, Session};
use gpui::{Focusable as _, TestAppContext, px};

use crate::{
    headless_ui_tests::ManagerHarness,
    model::{Page, day_bounds},
};

fn settle(manager: &mut ManagerHarness) {
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
}

fn seed(manager: &mut ManagerHarness) -> Session {
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let yesterday = Local::now()
            .date_naive()
            .checked_sub_days(Days::new(1))
            .unwrap();
        let start = day_bounds(yesterday).0 + 9 * 3600 + 17;
        let record = view
            .store
            .as_mut()
            .unwrap()
            .create_completed_record(
                "课程内容",
                1,
                &[
                    RecordIntervalEdit {
                        start,
                        end: start + 600,
                    },
                    RecordIntervalEdit {
                        start: start + 900,
                        end: start + 1500,
                    },
                ],
                view.now,
            )
            .unwrap();
        view.refresh(cx);
        record
    })
}

#[gpui::test]
fn completion_editor_metadata_preserves_exact_completion_seconds_and_timer_history(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
    let record = seed(&mut manager);
    let task_id = record.task_id.unwrap();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let running = view
            .store
            .as_mut()
            .unwrap()
            .start("正在进行", 2, view.now - 30)
            .unwrap();
        view.refresh(cx);
        view.edit_completion(task_id, window, cx);
        view.completion_editor.activity = 2;
        view.completion_editor
            .description
            .update(cx, |input, cx| input.set_value("新的事项说明", window, cx));
        assert!(view.save_completion_editor(window, cx));
        let saved = view
            .store
            .as_ref()
            .unwrap()
            .completed_task_edit_snapshot(task_id)
            .unwrap();
        assert_eq!(saved.task.title, "新的事项说明");
        assert_eq!(saved.task.tag_id, 2);
        assert_eq!(saved.task.completed_at, record.finished_at);
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .record_edit_snapshot(record.id)
                .unwrap()
                .session,
            record
        );
        assert_eq!(
            view.store.as_ref().unwrap().active_sessions().unwrap(),
            vec![running]
        );
        assert!(!view.completion_editor.visible);
    });
}

#[gpui::test]
fn completion_editor_rejects_invalid_time_and_stale_snapshot_while_retaining_fields(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
    let record = seed(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let id = record.task_id.unwrap();
        view.edit_completion(id, window, cx);
        view.completion_editor
            .date
            .update(cx, |input, cx| input.set_value("2099-01-01", window, cx));
        assert!(!view.save_completion_editor(window, cx));
        assert!(
            view.completion_editor
                .error
                .as_ref()
                .unwrap()
                .contains("未来")
        );
        view.cancel_completion_editor(window, cx);
        view.edit_completion(id, window, cx);
        view.completion_editor
            .description
            .update(cx, |input, cx| input.set_value("保留草稿", window, cx));
        let store = view.store.as_mut().unwrap();
        let current = store.completed_task_edit_snapshot(id).unwrap();
        store
            .update_completed_task(
                &current,
                "外部修改",
                2,
                record.finished_at.unwrap(),
                view.now,
            )
            .unwrap();
        assert!(!view.save_completion_editor(window, cx));
        assert!(
            view.completion_editor
                .error
                .as_ref()
                .unwrap()
                .contains("别处修改")
        );
        assert_eq!(
            view.completion_editor.description.read(cx).value(),
            "保留草稿"
        );
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .completed_task_edit_snapshot(id)
                .unwrap()
                .task
                .title,
            "外部修改"
        );
    });
}

#[gpui::test]
fn completion_editor_preserves_other_drafts_and_requires_a_choice_before_switching(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
    let record = seed(&mut manager);
    let other = seed(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_record(record.clone(), window, cx);
        view.record_editor
            .description
            .update(cx, |input, cx| input.set_value("记录草稿", window, cx));
        view.edit_completion(record.task_id.unwrap(), window, cx);
        assert!(view.record_editor.active && !view.record_editor.visible);
        assert_eq!(view.record_editor.description.read(cx).value(), "记录草稿");
        view.completion_editor
            .description
            .update(cx, |input, cx| input.set_value("事项草稿", window, cx));
        view.completion_editor
            .date
            .read(cx)
            .focus_handle(cx)
            .focus(window);
        view.edit_completion(other.task_id.unwrap(), window, cx);
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert_eq!(
            view.completion_editor.original.as_ref().unwrap().id,
            record.task_id.unwrap()
        );
        assert_eq!(
            view.completion_editor.description.read(cx).value(),
            "事项草稿"
        );
        assert!(
            view.completion_editor
                .date
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        view.suspend_completion_editor(window, cx);
        view.edit_completion(other.task_id.unwrap(), window, cx);
    });
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(
            view.completion_editor
                .description
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert_eq!(
            view.completion_editor.description.read(cx).value(),
            "事项草稿"
        );
        view.edit_completion(other.task_id.unwrap(), window, cx);
    });
    manager.cx.simulate_prompt_answer("保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(
            view.completion_editor.original.as_ref().unwrap().id,
            other.task_id.unwrap()
        );
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .completed_task_edit_snapshot(record.task_id.unwrap())
                .unwrap()
                .task
                .title,
            "事项草稿"
        );
        assert_eq!(view.record_editor.description.read(cx).value(), "记录草稿");
    });
}

#[gpui::test]
fn completion_editor_delete_cancel_restores_field_and_confirm_retains_timing_history(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
    let record = seed(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_completion(record.task_id.unwrap(), window, cx);
        view.completion_editor
            .time
            .read(cx)
            .focus_handle(cx)
            .focus(window);
        view.delete_completion(record.task_id.unwrap(), window, cx);
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(
            view.completion_editor
                .time
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        view.delete_completion(record.task_id.unwrap(), window, cx);
    });
    manager.cx.simulate_prompt_answer("删除完成事项");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.completion_editor.active);
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .completed_task_edit_snapshot(record.task_id.unwrap())
                .is_err()
        );
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .record_edit_snapshot(record.id)
                .unwrap()
                .session,
            record
        );
    });
}

#[gpui::test]
fn completion_editor_controls_remain_inside_default_and_minimum_windows(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        let record = seed(&mut manager);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.edit_completion(record.task_id.unwrap(), window, cx);
        });
        settle(&mut manager);
        manager.view.update(&mut manager.cx, |view, cx| {
            view.completion_editor.error =
                Some("事项或关联记录已在别处修改。草稿已保留，请取消后重新打开。".into());
            cx.notify();
        });
        settle(&mut manager);
        let panel = manager.cx.debug_bounds("completion-editor").unwrap();
        assert!(panel.left() >= px(0.) && panel.right() <= px(width));
        assert!(panel.top() >= px(0.) && panel.bottom() <= px(height));
        for name in [
            "completion-editor-activity",
            "completion-editor-description",
            "completion-editor-date",
            "completion-editor-time",
            "save-completion-editor",
            "cancel-completion-editor",
            "delete-completion-editor",
            "completion-editor-error",
        ] {
            let bounds = manager
                .cx
                .debug_bounds(name)
                .unwrap_or_else(|| panic!("missing rendered selector: {name}"));
            assert!(
                bounds.left() >= panel.left() && bounds.right() <= panel.right(),
                "{name}: {bounds:?}"
            );
            assert!(
                bounds.top() >= panel.top() && bounds.bottom() <= panel.bottom(),
                "{name}: {bounds:?}"
            );
            assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
        }
    }
}

#[gpui::test]
fn completion_editor_keyboard_dismissal_and_save_keep_the_same_draft(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
    let record = seed(&mut manager);
    let handle = manager.handle;
    manager.cx.update(|_, cx| {
        cx.bind_keys([
            gpui::KeyBinding::new("secondary-enter", crate::SaveMoment, None),
            gpui::KeyBinding::new("escape", crate::ClosePanel, Some("Force")),
        ]);
        cx.on_action(move |_: &crate::SaveMoment, cx| {
            crate::with_force(handle, cx, |view, window, cx| {
                view.save_completion_editor(window, cx);
            });
        });
        cx.on_action(move |_: &crate::ClosePanel, cx| {
            crate::with_force(handle, cx, |view, window, cx| view.close_panel(window, cx));
        });
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_completion(record.task_id.unwrap(), window, cx);
    });
    settle(&mut manager);
    manager.cx.simulate_keystrokes("secondary-a");
    manager.cx.simulate_input("Keyboard draft");
    manager.cx.simulate_keystrokes("enter");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.completion_editor.visible);
        assert_eq!(
            view.completion_editor.description.read(cx).value(),
            "Keyboard draft"
        );
    });
    manager.cx.simulate_keystrokes("escape");
    settle(&mut manager);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(!view.completion_editor.visible && view.completion_editor.active);
        view.edit_completion(record.task_id.unwrap(), window, cx);
        view.delete_completion(record.task_id.unwrap(), window, cx);
    });
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.cx.simulate_keystrokes("secondary-a");
    manager.cx.simulate_input("After cancel");
    manager.cx.simulate_keystrokes("secondary-enter");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.completion_editor.visible);
        let saved = view
            .store
            .as_ref()
            .unwrap()
            .completed_task_edit_snapshot(record.task_id.unwrap())
            .unwrap();
        assert_eq!(saved.task.title, "After cancel");
        assert_eq!(saved.task.completed_at, record.finished_at);
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .record_edit_snapshot(record.id)
                .unwrap()
                .session,
            record
        );
    });
}

#[gpui::test]
fn moved_completion_follows_the_visible_calendar_item_without_switching_month_or_day_mode(
    cx: &mut TestAppContext,
) {
    let today = Local::now().date_naive();
    let old_day = today.checked_sub_days(Days::new(1)).unwrap();
    let new_day = today.checked_sub_days(Days::new(40)).unwrap();
    let new_start = day_bounds(new_day).0;
    for month_overview in [true, false] {
        for representation in ["completion", "plan", "record"] {
            let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Agenda);
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                let store = view.store.as_mut().unwrap();
                let task = store
                    .create_task("完成事项", 2, new_start + 8 * 3600)
                    .unwrap();
                let plan = if representation == "plan" {
                    Some(
                        store
                            .create_plan(
                                &PlanDraft {
                                    task_id: Some(task.id),
                                    title: "安排标题".into(),
                                    tag_id: 2,
                                    day: new_day,
                                    start_minute: 540,
                                    end_minute: 600,
                                    note: String::new(),
                                },
                                new_start + 8 * 3600,
                            )
                            .unwrap(),
                    )
                } else {
                    None
                };
                let record = if representation == "record" {
                    let round = store
                        .start_task(task.id, new_start + 9 * 3600, None)
                        .unwrap();
                    Some(store.finish(round.id, new_start + 10 * 3600).unwrap())
                } else {
                    None
                };
                store
                    .complete_task(task.id, day_bounds(old_day).0 + 12 * 3600 + 17)
                    .unwrap();
                view.refresh(cx);
                view.agenda.month_overview = month_overview;
                view.select_agenda_date(old_day, window, cx);
                view.begin_moment(None, window, cx);
                view.moments
                    .content
                    .update(cx, |input, cx| input.set_value("独立随记草稿", window, cx));
                view.edit_completion(task.id, window, cx);
                view.completion_editor.date.update(cx, |input, cx| {
                    input.set_value(new_day.format("%Y-%m-%d").to_string(), window, cx)
                });
                view.completion_editor
                    .time
                    .update(cx, |input, cx| input.set_value("12:00:17", window, cx));
                assert!(view.save_completion_editor(window, cx));
                assert_eq!(view.page, Page::Agenda);
                assert_eq!(view.agenda.date, new_day);
                assert_eq!(view.agenda.month_overview, month_overview);
                assert_eq!(view.agenda.popup_open, month_overview);
                assert!(!view.completion_editor.visible);
                assert!(view.moments.active && !view.moments.visible);
                assert_eq!(view.moments.content.read(cx).value(), "独立随记草稿");
                assert_eq!(
                    view.agenda.selected_completion,
                    (representation == "completion").then_some(task.id)
                );
                assert_eq!(view.agenda.selected_plan, plan.as_ref().map(|plan| plan.id));
                assert_eq!(
                    view.agenda.selected_record,
                    record.as_ref().map(|record| record.id)
                );
                if let Some(plan) = plan {
                    assert_eq!(
                        view.store.as_ref().unwrap().plans_for_day(new_day).unwrap(),
                        vec![plan]
                    );
                }
                if let Some(record) = record {
                    assert_eq!(
                        view.store
                            .as_ref()
                            .unwrap()
                            .record_edit_snapshot(record.id)
                            .unwrap()
                            .session,
                        record
                    );
                }
                assert!(view.active.is_empty());
            });
        }
    }
}

#[gpui::test]
fn completion_save_keeps_browsing_position_without_an_agenda_date_move(cx: &mut TestAppContext) {
    let today = Local::now().date_naive();
    let old_day = today.checked_sub_days(Days::new(1)).unwrap();
    let new_day = today.checked_sub_days(Days::new(3)).unwrap();
    let browse_day = today.checked_sub_days(Days::new(5)).unwrap();
    for page in [Page::Agenda, Page::Records] {
        let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., page);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let store = view.store.as_mut().unwrap();
            let task = store
                .create_task("独立事项", 2, day_bounds(new_day).0)
                .unwrap();
            store
                .complete_task(task.id, day_bounds(old_day).0 + 12 * 3600)
                .unwrap();
            view.refresh(cx);
            view.select_agenda_date(browse_day, window, cx);
            view.edit_completion(task.id, window, cx);
            if page == Page::Records {
                view.completion_editor.date.update(cx, |input, cx| {
                    input.set_value(new_day.format("%Y-%m-%d").to_string(), window, cx)
                });
            } else {
                view.completion_editor
                    .time
                    .update(cx, |input, cx| input.set_value("13:00", window, cx));
            }
            assert!(view.save_completion_editor(window, cx));
            assert_eq!(view.page, page);
            assert_eq!(view.agenda.date, browse_day);
        });
    }
}
