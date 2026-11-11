//! User-tracked occurrences and dashboard preferences, independent of timers.

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifeModule {
    Table = 1,
    Trend = 2,
    Total = 4,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifeItem {
    pub id: i64,
    pub title: String,
    occurrences: Vec<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifeSpan {
    pub start: i64,
    pub end: Option<i64>,
    pub seconds: i64,
}

impl LifeItem {
    pub fn last_occurred_at(&self) -> i64 {
        *self
            .occurrences
            .last()
            .expect("a tracked item has a baseline")
    }

    pub fn elapsed_at(&self, now: i64) -> i64 {
        elapsed(self.last_occurred_at(), now)
    }

    pub fn cumulative_days(&self, now: i64) -> i64 {
        let finished: i64 = self
            .occurrences
            .windows(2)
            .map(|pair| elapsed(pair[0], pair[1]) / SECONDS_PER_DAY)
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
                    seconds: elapsed(start, end.unwrap_or(now)),
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
            load_board(&transaction)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
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

    pub fn create_life_item(&mut self, title: &str, occurred_at: i64, now: i64) -> Result<i64> {
        let title = valid_title(title)?;
        validate_time(occurred_at, now)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute("INSERT INTO life_items (title) VALUES (?1)", [title])?;
        let id = transaction.last_insert_rowid();
        insert_occurrence(&transaction, id, occurred_at)?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn record_life_occurrence(
        &mut self,
        original: &LifeItem,
        occurred_at: i64,
        now: i64,
    ) -> Result<()> {
        validate_time(occurred_at, now)?;
        if occurred_at <= original.last_occurred_at() {
            return Err(Error::Validation("Choose a time after the last occurrence"));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_baseline(&transaction, original)?;
        insert_occurrence(&transaction, original.id, occurred_at)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn edit_life_item(
        &mut self,
        original: &LifeItem,
        title: &str,
        occurred_at: i64,
        now: i64,
    ) -> Result<()> {
        let title = valid_title(title)?;
        validate_time(occurred_at, now)?;
        if original
            .occurrences
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|&previous| occurred_at <= previous)
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
        transaction.execute(
            "UPDATE life_occurrences SET occurred_at = ?1 WHERE item_id = ?2 AND occurred_at = ?3",
            params![occurred_at, original.id, original.last_occurred_at()],
        )?;
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

fn valid_title(title: &str) -> Result<&str> {
    let message = "Enter an item name of 1–100 characters on one line";
    let title = valid_text(title, LIFE_TITLE_LIMIT, message)?;
    if title.chars().any(char::is_control) {
        return Err(Error::Validation(message));
    }
    Ok(title)
}

fn validate_time(occurred_at: i64, now: i64) -> Result<()> {
    if DateTime::<Utc>::from_timestamp(occurred_at, 0).is_none() || occurred_at > now {
        return Err(Error::Validation(
            "Choose an occurrence time at or before now",
        ));
    }
    Ok(())
}

fn insert_occurrence(connection: &Connection, id: i64, occurred_at: i64) -> Result<()> {
    connection.execute(
        "INSERT INTO life_occurrences (item_id, occurred_at) VALUES (?1, ?2)",
        params![id, occurred_at],
    )?;
    Ok(())
}

fn verify_baseline(connection: &Connection, original: &LifeItem) -> Result<()> {
    let current: Option<(String, i64)> = connection
        .query_row(
            "SELECT title, (SELECT MAX(occurred_at) FROM life_occurrences WHERE item_id = life_items.id)
             FROM life_items WHERE id = ?1",
            [original.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match current {
        Some((title, time)) if title == original.title && time == original.last_occurred_at() => {
            Ok(())
        }
        Some(_) => Err(Error::InvalidTransition),
        None => Err(Error::NotFound),
    }
}

fn load_board(connection: &Connection) -> Result<LifeBoard> {
    let (visible_modules, chart_item) = connection.query_row(
        "SELECT visible_modules, chart_item FROM life_board WHERE id = 1",
        [],
        |row| Ok((row.get::<_, u8>(0)?, row.get(1)?)),
    )?;
    if visible_modules > 7 {
        return Err(Error::InvalidData);
    }
    let mut statement = connection.prepare(
        "SELECT item.id, item.title, occurrence.occurred_at FROM life_items AS item
         LEFT JOIN life_occurrences AS occurrence ON occurrence.item_id = item.id
         ORDER BY item.id, occurrence.occurred_at",
    )?;
    let mut rows = statement.query([])?;
    let mut items: Vec<LifeItem> = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let occurred_at: i64 = row.get(2)?;
        if DateTime::<Utc>::from_timestamp(occurred_at, 0).is_none() {
            return Err(Error::InvalidData);
        }
        if let Some(item) = items.last_mut().filter(|item| item.id == id) {
            item.occurrences.push(occurred_at);
        } else {
            let title: String = row.get(1)?;
            valid_title(&title).map_err(|_| Error::InvalidData)?;
            items.push(LifeItem {
                id,
                title,
                occurrences: vec![occurred_at],
            });
        }
    }
    Ok(LifeBoard {
        items,
        chart_item,
        visible_modules,
    })
}

#[cfg(test)]
mod tests;
