use std::collections::HashMap;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub tag_id: i64,
    pub tag_name: String,
    pub tag_kind: TagKind,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub total_seconds: i64,
    /// Includes a currently running or paused session.
    pub session_count: usize,
}

pub(super) const SCHEMA: &str = "
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    tag_id INTEGER NOT NULL REFERENCES tags(id),
    tag_name TEXT NOT NULL,
    tag_kind TEXT NOT NULL CHECK (tag_kind IN ('focus', 'rest')),
    created_at INTEGER NOT NULL,
    completed_at INTEGER CHECK (completed_at IS NULL OR completed_at >= created_at),
    deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
);
CREATE INDEX task_list ON tasks(deleted, completed_at, id DESC);
CREATE INDEX task_sessions ON sessions(task_id, started_at);
CREATE UNIQUE INDEX one_unfinished_task_session ON sessions(task_id)
    WHERE task_id IS NOT NULL AND finished_at IS NULL;
";

pub(super) fn migrate_v2(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        2 => {
            transaction.execute_batch(
                "ALTER TABLE sessions ADD COLUMN task_id INTEGER REFERENCES tasks(id) ON DELETE RESTRICT;",
            )?;
            transaction.execute_batch(SCHEMA)?;
            transaction.pragma_update(None, "user_version", 3)?;
        }
        3 | SCHEMA_VERSION => (),
        _ => return Err(Error::UnsupportedSchema),
    }
    transaction.commit()?;
    Ok(())
}

impl Store {
    /// Returns saved tasks, unfinished first. Elapsed totals include each task's
    /// own sessions and exclude all paused gaps; tasks persist across days.
    pub fn tasks(&self, now: i64) -> Result<Vec<Task>> {
        let transaction = self.connection.unchecked_transaction()?;
        let tasks = load_tasks(&transaction, now)?;
        transaction.commit()?;
        Ok(tasks)
    }

    pub fn create_task(&mut self, title: &str, tag_id: i64, now: i64) -> Result<Task> {
        let title = valid_text(title, 200, "Enter a task title of 1–200 characters")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let tag = available_tag(&transaction, tag_id)?;
        transaction.execute(
            "INSERT INTO tasks (title, tag_id, tag_name, tag_kind, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![title, tag.id, tag.name, tag.kind.as_str(), now],
        )?;
        let task = Task {
            id: transaction.last_insert_rowid(),
            title: title.to_owned(),
            tag_id,
            tag_name: tag.name,
            tag_kind: tag.kind,
            created_at: now,
            completed_at: None,
            total_seconds: 0,
            session_count: 0,
        };
        transaction.commit()?;
        Ok(task)
    }

