//! Structural and event tests on GPUI's TestPlatform only.
//!
//! This backend never opens an OS window. It runs the actual Force view tree,
//! layout and event dispatch with simulated font metrics; it cannot validate
//! native text shaping, raster output, IME or platform-window behavior.

use crate::model::{FloatingStrip, ForceView, Page, day_bounds};
use chrono::Local;
use gpui::{
    AppContext as _, Bounds, Entity, KeyUpEvent, Keystroke, Modifiers, Pixels, Point, ScrollDelta,
    ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext, WindowBounds, WindowHandle,
    WindowOptions, point, px, size,
};
use gpui_component::{Root, Theme, ThemeMode};
use std::path::PathBuf;

struct ManagerHarness {
    _directory: tempfile::TempDir,
    view: Entity<ForceView>,
    handle: WindowHandle<Root>,
    cx: VisualTestContext,
}

impl ManagerHarness {
    fn new(cx: &mut TestAppContext, width: f32, height: f32) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let data_dir: PathBuf = directory.path().to_path_buf();
        let (handle, view) = cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            let theme = Theme::global_mut(cx);
            theme.font_size = px(13.);
            theme.shadow = false;
            let mut view = None;
            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            Point::default(),
                            size(px(width), px(height)),
                        ))),
                        ..Default::default()
                    },
                    |window, cx| {
                        let force = cx.new(|cx| ForceView::new(data_dir, window, cx));
                        view = Some(force.clone());
                        cx.new(|cx| Root::new(force, window, cx))
                    },
                )
                .unwrap();
            (handle, view.unwrap())
        });
        let mut visual = VisualTestContext::from_window(*handle, cx);
        visual.simulate_resize(size(px(width), px(height)));
        view.update_in(&mut visual, |view, window, cx| {
            view.show_manager(window, cx)
        });
        visual.update(|window, _| window.refresh());
        visual.run_until_parked();
        Self {
            _directory: directory,
            view,
            handle,
            cx: visual,
        }
    }

    fn bounds(&mut self, selector: &'static str) -> Bounds<Pixels> {
        self.cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing rendered selector: {selector}"))
    }

    fn click(&mut self, selector: &'static str) {
        let bounds = self.bounds(selector);
        assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
        self.cx
            .simulate_click(bounds.center(), Modifiers::default());
        self.cx.update(|window, _| window.refresh());
        self.cx.run_until_parked();
    }

    fn scroll(&mut self, selector: &'static str, amount: f32) {
        let position = self.bounds(selector).center();
        self.cx
            .simulate_mouse_move(position, None, Modifiers::default());
        self.cx.simulate_event(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(amount))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
        });
        self.cx.update(|window, _| window.refresh());
        self.cx.run_until_parked();
    }

    fn page(&self) -> Page {
        self.view.read_with(&self.cx, |view, _| view.page)
    }
}

impl Drop for ManagerHarness {
    fn drop(&mut self) {
        self.cx.update(|window, _| window.remove_window());
        self.cx.run_until_parked();
    }
}

fn assert_horizontally_inside(child: Bounds<Pixels>, parent: Bounds<Pixels>) {
    let tolerance = px(1.);
    assert!(
        child.left() >= parent.left() - tolerance,
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.right() <= parent.right() + tolerance,
        "{child:?} outside {parent:?}"
    );
}

fn assert_same_bounds(actual: Bounds<Pixels>, expected: Bounds<Pixels>) {
    for (actual, expected) in [
        (actual.left(), expected.left()),
        (actual.top(), expected.top()),
        (actual.size.width, expected.size.width),
        (actual.size.height, expected.size.height),
    ] {
        assert!((actual - expected).abs() <= px(1.));
    }
}

