//! Completed record editing never uses the user's real database.
use chrono::{Days, Local, TimeZone};
use force_core::{RecordIntervalEdit, Session};
use gpui::{Bounds, Focusable as _, Pixels, TestAppContext, point, px, size};

use crate::{
    headless_ui_tests::ManagerHarness,
    model::{Page, day_bounds},
};

fn settle(manager: &mut ManagerHarness) {
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
}

fn seed(manager: &mut ManagerHarness, segments: usize) -> Session {
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let yesterday = Local::now()
            .date_naive()
            .checked_sub_days(Days::new(1))
            .unwrap();
        let start = day_bounds(yesterday).0 + 9 * 3600 + 17;
        let intervals: Vec<_> = (0..segments)
            .map(|index| RecordIntervalEdit {
                start: start + index as i64 * 3600,
                end: start + index as i64 * 3600 + 1200,
            })
            .collect();
        let record = view
            .store
            .as_mut()
            .unwrap()
            .create_completed_record("已有说明", 1, &intervals, view.now)
            .unwrap();
        view.record_date = yesterday;
        view.refresh(cx);
        record
    })
}

fn set_stamp(manager: &mut ManagerHarness, index: usize, end: bool, timestamp: i64) {
    let stamp = Local.timestamp_opt(timestamp, 0).single().unwrap();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let fields = &view.record_editor.intervals[index];
        let date = if end {
            &fields.end_date
        } else {
            &fields.start_date
        };
        let time = if end {
            &fields.end_time
        } else {
            &fields.start_time
        };
        date.update(cx, |input, cx| {
            input.set_value(stamp.format("%Y-%m-%d").to_string(), window, cx)
        });
        time.update(cx, |input, cx| {
            input.set_value(stamp.format("%H:%M:%S").to_string(), window, cx)
        });
    });
    settle(manager);
}

#[gpui::test]
fn record_editor_backfill_creates_completed_work_at_the_chosen_past_time(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Records);
    let yesterday = Local::now()
        .date_naive()
        .checked_sub_days(Days::new(1))
        .unwrap();
    let start = day_bounds(yesterday).0 + 12 * 3600 + 19;
    let end = start + 1800;
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.record_date = yesterday;
        view.begin_record_entry(window, cx);
        assert_eq!(view.record_editor.activity, 0);
        assert!(!view.save_record_editor(window, cx));
        view.record_editor.activity = 2;
    });
    set_stamp(&mut manager, 0, false, start);
    set_stamp(&mut manager, 0, true, end);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(view.save_record_editor(window, cx));
        assert!(view.active.is_empty());
        assert_eq!(view.history.len(), 1);
        assert_eq!(view.history[0].title, "学习");
        assert_eq!(view.history[0].started_at, start);
        assert_eq!(view.history[0].finished_at, Some(end));
        assert_eq!(view.tasks[0].completed_at, Some(end));
        assert_eq!(view.record_stats.focus_seconds, 1800);
        assert!(!view.record_editor.visible);
    });
}

#[gpui::test]
fn record_editor_metadata_preserves_seconds_pauses_and_archived_activity(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Records);
    let record = seed(&mut manager, 2);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.store
            .as_mut()
            .unwrap()
            .delete_tag(record.tag_id)
            .unwrap();
        view.refresh(cx);
        view.edit_record(record.clone(), window, cx);
        assert_eq!(view.record_editor.intervals.len(), 2);
        view.record_editor.description.update(cx, |input, cx| {
            input.set_value("只改说明，保留暂停", window, cx)
        });
        assert!(view.save_record_editor(window, cx));
        let saved = view
            .store
            .as_ref()
            .unwrap()
            .record_edit_snapshot(record.id)
            .unwrap()
            .session;
        assert_eq!(saved.intervals, record.intervals);
        assert_eq!(saved.started_at, record.started_at);
        assert_eq!(saved.finished_at, record.finished_at);
        assert_eq!(saved.tag_id, record.tag_id);
        assert_eq!(saved.tag_class, record.tag_class);
        assert_eq!(saved.paused_at(view.now), record.paused_at(view.now));
        assert_eq!(saved.title, "只改说明，保留暂停");
    });
}

