use chrono::{Local, NaiveDate, TimeDelta};
use force_core::{LifeModule, LifeOccurrence};
use gpui::{TestAppContext, px};

use crate::{headless_ui_tests::ManagerHarness, life_model::CheckInMetric, model::Page};

#[gpui::test]
fn habit_analysis_tracks_occurrences_and_preserves_drafts(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Analysis);
    manager.click("analysis-scope-habits");
    manager.click("life-add");
    manager.click("life-save");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.page, Page::Analysis);
        assert!(view.life.editor_visible);
        assert!(view.life.error.is_some());
    });
    let first = Local::now() - TimeDelta::hours(49);
    manager.cx.simulate_input("喝咖啡");
    manager.click("life-kind-abstain");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.life.date.update(cx, |input, cx| {
            input.set_value(first.format("%Y-%m-%d").to_string(), window, cx)
        });
        view.life.time.update(cx, |input, cx| {
            input.set_value(first.format("%H:%M:%S").to_string(), window, cx)
        });
    });
    manager.click("life-close");
    manager.click("records-tab");
    manager.click("analysis-tab");
    manager.click("life-add");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.life.name.read(cx).value(), "喝咖啡");
    });
    manager.cx.simulate_keystrokes("secondary-enter");
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();

    let trend = manager.bounds("life-trend");
    let total = manager.bounds("life-total");
    assert!(trend.top() > total.bottom());
    manager.click("life-occurrence");
    manager.click("life-save");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.life.editor_visible);
        let item = &view.life.board.as_ref().unwrap().items[0];
        assert_eq!(item.title, "喝咖啡");
        assert_eq!(item.cumulative_days(view.now), 2);
        assert_eq!(item.recent_spans(view.now, 8).len(), 2);
        assert!(item.elapsed_at(view.now) < 60);
        assert!(view.active.is_empty());
    });

    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        view.set_life_module(LifeModule::Trend, false, cx);
        view.set_life_module(LifeModule::Table, false, cx);
        view.open_store(cx);
    });
    manager.view.read_with(&manager.cx, |view, _| {
        let board = view.life.board.as_ref().unwrap();
        assert!(!board.shows(LifeModule::Trend));
        assert!(!board.shows(LifeModule::Table));
        assert!(board.shows(LifeModule::Total));
        assert_eq!(board.items[0].recent_spans(view.now, 8).len(), 2);
    });
}

#[gpui::test]
fn date_only_entries_support_duration_without_creating_a_timer(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Analysis);
    manager.click("analysis-scope-habits");
    let day = Local::now().date_naive().pred_opt().unwrap();

    manager.click("life-add");
    manager.cx.simulate_input("阅读");
    manager.click("life-kind-abstain");
    manager.click("life-day-only");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.life
            .date
            .update(cx, |input, cx| input.set_value(day.to_string(), window, cx));
    });
    manager.click("life-save");
    manager.view.read_with(&manager.cx, |view, _| {
        let item = &view.life.board.as_ref().unwrap().items[0];
        assert!(
            matches!(item.last_occurrence(), Some(LifeOccurrence::Day { date, .. }) if date == day)
        );
    });

    manager.click("records-tab");
    manager.click("global-new-task");
    manager.click("creation-type-moment");
    manager.cx.simulate_input("散步");
    manager.click("moment-day-only");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.moments
            .date
            .update(cx, |input, cx| input.set_value(day.to_string(), window, cx));
        view.moments
            .minutes
            .update(cx, |input, cx| input.set_value("35", window, cx));
    });
    manager.click("save-moment");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.record_date, day);
        assert_eq!(view.record_daily_entries[0].content, "散步");
        assert_eq!(view.record_daily_entries[0].duration_minutes, Some(35));
        assert!(view.record_moments.is_empty());
        assert!(view.history.is_empty());
        assert_eq!(view.record_stats.focus_seconds, 0);
    });
}

#[gpui::test]
fn calendar_records_check_ins_on_the_activated_date(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let day = NaiveDate::from_ymd_opt(2000, 1, 31).unwrap();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.select_agenda_date(day, window, cx);
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let grid = manager.bounds("agenda-overview-grid");
    assert!(grid.bottom() <= px(620.));
    manager.click("agenda-month-day-2000-01-31");
    manager.click("agenda-check-in");
    manager.click("agenda-check-in-add");
    manager.cx.simulate_input("拉伸");
    manager.click("life-save");
    manager.click("global-new-task");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.life.editor_visible);
        assert!(!view.task_editor.visible);
        assert_eq!(view.life.date.read(cx).value(), day.to_string());
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.life
            .minutes
            .update(cx, |input, cx| input.set_value("35", window, cx));
    });
    manager.click("life-save");
    assert_eq!(manager.bounds("agenda-overview-grid"), grid);
    let cell = manager.bounds("agenda-month-day-2000-01-31");
    assert!(
        cell.contains(
            &manager
                .bounds("agenda-month-check-in-2000-01-31-1")
                .center()
        )
    );
    assert!(manager.cx.debug_bounds("life-trend").is_none());
    manager.click("agenda-month-day-2000-01-31");
    let popup = manager.bounds("agenda-popover");
    assert!(popup.contains(&manager.bounds("agenda-day-checkin-1").center()));
    manager.click("agenda-check-in");
    manager.click("agenda-check-in-item-1");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.life.date.read(cx).value(), day.to_string());
    });
    manager.click("life-cancel");
    manager.click("agenda-month-day-2000-01-31");
    manager.click("agenda-check-in-records");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.page, Page::Records);
        assert_eq!(view.record_date, day);
        assert_eq!(view.record_daily_entries[0].duration_minutes, Some(35));
        assert!(view.active.is_empty());
        assert_eq!(view.record_stats.focus_seconds, 0);
    });
}

#[gpui::test]
fn habit_analysis_can_choose_subject_metric_and_displayed_modules(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Analysis);
    let today = Local::now().date_naive();
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        assert!(view.write(
            |store| {
                store.create_daily_entry(today, "拉伸", Some(35), today)?;
                store.create_life_habit("拉伸")?;
                store.create_life_habit("散步").map(|_| ())
            },
            cx
        ));
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.bounds("analysis-summary");
    manager.click("analysis-scope-habits");
    manager.bounds("life-analysis-item-1");
    manager.bounds("life-analysis-log");
    let trend_top = manager.bounds("life-trend").top();
    manager.click("life-metric-duration");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.life.metric, CheckInMetric::Duration)
    });
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        view.select_life_chart_item(2, cx);
        view.set_life_module(LifeModule::Total, false, cx);
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.bounds("life-analysis-item-2");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.life.board.as_ref().unwrap().shows(LifeModule::Total));
    });
    assert!(manager.bounds("life-trend").top() < trend_top);
    manager.click("analysis-scope-timing");
    manager.bounds("analysis-summary");
}
