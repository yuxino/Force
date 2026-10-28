use super::*;

fn database() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    (directory, store)
}

#[test]
fn shared_colors_persist_and_follow_categories_without_rewriting_records() {
    let (directory, mut store) = database();
    assert_eq!(store.class_colors().unwrap(), ClassColors::default());
    let inherited = store.create_tag("Inherited", TagKind::Focus).unwrap();
    let custom = store
        .create_tag_with_details(
            "Custom",
            TagKind::Focus,
            TagClass::Positive,
            Some("#123456"),
        )
        .unwrap();
    let task = store
        .create_task("Keep history", inherited.id, 100)
        .unwrap();
    let session = store.start_task(task.id, 100, None).unwrap();
    store.finish(session.id, 160).unwrap();
    let notes = store.create_moment("A point", 150, 160).unwrap();
    let tags = store.tags().unwrap();
    let sessions = store.sessions().unwrap();
    let stats = store.stats(0, 300, 300).unwrap();
    store
        .set_class_color(TagClass::Positive, Some("#6c9079"))
        .unwrap();
    store
        .set_class_color(TagClass::Negative, Some("#B56F67"))
        .unwrap();
    let colors = store.class_colors().unwrap();
    assert_eq!(
        colors.resolve(inherited.class, inherited.color.as_deref()),
        Some("#6C9079")
    );
    assert_eq!(
        colors.resolve(custom.class, custom.color.as_deref()),
        Some("#123456")
    );
    assert_eq!(colors.resolve(TagClass::Legacy, None), None);
    assert_eq!(
        colors.resolve(TagClass::Legacy, Some("#858585")),
        Some("#858585")
    );
    assert_eq!(store.tags().unwrap(), tags);
    assert_eq!(store.sessions().unwrap(), sessions);
    assert_eq!(store.stats(0, 300, 300).unwrap(), stats);
    assert_eq!(store.moments_between(0, 300).unwrap(), vec![notes]);
    drop(store);
    let mut store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert_eq!(store.class_colors().unwrap(), colors);
    store.set_class_color(TagClass::Positive, None).unwrap();
    let cleared = store.class_colors().unwrap();
    assert_eq!(cleared.resolve(inherited.class, None), None);
    assert_eq!(
        cleared.resolve(custom.class, custom.color.as_deref()),
        Some("#123456")
    );
    assert_eq!(cleared.negative, colors.negative);
}

#[test]
fn invalid_color_or_legacy_category_cannot_overwrite_a_saved_preference() {
    let (_directory, mut store) = database();
    store
        .set_class_color(TagClass::Positive, Some("#123456"))
        .unwrap();
    for color in ["", "red", "#abc", "#12345G", "#1234567"] {
        assert!(matches!(
            store.set_class_color(TagClass::Positive, Some(color)),
            Err(Error::Validation(_))
        ));
    }
    assert!(matches!(
        store.set_class_color(TagClass::Legacy, None),
        Err(Error::Validation(_))
    ));
    assert_eq!(
        store.class_colors().unwrap().positive.as_deref(),
        Some("#123456")
    );
}

#[test]
fn v7_upgrade_preserves_notes_tags_and_timer_history() {
    let (directory, mut store) = database();
    let tag = store
        .create_tag_with_details(
            "Custom",
            TagKind::Focus,
            TagClass::Negative,
            Some("#B56F67"),
        )
        .unwrap();
    let task = store.create_task("Task", tag.id, 100).unwrap();
    let session = store.start_task(task.id, 100, None).unwrap();
    store.finish(session.id, 160).unwrap();
    let moment = store.create_moment("Preserved", 150, 170).unwrap();
    let tags = store.tags().unwrap();
    let sessions = store.sessions().unwrap();
    store
        .connection
        .execute_batch("DROP TABLE class_colors; PRAGMA user_version=7;")
        .unwrap();
    drop(store);
    let store = Store::open(directory.path().join("force.sqlite3")).unwrap();
    assert_eq!(store.class_colors().unwrap(), ClassColors::default());
    assert_eq!(store.tags().unwrap(), tags);
    assert_eq!(store.sessions().unwrap(), sessions);
    assert_eq!(store.moments_between(0, 300).unwrap(), vec![moment]);
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        SCHEMA_VERSION
    );
}

#[test]
fn malformed_existing_category_table_rolls_back_upgrade() {
    let (directory, store) = database();
    store.connection.execute_batch("DROP TABLE class_colors; CREATE TABLE class_colors (class TEXT PRIMARY KEY, color TEXT); INSERT INTO class_colors VALUES ('positive', 'invalid'); PRAGMA user_version=7;").unwrap();
    drop(store);
    let path = directory.path().join("force.sqlite3");
    assert!(matches!(Store::open(&path), Err(Error::InvalidData)));
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        7
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM class_colors", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn concurrent_v7_upgrade_rechecks_version_after_acquiring_write_lock() {
    let (directory, store) = database();
    store
        .connection
        .execute_batch("DROP TABLE class_colors; PRAGMA user_version=7;")
        .unwrap();
    drop(store);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let path = directory.path().join("force.sqlite3");
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                Store::open(path).unwrap().class_colors().unwrap()
            })
        })
        .collect();
    for thread in threads {
        assert_eq!(thread.join().unwrap(), ClassColors::default());
    }
}
