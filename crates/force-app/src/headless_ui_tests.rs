//! Structural and event tests on GPUI's TestPlatform only.
//!
//! This backend never opens an OS window. It runs the actual Force view tree,
//! layout and event dispatch with simulated font metrics; it cannot validate
//! native text shaping, raster output, IME or platform-window behavior.

use crate::model::{FloatingStrip, ForceView, Page, day_bounds};
use chrono::{Local, TimeZone};
use gpui::{
    AppContext as _, Bounds, Entity, Focusable as _, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    Pixels, Point, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext,
    WindowBounds, WindowHandle, WindowOptions, point, px, size,
};
use gpui_component::{Root, Theme, ThemeMode};
use std::path::PathBuf;

pub(crate) struct ManagerHarness {
    _directory: tempfile::TempDir,
    pub(crate) view: Entity<ForceView>,
    pub(crate) handle: WindowHandle<Root>,
    pub(crate) cx: VisualTestContext,
}

impl ManagerHarness {
    pub(crate) fn new(cx: &mut TestAppContext, width: f32, height: f32) -> Self {
        Self::new_on_page(cx, width, height, Page::Focus)
    }

    pub(crate) fn new_on_page(
        cx: &mut TestAppContext,
        width: f32,
        height: f32,
        page: Page,
    ) -> Self {
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
            view.page = page;
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

    pub(crate) fn bounds(&mut self, selector: &'static str) -> Bounds<Pixels> {
        self.cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing rendered selector: {selector}"))
    }

    pub(crate) fn click(&mut self, selector: &'static str) {
        let bounds = self.bounds(selector);
        assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
        self.cx
            .simulate_click(bounds.center(), Modifiers::default());
        self.cx.update(|window, _| window.refresh());
        self.cx.run_until_parked();
    }

    pub(crate) fn scroll(&mut self, selector: &'static str, amount: f32) {
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

    pub(crate) fn page(&self) -> Page {
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
        // Render the largest normal daily total and goal without reading or
        // creating a day's worth of real records.
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.today.focus_seconds = 23 * 3_600 + 59 * 60;
            // Navigation refreshes the saved preference. Keep this temporary
            // fixture's database and field baseline consistent with the model.
            view.store
                .as_mut()
                .unwrap()
                .set_goal_minutes(1_439)
                .unwrap();
            view.goal = 1_439;
            view.goal_input
                .update(cx, |input, cx| input.set_value("1439", window, cx));
            cx.notify();
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
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
        assert_horizontally_inside(goal, summary);
        let total = manager.bounds("focus-day-total");
        let target = manager.bounds("focus-day-target");
        assert_horizontally_inside(total, goal);
        assert_horizontally_inside(target, goal);
        assert!(total.right() < target.left());
        assert!((goal.right() - tasks.right()).abs() <= px(1.));
        assert_eq!(task_heading.left(), tasks.left());
        assert!(summary.top() >= header.bottom() - px(1.));
        assert!(summary.bottom() <= viewport.bottom() + px(1.));
        assert!(operation.top() >= summary.bottom() - px(1.));
        assert!(tasks.top() >= summary.bottom() - px(1.));
        assert!((tasks.left() - operation.right() - px(32.)).abs() <= px(1.));
        assert_eq!(tasks.size.width, px(240.));
        assert!(start.top() >= operation.top() - px(1.));
        assert!(
            start.bottom() <= operation.bottom() + px(1.),
            "start action must be visible at {width}×{height}"
        );

        manager.click("settings-tab");
        assert!(manager.page() == Page::Settings);
        assert!(manager.cx.debug_bounds("save-goal").is_none());
        assert!(manager.cx.debug_bounds("tag-kind-rest").is_none());
        let goal_before_edit = manager.bounds("goal-input");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.goal_input
                .update(cx, |input, cx| input.set_value("90", window, cx));
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        assert_same_bounds(manager.bounds("goal-input"), goal_before_edit);
        let content = manager.bounds("manager-content");
        assert_horizontally_inside(manager.bounds("save-goal"), content);
        assert_horizontally_inside(manager.bounds("reset-goal"), content);
        for selector in [
            "settings-timing",
            "settings-tags",
            "settings-form",
            "goal-input",
            "tag-input",
            "save-tag",
        ] {
            assert_horizontally_inside(manager.bounds(selector), content);
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[gpui::test]
fn headless_ui_settings_sound_switch_center_activates_the_setting_once(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 780., 620.);
    manager.click("settings-tab");
    let original = manager
        .view
        .read_with(&manager.cx, |view, _| view.sound_enabled);
    manager.click("settings-sound");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.sound_enabled, !original);
        assert_eq!(
            view.store.as_ref().unwrap().completion_sound().unwrap(),
            !original
        );
    });
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
            input.set_value("0", window, cx);
            input.focus(window, cx);
        });
        view.save_goal(cx);
        assert!(view.toast.is_none());
        view.goal_input
            .update(cx, |input, cx| input.set_value("", window, cx));
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
        assert!(view.toast.is_none());
    });
    manager.click("records-tab");
    assert!(manager.page() == Page::Records);
    manager.click("agenda-tab");
    assert!(manager.page() == Page::Agenda);
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

fn assert_countdown_marker(manager: &mut ManagerHarness, elapsed: i64, target: i64) -> Pixels {
    let track = manager.bounds("countdown-progress-track");
    let marker = manager.bounds("countdown-progress-current");
    let fraction = (elapsed as f32 / target as f32).clamp(0., 1.);
    let expected = track.left() + track.size.width * fraction;
    assert!((marker.center().x - expected).abs() <= px(1.));
    assert_horizontally_inside(marker, manager.bounds("focus-operation"));
    marker.center().x
}

#[gpui::test]
fn headless_ui_titles_and_long_timers_stay_bounded_and_actions_remain_reachable(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        for (title, elapsed, target) in [
            ("A focused task", 378, Some(1_500)),
            (
                "这是需要完整保留并允许分行的长中文任务标题，用来检查较窄窗口中的计时数字和操作按钮是否仍然可用而且不会挤入右侧任务列表",
                100 * 3_600 + 5 * 60 + 7,
                None,
            ),
        ] {
            let mut manager = ManagerHarness::new(cx, width, height);
            let session_id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
                let session = view
                    .store
                    .as_mut()
                    .unwrap()
                    .create_and_start_task(title, view.selected_tag, view.now - elapsed, target)
                    .unwrap();
                view.refresh(cx);
                view.select_session(session.id, window, cx);
                session.id
            });
            manager.cx.update(|window, _| window.refresh());
            manager.cx.run_until_parked();
            let summary = manager.bounds("focus-summary");
            let tasks = manager.bounds("focus-tasks");
            let operation = manager.bounds("focus-operation");
            let heading = manager.bounds("focus-task-title");
            let timer = manager.bounds("focus-timer");
            assert_horizontally_inside(heading, operation);
            assert_horizontally_inside(timer, operation);
            assert!(heading.size.height > px(0.));
            assert!(heading.size.height <= px(28. * 1.35 * 2. + 1.));
            assert!(timer.size.width >= operation.size.width - px(4.));
            assert!(heading.bottom() <= timer.top() + px(1.));
            manager.view.read_with(&manager.cx, |view, _| {
                let session = view.current_session().unwrap();
                assert_eq!(session.id, session_id);
                assert_eq!(session.title, title);
                assert_eq!(session.target_seconds, target);
                assert!(session.elapsed_at(view.now) >= elapsed);
            });

            if let Some(target) = target {
                let elapsed = manager.view.read_with(&manager.cx, |view, _| {
                    view.current_session().unwrap().elapsed_at(view.now)
                });
                assert_countdown_marker(&mut manager, elapsed, target);
            } else {
                assert!(manager.cx.debug_bounds("countdown-progress").is_none());
            }
            let pause = manager.bounds("running-pause");
            let finish = manager.bounds("running-finish");
            assert_horizontally_inside(pause, operation);
            assert_horizontally_inside(finish, operation);
            assert!(pause.right() < finish.left());
            if target.is_some() {
                assert!(pause.top() >= operation.top());
                assert!(finish.bottom() <= operation.bottom() + px(1.));
            } else if finish.bottom() > operation.bottom() {
                manager.scroll("focus-operation", -1_000.);
            }
            let pause = manager.bounds("running-pause");
            let finish = manager.bounds("running-finish");
            assert!(pause.top() >= operation.top() - px(1.));
            assert!(finish.bottom() <= operation.bottom() + px(1.));
            manager.click("running-pause");
            let paused_elapsed = manager.view.read_with(&manager.cx, |view, _| {
                let session = view.current_session().unwrap();
                assert_eq!(session.id, session_id);
                assert!(session.is_paused());
                session.elapsed_at(view.now)
            });
            let paused_marker =
                target.map(|target| assert_countdown_marker(&mut manager, paused_elapsed, target));
            manager.view.update(&mut manager.cx, |view, cx| {
                view.now += 5 * 60;
                cx.notify();
            });
            manager.cx.update(|window, _| window.refresh());
            manager.cx.run_until_parked();
            if let Some(target) = target {
                assert_eq!(
                    assert_countdown_marker(&mut manager, paused_elapsed, target),
                    paused_marker.unwrap()
                );
            }
            manager.click("running-finish");
            manager.view.read_with(&manager.cx, |view, _| {
                let completed = view.completed.as_ref().unwrap();
                assert_eq!(completed.id, session_id);
                assert_eq!(completed.title, title);
                assert!(completed.elapsed_at(view.now) >= elapsed);
                assert!(view.active.is_empty());
            });
            assert_horizontally_inside(manager.bounds("focus-task-title"), operation);
            assert_horizontally_inside(manager.bounds("focus-timer"), operation);
            if let Some(target) = target {
                let finished_elapsed = manager.view.read_with(&manager.cx, |view, _| {
                    view.completed.as_ref().unwrap().elapsed_at(view.now)
                });
                assert_eq!(finished_elapsed, paused_elapsed);
                let marker = assert_countdown_marker(&mut manager, finished_elapsed, target);
                assert_eq!(marker, paused_marker.unwrap());
                assert!(marker < manager.bounds("countdown-progress-track").right());
            } else {
                assert!(manager.cx.debug_bounds("countdown-progress").is_none());
            }
            if manager.bounds("continue-task").bottom() > operation.bottom() {
                manager.scroll("focus-operation", -1_000.);
            }
            let repeat = manager.bounds("continue-task");
            assert!(repeat.top() >= operation.top() - px(1.));
            assert!(repeat.bottom() <= operation.bottom() + px(1.));
            manager.click("continue-task");
            manager.view.read_with(&manager.cx, |view, _| {
                assert_eq!(view.active.len(), 1);
                assert_ne!(view.active[0].id, session_id);
                assert_eq!(view.active[0].title, title);
                assert_eq!(view.active[0].target_seconds, target);
                assert!(view.active[0].is_running());
                assert!(view.completed.is_none());
            });
            assert_same_bounds(manager.bounds("focus-summary"), summary);
            assert_same_bounds(manager.bounds("focus-tasks"), tasks);
        }
    }
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
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        // Browsing a paused task while another timer runs adds the genuine
        // return-to-timer context above the operation, without moving summary
        // or task navigation. A toast is an overlay and cannot force scrolling.
        let selected = view.selected_session.unwrap();
        view.store
            .as_mut()
            .unwrap()
            .create_and_start_task("Another running task", view.selected_tag, view.now, None)
            .unwrap();
        view.refresh(cx);
        view.select_session(selected, window, cx);
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
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
    manager.click("global-new-task");
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
        let row = manager.bounds("record-row-1");
        let title = manager.bounds("record-title-column-1");
        let duration = manager.bounds("record-duration-column-1");
        let action = manager.bounds("record-action-column-1");
        assert_horizontally_inside(row, manager.bounds("records-content"));
        for column in [title, duration, action] {
            assert_horizontally_inside(column, row);
            assert!(column.size.width > px(0.));
        }
        assert!(title.right() < duration.left());
        assert!(duration.right() < action.left());
        assert_horizontally_inside(manager.bounds("record-day-time-1"), duration);
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
fn headless_ui_record_details_show_the_full_saved_tag(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Records);
        let tag_name = "需要完整显示的很长很长的专注标签用于检查记录详情中的文字换行";
        manager.view.update(&mut manager.cx, |view, cx| {
            let now = view.now;
            let store = view.store.as_mut().unwrap();
            let tag = store
                .create_tag(tag_name, force_core::TagKind::Focus)
                .unwrap();
            let session = store.start("A recorded task", tag.id, now - 100).unwrap();
            store.finish(session.id, now - 50).unwrap();
            // Finished records retain the tag name they had at the time.
            store.rename_tag(tag.id, "新名称").unwrap();
            view.refresh(cx);
            assert_eq!(view.history[0].tag_name, tag_name);
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        manager.scroll("records-content", -500.);
        manager.click("record-details-1");
        let row = manager.bounds("record-row-1");
        let tag = manager.bounds("record-detail-tag-1");
        assert_horizontally_inside(tag, row);
        assert!(tag.size.width > px(0.) && tag.size.height > px(0.));
    }
}