#[gpui::test]
fn record_editor_rejects_future_overlap_and_stale_updates_without_losing_draft(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Records);
    let record = seed(&mut manager, 2);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_record(record.clone(), window, cx)
    });
    set_stamp(&mut manager, 1, true, Local::now().timestamp() + 3600);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(!view.save_record_editor(window, cx));
        assert!(view.record_editor.error.as_ref().unwrap().contains("未来"));
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
    set_stamp(&mut manager, 1, true, record.intervals[1].end.unwrap());
    set_stamp(&mut manager, 1, false, record.intervals[0].start + 1);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(!view.save_record_editor(window, cx));
        assert!(view.record_editor.error.as_ref().unwrap().contains("重叠"));
    });
    set_stamp(&mut manager, 1, false, record.intervals[1].start);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let snapshot = view
            .store
            .as_ref()
            .unwrap()
            .record_edit_snapshot(record.id)
            .unwrap();
        let periods: Vec<_> = record
            .intervals
            .iter()
            .map(|interval| RecordIntervalEdit {
                start: interval.start,
                end: interval.end.unwrap(),
            })
            .collect();
        view.store
            .as_mut()
            .unwrap()
            .update_completed_record(&snapshot, "另一处修改", record.tag_id, &periods, view.now)
            .unwrap();
        view.record_editor
            .description
            .update(cx, |input, cx| input.set_value("保留我的草稿", window, cx));
        assert!(!view.save_record_editor(window, cx));
        assert!(view.record_editor.active && view.record_editor.visible);
        assert_eq!(
            view.record_editor.description.read(cx).value(),
            "保留我的草稿"
        );
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .record_edit_snapshot(record.id)
                .unwrap()
                .session
                .title,
            "另一处修改"
        );
        view.cancel_record_editor(window, cx);
        let running = view
            .store
            .as_mut()
            .unwrap()
            .start("Running", 1, view.now)
            .unwrap();
        view.edit_record(running, window, cx);
        assert!(!view.record_editor.visible);
    });
}

#[gpui::test]
fn record_editor_close_navigation_and_other_editors_suspend_without_saving(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Records);
    let record = seed(&mut manager, 1);
    let other = seed(&mut manager, 1);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_record(record.clone(), window, cx);
        view.record_editor
            .description
            .update(cx, |input, cx| input.set_value("尚未保存", window, cx));
        view.close_panel(window, cx);
        assert!(!view.record_editor.visible && view.record_editor.active);
        view.begin_record_entry(window, cx);
        assert_eq!(view.record_editor.description.read(cx).value(), "尚未保存");
        view.navigate(Page::Analysis, window, cx);
        assert!(!view.record_editor.visible && view.record_editor.active);
        view.begin_record_entry(window, cx);
        view.hide_manager(window, cx);
        view.show_manager(window, cx);
        assert!(
            view.record_editor
                .description
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        view.begin_moment(None, window, cx);
        assert!(!view.record_editor.visible && view.moments.visible);
        view.begin_record_entry(window, cx);
        assert!(view.record_editor.visible && !view.moments.visible);
        view.edit_record(other.clone(), window, cx);
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.record_editor.original.as_ref().unwrap().id, record.id);
        assert_eq!(view.record_editor.description.read(cx).value(), "尚未保存");
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.edit_record(other.clone(), window, cx)
    });
    manager.cx.simulate_prompt_answer("保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.record_editor.original.as_ref().unwrap().id, other.id);
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .record_edit_snapshot(record.id)
                .unwrap()
                .session
                .title,
            "尚未保存"
        );
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.record_editor
            .description
            .update(cx, |input, cx| input.set_value("丢弃此修改", window, cx));
        view.edit_record(record.clone(), window, cx);
    });
    manager.cx.simulate_prompt_answer("不保存");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .record_edit_snapshot(other.id)
                .unwrap()
                .session
                .title,
            other.title
        );
    });
}

