use super::*;

fn database() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    (directory, store)
}

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 25).unwrap()
}

fn draft() -> PlanDraft {
    PlanDraft {
        task_id: None,
        title: "Read and take notes".into(),
        tag_id: 1,
        day: day(),
        start_minute: 9 * 60,
        end_minute: 10 * 60,
        note: "One section at a time.\nKeep a question for later.".into(),
    }
}

#[test]
fn plan_creation_is_durable_and_never_starts_a_timer() {
    let (directory, mut store) = database();
    let plan = store.create_plan(&draft(), 100).unwrap();
    assert_eq!(plan.title, draft().title);
    assert!(store.active_sessions().unwrap().is_empty());
    assert_eq!(store.tasks(100).unwrap().len(), 1);
    assert_eq!(store.tasks(100).unwrap()[0].id, plan.task_id);
    drop(store);
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert_eq!(store.plans_for_day(day()).unwrap(), vec![plan]);
    assert!(store.active_sessions().unwrap().is_empty());
}

#[test]
fn plan_creation_failure_rolls_back_its_new_task() {
    let (_directory, mut store) = database();
    store.connection.execute_batch("CREATE TRIGGER fail_plan_insert BEFORE INSERT ON agenda_entries BEGIN SELECT RAISE(ABORT, 'fixture'); END;").unwrap();
    assert!(matches!(
        store.create_plan(&draft(), 100),
        Err(Error::Database(_))
    ));
    assert!(store.tasks(100).unwrap().is_empty());
    assert!(store.plans_for_day(day()).unwrap().is_empty());
}

#[test]
fn plans_validate_dates_text_and_minute_ranges_before_writing() {
    let (_directory, mut store) = database();
    for (start, end) in [(60, 60), (120, 60), (1440, 1441), (0, 1441)] {
        let mut value = draft();
        value.start_minute = start;
        value.end_minute = end;
        assert!(matches!(
            store.create_plan(&value, 100),
            Err(Error::Validation(_))
        ));
    }
    for title in [
        "".to_owned(),
        " ".to_owned(),
        "bad\0title".to_owned(),
        "字".repeat(201),
    ] {
        let mut value = draft();
        value.title = title;
        assert!(matches!(
            store.create_plan(&value, 100),
            Err(Error::Validation(_))
        ));
    }
    for note in ["bad\0note".to_owned(), "字".repeat(2001)] {
        let mut value = draft();
        value.note = note;
        assert!(matches!(
            store.create_plan(&value, 100),
            Err(Error::Validation(_))
        ));
    }
    for year in [0, 10_000] {
        let mut value = draft();
        value.day = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
        assert!(matches!(
            store.create_plan(&value, 100),
            Err(Error::Validation(_))
        ));
    }
    assert!(store.tasks(100).unwrap().is_empty());
    let mut value = draft();
    value.start_minute = 0;
    value.end_minute = 1440;
    value.note.clear();
    let plan = store.create_plan(&value, 100).unwrap();
    assert_eq!(plan.end_minute, 1440);
    assert!(plan.note.is_empty());
}