#[gpui::test]
fn headless_ui_global_new_task_is_available_without_scheduling_and_preserves_its_draft(
    cx: &mut TestAppContext,
) {
    for page in [Page::Agenda, Page::Records] {
        let mut manager = ManagerHarness::new_on_page(cx, 780., 620., page);
        let entry = manager.bounds("global-new-task");
        assert_horizontally_inside(entry, manager.bounds("manager-header"));
        manager.click("global-new-task");
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(matches!(view.page, Page::Focus));
            assert!(view.preparing);
            assert!(view.selected_task.is_none());
            assert!(!view.agenda.editing);
            assert!(view.tasks.is_empty());
            assert!(view.active.is_empty());
            assert!(view.agenda.plans.is_empty());
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.task_input.update(cx, |input, cx| {
                input.set_value("A task without a date", window, cx)
            });
            view.timer_minutes = 17;
            view.timer_input
                .update(cx, |input, cx| input.set_value("17", window, cx));
        });
        manager.click(if page == Page::Agenda {
            "agenda-tab"
        } else {
            "records-tab"
        });
        manager.click("global-new-task");
        assert!(!manager.cx.has_pending_prompt());
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(matches!(view.page, Page::Focus));
            assert_eq!(view.task_input.read(cx).value(), "A task without a date");
            assert_eq!(view.timer_minutes, 17);
            assert_eq!(view.timer_input.read(cx).value(), "17");
            assert!(view.active.is_empty());
            assert!(view.agenda.plans.is_empty());
        });
        manager.click("save-new-task");
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.tasks.len(), 1);
            assert_eq!(view.tasks[0].title, "A task without a date");
            assert!(view.active.is_empty());
            assert!(view.agenda.plans.is_empty());
        });
    }
}

#[gpui::test]
fn headless_ui_global_new_task_keeps_an_unsaved_calendar_plan(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.begin_plan(None, window, cx);
        view.agenda.title_input.update(cx, |input, cx| {
            input.set_value("Calendar draft worth keeping", window, cx)
        });
        view.agenda
            .end_input
            .update(cx, |input, cx| input.set_value("invalid", window, cx));
        view.agenda.note_input.update(cx, |input, cx| {
            input.set_value("Unfinished calendar note", window, cx)
        });
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.click("global-new-task");
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(matches!(view.page, Page::Focus));
        assert!(view.preparing);
        assert!(view.selected_task.is_none());
        assert!(view.agenda_has_suspended_draft(cx));
        assert!(!view.agenda.popup_open);
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Calendar draft worth keeping"
        );
        assert_eq!(view.agenda.end_input.read(cx).value(), "invalid");
        assert_eq!(
            view.agenda.note_input.read(cx).value(),
            "Unfinished calendar note"
        );
        assert!(view.tasks.is_empty());
        assert!(view.active.is_empty());
        assert!(view.agenda.plans.is_empty());
    });
    manager.click("agenda-tab");
    manager.click("agenda-resume-draft");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(matches!(view.page, Page::Agenda));
        assert!(view.agenda.popup_open && view.agenda.editor_visible);
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Calendar draft worth keeping"
        );
        assert_eq!(view.agenda.end_input.read(cx).value(), "invalid");
        assert_eq!(
            view.agenda.note_input.read(cx).value(),
            "Unfinished calendar note"
        );
        assert!(view.agenda.plans.is_empty());
    });
}

fn open_test_strip(manager: &mut ManagerHarness) -> (Entity<FloatingStrip>, VisualTestContext) {
    let model = manager.view.clone();
    let manager_window = manager.handle;
    let (strip_window, strip_view) = manager.cx.cx.update(|cx| {
        let mut strip_view = None;
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Point::default(),
                        size(
                            px(crate::platform::STRIP_WIDTH),
                            px(crate::platform::STRIP_HEIGHT),
                        ),
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
    refresh_test_strip(&strip_view, &mut strip);
    (strip_view, strip)
}

fn refresh_test_strip(view: &Entity<FloatingStrip>, strip: &mut VisualTestContext) {
    let dimensions = view.read_with(&*strip, |view, _| {
        if view.expanded {
            size(
                px(crate::floating_view::EXPANDED_WIDTH),
                px(crate::floating_view::EXPANDED_HEIGHT),
            )
        } else if view.docked.is_some() {
            size(
                px(crate::floating_view::DOCK_WIDTH),
                px(crate::floating_view::DOCK_HEIGHT),
            )
        } else {
            size(
                px(crate::platform::STRIP_WIDTH),
                px(crate::platform::STRIP_HEIGHT),
            )
        }
    });
    // Native reshaping is checked separately. TestPlatform does not deliver the
    // native resize notification, so supply its viewport explicitly.
    strip.simulate_resize(dimensions);
    strip.update(|window, _| window.refresh());
    strip.run_until_parked();
}

fn click_test_strip(
    view: &Entity<FloatingStrip>,
    strip: &mut VisualTestContext,
    selector: &'static str,
) {
    let bounds = strip.debug_bounds(selector).unwrap();
    strip.simulate_click(bounds.center(), Modifiers::default());
    strip
        .executor()
        .advance_clock(std::time::Duration::from_millis(200));
    strip.run_until_parked();
    refresh_test_strip(view, strip);
}

fn strip_control_bounds(strip: &mut VisualTestContext) -> [Bounds<Pixels>; 2] {
    let root = strip.debug_bounds("strip-root").unwrap();
    let face = strip.debug_bounds("strip-face").unwrap();
    let drawer = strip.debug_bounds("strip-drawer").unwrap();
    let silhouette = strip.debug_bounds("strip-expanded-silhouette").unwrap();
    assert_same_bounds(silhouette, root);
    let text = strip.debug_bounds("strip-title-text").unwrap();
    let time = strip.debug_bounds("strip-time").unwrap();
    let status = strip.debug_bounds("strip-status").unwrap();
    let controls = strip.debug_bounds("strip-controls").unwrap();
    let primary = strip.debug_bounds("strip-primary").unwrap();
    let primary_icon = strip.debug_bounds("strip-primary-icon").unwrap();
    let primary_label = strip.debug_bounds("strip-primary-label").unwrap();
    let open = strip.debug_bounds("strip-open").unwrap();
    for child in [face, drawer, text, time, status, controls, primary, open] {
        assert_horizontally_inside(child, root);
        assert!(child.top() >= root.top() - px(1.));
        assert!(child.bottom() <= root.bottom() + px(1.));
    }
    // Measure the real information boxes, not only the enclosing panel. Text
    // and controls stay usable as the optional finish action appears/disappears.
    assert!(text.size.width >= px(120.));
    assert!(status.size.width >= px(120.));
    assert!((text.left() - status.left()).abs() <= px(1.));
    assert!(text.bottom() < status.top());
    assert!(status.bottom() < controls.top());
    assert_horizontally_inside(time, face);
    // The drawer joins behind the cat while its content stays clickable.
    assert!(drawer.left() < face.right() - px(8.));
    let cat_body_center = face.top() + px(76.);
    assert!((drawer.center().y - cat_body_center).abs() <= px(8.));
    for child in [open, status, controls] {
        assert_horizontally_inside(child, drawer);
        assert!(child.top() >= drawer.top());
        assert!(child.bottom() <= drawer.bottom());
    }
    // The title and actions must not enter the cat's clickable hitbox.
    assert!(face.right() <= open.left());
    assert!(face.right() <= controls.left());
    assert!(primary.size.width >= px(28.) && primary.size.height >= px(24.));
    assert_horizontally_inside(text, open);
    assert!(open.bottom() < status.top());
    assert_horizontally_inside(primary, controls);
    assert_horizontally_inside(primary_icon, primary);
    assert_horizontally_inside(primary_label, primary);
    assert!(primary_icon.right() <= primary_label.left() + px(1.));
    assert!(primary_icon.size.width >= px(10.) && primary_label.size.width > px(0.));
    assert!(primary.top() >= controls.top() - px(1.));
    assert!(primary.bottom() <= controls.bottom() + px(1.));
    assert!(drawer.bottom() - controls.bottom() >= px(8.));
    [primary, open]
}

#[gpui::test]
fn headless_ui_cat_clock_disclosure_preserves_running_timer_and_manager_visibility(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let (running, browsed) = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let store = view.store.as_mut().unwrap();
        let running = store
            .start("Timer that keeps running", 1, view.now - 60)
            .unwrap();
        let browsed = store
            .start("Paused task being inspected", 1, view.now - 30)
            .unwrap();
        store.pause(browsed.id, view.now - 10).unwrap();
        view.refresh(cx);
        view.select_session(browsed.id, window, cx);
        view.hide_manager(window, cx);
        (running.id, browsed.id)
    });
    let (strip_view, mut strip) = open_test_strip(&mut manager);
    assert!(strip.debug_bounds("strip-controls").is_none());
    let collapsed = strip.debug_bounds("strip-root").unwrap();
    assert_eq!(collapsed.size, size(px(88.), px(128.)));
    click_test_strip(&strip_view, &mut strip, "strip-face");
    strip_view.read_with(&strip, |view, _| assert!(view.expanded));
    let controls = strip_control_bounds(&mut strip);
    let finish = strip.debug_bounds("strip-finish").unwrap();
    let finish_label = strip.debug_bounds("strip-finish-label").unwrap();
    let action_row = strip.debug_bounds("strip-controls").unwrap();
    assert_horizontally_inside(finish, action_row);
    assert_horizontally_inside(finish_label, finish);
    assert!(finish_label.top() >= finish.top() && finish_label.bottom() <= finish.bottom());
    assert!(finish_label.size.width > px(0.));
    assert!(finish.left() >= controls[0].right());
    assert!(finish.top() >= action_row.top() && finish.bottom() <= action_row.bottom());
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(
            view.compact,
            "opening the cat drawer must not open management"
        );
        assert_eq!(view.selected_session, Some(browsed));
        assert_eq!(view.strip_session().unwrap().id, running);
        assert!(
            view.active
                .iter()
                .find(|s| s.id == running)
                .unwrap()
                .is_running()
        );
        assert!(
            view.active
                .iter()
                .find(|s| s.id == browsed)
                .unwrap()
                .is_paused()
        );
    });
    click_test_strip(&strip_view, &mut strip, "strip-face");
    strip_view.read_with(&strip, |view, _| assert!(!view.expanded));
    assert_same_bounds(strip.debug_bounds("strip-root").unwrap(), collapsed);
    // Debug selectors retain old bounds. Clicking the former action location
    // proves hidden drawer controls are absent from the active event tree.
    strip.simulate_click(controls[0].center(), Modifiers::default());
    refresh_test_strip(&strip_view, &mut strip);
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.compact);
        assert_eq!(view.selected_session, Some(browsed));
        assert!(
            view.active
                .iter()
                .find(|s| s.id == running)
                .unwrap()
                .is_running()
        );
    });
    click_test_strip(&strip_view, &mut strip, "strip-face");
    click_test_strip(&strip_view, &mut strip, "strip-open");
    strip_view.read_with(&strip, |view, _| assert!(view.expanded));
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.compact);
        assert_eq!(view.selected_session, Some(running));
        assert_eq!(view.strip_session().unwrap().id, running);
        assert!(
            view.active
                .iter()
                .find(|s| s.id == running)
                .unwrap()
                .is_running()
        );
    });
    for edge in [
        crate::platform::DockEdge::Left,
        crate::platform::DockEdge::Right,
    ] {
        strip_view.update(&mut strip, |view, cx| {
            view.docked = Some(edge);
            cx.notify();
        });
        click_test_strip(&strip_view, &mut strip, "strip-face");
        assert_eq!(
            strip.debug_bounds("strip-root").unwrap().size,
            size(px(28.), px(80.))
        );
        click_test_strip(&strip_view, &mut strip, "strip-dock");
        strip_view.read_with(&strip, |view, _| {
            assert!(view.expanded);
            assert_eq!(view.docked, Some(edge));
        });
        strip_control_bounds(&mut strip);
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(!view.compact);
            assert_eq!(view.selected_session, Some(running));
            assert_eq!(view.strip_session().unwrap().id, running);
            assert!(
                view.active
                    .iter()
                    .find(|s| s.id == running)
                    .unwrap()
                    .is_running()
            );
        });
    }
    strip.update(|window, _| window.remove_window());
    strip.run_until_parked();
}

