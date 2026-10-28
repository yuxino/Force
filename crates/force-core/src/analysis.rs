//! Read-only, batched analysis of persisted timer intervals.

use rusqlite::params;

use crate::{Error, Result, Store, TagClass, TagKind};

/// Focus is merged wall-clock time. Investment is additive across records,
/// including overlapping timers and rest; classes use each session's snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DailyAnalysis {
    pub focus_seconds: i64,
    pub task_seconds: i64,
    pub positive_seconds: i64,
    pub negative_seconds: i64,
    pub neutral_seconds: i64,
    pub leisure_seconds: i64,
    pub legacy_seconds: i64,
}

impl DailyAnalysis {
    pub fn class_seconds(self, class: TagClass) -> i64 {
        match class {
            TagClass::Positive => self.positive_seconds,
            TagClass::Negative => self.negative_seconds,
            TagClass::Neutral => self.neutral_seconds,
            TagClass::Leisure => self.leisure_seconds,
            TagClass::Legacy => self.legacy_seconds,
        }
    }

    /// Adds disjoint time buckets. This does not merge overlapping buckets.
    pub fn add_bucket(&mut self, other: Self) {
        self.focus_seconds = self.focus_seconds.saturating_add(other.focus_seconds);
        self.task_seconds = self.task_seconds.saturating_add(other.task_seconds);
        self.positive_seconds = self.positive_seconds.saturating_add(other.positive_seconds);
        self.negative_seconds = self.negative_seconds.saturating_add(other.negative_seconds);
        self.neutral_seconds = self.neutral_seconds.saturating_add(other.neutral_seconds);
        self.leisure_seconds = self.leisure_seconds.saturating_add(other.leisure_seconds);
        self.legacy_seconds = self.legacy_seconds.saturating_add(other.legacy_seconds);
    }

