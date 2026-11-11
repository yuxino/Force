use chrono::{Local, TimeDelta};
use force_core::{LifeModule, LifeOccurrence};
use gpui::{TestAppContext, px};

use crate::{headless_ui_tests::ManagerHarness, model::Page};

#[gpui::test]
fn life_dashboard_tracks_occurrences_and_restores_its_layout(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    manager.click("open-life-dashboard");
    manager.click("life-add");
    manager.click("life-save");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.page, Page::Life);
        assert!(view.life.editor_visible);
        assert!(view.life.error.is_some());
    });
    let first = Local::now() - TimeDelta::hours(49);
    manager.cx.simulate_input("喝咖啡");
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
    manager.click("agenda-tab");
    manager.click("open-life-dashboard");
    manager.click("life-add");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.life.name.read(cx).value(), "喝咖啡");
    });
    manager.cx.simulate_keystrokes("secondary-enter");
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();

    let table = manager.bounds("life-table");
    let trend = manager.bounds("life-trend");
    let total = manager.bounds("life-total");
    assert!(trend.top() > table.bottom());
    assert!(total.left() >= trend.right() + px(30.));
    assert!(total.right() <= table.right() + px(1.));

    manager.click("life-record-1");
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

    manager.click("life-customize");
    manager.click("life-toggle-trend");
    manager.click("life-toggle-table");
    manager
        .view
        .update_in(&mut manager.cx, |view, _, cx| view.open_store(cx));
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
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let day = Local::now().date_naive().pred_opt().unwrap();

    manager.click("open-life-dashboard");
    manager.click("life-add");
    manager.cx.simulate_input("阅读");
    manager.click("life-day-only");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.life
            .date
            .update(cx, |input, cx| input.set_value(day.to_string(), window, cx));
    });
    manager.click("life-save");
    manager.view.read_with(&manager.cx, |view, _| {
        let item = &view.life.board.as_ref().unwrap().items[0];
        assert!(matches!(item.last_occurrence(), LifeOccurrence::Day { date, .. } if date == day));
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