#[gpui::test]
fn headless_ui_manager_layout_is_bounded_at_default_and_minimum_size(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new(cx, width, height);
        let viewport = Bounds::new(Point::default(), size(px(width), px(height)));
        let header = manager.bounds("manager-header");
        let content = manager.bounds("manager-content");
        let summary = manager.bounds("focus-summary");
        let operation = manager.bounds("focus-operation");
        let tasks = manager.bounds("focus-tasks");
        let form = manager.bounds("focus-form");
        let start = manager.bounds("start-focus");
        assert_horizontally_inside(header, viewport);
        assert_horizontally_inside(content, viewport);
        assert_horizontally_inside(summary, content);
        assert_horizontally_inside(operation, content);
        assert_horizontally_inside(tasks, content);
        assert_horizontally_inside(form, operation);
        assert_eq!(form.left(), operation.left());
        assert!((form.right() - operation.right()).abs() <= px(4.));
        let goal = manager.bounds("focus-goal");
        let task_heading = manager.bounds("task-list-heading");
        assert_eq!(goal.left(), tasks.left());
        assert_eq!(task_heading.left(), tasks.left());
        assert!(summary.top() >= header.bottom() - px(1.));
        assert!(summary.bottom() <= viewport.bottom() + px(1.));
        assert!(operation.top() >= summary.bottom() - px(1.));
        assert!(tasks.top() >= summary.bottom() - px(1.));
        assert!(operation.right() <= tasks.left() + px(1.));
        assert!(start.top() >= operation.top() - px(1.));
        assert!(
            start.bottom() <= operation.bottom() + px(1.),
            "start action must be visible at {width}×{height}"
        );

        manager.click("settings-tab");
        assert!(manager.page() == Page::Settings);
        let content = manager.bounds("manager-content");
        for selector in [
            "settings-timing",
            "settings-tags",
            "settings-form",
            "goal-input",
            "save-goal",
            "reset-goal",
            "tag-input",
            "save-tag",
        ] {
            assert_horizontally_inside(manager.bounds(selector), content);
        }
    }
}

#[gpui::test]
fn headless_ui_navigation_and_goal_save_use_real_gpui_events(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, _| {
        window.focus(&view.focus_handle);
    });
    manager.cx.simulate_keystrokes("tab tab enter");
    // GPUI's keystroke helper dispatches KeyDown only; buttons activate on KeyUp.
    manager.cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    assert!(manager.page() == Page::Records);
    manager.click("settings-tab");
    assert!(manager.page() == Page::Settings);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.goal_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
    });
    manager.cx.simulate_input("75");
    assert!(
        manager
            .view
            .read_with(&manager.cx, |view, cx| view.can_save_goal(cx))
    );
    manager.click("save-goal");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.goal, 75);
        assert!(!view.can_save_goal(cx));
        assert_eq!(view.store.as_ref().unwrap().goal_minutes().unwrap(), 75);
    });
    manager.click("records-tab");
    assert!(manager.page() == Page::Records);
    manager.click("focus-tab");
    assert!(manager.page() == Page::Focus);
}

#[gpui::test]
fn headless_ui_disabled_start_and_timer_transitions_keep_the_manager_stable(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let summary = manager.bounds("focus-summary");
    let tasks = manager.bounds("focus-tasks");
    let form = manager.bounds("focus-form");
    manager.click("start-focus");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.active.is_empty());
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .active_sessions()
                .unwrap()
                .is_empty()
        );
        assert!(
            view.store
                .as_ref()
                .unwrap()
                .tasks(view.now)
                .unwrap()
                .is_empty()
        );
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input
            .update(cx, |input, cx| input.focus(window, cx));
    });
    manager.cx.simulate_input("A headless test task");
    assert_same_bounds(manager.bounds("focus-summary"), summary);
    assert_eq!(
        manager
            .view
            .read_with(&manager.cx, |view, _| view.panel_scroll[0].offset().y),
        px(0.)
    );
    manager.click("start-focus");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active.len(), 1);
        assert!(view.active[0].is_running());
        assert_eq!(view.active[0].title, "A headless test task");
        assert!(!view.compact, "starting should keep the manager visible");
    });
    assert_same_bounds(manager.bounds("focus-summary"), summary);
    assert_same_bounds(manager.bounds("focus-tasks"), tasks);
    assert_eq!(manager.bounds("focus-running").left(), form.left());
    assert_eq!(manager.bounds("focus-running").size.width, form.size.width);
    let running_id = manager
        .view
        .read_with(&manager.cx, |view, _| view.active[0].id);
    manager.view.update(&mut manager.cx, |view, cx| {
        let other = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Another saved task", view.selected_tag, view.now)
            .unwrap();
        assert_eq!(other.id, 2);
        view.refresh(cx);
    });
    manager.click("task-row-2");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.selected_task, Some(2));
        assert!(view.preparing);
        let active = view.store.as_ref().unwrap().active_sessions().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, running_id);
        assert!(
            active[0].is_running(),
            "selecting another task must not alter the running timer"
        );
    });
    manager.click("task-row-1");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.selected_task, Some(1));
        assert_eq!(view.selected_session, Some(running_id));
        assert!(view.active[0].is_running());
    });
    assert_same_bounds(manager.bounds("focus-summary"), summary);
    assert_same_bounds(manager.bounds("focus-tasks"), tasks);
    manager.click("running-pause");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.active[0].is_paused());
        let active = view.store.as_ref().unwrap().active_sessions().unwrap();
        assert_eq!(active.len(), 1);
        assert!(active[0].is_paused());
    });
    manager.click("running-finish");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.active.is_empty());
        assert!(view.completed.is_some());
        assert!(!view.compact);
    });
    assert_eq!(manager.bounds("focus-completed").left(), form.left());
    assert_eq!(
        manager.bounds("focus-completed").size.width,
        form.size.width
    );
    assert_same_bounds(manager.bounds("focus-summary"), summary);
    assert_same_bounds(manager.bounds("focus-tasks"), tasks);
}