#[gpui::test]
fn headless_ui_strip_dispatches_to_the_same_manager_model_without_window_reborrows(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let (strip_view, mut strip) = open_test_strip(&mut manager);
    strip_view.read_with(&strip, |view, _| {
        assert_eq!(view.model.entity_id(), manager.view.entity_id())
    });
    click_test_strip(&strip_view, &mut strip, "strip-face");
    strip_view.read_with(&strip, |view, _| {
        assert!(!view.expanded && !view.drawer_visible)
    });
    assert!(strip.debug_bounds("strip-controls").is_none());
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.preparing);
        assert!(!view.compact);
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input
            .update(cx, |input, cx| input.focus(window, cx));
    });
    let title = "浮窗应当完整保留这段很长的任务名称，显示时仅在信息区标题行内省略";
    manager.cx.simulate_input(title);
    manager.click("start-focus");
    let started = manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active.len(), 1);
        assert!(view.active[0].is_running());
        assert_eq!(view.active[0].title, title);
        view.active[0].id
    });
    refresh_test_strip(&strip_view, &mut strip);
    click_test_strip(&strip_view, &mut strip, "strip-face");
    strip_control_bounds(&mut strip);
    click_test_strip(&strip_view, &mut strip, "strip-primary");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active[0].id, started);
        assert!(view.active[0].is_paused());
    });
    click_test_strip(&strip_view, &mut strip, "strip-primary");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active[0].id, started);
        assert!(view.active[0].is_running());
    });
    let browsed = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let store = view.store.as_mut().unwrap();
        let paused = store.start("Other paused timer", 1, view.now - 30).unwrap();
        store.pause(paused.id, view.now - 10).unwrap();
        view.refresh(cx);
        view.select_session(paused.id, window, cx);
        assert_eq!(view.strip_session().unwrap().id, started);
        paused.id
    });
    refresh_test_strip(&strip_view, &mut strip);
    click_test_strip(&strip_view, &mut strip, "strip-primary");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(
            view.active
                .iter()
                .find(|s| s.id == started)
                .unwrap()
                .is_paused()
        );
        assert!(
            view.active
                .iter()
                .find(|s| s.id == browsed)
                .unwrap()
                .is_paused()
        );
        assert_eq!(view.strip_session().unwrap().id, started);
    });
    strip_control_bounds(&mut strip);
    click_test_strip(&strip_view, &mut strip, "strip-open");
    click_test_strip(&strip_view, &mut strip, "strip-finish");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active.len(), 1);
        assert_eq!(view.active[0].id, browsed);
        assert!(view.active[0].is_paused());
        assert_eq!(view.completed.as_ref().unwrap().id, started);
    });
    strip_control_bounds(&mut strip);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.navigate(Page::Records, window, cx);
        view.hide_manager(window, cx);
    });
    click_test_strip(&strip_view, &mut strip, "strip-open");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(matches!(view.page, Page::Focus));
        assert!(!view.compact);
        assert_eq!(view.completed.as_ref().unwrap().id, started);
        assert!(view.active[0].is_paused());
    });
    // Viewing the receipt must not resume the other paused timer concealed by
    // that receipt, even after the finish control has disappeared.
    click_test_strip(&strip_view, &mut strip, "strip-primary");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active.len(), 1);
        assert!(view.active[0].is_paused());
        assert_eq!(view.completed.as_ref().unwrap().id, started);
    });
    let long_elapsed = 1_000_000_i64 * 3_600 + 7;
    let long_session = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let session = view
            .store
            .as_mut()
            .unwrap()
            .create_and_start_task(title, view.selected_tag, view.now - long_elapsed, None)
            .unwrap();
        view.refresh(cx);
        view.select_session(session.id, window, cx);
        session.id
    });
    refresh_test_strip(&strip_view, &mut strip);
    strip_control_bounds(&mut strip);
    click_test_strip(&strip_view, &mut strip, "strip-primary");
    manager.view.read_with(&manager.cx, |view, _| {
        let session = view.strip_session().unwrap();
        assert_eq!(session.id, long_session);
        assert!(session.is_paused());
        assert!(session.elapsed_at(view.now) >= long_elapsed);
    });
    manager.view.update(&mut manager.cx, |view, cx| {
        view.compact = true;
        view.storage_failed = true;
        view.toast = Some((
            "无法写入本地计时记录，请展开窗口检查磁盘空间，已有记录仍然保留。".to_owned(),
            std::time::Instant::now(),
        ));
        cx.notify();
    });
    refresh_test_strip(&strip_view, &mut strip);
    strip_control_bounds(&mut strip);
    click_test_strip(&strip_view, &mut strip, "strip-primary");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(!view.compact && view.storage_failed);
        assert!(matches!(view.page, Page::Focus));
        assert_eq!(view.strip_session().unwrap().id, long_session);
        assert!(view.strip_session().unwrap().is_paused());
    });
    strip.update(|window, _| window.remove_window());
    strip.run_until_parked();
}

#[gpui::test]
fn headless_ui_cat_title_opens_selected_saved_task_without_replacing_it(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let task_id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Saved task to revisit", view.selected_tag, view.now)
            .unwrap();
        view.refresh(cx);
        view.select_task(task.id, window, cx);
        view.navigate(Page::Records, window, cx);
        view.hide_manager(window, cx);
        task.id
    });
    let (cat, mut strip) = open_test_strip(&mut manager);
    let before = cat.read_with(&strip, |cat, _| cat.face_click_serial);
    click_test_strip(&cat, &mut strip, "strip-face");
    cat.read_with(&strip, |cat, _| {
        assert_eq!(cat.face_click_serial, before + 1)
    });
    click_test_strip(&cat, &mut strip, "strip-open");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(matches!(view.page, Page::Focus));
        assert!(!view.compact && view.preparing);
        assert_eq!(view.selected_task, Some(task_id));
        assert_eq!(view.task_input.read(cx).value(), "Saved task to revisit");
        assert!(view.active.is_empty());
    });
    strip.update(|window, _| window.remove_window());
    strip.run_until_parked();
}

