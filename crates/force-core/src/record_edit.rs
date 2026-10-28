//! Explicit, transactional corrections to completed records and backfilled work.

use chrono::Datelike;

use super::*;

/// One actual period of work. Gaps between periods remain paused time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordIntervalEdit {
    pub start: i64,
    pub end: i64,
}

/// An immutable concurrency baseline accompanies the editable public session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordEditSnapshot {
    pub session: Session,
    baseline: RecordBaseline,
}

impl RecordEditSnapshot {
    /// Whether changing this round's end also changes the task's completion.
    /// A separately marked task completion is intentionally independent.
    pub fn task_completion_follows_record(&self) -> bool {
        completion_follows_record(&self.baseline)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordBaseline {
    session: Session,
    interval_ids: Vec<i64>,
    task: Option<LinkedTask>,
    siblings: Vec<(i64, i64, Option<i64>)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkedTask {
    id: i64,
    title: String,
    tag_id: i64,
    tag_name: String,
    tag_kind: String,
    created_at: i64,
    completed_at: Option<i64>,
    deleted: bool,
}

impl Store {
    /// Loads every period, without the recent-history limit. Active and paused
    /// rounds cannot be edited as history; end them through normal timer actions.
    pub fn record_edit_snapshot(&self, session_id: i64) -> Result<RecordEditSnapshot> {
        let transaction = self.connection.unchecked_transaction()?;
        let baseline = record_baseline(&transaction, session_id)?;
        let snapshot = RecordEditSnapshot {
            session: baseline.session.clone(),
            baseline,
        };
        transaction.commit()?;
        Ok(snapshot)
    }

    /// Corrects a completed round while retaining every interval's identity and
    /// count. Keeping the tag id preserves its historical snapshot, even after
    /// archival; explicitly selecting another tag takes its current snapshot.
    pub fn update_completed_record(
        &mut self,
        expected: &RecordEditSnapshot,
        title: &str,
        tag_id: i64,
        intervals: &[RecordIntervalEdit],
        now: i64,
    ) -> Result<Session> {
        let (start, end) =
            validate_intervals(intervals, now, Some(&expected.baseline.session.intervals))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = record_baseline(&transaction, expected.baseline.session.id)?;
        if current != expected.baseline || expected.session != expected.baseline.session {
            return Err(Error::Validation(
                "This record changed while editing; reopen it before saving",
            ));
        }
        if intervals.len() != current.interval_ids.len() {
            return Err(Error::Validation(
                "Keep the existing number of recorded periods",
            ));
        }
        let old = &current.session;
        let timing_changed = intervals
            .iter()
            .zip(&old.intervals)
            .any(|(edited, original)| {
                edited.start != original.start || Some(edited.end) != original.end
            });
        let (start, end) = if timing_changed {
            (start, end)
        } else {
            (old.started_at, old.finished_at.expect("completed record"))
        };
        let tag = if tag_id == old.tag_id {
            Tag {
                id: old.tag_id,
                name: old.tag_name.clone(),
                kind: old.tag_kind,
                class: old.tag_class,
                color: old.tag_color.clone(),
            }
        } else {
            task::available_tag(&transaction, tag_id)?
        };
        let title = record_title(title, &tag.name)?;
        // Validate task linkage before any write, so an invalid correction
        // cannot leave the record and its visible completion on different days.
        let completion = if timing_changed {
            linked_completion_update(&current, start, end)?
        } else {
            None
        };
        transaction.execute(
            "UPDATE sessions SET title=?1, tag_id=?2, tag_name=?3, tag_kind=?4,
             tag_class=?5, tag_color=?6, started_at=?7, finished_at=?8 WHERE id=?9",
            params![
                title,
                tag.id,
                tag.name,
                tag.kind.as_str(),
                tag.class.as_str(),
                tag.color,
                start,
                end,
                old.id
            ],
        )?;
        for (id, interval) in current.interval_ids.iter().zip(intervals) {
            transaction.execute(
                "UPDATE intervals SET started_at=?1, ended_at=?2 WHERE id=?3 AND session_id=?4",
                params![interval.start, interval.end, id, old.id],
            )?;
        }
        if let Some(linked) = &current.task {
            if let Some((created_at, completed_at)) = completion {
                transaction.execute(
                    "UPDATE tasks SET created_at=?1, completed_at=?2 WHERE id=?3",
                    params![created_at, completed_at, linked.id],
                )?;
            }
            // A task with one matching round is the same visible item. Distinct
            // task titles, multi-round work and unfinished tasks stay untouched.
            if !linked.deleted
                && linked.completed_at.is_some()
                && current.siblings.is_empty()
                && linked.title == old.title
                && linked.tag_id == old.tag_id
            {
                transaction.execute(
                    "UPDATE tasks SET title=?1, tag_id=?2, tag_name=?3, tag_kind=?4 WHERE id=?5",
                    params![title, tag.id, tag.name, tag.kind.as_str(), linked.id],
                )?;
            }
        }
        let saved = load_session(&transaction, old.id)?;
        transaction.commit()?;
        Ok(saved)
    }

    /// Backfills completed work without starting, pausing or completing any
    /// existing timer. The last supplied end is the new task's completion time.
    pub fn create_completed_record(
        &mut self,
        title: &str,
        tag_id: i64,
        intervals: &[RecordIntervalEdit],
        now: i64,
    ) -> Result<Session> {
        let (start, end) = validate_intervals(intervals, now, None)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let tag = task::available_tag(&transaction, tag_id)?;
        let title = record_title(title, &tag.name)?;
        let task = task::insert_task(&transaction, title, tag_id, start)?;
        transaction.execute(
            "UPDATE tasks SET completed_at=?1 WHERE id=?2",
            params![end, task.id],
        )?;
        transaction.execute(
            "INSERT INTO sessions (task_id,title,tag_id,tag_name,tag_kind,tag_class,tag_color,started_at,finished_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![task.id,title,tag.id,tag.name,tag.kind.as_str(),tag.class.as_str(),tag.color,start,end],
        )?;
        let id = transaction.last_insert_rowid();
        for interval in intervals {
            transaction.execute(
                "INSERT INTO intervals (session_id,started_at,ended_at) VALUES (?1,?2,?3)",
                params![id, interval.start, interval.end],
            )?;
        }
        let saved = load_session(&transaction, id)?;
        transaction.commit()?;
        Ok(saved)
    }
}

fn record_title<'a>(title: &'a str, tag_name: &'a str) -> Result<&'a str> {
    let value = if title.trim().is_empty() {
        tag_name
    } else {
        title
    };
    valid_text(value, 200, "Enter a description of at most 200 characters")
}

fn validate_timestamp(timestamp: i64) -> Result<()> {
    if DateTime::<Utc>::from_timestamp(timestamp, 0)
        .is_none_or(|date| !(1..=9999).contains(&date.year()))
    {
        return Err(Error::Validation("Choose a valid date and time"));
    }
    Ok(())
}

fn validate_intervals(
    intervals: &[RecordIntervalEdit],
    now: i64,
    original: Option<&[Interval]>,
) -> Result<(i64, i64)> {
    validate_timestamp(now)?;
    let Some(first) = intervals.first() else {
        return Err(Error::Validation("Add at least one recorded period"));
    };
    let mut previous_end = None;
    for (index, interval) in intervals.iter().enumerate() {
        validate_timestamp(interval.start)?;
        validate_timestamp(interval.end)?;
        let unchanged_zero = original
            .and_then(|items| items.get(index))
            .is_some_and(|old| {
                old.start == interval.start
                    && old.end == Some(interval.end)
                    && interval.start == interval.end
            });
        if interval.end < interval.start || (interval.end == interval.start && !unchanged_zero) {
            return Err(Error::Validation("Each period must end after it starts"));
        }
        if interval.end > now {
            return Err(Error::Validation("Recorded work cannot be in the future"));
        }
        if previous_end.is_some_and(|end| interval.start < end) {
            return Err(Error::Validation(
                "Recorded periods must be ordered and cannot overlap",
            ));
        }
        previous_end = Some(interval.end);
    }
    Ok((first.start, previous_end.expect("nonempty intervals")))
}

fn record_baseline(connection: &Connection, id: i64) -> Result<RecordBaseline> {
    let session = load_session(connection, id)?;
    if session.finished_at.is_none() || session.intervals.iter().any(|period| period.end.is_none())
    {
        return Err(Error::InvalidTransition);
    }
    let mut statement = connection
        .prepare("SELECT id FROM intervals WHERE session_id=?1 ORDER BY started_at,id")?;
    let interval_ids = statement
        .query_map([id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let task = session.task_id.map(|task_id| {
        connection.query_row(
            "SELECT id,title,tag_id,tag_name,tag_kind,created_at,completed_at,deleted FROM tasks WHERE id=?1",
            [task_id],
            |row| Ok(LinkedTask {
                id: row.get(0)?, title: row.get(1)?, tag_id: row.get(2)?,
                tag_name: row.get(3)?, tag_kind: row.get(4)?, created_at: row.get(5)?,
                completed_at: row.get(6)?, deleted: row.get(7)?,
            }),
        ).optional()
    }).transpose()?.flatten();
    let siblings = if let Some(task_id) = session.task_id {
        let mut statement = connection.prepare(
            "SELECT id,started_at,finished_at FROM sessions WHERE task_id=?1 AND id!=?2 ORDER BY id",
        )?;
        statement
            .query_map(params![task_id, id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    Ok(RecordBaseline {
        session,
        interval_ids,
        task,
        siblings,
    })
}

fn completion_follows_record(baseline: &RecordBaseline) -> bool {
    let Some(task) = &baseline.task else {
        return false;
    };
    !task.deleted
        && task.completed_at.is_some()
        && task.completed_at == baseline.session.finished_at
}

fn linked_completion_update(
    baseline: &RecordBaseline,
    start: i64,
    end: i64,
) -> Result<Option<(i64, i64)>> {
    if !completion_follows_record(baseline) {
        return Ok(None);
    }
    let task = baseline.task.as_ref().expect("linked completed task");
    if baseline.siblings.is_empty() {
        return Ok(Some((task.created_at.min(start), end)));
    }
    let old_end = baseline.session.finished_at.expect("completed record");
    if baseline.siblings.iter().any(|(id, _, finish)| {
        finish.is_none_or(|finish| (finish, *id) > (old_end, baseline.session.id) || finish > end)
    }) || end < task.created_at
    {
        return Err(Error::Validation(
            "Task completion must remain after its other rounds; edit the final round instead",
        ));
    }
    Ok(Some((task.created_at, end)))
}

#[cfg(test)]
mod tests;
