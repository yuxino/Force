//! Shared classification colors are display preferences, separate from timer accounting.

use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClassColors {
    pub positive: Option<String>,
    pub neutral: Option<String>,
    pub negative: Option<String>,
    pub leisure: Option<String>,
}

impl ClassColors {
    pub fn color(&self, class: TagClass) -> Option<&str> {
        match class {
            TagClass::Positive => self.positive.as_deref(),
            TagClass::Neutral => self.neutral.as_deref(),
            TagClass::Negative => self.negative.as_deref(),
            TagClass::Leisure => self.leisure.as_deref(),
            TagClass::Legacy => None,
        }
    }

    /// Explicit tag colors win; an absent override follows its classification.
    pub fn resolve<'a>(
        &'a self,
        class: TagClass,
        override_color: Option<&'a str>,
    ) -> Option<&'a str> {
        override_color.or_else(|| self.color(class))
    }
}

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS class_colors (
    class TEXT PRIMARY KEY CHECK (class IN ('positive', 'neutral', 'negative', 'leisure')),
    color TEXT CHECK (color IS NULL OR (length(color) = 7 AND substr(color, 1, 1) = '#' AND substr(color, 2) NOT GLOB '*[^0-9A-Fa-f]*'))
);
INSERT OR IGNORE INTO class_colors (class, color) VALUES
    ('positive', NULL), ('neutral', NULL), ('negative', NULL), ('leisure', NULL);
";

pub(super) fn migrate_v7(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        7 => {
            transaction.execute_batch(SCHEMA)?;
            // Validate an existing table before claiming that the upgrade succeeded.
            load_class_colors(&transaction)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        SCHEMA_VERSION => (),
        _ => return Err(Error::UnsupportedSchema),
    }
    transaction.commit()?;
    Ok(())
}

impl Store {
    pub fn class_colors(&self) -> Result<ClassColors> {
        load_class_colors(&self.connection)
    }

    pub fn set_class_color(&mut self, class: TagClass, color: Option<&str>) -> Result<()> {
        if class == TagClass::Legacy {
            return Err(Error::Validation("Choose a classification"));
        }
        let color = valid_color(color)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if transaction.execute(
            "UPDATE class_colors SET color = ?1 WHERE class = ?2",
            params![color, class.as_str()],
        )? != 1
        {
            return Err(Error::InvalidData);
        }
        transaction.commit()?;
        Ok(())
    }
}

fn load_class_colors(connection: &Connection) -> Result<ClassColors> {
    let mut statement = connection.prepare("SELECT class, color FROM class_colors")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    let mut colors = ClassColors::default();
    let mut seen = [false; 4];
    for row in rows {
        let (class, color) = row?;
        let (index, slot) = match TagClass::parse(&class)? {
            TagClass::Positive => (0, &mut colors.positive),
            TagClass::Neutral => (1, &mut colors.neutral),
            TagClass::Negative => (2, &mut colors.negative),
            TagClass::Leisure => (3, &mut colors.leisure),
            TagClass::Legacy => return Err(Error::InvalidData),
        };
        if seen[index] {
            return Err(Error::InvalidData);
        }
        seen[index] = true;
        *slot = valid_color(color.as_deref()).map_err(|_| Error::InvalidData)?;
    }
    if seen.into_iter().any(|present| !present) {
        return Err(Error::InvalidData);
    }
    Ok(colors)
}

#[cfg(test)]
mod tests;