#[gpui::test]
fn headless_ui_cat_invalid_task_action_returns_to_unchanged_draft(cx: &mut TestAppContext) {
    // Invalid duration on a new draft, and unsaved edits on an existing task.
    for existing in [false, true] {
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        let selected = manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let selected = if existing {
                let task = view
                    .store
                    .as_mut()
                    .unwrap()
                    .create_task("Saved original", view.selected_tag, view.now)
                    .unwrap();
                view.refresh(cx);
                view.select_task(task.id, window, cx);
                Some(task.id)
            } else {
                view.new_task(window, cx);
                None
            };
            view.task_input
                .update(cx, |input, cx| input.set_value("Draft to keep", window, cx));
            if !existing {
                view.timer_input
                    .update(cx, |input, cx| input.set_value("0", window, cx));
            }
            assert!(!view.can_start_task(cx));
            view.navigate(Page::Records, window, cx);
            view.hide_manager(window, cx);
            selected
        });
        let (cat, mut strip) = open_test_strip(&mut manager);
        click_test_strip(&cat, &mut strip, "strip-face");
        strip_control_bounds(&mut strip);
        click_test_strip(&cat, &mut strip, "strip-primary");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(matches!(view.page, Page::Focus));
            assert!(!view.compact && view.preparing);
            assert_eq!(view.selected_task, selected);
            assert_eq!(view.task_input.read(cx).value(), "Draft to keep");
            if !existing {
                assert_eq!(view.timer_input.read(cx).value(), "0");
            }
            assert!(view.active.is_empty());
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            if existing {
                assert!(view.task_input.read(cx).focus_handle(cx).is_focused(window));
            } else {
                assert!(
                    view.timer_input
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                );
            }
        });
        strip.update(|window, _| window.remove_window());
        strip.run_until_parked();
    }
}

#[gpui::test]
fn headless_ui_cat_drag_cancels_old_blink_and_the_next_click_restarts_it(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.new_task(window, cx);
        view.task_input
            .update(cx, |input, cx| input.set_value("Timer draft", window, cx));
        view.hide_manager(window, cx);
    });
    let (cat, mut strip) = open_test_strip(&mut manager);
    click_test_strip(&cat, &mut strip, "strip-face");
    let serial = cat.read_with(&strip, |cat, _| {
        assert!(cat.face_click_started.is_some());
        cat.face_click_serial
    });
    let start = strip.debug_bounds("strip-face").unwrap().center();
    strip.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cat.read_with(&strip, |cat, _| assert!(cat.face_click_started.is_none()));
    let moved = point(start.x + px(10.), start.y);
    strip.simulate_mouse_move(moved, Some(MouseButton::Left), Modifiers::default());
    strip.simulate_mouse_up(moved, MouseButton::Left, Modifiers::default());
    strip.run_until_parked();
    cat.read_with(&strip, |cat, _| {
        assert!(cat.face_click_started.is_none());
        assert_eq!(cat.face_click_serial, serial);
    });
    refresh_test_strip(&cat, &mut strip);
    click_test_strip(&cat, &mut strip, "strip-face");
    cat.read_with(&strip, |cat, _| {
        assert!(cat.face_click_started.is_some());
        assert_eq!(cat.face_click_serial, serial + 1);
    });
    strip.update(|window, _| window.remove_window());
    strip.run_until_parked();
}

#[gpui::test]
fn headless_ui_cat_keeps_an_existing_task_whose_draft_title_was_cleared(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let task_id = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Original saved title", view.selected_tag, view.now)
            .unwrap();
        view.refresh(cx);
        view.select_task(task.id, window, cx);
        view.task_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        view.navigate(Page::Records, window, cx);
        view.hide_manager(window, cx);
        task.id
    });
    let (cat, mut strip) = open_test_strip(&mut manager);
    click_test_strip(&cat, &mut strip, "strip-face");
    cat.read_with(&strip, |cat, _| assert!(cat.expanded));
    click_test_strip(&cat, &mut strip, "strip-primary");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(matches!(view.page, Page::Focus));
        assert!(!view.compact && view.preparing);
        assert_eq!(view.selected_task, Some(task_id));
        assert_eq!(view.task_input.read(cx).value(), "");
        assert!(view.task_has_unsaved_changes(cx));
        assert!(view.active.is_empty());
        assert_eq!(
            view.tasks
                .iter()
                .find(|task| task.id == task_id)
                .unwrap()
                .title,
            "Original saved title"
        );
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(view.task_input.read(cx).focus_handle(cx).is_focused(window));
    });
    strip.update(|window, _| window.remove_window());
    strip.run_until_parked();
}

#[gpui::test]
fn headless_ui_cat_preserves_a_blank_new_task_with_changed_timing(cx: &mut TestAppContext) {
    for (countdown, minutes) in [(true, "37"), (true, ""), (false, "25")] {
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.new_task(window, cx);
            view.timer_input
                .update(cx, |input, cx| input.set_value(minutes, window, cx));
            view.countdown = countdown;
            view.navigate(Page::Records, window, cx);
            view.hide_manager(window, cx);
        });
        let (cat, mut strip) = open_test_strip(&mut manager);
        click_test_strip(&cat, &mut strip, "strip-face");
        cat.read_with(&strip, |cat, _| assert!(cat.expanded));
        click_test_strip(&cat, &mut strip, "strip-primary");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(matches!(view.page, Page::Focus));
            assert!(!view.compact && view.preparing);
            assert_eq!(view.selected_task, None);
            assert_eq!(view.task_input.read(cx).value(), "");
            assert_eq!(view.timer_input.read(cx).value(), minutes);
            assert_eq!(view.countdown, countdown);
            assert!(view.active.is_empty());
            assert!(view.tasks.is_empty());
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert!(view.task_input.read(cx).focus_handle(cx).is_focused(window));
        });
        strip.update(|window, _| window.remove_window());
        strip.run_until_parked();
    }
}

#[gpui::test]
fn headless_ui_cat_idle_blank_default_task_opens_new_task_directly(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.new_task(window, cx);
        view.hide_manager(window, cx);
    });
    let (cat, mut strip) = open_test_strip(&mut manager);
    click_test_strip(&cat, &mut strip, "strip-face");
    cat.read_with(&strip, |cat, _| assert!(!cat.expanded));
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(matches!(view.page, Page::Focus));
        assert!(!view.compact && view.preparing);
        assert!(view.selected_task.is_none());
        assert_eq!(view.task_input.read(cx).value(), "");
        assert_eq!(view.timer_input.read(cx).value(), "25");
        assert!(view.countdown && view.active.is_empty());
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

#[gpui::test]
fn headless_ui_records_reconcile_cross_day_and_live_rows(cx: &mut TestAppContext) {
    let date = Local::now().date_naive().pred_opt().unwrap();
    let midnight = day_bounds(date).0;
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new(cx, width, height);
        manager.view.update(&mut manager.cx, |view, cx| {
            view.record_date = date;
            let store = view.store.as_mut().unwrap();
            let work = store
                .start("Cross-day focus", 1, midnight - 11_226)
                .unwrap();
            store.pause(work.id, midnight - 521).unwrap();
            store.resume(work.id, midnight + 1_500).unwrap();
            store.finish(work.id, midnight + 4_740).unwrap();
            let live = store.start("Still active", 1, midnight + 5_000).unwrap();
            store.pause(live.id, midnight + 5_060).unwrap();
            view.refresh(cx);
        });
        manager.click("records-tab");
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.history.len(), 2);
            assert_eq!(view.record_stats.focus_seconds, 3_300);
            let (start, end) = day_bounds(date);
            assert_eq!(
                view.history
                    .iter()
                    .map(|s| s.elapsed_between(start, end, view.now))
                    .sum::<i64>(),
                3_300
            );
            assert!(view.active[0].is_paused());
        });
        assert_horizontally_inside(
            manager.bounds("record-day-time-1"),
            manager.bounds("manager-content"),
        );
        manager.click("previous-day");
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.history.len(), 1);
            assert_eq!(view.history[0].title, "Cross-day focus");
            assert_eq!(view.record_stats.focus_seconds, 10_705);
        });
    }
}

#[gpui::test]
fn headless_ui_midnight_follows_today_and_preserves_browsed_history(cx: &mut TestAppContext) {
    let today = Local::now().date_naive();
    let yesterday = today.pred_opt().unwrap();
    let older = yesterday.pred_opt().unwrap();
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    for (selected, expected) in [(yesterday, today), (older, older)] {
        manager.view.update(&mut manager.cx, |view, cx| {
            view.now = day_bounds(yesterday).0 + 3_600;
            view.record_date = selected;
            view.tick(cx);
            assert_eq!(view.record_date, expected);
            assert_eq!(
                Local
                    .timestamp_opt(view.now, 0)
                    .single()
                    .unwrap()
                    .date_naive(),
                today
            );
        });
    }
}

#[gpui::test]
fn headless_ui_unfinished_record_deletion_preserves_the_timer(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let now = view.now;
        let session = view
            .store
            .as_mut()
            .unwrap()
            .start("Active focus", 1, now - 60)
            .unwrap();
        view.refresh(cx);
        view.delete_record(session.id, window, cx);
        assert_eq!(
            view.store
                .as_ref()
                .unwrap()
                .active_sessions()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(view.toast.as_ref().unwrap().0, "请先结束计时，再删除记录。");
    });
}

#[gpui::test]
fn headless_ui_browsing_exposes_current_timer_and_switching_preserves_both_tasks(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new(cx, width, height);
        let (task_a, session_a) = manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let store = view.store.as_mut().unwrap();
            let task = store.create_task("Current task", 1, view.now - 60).unwrap();
            let session = store.start_task(task.id, view.now - 60, None).unwrap();
            view.refresh(cx);
            view.select_session(session.id, window, cx);
            (task.id, session.id)
        });
        for page in ["records-tab", "settings-tab"] {
            manager.click(page);
            assert_horizontally_inside(
                manager.bounds("timer-context"),
                manager.bounds("manager-content"),
            );
            manager.click("return-current-timer");
            manager.view.read_with(&manager.cx, |view, _| {
                assert!(view.page == Page::Focus);
                assert_eq!(view.selected_session, Some(session_a));
                assert!(
                    view.active
                        .iter()
                        .find(|s| s.id == session_a)
                        .unwrap()
                        .is_running()
                );
            });
        }
        manager.click("global-new-task");
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.preparing);
            assert!(
                view.active
                    .iter()
                    .find(|s| s.id == session_a)
                    .unwrap()
                    .is_running()
            );
        });
        assert_horizontally_inside(
            manager.bounds("timer-context"),
            manager.bounds("focus-operation"),
        );
        manager.click("return-current-timer");
        manager.click("global-new-task");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.task_input
                .update(cx, |input, cx| input.set_value("Next task", window, cx));
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        let start = manager.bounds("start-focus");
        let operation = manager.bounds("focus-operation");
        assert!(
            start.bottom() <= operation.bottom() + px(1.),
            "switch-and-start action must be fully visible at {width}×{height}"
        );
        manager
            .view
            .read_with(&manager.cx, |view, cx| assert!(view.can_start_task(cx)));
        manager.click("start-focus");
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(
                view.active
                    .iter()
                    .find(|s| s.id == session_a)
                    .unwrap()
                    .is_paused()
            );
            assert_eq!(view.active.iter().filter(|s| s.is_running()).count(), 1);
            assert_eq!(view.strip_session().unwrap().title, "Next task");
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.select_task(task_a, window, cx)
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        manager.click("running-pause");
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.strip_session().unwrap().id, session_a);
            assert_eq!(view.active.iter().filter(|s| s.is_running()).count(), 1);
            assert!(
                view.active
                    .iter()
                    .any(|s| s.title == "Next task" && s.is_paused())
            );
        });
    }
}

