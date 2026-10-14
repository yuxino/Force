use super::*;

fn database() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("data/force.sqlite3")).unwrap();
    (directory, store)
}

#[test]
fn timed_targets_survive_restart_and_finish_at_the_exact_unpaused_deadline() {
    let (directory, mut store) = database();
    let task = store.start_timed("Concentrate", 1, 100, 1_500).unwrap();
    store.pause(task.id, 700).unwrap();
    assert!(matches!(
        store.finish_exact_target(task.id, 5_000),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(store.active_sessions().unwrap()[0].elapsed_at(5_000), 600);
    store.resume(task.id, 1_000).unwrap();
    drop(store);

    let mut store = Store::open(directory.path().join("data/force.sqlite3")).unwrap();
    assert_eq!(
        store.active_sessions().unwrap()[0].target_seconds,
        Some(1_500)
    );
    assert!(matches!(
        store.finish_exact_target(task.id, 1_899),
        Err(Error::InvalidTransition)
    ));
    let done = store.finish_exact_target(task.id, 5_000).unwrap();
    assert_eq!(done.finished_at, Some(1_900));
    assert_eq!(done.elapsed_at(5_000), 1_500);
    assert_eq!(
        done.intervals,
        vec![
            Interval {
                start: 100,
                end: Some(700)
            },
            Interval {
                start: 1_000,
                end: Some(1_900)
            }
        ]
    );
    assert_eq!(
        store.stats(0, 6_000, 5_000).unwrap(),
        Stats {
            focus_seconds: 1_500,
            task_seconds: 1_500,
            completed_sessions: 1
        }
    );
    assert!(store.active_sessions().unwrap().is_empty());
    assert!(matches!(
        store.finish_exact_target(task.id, 6_000),
        Err(Error::InvalidTransition)
    ));
}

#[test]
fn delayed_timed_actions_never_retain_excess_intervals_or_overstate_a_day() {
    let (_directory, mut store) = database();
    let midnight = 86_400;
    let task = store
        .start_timed("Late task", 1, midnight - 100, 60)
        .unwrap();
    store.pause(task.id, midnight + 20).unwrap();
    store.resume(task.id, midnight + 100).unwrap();
    let done = store.finish_exact_target(task.id, midnight + 200).unwrap();
    assert_eq!(done.finished_at, Some(midnight - 40));
    assert_eq!(done.elapsed_at(midnight + 200), 60);
    assert_eq!(done.intervals.len(), 1);
    assert_eq!(
        store.stats(midnight, midnight * 2, midnight + 200).unwrap(),
        Stats::default()
    );
    assert_eq!(store.sessions_between(0, midnight).unwrap()[0].id, task.id);
}

#[test]
fn timed_duration_validation_and_manual_completion_preserve_intent() {
    let (_directory, mut store) = database();
    for target in [-1, 0, 59, 86_401] {
        assert!(matches!(
            store.start_timed("Invalid", 1, 100, target),
            Err(Error::Validation(_))
        ));
    }
    assert!(store.active_sessions().unwrap().is_empty());
    let early = store.start_timed("End early", 1, 100, 60).unwrap();
    let early = store.finish(early.id, 120).unwrap();
    assert_eq!(early.elapsed_at(1_000), 20);
    assert_eq!(early.finished_at, Some(120));
    let late = store.start_timed("End after deadline", 1, 200, 60).unwrap();
    let late = store.finish(late.id, 400).unwrap();
    assert_eq!(late.elapsed_at(1_000), 60);
    assert_eq!(late.finished_at, Some(260));
    let ordinary = store.start("No deadline", 1, 300).unwrap();
    assert!(matches!(
        store.finish_exact_target(ordinary.id, 1_000),
        Err(Error::InvalidTransition)
    ));
    assert!(store.active_sessions().unwrap()[0].is_running());
    assert!(
        store
            .csv_export(1_000)
            .unwrap()
            .contains("\"200\",\"260\",\"60\",\"60\",\"completed\"")
    );
}

fn create_v1(path: &Path) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(
        "CREATE TABLE tags(id INTEGER PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL, archived INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE sessions(id INTEGER PRIMARY KEY, title TEXT NOT NULL, tag_id INTEGER NOT NULL REFERENCES tags(id), tag_name TEXT NOT NULL, tag_kind TEXT NOT NULL, started_at INTEGER NOT NULL, finished_at INTEGER);
         CREATE TABLE intervals(id INTEGER PRIMARY KEY, session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE, started_at INTEGER NOT NULL, ended_at INTEGER);
         CREATE TABLE preferences(id INTEGER PRIMARY KEY, goal_minutes INTEGER NOT NULL);
         INSERT INTO tags VALUES(1, 'Writing', 'focus', 0);
         INSERT INTO sessions VALUES(1, 'Keep this task', 1, 'Writing', 'focus', 100, NULL);
         INSERT INTO intervals VALUES(1, 1, 100, 120);
         INSERT INTO preferences VALUES(1, 75);
         PRAGMA user_version = 1;",
    ).unwrap();
}

#[test]
fn schema_one_upgrade_preserves_tasks_pauses_tags_and_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("force.sqlite3");
    create_v1(&path);
    let mut store = Store::open(&path).unwrap();
    let task = store.active_sessions().unwrap().remove(0);
    assert_eq!(task.title, "Keep this task");
    assert_eq!(task.target_seconds, None);
    assert!(task.is_paused());
    assert_eq!(task.elapsed_at(1_000), 20);
    assert_eq!(store.goal_minutes().unwrap(), 75);
    assert_eq!(store.tags().unwrap()[0].name, "Writing");
    store.resume(task.id, 200).unwrap();
    assert_eq!(store.finish(task.id, 220).unwrap().elapsed_at(1_000), 40);
    let timed = store.start_timed("New timer", 1, 300, 60).unwrap();
    assert_eq!(timed.target_seconds, Some(60));
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.active_sessions().unwrap()[0].target_seconds, Some(60));
    assert_eq!(store.sessions().unwrap()[0].elapsed_at(1_000), 40);
}

#[test]
fn a_failed_schema_upgrade_rolls_back_both_column_and_version() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("force.sqlite3");
    create_v1(&path);
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("CREATE INDEX interval_time_window ON intervals(session_id);")
        .unwrap();
    drop(connection);
    assert!(matches!(Store::open(&path), Err(Error::Database(_))));
    let connection = Connection::open(&path).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);
    assert!(
        connection
            .prepare("SELECT target_seconds FROM sessions")
            .is_err()
    );
    let title: String = connection
        .query_row("SELECT title FROM sessions WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(title, "Keep this task");
}
