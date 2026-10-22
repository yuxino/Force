use super::*;

fn database() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("data/force.sqlite3")).unwrap();
    (directory, store)
}

#[test]
fn running_and_paused_tasks_survive_restart_and_exclude_pauses() {
    let (directory, mut store) = database();
    let work = store.start("Write a chapter", 1, 100).unwrap();
    let break_time = store.start("Tea", 4, 110).unwrap();
    store.pause(work.id, 130).unwrap();
    drop(store);

    let mut store = Store::open(directory.path().join("data/force.sqlite3")).unwrap();
    let tasks = store.active_sessions().unwrap();
    assert!(tasks.iter().find(|s| s.id == work.id).unwrap().is_paused());
    assert_eq!(
        tasks
            .iter()
            .find(|s| s.id == work.id)
            .unwrap()
            .elapsed_at(1_000),
        30
    );
    assert!(
        tasks
            .iter()
            .find(|s| s.id == break_time.id)
            .unwrap()
            .is_running()
    );
    assert_eq!(
        tasks
            .iter()
            .find(|s| s.id == break_time.id)
            .unwrap()
            .elapsed_at(140),
        30
    );

    store.resume(work.id, 200).unwrap();
    let done = store.finish(work.id, 220).unwrap();
    assert_eq!(done.elapsed_at(900), 50);
    assert!(!done.is_paused());
    assert!(!done.is_running());
    assert_eq!(store.sessions().unwrap(), vec![done]);
}

#[test]
fn overlapping_focus_is_merged_but_task_investment_and_rest_are_independent() {
    let (_directory, mut store) = database();
    let first = store.start("Draft", 1, 90).unwrap();
    let second = store.start("Read", 3, 130).unwrap();
    let rest = store.start("Lunch", 4, 150).unwrap();
    store.finish(first.id, 160).unwrap();
    store.finish(second.id, 180).unwrap();
    store.finish(rest.id, 190).unwrap();

    // Crop the pre-window portion of the first task and merge [100,160)+[130,180).
    assert_eq!(
        store.stats(100, 200, 200).unwrap(),
        Stats {
            focus_seconds: 80,
            task_seconds: 150,
            completed_sessions: 3,
        }
    );
    let rows = store.sessions_overlapping(100, 200, 200).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter()
            .map(|s| s.elapsed_between(100, 200, 200))
            .sum::<i64>(),
        150
    );
    assert_eq!(
        rows.iter()
            .filter(|s| s.tag_kind == TagKind::Focus)
            .map(|s| s.elapsed_between(100, 200, 200))
            .sum::<i64>(),
        110
    );
    // A completed session at 180 belongs to the next [180,200) window.
    assert_eq!(
        store.stats(100, 180, 200).unwrap(),
        Stats {
            focus_seconds: 80,
            task_seconds: 140,
            completed_sessions: 1,
        }
    );
    assert_eq!(
        store.stats(180, 200, 200).unwrap(),
        Stats {
            focus_seconds: 0,
            task_seconds: 10,
            completed_sessions: 2,
        }
    );
}