#[gpui::test]
fn headless_ui_task_edits_save_without_starting_and_prompt_before_switching(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let store = view.store.as_mut().unwrap();
        store
            .create_task("Original task", 1, view.now - 100)
            .unwrap();
        store.create_task("Other task", 2, view.now - 100).unwrap();
        view.refresh(cx);
        view.select_task(1, window, cx);
        view.task_input
            .update(cx, |input, cx| input.set_value("Renamed task", window, cx));
    });
    manager.click("start-focus");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.active.is_empty());
        assert_eq!(
            view.tasks.iter().find(|t| t.id == 1).unwrap().title,
            "Original task"
        );
    });
    manager.click("save-task");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.active.is_empty());
        assert!(!view.task_has_unsaved_changes(cx));
        assert_eq!(
            view.tasks.iter().find(|t| t.id == 1).unwrap().title,
            "Renamed task"
        );
    });
    for (choice, should_switch) in [("取消", false), ("不保存", true), ("保存", true)] {
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.select_task(1, window, cx);
            view.task_input
                .update(cx, |input, cx| input.set_value("Pending edit", window, cx));
        });
        manager.click("task-row-2");
        assert!(manager.cx.has_pending_prompt());
        manager.cx.simulate_prompt_answer(choice);
        manager.cx.run_until_parked();
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.selected_task, Some(if should_switch { 2 } else { 1 }));
            assert!(view.active.is_empty());
            let stored = view.tasks.iter().find(|t| t.id == 1).unwrap();
            assert_eq!(
                stored.title,
                if choice == "保存" {
                    "Pending edit"
                } else {
                    "Renamed task"
                }
            );
            if !should_switch {
                assert_eq!(
                    view.task_input
                        .read_with(&manager.cx, |input, _| input.value()),
                    "Pending edit"
                );
            }
        });
        if !should_switch {
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                view.reset_task_changes(window, cx)
            });
        }
    }
}

#[gpui::test]
fn headless_ui_task_submit_saves_and_invalid_save_keeps_the_draft(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.store
            .as_mut()
            .unwrap()
            .create_task("Original", 1, view.now - 100)
            .unwrap();
        view.refresh(cx);
        view.select_task(1, window, cx);
        view.task_input.update(cx, |input, cx| {
            input.set_value("Saved by Return", window, cx);
            input.focus(window, cx);
        });
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    // Exercise the input submit subscription; native Return is checked in the signed app.
    manager.view.update(&mut manager.cx, |view, cx| {
        view.task_input.update(cx, |_, cx| {
            cx.emit(gpui_component::input::InputEvent::PressEnter { secondary: false })
        });
    });
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.active.is_empty());
        assert_eq!(view.tasks[0].title, "Saved by Return");
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        assert!(!view.save_task_changes(cx));
        assert_eq!(view.task_input.read(cx).value(), "");
        assert_eq!(view.tasks[0].title, "Saved by Return");
        assert!(view.active.is_empty());
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let form = manager.bounds("focus-form");
    let tasks = manager.bounds("focus-tasks");
    manager.bounds("manager-feedback");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input
            .update(cx, |input, cx| input.set_value("Corrected", window, cx));
        assert!(view.save_task_changes(cx));
        assert!(view.toast.is_none());
        assert!(view.active.is_empty());
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let corrected_form = manager.bounds("focus-form");
    assert_eq!(corrected_form.left(), form.left());
    assert_eq!(corrected_form.size.width, form.size.width);
    // Clearing the inline validation error legitimately reduces form height.
    assert!(corrected_form.size.height < form.size.height);
    assert_same_bounds(manager.bounds("focus-tasks"), tasks);
}

#[gpui::test]
fn headless_ui_global_rest_protects_edits_and_returns_to_the_running_task(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let store = view.store.as_mut().unwrap();
        let a = store.create_task("Running A", 1, view.now - 100).unwrap();
        store.create_task("Viewed B", 2, view.now - 100).unwrap();
        store.start_task(a.id, view.now - 60, None).unwrap();
        view.refresh(cx);
        view.select_task(2, window, cx);
        view.task_input
            .update(cx, |input, cx| input.set_value("Edited B", window, cx));
        view.start_global_break(window, cx);
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("保存");
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.return_focus_task().unwrap().title, "Running A");
        assert_eq!(
            view.tasks.iter().find(|t| t.id == 2).unwrap().title,
            "Edited B"
        );
        assert_eq!(view.active.iter().filter(|s| s.is_running()).count(), 1);
        assert_eq!(
            view.strip_session().unwrap().tag_kind,
            force_core::TagKind::Rest
        );
    });
    manager.click("running-pause");
    manager.click("return-from-rest");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.strip_session().unwrap().title, "Running A");
        assert!(
            !view
                .active
                .iter()
                .any(|s| s.tag_kind == force_core::TagKind::Rest)
        );
    });
}

#[gpui::test]
fn headless_ui_rest_without_return_ends_before_preparing_and_new_tags_are_focus(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 780., 620.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.start_global_break(window, cx)
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    assert_horizontally_inside(
        manager.bounds("return-from-rest"),
        manager.bounds("focus-operation"),
    );
    assert!(manager.cx.debug_bounds("running-finish").is_none());
    assert!(manager.cx.debug_bounds("running-rest").is_none());
    manager.click("return-from-rest");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.preparing);
        assert!(view.active.is_empty());
        assert_eq!(view.history.len(), 1);
        assert_eq!(view.history[0].tag_kind, force_core::TagKind::Rest);
    });
    manager.click("settings-tab");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.new_tag_kind = force_core::TagKind::Rest;
        view.tag_input.update(cx, |input, cx| {
            input.set_value("New label", window, cx);
            input.focus(window, cx);
        });
    });
    manager.view.update(&mut manager.cx, |view, cx| {
        view.tag_input.update(cx, |_, cx| {
            cx.emit(gpui_component::input::InputEvent::PressEnter { secondary: false })
        });
    });
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(
            view.tags
                .iter()
                .find(|t| t.name == "New label")
                .unwrap()
                .kind,
            force_core::TagKind::Focus
        );
    });
}

#[gpui::test]
fn headless_ui_new_draft_is_saved_without_starting_before_global_rest(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.new_task(window, cx);
        view.task_input.update(cx, |input, cx| {
            input.set_value("Unstarted draft", window, cx)
        });
        view.start_global_break(window, cx);
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    manager.cx.run_until_parked();
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(view.active.is_empty());
        assert_eq!(view.task_input.read(cx).value(), "Unstarted draft");
        view.start_global_break(window, cx);
    });
    manager.cx.simulate_prompt_answer("保存");
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.tasks.len(), 1);
        assert_eq!(view.tasks[0].title, "Unstarted draft");
        assert_eq!(view.tasks[0].session_count, 0);
        assert!(
            view.active
                .iter()
                .all(|s| s.tag_kind == force_core::TagKind::Rest)
        );
    });
}

#[gpui::test]
fn headless_ui_completed_round_actions_are_visible_at_minimum_size(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 780., 620.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input.update(cx, |input, cx| {
            input.set_value("Completed round", window, cx)
        });
        view.start(window, cx);
        let id = view.active[0].id;
        view.finish(id, cx);
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    let operation = manager.bounds("focus-operation");
    for selector in ["continue-task", "rest-five", "complete-task"] {
        let action = manager.bounds(selector);
        assert_horizontally_inside(action, operation);
        assert!(action.top() >= operation.top());
        assert!(action.bottom() <= operation.bottom());
    }
}

#[gpui::test]
fn headless_ui_completing_an_inactive_task_clears_its_form_without_a_draft_prompt(
    cx: &mut TestAppContext,
) {
    for has_record in [false, true] {
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let now = view.now;
            let store = view.store.as_mut().unwrap();
            let task = store.create_task("Finished task", 1, now - 100).unwrap();
            if has_record {
                let session = store.start_task(task.id, now - 100, None).unwrap();
                store.finish(session.id, now - 50).unwrap();
            }
            view.refresh(cx);
            view.select_task(task.id, window, cx);
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        manager.click("complete-task");
        assert!(
            !manager.cx.has_pending_prompt(),
            "a completed task's old title must not become an unsaved new draft"
        );
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.preparing);
            assert!(view.selected_task.is_none());
            assert!(view.selected_session.is_none());
            assert!(view.task_input.read(cx).value().is_empty());
            assert_eq!(view.tasks.len(), 1);
            assert!(view.tasks[0].completed_at.is_some());
            assert!(view.active.is_empty());
            assert_eq!(view.history.len(), usize::from(has_record));
        });
    }
}

#[gpui::test]
fn headless_ui_repeated_new_preserves_draft_before_selection_and_global_rest(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.store
            .as_mut()
            .unwrap()
            .create_task("Existing task", 1, view.now - 100)
            .unwrap();
        view.refresh(cx);
        view.task_input.update(cx, |input, cx| {
            input.set_value("Initial draft", window, cx);
        });
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.bounds("global-new-task");
    manager.click("duration-custom");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.timer_input
            .update(cx, |input, cx| input.set_value("37", window, cx));
        view.new_task(window, cx);
        view.new_task(window, cx);
        assert_eq!(view.task_input.read(cx).value(), "Initial draft");
        assert_eq!(view.timer_input.read(cx).value(), "37");
        assert!(view.countdown);
        assert!(view.task_input.read(cx).focus_handle(cx).is_focused(window));
    });
    assert!(!manager.cx.has_pending_prompt());
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.start_global_break(window, cx);
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("取消");
    manager.cx.run_until_parked();
    manager.click("task-row-1");
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("保存");
    manager.cx.run_until_parked();
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.selected_task, Some(1));
        assert_eq!(view.task_input.read(cx).value(), "Existing task");
        assert_eq!(view.tasks.len(), 2);
        assert!(view.tasks.iter().any(|task| task.title == "Initial draft"));
        assert!(view.active.is_empty());
    });
}

