use super::*;

fn database() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    (directory, store)
}

#[test]
fn legacy_focus_resume_preserves_timing_and_atomically_switches_other_sessions() {
    for target in [None, Some(1500)] {
        let (_directory, mut store) = database();
        let focus = match target {
            Some(target) => store.start_timed("Legacy focus", 1, 100, target).unwrap(),
            None => store.start("Legacy focus", 1, 100).unwrap(),
        };
        store.pause(focus.id, 110).unwrap();
        let other_task = store.create_task("Other focus", 1, 120).unwrap();
        let other = store.start_task(other_task.id, 120, None).unwrap();
        // Legacy stores can contain overlapping sessions. Switching must settle
        // all running rest and focus in the same transaction.
        let rest = store.start_timed("Legacy rest", 4, 130, 300).unwrap();
        let resumed = store.resume_focus(focus.id, 140).unwrap();
        assert_eq!(resumed.id, focus.id);
        assert_eq!(resumed.task_id, None);
        assert_eq!(resumed.title, focus.title);
        assert_eq!(resumed.tag_id, focus.tag_id);
        assert_eq!(resumed.started_at, focus.started_at);
        assert_eq!(resumed.target_seconds, target);
        assert_eq!(resumed.intervals.len(), 2);
        assert_eq!(resumed.elapsed_at(150), 20);
        let active = store.active_sessions().unwrap();
        assert_eq!(active.len(), 2);
        assert_eq!(
            active.iter().filter(|session| session.is_running()).count(),
            1
        );
        let paused = active
            .iter()
            .find(|session| session.id == other.id)
            .unwrap();
        assert!(paused.is_paused());
        assert_eq!(paused.elapsed_at(150), 20);
        let records = store.sessions().unwrap();
        let rest = records
            .iter()
            .find(|session| session.id == rest.id)
            .unwrap();
        assert_eq!(rest.finished_at, Some(140));
        assert_eq!(rest.elapsed_at(150), 10);
        assert_eq!(store.stats(0, 200, 150).unwrap().focus_seconds, 40);
    }
}

#[test]
fn failed_focus_resume_insert_rolls_back_other_focus_and_rest_changes() {
    let (_directory, mut store) = database();
    let focus = store.start("Legacy focus", 1, 100).unwrap();
    store.pause(focus.id, 110).unwrap();
    store.start("Other focus", 1, 120).unwrap();
    store.start_timed("Rest", 4, 130, 300).unwrap();
    let before = store.active_sessions().unwrap();
    store
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_focus_resume BEFORE INSERT ON intervals
             BEGIN SELECT RAISE(ABORT, 'test rejection'); END;",
        )
        .unwrap();
    assert!(matches!(
        store.resume_focus(focus.id, 140),
        Err(Error::Database(_))
    ));
    assert_eq!(store.active_sessions().unwrap(), before);
    assert!(store.sessions().unwrap().is_empty());
}

#[test]
fn focus_resume_rejects_invalid_or_elapsed_rounds_without_changing_other_sessions() {
    let (_directory, mut store) = database();
    let elapsed = store.start_timed("Elapsed focus", 1, 100, 60).unwrap();
    store.pause(elapsed.id, 160).unwrap();
    let rest = store.start_timed("Paused rest", 4, 170, 300).unwrap();
    store.pause(rest.id, 180).unwrap();
    let focus = store.start("Running focus", 1, 190).unwrap();
    let before = store.active_sessions().unwrap();
    for id in [elapsed.id, rest.id, focus.id] {
        assert!(matches!(
            store.resume_focus(id, 200),
            Err(Error::InvalidTransition)
        ));
        assert_eq!(store.active_sessions().unwrap(), before);
    }
}

