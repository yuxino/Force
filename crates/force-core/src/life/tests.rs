use super::*;

fn fixture() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    (directory, store)
}

#[test]
fn life_counts_full_elapsed_days_in_each_interval() {
    let (_directory, mut store) = fixture();
    let start = 23 * 3_600;
    store.create_life_item("喝咖啡", start, start).unwrap();
    let first = store.life_board().unwrap().items.remove(0);
    assert_eq!(first.elapsed_at(start - 1), 0);
    assert_eq!(first.cumulative_days(start + 7_200), 0);
    assert_eq!(first.cumulative_days(start + SECONDS_PER_DAY - 1), 0);
    assert_eq!(first.cumulative_days(start + SECONDS_PER_DAY), 1);

    let second = start + 2 * SECONDS_PER_DAY - 60;
    store
        .record_life_occurrence(&first, second, second)
        .unwrap();
    let item = store.life_board().unwrap().items.remove(0);
    let now = second + SECONDS_PER_DAY + 120;
    assert_eq!(item.elapsed_at(now), SECONDS_PER_DAY + 120);
    assert_eq!(item.cumulative_days(now), 2);
    assert_eq!(
        item.recent_spans(now, 8),
        [
            LifeSpan {
                start: LifeOccurrence::Exact(start),
                end: Some(LifeOccurrence::Exact(second)),
                seconds: second - start,
            },
            LifeSpan {
                start: LifeOccurrence::Exact(second),
                end: None,
                seconds: now - second,
            },
        ]
    );
    assert_eq!(item.recent_spans(now, 1), item.recent_spans(now, 8)[1..]);
}

#[test]
fn life_rejects_future_and_stale_edits_and_persists_preferences() {
    let (directory, mut store) = fixture();
    assert!(matches!(
        store.create_life_item("运动", 101, 100),
        Err(Error::Validation(_))
    ));
    assert!(store.life_board().unwrap().items.is_empty());

    let id = store.create_life_item("  看短视频  ", 80, 100).unwrap();
    let original = store.life_board().unwrap().items.remove(0);
    assert_eq!(original.title, "看短视频");
    let mut other = Store::open(directory.path().join("force.sqlite3")).unwrap();
    other.record_life_occurrence(&original, 90, 100).unwrap();
    assert!(matches!(
        store.edit_life_item(&original, "刷短视频", 85, 100),
        Err(Error::InvalidTransition)
    ));
    let latest = store.life_board().unwrap().items.remove(0);
    assert_eq!(latest.title, original.title);
    assert!(matches!(
        store.edit_life_item(&latest, "刷短视频", 80, 100),
        Err(Error::Validation(_))
    ));
    store.edit_life_item(&latest, "刷短视频", 88, 100).unwrap();
    store.set_life_module(LifeModule::Table, false).unwrap();
    other.set_life_module(LifeModule::Total, false).unwrap();
    store.set_life_chart_item(id).unwrap();
    drop(store);
    drop(other);

    let mut reopened = Store::open(directory.path().join("force.sqlite3")).unwrap();
    let board = reopened.life_board().unwrap();
    assert!(!board.shows(LifeModule::Table));
    assert!(board.shows(LifeModule::Trend));
    assert!(!board.shows(LifeModule::Total));
    assert_eq!(board.chart_item, Some(id));
    assert_eq!(board.items[0].title, "刷短视频");
    assert_eq!(
        board.items[0].occurrences,
        [LifeOccurrence::Exact(80), LifeOccurrence::Exact(88)]
    );
    reopened.delete_life_item(&board.items[0]).unwrap();
    let board = reopened.life_board().unwrap();
    assert_eq!(board.chart_item, None);
    assert!(board.items.is_empty());
}

#[test]
fn life_v8_upgrade_preserves_timer_and_preferences() {
    let (directory, mut store) = fixture();
    let session = store.start("阅读", 3, 100).unwrap();
    store.pause(session.id, 160).unwrap();
    store.set_goal_minutes(240).unwrap();
    store
        .connection
        .execute_batch(
            "DROP TABLE life_board;
             DROP TABLE life_occurrences;
             DROP TABLE life_items;
             PRAGMA user_version = 8;",
        )
        .unwrap();
    drop(store);

    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    let saved = &store.active_sessions().unwrap()[0];
    assert_eq!(saved.id, session.id);
    assert!(saved.is_paused());
    assert_eq!(saved.elapsed_at(900), 60);
    assert_eq!(store.goal_minutes().unwrap(), 240);
    let board = store.life_board().unwrap();
    assert!(board.items.is_empty());
    assert!(board.shows(LifeModule::Table));
    assert!(board.shows(LifeModule::Trend));
    assert!(board.shows(LifeModule::Total));
}

#[test]
fn date_only_occurrences_keep_uncertainty_when_mixed_with_exact_records() {
    let (directory, mut store) = fixture();
    let date = NaiveDate::from_ymd_opt(2026, 2, 2).unwrap();
    let start = 10 * SECONDS_PER_DAY;
    let only_date = LifeOccurrence::Day {
        date,
        earliest: start,
        latest: start + SECONDS_PER_DAY,
    };
    let now = start + 2 * SECONDS_PER_DAY + 3_600;
    store.create_life_item("阅读", only_date, now).unwrap();
    let item = store.life_board().unwrap().items.remove(0);
    assert_eq!(item.last_occurrence(), only_date);
    assert!(item.recent_spans(now, 1)[0].is_estimated());
    assert_eq!(item.cumulative_days(now), 1);
    store.record_life_occurrence(&item, now, now).unwrap();
    drop(store);

    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    let item = store.life_board().unwrap().items.remove(0);
    assert_eq!(item.last_occurrence(), LifeOccurrence::Exact(now));
    assert_eq!(item.cumulative_days(now), 1);
    assert!(item.has_date_only_entries());
    assert!(item.recent_spans(now, 2)[0].is_estimated());
    assert!(!item.recent_spans(now, 1)[0].is_estimated());
}
