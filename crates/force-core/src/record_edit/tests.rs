use super::*;

fn database() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    (directory, store)
}

fn periods(values: &[(i64, i64)]) -> Vec<RecordIntervalEdit> {
    values
        .iter()
        .map(|&(start, end)| RecordIntervalEdit { start, end })
        .collect()
}

fn interval_ids(store: &Store, id: i64) -> Vec<i64> {
    store
        .connection
        .prepare("SELECT id FROM intervals WHERE session_id=?1 ORDER BY started_at,id")
        .unwrap()
        .query_map([id], |row| row.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap()
}

#[test]
fn backfill_preserves_running_work_and_persists_real_completion_and_pauses() {
    let (directory, mut store) = database();
    let running = store.start("Running", 1, 900).unwrap();
    let saved = store
        .create_completed_record("  ", 2, &periods(&[(100, 160), (200, 230)]), 1000)
        .unwrap();
    assert_eq!(saved.title, "学习");
    assert_eq!((saved.started_at, saved.finished_at), (100, Some(230)));
    assert_eq!(saved.elapsed_at(1000), 90);
    assert_eq!(saved.paused_at(1000), 40);
    assert_eq!(store.active_sessions().unwrap(), vec![running.clone()]);
    let task = store
        .tasks(1000)
        .unwrap()
        .into_iter()
        .find(|task| Some(task.id) == saved.task_id)
        .unwrap();
    assert_eq!(
        (
            task.created_at,
            task.completed_at,
            task.total_seconds,
            task.session_count
        ),
        (100, Some(230), 90, 1)
    );
    let stats = store.stats(0, 500, 1000).unwrap();
    assert_eq!(
        (
            stats.focus_seconds,
            stats.task_seconds,
            stats.completed_sessions
        ),
        (90, 90, 1)
    );
    assert_eq!(
        store
            .analyze_period(&[0, 180, 500], 1000)
            .unwrap()
            .days
            .iter()
            .map(|day| day.focus_seconds)
            .collect::<Vec<_>>(),
        vec![60, 30]
    );
    drop(store);
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert_eq!(store.record_edit_snapshot(saved.id).unwrap().session, saved);
    assert_eq!(store.active_sessions().unwrap(), vec![running]);
}

#[test]
fn edits_keep_interval_ids_and_snapshots_while_backdating_a_single_completed_task() {
    let (_directory, mut store) = database();
    let original = store
        .create_completed_record("Original", 1, &periods(&[(500, 550), (600, 650)]), 1000)
        .unwrap();
    let ids = interval_ids(&store, original.id);
    let snapshot = store.record_edit_snapshot(original.id).unwrap();
    assert!(snapshot.task_completion_follows_record());
    store
        .update_tag(1, "Renamed", TagClass::Negative, Some("#B56F67"))
        .unwrap();
    store.delete_tag(1).unwrap();
    let edited = store
        .update_completed_record(
            &snapshot,
            "Past work",
            1,
            &periods(&[(100, 140), (200, 260)]),
            1000,
        )
        .unwrap();
    assert_eq!(interval_ids(&store, original.id), ids);
    assert_eq!(
        (&edited.tag_name, edited.tag_class, &edited.tag_color),
        (&original.tag_name, original.tag_class, &original.tag_color)
    );
    assert_eq!(
        (
            edited.started_at,
            edited.finished_at,
            edited.elapsed_at(1000)
        ),
        (100, Some(260), 100)
    );
    let task = store.tasks(1000).unwrap().pop().unwrap();
    assert_eq!(
        (task.created_at, task.completed_at, task.title),
        (100, Some(260), "Past work".into())
    );
    assert_eq!(store.stats(500, 700, 1000).unwrap().task_seconds, 0);
    assert_eq!(store.stats(0, 500, 1000).unwrap().task_seconds, 100);
}

#[test]
fn changing_activity_updates_the_single_matching_task_but_no_other_records() {
    let (_directory, mut store) = database();
    let original = store
        .create_completed_record("Work", 1, &periods(&[(100, 200)]), 1000)
        .unwrap();
    let other = store
        .create_completed_record("Other", 2, &periods(&[(300, 400)]), 1000)
        .unwrap();
    let tag = store
        .create_tag_with_details("Life", TagKind::Focus, TagClass::Neutral, Some("#688BA5"))
        .unwrap();
    let snapshot = store.record_edit_snapshot(original.id).unwrap();
    let edited = store
        .update_completed_record(&snapshot, "", tag.id, &periods(&[(110, 210)]), 1000)
        .unwrap();
    assert_eq!(
        (
            edited.title.as_str(),
            edited.tag_id,
            edited.tag_class,
            edited.tag_color.as_deref()
        ),
        ("Life", tag.id, TagClass::Neutral, Some("#688BA5"))
    );
    let task = store
        .tasks(1000)
        .unwrap()
        .into_iter()
        .find(|task| Some(task.id) == edited.task_id)
        .unwrap();
    assert_eq!(
        (task.title, task.tag_id, task.tag_name, task.completed_at),
        ("Life".into(), tag.id, "Life".into(), Some(210))
    );
    assert_eq!(store.record_edit_snapshot(other.id).unwrap().session, other);
    assert_eq!(store.stats(0, 250, 1000).unwrap().focus_seconds, 0);
    assert_eq!(store.stats(0, 250, 1000).unwrap().task_seconds, 100);
}

#[test]
fn independent_task_completion_and_multi_round_names_are_preserved() {
    let (_directory, mut store) = database();
    let task = store.create_task("Whole task", 1, 100).unwrap();
    let first = store.start_task(task.id, 100, None).unwrap();
    store.finish(first.id, 200).unwrap();
    let last = store.start_task(task.id, 300, None).unwrap();
    store.complete_task(task.id, 400).unwrap();
    let first_snapshot = store.record_edit_snapshot(first.id).unwrap();
    assert!(!first_snapshot.task_completion_follows_record());
    store
        .update_completed_record(
            &first_snapshot,
            "First corrected",
            2,
            &periods(&[(110, 210)]),
            1000,
        )
        .unwrap();
    let last_snapshot = store.record_edit_snapshot(last.id).unwrap();
    store
        .update_completed_record(
            &last_snapshot,
            "Last corrected",
            2,
            &periods(&[(310, 450)]),
            1000,
        )
        .unwrap();
    let saved = store.tasks(1000).unwrap().pop().unwrap();
    assert_eq!(
        (
            saved.title,
            saved.tag_id,
            saved.created_at,
            saved.completed_at
        ),
        ("Whole task".into(), 1, 100, Some(450))
    );
    let last_snapshot = store.record_edit_snapshot(last.id).unwrap();
    let before = store.sessions().unwrap();
    assert!(
        store
            .update_completed_record(&last_snapshot, "No", 1, &periods(&[(120, 180)]), 1000)
            .is_err()
    );
    assert_eq!(store.sessions().unwrap(), before);

    let standalone = store.create_task("Marked later", 1, 500).unwrap();
    let round = store.start_task(standalone.id, 500, None).unwrap();
    store.finish(round.id, 600).unwrap();
    store.complete_task(standalone.id, 700).unwrap();
    let snapshot = store.record_edit_snapshot(round.id).unwrap();
    assert!(!snapshot.task_completion_follows_record());
    store
        .update_completed_record(&snapshot, "Marked later", 1, &periods(&[(510, 610)]), 1000)
        .unwrap();
    assert_eq!(
        store
            .tasks(1000)
            .unwrap()
            .into_iter()
            .find(|task| task.id == standalone.id)
            .unwrap()
            .completed_at,
        Some(700)
    );
}

#[test]
fn editing_a_finished_round_never_rewrites_its_running_task_or_sibling() {
    let (_directory, mut store) = database();
    let task = store.create_task("Running task", 1, 100).unwrap();
    let first = store.start_task(task.id, 100, None).unwrap();
    store.finish(first.id, 200).unwrap();
    let running = store.start_task(task.id, 500, None).unwrap();
    let snapshot = store.record_edit_snapshot(first.id).unwrap();
    store
        .update_completed_record(&snapshot, "History", 2, &periods(&[(110, 210)]), 1000)
        .unwrap();
    assert_eq!(store.active_sessions().unwrap(), vec![running]);
    let task = store.tasks(1000).unwrap().pop().unwrap();
    assert_eq!(
        (task.title, task.tag_id, task.created_at, task.completed_at),
        ("Running task".into(), 1, 100, None)
    );
}

#[test]
fn stale_session_task_and_interval_identity_are_rejected_without_partial_writes() {
    let (directory, mut store) = database();
    let original = store
        .create_completed_record("Original", 1, &periods(&[(100, 200)]), 1000)
        .unwrap();
    let snapshot = store.record_edit_snapshot(original.id).unwrap();
    let mut other = Store::open(directory.path().join("force.sqlite3")).unwrap();
    other
        .update_completed_record(&snapshot, "Newer", 1, &periods(&[(110, 210)]), 1000)
        .unwrap();
    let before = store.sessions().unwrap();
    assert!(
        store
            .update_completed_record(&snapshot, "Stale", 1, &periods(&[(120, 220)]), 1000)
            .is_err()
    );
    assert_eq!(store.sessions().unwrap(), before);
    let snapshot = store.record_edit_snapshot(original.id).unwrap();
    other.reopen_task(original.task_id.unwrap()).unwrap();
    assert!(
        store
            .update_completed_record(&snapshot, "Stale task", 1, &periods(&[(120, 220)]), 1000)
            .is_err()
    );
    let snapshot = store.record_edit_snapshot(original.id).unwrap();
    other
        .connection
        .execute(
            "UPDATE intervals SET id=id+1000 WHERE session_id=?1",
            [original.id],
        )
        .unwrap();
    assert!(
        store
            .update_completed_record(
                &snapshot,
                "Stale identity",
                1,
                &periods(&[(120, 220)]),
                1000
            )
            .is_err()
    );
    assert_eq!(store.sessions().unwrap(), before);
}

#[test]
fn invalid_periods_and_activity_never_create_or_overwrite_records() {
    let (_directory, mut store) = database();
    let original = store
        .create_completed_record("Original", 1, &periods(&[(100, 200), (300, 400)]), 1000)
        .unwrap();
    let snapshot = store.record_edit_snapshot(original.id).unwrap();
    let tasks = store.tasks(1000).unwrap();
    let sessions = store.sessions().unwrap();
    for invalid in [
        vec![],
        periods(&[(100, 100)]),
        periods(&[(200, 100)]),
        periods(&[(999, 1001)]),
        periods(&[(100, 200), (150, 250)]),
        periods(&[(300, 400), (100, 200)]),
        periods(&[(i64::MIN, 200)]),
        periods(&[(100, i64::MAX)]),
    ] {
        assert!(
            store
                .create_completed_record("No", 1, &invalid, 1000)
                .is_err()
        );
        assert!(
            store
                .update_completed_record(&snapshot, "No", 1, &invalid, 1000)
                .is_err()
        );
    }
    assert!(
        store
            .update_completed_record(&snapshot, "No", 1, &periods(&[(100, 400)]), 1000)
            .is_err()
    );
    for title in ["x".repeat(201), "bad\0text".into()] {
        assert!(
            store
                .create_completed_record(&title, 1, &periods(&[(100, 200)]), 1000)
                .is_err()
        );
        assert!(
            store
                .update_completed_record(
                    &snapshot,
                    &title,
                    1,
                    &periods(&[(100, 200), (300, 400)]),
                    1000
                )
                .is_err()
        );
    }
    store.delete_tag(2).unwrap();
    for tag_id in [2, 9999] {
        assert!(
            store
                .create_completed_record("No", tag_id, &periods(&[(100, 200)]), 1000)
                .is_err()
        );
        assert!(
            store
                .update_completed_record(
                    &snapshot,
                    "No",
                    tag_id,
                    &periods(&[(100, 200), (300, 400)]),
                    1000
                )
                .is_err()
        );
    }
    assert_eq!(store.tasks(1000).unwrap(), tasks);
    assert_eq!(store.sessions().unwrap(), sessions);
}

#[test]
fn running_and_paused_records_are_not_editable_but_original_zero_seconds_are() {
    let (_directory, mut store) = database();
    let active = store.start("Active", 1, 100).unwrap();
    assert!(matches!(
        store.record_edit_snapshot(active.id),
        Err(Error::InvalidTransition)
    ));
    store.pause(active.id, 100).unwrap();
    assert!(matches!(
        store.record_edit_snapshot(active.id),
        Err(Error::InvalidTransition)
    ));
    store.finish(active.id, 100).unwrap();
    let snapshot = store.record_edit_snapshot(active.id).unwrap();
    let saved = store
        .update_completed_record(
            &snapshot,
            "Zero preserved",
            1,
            &periods(&[(100, 100)]),
            1000,
        )
        .unwrap();
    assert_eq!(saved.elapsed_at(1000), 0);
    let snapshot = store.record_edit_snapshot(active.id).unwrap();
    assert!(
        store
            .update_completed_record(&snapshot, "No", 1, &periods(&[(200, 200)]), 1000)
            .is_err()
    );
    assert!(
        store
            .create_completed_record("No", 1, &periods(&[(100, 100)]), 1000)
            .is_err()
    );
}

#[test]
fn legacy_rest_without_a_task_and_import_provenance_survive_edits() {
    let (_directory, mut store) = database();
    let rest = store.start("Rest", 4, 100).unwrap();
    store.finish(rest.id, 200).unwrap();
    store.connection.execute("INSERT INTO imported_sessions (source,source_id,session_id,source_start,source_end,source_duration_seconds,timing_estimated,timing_precision) VALUES ('fixture','one',?1,'original start','original end',100,1,'estimated_from_start')",[rest.id]).unwrap();
    store.delete_tag(4).unwrap();
    let snapshot = store.record_edit_snapshot(rest.id).unwrap();
    let edited = store
        .update_completed_record(
            &snapshot,
            "Rest corrected",
            4,
            &periods(&[(110, 210)]),
            1000,
        )
        .unwrap();
    assert!(edited.timing_estimated && edited.timing_adjusted);
    assert_eq!((edited.task_id, edited.tag_kind), (None, TagKind::Rest));
    assert!(store.tasks(1000).unwrap().is_empty());
    assert_eq!(store.connection.query_row("SELECT source_start,source_end,source_duration_seconds FROM imported_sessions WHERE session_id=?1",[rest.id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?))).unwrap(),("original start".into(),"original end".into(),100));
}