#[test]
fn repeating_break_reuses_the_same_round_and_pauses_focus_atomically() {
    let (_directory, mut store) = database();
    let task = store.create_task("Write", 1, 100).unwrap();
    let focus = store.start_task(task.id, 100, Some(1500)).unwrap();
    let first = store.start_break(4, 130, 300).unwrap();
    let repeated = store.start_break(4, 140, 600).unwrap();
    assert_eq!(repeated.id, first.id);
    assert_eq!(repeated.target_seconds, Some(300));
    assert_eq!(repeated.intervals.len(), 1);
    assert_eq!(repeated.elapsed_at(140), 10);
    let active = store.active_sessions().unwrap();
    assert_eq!(active.len(), 2);
    assert!(
        active
            .iter()
            .find(|s| s.id == focus.id)
            .unwrap()
            .is_paused()
    );
    assert_eq!(
        active
            .iter()
            .find(|s| s.id == focus.id)
            .unwrap()
            .elapsed_at(140),
        30
    );
    store.pause(first.id, 150).unwrap();
    let resumed = store.start_break(4, 200, 300).unwrap();
    assert_eq!(resumed.id, first.id);
    assert_eq!(resumed.elapsed_at(210), 30);
    let continued = store.start_task(task.id, 220, Some(1500)).unwrap();
    assert_eq!(continued.id, focus.id);
    assert_eq!(store.active_sessions().unwrap().len(), 1);
    assert_eq!(store.stats(0, 300, 230).unwrap().focus_seconds, 40);
}

#[test]
fn returning_to_focus_ends_paused_rest_and_the_next_break_starts_fresh() {
    for use_resume_focus in [false, true] {
        let (_directory, mut store) = database();
        let task = store.create_task("Write", 1, 100).unwrap();
        let focus = store.start_task(task.id, 100, Some(1500)).unwrap();
        let rest = store.start_break(4, 130, 300).unwrap();
        store.pause(rest.id, 160).unwrap();

        let resumed = if use_resume_focus {
            store.resume_focus(focus.id, 200).unwrap()
        } else {
            store.start_task(task.id, 200, Some(1500)).unwrap()
        };
        assert_eq!(resumed.id, focus.id);
        assert_eq!(resumed.target_seconds, Some(1500));
        assert_eq!(resumed.elapsed_at(210), 40);
        assert_eq!(store.active_sessions().unwrap(), vec![resumed]);
        let records = store.sessions().unwrap();
        let finished_rest = records.iter().find(|s| s.id == rest.id).unwrap();
        assert_eq!(finished_rest.finished_at, Some(200));
        assert_eq!(finished_rest.elapsed_at(210), 30);

        let next = store.start_break(4, 230, 300).unwrap();
        assert_ne!(next.id, rest.id);
        assert_eq!(next.target_seconds, Some(300));
        assert_eq!(next.elapsed_at(230), 0);
    }
}

#[test]
fn failed_focus_resume_rolls_back_paused_rest_completion_and_other_focus_pause() {
    for use_resume_focus in [false, true] {
        let (_directory, mut store) = database();
        let task = store.create_task("Write", 1, 100).unwrap();
        let focus = store.start_task(task.id, 100, None).unwrap();
        let rest = store.start_break(4, 130, 300).unwrap();
        store.pause(rest.id, 160).unwrap();
        // Legacy data can contain running focus beside a paused break.
        store.start("Other focus", 1, 170).unwrap();
        let before = store.active_sessions().unwrap();
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_focus_resume BEFORE INSERT ON intervals
                 BEGIN SELECT RAISE(ABORT, 'test rejection'); END;",
            )
            .unwrap();

        let result = if use_resume_focus {
            store.resume_focus(focus.id, 200)
        } else {
            store.start_task(task.id, 200, None)
        };
        assert!(matches!(result, Err(Error::Database(_))));
        assert_eq!(store.active_sessions().unwrap(), before);
        assert!(store.sessions().unwrap().is_empty());
    }
}