    fn add_investment(&mut self, class: TagClass, seconds: i64) {
        self.task_seconds = self.task_seconds.saturating_add(seconds);
        let total = match class {
            TagClass::Positive => &mut self.positive_seconds,
            TagClass::Negative => &mut self.negative_seconds,
            TagClass::Neutral => &mut self.neutral_seconds,
            TagClass::Leisure => &mut self.leisure_seconds,
            TagClass::Legacy => &mut self.legacy_seconds,
        };
        *total = total.saturating_add(seconds);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PeriodAnalysis {
    /// One bucket for each adjacent pair of caller-supplied boundaries.
    pub days: Vec<DailyAnalysis>,
    pub total: DailyAnalysis,
}

impl Store {
    /// Analyze adjacent half-open time buckets using one bounded interval query.
    /// Boundaries must increase strictly, but may be irregular (e.g. local DST
    /// days). Empty input produces no rows. Intervals are clipped to `now`;
    /// pauses, plans and moment notes never add time. Like `stats`, settle expired
    /// countdowns before querying. Does not write or apply the history limit.
    pub fn analyze_period(&self, boundaries: &[i64], now: i64) -> Result<PeriodAnalysis> {
        if boundaries.len() < 2 {
            return Ok(PeriodAnalysis::default());
        }
        if boundaries.windows(2).any(|pair| pair[1] <= pair[0]) {
            return Err(Error::Validation("Analysis boundaries must increase"));
        }
        let mut days = vec![DailyAnalysis::default(); boundaries.len() - 1];
        let start = boundaries[0];
        let end = boundaries[boundaries.len() - 1].min(now);
        if end <= start {
            return Ok(PeriodAnalysis {
                days,
                total: DailyAnalysis::default(),
            });
        }
        let mut statement = self.connection.prepare(
            "SELECT i.started_at, MIN(COALESCE(i.ended_at, ?2), ?2), s.tag_kind, s.tag_class
             FROM intervals i JOIN sessions s ON s.id = i.session_id
             WHERE i.started_at < ?2 AND (i.ended_at > ?1 OR i.ended_at IS NULL)
             ORDER BY i.started_at, i.id",
        )?;
        let intervals = statement.query_map(params![start, end], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        // Query order means each bucket can merge focus intervals incrementally.
        let mut focus_ends = vec![None::<i64>; days.len()];
        for interval in intervals {
            let (left, right, kind, class) = interval?;
            let left = left.max(start);
            if right <= left {
                continue;
            }
            let kind = TagKind::parse(&kind)?;
            let class = TagClass::parse(&class)?;
            let counts_focus = kind == TagKind::Focus
                && matches!(
                    class,
                    TagClass::Positive | TagClass::Negative | TagClass::Legacy
                );
            let first = boundaries.partition_point(|boundary| *boundary <= left) - 1;
            for index in first..days.len() {
                if boundaries[index] >= right {
                    break;
                }
                let a = left.max(boundaries[index]);
                let b = right.min(boundaries[index + 1]);
                if b <= a {
                    continue;
                }
                days[index].add_investment(class, b.saturating_sub(a));
                if counts_focus {
                    let uncovered = a.max(focus_ends[index].unwrap_or(a));
                    days[index].focus_seconds = days[index]
                        .focus_seconds
                        .saturating_add(b.saturating_sub(uncovered).max(0));
                    focus_ends[index] = Some(focus_ends[index].unwrap_or(b).max(b));
                }
            }
        }
        let mut total = DailyAnalysis::default();
        for day in &days {
            total.add_bucket(*day);
        }
        Ok(PeriodAnalysis { days, total })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(directory.path().join("data/force.sqlite3")).unwrap();
        (directory, store)
    }

    #[test]
    fn analysis_matches_focus_statistics_and_keeps_category_investment_additive() {
        let (_directory, mut store) = database();
        let positive = store
            .create_tag_with_details("Positive", TagKind::Focus, TagClass::Positive, None)
            .unwrap();
        let negative = store
            .create_tag_with_details("Negative", TagKind::Focus, TagClass::Negative, None)
            .unwrap();
        let neutral = store
            .create_tag_with_details("Neutral", TagKind::Focus, TagClass::Neutral, None)
            .unwrap();
        let leisure = store
            .create_tag_with_details("Leisure", TagKind::Rest, TagClass::Leisure, None)
            .unwrap();
        let a = store.start("A", positive.id, 80).unwrap();
        let b = store.start("B", negative.id, 110).unwrap();
        let c = store.start("C", neutral.id, 120).unwrap();
        let d = store.start("D", leisure.id, 150).unwrap();
        store.pause(a.id, 130).unwrap();
        store.finish(b.id, 180).unwrap();
        store.finish(c.id, 160).unwrap();
        store.finish(d.id, 190).unwrap();
        store.resume(a.id, 210).unwrap();
        store.finish(a.id, 260).unwrap();
        store
            .create_moment("A point is not an interval", 230, 300)
            .unwrap();

        let analysis = store.analyze_period(&[100, 200, 300, 400], 300).unwrap();
        assert_eq!(analysis.days.len(), 3);
        assert_eq!(analysis.days[0].focus_seconds, 80);
        assert_eq!(analysis.days[0].positive_seconds, 30);
        assert_eq!(analysis.days[0].negative_seconds, 70);
        assert_eq!(analysis.days[0].neutral_seconds, 40);
        assert_eq!(analysis.days[0].leisure_seconds, 40);
        assert_eq!(analysis.days[1].focus_seconds, 50);
        assert_eq!(analysis.days[2], DailyAnalysis::default());
        for (index, (start, end)) in [(100, 200), (200, 300), (300, 400)].into_iter().enumerate() {
            let stats = store.stats(start, end, 300).unwrap();
            assert_eq!(analysis.days[index].focus_seconds, stats.focus_seconds);
            assert_eq!(analysis.days[index].task_seconds, stats.task_seconds);
        }
        let stats = store.stats(100, 400, 300).unwrap();
        assert_eq!(analysis.total.focus_seconds, stats.focus_seconds);
        assert_eq!(analysis.total.task_seconds, stats.task_seconds);
        assert_eq!(analysis.total.task_seconds, 230);
    }

    #[test]
    fn analysis_clips_running_and_finished_intervals_to_irregular_days_and_snapshot() {
        let (_directory, mut store) = database();
        let running = store.start("Across a clock change", 1, 90).unwrap();
        let finished = store.start("Already finished", 2, 170).unwrap();
        store.finish(finished.id, 260).unwrap();
        let analysis = store.analyze_period(&[0, 100, 190, 300], 220).unwrap();
        assert_eq!(
            analysis
                .days
                .iter()
                .map(|day| day.focus_seconds)
                .collect::<Vec<_>>(),
            vec![10, 90, 30]
        );
        assert_eq!(analysis.total.focus_seconds, 130);
        assert_eq!(analysis.total.task_seconds, 180);
        assert_eq!(store.active_sessions().unwrap(), vec![running]);
        assert_eq!(
            store.analyze_period(&[300, 400], 220).unwrap().days,
            vec![DailyAnalysis::default()]
        );
        assert_eq!(
            store.analyze_period(&[], 220).unwrap(),
            PeriodAnalysis::default()
        );
        assert!(store.analyze_period(&[0, 100, 100], 220).is_err());
    }

    #[test]
    fn analysis_uses_all_intervals_without_the_recent_history_limit() {
        let (_directory, mut store) = database();
        for index in 0..(crate::HISTORY_LIMIT as i64 + 3) {
            let at = 100 + index * 2;
            let session = store.start("A small interval", 1, at).unwrap();
            store.finish(session.id, at + 1).unwrap();
        }
        let analysis = store.analyze_period(&[0, 100, 2000], 2000).unwrap();
        assert_eq!(analysis.days[0], DailyAnalysis::default());
        assert_eq!(
            analysis.total.focus_seconds,
            crate::HISTORY_LIMIT as i64 + 3
        );
        assert_eq!(analysis.total.positive_seconds, analysis.total.task_seconds);
    }
}