#[gpui::test]
fn headless_ui_task_titles_use_the_row_content_width(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new(cx, width, height);
        manager.view.update(&mut manager.cx, |view, cx| {
            let store = view.store.as_mut().unwrap();
            for (id, title) in [
                (1, "英语"),
                (2, "英语阅读与写作练习这是需要省略的较长中文任务标题"),
            ] {
                let task = store
                    .create_task(title, view.selected_tag, view.now)
                    .unwrap();
                assert_eq!(task.id, id);
            }
            view.refresh(cx);
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        for (row, column, title, status) in [
            (
                "task-row-1",
                "task-column-1",
                "task-title-1",
                "task-status-1",
            ),
            (
                "task-row-2",
                "task-column-2",
                "task-title-2",
                "task-status-2",
            ),
        ] {
            let row = manager.bounds(row);
            let column = manager.bounds(column);
            let title = manager.bounds(title);
            let status = manager.bounds(status);
            assert_horizontally_inside(column, row);
            assert_horizontally_inside(title, column);
            assert_horizontally_inside(status, column);
            assert!(
                title.size.width >= row.size.width - px(28.),
                "task title must use the row content width at {width}×{height}: row={row:?}, column={column:?}, title={title:?}, status={status:?}"
            );
        }
    }
}

#[gpui::test]
fn headless_ui_independent_scrolls_leave_the_summary_visible(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 780., 620.);
    manager.view.update(&mut manager.cx, |view, cx| {
        let store = view.store.as_mut().unwrap();
        for index in 1..=20 {
            store
                .create_task(&format!("Saved task {index}"), view.selected_tag, view.now)
                .unwrap();
        }
        view.refresh(cx);
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input
            .update(cx, |input, cx| input.focus(window, cx));
    });
    manager
        .cx
        .simulate_input("A long running task title that needs two lines in the operation pane");
    manager.click("start-focus");
    manager.view.update(&mut manager.cx, |view, cx| {
        // A real feedback state reduces the operation viewport without moving
        // the summary. It also makes the operation genuinely scrollable.
        view.notify("无法保存浮窗位置。计时记录不受影响。", cx);
    });
    let summary = manager.bounds("focus-summary");
    let tasks = manager.bounds("focus-tasks");
    let header = manager.bounds("manager-header");
    let running_before = manager.bounds("focus-running");
    manager.scroll("focus-operation", -240.);
    assert!(
        manager
            .view
            .read_with(&manager.cx, |view, _| view.panel_scroll[0].offset().y)
            < px(0.),
        "the operation fixture must actually scroll"
    );
    assert!(manager.bounds("focus-running").top() < running_before.top());
    assert_same_bounds(manager.bounds("focus-summary"), summary);
    assert_same_bounds(manager.bounds("focus-tasks"), tasks);
    assert!(manager.bounds("focus-summary").top() >= header.bottom() - px(1.));

    let task_before = manager.bounds("task-row-20");
    manager.scroll("task-list-scroll", -50.);
    assert!(
        manager.bounds("task-row-20").top() < task_before.top(),
        "the task list fixture must actually scroll"
    );
    assert_same_bounds(manager.bounds("focus-summary"), summary);
    assert_same_bounds(manager.bounds("focus-tasks"), tasks);
    manager.click("new-task");
    assert_eq!(
        manager
            .view
            .read_with(&manager.cx, |view, _| view.panel_scroll[0].offset().y),
        px(0.)
    );
    assert_same_bounds(manager.bounds("focus-summary"), summary);
}