#[test]
fn failed_new_focus_insert_rolls_back_paused_rest_completion_and_other_focus_pause() {
    let (_directory, mut store) = database();
    let task = store.create_task("Write", 1, 100).unwrap();
    let rest = store.start_break(4, 130, 300).unwrap();
    store.pause(rest.id, 160).unwrap();
    store.start("Other focus", 1, 170).unwrap();
    let before = store.active_sessions().unwrap();
    store
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_new_focus BEFORE INSERT ON sessions
             BEGIN SELECT RAISE(ABORT, 'test rejection'); END;",
        )
        .unwrap();

    assert!(matches!(
        store.start_task(task.id, 200, None),
        Err(Error::Database(_))
    ));
    assert_eq!(store.active_sessions().unwrap(), before);
    assert!(store.sessions().unwrap().is_empty());
}

#[test]
fn a_new_break_settles_expired_timers_and_previous_duplicate_breaks() {
    let (_directory, mut store) = database();
    let focus = store.create_task("Read", 1, 100).unwrap();
    let focus = store.start_task(focus.id, 100, Some(60)).unwrap();
    let expired_rest = store.start_timed("Old rest", 4, 100, 60).unwrap();
    let older_rest = store.start_timed("Earlier rest", 4, 170, 300).unwrap();
    let current_rest = store.start_timed("Current rest", 4, 180, 300).unwrap();
    let selected = store.start_break(4, 200, 300).unwrap();
    assert_eq!(selected.id, current_rest.id);
    assert_eq!(store.active_sessions().unwrap().len(), 1);
    let records = store.sessions().unwrap();
    for id in [focus.id, expired_rest.id] {
        let session = records.iter().find(|s| s.id == id).unwrap();
        assert_eq!(session.finished_at, Some(160));
        assert_eq!(session.elapsed_at(200), 60);
    }
    assert_eq!(
        records
            .iter()
            .find(|s| s.id == older_rest.id)
            .unwrap()
            .elapsed_at(200),
        30
    );
    let next = store.start_break(4, 600, 300).unwrap();
    assert_ne!(next.id, selected.id);
    assert_eq!(store.active_sessions().unwrap().len(), 1);
    assert_eq!(
        store
            .sessions()
            .unwrap()
            .iter()
            .find(|s| s.id == selected.id)
            .unwrap()
            .finished_at,
        Some(480)
    );
}

