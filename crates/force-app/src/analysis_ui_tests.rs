//! Layout and dispatched events on TestPlatform; native rendering is checked separately.

use chrono::{Days, Local};
use force_core::{TagClass, TagKind};
use gpui::{Bounds, Pixels, TestAppContext, px};

use crate::{
    analysis_model::AnalysisPeriod,
    headless_ui_tests::ManagerHarness,
    model::{Page, day_bounds},
};

fn settle(manager: &mut ManagerHarness) {
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
}

fn fits_width(child: Bounds<Pixels>, parent: Bounds<Pixels>) {
    assert!(
        child.left() >= parent.left() - px(1.),
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.right() <= parent.right() + px(1.),
        "{child:?} outside {parent:?}"
    );
}

#[gpui::test]
fn analysis_ui_periods_and_dense_chart_fit_default_and_minimum_width(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Analysis);
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            let now = Local::now().timestamp();
            view.store
                .as_mut()
                .unwrap()
                .create_moment("A note adds no duration", now, now)
                .unwrap();
            view.refresh(cx);
            view.refresh_analysis(cx);
        });
        settle(&mut manager);
        for (selector, period) in [
            ("analysis-period-7", AnalysisPeriod::Week),
            ("analysis-period-90", AnalysisPeriod::Quarter),
            ("analysis-period-30", AnalysisPeriod::Month),
        ] {
            manager.click(selector);
            let content = manager.bounds("analysis-content");
            for selector in [
                "analysis-toolbar",
                "analysis-summary",
                "analysis-chart-plot",
                "analysis-table-header",
            ] {
                let bounds = manager.bounds(selector);
                fits_width(bounds, content);
            }
            manager.view.read_with(&manager.cx, |view, _| {
                assert_eq!(view.analysis.period, period);
                assert_eq!(view.analysis.days.len(), period.days() as usize);
                assert_eq!(view.analysis.total.focus_seconds, 0);
                assert_eq!(view.analysis.total.task_seconds, 0);
                assert!(view.active.is_empty());
                assert!(view.tasks.is_empty());
                assert!(!view.analysis.error);
            });
        }
        manager.click("analysis-help");
        let help = manager.bounds("analysis-help-text");
        let content = manager.bounds("analysis-content");
        fits_width(help, content);
    }
}

#[gpui::test]
fn analysis_ui_uses_real_overlap_and_updates_today_without_rewriting_history(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Analysis);
    let today = Local::now().date_naive();
    let yesterday = today.checked_sub_days(Days::new(1)).unwrap();
    let (yesterday_start, midnight) = day_bounds(yesterday);
    let before = manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let store = view.store.as_mut().unwrap();
        let positive = store
            .create_tag_with_details(
                "Analysis positive",
                TagKind::Focus,
                TagClass::Positive,
                None,
            )
            .unwrap();
        let neutral = store
            .create_tag_with_details("Analysis neutral", TagKind::Focus, TagClass::Neutral, None)
            .unwrap();
        let first = store
            .start("First", positive.id, yesterday_start + 100)
            .unwrap();
        store.finish(first.id, yesterday_start + 700).unwrap();
        let second = store
            .start("Overlapping", positive.id, yesterday_start + 400)
            .unwrap();
        store.finish(second.id, yesterday_start + 1000).unwrap();
        let other = store
            .start("Neutral", neutral.id, yesterday_start + 500)
            .unwrap();
        store.finish(other.id, yesterday_start + 800).unwrap();
        let csv = store.csv_export(Local::now().timestamp()).unwrap();
        view.refresh(cx);
        view.refresh_analysis(cx);
        csv
    });
    settle(&mut manager);
    manager.click("analysis-period-7");
    manager.view.read_with(&manager.cx, |view, _| {
        let day = view
            .analysis
            .days
            .iter()
            .find(|day| day.date == yesterday)
            .unwrap();
        assert_eq!(day.values.focus_seconds, 900);
        assert_eq!(day.values.positive_seconds, 1200);
        assert_eq!(day.values.neutral_seconds, 300);
        assert_eq!(day.values.task_seconds, 1500);
        assert_eq!(view.analysis.total.focus_seconds, 900);
        assert_eq!(
            view.store.as_ref().unwrap().csv_export(view.now).unwrap(),
            before
        );
    });
    manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let store = view.store.as_mut().unwrap();
        store.start("Live today", 1, midnight).unwrap();
        view.now = midnight + 60;
        view.update_analysis_today(cx);
    });
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.analysis.days.last().unwrap().values.focus_seconds, 60);
        assert_eq!(view.analysis.total.focus_seconds, 960);
        assert_eq!(
            view.analysis
                .days
                .iter()
                .find(|day| day.date == yesterday)
                .unwrap()
                .values
                .focus_seconds,
            900
        );
    });
}
