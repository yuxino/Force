//! User-tracked occurrences and dashboard preferences, independent of timers.

use chrono::{Datelike, NaiveDate};

use super::*;

const LIFE_TITLE_LIMIT: usize = 100;
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS life_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 100)
);
CREATE TABLE IF NOT EXISTS life_occurrences (
    item_id INTEGER NOT NULL REFERENCES life_items(id) ON DELETE CASCADE,
    occurred_at INTEGER NOT NULL,
    PRIMARY KEY (item_id, occurred_at)
);
CREATE TABLE IF NOT EXISTS life_board (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    visible_modules INTEGER NOT NULL CHECK (visible_modules BETWEEN 0 AND 7),
    chart_item INTEGER REFERENCES life_items(id) ON DELETE SET NULL
);
INSERT OR IGNORE INTO life_board (id, visible_modules) VALUES (1, 7);
";

pub(super) const DAY_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS life_day_occurrences (
    item_id INTEGER NOT NULL REFERENCES life_items(id) ON DELETE CASCADE,
    day TEXT NOT NULL,
    earliest INTEGER NOT NULL,
    latest INTEGER NOT NULL CHECK (latest >= earliest),
    PRIMARY KEY (item_id, day)
);
";

pub(super) const CHECK_IN_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS life_habits (
    item_id INTEGER PRIMARY KEY REFERENCES life_items(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS life_check_ins (
    entry_id INTEGER PRIMARY KEY REFERENCES daily_entries(id) ON DELETE CASCADE,
    item_id INTEGER NOT NULL REFERENCES life_habits(item_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS life_check_ins_by_item ON life_check_ins(item_id);
";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifeModule {
    Table = 1,
    Trend = 2,
    Total = 4,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LifeKind {
    #[default]
    CheckIn,
    Abstain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckInDay {
    pub day: NaiveDate,
    pub count: u64,
    pub minutes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CheckInSummary {
    pub streak_days: usize,
    pub total_days: usize,
    pub count: u64,
    pub minutes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifeItem {
    pub id: i64,
    pub title: String,
    pub kind: LifeKind,
    occurrences: Vec<LifeOccurrence>,
    check_in_days: Vec<CheckInDay>,
}

/// A date preserves the range in which the event could have occurred.
/// The bounds reflect the local day and the time at which it was recorded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifeOccurrence {
    Exact(i64),
    Day {
        date: NaiveDate,
        earliest: i64,
        latest: i64,
    },
}

impl From<i64> for LifeOccurrence {
    fn from(value: i64) -> Self {
        Self::Exact(value)
    }
}

impl LifeOccurrence {
    fn earliest(self) -> i64 {
        match self {
            Self::Exact(at) => at,
            Self::Day { earliest, .. } => earliest,
        }
    }

    fn latest(self) -> i64 {
        match self {
            Self::Exact(at) => at,
            Self::Day { latest, .. } => latest,
        }
    }

    fn order_key(self) -> (i64, bool, Option<NaiveDate>) {
        match self {
            Self::Exact(at) => (at, true, None),
            Self::Day { date, latest, .. } => (latest, false, Some(date)),
        }
    }

    pub fn is_day(self) -> bool {
        matches!(self, Self::Day { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifeSpan {
    pub start: LifeOccurrence,
    pub end: Option<LifeOccurrence>,
    pub seconds: i64,
}

impl LifeSpan {
    pub fn is_estimated(self) -> bool {
        self.start.is_day() || self.end.is_some_and(LifeOccurrence::is_day)
    }
}

impl LifeItem {
    pub fn last_occurrence(&self) -> Option<LifeOccurrence> {
        self.occurrences.last().copied()
    }

    pub fn elapsed_at(&self, now: i64) -> i64 {
        self.last_occurrence()
            .map_or(0, |last| elapsed(last.latest(), now))
    }

    pub fn check_in_days(&self) -> &[CheckInDay] {
        &self.check_in_days
    }

    pub fn check_in_summary(&self, today: NaiveDate) -> CheckInSummary {
        let mut summary = CheckInSummary::default();
        let mut next_day = today;
        let mut streak_open = true;
        for entry in self
            .check_in_days
            .iter()
            .rev()
            .filter(|entry| entry.day <= today)
        {
            summary.total_days += 1;
            summary.count += entry.count;
            if let Some(minutes) = entry.minutes {
                summary.minutes = Some(summary.minutes.unwrap_or_default() + minutes);
            }
            if streak_open {
                // Until today ends, yesterday's streak can still be continued.
                let gap = next_day.signed_duration_since(entry.day).num_days();
                if gap == 1 || (summary.streak_days == 0 && gap == 0) {
                    summary.streak_days += 1;
                    next_day = entry.day;
                } else {
                    streak_open = false;
                }
            }
        }
        summary
    }

    pub fn has_date_only_entries(&self) -> bool {
        self.occurrences
            .iter()
            .any(|occurrence| occurrence.is_day())
    }

    pub fn cumulative_days(&self, now: i64) -> i64 {
        let finished: i64 = self
            .occurrences
            .windows(2)
            .map(|pair| elapsed(pair[0].latest(), pair[1].earliest()) / SECONDS_PER_DAY)
            .sum();
        finished + self.elapsed_at(now) / SECONDS_PER_DAY
    }

    /// Up to limit spans, oldest first, with the current open span last.
    pub fn recent_spans(&self, now: i64, limit: usize) -> Vec<LifeSpan> {
        let recent = &self.occurrences[self.occurrences.len().saturating_sub(limit)..];
        recent
            .iter()
            .enumerate()
            .map(|(index, &start)| {
                let end = recent.get(index + 1).copied();
                LifeSpan {
                    start,
                    end,
                    seconds: elapsed(start.latest(), end.map_or(now, LifeOccurrence::earliest)),
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifeBoard {
    pub items: Vec<LifeItem>,
    pub chart_item: Option<i64>,
    visible_modules: u8,
}

impl LifeBoard {
    pub fn shows(&self, module: LifeModule) -> bool {
        self.visible_modules & module as u8 != 0
    }
}

pub(super) fn migrate_v8(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        8 => {
            transaction.execute_batch(SCHEMA)?;
            transaction.pragma_update(None, "user_version", 9)?;
        }
        9..=SCHEMA_VERSION => (),
        _ => return Err(Error::UnsupportedSchema),
    }
    transaction.commit()?;
    Ok(())
}

pub(super) fn migrate_v10(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        10 => {
            transaction.execute_batch(CHECK_IN_SCHEMA)?;
            load_board(&transaction)?;
            transaction.pragma_update(None, "user_version", 11)?;
        }
        SCHEMA_VERSION => (),
        _ => return Err(Error::UnsupportedSchema),
    }
    transaction.commit()?;
    Ok(())
}

impl Store {
    pub fn life_board(&self) -> Result<LifeBoard> {
        let transaction = self.connection.unchecked_transaction()?;
        let board = load_board(&transaction)?;
        transaction.commit()?;
        Ok(board)
    }

    pub fn create_life_item(
        &mut self,
        title: &str,
        occurrence: impl Into<LifeOccurrence>,
        now: i64,
    ) -> Result<i64> {
        let title = valid_title(title)?;
        let occurrence = occurrence.into();
        validate_time(occurrence, now)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute("INSERT INTO life_items (title) VALUES (?1)", [title])?;
        let id = transaction.last_insert_rowid();
        insert_occurrence(&transaction, id, occurrence)?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn create_life_habit(&mut self, title: &str) -> Result<i64> {
        let title = valid_title(title)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_unique_habit(&transaction, title, None)?;
        transaction.execute("INSERT INTO life_items (title) VALUES (?1)", [title])?;
        let id = transaction.last_insert_rowid();
        transaction.execute("INSERT INTO life_habits (item_id) VALUES (?1)", [id])?;
        transaction.execute(
            "INSERT INTO life_check_ins (item_id, entry_id)
             SELECT ?1, entry.id FROM daily_entries entry
             WHERE entry.content = ?2
               AND NOT EXISTS (SELECT 1 FROM life_check_ins WHERE entry_id = entry.id)",
            params![id, title],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn record_life_check_in(
        &mut self,
        original: &LifeItem,
        day: NaiveDate,
        minutes: Option<u16>,
        today: NaiveDate,
    ) -> Result<()> {
        if original.kind != LifeKind::CheckIn {
            return Err(Error::InvalidTransition);
        }
        let title = daily::validate_entry(day, &original.title, minutes, today)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_baseline(&transaction, original)?;
        transaction.execute(
            "INSERT INTO daily_entries (day, content, duration_minutes) VALUES (?1, ?2, ?3)",
            params![day.to_string(), title, minutes],
        )?;
        transaction.execute(
            "INSERT INTO life_check_ins (item_id, entry_id) VALUES (?1, ?2)",
            params![original.id, transaction.last_insert_rowid()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn rename_life_habit(&mut self, original: &LifeItem, title: &str) -> Result<()> {
        if original.kind != LifeKind::CheckIn {
            return Err(Error::InvalidTransition);
        }
        let title = valid_title(title)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_baseline(&transaction, original)?;
        ensure_unique_habit(&transaction, title, Some(original.id))?;
        transaction.execute(
            "UPDATE life_items SET title = ?1 WHERE id = ?2",
            params![title, original.id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn record_life_occurrence(
        &mut self,
        original: &LifeItem,
        occurrence: impl Into<LifeOccurrence>,
        now: i64,
    ) -> Result<()> {
        let occurrence = occurrence.into();
        validate_time(occurrence, now)?;
        reject_duplicate_day(&original.occurrences, occurrence)?;
        let last = original.last_occurrence().ok_or(Error::InvalidTransition)?;
        if occurrence.order_key() <= last.order_key() {
            return Err(Error::Validation("Choose a time after the last occurrence"));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_baseline(&transaction, original)?;
        insert_occurrence(&transaction, original.id, occurrence)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn edit_life_item(
        &mut self,
        original: &LifeItem,
        title: &str,
        occurrence: impl Into<LifeOccurrence>,
        now: i64,
    ) -> Result<()> {
        let title = valid_title(title)?;
        let occurrence = occurrence.into();
        validate_time(occurrence, now)?;
        let (&last, earlier) = original
            .occurrences
            .split_last()
            .ok_or(Error::InvalidTransition)?;
        reject_duplicate_day(earlier, occurrence)?;
        if original
            .occurrences
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|&previous| occurrence.order_key() <= previous.order_key())
        {
            return Err(Error::Validation(
                "Choose a time after the previous occurrence",
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_baseline(&transaction, original)?;
        transaction.execute(
            "UPDATE life_items SET title = ?1 WHERE id = ?2",
            params![title, original.id],
        )?;
        if occurrence != last {
            remove_occurrence(&transaction, original.id, last)?;
            insert_occurrence(&transaction, original.id, occurrence)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_life_item(&mut self, original: &LifeItem) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_baseline(&transaction, original)?;
        transaction.execute("DELETE FROM life_items WHERE id = ?1", [original.id])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn set_life_module(&mut self, module: LifeModule, visible: bool) -> Result<()> {
        let bit = module as u8;
        if self.connection.execute(
            "UPDATE life_board SET visible_modules = (visible_modules & ~?1) | ?2 WHERE id = 1",
            params![bit, if visible { bit } else { 0 }],
        )? != 1
        {
            return Err(Error::InvalidData);
        }
        Ok(())
    }

    pub fn set_life_chart_item(&mut self, id: i64) -> Result<()> {
        if self.connection.execute(
            "UPDATE life_board SET chart_item = ?1
             WHERE id = 1 AND EXISTS (SELECT 1 FROM life_items WHERE id = ?1)",
            [id],
        )? != 1
        {
            return Err(Error::NotFound);
        }
        Ok(())
    }
}

fn elapsed(start: i64, end: i64) -> i64 {
    end.saturating_sub(start).max(0)
}

fn ensure_unique_habit(connection: &Connection, title: &str, except: Option<i64>) -> Result<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM life_habits habit JOIN life_items item ON item.id = habit.item_id
            WHERE item.title = ?1 AND (?2 IS NULL OR item.id != ?2)
        )",
        params![title, except],
        |row| row.get(0),
    )?;
    if exists {
        return Err(Error::Validation("A habit with this name already exists"));
    }
    Ok(())
}

fn reject_duplicate_day(existing: &[LifeOccurrence], occurrence: LifeOccurrence) -> Result<()> {
    if let LifeOccurrence::Day { date, .. } = occurrence
        && existing.iter().any(
            |previous| matches!(previous, LifeOccurrence::Day { date: day, .. } if *day == date),
        )
    {
        return Err(Error::Validation(
            "This date already has a date-only occurrence",
        ));
    }
    Ok(())
}

fn valid_title(title: &str) -> Result<&str> {
    let message = "Enter an item name of 1–100 characters on one line";
    let title = valid_text(title, LIFE_TITLE_LIMIT, message)?;
    if title.chars().any(char::is_control) {
        return Err(Error::Validation(message));
    }
    Ok(title)
}

fn validate_time(occurrence: LifeOccurrence, now: i64) -> Result<()> {
    let earliest = occurrence.earliest();
    let latest = occurrence.latest();
    if DateTime::<Utc>::from_timestamp(earliest, 0).is_none()
        || DateTime::<Utc>::from_timestamp(latest, 0).is_none()
        || latest < earliest
        || latest > now
        || matches!(occurrence, LifeOccurrence::Day { date, .. } if !(1..=9999).contains(&date.year()))
    {
        return Err(Error::Validation(
            "Choose an occurrence time at or before now",
        ));
    }
    Ok(())
}

fn insert_occurrence(connection: &Connection, id: i64, occurrence: LifeOccurrence) -> Result<()> {
    match occurrence {
        LifeOccurrence::Exact(at) => connection.execute(
            "INSERT INTO life_occurrences (item_id, occurred_at) VALUES (?1, ?2)", params![id, at],
        )?,
        LifeOccurrence::Day { date, earliest, latest } => connection.execute(
            "INSERT INTO life_day_occurrences (item_id, day, earliest, latest) VALUES (?1, ?2, ?3, ?4)",
            params![id, date.to_string(), earliest, latest],
        )?,
    };
    Ok(())
}

fn remove_occurrence(connection: &Connection, id: i64, occurrence: LifeOccurrence) -> Result<()> {
    match occurrence {
        LifeOccurrence::Exact(at) => connection.execute(
            "DELETE FROM life_occurrences WHERE item_id = ?1 AND occurred_at = ?2",
            params![id, at],
        )?,
        LifeOccurrence::Day { date, .. } => connection.execute(
            "DELETE FROM life_day_occurrences WHERE item_id = ?1 AND day = ?2",
            params![id, date.to_string()],
        )?,
    };
    Ok(())
}

fn verify_baseline(connection: &Connection, original: &LifeItem) -> Result<()> {
    match load_items(connection, Some(original.id))?.first() {
        Some(item) if item == original => Ok(()),
        Some(_) => Err(Error::InvalidTransition),
        None => Err(Error::NotFound),
    }
}

pub(super) fn load_board(connection: &Connection) -> Result<LifeBoard> {
    let (visible_modules, chart_item) = connection.query_row(
        "SELECT visible_modules, chart_item FROM life_board WHERE id = 1",
        [],
        |row| Ok((row.get::<_, u8>(0)?, row.get(1)?)),
    )?;
    if visible_modules > 7 {
        return Err(Error::InvalidData);
    }
    Ok(LifeBoard {
        items: load_items(connection, None)?,
        chart_item,
        visible_modules,
    })
}

fn load_items(connection: &Connection, only_id: Option<i64>) -> Result<Vec<LifeItem>> {
    let mut statement = connection.prepare(
        "WITH occurrences AS (
            SELECT item_id, occurred_at, NULL AS day, occurred_at AS earliest, occurred_at AS latest
            FROM life_occurrences
            UNION ALL
            SELECT item_id, NULL, day, earliest, latest FROM life_day_occurrences
         )
         SELECT item.id, item.title, occurrence.occurred_at, occurrence.day,
                occurrence.earliest, occurrence.latest, habit.item_id IS NOT NULL
         FROM life_items AS item
         LEFT JOIN occurrences AS occurrence ON occurrence.item_id = item.id
         LEFT JOIN life_habits AS habit ON habit.item_id = item.id
         WHERE ?1 IS NULL OR item.id = ?1
         ORDER BY item.id, occurrence.latest, occurrence.occurred_at IS NOT NULL, occurrence.day",
    )?;
    let mut rows = statement.query([only_id])?;
    let mut items: Vec<LifeItem> = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let kind = if row.get::<_, bool>(6)? {
            LifeKind::CheckIn
        } else {
            LifeKind::Abstain
        };
        let occurrence = if let Some(at) = row.get::<_, Option<i64>>(2)? {
            Some(LifeOccurrence::Exact(at))
        } else if let Some(day) = row.get::<_, Option<String>>(3)? {
            Some(LifeOccurrence::Day {
                date: NaiveDate::parse_from_str(&day, "%Y-%m-%d")
                    .map_err(|_| Error::InvalidData)?,
                earliest: row.get(4)?,
                latest: row.get(5)?,
            })
        } else {
            None
        };
        if let Some(occurrence) = occurrence {
            validate_time(occurrence, i64::MAX).map_err(|_| Error::InvalidData)?;
        }
        if (kind == LifeKind::Abstain) != occurrence.is_some() {
            return Err(Error::InvalidData);
        }
        if let Some(item) = items.last_mut().filter(|item| item.id == id) {
            item.occurrences.extend(occurrence);
        } else {
            let title: String = row.get(1)?;
            valid_title(&title).map_err(|_| Error::InvalidData)?;
            items.push(LifeItem {
                id,
                title,
                kind,
                occurrences: occurrence.into_iter().collect(),
                check_in_days: Vec::new(),
            });
        }
    }
    let mut statement = connection.prepare(
        "SELECT link.item_id, entry.day, COUNT(*), SUM(entry.duration_minutes)
         FROM life_check_ins link JOIN daily_entries entry ON entry.id = link.entry_id
         WHERE ?1 IS NULL OR link.item_id = ?1
         GROUP BY link.item_id, entry.day
         ORDER BY link.item_id, entry.day",
    )?;
    let mut rows = statement.query([only_id])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let day: String = row.get(1)?;
        let day = NaiveDate::parse_from_str(&day, "%Y-%m-%d").map_err(|_| Error::InvalidData)?;
        let index = items
            .binary_search_by_key(&id, |item| item.id)
            .map_err(|_| Error::InvalidData)?;
        items[index].check_in_days.push(CheckInDay {
            day,
            count: row.get(2)?,
            minutes: row.get(3)?,
        });
    }
    Ok(items)
}

#[cfg(test)]
mod tests;