#[gpui::test]
fn headless_ui_activity_days_fit_and_select_records_without_resetting_scroll(
    cx: &mut TestAppContext,
) {
    let actual_today = Local::now().date_naive();
    let recent = actual_today.pred_opt().unwrap();
    let earlier = recent.pred_opt().unwrap();
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new(cx, width, height);
        manager.view.update(&mut manager.cx, |view, cx| {
            view.record_date = recent;
            let store = view.store.as_mut().unwrap();
            for (date, duration, prefix) in [(recent, 60, "Recent"), (earlier, 120, "Past")] {
                for index in 0..20 {
                    let start = day_bounds(date).0 + 3_600 + index * 180;
                    let session = store
                        .start(
                            &format!("{prefix} record {index}"),
                            view.selected_tag,
                            start,
                        )
                        .unwrap();
                    store.finish(session.id, start + duration).unwrap();
                }
            }
            view.refresh(cx);
        });
        manager.click("records-tab");
        let grid = manager.bounds("activity-grid");
        assert_horizontally_inside(grid, manager.bounds("manager-content"));
        let dates = manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.activity_days.len(), 53 * 7);
            view.activity_days
                .iter()
                .map(|day| day.date)
                .collect::<Vec<_>>()
        });
        // GPUI's debug-bounds API requires static selector strings. These
        // bounded test-only allocations live until the test process exits.
        let selectors: Vec<_> = dates
            .iter()
            .map(|date| &*format!("activity-day-{date}").leak())
            .collect();
        for selector in &selectors {
            let cell = manager.bounds(selector);
            assert_horizontally_inside(cell, grid);
            assert!(cell.top() >= grid.top() - px(1.));
            assert!(cell.bottom() <= grid.bottom() + px(1.));
            assert!(cell.size.width > px(0.) && cell.size.width <= px(12.));
            assert_eq!(cell.size.height, px(12.));
        }
        let previous = manager.bounds("previous-day");
        let label = manager.bounds("record-date-label");
        let next = manager.bounds("next-day");
        assert!(previous.right() <= label.left());
        assert!(label.left() - previous.right() <= px(8.));
        assert!(label.right() <= next.left());
        assert!(next.left() - label.right() <= px(8.));
        let today_index = dates.iter().position(|date| *date == actual_today).unwrap();
        let recent_index = dates.iter().position(|date| *date == recent).unwrap();
        let earlier_index = dates.iter().position(|date| *date == earlier).unwrap();
        manager.click(selectors[today_index]);
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.record_date, actual_today);
            assert!(view.history.is_empty());
            assert_eq!(view.record_stats.focus_seconds, 0);
        });
        manager.click(selectors[0]);
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.record_date, dates[0]);
            assert!(view.history.is_empty());
            assert_eq!(view.record_stats.focus_seconds, 0);
        });
        manager.click(selectors[recent_index]);
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.record_date, recent);
            assert_eq!(view.record_stats.focus_seconds, 1_200);
            assert_eq!(view.history.len(), 20);
        });
        assert!(manager.cx.debug_bounds("record-row-1").is_some());
        assert!(manager.cx.debug_bounds("record-row-21").is_none());
        manager.scroll("activity-grid", -20.);
        let offset = manager
            .view
            .read_with(&manager.cx, |view, _| view.panel_scroll[1].offset());
        assert!(
            offset.y < px(0.),
            "the records fixture must actually scroll"
        );
        let grid = manager.bounds("activity-grid");
        manager.click(selectors[earlier_index]);
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.record_date, earlier);
            assert_eq!(view.record_stats.focus_seconds, 2_400);
            assert_eq!(view.history.len(), 20);
            assert!(
                view.history
                    .iter()
                    .all(|session| session.title.starts_with("Past"))
            );
            assert_eq!(view.panel_scroll[1].offset(), offset);
        });
        assert_same_bounds(manager.bounds("activity-grid"), grid);
        // GPUI retains old debug-bounds entries between frames. A new row
        // appearing is meaningful; absence of a previously seen row is not.
        assert!(manager.cx.debug_bounds("record-row-21").is_some());
        // Render a known, otherwise selectable date as future. This proves
        // Button::disabled blocks its real click path independently of the
        // model's additional guard against dates after the actual OS clock.
        manager.view.update(&mut manager.cx, |view, cx| {
            view.now = day_bounds(earlier).0 + 12 * 3_600;
            cx.notify();
        });
        manager.click(selectors[recent_index]);
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(
                view.record_date, earlier,
                "future-day clicks must be disabled"
            );
            assert_eq!(view.record_stats.focus_seconds, 2_400);
            assert_eq!(view.panel_scroll[1].offset(), offset);
        });
        manager.view.update(&mut manager.cx, |view, cx| {
            view.select_record_date(actual_today.succ_opt().unwrap(), cx);
            assert_eq!(view.record_date, earlier);
            view.now = Local::now().timestamp();
            cx.notify();
        });
        manager.click(selectors[recent_index]);
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.record_date, recent);
            assert_eq!(view.record_stats.focus_seconds, 1_200);
            assert_eq!(view.panel_scroll[1].offset(), offset);
        });
        assert!(manager.cx.debug_bounds("record-row-1").is_some());
        assert_same_bounds(manager.bounds("activity-grid"), grid);
    }
}

