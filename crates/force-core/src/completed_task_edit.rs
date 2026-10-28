//! Explicit edits to task completion markers, independent of recorded work.

use chrono::Datelike;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedTaskEditSnapshot {
    pub task: Task,
    baseline: CompletionBaseline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompletionBaseline {
    task: Task,
    sessions: Vec<Session>,
    interval_ids: Vec<i64>,
    plans: Vec<CompletionPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompletionPlan {
    id: i64,
    title: String,
    day: String,
    start: i64,
    end: i64,
    note: String,
    completed_at: Option<i64>,
    moved_to: Option<i64>,
    created_at: i64,
    deleted: bool,
}

impl Store {
    pub fn completed_task_edit_snapshot(&self, task_id: i64) -> Result<CompletedTaskEditSnapshot> {
        let transaction = self.connection.unchecked_transaction()?;
        let baseline = completion_baseline(&transaction, task_id)?;
        let snapshot = CompletedTaskEditSnapshot {
            task: baseline.task.clone(),
            baseline,
        };
        transaction.commit()?;
        Ok(snapshot)
    }

    /// Changes the completion marker only. Existing rounds keep their titles,
    /// activity snapshots, intervals and timestamps; plans keep their own times.
    pub fn update_completed_task(
        &mut self,
        expected: &CompletedTaskEditSnapshot,
        description: &str,
        tag_id: i64,
        completed_at: i64,
        now: i64,
    ) -> Result<Task> {
        for stamp in [completed_at, now] {
            if DateTime::<Utc>::from_timestamp(stamp, 0)
                .is_none_or(|date| !(1..=9999).contains(&date.year()))
            {
                return Err(Error::Validation("Choose a valid completion date and time"));
            }
        }
        if completed_at > now {
            return Err(Error::Validation("Task completion cannot be in the future"));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = verify_completion(&transaction, expected)?;
        if completed_at < current.task.created_at
            || current
                .sessions
                .iter()
                .any(|session| session.finished_at.is_none_or(|end| end > completed_at))
        {
            return Err(Error::Validation(
                "Task completion must follow its creation and recorded work",
            ));
        }
        let (name, kind) = if tag_id == current.task.tag_id {
            (current.task.tag_name.clone(), current.task.tag_kind)
        } else {
            let tag = task::available_tag(&transaction, tag_id)?;
            if tag.kind != TagKind::Focus {
                return Err(Error::Validation("Choose a focus activity for this task"));
            }
            (tag.name, tag.kind)
        };
        let title = valid_text(
            if description.trim().is_empty() {
                &name
            } else {
                description
            },
            200,
            "Enter a description of at most 200 characters",
        )?;
        transaction.execute(
            "UPDATE tasks SET title=?1,tag_id=?2,tag_name=?3,tag_kind=?4,completed_at=?5 WHERE id=?6 AND deleted=0",
            params![title, tag_id, name, kind.as_str(), completed_at, current.task.id],
        )?;
        let saved = Task {
            title: title.to_owned(),
            tag_id,
            tag_name: name,
            tag_kind: kind,
            completed_at: Some(completed_at),
            ..current.task
        };
        transaction.commit()?;
        Ok(saved)
    }

    /// Removes the completion and its arrangements from the calendar, retaining
    /// all recorded work, imported provenance and links to this soft-deleted task.
    pub fn delete_completed_task(&mut self, expected: &CompletedTaskEditSnapshot) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = verify_completion(&transaction, expected)?;
        transaction.execute("UPDATE tasks SET deleted=1 WHERE id=?1", [current.task.id])?;
        transaction.execute(
            "UPDATE agenda_entries SET deleted=1 WHERE task_id=?1",
            [current.task.id],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn verify_completion(
    connection: &Connection,
    expected: &CompletedTaskEditSnapshot,
) -> Result<CompletionBaseline> {
    let current = completion_baseline(connection, expected.baseline.task.id)?;
    if current != expected.baseline || expected.task != expected.baseline.task {
        return Err(Error::Validation(
            "This completion changed while editing; reopen it before saving",
        ));
    }
    Ok(current)
}

fn completion_baseline(connection: &Connection, id: i64) -> Result<CompletionBaseline> {
    let (title, tag_id, tag_name, kind, created_at, completed_at) = connection.query_row(
        "SELECT title,tag_id,tag_name,tag_kind,created_at,completed_at FROM tasks WHERE id=?1 AND deleted=0",
        [id],
        |row| Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?, row.get::<_, String>(3)?, row.get(4)?, row.get::<_, Option<i64>>(5)?)),
    ).optional()?.ok_or(Error::NotFound)?;
    let mut task = Task {
        id,
        title,
        tag_id,
        tag_name,
        tag_kind: TagKind::parse(&kind)?,
        created_at,
        completed_at,
        total_seconds: 0,
        session_count: 0,
    };
    let Some(completed_at) = task.completed_at else {
        return Err(Error::InvalidTransition);
    };
    let sessions = load_sessions_in_snapshot(connection, "WHERE task_id=?1 ORDER BY id", [id])?;
    if sessions.iter().any(|session| {
        session.finished_at.is_none()
            || session
                .intervals
                .iter()
                .any(|interval| interval.end.is_none())
    }) {
        return Err(Error::InvalidTransition);
    }
    task.total_seconds = sessions
        .iter()
        .map(|session| session.elapsed_at(completed_at))
        .sum();
    task.session_count = sessions.len();
    let interval_ids = connection.prepare(
        "SELECT i.id FROM intervals i JOIN sessions s ON s.id=i.session_id WHERE s.task_id=?1 ORDER BY i.id",
    )?.query_map([id], |row| row.get(0))?.collect::<std::result::Result<Vec<_>, _>>()?;
    let plans = connection.prepare(
        "SELECT id,title,day,start_minute,end_minute,note,completed_at,moved_to,created_at,deleted FROM agenda_entries WHERE task_id=?1 ORDER BY id",
    )?.query_map([id], |row| Ok(CompletionPlan {
        id: row.get(0)?, title: row.get(1)?, day: row.get(2)?, start: row.get(3)?, end: row.get(4)?,
        note: row.get(5)?, completed_at: row.get(6)?, moved_to: row.get(7)?, created_at: row.get(8)?, deleted: row.get(9)?,
    }))?.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(CompletionBaseline {
        task,
        sessions,
        interval_ids,
        plans,
    })
}

#[cfg(test)]
mod tests;