#[gpui::test]
fn headless_ui_custom_timer_starts_with_typed_minutes_at_default_and_minimum_size(
    cx: &mut TestAppContext,
) {
    for (width, height, minutes) in [(1040., 780., 37), (780., 620., 1), (780., 620., 1_440)] {
        let mut manager = ManagerHarness::new(cx, width, height);
        manager.bounds("global-new-task");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.task_input.update(cx, |input, cx| {
                input.set_value("Custom duration", window, cx)
            });
        });
        manager.bounds("duration-input");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert_eq!(view.timer_input.read(cx).value(), "25");
            view.timer_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
        });
        manager.cx.simulate_input(&minutes.to_string());
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        let operation = manager.bounds("focus-operation");
        for selector in [
            "duration-25",
            "duration-45",
            "duration-60",
            "duration-0",
            "duration-custom",
            "duration-input",
            "start-focus",
            "save-new-task",
        ] {
            let control = manager.bounds(selector);
            assert_horizontally_inside(control, operation);
            assert!(control.top() >= operation.top() - px(1.));
            assert!(control.bottom() <= operation.bottom() + px(1.));
        }
        let start = manager.bounds("start-focus");
        let save = manager.bounds("save-new-task");
        assert!((start.top() - save.top()).abs() <= px(1.));
        assert!(start.right() <= save.left() + px(1.));
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.can_start_task(cx));
            assert!(view.timer_duration_error(cx).is_none());
        });
        if minutes == 37 {
            manager.click("start-focus");
        } else {
            manager.cx.simulate_keystrokes("enter");
            manager.cx.run_until_parked();
        }
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.active.len(), 1);
            let session = &view.active[0];
            assert_eq!(session.title, "Custom duration");
            assert_eq!(session.target_seconds, Some(i64::from(minutes) * 60));
            assert!(session.is_running());
            assert_eq!(
                view.store.as_ref().unwrap().active_sessions().unwrap(),
                view.active
            );
        });
        manager.click("global-new-task");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.is_new_task_form());
            assert!(view.countdown);
            assert_eq!(view.timer_input.read(cx).value(), "25");
            assert_eq!(view.timer_minutes, 25);
            assert!(view.active[0].is_running());
            assert_eq!(view.active[0].target_seconds, Some(i64::from(minutes) * 60));
        });
    }
}

#[gpui::test]
fn headless_ui_invalid_custom_minutes_block_start_and_presets_restore_validity(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input.update(cx, |input, cx| {
            input.set_value("Validated duration", window, cx)
        });
    });
    for invalid in ["0", "1441", "", "minutes"] {
        manager.click("duration-custom");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.timer_input
                .update(cx, |input, cx| input.set_value(invalid, window, cx));
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        manager.bounds("timer-duration-error");
        manager.click("start-focus");
        manager.view.update(&mut manager.cx, |view, cx| {
            view.timer_input.update(cx, |_, cx| {
                cx.emit(gpui_component::input::InputEvent::PressEnter { secondary: false });
            });
            view.task_input.update(cx, |_, cx| {
                cx.emit(gpui_component::input::InputEvent::PressEnter { secondary: false });
            });
        });
        manager.cx.run_until_parked();
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(!view.can_start_task(cx));
            assert!(view.active.is_empty());
            assert!(
                view.store
                    .as_ref()
                    .unwrap()
                    .tasks(view.now)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(view.timer_input.read(cx).value(), invalid);
        });
        manager.click("duration-45");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.can_start_task(cx));
            assert!(view.countdown);
            assert_eq!(view.timer_input.read(cx).value(), "45");
            assert!(view.timer_duration_error(cx).is_none());
            assert_eq!(view.timer_minutes, 45);
        });
        manager.click("duration-custom");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            assert_eq!(view.timer_input.read(cx).value(), "45");
            view.timer_input
                .update(cx, |input, cx| input.set_value(invalid, window, cx));
        });
        manager.click("duration-0");
        // GPUI 0.2.2 retains removed selectors in its recycled Frame map.
        // Establish mode changes through live state and dispatched behavior.
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(!view.countdown);
            assert!(view.can_start_task(cx));
            assert!(view.timer_duration_error(cx).is_none());
            assert_eq!(view.timer_input.read(cx).value(), invalid);
        });
        manager.click("duration-custom");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.countdown);
            assert!(!view.can_start_task(cx));
            assert!(view.timer_duration_error(cx).is_some());
            assert_eq!(view.timer_input.read(cx).value(), invalid);
        });
        manager.click("duration-0");
    }
    manager.click("start-focus");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.active.len(), 1);
        assert_eq!(view.active[0].target_seconds, None);
    });
}

#[gpui::test]
fn headless_ui_saving_a_new_task_preserves_timer_draft_without_starting(cx: &mut TestAppContext) {
    for (width, height, countdown, minutes) in [
        (1040., 780., true, "37"),
        (780., 620., false, "42"),
        (780., 620., true, "0"),
    ] {
        let mut manager = ManagerHarness::new(cx, width, height);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.task_input.update(cx, |input, cx| {
                input.set_value("Save for later", window, cx)
            });
            view.timer_input
                .update(cx, |input, cx| input.set_value(minutes, window, cx));
            view.countdown = countdown;
            cx.notify();
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        let operation = manager.bounds("focus-operation");
        let start = manager.bounds("start-focus");
        let save = manager.bounds("save-new-task");
        for control in [start, save] {
            assert_horizontally_inside(control, operation);
            assert!(control.top() >= operation.top() - px(1.));
            assert!(control.bottom() <= operation.bottom() + px(1.));
        }
        assert!((start.top() - save.top()).abs() <= px(1.));
        assert!(start.right() <= save.left() + px(1.));
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.can_save_task(cx));
            assert_eq!(view.can_start_task(cx), !countdown || minutes != "0");
        });
        manager.click("save-new-task");
        let saved_id = manager.view.read_with(&manager.cx, |view, cx| {
            let id = view.selected_task.unwrap();
            assert!(view.preparing);
            assert!(view.active.is_empty());
            assert!(view.history.is_empty());
            assert_eq!(view.tasks.len(), 1);
            assert_eq!(view.tasks[0].id, id);
            assert_eq!(view.tasks[0].title, "Save for later");
            assert_eq!(view.tasks[0].total_seconds, 0);
            assert_eq!(view.countdown, countdown);
            assert_eq!(view.timer_input.read(cx).value(), minutes);
            assert!(!view.task_has_unsaved_changes(cx));
            assert!(
                view.store
                    .as_ref()
                    .unwrap()
                    .active_sessions()
                    .unwrap()
                    .is_empty()
            );
            id
        });
        assert!(!manager.cx.has_pending_prompt());
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(!view.is_new_task_form());
            assert!(!view.can_save_task(cx));
        });
        // Re-clicking the former save location must not create another task.
        manager
            .cx
            .simulate_click(save.center(), Modifiers::default());
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        manager
            .view
            .read_with(&manager.cx, |view, _| assert_eq!(view.tasks.len(), 1));
        manager.bounds("global-new-task");
        if countdown && minutes == "0" {
            manager.bounds("timer-duration-error");
            manager.click("start-focus");
            manager.view.read_with(&manager.cx, |view, _| {
                assert!(view.active.is_empty());
            });
            manager.click("duration-60");
        }
        manager.click("start-focus");
        manager.view.read_with(&manager.cx, |view, _| {
            assert_eq!(view.tasks.len(), 1);
            assert_eq!(view.active.len(), 1);
            assert_eq!(view.active[0].task_id, Some(saved_id));
            assert_eq!(
                view.active[0].target_seconds,
                match (countdown, minutes) {
                    (true, "0") => Some(3_600),
                    (true, _) => Some(2_220),
                    (false, _) => None,
                }
            );
        });
    }
}

#[gpui::test]
fn headless_ui_saving_a_new_task_keeps_the_current_round_running(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let original = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let session = view
            .store
            .as_mut()
            .unwrap()
            .create_and_start_task("Keep focusing", 1, view.now - 60, Some(1_500))
            .unwrap();
        view.refresh(cx);
        view.select_session(session.id, window, cx);
        view.new_task(window, cx);
        view.task_input
            .update(cx, |input, cx| input.set_value("Queued task", window, cx));
        session
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.click("save-new-task");
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.tasks.len(), 2);
        assert_eq!(view.active, vec![original.clone()]);
        assert_eq!(
            view.store.as_ref().unwrap().active_sessions().unwrap(),
            vec![original]
        );
        assert!(view.preparing);
        assert!(view.selected_task.is_some());
        assert!(view.browsing_other_timer());
    });
}

#[gpui::test]
fn headless_ui_edit_duration_preserves_running_and_paused_sessions_and_intervals(
    cx: &mut TestAppContext,
) {
    for (target, paused, width, height) in [
        (Some(1_500), false, 1040., 780.),
        (Some(1_500), true, 780., 620.),
        (None, false, 780., 620.),
        (None, true, 1040., 780.),
    ] {
        let mut manager = ManagerHarness::new(cx, width, height);
        let original = manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let now = view.now;
            let store = view.store.as_mut().unwrap();
            let session = store
                .create_and_start_task("Editable duration", 1, now - 180, target)
                .unwrap();
            if paused {
                store.pause(session.id, now - 60).unwrap();
            }
            view.refresh(cx);
            view.select_session(session.id, window, cx);
            view.active[0].clone()
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        manager.click("edit-duration");
        let operation = manager.bounds("focus-operation");
        for selector in ["session-duration-input", "save-duration", "cancel-duration"] {
            assert_horizontally_inside(manager.bounds(selector), operation);
        }
        for invalid in ["0", "1441", "", "minutes", "1"] {
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                view.duration_input
                    .update(cx, |input, cx| input.set_value(invalid, window, cx));
            });
            manager.cx.update(|window, _| window.refresh());
            manager.cx.run_until_parked();
            assert_horizontally_inside(manager.bounds("duration-error"), operation);
            manager.click("save-duration");
            manager.view.update(&mut manager.cx, |view, cx| {
                view.duration_input.update(cx, |_, cx| {
                    cx.emit(gpui_component::input::InputEvent::PressEnter { secondary: false });
                });
            });
            manager.cx.run_until_parked();
            manager.view.read_with(&manager.cx, |view, cx| {
                assert!(!view.can_save_duration(cx));
                assert_eq!(view.editing_duration, Some(original.id));
                assert_eq!(view.active, vec![original.clone()]);
                assert_eq!(
                    view.store.as_ref().unwrap().active_sessions().unwrap(),
                    vec![original.clone()]
                );
            });
        }
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.duration_input.update(cx, |input, cx| {
                input.set_value("37", window, cx);
                input.focus(window, cx);
            });
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        if target.is_some() {
            manager.click("save-duration");
        } else {
            manager.cx.simulate_keystrokes("enter");
            manager.cx.run_until_parked();
        }
        let mut expected = original;
        expected.target_seconds = Some(37 * 60);
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.editing_duration.is_none());
            assert_eq!(view.active, vec![expected.clone()]);
            assert_eq!(
                view.store.as_ref().unwrap().active_sessions().unwrap(),
                vec![expected.clone()]
            );
            assert_eq!(view.active[0].is_paused(), paused);
        });
        manager.bounds("edit-duration");
    }
}