#[gpui::test]
fn headless_ui_strip_dispatches_to_the_same_manager_model_without_window_reborrows(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let model = manager.view.clone();
    let manager_window = manager.handle;
    let (strip_window, strip_view) = manager.cx.cx.update(|cx| {
        let mut strip_view = None;
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Point::default(),
                        size(px(328.), px(68.)),
                    ))),
                    ..Default::default()
                },
                |window, cx| {
                    let strip = cx.new(|cx| FloatingStrip::new(model, manager_window, window, cx));
                    strip_view = Some(strip.clone());
                    cx.new(|cx| Root::new(strip, window, cx))
                },
            )
            .unwrap();
        (handle, strip_view.unwrap())
    });
    let mut strip = VisualTestContext::from_window(*strip_window, &manager.cx.cx);
    strip.simulate_resize(size(px(328.), px(68.)));
    strip.update(|window, _| window.refresh());
    strip.run_until_parked();
    strip_view.read_with(&strip, |view, _| {
        assert_eq!(view.model.entity_id(), manager.view.entity_id());
    });

    let primary = strip.debug_bounds("strip-primary").unwrap();
    strip.simulate_click(primary.center(), Modifiers::default());
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.preparing);
        assert!(!view.compact);
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input
            .update(cx, |input, cx| input.focus(window, cx));
    });
    manager.cx.simulate_input("One shared timer");
    manager.click("start-focus");
    let started = manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active.len(), 1);
        assert!(view.active[0].is_running());
        view.active[0].id
    });

    strip.update(|window, _| window.refresh());
    let primary = strip.debug_bounds("strip-primary").unwrap();
    strip.simulate_click(primary.center(), Modifiers::default());
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active.len(), 1);
        assert_eq!(view.active[0].id, started);
        assert!(view.active[0].is_paused());
    });
    strip.update(|window, _| window.refresh());
    let open = strip.debug_bounds("strip-open").unwrap();
    strip.simulate_click(open.center(), Modifiers::default());
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.compact);
        assert_eq!(view.active[0].id, started);
    });
    strip.update(|window, _| window.remove_window());
    strip.run_until_parked();
}

#[gpui::test]
fn headless_ui_tag_menu_is_anchored_to_its_visible_control(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new(cx, width, height);
        let trigger = manager.bounds("focus-tag-picker");
        manager.click("focus-tag-picker");
        let option = manager.bounds("focus-tag-option-1");
        assert!(option.top() >= trigger.bottom());
        assert!(
            option.top() - trigger.bottom() <= px(24.),
            "menu should sit directly below its trigger: {trigger:?}, {option:?}"
        );
        assert_horizontally_inside(option, trigger);
    }
}