#[test]
fn a_database_failure_rolls_back_every_record_and_task_change() {
    let (_directory, mut store) = database();
    let original = store
        .create_completed_record("Original", 1, &periods(&[(100, 200), (300, 400)]), 1000)
        .unwrap();
    let snapshot = store.record_edit_snapshot(original.id).unwrap();
    let before = store.sessions().unwrap();
    let tasks = store.tasks(1000).unwrap();
    store.connection.execute_batch("CREATE TRIGGER reject_interval_update BEFORE UPDATE ON intervals WHEN NEW.started_at=310 BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
    assert!(
        store
            .update_completed_record(
                &snapshot,
                "No",
                2,
                &periods(&[(110, 210), (310, 410)]),
                1000
            )
            .is_err()
    );
    assert_eq!(store.sessions().unwrap(), before);
    assert_eq!(store.tasks(1000).unwrap(), tasks);
    store.connection.execute_batch("CREATE TRIGGER reject_interval_insert BEFORE INSERT ON intervals WHEN NEW.started_at=310 BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
    assert!(
        store
            .create_completed_record("No", 2, &periods(&[(110, 210), (310, 410)]), 1000)
            .is_err()
    );
    assert_eq!(store.sessions().unwrap(), before);
    assert_eq!(store.tasks(1000).unwrap(), tasks);
}

#[test]
fn metadata_only_edits_preserve_completion_after_a_paused_gap() {
    let (_directory, mut store) = database();
    let task = store.create_task("Original", 1, 100).unwrap();
    let session = store.start_task(task.id, 100, None).unwrap();
    store.pause(session.id, 200).unwrap();
    store.complete_task(task.id, 300).unwrap();
    let snapshot = store.record_edit_snapshot(session.id).unwrap();
    let saved = store
        .update_completed_record(
            &snapshot,
            "Description only",
            2,
            &periods(&[(100, 200)]),
            1000,
        )
        .unwrap();
    assert_eq!((saved.started_at, saved.finished_at), (100, Some(300)));
    let task = store.tasks(1000).unwrap().pop().unwrap();
    assert_eq!(
        (task.title, task.tag_id, task.created_at, task.completed_at),
        ("Description only".into(), 2, 100, Some(300))
    );
    let snapshot = store.record_edit_snapshot(session.id).unwrap();
    let changed = store
        .update_completed_record(&snapshot, "Time changed", 2, &periods(&[(110, 210)]), 1000)
        .unwrap();
    assert_eq!((changed.started_at, changed.finished_at), (110, Some(210)));
    assert_eq!(
        store.tasks(1000).unwrap().pop().unwrap().completed_at,
        Some(210)
    );
}