#[gpui::test]
fn headless_ui_cancelling_or_leaving_duration_edit_preserves_the_saved_target(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let original = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let store = view.store.as_mut().unwrap();
        let session = store
            .create_and_start_task("First task", 1, view.now - 120, Some(1_500))
            .unwrap();
        store.create_task("Second task", 1, view.now).unwrap();
        view.refresh(cx);
        view.select_session(session.id, window, cx);
        session
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.click("edit-duration");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.duration_input
            .update(cx, |input, cx| input.set_value("37", window, cx));
        view.hide_manager(window, cx);
        assert!(view.compact);
        assert_eq!(view.editing_duration, Some(original.id));
        assert_eq!(view.duration_input.read(cx).value(), "37");
        view.show_manager(window, cx);
        assert_eq!(view.editing_duration, Some(original.id));
        assert_eq!(view.duration_input.read(cx).value(), "37");
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.bounds("session-duration-input");
    manager.click("cancel-duration");
    for destination in [
        "cancel-duration",
        "task-row-2",
        "records-tab",
        "global-new-task",
    ] {
        manager.click("edit-duration");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.duration_input
                .update(cx, |input, cx| input.set_value("37", window, cx));
        });
        manager.click(destination);
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.editing_duration.is_none());
            assert_eq!(
                view.store.as_ref().unwrap().active_sessions().unwrap(),
                vec![original.clone()]
            );
        });
        assert!(!manager.cx.has_pending_prompt());
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.select_session(original.id, window, cx);
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
    }
}

#[gpui::test]
fn headless_ui_expired_session_closes_duration_editor_without_changing_its_target(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let original = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let session = view
            .store
            .as_mut()
            .unwrap()
            .create_and_start_task("Expiring task", 1, view.now - 10, Some(60))
            .unwrap();
        view.refresh(cx);
        view.select_session(session.id, window, cx);
        session
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.click("edit-duration");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.duration_input
            .update(cx, |input, cx| input.set_value("37", window, cx));
        // Simulate the durable completion boundary without waiting on wall time.
        let expired = view
            .store
            .as_mut()
            .unwrap()
            .finish_exact_target(original.id, original.started_at + 60)
            .unwrap();
        assert_eq!(expired.target_seconds, Some(60));
        assert_eq!(expired.elapsed_at(view.now), 60);
        view.refresh(cx);
        assert!(view.editing_duration.is_none());
        assert!(!view.can_save_duration(cx));
        view.save_duration(window, cx);
        assert!(view.active.is_empty());
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, _| {
        let session = view
            .store
            .as_ref()
            .unwrap()
            .sessions()
            .unwrap()
            .into_iter()
            .find(|session| session.id == original.id)
            .unwrap();
        assert_eq!(session.target_seconds, Some(60));
        assert_eq!(session.elapsed_at(view.now), 60);
    });
}

#[gpui::test]
fn headless_ui_storage_retry_preserves_goal_drafts_and_loads_an_untouched_goal(
    cx: &mut TestAppContext,
) {
    for draft in [Some("75"), Some("invalid"), None] {
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        manager.click("settings-tab");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            if let Some(draft) = draft {
                view.goal_input
                    .update(cx, |input, cx| input.set_value(draft, window, cx));
            }
            // A saved value unseen by this window also covers recovery after
            // startup could not load the goal and left the untouched default.
            view.store.as_mut().unwrap().set_goal_minutes(90).unwrap();
            view.store = None;
            let database = view.data_dir.join("force.sqlite3");
            let backup = view.data_dir.join("recovery.sqlite3");
            std::fs::rename(&database, &backup).unwrap();
            std::fs::write(&database, b"invalid temporary test database").unwrap();
            view.open_store(cx);
            assert!(view.storage_failed);
            assert!(view.store.is_none());
            assert_eq!(view.goal, 300);
            std::fs::remove_file(&database).unwrap();
            std::fs::rename(&backup, &database).unwrap();
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        manager.click("retry-storage");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(!view.storage_failed);
            assert_eq!(view.goal, 90);
            assert_eq!(view.goal_input.read(cx).value(), draft.unwrap_or("90"));
            assert_eq!(view.store.as_ref().unwrap().goal_minutes().unwrap(), 90);
        });
    }
}

#[gpui::test]
fn headless_ui_starting_a_stale_selected_task_preserves_external_edits(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Original task", 1, view.now - 100)
            .unwrap();
        view.refresh(cx);
        view.select_task(task.id, window, cx);
        let mut external = force_core::Store::open(view.data_dir.join("force.sqlite3")).unwrap();
        external
            .update_task(task.id, "Updated elsewhere", 2)
            .unwrap();
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.click("start-focus");
    manager.view.read_with(&manager.cx, |view, cx| {
        let tasks = view.store.as_ref().unwrap().tasks(view.now).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Updated elsewhere");
        assert_eq!(tasks[0].tag_id, 2);
        assert_eq!(view.active.len(), 1);
        let session = &view.active[0];
        assert!(session.is_running());
        assert_eq!(session.task_id, Some(tasks[0].id));
        assert_eq!(session.title, "Updated elsewhere");
        assert_eq!(session.tag_id, 2);
        assert_eq!(view.selected_session, Some(session.id));
        assert_eq!(view.selected_tag, 2);
        assert_eq!(view.task_input.read(cx).value(), "Updated elsewhere");
    });
}

#[gpui::test]
fn headless_ui_reopening_the_same_tag_preserves_its_draft(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.navigate(Page::Settings, window, cx);
        view.begin_edit_tag(1, window, cx);
        view.tag_input.update(cx, |input, cx| {
            input.set_value("Pending rename", window, cx)
        });
        view.begin_edit_tag(1, window, cx);
    });
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.tag_input.read(cx).value(), "Pending rename");
    });
    assert!(!manager.cx.has_pending_prompt());
}

#[gpui::test]
fn headless_ui_tag_drafts_require_a_choice_before_switching_edit_targets(cx: &mut TestAppContext) {
    for new_draft in [false, true] {
        for choice in ["取消", "不保存", "保存"] {
            let mut manager = ManagerHarness::new(cx, 1040., 780.);
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                view.navigate(Page::Settings, window, cx);
                if !new_draft {
                    view.begin_edit_tag(1, window, cx);
                }
                view.tag_input
                    .update(cx, |input, cx| input.set_value("Pending tag", window, cx));
                view.begin_edit_tag(2, window, cx);
            });
            assert!(manager.cx.has_pending_prompt());
            manager.cx.simulate_prompt_answer(choice);
            manager.cx.run_until_parked();
            manager.view.read_with(&manager.cx, |view, cx| {
                if choice == "取消" {
                    assert_eq!(view.editing_tag, if new_draft { None } else { Some(1) });
                    assert_eq!(view.tag_input.read(cx).value(), "Pending tag");
                } else {
                    assert_eq!(view.editing_tag, Some(2));
                    assert_eq!(
                        view.tag_input.read(cx).value(),
                        view.tags.iter().find(|tag| tag.id == 2).unwrap().name
                    );
                }
                assert_eq!(
                    view.tags.iter().any(|tag| tag.name == "Pending tag"),
                    choice == "保存"
                );
            });
        }
    }
}

#[gpui::test]
fn headless_ui_adding_a_tag_does_not_retag_a_saved_task(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Saved task", 1, view.now)
            .unwrap();
        view.refresh(cx);
        view.select_task(task.id, window, cx);
        view.navigate(Page::Settings, window, cx);
        view.tag_input.update(cx, |input, cx| {
            input.set_value("Additional tag", window, cx)
        });
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.click("save-tag");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.tags.iter().any(|tag| tag.name == "Additional tag"));
        assert_eq!(view.selected_tag, 1);
        assert!(!view.task_has_unsaved_changes(cx));
    });
}

#[gpui::test]
fn headless_ui_tag_edit_baseline_does_not_turn_external_renames_into_user_edits(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.navigate(Page::Settings, window, cx);
        view.begin_edit_tag(1, window, cx);
        let mut external = force_core::Store::open(view.data_dir.join("force.sqlite3")).unwrap();
        external.rename_tag(1, "External rename").unwrap();
        view.refresh(cx);
        view.save_tag(window, cx);
        view.begin_edit_tag(2, window, cx);
    });
    assert!(!manager.cx.has_pending_prompt());
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.editing_tag, Some(2));
        assert_eq!(
            view.tags.iter().find(|tag| tag.id == 1).unwrap().name,
            "External rename"
        );
    });
}

#[gpui::test]
fn headless_ui_invalid_tag_draft_blocks_save_and_switch_to_new_until_discarded(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.click("settings-tab");
    manager.click("edit-tag-1");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.tag_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        view.open_tag_settings(window, cx);
    });
    assert!(manager.cx.has_pending_prompt());
    manager.cx.simulate_prompt_answer("保存");
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.editing_tag, Some(1));
        assert!(view.tag_input.read(cx).value().is_empty());
        assert!(view.tag_draft_error(cx).is_some());
        assert!(!view.can_save_tag(cx));
    });
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.open_tag_settings(window, cx)
    });
    manager.cx.simulate_prompt_answer("不保存");
    manager.cx.run_until_parked();
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.editing_tag.is_none());
        assert!(view.tag_input.read(cx).value().is_empty());
    });
}

#[gpui::test]
fn headless_ui_task_validation_agrees_with_return_and_preserves_invalid_edits(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Saved task", 1, view.now)
            .unwrap();
        view.refresh(cx);
        view.select_task(task.id, window, cx);
    });
    for value in [String::new(), "Long".repeat(51), "Invalid\0name".to_owned()] {
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.task_input
                .update(cx, |input, cx| input.set_value(value.clone(), window, cx));
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        manager.bounds("task-title-error");
        manager.click("save-task");
        manager.view.update(&mut manager.cx, |view, cx| {
            view.task_input.update(cx, |_, cx| {
                cx.emit(gpui_component::input::InputEvent::PressEnter { secondary: false })
            });
        });
        manager.cx.run_until_parked();
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(!view.can_save_task(cx));
            assert!(!view.can_start_task(cx));
            assert_eq!(view.task_input.read(cx).value(), value);
            assert_eq!(view.tasks[0].title, "Saved task");
            assert!(view.active.is_empty());
        });
    }
}

