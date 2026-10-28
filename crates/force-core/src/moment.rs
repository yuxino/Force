//! Point-in-time notes are independent from tasks and measured intervals.

use rusqlite::Row;

use super::*;

pub const MOMENT_CONTENT_LIMIT: usize = 2000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Moment {
    pub id: i64,
    pub content: String,
    pub occurred_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS moments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL CHECK (length(content) BETWEEN 1 AND 2000),
    occurred_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (updated_at >= created_at),
    deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
);
CREATE INDEX IF NOT EXISTS visible_moment_times ON moments(occurred_at, id) WHERE deleted = 0;
";

const SELECT_MOMENT: &str = "SELECT id, content, occurred_at, created_at, updated_at FROM moments";

pub(super) fn migrate_v6(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        6 => {
            transaction.execute_batch(SCHEMA)?;
            // Do not mark a partial/foreign table as migrated when required
            // fields are missing; the transaction rolls back its index too.
            transaction.prepare(SELECT_MOMENT)?;
            transaction.pragma_update(None, "user_version", 7)?;
        }
        7 | SCHEMA_VERSION => (),
        _ => return Err(Error::UnsupportedSchema),
    }
    transaction.commit()?;
    Ok(())
}

impl Store {
    pub fn create_moment(&mut self, content: &str, occurred_at: i64, now: i64) -> Result<Moment> {
        let content = valid_moment_content(content)?;
        valid_moment_timestamp(occurred_at)?;
        valid_moment_timestamp(now)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO moments (content, occurred_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![content, occurred_at, now],
        )?;
        let moment = load_moment(&transaction, transaction.last_insert_rowid())?;
        transaction.commit()?;
        Ok(moment)
    }

    pub fn update_moment(
        &mut self,
        id: i64,
        content: &str,
        occurred_at: i64,
        now: i64,
    ) -> Result<Moment> {
        let content = valid_moment_content(content)?;
        valid_moment_timestamp(occurred_at)?;
        valid_moment_timestamp(now)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if transaction.execute(
            "UPDATE moments SET content = ?1, occurred_at = ?2, updated_at = MAX(updated_at, ?3) WHERE id = ?4 AND deleted = 0",
            params![content, occurred_at, now, id],
        )? != 1 {
            return Err(Error::NotFound);
        }
        let moment = load_moment(&transaction, id)?;
        transaction.commit()?;
        Ok(moment)
    }

    pub fn delete_moment(&mut self, id: i64) -> Result<()> {
        if self.connection.execute(
            "UPDATE moments SET deleted = 1 WHERE id = ?1 AND deleted = 0",
            [id],
        )? != 1
        {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Query a half-open range in timestamp/id order, without a history cap.
    /// Notes have no duration and do not participate in session statistics.
    pub fn moments_between(&self, start: i64, end: i64) -> Result<Vec<Moment>> {
        if start >= end {
            return Err(Error::Validation("The end must be later than the start"));
        }
        let mut statement = self.connection.prepare(&format!(
            "{SELECT_MOMENT} WHERE deleted = 0 AND occurred_at >= ?1 AND occurred_at < ?2 ORDER BY occurred_at, id"
        ))?;
        statement
            .query_map(params![start, end], read_moment)?
            .map(|row| validate_stored_moment(row?))
            .collect()
    }
}

fn valid_moment_content(value: &str) -> Result<&str> {
    valid_text(
        value,
        MOMENT_CONTENT_LIMIT,
        "Enter a note of 1–2000 characters",
    )
}

fn valid_moment_timestamp(timestamp: i64) -> Result<()> {
    if DateTime::<Utc>::from_timestamp(timestamp, 0).is_none() {
        return Err(Error::Validation("Enter a valid note time"));
    }
    Ok(())
}

fn read_moment(row: &Row<'_>) -> rusqlite::Result<Moment> {
    Ok(Moment {
        id: row.get(0)?,
        content: row.get(1)?,
        occurred_at: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn validate_stored_moment(moment: Moment) -> Result<Moment> {
    if valid_moment_content(&moment.content).is_err()
        || valid_moment_timestamp(moment.occurred_at).is_err()
        || valid_moment_timestamp(moment.created_at).is_err()
        || valid_moment_timestamp(moment.updated_at).is_err()
        || moment.updated_at < moment.created_at
    {
        return Err(Error::InvalidData);
    }
    Ok(moment)
}

fn load_moment(connection: &Connection, id: i64) -> Result<Moment> {
    let moment = connection
        .query_row(
            &format!("{SELECT_MOMENT} WHERE id = ?1 AND deleted = 0"),
            [id],
            read_moment,
        )
        .optional()?
        .ok_or(Error::NotFound)?;
    validate_stored_moment(moment)
}

#[cfg(test)]
mod tests;
