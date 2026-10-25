//! Local timer records, durable intervals, and bounded history queries.
//!
//! Timestamps are Unix seconds. Active intervals include computer sleep. The
//! recent history query is capped at [`HISTORY_LIMIT`]; CSV exports all records.

use std::{fs, path::Path, time::Duration};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use thiserror::Error;

pub const HISTORY_LIMIT: usize = 500;
pub const MAX_GOAL_MINUTES: u32 = 24 * 60;
pub const MIN_TARGET_SECONDS: i64 = 60;
pub const MAX_TARGET_SECONDS: i64 = 24 * 60 * 60;
const SCHEMA_VERSION: i64 = 5;

mod agenda;
mod task;
pub use agenda::{PlanDraft, PlanEntry};
pub use task::Task;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not access local data: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not read or save local data: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("This data was created by a newer version of Force")]
    UnsupportedSchema,
    #[error("The requested record no longer exists")]
    NotFound,
    #[error("This task's tag is archived; choose another tag")]
    ArchivedTag,
    #[error("{0}")]
    Validation(&'static str),
    #[error("This action is no longer available, or the clock moved backwards")]
    InvalidTransition,
    #[error("The local data contains an invalid value")]
    InvalidData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TagKind {
    Focus,
    Rest,
}

impl TagKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Rest => "rest",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "focus" => Ok(Self::Focus),
            "rest" => Ok(Self::Rest),
            _ => Err(Error::InvalidData),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub kind: TagKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Interval {
    pub start: i64,
    pub end: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub id: i64,
    pub task_id: Option<i64>,
    pub title: String,
    pub tag_id: i64,
    pub tag_name: String,
    pub tag_kind: TagKind,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub target_seconds: Option<i64>,
    pub intervals: Vec<Interval>,
}

impl Session {
    pub fn elapsed_at(&self, now: i64) -> i64 {
        self.intervals.iter().fold(0_i64, |total, interval| {
            total.saturating_add(
                interval
                    .end
                    .unwrap_or(now)
                    .saturating_sub(interval.start)
                    .max(0),
            )
        })
    }

    /// Active investment clipped to a half-open date/range and the snapshot time.
    /// Pauses contribute nothing; a session can contribute to multiple dates.
    pub fn elapsed_between(&self, start: i64, end: i64, now: i64) -> i64 {
        self.intervals.iter().fold(0_i64, |total, interval| {
            let left = interval.start.max(start);
            let right = interval.end.unwrap_or(now).min(end).min(now);
            total.saturating_add(right.saturating_sub(left).max(0))
        })
    }

    pub fn paused_at(&self, now: i64) -> i64 {
        self.finished_at
            .unwrap_or(now)
            .saturating_sub(self.started_at)
            .saturating_sub(self.elapsed_at(now))
            .max(0)
    }

    pub fn is_running(&self) -> bool {
        self.finished_at.is_none() && self.intervals.last().is_some_and(|i| i.end.is_none())
    }

    pub fn is_paused(&self) -> bool {
        self.finished_at.is_none() && !self.is_running()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Stats {
    /// Merged time from focus intervals, so parallel focus tasks count once.
    pub focus_seconds: i64,
    /// Time invested across every task, including rest and overlapping tasks.
    pub task_seconds: i64,
    /// Sessions whose completion timestamp falls within the requested window.
    pub completed_sessions: usize,
}

pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens an existing database without replacing it on any failure.
    ///
    /// The containing directory belongs to this app and is made private on Unix.
    /// Use a dedicated data directory rather than a shared directory such as HOME.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
            private_directory(parent)?;
        }
        // Precreate with private permissions, eliminating a world-readable window.
        let file = private_database_file(path)?;
        drop(file);
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => initialize(&mut connection)?,
            1 => {
                migrate_v1(&mut connection)?;
                task::migrate_v2(&mut connection)?;
                migrate_v3(&mut connection)?;
                agenda::migrate_v4(&mut connection)?;
            }
            2 => {
                task::migrate_v2(&mut connection)?;
                migrate_v3(&mut connection)?;
                agenda::migrate_v4(&mut connection)?;
            }
            3 => {
                migrate_v3(&mut connection)?;
                agenda::migrate_v4(&mut connection)?;
            }
            4 => agenda::migrate_v4(&mut connection)?,
            SCHEMA_VERSION => (),
            _ => return Err(Error::UnsupportedSchema),
        }
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        Ok(Self { connection })
    }

    /// Detects commits by other database connections. Compare values only from
    /// this Store instance; its own writes and ordinary reads do not change it.
    pub fn data_version(&self) -> Result<i64> {
        Ok(self
            .connection
            .pragma_query_value(None, "data_version", |row| row.get(0))?)
    }

    pub fn tags(&self) -> Result<Vec<Tag>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, kind FROM tags WHERE archived = 0 ORDER BY id")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get::<_, String>(2)?))
        })?;
        rows.map(|row| {
            let (id, name, kind) = row?;
            Ok(Tag {
                id,
                name,
                kind: TagKind::parse(&kind)?,
            })
        })
        .collect()
    }

    pub fn create_tag(&mut self, name: &str, kind: TagKind) -> Result<Tag> {
        let name = valid_text(name, 40, "Enter a tag name of 1–40 characters")?;
        let transaction = self.connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM tags WHERE name = ?1 COLLATE NOCASE AND archived = 0)",
            [name],
            |row| row.get(0),
        )?;
        if exists {
            return Err(Error::Validation("A tag with that name already exists"));
        }
        transaction.execute(
            "INSERT INTO tags (name, kind) VALUES (?1, ?2)",
            params![name, kind.as_str()],
        )?;
        let tag = Tag {
            id: transaction.last_insert_rowid(),
            name: name.to_owned(),
            kind,
        };
        transaction.commit()?;
        Ok(tag)
    }

    /// Renames an available tag and its unfinished sessions in one transaction.
    /// Completed sessions preserve the name shown when they were finished.
    pub fn rename_tag(&mut self, id: i64, name: &str) -> Result<Tag> {
        let name = valid_text(name, 40, "Enter a tag name of 1–40 characters")?;
        let transaction = self.connection.transaction()?;
        let kind: String = transaction
            .query_row(
                "SELECT kind FROM tags WHERE id = ?1 AND archived = 0",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(Error::NotFound)?;
        let kind = TagKind::parse(&kind)?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM tags WHERE id != ?1 AND name = ?2 COLLATE NOCASE AND archived = 0)",
            params![id, name],
            |row| row.get(0),
        )?;
        if exists {
            return Err(Error::Validation("A tag with that name already exists"));
        }
        transaction.execute("UPDATE tags SET name = ?1 WHERE id = ?2", params![name, id])?;
        transaction.execute(
            "UPDATE sessions SET tag_name = ?1 WHERE tag_id = ?2 AND finished_at IS NULL",
            params![name, id],
        )?;
        transaction.execute(
            "UPDATE tasks SET tag_name = ?1 WHERE tag_id = ?2 AND completed_at IS NULL AND deleted = 0",
            params![name, id],
        )?;
        transaction.commit()?;
        Ok(Tag {
            id,
            name: name.to_owned(),
            kind,
        })
    }

    /// Archives the tag. Existing sessions retain its name and kind snapshot.
    pub fn delete_tag(&mut self, id: i64) -> Result<()> {
        let transaction = self.connection.transaction()?;
        if transaction.execute(
            "UPDATE tags SET archived = 1 WHERE id = ?1 AND archived = 0",
            [id],
        )? != 1
        {
            return Err(Error::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn active_sessions(&self) -> Result<Vec<Session>> {
        load_sessions(
            &self.connection,
            "WHERE finished_at IS NULL ORDER BY started_at, id",
            [],
        )
    }

    /// Returns the most recent 500 completed sessions, newest first.
    pub fn sessions(&self) -> Result<Vec<Session>> {
        load_sessions(
            &self.connection,
            "WHERE finished_at IS NOT NULL ORDER BY finished_at DESC, id DESC LIMIT 500",
            [],
        )
    }

    /// Returns all completed sessions in `[start, end)`, newest first.
    /// Unlike recent history, a selected date is not truncated at 500 records.
    pub fn sessions_between(&self, start: i64, end: i64) -> Result<Vec<Session>> {
        if end <= start {
            return Ok(Vec::new());
        }
        load_sessions(
            &self.connection,
            "WHERE finished_at >= ?1 AND finished_at < ?2 ORDER BY finished_at DESC, id DESC",
            params![start, end],
        )
    }

    /// Sessions with actual investment in the range, including running/paused
    /// sessions. Retain start/completion events even for zero-second sessions.
    /// A paused gap alone never makes a session belong to a date.
    pub fn sessions_overlapping(&self, start: i64, end: i64, now: i64) -> Result<Vec<Session>> {
        if end <= start || now < start {
            return Ok(Vec::new());
        }
        load_sessions(
            &self.connection,
            "WHERE EXISTS (SELECT 1 FROM intervals i WHERE i.session_id = sessions.id
                AND i.started_at < MIN(?2, ?3)
                AND MIN(COALESCE(i.ended_at, ?3), ?2, ?3) > MAX(i.started_at, ?1))
             OR (started_at >= ?1 AND started_at < ?2 AND started_at <= ?3)
             OR (finished_at >= ?1 AND finished_at < ?2 AND finished_at <= ?3)
             ORDER BY COALESCE(finished_at, ?3) DESC, started_at DESC, id DESC",
            params![start, end, now],
        )
    }

    pub fn start(&mut self, title: &str, tag_id: i64, now: i64) -> Result<Session> {
        self.start_with_target(title, tag_id, now, None)
    }

    pub fn start_timed(
        &mut self,
        title: &str,
        tag_id: i64,
        now: i64,
        target_seconds: i64,
    ) -> Result<Session> {
        if !(MIN_TARGET_SECONDS..=MAX_TARGET_SECONDS).contains(&target_seconds) {
            return Err(Error::Validation(
                "Timer duration must be between 1 minute and 24 hours",
            ));
        }
        self.start_with_target(title, tag_id, now, Some(target_seconds))
    }

    fn start_with_target(
        &mut self,
        title: &str,
        tag_id: i64,
        now: i64,
        target_seconds: Option<i64>,
    ) -> Result<Session> {
        let title = valid_text(title, 200, "Enter a task title of 1–200 characters")?;
        let transaction = self.connection.transaction()?;
        let tag: Option<(String, String)> = transaction
            .query_row(
                "SELECT name, kind FROM tags WHERE id = ?1 AND archived = 0",
                [tag_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (tag_name, tag_kind) = tag.ok_or(Error::NotFound)?;
        TagKind::parse(&tag_kind)?;
        transaction.execute(
            "INSERT INTO sessions (title, tag_id, tag_name, tag_kind, started_at, target_seconds) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![title, tag_id, tag_name, tag_kind, now, target_seconds],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO intervals (session_id, started_at) VALUES (?1, ?2)",
            params![id, now],
        )?;
        let session = load_session(&transaction, id)?;
        transaction.commit()?;
        Ok(session)
    }

    pub fn pause(&mut self, id: i64, now: i64) -> Result<Session> {
        let transaction = self.connection.transaction()?;
        let session = load_session(&transaction, id)?;
        if !session.is_running() {
            return Err(Error::InvalidTransition);
        }
        close_open_interval(&transaction, &session, now)?;
        let session = load_session(&transaction, id)?;
        transaction.commit()?;
        Ok(session)
    }

    pub fn resume(&mut self, id: i64, now: i64) -> Result<Session> {
        let transaction = self.connection.transaction()?;
        let session = load_session(&transaction, id)?;
        if !session.is_paused() || now < last_boundary(&session) {
            return Err(Error::InvalidTransition);
        }
        transaction.execute(
            "INSERT INTO intervals (session_id, started_at) VALUES (?1, ?2)",
            params![id, now],
        )?;
        let session = load_session(&transaction, id)?;
        transaction.commit()?;
        Ok(session)
    }

    pub fn finish(&mut self, id: i64, now: i64) -> Result<Session> {
        let transaction = self.connection.transaction()?;
        let session = load_session(&transaction, id)?;
        if session.finished_at.is_some() || now < last_boundary(&session) {
            return Err(Error::InvalidTransition);
        }
        if session
            .target_seconds
            .is_some_and(|target| session.elapsed_at(now) >= target)
        {
            finish_at_target(&transaction, &session, now)?;
        } else {
            if session.is_running() {
                close_open_interval(&transaction, &session, now)?;
            }
            transaction.execute(
                "UPDATE sessions SET finished_at = ?1 WHERE id = ?2",
                params![now, id],
            )?;
        }
        let session = load_session(&transaction, id)?;
        transaction.commit()?;
        Ok(session)
    }

    /// Completes a timed session at its exact accumulated target, excluding
    /// pauses. Excess time after sleep or a delayed tick is never recorded.
    pub fn finish_exact_target(&mut self, id: i64, now: i64) -> Result<Session> {
        let transaction = self.connection.transaction()?;
        let session = load_session(&transaction, id)?;
        if session.finished_at.is_some() || now < last_boundary(&session) {
            return Err(Error::InvalidTransition);
        }
        finish_at_target(&transaction, &session, now)?;
        let session = load_session(&transaction, id)?;
        transaction.commit()?;
        Ok(session)
    }

    pub fn delete_session(&mut self, id: i64) -> Result<()> {
        let transaction = self.connection.transaction()?;
        if transaction.execute("DELETE FROM sessions WHERE id = ?1", [id])? != 1 {
            return Err(Error::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn goal_minutes(&self) -> Result<u32> {
        let value: i64 = self.connection.query_row(
            "SELECT goal_minutes FROM preferences WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        let value = u32::try_from(value).map_err(|_| Error::InvalidData)?;
        if !(1..=MAX_GOAL_MINUTES).contains(&value) {
            return Err(Error::InvalidData);
        }
        Ok(value)
    }

    pub fn set_goal_minutes(&mut self, minutes: u32) -> Result<()> {
        if !(1..=MAX_GOAL_MINUTES).contains(&minutes) {
            return Err(Error::Validation(
                "Daily goal must be between 1 and 1440 minutes",
            ));
        }
        let transaction = self.connection.transaction()?;
        if transaction.execute(
            "UPDATE preferences SET goal_minutes = ?1 WHERE id = 1",
            [minutes],
        )? != 1
        {
            return Err(Error::InvalidData);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn completion_sound(&self) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT completion_sound FROM preferences WHERE id = 1",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn set_completion_sound(&mut self, enabled: bool) -> Result<()> {
        let transaction = self.connection.transaction()?;
        if transaction.execute(
            "UPDATE preferences SET completion_sound = ?1 WHERE id = 1",
            [enabled],
        )? != 1
        {
            return Err(Error::InvalidData);
        }
        transaction.commit()?;
        Ok(())
    }

    /// Returns merged focus seconds for every `[start, min(end, now))` range,
    /// preserving input order. Empty, reversed and future ranges return zero.
    /// All buckets share one snapshot and one bounded interval query; rest and
    /// overlapping focus never inflate a bucket. Settle expired timed sessions
    /// before querying, as for [`Store::stats`].
    pub fn focus_totals_for_ranges(&self, ranges: &[(i64, i64)], now: i64) -> Result<Vec<i64>> {
        let transaction = self.connection.unchecked_transaction()?;
        let totals = calculate_focus_totals(&transaction, ranges, now)?;
        transaction.commit()?;
        Ok(totals)
    }

    /// Clips intervals to `[start, min(end, now))`. Completed events at `now`
    /// are included if they also belong to the requested `[start, end)` window.
    pub fn stats(&self, start: i64, end: i64, now: i64) -> Result<Stats> {
        let transaction = self.connection.unchecked_transaction()?;
        let stats = calculate_stats(&transaction, start, end, now)?;
        transaction.commit()?;
        Ok(stats)
    }

    /// Exports every session as UTF-8 CSV, with Unix seconds and readable UTC.
    /// Timestamps outside the calendar range retain seconds and an empty UTC cell.
    /// Expired countdowns are capped in the export snapshot without writing to
    /// the database, even when the normal timer tick has not settled them yet.
    pub fn csv_export(&self, now: i64) -> Result<String> {
        let sessions = load_sessions(&self.connection, "ORDER BY started_at, id", [])?;
        let mut csv = String::from(
            "id,title,tag,kind,started_at,finished_at,target_seconds,elapsed_seconds,status,started_at_utc,finished_at_utc\r\n",
        );
        for session in sessions {
            let session = export_session_snapshot(session, now)?;
            let status = if session.finished_at.is_some() {
                "completed"
            } else if session.is_running() {
                "running"
            } else {
                "paused"
            };
            let fields = [
                session.id.to_string(),
                session.title.clone(),
                session.tag_name.clone(),
                session.tag_kind.as_str().to_owned(),
                session.started_at.to_string(),
                session
                    .finished_at
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                session
                    .target_seconds
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                session.elapsed_at(now).to_string(),
                status.to_owned(),
                utc_timestamp(Some(session.started_at)),
                utc_timestamp(session.finished_at),
            ];
            csv.push_str(
                &fields
                    .iter()
                    .map(|field| csv_field(field))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            csv.push_str("\r\n");
        }
        Ok(csv)
    }
}

fn calculate_focus_totals(
    connection: &Connection,
    ranges: &[(i64, i64)],
    now: i64,
) -> Result<Vec<i64>> {
    let bounds = ranges
        .iter()
        .filter_map(|&(start, end)| (end.min(now) > start).then_some((start, end.min(now))))
        .fold(None, |bounds, (start, end)| match bounds {
            Some((left, right)) => Some((start.min(left), end.max(right))),
            None => Some((start, end)),
        });
    let Some((start, end)) = bounds else {
        return Ok(vec![0; ranges.len()]);
    };
    let mut statement = connection.prepare(
        "SELECT i.started_at, MIN(COALESCE(i.ended_at, ?2), ?2)
         FROM intervals i JOIN sessions s ON s.id = i.session_id
         WHERE s.tag_kind = 'focus' AND i.started_at < ?2
           AND (i.ended_at > ?1 OR i.ended_at IS NULL)
         ORDER BY i.started_at",
    )?;
    let rows = statement.query_map(params![start, end], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for row in rows {
        let (a, b) = row?;
        let a = a.max(start);
        if b <= a {
            continue;
        }
        if let Some((_, right)) = merged.last_mut()
            && a <= *right
        {
            *right = (*right).max(b);
        } else {
            merged.push((a, b));
        }
    }
    let mut prefix = Vec::with_capacity(merged.len() + 1);
    prefix.push(0_i64);
    for &(a, b) in &merged {
        prefix.push(
            prefix
                .last()
                .copied()
                .unwrap_or(0)
                .saturating_add(b.saturating_sub(a)),
        );
    }
    let covered_before = |point| {
        let count = merged.partition_point(|&(a, _)| a < point);
        if count == 0 {
            0
        } else {
            let (a, b) = merged[count - 1];
            prefix[count - 1].saturating_add(point.min(b).saturating_sub(a).max(0))
        }
    };
    Ok(ranges
        .iter()
        .map(|&(a, b)| {
            if b.min(now) <= a {
                0
            } else {
                covered_before(b.min(now)).saturating_sub(covered_before(a))
            }
        })
        .collect())
}

fn calculate_stats(connection: &Connection, start: i64, end: i64, now: i64) -> Result<Stats> {
    if end <= start || now < start {
        return Ok(Stats::default());
    }
    let interval_end = end.min(now);
    let mut statement = connection.prepare(
        "SELECT i.started_at, MIN(COALESCE(i.ended_at, ?2), ?2), s.tag_kind
         FROM intervals i JOIN sessions s ON s.id = i.session_id
         WHERE i.started_at < ?2 AND (i.ended_at > ?1 OR i.ended_at IS NULL)",
    )?;
    let rows = statement.query_map(params![start, interval_end], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut focus = Vec::new();
    let mut task_seconds = 0_i64;
    for row in rows {
        let (interval_start, interval_end, kind) = row?;
        let a = interval_start.max(start);
        let b = interval_end.min(end).min(now);
        if b > a {
            task_seconds = task_seconds.saturating_add(b.saturating_sub(a));
            if TagKind::parse(&kind)? == TagKind::Focus {
                focus.push((a, b));
            }
        }
    }
    focus.sort_unstable();
    let mut focus_seconds = 0_i64;
    let mut merged: Option<(i64, i64)> = None;
    for (a, b) in focus {
        match merged {
            Some((left, right)) if a <= right => merged = Some((left, right.max(b))),
            Some((left, right)) => {
                focus_seconds = focus_seconds.saturating_add(right.saturating_sub(left));
                merged = Some((a, b));
            }
            None => merged = Some((a, b)),
        }
    }
    if let Some((a, b)) = merged {
        focus_seconds = focus_seconds.saturating_add(b.saturating_sub(a));
    }
    let completed_sessions: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sessions WHERE finished_at >= ?1 AND finished_at < ?2 AND finished_at <= ?3",
        params![start, end, now],
        |row| row.get(0),
    )?;
    Ok(Stats {
        focus_seconds,
        task_seconds,
        completed_sessions: usize::try_from(completed_sessions).map_err(|_| Error::InvalidData)?,
    })
}

fn initialize(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            kind TEXT NOT NULL CHECK (kind IN ('focus', 'rest')),
            archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1))
        );
        CREATE UNIQUE INDEX active_tag_name ON tags(name COLLATE NOCASE) WHERE archived = 0;
        CREATE TABLE sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER REFERENCES tasks(id) ON DELETE RESTRICT,
            title TEXT NOT NULL,
            tag_id INTEGER NOT NULL REFERENCES tags(id),
            tag_name TEXT NOT NULL,
            tag_kind TEXT NOT NULL CHECK (tag_kind IN ('focus', 'rest')),
            started_at INTEGER NOT NULL,
            finished_at INTEGER CHECK (finished_at IS NULL OR finished_at >= started_at),
            target_seconds INTEGER CHECK (target_seconds IS NULL OR target_seconds BETWEEN 60 AND 86400)
        );
        CREATE TABLE intervals (
            id INTEGER PRIMARY KEY,
            session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            started_at INTEGER NOT NULL,
            ended_at INTEGER CHECK (ended_at IS NULL OR ended_at >= started_at)
        );
        CREATE UNIQUE INDEX one_open_interval ON intervals(session_id) WHERE ended_at IS NULL;
        CREATE INDEX session_intervals ON intervals(session_id, started_at, id);
        CREATE INDEX interval_time_window ON intervals(ended_at, started_at);
        CREATE INDEX completed_history ON sessions(finished_at DESC, id DESC);
        CREATE TABLE preferences (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            goal_minutes INTEGER NOT NULL CHECK (goal_minutes BETWEEN 1 AND 1440),
            completion_sound INTEGER NOT NULL DEFAULT 1 CHECK (completion_sound IN (0, 1))
        );
        INSERT INTO preferences (id, goal_minutes) VALUES (1, 300);
        INSERT INTO tags (name, kind) VALUES ('工作', 'focus'), ('学习', 'focus'), ('阅读', 'focus'), ('休息', 'rest');
        PRAGMA user_version = 4;",
    )?;
    transaction.execute_batch(task::SCHEMA)?;
    transaction.execute_batch(agenda::SCHEMA)?;
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_v1(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        1 => transaction.execute_batch(
            "ALTER TABLE sessions ADD COLUMN target_seconds INTEGER
             CHECK (target_seconds IS NULL OR target_seconds BETWEEN 60 AND 86400);
             CREATE INDEX interval_time_window ON intervals(ended_at, started_at);
             PRAGMA user_version = 2;",
        )?,
        2 | 3 | 4 | SCHEMA_VERSION => (), // Another process completed the same migration.
        _ => return Err(Error::UnsupportedSchema),
    }
    transaction.commit()?;
    Ok(())
}

fn migrate_v3(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        3 => {
            transaction.execute_batch("ALTER TABLE preferences ADD COLUMN completion_sound INTEGER NOT NULL DEFAULT 1 CHECK (completion_sound IN (0, 1));")?;
            transaction.pragma_update(None, "user_version", 4)?;
        }
        4 | SCHEMA_VERSION => (),
        _ => return Err(Error::UnsupportedSchema),
    }
    transaction.commit()?;
    Ok(())
}

fn load_sessions(
    connection: &Connection,
    suffix: &str,
    parameters: impl rusqlite::Params,
) -> Result<Vec<Session>> {
    let transaction = connection.unchecked_transaction()?;
    let sessions = load_sessions_in_snapshot(&transaction, suffix, parameters)?;
    transaction.commit()?;
    Ok(sessions)
}

fn load_sessions_in_snapshot(
    connection: &Connection,
    suffix: &str,
    parameters: impl rusqlite::Params,
) -> Result<Vec<Session>> {
    // `suffix` is always a fixed internal query, never user-provided SQL.
    let mut statement = connection.prepare(&format!(
        "SELECT id, title, tag_id, tag_name, tag_kind, started_at, finished_at, target_seconds, task_id FROM sessions {suffix}"
    ))?;
    let mut rows = statement.query(parameters)?;
    let mut sessions = Vec::new();
    while let Some(row) = rows.next()? {
        sessions.push(Session {
            id: row.get(0)?,
            task_id: row.get(8)?,
            title: row.get(1)?,
            tag_id: row.get(2)?,
            tag_name: row.get(3)?,
            tag_kind: TagKind::parse(&row.get::<_, String>(4)?)?,
            started_at: row.get(5)?,
            finished_at: row.get(6)?,
            target_seconds: row.get(7)?,
            intervals: Vec::new(),
        });
    }
    // One indexed interval query per returned session avoids loading other history.
    let mut intervals = connection.prepare(
        "SELECT started_at, ended_at FROM intervals WHERE session_id = ?1 ORDER BY started_at, id",
    )?;
    for session in &mut sessions {
        session.intervals = intervals
            .query_map([session.id], |row| {
                Ok(Interval {
                    start: row.get(0)?,
                    end: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
    }
    Ok(sessions)
}

fn load_session(connection: &Connection, id: i64) -> Result<Session> {
    let session = connection.query_row(
        "SELECT title, tag_id, tag_name, tag_kind, started_at, finished_at, target_seconds, task_id FROM sessions WHERE id = ?1", [id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, i64>(4)?, row.get::<_, Option<i64>>(5)?, row.get::<_, Option<i64>>(6)?, row.get::<_, Option<i64>>(7)?)),
    ).optional()?.ok_or(Error::NotFound)?;
    let mut statement = connection.prepare(
        "SELECT started_at, ended_at FROM intervals WHERE session_id = ?1 ORDER BY started_at, id",
    )?;
    let intervals = statement
        .query_map([id], |row| {
            Ok(Interval {
                start: row.get(0)?,
                end: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(Session {
        id,
        task_id: session.7,
        title: session.0,
        tag_id: session.1,
        tag_name: session.2,
        tag_kind: TagKind::parse(&session.3)?,
        started_at: session.4,
        finished_at: session.5,
        target_seconds: session.6,
        intervals,
    })
}

fn last_boundary(session: &Session) -> i64 {
    session
        .intervals
        .last()
        .map(|i| i.end.unwrap_or(i.start))
        .unwrap_or(session.started_at)
}

fn finish_at_target(transaction: &Transaction<'_>, session: &Session, now: i64) -> Result<()> {
    let mut remaining = session.target_seconds.ok_or(Error::InvalidTransition)?;
    if session.elapsed_at(now) < remaining {
        return Err(Error::InvalidTransition);
    }
    let mut statement = transaction.prepare(
        "SELECT id, started_at, ended_at FROM intervals WHERE session_id = ?1 ORDER BY started_at, id",
    )?;
    let rows = statement.query_map([session.id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;
    let mut boundary = None;
    for row in rows {
        let (id, start, end) = row?;
        let duration = end.unwrap_or(now).saturating_sub(start).max(0);
        if duration >= remaining {
            boundary = Some((
                id,
                start,
                start.checked_add(remaining).ok_or(Error::InvalidData)?,
            ));
            break;
        }
        remaining -= duration;
    }
    let (interval_id, interval_start, finished_at) = boundary.ok_or(Error::InvalidData)?;
    // Clear later intervals in the same transaction; delayed actions after the
    // deadline cannot survive as extra investment or an open interval.
    transaction.execute(
        "DELETE FROM intervals WHERE session_id = ?1 AND
         (started_at > ?2 OR (started_at = ?2 AND id > ?3))",
        params![session.id, interval_start, interval_id],
    )?;
    transaction.execute(
        "UPDATE intervals SET ended_at = ?1 WHERE id = ?2",
        params![finished_at, interval_id],
    )?;
    transaction.execute(
        "UPDATE sessions SET finished_at = ?1 WHERE id = ?2",
        params![finished_at, session.id],
    )?;
    Ok(())
}

fn close_open_interval(transaction: &Transaction<'_>, session: &Session, now: i64) -> Result<()> {
    if now < last_boundary(session) {
        return Err(Error::InvalidTransition);
    }
    if transaction.execute(
        "UPDATE intervals SET ended_at = ?1 WHERE session_id = ?2 AND ended_at IS NULL",
        params![now, session.id],
    )? != 1
    {
        return Err(Error::InvalidTransition);
    }
    Ok(())
}

fn valid_text<'a>(value: &'a str, limit: usize, message: &'static str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > limit || value.contains('\0') {
        return Err(Error::Validation(message));
    }
    Ok(value)
}

// Mirror exact-target completion on the export's owned snapshot. Exporting must
// not depend on the UI tick or change a timer while a save dialog is open.
fn export_session_snapshot(mut session: Session, now: i64) -> Result<Session> {
    let Some(mut remaining) = session
        .target_seconds
        .filter(|_| session.finished_at.is_none())
    else {
        return Ok(session);
    };
    for index in 0..session.intervals.len() {
        let interval = &session.intervals[index];
        let duration = interval
            .end
            .unwrap_or(now)
            .saturating_sub(interval.start)
            .max(0);
        if duration >= remaining {
            let finished_at = interval
                .start
                .checked_add(remaining)
                .ok_or(Error::InvalidData)?;
            session.intervals[index].end = Some(finished_at);
            session.intervals.truncate(index + 1);
            session.finished_at = Some(finished_at);
            break;
        }
        remaining -= duration;
    }
    Ok(session)
}

fn csv_field(value: &str) -> String {
    // Spreadsheet applications may execute formulas even inside quoted CSV.
    // Ignore leading whitespace/control characters when detecting a formula.
    let first = value
        .trim_start_matches(|c: char| c.is_whitespace() || c.is_control())
        .chars()
        .next();
    let prefix =
        if matches!(first, Some('=' | '+' | '-' | '@')) || value.starts_with(['\t', '\r', '\n']) {
            "'"
        } else {
            ""
        };
    format!("\"{prefix}{}\"", value.replace('"', "\"\""))
}

fn utc_timestamp(timestamp: Option<i64>) -> String {
    timestamp
        .and_then(|value| DateTime::<Utc>::from_timestamp(value, 0))
        .map(|value| value.to_rfc3339_opts(SecondsFormat::Secs, true))
        .unwrap_or_default()
}

#[cfg(unix)]
fn private_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn private_directory(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn private_database_file(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        let file = options.open(path)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        Ok(file)
    }
    #[cfg(not(unix))]
    options.open(path)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod timed_tests;
