use super::*;
use chrono::NaiveDate;

fn database() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    (directory, store)
}

fn sample(store: &mut Store) -> CompletedTaskEditSnapshot {
    let task = store.create_task("课程内容", 1, 100).unwrap();
    let round = store.start_task(task.id, 120, None).unwrap();
    store.pause(round.id, 150).unwrap();
    store.start_task(task.id, 180, None).unwrap();
    store.finish(round.id, 220).unwrap();
    store.complete_task(task.id, 240).unwrap();
    store.completed_task_edit_snapshot(task.id).unwrap()
}

#[test]
fn completion_edit_keeps_work_intervals_statistics_and_running_timer_unchanged() {
    let (_directory, mut store) = database();
    let original = sample(&mut store);
    let running = store.start("Elsewhere", 2, 250).unwrap();
    let before_stats = store.stats(0, 1000, 1000).unwrap();
    let saved = store
        .update_completed_task(&original, "更新说明", 2, 300, 1000)
        .unwrap();
    assert_eq!(saved.title, "更新说明");
    assert_eq!(saved.tag_id, 2);
    assert_eq!(saved.completed_at, Some(300));
    assert_eq!(saved.created_at, original.task.created_at);
    let after = store.completed_task_edit_snapshot(saved.id).unwrap();
    assert_eq!(after.baseline.sessions, original.baseline.sessions);
    assert_eq!(after.baseline.interval_ids, original.baseline.interval_ids);
    assert_eq!(after.baseline.plans, original.baseline.plans);
    assert_eq!(store.active_sessions().unwrap(), vec![running]);
    assert_eq!(store.stats(0, 1000, 1000).unwrap(), before_stats);
}

#[test]
fn completed_untimed_task_and_archived_activity_can_be_edited_without_new_work() {
    let (_directory, mut store) = database();
    let task = store.create_task("旧说明", 2, 100).unwrap();
    store.complete_task(task.id, 150).unwrap();
    store.delete_tag(2).unwrap();
    let original = store.completed_task_edit_snapshot(task.id).unwrap();
    let saved = store
        .update_completed_task(&original, "", 2, 160, 1000)
        .unwrap();
    assert_eq!(saved.title, "学习");
    assert_eq!(saved.tag_name, "学习");
    assert_eq!(saved.total_seconds, 0);
    assert_eq!(saved.session_count, 0);
    assert!(store.sessions().unwrap().is_empty());
}

#[test]
fn completion_time_validation_and_stale_snapshot_never_overwrite_work() {
    let (_directory, mut store) = database();
    let original = sample(&mut store);
    for stamp in [99, 219, 1001, i64::MAX] {
        assert!(
            store
                .update_completed_task(&original, "Bad", 2, stamp, 1000)
                .is_err()
        );
        assert_eq!(
            store
                .completed_task_edit_snapshot(original.task.id)
                .unwrap(),
            original
        );
    }
    let edited = store
        .update_completed_task(&original, "Changed", 2, 260, 1000)
        .unwrap();
    assert!(
        store
            .update_completed_task(&original, "Stale", 1, 280, 1000)
            .is_err()
    );
    assert!(store.delete_completed_task(&original).is_err());
    assert_eq!(
        store.completed_task_edit_snapshot(edited.id).unwrap().task,
        edited
    );
}

#[test]
fn delete_completion_hides_its_plans_and_keeps_actual_history_and_other_tasks() {
    let (_directory, mut store) = database();
    let task = store.create_task("安排事项", 1, 100).unwrap();
    let day = NaiveDate::from_ymd_opt(2026, 10, 28).unwrap();
    let plan = store
        .create_plan(
            &PlanDraft {
                task_id: Some(task.id),
                title: "保留安排标题".into(),
                tag_id: 1,
                day,
                start_minute: 540,
                end_minute: 600,
                note: "安排备注".into(),
            },
            100,
        )
        .unwrap();
    let round = store.start_task(task.id, 120, None).unwrap();
    store.finish(round.id, 180).unwrap();
    store.complete_task(task.id, 200).unwrap();
    let expected = store.completed_task_edit_snapshot(task.id).unwrap();
    let history = store.sessions().unwrap();
    let intervals = expected.baseline.interval_ids.clone();
    let running = store
        .create_and_start_task("仍在进行", 2, 300, None)
        .unwrap();
    store.delete_completed_task(&expected).unwrap();
    assert!(store.completed_task_edit_snapshot(task.id).is_err());
    assert!(
        store
            .plans_for_day(day)
            .unwrap()
            .iter()
            .all(|p| p.id != plan.id)
    );
    assert_eq!(store.sessions().unwrap(), history);
    assert_eq!(store.active_sessions().unwrap(), vec![running]);
    let ids = store
        .connection
        .prepare("SELECT id FROM intervals WHERE session_id=?1 ORDER BY id")
        .unwrap()
        .query_map([round.id], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(ids, intervals);
    assert!(
        store
            .connection
            .query_row("SELECT deleted FROM tasks WHERE id=?1", [task.id], |row| {
                row.get::<_, bool>(0)
            })
            .unwrap()
    );
}

#[test]
fn completion_delete_rolls_back_if_any_plan_write_fails() {
    let (_directory, mut store) = database();
    let original = sample(&mut store);
    store.connection.execute(
        "INSERT INTO agenda_entries(task_id,title,day,start_minute,end_minute,created_at) VALUES (?1,'Plan','2026-10-28',540,600,100)",
        [original.task.id],
    ).unwrap();
    let expected = store
        .completed_task_edit_snapshot(original.task.id)
        .unwrap();
    store.connection.execute_batch("CREATE TRIGGER reject_plan_delete BEFORE UPDATE OF deleted ON agenda_entries BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(store.delete_completed_task(&expected).is_err());
    assert_eq!(
        store
            .completed_task_edit_snapshot(original.task.id)
            .unwrap(),
        expected
    );
    assert!(store.delete_completed_task(&original).is_err());
}

#[test]
fn unfinished_or_reopened_task_cannot_be_edited_as_a_completion() {
    let (_directory, mut store) = database();
    let task = store.create_task("New", 1, 100).unwrap();
    assert!(store.completed_task_edit_snapshot(task.id).is_err());
    store.complete_task(task.id, 200).unwrap();
    let expected = store.completed_task_edit_snapshot(task.id).unwrap();
    store.reopen_task(task.id).unwrap();
    assert!(
        store
            .update_completed_task(&expected, "", 1, 250, 1000)
            .is_err()
    );
    assert!(store.delete_completed_task(&expected).is_err());
}