#[gpui::test]
fn headless_ui_removed_task_tag_stays_explicit_and_paused_round_can_resume(
    cx: &mut TestAppContext,
) {
    for paused in [false, true] {
        let mut manager = ManagerHarness::new(cx, 1040., 780.);
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            let now = view.now;
            let store = view.store.as_mut().unwrap();
            let tag = store
                .create_tag("Custom tag", force_core::TagKind::Focus)
                .unwrap();
            let task = store.create_task("Saved task", tag.id, now - 100).unwrap();
            if paused {
                let session = store.start_task(task.id, now - 100, None).unwrap();
                store.pause(session.id, now - 50).unwrap();
            }
            view.refresh(cx);
            view.select_task(task.id, window, cx);
            view.navigate(Page::Settings, window, cx);
            view.delete_tag(tag.id, window, cx);
        });
        assert!(manager.cx.has_pending_prompt());
        manager.cx.simulate_prompt_answer("删除");
        manager.cx.run_until_parked();
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.navigate(Page::Focus, window, cx)
        });
        manager.cx.run_until_parked();
        if paused {
            manager.click("running-pause");
            manager.view.read_with(&manager.cx, |view, _| {
                assert_eq!(view.active.len(), 1);
                assert!(view.active[0].is_running());
                assert_eq!(view.active[0].tag_name, "Custom tag");
            });
        } else {
            manager.bounds("task-tag-error");
            manager.view.read_with(&manager.cx, |view, cx| {
                assert_eq!(view.selected_tag, 5);
                assert!(!view.task_has_unsaved_changes(cx));
                assert!(!view.can_start_task(cx));
            });
            manager.click("global-new-task");
            assert!(!manager.cx.has_pending_prompt());
        }
    }
}

#[gpui::test]
fn headless_ui_empty_record_search_clears_and_returns_keyboard_focus(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 780., 620.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let now = view.now;
        let session = view
            .store
            .as_mut()
            .unwrap()
            .start("Visible record", 1, now - 100)
            .unwrap();
        view.store
            .as_mut()
            .unwrap()
            .finish(session.id, now - 50)
            .unwrap();
        view.record_search
            .update(cx, |input, cx| input.set_value("No matches", window, cx));
        view.navigate(Page::Records, window, cx);
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.scroll("records-content", -500.);
    manager.click("clear-empty-record-search");
    let focused = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        assert!(view.record_search.read(cx).value().is_empty());
        view.record_search
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
    });
    assert!(focused);
    manager.bounds("record-row-1");
    manager.cx.simulate_input("Visible");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(view.record_search.read(cx).value(), "Visible")
    });
}

#[gpui::test]
fn headless_ui_tag_edit_row_keeps_its_height_through_save_and_cancel(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 780., 620.);
    manager.click("settings-tab");
    let height = manager.bounds("tag-row-1").size.height;
    assert_eq!(height, px(40.));
    manager.click("edit-tag-1");
    assert_eq!(manager.bounds("editing-tag-row-1").size.height, height);
    // TestPlatform drops native frame callbacks, so ScrollAnchor's deferred
    // auto-reveal cannot run here. Exercise real wheel events for geometry.
    manager.scroll("manager-content", -320.);
    for selector in ["cancel-tag-edit", "save-tag"] {
        let bounds = manager.bounds(selector);
        assert!(bounds.top() >= px(68.) && bounds.bottom() <= px(620.));
        assert!(bounds.left() >= px(0.) && bounds.right() <= px(780.));
    }
    manager.click("cancel-tag-edit");
    manager
        .view
        .read_with(&manager.cx, |view, _| assert!(view.editing_tag.is_none()));
    manager.scroll("manager-content", 500.);
    manager.click("edit-tag-1");
    manager.scroll("manager-content", -320.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.tag_input
            .update(cx, |input, cx| input.set_value("Renamed", window, cx));
    });
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
    manager.click("save-tag");
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.editing_tag.is_none());
        assert_eq!(
            view.tags.iter().find(|tag| tag.id == 1).unwrap().name,
            "Renamed"
        );
    });
    assert_eq!(manager.bounds("tag-row-1").size.height, height);
}

#[gpui::test]
fn headless_ui_cat_controls_wait_until_reversed_disclosure_finishes(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    let session = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let session = view
            .store
            .as_mut()
            .unwrap()
            .start("Running", 1, view.now - 60)
            .unwrap();
        view.refresh(cx);
        view.select_session(session.id, window, cx);
        view.hide_manager(window, cx);
        session.id
    });
    let (cat, mut strip) = open_test_strip(&mut manager);
    let face = strip.debug_bounds("strip-face").unwrap().center();
    // Start opening without the helper's automatic 200ms advance.
    strip.simulate_click(face, Modifiers::default());
    strip.simulate_resize(size(
        px(crate::floating_view::EXPANDED_WIDTH),
        px(crate::floating_view::EXPANDED_HEIGHT),
    ));
    strip.update(|window, _| window.refresh());
    strip.run_until_parked();
    let primary = strip.debug_bounds("strip-primary").unwrap().center();
    let finish = strip.debug_bounds("strip-finish").unwrap().center();
    let open = strip.debug_bounds("strip-open").unwrap().center();
    cat.read_with(&strip, |cat, _| {
        assert!(cat.expanded && cat.disclosure_task.is_some())
    });

    // All three controls must ignore real dispatched clicks during opening.
    for position in [open, primary, finish] {
        strip.simulate_click(position, Modifiers::default());
        strip.run_until_parked();
    }
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.compact);
        assert!(
            view.active
                .iter()
                .find(|s| s.id == session)
                .unwrap()
                .is_running()
        );
        assert!(view.completed.is_none());
    });

    // Reverse twice. Reaching the first opening's old deadline must not make
    // controls active before the final opening finishes.
    strip
        .executor()
        .advance_clock(std::time::Duration::from_millis(50));
    strip.run_until_parked();
    strip.simulate_click(face, Modifiers::default());
    strip.run_until_parked();
    cat.read_with(&strip, |cat, _| {
        assert!(!cat.expanded && cat.disclosure_task.is_some())
    });
    for position in [open, primary, finish] {
        strip.simulate_click(position, Modifiers::default());
        strip.run_until_parked();
    }
    strip
        .executor()
        .advance_clock(std::time::Duration::from_millis(50));
    strip.run_until_parked();
    strip.simulate_click(face, Modifiers::default());
    strip.run_until_parked();
    strip
        .executor()
        .advance_clock(std::time::Duration::from_millis(60));
    strip.run_until_parked();
    cat.read_with(&strip, |cat, _| {
        assert!(cat.expanded && cat.disclosure_task.is_some())
    });
    for position in [open, primary, finish] {
        strip.simulate_click(position, Modifiers::default());
        strip.run_until_parked();
    }
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.compact);
        assert!(
            view.active
                .iter()
                .find(|s| s.id == session)
                .unwrap()
                .is_running()
        );
        assert!(view.completed.is_none());
    });

    strip
        .executor()
        .advance_clock(std::time::Duration::from_millis(101));
    strip.run_until_parked();
    // TestPlatform paints the fully open frame as the timer settles.
    cat.read_with(&strip, |cat, _| {
        assert!(cat.expanded && cat.disclosure_task.is_none());
        assert_eq!(cat.drawer_painted_serial, Some(cat.disclosure_serial));
    });
    strip.update(|window, _| window.refresh());
    strip.run_until_parked();
    cat.read_with(&strip, |cat, _| {
        assert!(cat.expanded && cat.disclosure_task.is_none())
    });
    // Once fully open, the same primary control really pauses the timer.
    let primary = strip.debug_bounds("strip-primary").unwrap().center();
    strip.simulate_click(primary, Modifiers::default());
    strip.run_until_parked();
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(
            view.active
                .iter()
                .find(|s| s.id == session)
                .unwrap()
                .is_paused()
        );
    });
    strip.update(|window, _| window.remove_window());
    strip.run_until_parked();
}

#[gpui::test]
fn headless_ui_reversing_cat_disclosure_cancels_old_close_and_ignores_fading_controls(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new(cx, 1040., 780.);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input.update(cx, |input, cx| {
            input.set_value("Prepared draft", window, cx)
        });
        view.hide_manager(window, cx)
    });
    let (view, mut strip) = open_test_strip(&mut manager);
    click_test_strip(&view, &mut strip, "strip-face");
    let primary = strip.debug_bounds("strip-primary").unwrap().center();
    let collapse = strip.debug_bounds("strip-face").unwrap().center();
    let face = strip.debug_bounds("strip-face").unwrap().center();
    strip.simulate_click(collapse, Modifiers::default());
    view.read_with(&strip, |view, _| {
        assert!(!view.expanded && view.drawer_visible)
    });
    strip.simulate_click(primary, Modifiers::default());
    manager
        .view
        .read_with(&manager.cx, |view, _| assert!(view.compact));
    strip.simulate_click(face, Modifiers::default());
    strip
        .executor()
        .advance_clock(std::time::Duration::from_millis(200));
    strip.run_until_parked();
    refresh_test_strip(&view, &mut strip);
    view.read_with(&strip, |view, _| {
        assert!(view.expanded && view.drawer_visible)
    });
    click_test_strip(&view, &mut strip, "strip-face");
    view.read_with(&strip, |view, _| {
        assert!(!view.expanded && !view.drawer_visible)
    });
    manager.view.read_with(&manager.cx, |view, _| {
        assert!(view.active.is_empty() && view.compact)
    });
}

#[gpui::test]
fn headless_ui_update_status_and_download_action_fit_small_settings(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new(cx, 780., 620.);
    manager.click("settings-tab");
    for state in [
        crate::updates::UpdateState::Current,
        crate::updates::UpdateState::Failed,
        crate::updates::UpdateState::Available {
            version: "100.100.100".into(),
            download: Some("https://github.com/yuxino/Force/releases/latest".into()),
        },
    ] {
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            view.update_state = state;
            cx.notify();
        });
        manager.cx.update(|window, _| window.refresh());
        manager.cx.run_until_parked();
        assert_horizontally_inside(
            manager.bounds("settings-update"),
            manager.bounds("manager-content"),
        );
        assert_horizontally_inside(
            manager.bounds("settings-update-status"),
            manager.bounds("manager-content"),
        );
    }
}

#[gpui::test]
fn headless_ui_export_cancel_restores_input_focus_and_keeps_the_record_filter(
    cx: &mut TestAppContext,
) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.record_search.update(cx, |input, cx| {
            input.set_value("Keep", window, cx);
            input.focus(window, cx);
        });
        view.export(window, cx);
    });
    manager.cx.run_until_parked();
    manager.cx.simulate_new_path_selection(|_| None);
    manager.cx.run_until_parked();
    manager.cx.simulate_input(" typing");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.export_busy);
        assert!(matches!(view.page, Page::Records));
        assert_eq!(view.record_search.read(cx).value(), "Keep typing");
    });
}
