use chrono::NaiveDate;

use super::*;

fn database() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    (directory, store)
}

#[test]
fn notes_survive_restart_edit_and_soft_delete_without_changing_timer_statistics() {
    let (directory, mut store) = database();
    let task = store.create_task("Existing task", 1, 100).unwrap();
    let session = store.start_task(task.id, 110, None).unwrap();
    store.finish(session.id, 170).unwrap();
    let stats = store.stats(0, 300, 300).unwrap();
    let tasks = store.tasks(300).unwrap();
    let sessions = store.sessions().unwrap();
    let note = store
        .create_moment("  起床\n今天想早点开始。  ", 80, 200)
        .unwrap();
    assert_eq!(note.content, "起床\n今天想早点开始。");
    assert_eq!(
        (note.occurred_at, note.created_at, note.updated_at),
        (80, 200, 200)
    );
    drop(store);

    let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert_eq!(store.moments_between(0, 300).unwrap(), vec![note.clone()]);
    let updated = store
        .update_moment(note.id, "醒来，记一个想法", 90, 210)
        .unwrap();
    assert_eq!(updated.created_at, 200);
    assert_eq!(updated.updated_at, 210);
    assert_eq!(updated.occurred_at, 90);
    assert_eq!(store.stats(0, 300, 300).unwrap(), stats);
    assert_eq!(store.tasks(300).unwrap(), tasks);
    assert_eq!(store.sessions().unwrap(), sessions);
    store.delete_moment(note.id).unwrap();
    assert!(store.moments_between(0, 300).unwrap().is_empty());
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT deleted FROM moments WHERE id=?1",
                [note.id],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert!(matches!(
        store.update_moment(note.id, "Again", 90, 220),
        Err(Error::NotFound)
    ));
    assert!(matches!(store.delete_moment(note.id), Err(Error::NotFound)));
    drop(store);
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert!(store.moments_between(0, 300).unwrap().is_empty());
    assert_eq!(store.stats(0, 300, 300).unwrap(), stats);
}

#[test]
fn ranges_are_half_open_and_stable_for_equal_times_and_backdated_notes() {
    let (_directory, mut store) = database();
    let later = store.create_moment("Later", 200, 100).unwrap();
    let first = store.create_moment("First", 100, 300).unwrap();
    let same_time = store.create_moment("Same time", 100, 300).unwrap();
    store.create_moment("Before", 99, 300).unwrap();
    assert_eq!(
        store.moments_between(100, 200).unwrap(),
        vec![first.clone(), same_time.clone()]
    );
    assert_eq!(
        store.moments_between(100, 201).unwrap(),
        vec![first, same_time, later]
    );
    assert!(store.moments_between(200, 200).is_err());
    assert!(store.moments_between(300, 100).is_err());
}

#[test]
fn invalid_input_does_not_overwrite_existing_note_or_create_partial_data() {
    let (_directory, mut store) = database();
    let note = store
        .create_moment(&"中".repeat(MOMENT_CONTENT_LIMIT), 100, 100)
        .unwrap();
    for invalid in [
        " \n\t\u{3000}".to_owned(),
        "a\0b".to_owned(),
        "中".repeat(MOMENT_CONTENT_LIMIT + 1),
    ] {
        assert!(store.create_moment(&invalid, 200, 200).is_err());
        assert!(store.update_moment(note.id, &invalid, 200, 200).is_err());
    }
    assert!(store.create_moment("Invalid time", i64::MAX, 200).is_err());
    assert!(
        store
            .create_moment("Invalid creation time", 200, i64::MIN)
            .is_err()
    );
    assert!(
        store
            .update_moment(note.id, "Invalid edit time", 200, i64::MAX)
            .is_err()
    );
    assert_eq!(store.moments_between(0, 300).unwrap(), vec![note.clone()]);
    // A clock correction cannot move audit time backwards or reject valid text.
    let edited = store.update_moment(note.id, "Changed", 110, 90).unwrap();
    assert_eq!((edited.created_at, edited.updated_at), (100, 100));
}

#[test]
fn v6_upgrade_preserves_existing_tasks_plans_intervals_and_preferences() {
    let (directory, mut store) = database();
    let task = store.create_task("Existing task", 1, 100).unwrap();
    let session = store.start_task(task.id, 110, None).unwrap();
    store.finish(session.id, 170).unwrap();
    let day = NaiveDate::from_ymd_opt(2026, 10, 28).unwrap();
    let plan = store
        .create_plan(
            &PlanDraft {
                task_id: Some(task.id),
                title: task.title.clone(),
                tag_id: 1,
                day,
                start_minute: 540,
                end_minute: 600,
                note: "Keep this note".to_owned(),
            },
            200,
        )
        .unwrap();
    let tags = store.tags().unwrap();
    let sessions = store.sessions().unwrap();
    let tasks = store.tasks(300).unwrap();
    store
        .connection
        .execute(
            "UPDATE preferences SET goal_minutes=240, completion_sound=0 WHERE id=1",
            [],
        )
        .unwrap();
    store
        .connection
        .execute_batch("DROP TABLE moments; PRAGMA user_version=6;")
        .unwrap();
    drop(store);

    let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        SCHEMA_VERSION
    );
    assert_eq!(store.tags().unwrap(), tags);
    assert_eq!(store.sessions().unwrap(), sessions);
    assert_eq!(store.tasks(300).unwrap(), tasks);
    assert_eq!(store.plans_for_day(day).unwrap(), vec![plan]);
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT goal_minutes, completion_sound FROM preferences WHERE id=1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            )
            .unwrap(),
        (240, 0)
    );
    assert_eq!(store.stats(0, 300, 300).unwrap().focus_seconds, 60);
    let note = store.create_moment("After upgrade", 250, 300).unwrap();
    assert_eq!(store.moments_between(0, 301).unwrap(), vec![note]);
}

#[test]
fn failed_v6_upgrade_rolls_back_version_and_preserves_existing_records() {
    let (directory, mut store) = database();
    let session = store.start("Existing", 1, 100).unwrap();
    store.finish(session.id, 160).unwrap();
    store.connection.execute_batch("DROP TABLE moments; CREATE TABLE moments (id INTEGER PRIMARY KEY, occurred_at INTEGER, deleted INTEGER); PRAGMA user_version=6;").unwrap();
    drop(store);
    let path = directory.path().join("force.sqlite3");
    assert!(Store::open(&path).is_err());
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        6
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='visible_moment_times'", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
}

#[test]
fn concurrent_v6_upgrade_rechecks_version_under_the_write_lock() {
    let (directory, store) = database();
    store
        .connection
        .execute_batch("DROP TABLE moments; PRAGMA user_version=6;")
        .unwrap();
    drop(store);
    concurrent_open(directory.path().join("force.sqlite3"));
}

#[test]
fn concurrent_fresh_open_does_not_recreate_the_committed_schema() {
    let directory = tempfile::tempdir().unwrap();
    concurrent_open(directory.path().join("force.sqlite3"));
}

fn concurrent_open(path: std::path::PathBuf) {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let barrier = barrier.clone();
            let path = path.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let mut store = Store::open(path).unwrap();
                store.create_moment("Concurrent note", 100, 100).unwrap()
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let store = Store::open(path).unwrap();
    assert_eq!(store.moments_between(0, 200).unwrap().len(), 2);
    assert_eq!(store.tags().unwrap().len(), 4);
}
