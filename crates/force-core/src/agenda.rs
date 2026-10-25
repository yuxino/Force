//! Planned local wall-clock slots, kept separate from actual timer intervals.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rusqlite::Row;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanEntry {
    pub id: i64,
    pub task_id: i64,
    /// A snapshot for this occurrence; changing a task never rewrites old plans.
    pub title: String,
    pub day: NaiveDate,
    pub start_minute: u16,
    pub end_minute: u16,
    pub note: String,
    pub completed_at: Option<i64>,
    /// The original occurrence remains visible when it is moved to another day.
    pub moved_to: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanDraft {
    pub task_id: Option<i64>,
    pub title: String,
    /// Used only when a new task is created with the plan.
    pub tag_id: i64,
    pub day: NaiveDate,
    pub start_minute: u16,
    pub end_minute: u16,
    pub note: String,
}

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS agenda_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    day TEXT NOT NULL CHECK (length(day) = 10),
    start_minute INTEGER NOT NULL CHECK (start_minute BETWEEN 0 AND 1439),
    end_minute INTEGER NOT NULL CHECK (end_minute > start_minute AND end_minute <= 1440),
    note TEXT NOT NULL DEFAULT '',
    completed_at INTEGER CHECK (completed_at IS NULL OR completed_at >= created_at),
    moved_to INTEGER REFERENCES agenda_entries(id) ON DELETE RESTRICT,
    created_at INTEGER NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
    CHECK (moved_to IS NULL OR moved_to <> id)
);
CREATE INDEX IF NOT EXISTS agenda_day_entries ON agenda_entries(day, deleted, start_minute, id);
CREATE INDEX IF NOT EXISTS completed_task_calendar ON tasks(completed_at) WHERE completed_at IS NOT NULL AND deleted = 0;
";

pub(super) fn migrate_v4(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        4 => {
            transaction.execute_batch(SCHEMA)?;
            // An interrupted/foreign schema with the same table name must fail
            // rather than receive a newer version marker over missing columns.
            transaction.prepare(SELECT_PLAN)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        SCHEMA_VERSION => (),
        _ => return Err(Error::UnsupportedSchema),
    }
    transaction.commit()?;
    Ok(())
}

const SELECT_PLAN: &str = "SELECT id, task_id, title, day, start_minute, end_minute, note, completed_at, moved_to, created_at FROM agenda_entries";

impl Store {
    /// One selected local date, including moved markers, in calendar order.
    pub fn plans_for_day(&self, day: NaiveDate) -> Result<Vec<PlanEntry>> {
        validate_day(day)?;
        let mut statement = self.connection.prepare(&format!(
            "{SELECT_PLAN} WHERE day = ?1 AND deleted = 0 ORDER BY start_minute, end_minute, id"
        ))?;
        statement
            .query_map([day.format("%Y-%m-%d").to_string()], read_raw_plan)?
            .map(|row| row.map_err(Error::from).and_then(RawPlan::validated))
            .collect()
    }

