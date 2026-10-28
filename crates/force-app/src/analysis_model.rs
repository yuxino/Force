//! Analysis dates are local calendar days; all durations come from core intervals.

use chrono::{Days, Local, NaiveDate, TimeZone};
use force_core::{DailyAnalysis, Store};
use gpui::{Context, Window};

use crate::model::{ForceView, Page, day_bounds};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum AnalysisPeriod {
    Week,
    #[default]
    Month,
    Quarter,
}

impl AnalysisPeriod {
    pub(crate) fn days(self) -> u64 {
        match self {
            Self::Week => 7,
            Self::Month => 30,
            Self::Quarter => 90,
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Week => "analysis-period-7",
            Self::Month => "analysis-period-30",
            Self::Quarter => "analysis-period-90",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AnalysisDay {
    pub(crate) date: NaiveDate,
    pub(crate) values: DailyAnalysis,
}

#[derive(Default)]
pub(crate) struct AnalysisState {
    pub(crate) period: AnalysisPeriod,
    pub(crate) days: Vec<AnalysisDay>,
    pub(crate) total: DailyAnalysis,
    pub(crate) loaded_day: Option<NaiveDate>,
    pub(crate) help_open: bool,
    pub(crate) error: bool,
}

pub(crate) struct AnalysisSnapshot {
    days: Vec<AnalysisDay>,
    total: DailyAnalysis,
    today: NaiveDate,
}

impl AnalysisState {
    fn apply(&mut self, snapshot: AnalysisSnapshot) {
        self.days = snapshot.days;
        self.total = snapshot.total;
        self.loaded_day = Some(snapshot.today);
        self.error = false;
    }

    pub(crate) fn average_focus_seconds(&self) -> i64 {
        self.total.focus_seconds / (self.days.len() as i64).max(1)
    }
}

pub(crate) fn load_analysis(
    store: &Store,
    period: AnalysisPeriod,
    now: i64,
) -> force_core::Result<AnalysisSnapshot> {
    let today = Local
        .timestamp_opt(now, 0)
        .single()
        .ok_or(force_core::Error::InvalidData)?
        .date_naive();
    let start = today
        .checked_sub_days(Days::new(period.days() - 1))
        .ok_or(force_core::Error::InvalidData)?;
    let dates: Vec<_> = (0..period.days())
        .map(|offset| start.checked_add_days(Days::new(offset)))
        .collect::<Option<Vec<_>>>()
        .ok_or(force_core::Error::InvalidData)?;
    let mut boundaries: Vec<_> = dates.iter().map(|date| day_bounds(*date).0).collect();
    boundaries.push(day_bounds(today).1);
    let values = store.analyze_period(&boundaries, now)?;
    Ok(AnalysisSnapshot {
        days: dates
            .into_iter()
            .zip(values.days)
            .map(|(date, values)| AnalysisDay { date, values })
            .collect(),
        total: values.total,
        today,
    })
}

impl ForceView {
    pub(crate) fn open_analysis_day(
        &mut self,
        date: NaiveDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.analysis.error
            || date > Local::now().date_naive()
            || !self.analysis.days.iter().any(|day| day.date == date)
        {
            return;
        }
        // Keep date changes consistent with the records calendar, including its
        // search/filter state. Normal navigation preserves suspended editors.
        self.select_record_date(date, cx);
        self.navigate(Page::Records, window, cx);
    }

    pub(crate) fn select_analysis_period(
        &mut self,
        period: AnalysisPeriod,
        cx: &mut Context<Self>,
    ) {
        if self.analysis.period != period || self.analysis.error {
            self.analysis.period = period;
            self.refresh_analysis(cx);
        }
    }

    /// Call after the regular refresh has settled expired countdown sessions.
    pub(crate) fn refresh_analysis(&mut self, cx: &mut Context<Self>) {
        if self.page != Page::Analysis {
            return;
        }
        match self
            .store
            .as_ref()
            .map(|store| load_analysis(store, self.analysis.period, self.now))
        {
            Some(Ok(snapshot)) => self.analysis.apply(snapshot),
            _ => self.analysis.error = true,
        }
        cx.notify();
    }

    /// Active timers only need today's bucket refreshed; older days stay cached.
    pub(crate) fn update_analysis_today(&mut self, cx: &mut Context<Self>) {
        if self.page != Page::Analysis || self.compact {
            return;
        }
        let today = Local
            .timestamp_opt(self.now, 0)
            .single()
            .map(|at| at.date_naive());
        if self.analysis.error || self.analysis.loaded_day != today || self.analysis.days.is_empty()
        {
            self.refresh_analysis(cx);
            return;
        }
        let Some(today) = today else { return };
        let (start, end) = day_bounds(today);
        match self
            .store
            .as_ref()
            .map(|store| store.analyze_period(&[start, end], self.now))
        {
            Some(Ok(snapshot)) => {
                if let (Some(day), Some(values)) =
                    (self.analysis.days.last_mut(), snapshot.days.first())
                {
                    day.values = *values;
                }
                let mut total = DailyAnalysis::default();
                for day in &self.analysis.days {
                    total.add_bucket(day.values);
                }
                self.analysis.total = total;
            }
            _ => self.analysis.error = true,
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_calendar_series_has_exact_period_and_matches_record_day_bounds() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("data/force.sqlite3")).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
        let (midnight, _) = day_bounds(today);
        let session = store.start("Across midnight", 1, midnight - 20).unwrap();
        store.finish(session.id, midnight + 40).unwrap();
        store
            .create_moment("Only a point", midnight + 50, midnight + 100)
            .unwrap();
        for period in [
            AnalysisPeriod::Week,
            AnalysisPeriod::Month,
            AnalysisPeriod::Quarter,
        ] {
            let snapshot = load_analysis(&store, period, midnight + 100).unwrap();
            assert_eq!(snapshot.days.len(), period.days() as usize);
            assert_eq!(snapshot.days.last().unwrap().date, today);
            assert_eq!(
                snapshot.days[snapshot.days.len() - 2].values.focus_seconds,
                20
            );
            assert_eq!(snapshot.days.last().unwrap().values.focus_seconds, 40);
            assert_eq!(snapshot.total.focus_seconds, 60);
            for day in snapshot.days {
                let (start, end) = day_bounds(day.date);
                assert_eq!(
                    day.values.focus_seconds,
                    store
                        .stats(start, end, midnight + 100)
                        .unwrap()
                        .focus_seconds
                );
            }
        }
    }

    #[test]
    fn daily_average_includes_zero_days_and_empty_data_stays_zero() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(directory.path().join("data/force.sqlite3")).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 28).unwrap();
        let now = day_bounds(today).0 + 3600;
        let mut state = AnalysisState::default();
        state.apply(load_analysis(&store, AnalysisPeriod::Week, now).unwrap());
        assert_eq!(state.average_focus_seconds(), 0);
        let session = store.start("One day", 1, now - 700).unwrap();
        store.finish(session.id, now).unwrap();
        state.apply(load_analysis(&store, AnalysisPeriod::Week, now).unwrap());
        assert_eq!(state.average_focus_seconds(), 100);
        assert_eq!(state.total.focus_seconds, 700);
    }
}
