use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

use force_core::Store;

// Run in a separate test process so the parent can terminate it while the
// database is open, without running Store/Connection destructors.
#[test]
fn interrupted_writer() {
    let Some(path) = std::env::var_os("FORCE_INTERRUPTION_TEST_DB") else {
        return;
    };
    let mut store = Store::open(path).unwrap();
    let paused = store.create_task("Paused task", 1, 100).unwrap();
    let paused = store.start_task(paused.id, 100, Some(2700)).unwrap();
    store.pause(paused.id, 130).unwrap();
    let running = store.create_task("Running task", 1, 140).unwrap();
    store.start_task(running.id, 140, Some(1500)).unwrap();
    let finished = store.create_task("Finished task", 1, 150).unwrap();
    store.complete_task(finished.id, 160).unwrap();
    println!("WRITES_COMMITTED");
    std::io::stdout().flush().unwrap();
    // The parent closes stdin after killing us; no sleeps or polling needed.
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    drop(store);
}

#[test]
fn committed_tasks_and_timers_survive_forced_process_termination() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("force.sqlite3");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "interrupted_writer", "--nocapture"])
        .env("FORCE_INTERRUPTION_TEST_DB", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let ready = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .any(|line| line.unwrap() == "WRITES_COMMITTED");
    if !ready {
        let status = child.wait().unwrap();
        panic!("writer exited before committing: {status}");
    }
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());

    let mut store = Store::open(&path).unwrap();
    let tasks = store.tasks(200).unwrap();
    assert_eq!(tasks.len(), 3);
    assert!(
        tasks
            .iter()
            .find(|t| t.title == "Finished task")
            .unwrap()
            .completed_at
            .is_some()
    );
    let active = store.active_sessions().unwrap();
    assert_eq!(active.len(), 2);
    let paused = active.iter().find(|s| s.title == "Paused task").unwrap();
    assert!(paused.is_paused());
    assert_eq!(paused.elapsed_at(5000), 30);
    assert_eq!(paused.target_seconds, Some(2700));
    let running = active.iter().find(|s| s.title == "Running task").unwrap();
    assert!(running.is_running());
    assert_eq!(running.elapsed_at(200), 60);
    assert_eq!(running.target_seconds, Some(1500));
    let done = store.finish_exact_target(running.id, 5000).unwrap();
    assert_eq!(done.finished_at, Some(1640));
    assert_eq!(done.elapsed_at(5000), 1500);
    drop(store);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.tasks(5000).unwrap().len(), 3);
    assert_eq!(store.sessions().unwrap(), vec![done]);
    assert_eq!(store.active_sessions().unwrap().len(), 1);
}