    /// Edits an unfinished task and its unfinished session. Reopen completed
    /// tasks before editing. End an existing round before changing its kind.
    pub fn update_task(&mut self, id: i64, title: &str, tag_id: i64) -> Result<()> {
        let title = valid_text(title, 200, "Enter a task title of 1–200 characters")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = task_record(&transaction, id)?;
        if task.completed_at.is_some() {
            return Err(Error::InvalidTransition);
        }
        let tag = available_tag(&transaction, tag_id)?;
        if unfinished_session(&transaction, id)?.is_some_and(|session| session.tag_kind != tag.kind)
        {
            return Err(Error::InvalidTransition);
        }
        transaction.execute(
            "UPDATE tasks SET title = ?1, tag_id = ?2, tag_name = ?3, tag_kind = ?4 WHERE id = ?5",
            params![title, tag.id, tag.name, tag.kind.as_str(), id],
        )?;
        transaction.execute(
            "UPDATE sessions SET title = ?1, tag_id = ?2, tag_name = ?3, tag_kind = ?4 WHERE task_id = ?5 AND finished_at IS NULL",
            params![title, tag.id, tag.name, tag.kind.as_str(), id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Completing a task is explicit. Ending a session or reaching a timer
    /// target never marks the task complete.
    pub fn complete_task(&mut self, id: i64, now: i64) -> Result<Task> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = task_record(&transaction, id)?;
        if task.completed_at.is_some() || now < task.created_at {
            return Err(Error::InvalidTransition);
        }
        if let Some(session) = unfinished_session(&transaction, id)? {
            end_session(&transaction, &session, now)?;
        }
        transaction.execute(
            "UPDATE tasks SET completed_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        let task = load_tasks(&transaction, now)?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or(Error::NotFound)?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn reopen_task(&mut self, id: i64) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = task_record(&transaction, id)?;
        if task.completed_at.is_none() {
            return Err(Error::InvalidTransition);
        }
        transaction.execute(
            "UPDATE tasks SET completed_at = NULL,
             tag_name = (SELECT name FROM tags WHERE tags.id = tasks.tag_id),
             tag_kind = (SELECT kind FROM tags WHERE tags.id = tasks.tag_id)
             WHERE id = ?1",
            [id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Hides a saved task without deleting its session history. End its active
    /// session first so deletion can never silently leave a timer running.
    pub fn delete_task(&mut self, id: i64) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        task_record(&transaction, id)?;
        if unfinished_session(&transaction, id)?.is_some() {
            return Err(Error::InvalidTransition);
        }
        transaction.execute("UPDATE tasks SET deleted = 1 WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
    }

    /// Reuses a paused session for the same task, preserving its original timer
    /// target. Starting focus atomically pauses other focus and ends running
    /// rest, so a break never continues hidden behind the selected focus task.
    pub fn start_task(&mut self, id: i64, now: i64, target: Option<i64>) -> Result<Session> {
        if target.is_some_and(|value| !(MIN_TARGET_SECONDS..=MAX_TARGET_SECONDS).contains(&value)) {
            return Err(Error::Validation(
                "Timer duration must be between 1 minute and 24 hours",
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = task_record(&transaction, id)?;
        if task.completed_at.is_some() || now < task.created_at {
            return Err(Error::InvalidTransition);
        }
        let tag = available_tag(&transaction, task.tag_id)?;
        let existing = unfinished_session(&transaction, id)?;
        if existing
            .as_ref()
            .is_some_and(|session| session.is_running() || now < last_boundary(session))
        {
            return Err(Error::InvalidTransition);
        }
        if tag.kind == TagKind::Focus {
            let active = load_sessions_in_snapshot(&transaction, "WHERE finished_at IS NULL", [])?;
            pause_for_focus(
                &transaction,
                &active,
                existing.as_ref().map(|session| session.id),
                now,
            )?;
        }
        let session_id = if let Some(session) = existing {
            if session
                .target_seconds
                .is_some_and(|value| session.elapsed_at(now) >= value)
            {
                return Err(Error::InvalidTransition);
            }
            transaction.execute(
                "INSERT INTO intervals (session_id, started_at) VALUES (?1, ?2)",
                params![session.id, now],
            )?;
            session.id
        } else {
            transaction.execute(
                "INSERT INTO sessions (task_id, title, tag_id, tag_name, tag_kind, started_at, target_seconds) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, task.title, tag.id, tag.name, tag.kind.as_str(), now, target],
            )?;
            let session_id = transaction.last_insert_rowid();
            transaction.execute(
                "INSERT INTO intervals (session_id, started_at) VALUES (?1, ?2)",
                params![session_id, now],
            )?;
            session_id
        };
        let session = load_session(&transaction, session_id)?;
        transaction.commit()?;
        Ok(session)
    }

    /// Resumes an existing focus round without changing its identity or timer
    /// target. Legacy rounds without a task use the same atomic switching policy
    /// as task focus: pause other focus and finish running rest.
    pub fn resume_focus(&mut self, id: i64, now: i64) -> Result<Session> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = load_session(&transaction, id)?;
        if session.tag_kind != TagKind::Focus
            || !session.is_paused()
            || now < last_boundary(&session)
            || session
                .target_seconds
                .is_some_and(|target| session.elapsed_at(now) >= target)
        {
            return Err(Error::InvalidTransition);
        }
        let active = load_sessions_in_snapshot(&transaction, "WHERE finished_at IS NULL", [])?;
        pause_for_focus(&transaction, &active, Some(id), now)?;
        transaction.execute(
            "INSERT INTO intervals (session_id, started_at) VALUES (?1, ?2)",
            params![id, now],
        )?;
        let session = load_session(&transaction, id)?;
        transaction.commit()?;
        Ok(session)
    }

    /// Resumes the requested rest session without letting focus continue
    /// behind it. Keep its original identity and timer mode, including legacy
    /// untimed rest sessions.
    pub fn resume_break(&mut self, id: i64, now: i64) -> Result<Session> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = load_session(&transaction, id)?;
        if session.tag_kind != TagKind::Rest
            || !session.is_paused()
            || now < last_boundary(&session)
            || session
                .target_seconds
                .is_some_and(|target| session.elapsed_at(now) >= target)
        {
            return Err(Error::InvalidTransition);
        }
        let active = load_sessions_in_snapshot(&transaction, "WHERE finished_at IS NULL", [])?;
        pause_for_break(&transaction, &active, Some(id), now)?;
        transaction.execute(
            "INSERT INTO intervals (session_id, started_at) VALUES (?1, ?2)",
            params![id, now],
        )?;
        let session = load_session(&transaction, id)?;
        transaction.commit()?;
        Ok(session)
    }

    /// Starts one dedicated break, or resumes the existing break without
    /// resetting its duration. The focus-to-break transition is atomic.
    pub fn start_break(&mut self, tag_id: i64, now: i64, target: i64) -> Result<Session> {
        if !(MIN_TARGET_SECONDS..=MAX_TARGET_SECONDS).contains(&target) {
            return Err(Error::Validation(
                "Timer duration must be between 1 minute and 24 hours",
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let tag = available_tag(&transaction, tag_id)?;
        if tag.kind != TagKind::Rest {
            return Err(Error::InvalidTransition);
        }
        let active = load_sessions_in_snapshot(&transaction, "WHERE finished_at IS NULL", [])?;
        let existing = active
            .iter()
            .filter(|session| {
                session.tag_kind == TagKind::Rest
                    && session.task_id.is_none()
                    && session
                        .target_seconds
                        .is_some_and(|limit| session.elapsed_at(now) < limit)
            })
            .max_by_key(|session| (session.is_running(), session.id));
        pause_for_break(
            &transaction,
            &active,
            existing.map(|session| session.id),
            now,
        )?;
        let id = if let Some(existing) = existing {
            if existing.is_paused() {
                transaction.execute(
                    "INSERT INTO intervals (session_id, started_at) VALUES (?1, ?2)",
                    params![existing.id, now],
                )?;
            }
            existing.id
        } else {
            transaction.execute(
                "INSERT INTO sessions (title, tag_id, tag_name, tag_kind, started_at, target_seconds) VALUES ('休息', ?1, ?2, 'rest', ?3, ?4)",
                params![tag.id, tag.name, now, target],
            )?;
            let id = transaction.last_insert_rowid();
            transaction.execute(
                "INSERT INTO intervals (session_id, started_at) VALUES (?1, ?2)",
                params![id, now],
            )?;
            id
        };
        let session = load_session(&transaction, id)?;
        transaction.commit()?;
        Ok(session)
    }
}

fn pause_for_focus(
    transaction: &Transaction<'_>,
    active: &[Session],
    selected: Option<i64>,
    now: i64,
) -> Result<()> {
    for session in active
        .iter()
        .filter(|session| Some(session.id) != selected && session.is_running())
    {
        if session.tag_kind == TagKind::Rest
            || session
                .target_seconds
                .is_some_and(|target| session.elapsed_at(now) >= target)
        {
            end_session(transaction, session, now)?;
        } else {
            close_open_interval(transaction, session, now)?;
        }
    }
    Ok(())
}

fn pause_for_break(
    transaction: &Transaction<'_>,
    active: &[Session],
    selected: Option<i64>,
    now: i64,
) -> Result<()> {
    for session in active {
        if now < last_boundary(session) {
            return Err(Error::InvalidTransition);
        }
        if Some(session.id) == selected {
            continue;
        }
        if session.tag_kind == TagKind::Rest
            || session
                .target_seconds
                .is_some_and(|limit| session.elapsed_at(now) >= limit)
        {
            end_session(transaction, session, now)?;
        } else if session.is_running() {
            close_open_interval(transaction, session, now)?;
        }
    }
    Ok(())
}

fn available_tag(connection: &Connection, id: i64) -> Result<Tag> {
    let (name, kind, archived): (String, String, bool) = connection
        .query_row(
            "SELECT name, kind, archived FROM tags WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(Error::NotFound)?;
    if archived {
        return Err(Error::ArchivedTag);
    }
    Ok(Tag {
        id,
        name,
        kind: TagKind::parse(&kind)?,
    })
}

fn task_record(connection: &Connection, id: i64) -> Result<Task> {
    let record = connection.query_row(
        "SELECT title, tag_id, tag_name, tag_kind, created_at, completed_at FROM tasks WHERE id = ?1 AND deleted = 0", [id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, i64>(4)?, row.get::<_, Option<i64>>(5)?)),
    ).optional()?.ok_or(Error::NotFound)?;
    Ok(Task {
        id,
        title: record.0,
        tag_id: record.1,
        tag_name: record.2,
        tag_kind: TagKind::parse(&record.3)?,
        created_at: record.4,
        completed_at: record.5,
        total_seconds: 0,
        session_count: 0,
    })
}

fn unfinished_session(connection: &Connection, task_id: i64) -> Result<Option<Session>> {
    let id: Option<i64> = connection
        .query_row(
            "SELECT id FROM sessions WHERE task_id = ?1 AND finished_at IS NULL",
            [task_id],
            |row| row.get(0),
        )
        .optional()?;
    id.map(|id| load_session(connection, id)).transpose()
}

fn end_session(transaction: &Transaction<'_>, session: &Session, now: i64) -> Result<()> {
    if session.finished_at.is_some() || now < last_boundary(session) {
        return Err(Error::InvalidTransition);
    }
    if session
        .target_seconds
        .is_some_and(|value| session.elapsed_at(now) >= value)
    {
        finish_at_target(transaction, session, now)?;
    } else {
        if session.is_running() {
            close_open_interval(transaction, session, now)?;
        }
        transaction.execute(
            "UPDATE sessions SET finished_at = ?1 WHERE id = ?2",
            params![now, session.id],
        )?;
    }
    Ok(())
}

fn load_tasks(connection: &Connection, now: i64) -> Result<Vec<Task>> {
    let mut statement = connection.prepare(
        "SELECT id FROM tasks WHERE deleted = 0 ORDER BY completed_at IS NOT NULL, id DESC",
    )?;
    let ids = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut tasks = ids
        .into_iter()
        .map(|id| task_record(connection, id))
        .collect::<Result<Vec<_>>>()?;
    let positions: HashMap<_, _> = tasks
        .iter()
        .enumerate()
        .map(|(index, task)| (task.id, index))
        .collect();
    let mut counts = connection.prepare(
        "SELECT task_id, COUNT(*) FROM sessions WHERE task_id IS NOT NULL GROUP BY task_id",
    )?;
    let rows = counts.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?;
    for row in rows {
        let (id, count) = row?;
        if let Some(index) = positions.get(&id) {
            tasks[*index].session_count = usize::try_from(count).map_err(|_| Error::InvalidData)?;
        }
    }
    let mut intervals = connection.prepare("SELECT s.task_id, i.started_at, i.ended_at FROM intervals i JOIN sessions s ON s.id = i.session_id JOIN tasks t ON t.id = s.task_id WHERE t.deleted = 0")?;
    let rows = intervals.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;
    for row in rows {
        let (id, start, end) = row?;
        if let Some(index) = positions.get(&id) {
            tasks[*index].total_seconds = tasks[*index]
                .total_seconds
                .saturating_add(end.unwrap_or(now).saturating_sub(start).max(0));
        }
    }
    Ok(tasks)
}

#[cfg(test)]
mod tests;