#[test]
fn failed_break_keeps_focus_and_existing_rest_unchanged() {
    let (_directory, mut store) = database();
    let task = store.create_task("Write", 1, 100).unwrap();
    store.start_task(task.id, 100, None).unwrap();
    store.start_timed("Future rest", 4, 200, 300).unwrap();
    let before = store.active_sessions().unwrap();
    assert!(matches!(
        store.start_break(4, 150, 300),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(store.active_sessions().unwrap(), before);
    assert!(store.sessions().unwrap().is_empty());
    assert!(matches!(
        store.start_break(1, 210, 300),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(store.active_sessions().unwrap(), before);
    assert!(store.sessions().unwrap().is_empty());
}

#[test]
fn failed_break_insert_rolls_back_focus_pause_and_expired_rest_completion() {
    let (_directory, mut store) = database();
    let task = store.create_task("Write", 1, 100).unwrap();
    store.start_task(task.id, 100, None).unwrap();
    store.start_timed("Expired rest", 4, 100, 60).unwrap();
    let before = store.active_sessions().unwrap();
    store.connection.execute_batch("CREATE TRIGGER reject_break BEFORE INSERT ON sessions BEGIN SELECT RAISE(ABORT, 'test rejection'); END;").unwrap();
    assert!(matches!(
        store.start_break(4, 200, 300),
        Err(Error::Database(_))
    ));
    assert_eq!(store.active_sessions().unwrap(), before);
    assert!(store.sessions().unwrap().is_empty());
}

#[test]
fn saved_tasks_survive_days_and_restart_without_starting_a_timer() {
    let (directory, mut store) = database();
    let task = store.create_task("  Write  ", 1, 100).unwrap();
    assert_eq!(task.title, "Write");
    assert!(store.active_sessions().unwrap().is_empty());
    assert_eq!(store.tasks(86_500).unwrap(), vec![task.clone()]);
    drop(store);
    let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert_eq!(store.tasks(86_500).unwrap(), vec![task.clone()]);
    let completed = store.complete_task(task.id, 86_500).unwrap();
    assert_eq!(completed.completed_at, Some(86_500));
    assert_eq!(completed.total_seconds, 0);
    assert!(matches!(
        store.start_task(task.id, 86_501, None),
        Err(Error::InvalidTransition)
    ));
    store.reopen_task(task.id).unwrap();
    assert_eq!(
        store.start_task(task.id, 86_502, None).unwrap().task_id,
        Some(task.id)
    );
}

#[test]
fn switching_focus_pauses_other_focus_and_reuses_the_same_paused_session() {
    let (_directory, mut store) = database();
    let a = store.create_task("A", 1, 100).unwrap();
    let b = store.create_task("B", 1, 100).unwrap();
    let first = store.start_task(a.id, 100, Some(60)).unwrap();
    let second = store.start_task(b.id, 120, None).unwrap();
    let rest = store.start("Rest", 4, 125).unwrap();
    let continued = store.start_task(a.id, 140, Some(300)).unwrap();
    assert_eq!(continued.id, first.id);
    assert_eq!(continued.target_seconds, Some(60));
    let active = store.active_sessions().unwrap();
    assert!(
        active
            .iter()
            .find(|s| s.id == second.id)
            .unwrap()
            .is_paused()
    );
    assert!(!active.iter().any(|s| s.id == rest.id));
    assert_eq!(
        store
            .sessions()
            .unwrap()
            .iter()
            .find(|s| s.id == rest.id)
            .unwrap()
            .elapsed_at(150),
        15
    );
    assert_eq!(
        store
            .tasks(150)
            .unwrap()
            .iter()
            .find(|t| t.id == a.id)
            .unwrap()
            .total_seconds,
        30
    );
    assert!(matches!(
        store.start_task(a.id, 150, None),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(
        store
            .tasks(150)
            .unwrap()
            .iter()
            .find(|t| t.id == a.id)
            .unwrap()
            .session_count,
        1
    );
}

#[test]
fn ending_a_round_does_not_complete_a_task_and_later_rounds_share_its_identity() {
    let (_directory, mut store) = database();
    let task = store.create_task("Read", 3, 100).unwrap();
    let first = store.start_task(task.id, 100, Some(60)).unwrap();
    store.finish_exact_target(first.id, 300).unwrap();
    assert_eq!(store.tasks(300).unwrap()[0].completed_at, None);
    let second = store.start_task(task.id, 400, None).unwrap();
    assert_ne!(first.id, second.id);
    store.pause(second.id, 420).unwrap();
    let done = store.complete_task(task.id, 500).unwrap();
    assert_eq!(done.completed_at, Some(500));
    assert_eq!(done.total_seconds, 80);
    assert_eq!(done.session_count, 2);
    assert!(store.active_sessions().unwrap().is_empty());
    store.reopen_task(task.id).unwrap();
    let third = store.start_task(task.id, 600, None).unwrap();
    assert_eq!(third.task_id, Some(task.id));
    assert_eq!(store.tasks(610).unwrap()[0].total_seconds, 90);
}

#[test]
fn edits_keep_finished_history_and_archived_tags_require_explicit_reassignment() {
    let (_directory, mut store) = database();
    let task = store.create_task("Before", 1, 100).unwrap();
    let old = store.start_task(task.id, 100, None).unwrap();
    store.finish(old.id, 110).unwrap();
    let current = store.start_task(task.id, 120, None).unwrap();
    store.update_task(task.id, "After", 2).unwrap();
    let active = store.active_sessions().unwrap();
    assert_eq!(active[0].title, "After");
    assert_eq!(active[0].tag_id, 2);
    assert_eq!(store.sessions().unwrap()[0].title, "Before");
    store.finish(current.id, 130).unwrap();
    store.delete_tag(2).unwrap();
    assert_eq!(store.tasks(140).unwrap()[0].tag_name, "学习");
    assert!(matches!(
        store.start_task(task.id, 140, None),
        Err(Error::ArchivedTag)
    ));
    store.update_task(task.id, "Reassigned", 3).unwrap();
    store.rename_tag(3, "Books").unwrap();
    assert_eq!(store.tasks(150).unwrap()[0].tag_name, "Books");
    assert_eq!(
        store.start_task(task.id, 150, None).unwrap().tag_name,
        "Books"
    );
}

#[test]
fn deleting_a_task_hides_identity_but_preserves_all_recorded_time() {
    let (_directory, mut store) = database();
    let task = store.create_task("Keep evidence", 1, 100).unwrap();
    let session = store.start_task(task.id, 100, None).unwrap();
    assert!(matches!(
        store.delete_task(task.id),
        Err(Error::InvalidTransition)
    ));
    store.finish(session.id, 140).unwrap();
    let before = store.stats(0, 200, 200).unwrap();
    store.delete_task(task.id).unwrap();
    assert!(store.tasks(200).unwrap().is_empty());
    assert_eq!(store.sessions().unwrap()[0].task_id, Some(task.id));
    assert_eq!(store.stats(0, 200, 200).unwrap(), before);
    assert!(matches!(
        store.start_task(task.id, 200, None),
        Err(Error::NotFound)
    ));
}

#[test]
fn failed_switch_and_completion_roll_back_task_and_timer_changes_together() {
    let (_directory, mut store) = database();
    let a = store.create_task("A", 1, 100).unwrap();
    let b = store.create_task("B", 1, 100).unwrap();
    let original = store.start_task(a.id, 100, None).unwrap();
    store.connection.execute_batch("CREATE TRIGGER reject_new_interval BEFORE INSERT ON intervals BEGIN SELECT RAISE(ABORT, 'test rejection'); END;").unwrap();
    assert!(matches!(
        store.start_task(b.id, 120, None),
        Err(Error::Database(_))
    ));
    assert_eq!(store.active_sessions().unwrap(), vec![original.clone()]);
    store.connection.execute_batch("DROP TRIGGER reject_new_interval; CREATE TRIGGER reject_task_completion BEFORE UPDATE OF completed_at ON tasks BEGIN SELECT RAISE(ABORT, 'test rejection'); END;").unwrap();
    assert!(matches!(
        store.complete_task(a.id, 130),
        Err(Error::Database(_))
    ));
    assert_eq!(store.active_sessions().unwrap(), vec![original]);
    assert!(
        store
            .tasks(140)
            .unwrap()
            .iter()
            .all(|t| t.completed_at.is_none())
    );
}

fn create_v2(path: &Path) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(
        "CREATE TABLE tags(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, kind TEXT NOT NULL, archived INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE sessions(id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, tag_id INTEGER NOT NULL REFERENCES tags(id), tag_name TEXT NOT NULL, tag_kind TEXT NOT NULL, started_at INTEGER NOT NULL, finished_at INTEGER, target_seconds INTEGER);
         CREATE TABLE intervals(id INTEGER PRIMARY KEY, session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE, started_at INTEGER NOT NULL, ended_at INTEGER);
         CREATE TABLE preferences(id INTEGER PRIMARY KEY, goal_minutes INTEGER NOT NULL);
         INSERT INTO tags VALUES(1, 'Work', 'focus', 0);
         INSERT INTO sessions VALUES(1, 'Old record', 1, 'Work', 'focus', 100, 120, NULL);
         INSERT INTO sessions VALUES(2, 'Old active timer', 1, 'Work', 'focus', 200, NULL, 60);
         INSERT INTO intervals VALUES(1, 1, 100, 120), (2, 2, 200, NULL);
         INSERT INTO preferences VALUES(1, 75);
         PRAGMA user_version = 2;",
    ).unwrap();
}

#[test]
fn version_two_upgrade_keeps_old_records_standalone_and_recovers_timers() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("force.sqlite3");
    create_v2(&path);
    let mut store = Store::open(&path).unwrap();
    assert!(store.tasks(250).unwrap().is_empty());
    assert_eq!(store.sessions().unwrap()[0].task_id, None);
    let old = store.active_sessions().unwrap().remove(0);
    assert_eq!(old.id, 2);
    assert_eq!(old.target_seconds, Some(60));
    assert_eq!(old.task_id, None);
    assert!(old.is_running());
    assert_eq!(store.goal_minutes().unwrap(), 75);
    let task = store.create_task("New identity", 1, 250).unwrap();
    store.start_task(task.id, 250, None).unwrap();
    assert!(
        store
            .active_sessions()
            .unwrap()
            .iter()
            .find(|s| s.id == 2)
            .unwrap()
            .is_paused()
    );
    assert_eq!(store.sessions().unwrap()[0].title, "Old record");
}

#[test]
fn failed_task_schema_upgrade_preserves_version_columns_and_old_records() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("force.sqlite3");
    create_v2(&path);
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("CREATE INDEX task_sessions ON sessions(id);")
        .unwrap();
    drop(connection);
    assert!(matches!(Store::open(&path), Err(Error::Database(_))));
    let connection = Connection::open(&path).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2);
    assert!(connection.prepare("SELECT task_id FROM sessions").is_err());
    assert!(connection.prepare("SELECT id FROM tasks").is_err());
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn completion_sound_defaults_on_and_remains_off_after_restart_and_schema_upgrade() {
    let (directory, mut store) = database();
    assert!(store.completion_sound().unwrap());
    store.set_completion_sound(false).unwrap();
    drop(store);
    let path = directory.path().join("force.sqlite3");
    let store = Store::open(&path).unwrap();
    assert!(!store.completion_sound().unwrap());
    drop(store);
    // Recreate the immediately preceding schema, which had no sound column.
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "ALTER TABLE preferences DROP COLUMN completion_sound; PRAGMA user_version = 3;",
        )
        .unwrap();
    drop(connection);
    let mut store = Store::open(&path).unwrap();
    assert!(store.completion_sound().unwrap());
    store.set_completion_sound(false).unwrap();
    drop(store);
    assert!(!Store::open(&path).unwrap().completion_sound().unwrap());
}

#[test]
fn delayed_focus_switch_caps_expired_focus_and_rest_without_completing_the_old_task() {
    let (_directory, mut store) = database();
    let a = store.create_task("First", 1, 90).unwrap();
    let b = store.create_task("Next", 1, 90).unwrap();
    let first = store.start_task(a.id, 100, Some(60)).unwrap();
    let rest = store.start_timed("Break", 4, 110, 60).unwrap();
    store.start_task(b.id, 300, None).unwrap();
    let history = store.sessions().unwrap();
    assert_eq!(
        history
            .iter()
            .find(|s| s.id == first.id)
            .unwrap()
            .finished_at,
        Some(160)
    );
    assert_eq!(
        history
            .iter()
            .find(|s| s.id == rest.id)
            .unwrap()
            .finished_at,
        Some(170)
    );
    let tasks = store.tasks(310).unwrap();
    assert_eq!(
        tasks.iter().find(|t| t.id == a.id).unwrap().total_seconds,
        60
    );
    assert_eq!(
        tasks.iter().find(|t| t.id == a.id).unwrap().completed_at,
        None
    );
    assert_eq!(store.active_sessions().unwrap().len(), 1);
    assert_eq!(
        store.stats(0, 400, 310).unwrap(),
        Stats {
            focus_seconds: 70,
            task_seconds: 130,
            completed_sessions: 2
        }
    );
    let csv = store.csv_export(310).unwrap();
    assert!(csv.contains("\"100\",\"160\",\"60\",\"60\",\"completed\""));
    assert!(csv.contains("\"110\",\"170\",\"60\",\"60\",\"completed\""));
}

#[test]
fn completing_an_expired_task_uses_its_exact_target_but_the_explicit_task_completion_time() {
    let (_directory, mut store) = database();
    let task = store.create_task("Finish deliberately", 1, 90).unwrap();
    let session = store.start_task(task.id, 100, Some(60)).unwrap();
    store.pause(session.id, 140).unwrap();
    store.start_task(task.id, 200, None).unwrap();
    let done = store.complete_task(task.id, 500).unwrap();
    assert_eq!(done.completed_at, Some(500));
    assert_eq!(done.total_seconds, 60);
    assert_eq!(store.sessions().unwrap()[0].finished_at, Some(220));
    assert_eq!(store.sessions().unwrap()[0].elapsed_at(500), 60);
}

#[test]
fn a_failed_focus_start_does_not_end_rest_or_leave_partially_changed_records() {
    let (_directory, mut store) = database();
    let task = store.create_task("Next", 1, 90).unwrap();
    let rest = store.start_timed("Break", 4, 100, 60).unwrap();
    store.connection.execute_batch("CREATE TRIGGER reject_start BEFORE INSERT ON intervals BEGIN SELECT RAISE(ABORT, 'test rejection'); END;").unwrap();
    assert!(matches!(
        store.start_task(task.id, 300, None),
        Err(Error::Database(_))
    ));
    assert_eq!(store.active_sessions().unwrap(), vec![rest]);
    assert!(store.sessions().unwrap().is_empty());
    assert_eq!(store.tasks(300).unwrap()[0].session_count, 0);
}

#[test]
fn stale_edits_cannot_modify_completed_tasks_and_reopening_refreshes_current_tag_names() {
    let (directory, mut store) = database();
    let task = store.create_task("Original", 1, 90).unwrap();
    store.start_task(task.id, 100, None).unwrap();
    let mut other = Store::open(directory.path().join("force.sqlite3")).unwrap();
    other.complete_task(task.id, 120).unwrap();
    assert!(matches!(
        store.update_task(task.id, "Stale title", 2),
        Err(Error::InvalidTransition)
    ));
    assert!(matches!(
        store.start_task(task.id, 130, None),
        Err(Error::InvalidTransition)
    ));
    store.rename_tag(1, "Current work").unwrap();
    let completed = store.tasks(130).unwrap().remove(0);
    assert_eq!(completed.title, "Original");
    assert_eq!(completed.tag_name, "工作");
    store.reopen_task(task.id).unwrap();
    assert_eq!(store.tasks(130).unwrap()[0].tag_name, "Current work");
    store.update_task(task.id, "Explicitly edited", 2).unwrap();
    assert_eq!(store.tasks(130).unwrap()[0].title, "Explicitly edited");
    assert_eq!(store.sessions().unwrap()[0].title, "Original");
    assert_eq!(store.sessions().unwrap()[0].tag_name, "工作");
}

#[test]
fn an_unfinished_round_cannot_reclassify_past_rest_as_focus() {
    let (_directory, mut store) = database();
    let task = store.create_task("Rest identity", 4, 90).unwrap();
    let session = store.start_task(task.id, 100, None).unwrap();
    store.pause(session.id, 120).unwrap();
    assert!(matches!(
        store.update_task(task.id, "Now focus", 1),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(store.tasks(130).unwrap()[0].tag_kind, TagKind::Rest);
    assert_eq!(store.stats(0, 200, 130).unwrap().focus_seconds, 0);
    store.finish(session.id, 130).unwrap();
    store.update_task(task.id, "Now focus", 1).unwrap();
    store.start_task(task.id, 140, None).unwrap();
    assert_eq!(store.tasks(150).unwrap()[0].total_seconds, 30);
    assert_eq!(store.stats(0, 200, 150).unwrap().focus_seconds, 10);
}