#[test]
fn existing_task_association_and_occurrence_title_are_preserved_independently() {
    let (_directory, mut store) = database();
    let task = store.create_task("Shared task", 1, 100).unwrap();
    let other = store.create_task("Different task", 1, 100).unwrap();
    let session = store.start_task(task.id, 110, None).unwrap();
    let mut value = draft();
    value.task_id = Some(task.id);
    let plan = store.create_plan(&value, 120).unwrap();
    value.title = "This occurrence only".into();
    value.start_minute = 14 * 60;
    value.end_minute = 15 * 60;
    let updated = store.update_plan(plan.id, &value).unwrap();
    assert_eq!(updated.title, value.title);
    assert_eq!(updated.task_id, task.id);
    assert_eq!(store.active_sessions().unwrap(), vec![session]);
    assert_eq!(
        store
            .tasks(130)
            .unwrap()
            .iter()
            .find(|task| task.id == plan.task_id)
            .unwrap()
            .title,
        "Shared task"
    );
    value.task_id = Some(other.id);
    assert!(matches!(
        store.update_plan(plan.id, &value),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(store.plans_for_day(day()).unwrap(), vec![updated]);
}

#[test]
fn occurrence_completion_never_completes_or_pauses_its_task() {
    let (_directory, mut store) = database();
    let plan = store.create_plan(&draft(), 100).unwrap();
    let session = store.start_task(plan.task_id, 110, Some(3600)).unwrap();
    store.set_plan_completed(plan.id, true, 120).unwrap();
    store.set_plan_completed(plan.id, true, 140).unwrap();
    assert_eq!(
        store.plans_for_day(day()).unwrap()[0].completed_at,
        Some(120)
    );
    assert_eq!(store.active_sessions().unwrap(), vec![session.clone()]);
    assert_eq!(store.tasks(140).unwrap()[0].completed_at, None);
    store.set_plan_completed(plan.id, false, 150).unwrap();
    assert_eq!(store.plans_for_day(day()).unwrap()[0].completed_at, None);
    assert_eq!(store.active_sessions().unwrap(), vec![session]);
    assert!(matches!(
        store.set_plan_completed(plan.id, true, 90),
        Err(Error::InvalidTransition)
    ));
}

#[test]
fn postponing_leaves_a_marker_and_does_not_copy_or_rewrite_actual_work() {
    let (_directory, mut store) = database();
    let plan = store.create_plan(&draft(), 100).unwrap();
    let session = store.start_task(plan.task_id, 110, None).unwrap();
    store.pause(session.id, 150).unwrap();
    let before = store.sessions().unwrap();
    let tomorrow = day().succ_opt().unwrap();
    let next = store.postpone_plan(plan.id, tomorrow, 160).unwrap();
    assert_ne!(next.id, plan.id);
    assert_eq!(next.task_id, plan.task_id);
    assert_eq!(next.note, plan.note);
    assert_eq!(next.completed_at, None);
    assert_eq!(next.day, tomorrow);
    assert_eq!(
        store.plans_for_day(day()).unwrap()[0].moved_to,
        Some(next.id)
    );
    assert_eq!(store.plans_for_day(tomorrow).unwrap(), vec![next]);
    assert_eq!(store.sessions().unwrap(), before);
    assert_eq!(store.stats(0, 300, 200).unwrap().focus_seconds, 40);
    assert!(matches!(
        store.postpone_plan(plan.id, tomorrow, 170),
        Err(Error::InvalidTransition)
    ));
    assert!(matches!(
        store.update_plan(plan.id, &draft()),
        Err(Error::InvalidTransition)
    ));
}

#[test]
fn failed_postpone_or_update_keeps_both_plan_and_timer_unchanged() {
    let (_directory, mut store) = database();
    let plan = store.create_plan(&draft(), 100).unwrap();
    let session = store.start_task(plan.task_id, 110, None).unwrap();
    store.connection.execute_batch("CREATE TRIGGER fail_plan_update BEFORE UPDATE ON agenda_entries BEGIN SELECT RAISE(ABORT, 'fixture'); END;").unwrap();
    assert!(matches!(
        store.postpone_plan(plan.id, day().succ_opt().unwrap(), 120),
        Err(Error::Database(_))
    ));
    assert_eq!(store.plans_for_day(day()).unwrap(), vec![plan.clone()]);
    assert!(
        store
            .plans_for_day(day().succ_opt().unwrap())
            .unwrap()
            .is_empty()
    );
    let mut changed = draft();
    changed.note = "Changed note".into();
    assert!(matches!(
        store.update_plan(plan.id, &changed),
        Err(Error::Database(_))
    ));
    assert_eq!(store.plans_for_day(day()).unwrap(), vec![plan]);
    assert_eq!(store.active_sessions().unwrap(), vec![session]);
}

#[test]
fn soft_deleting_a_plan_preserves_its_task_and_actual_history() {
    let (_directory, mut store) = database();
    let plan = store.create_plan(&draft(), 100).unwrap();
    let session = store.start_task(plan.task_id, 110, None).unwrap();
    store.finish(session.id, 140).unwrap();
    let before = store.sessions().unwrap();
    store.delete_plan(plan.id).unwrap();
    assert!(store.plans_for_day(day()).unwrap().is_empty());
    assert_eq!(store.sessions().unwrap(), before);
    assert_eq!(store.tasks(150).unwrap().len(), 1);
    assert!(matches!(
        store.set_plan_completed(plan.id, true, 160),
        Err(Error::NotFound)
    ));
}

#[test]
fn completed_tasks_are_queried_by_actual_half_open_day_without_a_plan() {
    let (_directory, mut store) = database();
    let timed = store.create_task("Timed work", 1, 100).unwrap();
    let session = store.start_task(timed.id, 110, None).unwrap();
    store.finish(session.id, 160).unwrap();
    store.complete_task(timed.id, 200).unwrap();
    let untimed = store.create_task("Untimed completion", 1, 100).unwrap();
    store.complete_task(untimed.id, 210).unwrap();
    let next_day = store.create_task("Boundary", 1, 100).unwrap();
    store.complete_task(next_day.id, 300).unwrap();
    let found = store.completed_tasks_between(200, 300, 250).unwrap();
    assert_eq!(
        found.iter().map(|task| task.id).collect::<Vec<_>>(),
        vec![timed.id, untimed.id]
    );
    assert_eq!(found[0].total_seconds, 50);
    assert_eq!(found[0].session_count, 1);
    assert_eq!(found[1].total_seconds, 0);
    assert_eq!(found[1].session_count, 0);
    assert_eq!(
        store.completed_tasks_between(200, 400, 205).unwrap().len(),
        1
    );
    assert!(
        store
            .completed_tasks_between(300, 200, 400)
            .unwrap()
            .is_empty()
    );
    assert!(store.plans_for_day(day()).unwrap().is_empty());
}

#[test]
fn planned_day_query_is_distinct_sorted_and_excludes_deleted_entries() {
    let (_directory, mut store) = database();
    let first = store.create_plan(&draft(), 100).unwrap();
    store.create_plan(&draft(), 101).unwrap();
    let next_day = day().succ_opt().unwrap();
    store.postpone_plan(first.id, next_day, 110).unwrap();
    let mut value = draft();
    value.day = next_day.succ_opt().unwrap();
    let removed = store.create_plan(&value, 120).unwrap();
    store.delete_plan(removed.id).unwrap();
    assert_eq!(
        store.plan_days_between(day(), value.day).unwrap(),
        vec![day(), next_day]
    );
    assert_eq!(
        store.plan_days_between(day(), next_day).unwrap(),
        vec![day()]
    );
    assert!(store.plan_days_between(next_day, day()).unwrap().is_empty());
}

#[test]
fn version_four_migration_preserves_existing_tasks_intervals_and_preferences() {
    let (directory, mut store) = database();
    let task = store.create_task("Before calendar", 1, 100).unwrap();
    let session = store.start_task(task.id, 110, Some(1800)).unwrap();
    store.pause(session.id, 130).unwrap();
    store.set_goal_minutes(90).unwrap();
    let sessions = store.sessions().unwrap();
    store.connection.execute_batch("DROP TABLE agenda_entries; DROP INDEX completed_task_calendar;
        DROP TABLE imported_sessions; ALTER TABLE tags DROP COLUMN class; ALTER TABLE tags DROP COLUMN color;
        ALTER TABLE sessions DROP COLUMN tag_class; ALTER TABLE sessions DROP COLUMN tag_color;
        PRAGMA user_version = 4;").unwrap();
    drop(store);
    let path = directory.path().join("force.sqlite3");
    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.sessions().unwrap(), sessions);
    assert_eq!(store.tasks(140).unwrap()[0].id, task.id);
    assert_eq!(store.goal_minutes().unwrap(), 90);
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        SCHEMA_VERSION
    );
    let plan = store.create_plan(&draft(), 150).unwrap();
    drop(store);
    assert_eq!(
        Store::open(path).unwrap().plans_for_day(day()).unwrap(),
        vec![plan]
    );
}

#[test]
fn failed_calendar_migration_keeps_the_old_schema_version_and_records() {
    let (directory, mut store) = database();
    let task = store.create_task("Keep this record", 1, 100).unwrap();
    let session = store.start_task(task.id, 110, None).unwrap();
    store.finish(session.id, 130).unwrap();
    store.connection.execute_batch("DROP TABLE agenda_entries; DROP INDEX completed_task_calendar; CREATE TABLE agenda_entries (id INTEGER PRIMARY KEY); PRAGMA user_version = 4;").unwrap();
    drop(store);
    let path = directory.path().join("force.sqlite3");
    assert!(matches!(Store::open(&path), Err(Error::Database(_))));
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT title FROM sessions WHERE id = ?1",
                [session.id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "Keep this record"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT ended_at FROM intervals WHERE session_id = ?1",
                [session.id],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        130
    );
}

#[test]
fn final_supported_month_accepts_its_exclusive_year_boundary() {
    let (_directory, mut store) = database();
    let mut value = draft();
    value.day = NaiveDate::from_ymd_opt(9999, 12, 31).unwrap();
    store.create_plan(&value, 100).unwrap();
    assert_eq!(
        store
            .plan_days_between(
                NaiveDate::from_ymd_opt(9999, 12, 1).unwrap(),
                NaiveDate::from_ymd_opt(10000, 1, 1).unwrap()
            )
            .unwrap(),
        vec![value.day]
    );
}

#[test]
fn atomic_plan_edit_preserves_a_second_connection_commit_between_read_and_save() {
    let (directory, mut first) = database();
    let plan = first.create_plan(&draft(), 100).unwrap();
    let session = first.start_task(plan.task_id, 110, None).unwrap();
    let mut second = Store::open(directory.path().join("force.sqlite3")).unwrap();
    let mut theirs = draft();
    theirs.task_id = Some(plan.task_id);
    theirs.note = "Committed from another window".into();
    let external = second.update_plan(plan.id, &theirs).unwrap();
    let mut ours = draft();
    ours.task_id = Some(plan.task_id);
    ours.note = "Keep this unsaved draft".into();
    assert_eq!(
        first
            .update_plan_if_unchanged(plan.id, &ours, &plan)
            .unwrap(),
        None
    );
    assert_eq!(first.plans_for_day(day()).unwrap(), vec![external.clone()]);
    assert_eq!(ours.note, "Keep this unsaved draft");
    assert_eq!(first.active_sessions().unwrap(), vec![session]);
    let saved = first
        .update_plan_if_unchanged(plan.id, &ours, &external)
        .unwrap()
        .unwrap();
    assert_eq!(saved.note, ours.note);
    assert_eq!(second.plans_for_day(day()).unwrap(), vec![saved]);
}

#[test]
fn atomic_plan_edit_does_not_resurrect_a_moved_or_deleted_occurrence() {
    let (directory, mut first) = database();
    let original = first.create_plan(&draft(), 100).unwrap();
    let mut second = Store::open(directory.path().join("force.sqlite3")).unwrap();
    let next = second
        .postpone_plan(original.id, day().succ_opt().unwrap(), 120)
        .unwrap();
    let mut changed = draft();
    changed.task_id = Some(original.task_id);
    changed.note = "Stale draft".into();
    assert_eq!(
        first
            .update_plan_if_unchanged(original.id, &changed, &original)
            .unwrap(),
        None
    );
    let marker = first.plans_for_day(day()).unwrap().remove(0);
    assert_eq!(marker.moved_to, Some(next.id));
    assert!(matches!(
        first.update_plan_if_unchanged(marker.id, &changed, &marker),
        Err(Error::InvalidTransition)
    ));
    let retained = first.create_plan(&draft(), 130).unwrap();
    second.delete_plan(retained.id).unwrap();
    assert_eq!(
        first
            .update_plan_if_unchanged(retained.id, &changed, &retained)
            .unwrap(),
        None
    );
    assert_eq!(first.plans_for_day(day()).unwrap(), vec![marker]);
    assert_eq!(first.plans_for_day(next.day).unwrap(), vec![next]);
}

#[test]
fn month_occurrences_use_half_open_range_and_keep_moved_markers_without_deleted_plans() {
    let (_directory, mut store) = database();
    let original = store.create_plan(&draft(), 100).unwrap();
    let tomorrow = day().succ_opt().unwrap();
    let next = store.postpone_plan(original.id, tomorrow, 110).unwrap();
    let deleted = store.create_plan(&draft(), 120).unwrap();
    store.delete_plan(deleted.id).unwrap();
    let marker = store.plans_for_day(day()).unwrap().remove(0);
    assert_eq!(
        store.plans_between(day(), tomorrow).unwrap(),
        vec![marker.clone()]
    );
    assert_eq!(
        store
            .plans_between(day(), tomorrow.succ_opt().unwrap())
            .unwrap(),
        vec![marker, next.clone()]
    );
    assert_eq!(
        store
            .plans_between(tomorrow, tomorrow.succ_opt().unwrap())
            .unwrap(),
        vec![next]
    );
    assert!(store.plans_between(tomorrow, day()).unwrap().is_empty());
    assert!(store.plans_between(day(), day()).unwrap().is_empty());
}

#[test]
fn month_occurrences_validate_rows_and_accept_final_exclusive_boundary() {
    let (_directory, mut store) = database();
    let mut value = draft();
    value.day = NaiveDate::from_ymd_opt(9999, 12, 31).unwrap();
    let plan = store.create_plan(&value, 100).unwrap();
    let end = NaiveDate::from_ymd_opt(10000, 1, 1).unwrap();
    assert_eq!(
        store.plans_between(value.day, end).unwrap(),
        vec![plan.clone()]
    );
    assert!(matches!(
        store.plans_between(NaiveDate::from_ymd_opt(0, 1, 1).unwrap(), day()),
        Err(Error::Validation(_))
    ));
    store
        .connection
        .execute(
            "UPDATE agenda_entries SET title = '' WHERE id = ?1",
            [plan.id],
        )
        .unwrap();
    assert!(matches!(
        store.plans_between(value.day, end),
        Err(Error::InvalidData)
    ));
}

#[test]
fn activity_aware_edit_is_durable_and_preserves_finished_history_and_interval_rows() {
    let (directory, mut store) = database();
    let selected = store
        .update_tag(2, "Selected activity", TagClass::Neutral, Some("#688BA5"))
        .unwrap();
    let task = store.create_task("Shared task title", 1, 100).unwrap();
    let finished = store.start_task(task.id, 110, None).unwrap();
    store.finish(finished.id, 130).unwrap();
    let finished = store.sessions().unwrap().remove(0);
    let running = store.start_task(task.id, 140, Some(3600)).unwrap();
    store.pause(running.id, 150).unwrap();
    store.resume(running.id, 170).unwrap();
    let before = store.active_sessions().unwrap().remove(0);
    let intervals = stored_interval_rows(&store);
    let mut value = draft();
    value.task_id = Some(task.id);
    let plan = store.create_plan(&value, 180).unwrap();
    value.title = "Only this occurrence".into();
    value.tag_id = 2;
    value.start_minute = 12 * 60;
    value.end_minute = 13 * 60;
    let saved = store
        .update_plan_with_activity_if_unchanged(plan.id, &value, &plan, 1)
        .unwrap()
        .unwrap();
    let updated_task = store.tasks(190).unwrap().remove(0);
    assert_eq!(updated_task.id, task.id);
    assert_eq!(updated_task.title, task.title);
    assert_eq!(updated_task.tag_id, 2);
    assert_eq!(saved.title, value.title);
    let active = store.active_sessions().unwrap().remove(0);
    assert_eq!(active.id, before.id);
    assert_eq!(active.title, task.title);
    assert_eq!(active.tag_id, 2);
    assert_eq!(active.tag_name, selected.name);
    assert_eq!(active.tag_class, selected.class);
    assert_eq!(active.tag_color, selected.color);
    assert_eq!(active.intervals, before.intervals);
    assert_eq!(active.started_at, before.started_at);
    assert_eq!(active.target_seconds, before.target_seconds);
    assert_eq!(active.finished_at, before.finished_at);
    assert_eq!(active.elapsed_at(190), before.elapsed_at(190));
    assert_eq!(stored_interval_rows(&store), intervals);
    assert_eq!(store.sessions().unwrap(), vec![finished.clone()]);
    drop(store);
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert_eq!(store.plans_for_day(day()).unwrap(), vec![saved]);
    assert_eq!(store.tasks(190).unwrap()[0].tag_id, 2);
    assert_eq!(store.active_sessions().unwrap(), vec![active]);
    assert_eq!(store.sessions().unwrap(), vec![finished]);
    assert_eq!(stored_interval_rows(&store), intervals);
}

#[test]
fn activity_aware_edit_rolls_back_task_and_session_changes_if_occurrence_write_fails() {
    let (_directory, mut store) = database();
    let plan = store.create_plan(&draft(), 100).unwrap();
    store.start_task(plan.task_id, 110, None).unwrap();
    let tasks = store.tasks(120).unwrap();
    let active = store.active_sessions().unwrap();
    let intervals = stored_interval_rows(&store);
    store.connection.execute_batch("CREATE TRIGGER fail_activity_plan_update BEFORE UPDATE ON agenda_entries BEGIN SELECT RAISE(ABORT, 'fixture'); END;").unwrap();
    let mut value = draft();
    value.task_id = Some(plan.task_id);
    value.tag_id = 2;
    value.note = "Must roll back together".into();
    assert!(matches!(
        store.update_plan_with_activity_if_unchanged(plan.id, &value, &plan, 1),
        Err(Error::Database(_))
    ));
    assert_eq!(store.plans_for_day(day()).unwrap(), vec![plan]);
    assert_eq!(store.tasks(120).unwrap(), tasks);
    assert_eq!(store.active_sessions().unwrap(), active);
    assert_eq!(stored_interval_rows(&store), intervals);
}

#[test]
fn activity_aware_edit_detects_stale_occurrence_and_activity_before_writing() {
    let (directory, mut first) = database();
    let plan = first.create_plan(&draft(), 100).unwrap();
    let mut second = Store::open(directory.path().join("force.sqlite3")).unwrap();
    let mut ours = draft();
    ours.task_id = Some(plan.task_id);
    ours.tag_id = 3;
    ours.note = "Unsaved draft".into();
    second
        .update_task(plan.task_id, "External task title", 2)
        .unwrap();
    assert_eq!(
        first
            .update_plan_with_activity_if_unchanged(plan.id, &ours, &plan, 1)
            .unwrap(),
        None
    );
    assert_eq!(first.plans_for_day(day()).unwrap(), vec![plan.clone()]);
    assert_eq!(first.tasks(120).unwrap()[0].tag_id, 2);
    assert_eq!(first.tasks(120).unwrap()[0].title, "External task title");
    let mut theirs = draft();
    theirs.task_id = Some(plan.task_id);
    theirs.note = "External occurrence edit".into();
    let external = second.update_plan(plan.id, &theirs).unwrap();
    assert_eq!(
        first
            .update_plan_with_activity_if_unchanged(plan.id, &ours, &plan, 2)
            .unwrap(),
        None
    );
    assert_eq!(first.plans_for_day(day()).unwrap(), vec![external]);
    assert_eq!(first.tasks(120).unwrap()[0].tag_id, 2);
}

#[test]
fn activity_aware_edit_keeps_completed_archived_activity_when_unchanged() {
    let (_directory, mut store) = database();
    let plan = store.create_plan(&draft(), 100).unwrap();
    store.complete_task(plan.task_id, 110).unwrap();
    store.delete_tag(1).unwrap();
    let tasks = store.tasks(120).unwrap();
    let mut value = draft();
    value.task_id = Some(plan.task_id);
    value.note = "A schedule correction only".into();
    let saved = store
        .update_plan_with_activity_if_unchanged(plan.id, &value, &plan, 1)
        .unwrap()
        .unwrap();
    assert_eq!(saved.note, value.note);
    assert_eq!(store.tasks(120).unwrap(), tasks);
    value.tag_id = 2;
    assert!(matches!(
        store.update_plan_with_activity_if_unchanged(plan.id, &value, &saved, 1),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(store.plans_for_day(day()).unwrap(), vec![saved]);
    assert_eq!(store.tasks(120).unwrap(), tasks);
}

#[test]
fn activity_aware_creation_retags_linked_task_without_changing_its_title_or_history() {
    let (directory, mut store) = database();
    let task = store.create_task("Shared task", 1, 100).unwrap();
    let finished = store.start_task(task.id, 110, None).unwrap();
    store.finish(finished.id, 130).unwrap();
    let history = store.sessions().unwrap();
    let mut value = draft();
    value.task_id = Some(task.id);
    value.title = "Independent occurrence title".into();
    value.tag_id = 2;
    let plan = store
        .create_plan_with_activity(&value, 140, Some(1))
        .unwrap()
        .unwrap();
    assert_eq!(plan.task_id, task.id);
    assert_eq!(plan.title, value.title);
    assert_eq!(store.tasks(150).unwrap()[0].title, task.title);
    assert_eq!(store.tasks(150).unwrap()[0].tag_id, 2);
    assert_eq!(store.sessions().unwrap(), history);
    assert!(store.active_sessions().unwrap().is_empty());
    drop(store);
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert_eq!(store.plans_for_day(day()).unwrap(), vec![plan]);
    assert_eq!(store.tasks(150).unwrap()[0].tag_id, 2);
    assert_eq!(store.sessions().unwrap(), history);
}

#[test]
fn activity_aware_creation_detects_stale_activity_and_rolls_back_failed_insert() {
    let (_directory, mut store) = database();
    let task = store.create_task("Shared task", 1, 100).unwrap();
    store.start_task(task.id, 110, None).unwrap();
    let mut value = draft();
    value.task_id = Some(task.id);
    value.tag_id = 3;
    store.update_task(task.id, &task.title, 2).unwrap();
    assert_eq!(
        store
            .create_plan_with_activity(&value, 120, Some(1))
            .unwrap(),
        None
    );
    assert!(store.plans_for_day(day()).unwrap().is_empty());
    assert_eq!(store.tasks(120).unwrap()[0].tag_id, 2);
    let tasks = store.tasks(120).unwrap();
    let active = store.active_sessions().unwrap();
    store.connection.execute_batch("CREATE TRIGGER fail_activity_plan_insert BEFORE INSERT ON agenda_entries BEGIN SELECT RAISE(ABORT, 'fixture'); END;").unwrap();
    assert!(matches!(
        store.create_plan_with_activity(&value, 120, Some(2)),
        Err(Error::Database(_))
    ));
    assert!(store.plans_for_day(day()).unwrap().is_empty());
    assert_eq!(store.tasks(120).unwrap(), tasks);
    assert_eq!(store.active_sessions().unwrap(), active);
}

#[test]
fn activity_aware_creation_without_linked_task_creates_one_task_and_no_timer() {
    let (_directory, mut store) = database();
    let plan = store
        .create_plan_with_activity(&draft(), 100, None)
        .unwrap()
        .unwrap();
    let tasks = store.tasks(110).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, plan.task_id);
    assert_eq!(tasks[0].tag_id, draft().tag_id);
    assert!(store.active_sessions().unwrap().is_empty());
}

#[test]
fn activity_aware_saves_reject_rest_activities_without_reclassifying_legacy_tasks() {
    let (_directory, mut store) = database();
    let plan = store.create_plan(&draft(), 100).unwrap();
    let mut value = draft();
    value.task_id = Some(plan.task_id);
    value.tag_id = 4;
    assert!(matches!(
        store.update_plan_with_activity_if_unchanged(plan.id, &value, &plan, 1),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(store.plans_for_day(day()).unwrap(), vec![plan]);
    let legacy_rest = store.create_task("Legacy rest", 4, 100).unwrap();
    value.task_id = Some(legacy_rest.id);
    value.tag_id = 1;
    assert!(matches!(
        store.create_plan_with_activity(&value, 120, Some(4)),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(
        store
            .tasks(120)
            .unwrap()
            .iter()
            .find(|task| task.id == legacy_rest.id)
            .unwrap()
            .tag_id,
        4
    );
}

fn stored_interval_rows(store: &Store) -> Vec<(i64, i64, i64, Option<i64>)> {
    store
        .connection
        .prepare("SELECT id, session_id, started_at, ended_at FROM intervals ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
}
