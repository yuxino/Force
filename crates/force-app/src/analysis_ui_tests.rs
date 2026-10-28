//! Layout and dispatched events on TestPlatform; native rendering is checked separately.

use chrono::{Days, Local};
use force_core::{TagClass, TagKind};
use gpui::{Bounds, Modifiers, Pixels, TestAppContext, point, px};

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
            let summary = manager.bounds("analysis-summary");
            assert!((summary.left() - content.left()).abs() <= px(1.));
            assert!((summary.right() - content.right()).abs() <= px(1.));
            let expected_column_width = (content.size.width - px(48.)) / 3.;
            for selector in [
                "analysis-total-focus",
                "analysis-average-focus",
                "analysis-total-investment",
            ] {
                let column = manager.bounds(selector);
                fits_width(column, summary);
                assert!((column.size.width - expected_column_width).abs() <= px(1.));
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

#[gpui::test]
fn analysis_ui_day_links_preserve_drafts_filters_and_saved_intervals(cx: &mut TestAppContext) {
    let today = Local::now().date_naive();
    let yesterday = today.checked_sub_days(Days::new(1)).unwrap();
    for (width, height) in [(1040., 780.), (780., 620.)] {
        for from_chart in [true, false] {
            let mut manager = ManagerHarness::new(cx, width, height);
            let before = manager.view.update_in(&mut manager.cx, |view, window, cx| {
                let start = day_bounds(yesterday).0;
                let session = view
                    .store
                    .as_mut()
                    .unwrap()
                    .start("Saved record", 1, start + 60)
                    .unwrap();
                view.store
                    .as_mut()
                    .unwrap()
                    .finish(session.id, start + 660)
                    .unwrap();
                view.task_input
                    .update(cx, |input, cx| input.set_value("Unsaved task", window, cx));
                view.timer_input
                    .update(cx, |input, cx| input.set_value("37", window, cx));
                view.countdown = true;
                view.record_search
                    .update(cx, |input, cx| input.set_value("Saved", window, cx));
                view.record_class_filter = Some(TagClass::Positive);
                view.navigate(Page::Agenda, window, cx);
                view.begin_plan(None, window, cx);
                view.agenda
                    .title_input
                    .update(cx, |input, cx| input.set_value("Unsaved plan", window, cx));
                view.begin_moment(None, window, cx);
                view.moments
                    .content
                    .update(cx, |input, cx| input.set_value("Unsaved note", window, cx));
                view.navigate(Page::Analysis, window, cx);
                assert!(view.agenda_has_suspended_draft(cx));
                view.store.as_ref().unwrap().csv_export(view.now).unwrap()
            });
            settle(&mut manager);
            manager.click("analysis-period-7");
            let selector: &'static str = Box::leak(
                format!(
                    "analysis-{}-{yesterday}",
                    if from_chart { "bar" } else { "day" }
                )
                .into_boxed_str(),
            );
            if !from_chart {
                let viewport = manager.bounds("manager-viewport");
                let row = manager.bounds(selector);
                let delta = viewport.bottom() - px(12.) - row.bottom();
                if delta < px(0.) {
                    manager.scroll_at(
                        point(viewport.left() + px(8.), viewport.center().y),
                        f32::from(delta),
                    );
                }
            }
            manager.click(selector);
            assert!(!manager.cx.has_pending_prompt());
            manager.view.read_with(&manager.cx, |view, cx| {
                assert!(view.page == Page::Records);
                assert_eq!(view.record_date, yesterday);
                assert_eq!(view.record_search.read(cx).value(), "Saved");
                assert_eq!(view.record_class_filter, Some(TagClass::Positive));
                assert_eq!(view.task_input.read(cx).value(), "Unsaved task");
                assert_eq!(view.timer_input.read(cx).value(), "37");
                assert!(view.countdown);
                assert!(view.agenda_has_suspended_draft(cx));
                assert_eq!(view.agenda.title_input.read(cx).value(), "Unsaved plan");
                assert!(view.moments.active && !view.moments.visible);
                assert_eq!(view.moments.content.read(cx).value(), "Unsaved note");
                assert!(view.tasks.is_empty());
                assert!(view.active.is_empty());
                assert_eq!(
                    view.store.as_ref().unwrap().csv_export(view.now).unwrap(),
                    before
                );
            });
        }
    }
}

#[gpui::test]
fn analysis_ui_empty_chart_dates_still_open_records(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Analysis);
    manager.click("analysis-period-7");
    let today = Local::now().date_naive();
    let selector: &'static str = Box::leak(format!("analysis-bar-{today}").into_boxed_str());
    let bar = manager.bounds(selector);
    // Zero-valued bars retain the full column as their hit target.
    manager.cx.simulate_click(
        point(bar.center().x, bar.top() + px(12.)),
        Modifiers::default(),
    );
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.page == Page::Records);
        assert_eq!(view.record_date, today);
        assert!(view.history.is_empty());
        assert!(view.tasks.is_empty());
        assert!(view.active.is_empty());
    });
}

#[gpui::test]
fn analysis_ui_link_rows_keep_long_totals_and_arrow_inside_the_grid(cx: &mut TestAppContext) {
    let today = Local::now().date_naive();
    let row_selector: &'static str = Box::leak(format!("analysis-day-{today}").into_boxed_str());
    let content_selector: &'static str =
        Box::leak(format!("analysis-day-content-{today}").into_boxed_str());
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Analysis);
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            view.refresh_analysis(cx);
            let day = view.analysis.days.last_mut().unwrap();
            // Investment is additive across overlapping records; it may exceed
            // one calendar day even though merged focus never does.
            day.values.focus_seconds = 86_399;
            day.values.positive_seconds = 10_000 * 3_600 + 59 * 60 + 59;
            cx.notify();
        });
        settle(&mut manager);
        let row = manager.bounds(row_selector);
        let inner = manager.bounds(content_selector);
        fits_width(row, manager.bounds("analysis-content"));
        fits_width(inner, row);
        let mut previous_right = inner.left();
        for part in [
            "focus", "class-0", "class-1", "class-2", "class-3", "class-4", "arrow",
        ] {
            let selector: &'static str =
                Box::leak(format!("analysis-day-{part}-{today}").into_boxed_str());
            let cell = manager.bounds(selector);
            let header_selector: &'static str =
                Box::leak(format!("analysis-header-{part}").into_boxed_str());
            let header = manager.bounds(header_selector);
            fits_width(cell, inner);
            assert!((cell.left() - header.left()).abs() <= px(1.));
            assert!((cell.right() - header.right()).abs() <= px(1.));
            assert!(cell.left() >= previous_right - px(1.));
            assert!(cell.size.height <= row.size.height);
            previous_right = cell.right();
        }
        assert_eq!(
            crate::analysis_view::table_duration(10_000 * 3_600 + 59 * 60 + 59),
            "10000:59:59"
        );
        assert!(inner.size.height <= row.size.height);
    }
}
