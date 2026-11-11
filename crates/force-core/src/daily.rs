//! Entries whose calendar date and optional duration are known.

use chrono::{Datelike, NaiveDate};

use super::*;

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS daily_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    day TEXT NOT NULL,
    content TEXT NOT NULL CHECK (length(content) BETWEEN 1 AND 2000),
    duration_minutes INTEGER CHECK (duration_minutes BETWEEN 1 AND 1440)
);
CREATE INDEX IF NOT EXISTS daily_entries_by_day ON daily_entries(day, id);
";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DailyEntry {
    pub id: i64,
    pub day: NaiveDate,
    pub content: String,
    pub duration_minutes: Option<u16>,
}

pub(super) fn migrate_v9(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        9 => {
            transaction.execute_batch(SCHEMA)?;
            transaction.execute_batch(life::DAY_SCHEMA)?;
            transaction.prepare("SELECT id, day, content, duration_minutes FROM daily_entries")?;
            life::load_board(&transaction)?;
            transaction.pragma_update(None, "user_version", 10)?;
        }
        SCHEMA_VERSION => (),
        _ => return Err(Error::UnsupportedSchema),
    }
    transaction.commit()?;
    Ok(())
}

impl Store {
    pub fn daily_entries(&self, day: NaiveDate) -> Result<Vec<DailyEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT id, content, duration_minutes FROM daily_entries WHERE day = ?1 ORDER BY id",
        )?;
        let rows = statement.query_map([day.to_string()], |row| {
            Ok(DailyEntry {
                id: row.get(0)?,
                day,
                content: row.get(1)?,
                duration_minutes: row.get(2)?,
            })
        })?;
        rows.map(|row| Ok(row?)).collect()
    }

    pub fn create_daily_entry(
        &mut self,
        day: NaiveDate,
        content: &str,
        duration_minutes: Option<u16>,
        today: NaiveDate,
    ) -> Result<()> {
        let content = validate_entry(day, content, duration_minutes, today)?;
        self.connection.execute(
            "INSERT INTO daily_entries (day, content, duration_minutes) VALUES (?1, ?2, ?3)",
            params![day.to_string(), content, duration_minutes],
        )?;
        Ok(())
    }

    pub fn update_daily_entry(
        &mut self,
        original: &DailyEntry,
        day: NaiveDate,
        content: &str,
        duration_minutes: Option<u16>,
        today: NaiveDate,
    ) -> Result<()> {
        let content = validate_entry(day, content, duration_minutes, today)?;
        let changed = self.connection.execute(
            "UPDATE daily_entries SET day = ?1, content = ?2, duration_minutes = ?3
             WHERE id = ?4 AND day = ?5 AND content = ?6 AND duration_minutes IS ?7",
            params![
                day.to_string(),
                content,
                duration_minutes,
                original.id,
                original.day.to_string(),
                original.content,
                original.duration_minutes
            ],
        )?;
        if changed != 1 {
            return Err(Error::InvalidTransition);
        }
        Ok(())
    }

    pub fn delete_daily_entry(&mut self, original: &DailyEntry) -> Result<()> {
        let changed = self.connection.execute(
            "DELETE FROM daily_entries
             WHERE id = ?1 AND day = ?2 AND content = ?3 AND duration_minutes IS ?4",
            params![
                original.id,
                original.day.to_string(),
                original.content,
                original.duration_minutes
            ],
        )?;
        if changed != 1 {
            return Err(Error::InvalidTransition);
        }
        Ok(())
    }
}

fn validate_entry(
    day: NaiveDate,
    content: &str,
    minutes: Option<u16>,
    today: NaiveDate,
) -> Result<&str> {
    if !(1..=9999).contains(&day.year()) || day > today {
        return Err(Error::Validation("Choose a date on or before today"));
    }
    if minutes.is_some_and(|minutes| !(1..=1440).contains(&minutes)) {
        return Err(Error::Validation("Enter a duration from 1 to 1440 minutes"));
    }
    valid_text(
        content,
        MOMENT_CONTENT_LIMIT,
        "Enter a daily entry of 1–2000 characters",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v9_upgrade_keeps_exact_entries_and_stores_daily_duration_separately() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("force.sqlite3");
        let mut old = Store::open(&path).unwrap();
        old.create_life_item("阅读", 100, 200).unwrap();
        old.connection.execute_batch(
            "DROP TABLE daily_entries; DROP TABLE life_day_occurrences; PRAGMA user_version = 9;",
        ).unwrap();
        drop(old);

        let today = NaiveDate::from_ymd_opt(2026, 2, 3).unwrap();
        let day = today.pred_opt().unwrap();
        let mut store = Store::open(&path).unwrap();
        assert_eq!(
            store.life_board().unwrap().items[0].last_occurrence(),
            LifeOccurrence::Exact(100)
        );
        store
            .create_daily_entry(day, "  拉伸  ", Some(45), today)
            .unwrap();
        store
            .create_daily_entry(today, "散步", None, today)
            .unwrap();
        assert!(
            store
                .create_daily_entry(today.succ_opt().unwrap(), "拉伸", None, today)
                .is_err()
        );
        drop(store);

        let mut reopened = Store::open(&path).unwrap();
        let entry = reopened.daily_entries(day).unwrap().remove(0);
        assert_eq!(entry.content, "拉伸");
        assert_eq!(entry.duration_minutes, Some(45));
        assert_eq!(
            reopened.daily_entries(today).unwrap()[0].duration_minutes,
            None
        );
        reopened
            .update_daily_entry(&entry, today, "拉伸", Some(30), today)
            .unwrap();
        assert!(matches!(
            reopened.delete_daily_entry(&entry),
            Err(Error::InvalidTransition)
        ));
        assert!(reopened.daily_entries(day).unwrap().is_empty());
        assert!(reopened.moments_between(0, i64::MAX).unwrap().is_empty());
        assert_eq!(reopened.stats(0, 10_000, 10_000).unwrap().focus_seconds, 0);
    }
}