#[test]
fn midnight_windows_and_clock_rollbacks_never_count_future_or_negative_time() {
    let (_directory, mut store) = database();
    let midnight = 86_400;
    let task = store.start("Across midnight", 1, midnight - 30).unwrap();
    assert_eq!(
        store
            .stats(0, midnight, midnight + 20)
            .unwrap()
            .focus_seconds,
        30
    );
    assert_eq!(
        store
            .stats(midnight, midnight * 2, midnight + 20)
            .unwrap()
            .focus_seconds,
        20
    );
    assert_eq!(
        store.stats(midnight, midnight * 2, midnight - 5).unwrap(),
        Stats::default()
    );
    assert_eq!(task.elapsed_at(midnight - 100), 0);
    assert!(matches!(
        store.pause(task.id, midnight - 31),
        Err(Error::InvalidTransition)
    ));
    store.pause(task.id, midnight + 20).unwrap();
    assert!(matches!(
        store.resume(task.id, midnight + 10),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(
        store.active_sessions().unwrap()[0].elapsed_at(midnight + 100),
        50
    );
}

#[test]
fn completion_is_visible_immediately_even_with_no_elapsed_time() {
    let (_directory, mut store) = database();
    let task = store.start("Quick decision", 1, 100).unwrap();
    store.finish(task.id, 100).unwrap();
    assert_eq!(
        store.stats(100, 200, 100).unwrap(),
        Stats {
            focus_seconds: 0,
            task_seconds: 0,
            completed_sessions: 1,
        }
    );
    assert_eq!(store.stats(0, 100, 100).unwrap().completed_sessions, 0);
    assert_eq!(store.stats(0, 200, 99).unwrap().completed_sessions, 0);
}

#[test]
fn stale_actions_preserve_records_and_deleted_ids_are_not_reused() {
    let (_directory, mut store) = database();
    let task = store.start("One", 1, 10).unwrap();
    assert!(matches!(
        store.resume(task.id, 20),
        Err(Error::InvalidTransition)
    ));
    let paused = store.pause(task.id, 20).unwrap();
    assert!(matches!(
        store.pause(task.id, 21),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(store.active_sessions().unwrap(), vec![paused]);
    let done = store.finish(task.id, 25).unwrap();
    assert!(matches!(
        store.finish(task.id, 30),
        Err(Error::InvalidTransition)
    ));
    assert!(matches!(
        store.resume(task.id, 30),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(store.sessions().unwrap(), vec![done]);
    store.delete_session(task.id).unwrap();
    let next = store.start("Two", 1, 40).unwrap();
    assert_ne!(task.id, next.id);
    assert!(matches!(store.finish(task.id, 45), Err(Error::NotFound)));
    assert!(store.active_sessions().unwrap()[0].is_running());
}

#[test]
fn archiving_tags_preserves_history_snapshots_and_stops_new_use() {
    let (_directory, mut store) = database();
    let tag = store.create_tag("Personal", TagKind::Focus).unwrap();
    let task = store.start("Plan", tag.id, 1).unwrap();
    store.delete_tag(tag.id).unwrap();
    assert!(!store.tags().unwrap().iter().any(|t| t.id == tag.id));
    assert!(matches!(
        store.start("Another", tag.id, 2),
        Err(Error::NotFound)
    ));
    let done = store.finish(task.id, 11).unwrap();
    assert_eq!(done.tag_name, "Personal");
    assert_eq!(done.tag_kind, TagKind::Focus);
    let replacement = store.create_tag("Personal", TagKind::Rest).unwrap();
    assert_ne!(replacement.id, tag.id);
    assert_eq!(store.sessions().unwrap()[0].tag_kind, TagKind::Focus);
}

#[test]
fn renaming_tags_updates_running_and_paused_tasks_but_preserves_completed_history() {
    let (_directory, mut store) = database();
    let tag = store.create_tag("Project", TagKind::Focus).unwrap();
    let completed = store.start("Finished draft", tag.id, 100).unwrap();
    store.finish(completed.id, 110).unwrap();
    let running = store.start("Current chapter", tag.id, 120).unwrap();
    let paused = store.start("Research", tag.id, 130).unwrap();
    store.pause(paused.id, 140).unwrap();

    let renamed = store.rename_tag(tag.id, "  Book  ").unwrap();
    assert_eq!(renamed.name, "Book");
    assert_eq!(renamed.kind, TagKind::Focus);
    let active = store.active_sessions().unwrap();
    assert!(active.iter().all(|task| task.tag_name == "Book"));
    assert!(
        active
            .iter()
            .find(|task| task.id == running.id)
            .unwrap()
            .is_running()
    );
    assert!(
        active
            .iter()
            .find(|task| task.id == paused.id)
            .unwrap()
            .is_paused()
    );
    assert_eq!(store.sessions().unwrap()[0].tag_name, "Project");
    assert_eq!(store.finish(running.id, 150).unwrap().tag_name, "Book");
    assert_eq!(store.start("Next", tag.id, 160).unwrap().tag_name, "Book");
}

#[test]
fn rename_checks_available_names_and_rejects_archived_or_missing_tags() {
    let (_directory, mut store) = database();
    let tag = store.create_tag("Personal", TagKind::Focus).unwrap();
    let other = store.create_tag("Ideas", TagKind::Focus).unwrap();
    assert_eq!(
        store.rename_tag(tag.id, "PERSONAL").unwrap().name,
        "PERSONAL"
    );
    assert!(matches!(
        store.rename_tag(tag.id, "ideas"),
        Err(Error::Validation(_))
    ));
    assert!(matches!(
        store.rename_tag(tag.id, "   "),
        Err(Error::Validation(_))
    ));
    assert!(matches!(
        store.rename_tag(tag.id, &"字".repeat(41)),
        Err(Error::Validation(_))
    ));
    store.delete_tag(other.id).unwrap();
    assert_eq!(store.rename_tag(tag.id, "Ideas").unwrap().name, "Ideas");
    assert!(matches!(
        store.rename_tag(other.id, "Archived"),
        Err(Error::NotFound)
    ));
    assert!(matches!(
        store.rename_tag(i64::MAX, "Missing"),
        Err(Error::NotFound)
    ));
}

#[test]
fn failed_rename_preserves_both_tag_metadata_and_session_names() {
    let (_directory, mut store) = database();
    let tag = store.create_tag("Before", TagKind::Focus).unwrap();
    store.start("Current task", tag.id, 100).unwrap();
    store.connection.execute_batch(
        "CREATE TRIGGER reject_name_change BEFORE UPDATE OF tag_name ON sessions BEGIN SELECT RAISE(ABORT, 'test write rejection'); END;",
    ).unwrap();
    assert!(matches!(
        store.rename_tag(tag.id, "After"),
        Err(Error::Database(_))
    ));
    assert_eq!(
        store
            .tags()
            .unwrap()
            .iter()
            .find(|item| item.id == tag.id)
            .unwrap()
            .name,
        "Before"
    );
    assert_eq!(store.active_sessions().unwrap()[0].tag_name, "Before");
}

#[test]
fn read_snapshots_keep_sessions_and_totals_consistent_while_another_instance_writes() {
    let (directory, mut store) = database();
    let task = store.start("Shared record", 1, 100).unwrap();
    let mut other = Store::open(directory.path().join("data/force.sqlite3")).unwrap();
    let version = store.data_version().unwrap();
    store.tags().unwrap();
    store.goal_minutes().unwrap();
    assert_eq!(store.data_version().unwrap(), version);
    let snapshot = store.connection.unchecked_transaction().unwrap();
    let before = calculate_stats(&snapshot, 0, 200, 120).unwrap();
    let sessions = load_sessions_in_snapshot(&snapshot, "WHERE finished_at IS NULL", []).unwrap();
    assert_eq!(
        before,
        Stats {
            focus_seconds: 20,
            task_seconds: 20,
            completed_sessions: 0
        }
    );
    assert!(sessions[0].is_running());

    other.finish(task.id, 110).unwrap();
    assert_eq!(calculate_stats(&snapshot, 0, 200, 120).unwrap(), before);
    assert_eq!(
        load_sessions_in_snapshot(&snapshot, "WHERE finished_at IS NULL", []).unwrap(),
        sessions
    );
    snapshot.commit().unwrap();

    let external_version = store.data_version().unwrap();
    assert_ne!(external_version, version);
    store.set_goal_minutes(90).unwrap();
    assert_eq!(store.data_version().unwrap(), external_version);

    assert_eq!(
        store.stats(0, 200, 120).unwrap(),
        Stats {
            focus_seconds: 10,
            task_seconds: 10,
            completed_sessions: 1
        }
    );
    assert!(store.active_sessions().unwrap().is_empty());
    assert_eq!(store.sessions().unwrap()[0].elapsed_at(120), 10);
}

#[test]
fn goal_validation_persists_valid_values_and_rejects_bad_writes() {
    let (directory, mut store) = database();
    assert_eq!(store.goal_minutes().unwrap(), 300);
    store.set_goal_minutes(75).unwrap();
    assert!(matches!(
        store.set_goal_minutes(0),
        Err(Error::Validation(_))
    ));
    assert!(matches!(
        store.set_goal_minutes(1441),
        Err(Error::Validation(_))
    ));
    assert_eq!(store.goal_minutes().unwrap(), 75);
    drop(store);
    let store = Store::open(directory.path().join("data/force.sqlite3")).unwrap();
    assert_eq!(store.goal_minutes().unwrap(), 75);
}

#[test]
fn a_failed_multistep_write_rolls_back_without_a_half_created_task() {
    let (_directory, mut store) = database();
    store.connection.execute_batch(
        "CREATE TRIGGER reject_intervals BEFORE INSERT ON intervals BEGIN SELECT RAISE(ABORT, 'test write rejection'); END;",
    ).unwrap();
    assert!(matches!(
        store.start("Unsaved", 1, 100),
        Err(Error::Database(_))
    ));
    assert!(store.active_sessions().unwrap().is_empty());
    assert!(store.sessions().unwrap().is_empty());
    store
        .connection
        .execute_batch("DROP TRIGGER reject_intervals;")
        .unwrap();
    let task = store.start("Saved", 1, 110).unwrap();
    store.connection.execute_batch(
        "CREATE TRIGGER reject_finish BEFORE UPDATE ON sessions BEGIN SELECT RAISE(ABORT, 'test write rejection'); END;",
    ).unwrap();
    assert!(matches!(
        store.finish(task.id, 120),
        Err(Error::Database(_))
    ));
    assert!(store.active_sessions().unwrap()[0].is_running());
    assert_eq!(store.active_sessions().unwrap()[0].elapsed_at(130), 20);
}

#[test]
fn damaged_or_newer_databases_are_never_replaced() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("damaged.sqlite3");
    let content = b"not a database; do not destroy me";
    fs::write(&path, content).unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), content);

    let newer = directory.path().join("newer.sqlite3");
    let connection = Connection::open(&newer).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    let content = fs::read(&newer).unwrap();
    assert!(matches!(Store::open(&newer), Err(Error::UnsupportedSchema)));
    assert_eq!(fs::read(&newer).unwrap(), content);
}

#[test]
fn csv_escapes_user_text_and_neutralizes_spreadsheet_formulas() {
    let (_directory, mut store) = database();
    let tag = store.create_tag("@tag", TagKind::Focus).unwrap();
    let task = store
        .start("=HYPERLINK(\"example\")\nnext, line", tag.id, 10)
        .unwrap();
    store.finish(task.id, 20).unwrap();
    let csv = store.csv_export(30).unwrap();
    assert!(csv.starts_with(
        "id,title,tag,kind,started_at,finished_at,target_seconds,elapsed_seconds,status,started_at_utc,finished_at_utc\r\n"
    ));
    assert!(csv.contains("\"'=HYPERLINK(\"\"example\"\")\nnext, line\",\"'@tag\""));
    assert!(csv.ends_with("\"10\",\"20\",\"\",\"10\",\"completed\",\"1970-01-01T00:00:10Z\",\"1970-01-01T00:00:20Z\"\r\n"));
    assert_eq!(csv_field(" \t+SUM(1,2)"), "\"' \t+SUM(1,2)\"");
    assert_eq!(csv_field("\r\ntext"), "\"'\r\ntext\"");
}

#[test]
fn csv_preserves_chinese_and_provides_readable_utc_without_panicking_on_extreme_times() {
    let (_directory, mut store) = database();
    let task = store.start("工作计划", 1, 1_700_000_000).unwrap();
    store.finish(task.id, 1_700_000_065).unwrap();
    store.start("远期任务", 1, i64::MAX).unwrap();
    let csv = store.csv_export(i64::MAX).unwrap();
    assert!(csv.contains("\"工作计划\",\"工作\""));
    assert!(csv.contains("\"1700000000\",\"1700000065\""));
    assert!(csv.contains("\"2023-11-14T22:13:20Z\",\"2023-11-14T22:14:25Z\""));
    assert!(csv.contains(&format!(
        "\"{}\",\"\",\"\",\"0\",\"running\",\"\",\"\"",
        i64::MAX
    )));
}

#[test]
fn recent_history_is_bounded_while_exports_retain_every_record() {
    let (_directory, mut store) = database();
    for index in 0..505 {
        let task = store
            .start(&format!("Task {index}"), 1, index * 10)
            .unwrap();
        store.finish(task.id, index * 10 + 5).unwrap();
    }
    let history = store.sessions().unwrap();
    assert_eq!(history.len(), HISTORY_LIMIT);
    assert_eq!(history.first().unwrap().title, "Task 504");
    assert_eq!(history.last().unwrap().title, "Task 5");
    assert_eq!(store.sessions_between(0, 6_000).unwrap().len(), 505);
    assert_eq!(
        store.sessions_overlapping(0, 6_000, 6_000).unwrap().len(),
        505
    );
    assert_eq!(store.sessions_between(0, 5).unwrap().len(), 0);
    assert_eq!(store.sessions_between(5, 6).unwrap()[0].title, "Task 0");
    assert!(store.sessions_between(6, 6).unwrap().is_empty());
    let csv = store.csv_export(6_000).unwrap();
    assert_eq!(csv.lines().count(), 506);
    assert!(csv.contains("\"Task 0\""));
}

#[cfg(unix)]
#[test]
fn app_database_directory_and_file_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let (directory, _store) = database();
    assert_eq!(
        fs::metadata(directory.path().join("data"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(directory.path().join("data/force.sqlite3"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn batched_focus_days_split_midnight_merge_overlap_and_exclude_rest() {
    let (_directory, mut store) = database();
    let midnight = 86_400;
    let first = store.start("Across midnight", 1, midnight - 30).unwrap();
    let overlap = store.start("Parallel focus", 3, midnight - 10).unwrap();
    let rest = store.start("Rest", 4, midnight - 100).unwrap();
    store.finish(first.id, midnight + 20).unwrap();
    store.finish(overlap.id, midnight + 50).unwrap();
    store.finish(rest.id, midnight + 100).unwrap();
    store.start("Still running", 1, midnight + 55).unwrap();
    let ranges = [
        (midnight, midnight * 2),
        (0, midnight),
        (midnight * 2, midnight * 3),
        (midnight + 5, midnight + 5),
        (midnight + 10, midnight + 1),
        (-100, 0),
    ];
    assert_eq!(
        store
            .focus_totals_for_ranges(&ranges, midnight + 60)
            .unwrap(),
        vec![55, 30, 0, 0, 0, 0]
    );
    assert_eq!(
        store
            .focus_totals_for_ranges(&[(midnight - 15, midnight + 25)], midnight + 10)
            .unwrap(),
        vec![25]
    );
}

#[test]
fn batched_focus_days_handle_empty_data_and_settled_timed_targets() {
    let (_directory, mut store) = database();
    assert_eq!(
        store.focus_totals_for_ranges(&[], 100).unwrap(),
        Vec::<i64>::new()
    );
    assert_eq!(
        store
            .focus_totals_for_ranges(&[(0, 200), (200, 300)], 100)
            .unwrap(),
        vec![0, 0]
    );
    let midnight = 86_400;
    let timed = store.start_timed("Timed", 1, midnight - 20, 60).unwrap();
    store.pause(timed.id, midnight + 10).unwrap();
    store.resume(timed.id, midnight + 30).unwrap();
    store.finish_exact_target(timed.id, midnight + 200).unwrap();
    assert_eq!(
        store
            .focus_totals_for_ranges(&[(0, midnight), (midnight, midnight * 2)], midnight + 200)
            .unwrap(),
        vec![20, 40]
    );
}

#[test]
fn cross_midnight_records_reconcile_daily_total_and_paused_time() {
    let (_directory, mut store) = database();
    let midnight = 86_400;
    // Synthetic 20:52:54–01:19 case: 33:41 paused, 54:00 after midnight.
    let session = store
        .start("Cross-day review", 1, midnight - 11_226)
        .unwrap();
    store.pause(session.id, midnight - 521).unwrap();
    store.resume(session.id, midnight + 1_500).unwrap();
    let session = store.finish(session.id, midnight + 4_740).unwrap();
    let now = midnight + 5_000;
    assert_eq!(session.elapsed_at(now), 13_945);
    assert_eq!(session.paused_at(now), 2_021);
    let mut split = 0;
    for (start, end, expected) in [(0, midnight, 10_705), (midnight, midnight * 2, 3_240)] {
        let rows = store.sessions_overlapping(start, end, now).unwrap();
        assert_eq!(rows, vec![session.clone()]);
        let seconds = rows[0].elapsed_between(start, end, now);
        assert_eq!(seconds, expected);
        assert_eq!(seconds, store.stats(start, end, now).unwrap().focus_seconds);
        assert_eq!(
            store.focus_totals_for_ranges(&[(start, end)], now).unwrap(),
            vec![seconds]
        );
        split += seconds;
    }
    assert_eq!(split, session.elapsed_at(now));
    assert_eq!(session.elapsed_between(midnight, midnight, now), 0);
    assert_eq!(session.elapsed_between(midnight + 5, midnight, now), 0);
    assert_eq!(
        session.elapsed_between(midnight, midnight * 2, midnight + 1_600),
        100
    );
    assert_eq!(session.elapsed_between(midnight * 2, midnight * 3, now), 0);
}

#[test]
fn date_records_include_live_sessions_but_not_days_spent_only_paused() {
    let (_directory, mut store) = database();
    let day = 86_400;
    let work = store.start("Long task", 1, day - 100).unwrap();
    store.pause(work.id, day - 10).unwrap();
    assert!(
        store
            .sessions_overlapping(day, day * 2, day + 500)
            .unwrap()
            .is_empty()
    );
    store.resume(work.id, day * 2 + 100).unwrap();
    let now = day * 2 + 200;
    assert!(
        store
            .sessions_overlapping(day, day * 2, now)
            .unwrap()
            .is_empty()
    );
    let active = store.sessions_overlapping(day * 2, day * 3, now).unwrap();
    assert_eq!(active.len(), 1);
    assert!(active[0].is_running());
    assert_eq!(active[0].elapsed_between(day * 2, day * 3, now), 100);
    store.pause(work.id, now).unwrap();
    let paused = store
        .sessions_overlapping(day * 2, day * 3, now + 50)
        .unwrap();
    assert!(paused[0].is_paused());
    assert_eq!(paused[0].elapsed_between(day * 2, day * 3, now + 50), 100);
    assert!(
        store
            .sessions_overlapping(day * 3, day * 4, now)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .sessions_overlapping(day, day, now)
            .unwrap()
            .is_empty()
    );
    // Zero-second events remain inspectable, on the exact half-open date boundary.
    let empty = store.start("Empty round", 1, day * 3).unwrap();
    store.finish(empty.id, day * 3).unwrap();
    let next = store
        .sessions_overlapping(day * 3, day * 4, day * 3)
        .unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].id, empty.id);
    assert_eq!(next[0].elapsed_between(day * 3, day * 4, day * 3), 0);
    assert!(
        store
            .sessions_overlapping(0, day * 3, day * 3 - 1)
            .unwrap()
            .iter()
            .all(|s| s.id != empty.id)
    );
}