    /// Full occurrences for a bounded visible calendar range, including moved
    /// markers. Load once for the month rather than issuing a query per cell.
    pub fn plans_between(
        &self,
        start: NaiveDate,
        end_exclusive: NaiveDate,
    ) -> Result<Vec<PlanEntry>> {
        validate_day(start)?;
        let end_key = if end_exclusive == NaiveDate::from_ymd_opt(10000, 1, 1).unwrap() {
            "9999-12-32".to_owned()
        } else {
            validate_day(end_exclusive)?;
            end_exclusive.format("%Y-%m-%d").to_string()
        };
        if start >= end_exclusive {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare(&format!(
            "{SELECT_PLAN} WHERE day >= ?1 AND day < ?2 AND deleted = 0 ORDER BY day, start_minute, end_minute, id"
        ))?;
        statement
            .query_map(
                params![start.format("%Y-%m-%d").to_string(), end_key],
                read_raw_plan,
            )?
            .map(|row| row.map_err(Error::from).and_then(RawPlan::validated))
            .collect()
    }

    /// Planned dates for a visible month, loaded in one bounded query.
    pub fn plan_days_between(
        &self,
        start: NaiveDate,
        end_exclusive: NaiveDate,
    ) -> Result<Vec<NaiveDate>> {
        validate_day(start)?;
        // Permit the exclusive boundary after the last supported stored date.
        let end_key = if end_exclusive == NaiveDate::from_ymd_opt(10000, 1, 1).unwrap() {
            "9999-12-32".to_owned()
        } else {
            validate_day(end_exclusive)?;
            end_exclusive.format("%Y-%m-%d").to_string()
        };
        if start >= end_exclusive {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare("SELECT DISTINCT day FROM agenda_entries WHERE day >= ?1 AND day < ?2 AND deleted = 0 ORDER BY day")?;
        statement
            .query_map(
                params![start.format("%Y-%m-%d").to_string(), end_key],
                |row| row.get::<_, String>(0),
            )?
            .map(|row| {
                let value = row?;
                let day = NaiveDate::parse_from_str(&value, "%Y-%m-%d")
                    .map_err(|_| Error::InvalidData)?;
                if day.format("%Y-%m-%d").to_string() != value {
                    return Err(Error::InvalidData);
                }
                Ok(day)
            })
            .collect()
    }

    /// Creates the task and its first arrangement atomically, without timing it.
    pub fn create_plan(&mut self, draft: &PlanDraft, now: i64) -> Result<PlanEntry> {
        let (title, note) = validate_draft(draft)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task_id = if let Some(id) = draft.task_id {
            let task = transaction
                .query_row(
                    "SELECT tag_kind, completed_at FROM tasks WHERE id = ?1 AND deleted = 0",
                    [id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
                )
                .optional()?
                .ok_or(Error::NotFound)?;
            if task.0 != "focus" || task.1.is_some() {
                return Err(Error::InvalidTransition);
            }
            id
        } else {
            if task::available_tag(&transaction, draft.tag_id)?.kind != TagKind::Focus {
                return Err(Error::InvalidTransition);
            }
            task::insert_task(&transaction, title, draft.tag_id, now)?.id
        };
        transaction.execute(
            "INSERT INTO agenda_entries (task_id, title, day, start_minute, end_minute, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![task_id, title, draft.day.format("%Y-%m-%d").to_string(), draft.start_minute, draft.end_minute, note, now],
        )?;
        let plan = load_plan(&transaction, transaction.last_insert_rowid())?;
        transaction.commit()?;
        Ok(plan)
    }

    /// Edits only the occurrence, never its task, session snapshots or intervals.
    pub fn update_plan(&mut self, id: i64, draft: &PlanDraft) -> Result<PlanEntry> {
        let (title, note) = validate_draft(draft)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = load_plan(&transaction, id)?;
        let plan = update_plan_in_transaction(&transaction, &previous, draft, title, note)?;
        transaction.commit()?;
        Ok(plan)
    }

    /// Atomically compares the edit baseline and writes under the same immediate
    /// transaction. `None` preserves another window's edit, move or deletion.
    pub fn update_plan_if_unchanged(
        &mut self,
        id: i64,
        draft: &PlanDraft,
        expected: &PlanEntry,
    ) -> Result<Option<PlanEntry>> {
        let (title, note) = validate_draft(draft)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = match load_plan(&transaction, id) {
            Ok(current) => current,
            Err(Error::NotFound) => return Ok(None),
            Err(error) => return Err(error),
        };
        if &current != expected {
            return Ok(None);
        }
        let plan = update_plan_in_transaction(&transaction, &current, draft, title, note)?;
        transaction.commit()?;
        Ok(Some(plan))
    }

    /// This occurrence's check mark does not complete the global task or timer.
    pub fn set_plan_completed(&mut self, id: i64, completed: bool, now: i64) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = load_plan(&transaction, id)?;
        if previous.moved_to.is_some() || now < previous.created_at {
            return Err(Error::InvalidTransition);
        }
        // Repeated checked events retain the original completion timestamp.
        let completed_at = completed.then_some(previous.completed_at.unwrap_or(now));
        transaction.execute(
            "UPDATE agenda_entries SET completed_at = ?2 WHERE id = ?1 AND deleted = 0",
            params![id, completed_at],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Leaves a dated moved marker and creates the successor in one transaction.
    pub fn postpone_plan(&mut self, id: i64, new_day: NaiveDate, now: i64) -> Result<PlanEntry> {
        validate_day(new_day)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = load_plan(&transaction, id)?;
        if previous.moved_to.is_some()
            || previous.completed_at.is_some()
            || now < previous.created_at
            || new_day <= previous.day
        {
            return Err(Error::InvalidTransition);
        }
        transaction.execute(
            "INSERT INTO agenda_entries (task_id, title, day, start_minute, end_minute, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![previous.task_id, previous.title, new_day.format("%Y-%m-%d").to_string(), previous.start_minute, previous.end_minute, previous.note, now],
        )?;
        let next_id = transaction.last_insert_rowid();
        transaction.execute(
            "UPDATE agenda_entries SET moved_to = ?2 WHERE id = ?1",
            params![id, next_id],
        )?;
        let next = load_plan(&transaction, next_id)?;
        transaction.commit()?;
        Ok(next)
    }

    /// Removes the plan from ordinary views without deleting any actual work.
    pub fn delete_plan(&mut self, id: i64) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        load_plan(&transaction, id)?;
        transaction.execute("UPDATE agenda_entries SET deleted = 1 WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
    }

    /// Actual global task completions for one half-open time range. Callers
    /// supply local-day boundaries; no UTC date is inferred for a local plan.
    pub fn completed_tasks_between(&self, start: i64, end: i64, now: i64) -> Result<Vec<Task>> {
        if start >= end || start > now {
            return Ok(Vec::new());
        }
        let transaction = self.connection.unchecked_transaction()?;
        let mut statement = transaction.prepare(
            "SELECT id, title, tag_id, tag_name, tag_kind, created_at, completed_at FROM tasks WHERE deleted = 0 AND completed_at >= ?1 AND completed_at < ?2 AND completed_at <= ?3 ORDER BY completed_at, id",
        )?;
        let records = statement
            .query_map(params![start, end, now], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);
        let mut tasks = records
            .into_iter()
            .map(
                |(id, title, tag_id, tag_name, kind, created_at, completed_at)| {
                    Ok(Task {
                        id,
                        title,
                        tag_id,
                        tag_name,
                        tag_kind: TagKind::parse(&kind)?,
                        created_at,
                        completed_at,
                        total_seconds: 0,
                        session_count: 0,
                    })
                },
            )
            .collect::<Result<Vec<_>>>()?;
        let positions: HashMap<_, _> = tasks
            .iter()
            .enumerate()
            .map(|(index, task)| (task.id, index))
            .collect();
        let mut counts = transaction.prepare("SELECT s.task_id, COUNT(*) FROM sessions s JOIN tasks t ON t.id = s.task_id WHERE t.deleted = 0 AND t.completed_at >= ?1 AND t.completed_at < ?2 AND t.completed_at <= ?3 GROUP BY s.task_id")?;
        for row in counts.query_map(params![start, end, now], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })? {
            let (id, count) = row?;
            if let Some(index) = positions.get(&id) {
                tasks[*index].session_count =
                    usize::try_from(count).map_err(|_| Error::InvalidData)?;
            }
        }
        drop(counts);
        let mut intervals = transaction.prepare("SELECT s.task_id, i.started_at, i.ended_at FROM intervals i JOIN sessions s ON s.id = i.session_id JOIN tasks t ON t.id = s.task_id WHERE t.deleted = 0 AND t.completed_at >= ?1 AND t.completed_at < ?2 AND t.completed_at <= ?3")?;
        for row in intervals.query_map(params![start, end, now], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })? {
            let (id, interval_start, interval_end) = row?;
            if let Some(index) = positions.get(&id) {
                tasks[*index].total_seconds = tasks[*index].total_seconds.saturating_add(
                    interval_end
                        .unwrap_or(now)
                        .saturating_sub(interval_start)
                        .max(0),
                );
            }
        }
        drop(intervals);
        transaction.commit()?;
        Ok(tasks)
    }
}

fn update_plan_in_transaction(
    transaction: &Transaction<'_>,
    previous: &PlanEntry,
    draft: &PlanDraft,
    title: &str,
    note: &str,
) -> Result<PlanEntry> {
    if previous.moved_to.is_some()
        || draft
            .task_id
            .is_some_and(|task_id| task_id != previous.task_id)
    {
        return Err(Error::InvalidTransition);
    }
    transaction.execute(
        "UPDATE agenda_entries SET title = ?2, day = ?3, start_minute = ?4, end_minute = ?5, note = ?6 WHERE id = ?1 AND deleted = 0",
        params![previous.id, title, draft.day.format("%Y-%m-%d").to_string(), draft.start_minute, draft.end_minute, note],
    )?;
    load_plan(transaction, previous.id)
}

fn validate_day(day: NaiveDate) -> Result<()> {
    if !(1..=9999).contains(&day.year()) {
        return Err(Error::Validation(
            "Enter a calendar date between years 1 and 9999",
        ));
    }
    Ok(())
}

fn validate_draft(draft: &PlanDraft) -> Result<(&str, &str)> {
    validate_day(draft.day)?;
    let title = valid_text(&draft.title, 200, "Enter a task title of 1–200 characters")?;
    if draft.start_minute >= 1440
        || draft.end_minute > 1440
        || draft.end_minute <= draft.start_minute
    {
        return Err(Error::Validation(
            "Plan end time must be later than start time within one day",
        ));
    }
    let note = draft.note.trim();
    if note.chars().count() > 2000 || note.contains('\0') {
        return Err(Error::Validation("Enter a note of at most 2000 characters"));
    }
    Ok((title, note))
}

struct RawPlan {
    id: i64,
    task_id: i64,
    title: String,
    day: String,
    start_minute: u16,
    end_minute: u16,
    note: String,
    completed_at: Option<i64>,
    moved_to: Option<i64>,
    created_at: i64,
}

fn read_raw_plan(row: &Row<'_>) -> rusqlite::Result<RawPlan> {
    Ok(RawPlan {
        id: row.get(0)?,
        task_id: row.get(1)?,
        title: row.get(2)?,
        day: row.get(3)?,
        start_minute: row.get(4)?,
        end_minute: row.get(5)?,
        note: row.get(6)?,
        completed_at: row.get(7)?,
        moved_to: row.get(8)?,
        created_at: row.get(9)?,
    })
}

impl RawPlan {
    fn validated(self) -> Result<PlanEntry> {
        let day =
            NaiveDate::parse_from_str(&self.day, "%Y-%m-%d").map_err(|_| Error::InvalidData)?;
        if day.format("%Y-%m-%d").to_string() != self.day
            || !(1..=9999).contains(&day.year())
            || self.start_minute >= self.end_minute
            || self.end_minute > 1440
            || self.title.trim().is_empty()
            || self.title.chars().count() > 200
            || self.title.contains('\0')
            || self.note.chars().count() > 2000
            || self.note.contains('\0')
        {
            return Err(Error::InvalidData);
        }
        Ok(PlanEntry {
            id: self.id,
            task_id: self.task_id,
            title: self.title,
            day,
            start_minute: self.start_minute,
            end_minute: self.end_minute,
            note: self.note,
            completed_at: self.completed_at,
            moved_to: self.moved_to,
            created_at: self.created_at,
        })
    }
}

fn load_plan(connection: &Connection, id: i64) -> Result<PlanEntry> {
    connection
        .query_row(
            &format!("{SELECT_PLAN} WHERE id = ?1 AND deleted = 0"),
            [id],
            read_raw_plan,
        )
        .optional()?
        .ok_or(Error::NotFound)?
        .validated()
}

#[cfg(test)]
mod tests;