fn inside(child: Bounds<Pixels>, outer: Bounds<Pixels>) {
    assert!(
        child.left() >= outer.left() - px(1.) && child.right() <= outer.right() + px(1.),
        "{child:?} outside {outer:?}"
    );
    assert!(
        child.top() >= outer.top() - px(1.) && child.bottom() <= outer.bottom() + px(1.),
        "{child:?} outside {outer:?}"
    );
}

#[gpui::test]
fn record_editor_multi_period_layout_and_error_actions_fit_both_window_sizes(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Records);
        let record = seed(&mut manager, 3);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.edit_record(record.clone(), window, cx);
        });
        settle(&mut manager);
        set_stamp(&mut manager, 1, false, record.intervals[0].start + 1);
        manager.click("save-record-editor");
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.record_editor.visible);
            assert!(view.record_editor.error.as_ref().unwrap().contains("重叠"));
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
        let viewport = Bounds::new(point(px(0.), px(0.)), size(px(width), px(height)));
        let editor = manager.bounds("record-editor");
        inside(editor, viewport);
        for selector in [
            "record-editor-activity",
            "record-editor-description",
            "save-record-editor",
            "cancel-record-editor",
            "close-record-editor",
            "record-editor-error",
        ] {
            inside(manager.bounds(selector), editor);
        }
        assert!(manager.bounds("record-editor-activity").size.height >= px(38.));
    }
}

#[gpui::test]
fn record_editor_shortcut_updates_completion_receipt_and_does_not_save_on_return(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Records);
    let record = seed(&mut manager, 1);
    let handle = manager.handle;
    manager.cx.update(|_, cx| {
        cx.bind_keys([gpui::KeyBinding::new(
            "secondary-enter",
            crate::SaveMoment,
            None,
        )]);
        cx.on_action(move |_: &crate::SaveMoment, cx| {
            crate::with_force(handle, cx, |view, window, cx| {
                view.save_record_editor(window, cx);
            });
        });
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.completed = Some(record.clone());
        view.edit_record(record.clone(), window, cx);
    });
    settle(&mut manager);
    manager.cx.simulate_keystrokes("secondary-a");
    manager.cx.simulate_input("完成后的修正说明");
    manager.cx.simulate_keystrokes("enter");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.record_editor.visible);
        assert_eq!(
            view.record_editor.description.read(cx).value(),
            "完成后的修正说明"
        );
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .record_edit_snapshot(record.id)
                .unwrap()
                .session
                .title,
            record.title
        );
    });
    manager.cx.simulate_keystrokes("secondary-enter");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.record_editor.visible);
        assert_eq!(view.completed.as_ref().unwrap().title, "完成后的修正说明");
        assert_eq!(view.completed.as_ref().unwrap().intervals, record.intervals);
    });
    manager.cx.simulate_keystrokes("secondary-enter");
    settle(&mut manager);
    manager
        .view
        .read_with(&manager.cx, |view, _| assert_eq!(view.history.len(), 1));
}

#[gpui::test]
fn record_editor_existing_zero_second_round_can_keep_its_time_while_changing_description(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Records);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let at = view.now - 100;
        let store = view.store.as_mut().unwrap();
        let running = store.start("Zero", 1, at).unwrap();
        let original = store.finish(running.id, at).unwrap();
        view.refresh(cx);
        view.edit_record(original.clone(), window, cx);
        view.record_editor
            .description
            .update(cx, |input, cx| input.set_value("补上说明", window, cx));
        assert!(view.save_record_editor(window, cx));
        let saved = view
            .store
            .as_ref()
            .unwrap()
            .record_edit_snapshot(original.id)
            .unwrap()
            .session;
        assert_eq!(saved.intervals, original.intervals);
        assert_eq!(saved.elapsed_at(view.now), 0);
        assert_eq!(saved.title, "补上说明");
    });
}
