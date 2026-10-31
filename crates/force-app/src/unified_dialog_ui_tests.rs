//! Shared creation dialogs use disposable data. Bounds and event dispatch do
//! not establish native pixels, IME composition or other-platform interaction.
use chrono::Local;
use gpui::{Bounds, KeyBinding, Modifiers, Pixels, Point, TestAppContext, px, size};

use crate::{headless_ui_tests::ManagerHarness, model::Page};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Editor {
    Task,
    Moment,
    Arrangement,
}

fn settle(manager: &mut ManagerHarness) {
    manager.cx.update(|window, _| window.refresh());
    manager.cx.run_until_parked();
}

fn bind_actions(manager: &mut ManagerHarness) {
    let handle = manager.handle;
    manager.cx.update(|_, cx| {
        cx.bind_keys([
            KeyBinding::new("escape", crate::ClosePanel, Some("Force")),
            KeyBinding::new("secondary-n", crate::NewTask, None),
        ]);
        cx.on_action(move |_: &crate::ClosePanel, cx| {
            crate::with_force(handle, cx, |view, window, cx| view.close_panel(window, cx));
        });
        cx.on_action(move |_: &crate::NewTask, cx| {
            crate::with_force(handle, cx, |view, window, cx| {
                view.open_task_dialog(window, cx)
            });
        });
    });
}

fn open(manager: &mut ManagerHarness, editor: Editor) {
    match editor {
        Editor::Task => manager.click("global-new-task"),
        Editor::Moment => manager.click("global-new-moment"),
        Editor::Arrangement => {
            manager.click("agenda-tab");
            manager.click("agenda-overview-add");
        }
    }
    manager.view.read_with(&manager.cx, |view, _| {
        assert_eq!(view.task_editor.visible, editor == Editor::Task);
        assert_eq!(view.moments.visible, editor == Editor::Moment);
        assert_eq!(view.agenda.editor_visible, editor == Editor::Arrangement);
    });
}

fn assert_inside(child: Bounds<Pixels>, parent: Bounds<Pixels>) {
    let tolerance = px(1.);
    assert!(
        child.left() >= parent.left() - tolerance,
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.right() <= parent.right() + tolerance,
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.top() >= parent.top() - tolerance,
        "{child:?} outside {parent:?}"
    );
    assert!(
        child.bottom() <= parent.bottom() + tolerance,
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
        assert!(
            (actual - expected).abs() <= px(1.),
            "{actual:?} != {expected:?}"
        );
    }
}

#[gpui::test]
fn creation_dialogs_share_chrome_without_forcing_equal_heights(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        let viewport = Bounds::new(Point::default(), size(px(width), px(height)));
        let month = manager.bounds("agenda-overview-grid");
        let mut reference = None;
        for (editor, panel, fields, footer, cancel, save) in [
            (
                Editor::Arrangement,
                "agenda-popover",
                "agenda-editor-fields",
                "agenda-editor-actions",
                "agenda-cancel",
                "agenda-save",
            ),
            (
                Editor::Moment,
                "moment-editor",
                "moment-editor-fields",
                "moment-editor-actions",
                "cancel-moment",
                "save-moment",
            ),
            (
                Editor::Task,
                "task-editor",
                "task-editor-fields",
                "task-editor-actions",
                "cancel-task-dialog",
                "save-task-dialog",
            ),
        ] {
            open(&mut manager, editor);
            let panel = manager.bounds(panel);
            let heading = manager.bounds("editor-dialog-heading");
            let footer = manager.bounds(footer);
            let cancel = manager.bounds(cancel);
            let save = manager.bounds(save);
            for bounds in [panel, heading, manager.bounds(fields), footer, cancel, save] {
                assert_inside(bounds, viewport);
                if bounds != panel {
                    assert_inside(bounds, panel);
                }
            }
            assert!(panel.size.width >= px(500.));
            assert!((panel.center().x - viewport.center().x).abs() <= px(1.));
            assert!(panel.top() > manager.bounds("manager-header").bottom());
            assert!(cancel.right() <= save.left() + px(1.));
            assert!((cancel.top() - save.top()).abs() <= px(1.));
            let trailing = if editor == Editor::Task {
                let start = manager.bounds("start-task-dialog");
                assert_inside(start, panel);
                assert!(save.right() + px(6.) <= start.left());
                assert!((save.top() - start.top()).abs() <= px(1.));
                start
            } else {
                save
            };
            assert!((trailing.right() - footer.right()).abs() <= px(1.));
            let chrome = [
                panel.left(),
                panel.size.width,
                heading.left() - panel.left(),
                heading.top() - panel.top(),
                heading.size.width,
                heading.size.height,
                footer.size.width,
                footer.size.height,
                panel.bottom() - footer.bottom(),
            ];
            if let Some(expected) = reference {
                for (actual, expected) in chrome.into_iter().zip(expected) {
                    assert!((actual - expected).abs() <= px(1.));
                }
            } else {
                reference = Some(chrome);
            }
            manager.click(match editor {
                Editor::Arrangement => "agenda-cancel",
                Editor::Moment => "cancel-moment",
                Editor::Task => "cancel-task-dialog",
            });
            assert_same_bounds(manager.bounds("agenda-overview-grid"), month);
        }
    }
}

#[gpui::test]
fn task_dialog_fits_content_and_keeps_actions_visible_with_errors(cx: &mut TestAppContext) {
    for (width, height) in [(1040., 780.), (780., 620.)] {
        let mut manager = ManagerHarness::new_on_page(cx, width, height, Page::Agenda);
        open(&mut manager, Editor::Task);
        let count_up = manager.bounds("task-editor");
        assert!(count_up.size.height < px(420.), "{count_up:?}");
        let fields = manager.bounds("task-editor-fields");
        let mode = manager.bounds("task-editor-mode");
        assert_inside(mode, fields);
        assert!(fields.bottom() - mode.bottom() <= px(2.));
        manager.click("task-dialog-duration-custom");
        let countdown = manager.bounds("task-editor");
        assert!(countdown.size.height > count_up.size.height + px(50.));
        assert_inside(
            manager.bounds("task-editor-duration"),
            manager.bounds("task-editor-fields"),
        );
        manager.click("task-dialog-duration-45");
        manager.click("task-dialog-duration-0");
        assert_same_bounds(manager.bounds("task-editor"), count_up);
        manager.click("task-dialog-duration-custom");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.task_editor.selected_tag = 2;
            view.task_editor
                .timer_input
                .update(cx, |input, cx| input.set_value("0", window, cx));
            view.task_editor.error =
                Some("Could not save.\nTry again.\nThe task draft is preserved.".into());
        });
        settle(&mut manager);
        let panel = manager.bounds("task-editor");
        let fields = manager.bounds("task-editor-fields");
        let error = manager.bounds("task-editor-duration-error");
        assert_inside(error, fields);
        assert_inside(manager.bounds("task-editor-validation"), panel);
        let footer = manager.bounds("task-editor-actions");
        assert_inside(footer, panel);
        assert!(fields.bottom() <= footer.top());
        assert!(panel.top() >= px(68. + 24.));
        assert!(panel.bottom() <= px(height - 24.));
        manager.click("start-task-dialog");
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.task_editor.visible);
            assert!(view.tasks.is_empty() && view.active.is_empty());
        });
        manager.click("save-task-dialog");
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(!view.task_editor.visible);
            assert_eq!(view.tasks.len(), 1);
            assert!(view.active.is_empty());
        });
    }
}

fn fill_draft(manager: &mut ManagerHarness, editor: Editor) {
    manager
        .view
        .update_in(&mut manager.cx, |view, window, cx| match editor {
            Editor::Task => {
                view.task_editor.selected_tag = 2;
                view.task_editor.countdown = true;
                view.task_editor.title_input.update(cx, |input, cx| {
                    input.set_value("Keep task draft", window, cx)
                });
                view.task_editor
                    .timer_input
                    .update(cx, |input, cx| input.set_value("0", window, cx));
            }
            Editor::Moment => {
                view.moments.content.update(cx, |input, cx| {
                    input.set_value("Keep note draft\nSecond line", window, cx)
                });
                view.moments
                    .date
                    .update(cx, |input, cx| input.set_value("2026-02-30", window, cx));
                view.moments
                    .time
                    .update(cx, |input, cx| input.set_value("09:15", window, cx));
            }
            Editor::Arrangement => {
                view.agenda.title_input.update(cx, |input, cx| {
                    input.set_value("Keep arrangement draft", window, cx)
                });
                view.agenda
                    .date_input
                    .update(cx, |input, cx| input.set_value("2026-11-05", window, cx));
                view.agenda.note_input.update(cx, |input, cx| {
                    input.set_value("Keep arrangement note", window, cx)
                });
            }
        });
    settle(manager);
}

fn assert_all_drafts(manager: &ManagerHarness) {
    manager.view.read_with(&manager.cx, |view, cx| {
        assert_eq!(
            view.task_editor.title_input.read(cx).value(),
            "Keep task draft"
        );
        assert_eq!(view.task_editor.selected_tag, 2);
        assert!(view.task_editor.countdown);
        assert_eq!(view.task_editor.timer_input.read(cx).value(), "0");
        assert_eq!(
            view.moments.content.read(cx).value(),
            "Keep note draft\nSecond line"
        );
        assert_eq!(view.moments.date.read(cx).value(), "2026-02-30");
        assert_eq!(view.moments.time.read(cx).value(), "09:15");
        assert_eq!(
            view.agenda.title_input.read(cx).value(),
            "Keep arrangement draft"
        );
        assert_eq!(view.agenda.date_input.read(cx).value(), "2026-11-05");
        assert_eq!(
            view.agenda.note_input.read(cx).value(),
            "Keep arrangement note"
        );
        assert!(view.tasks.is_empty() && view.active.is_empty() && view.history.is_empty());
        assert!(view.record_moments.is_empty() && view.agenda.plans.is_empty());
    });
}

#[gpui::test]
fn creation_dialog_switches_preserve_all_three_independent_drafts(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    for (editor, close) in [
        (Editor::Arrangement, "agenda-popover-close"),
        (Editor::Moment, "close-moment"),
        (Editor::Task, "close-task-dialog"),
    ] {
        open(&mut manager, editor);
        fill_draft(&mut manager, editor);
        manager.click(close);
    }
    for source in [Editor::Task, Editor::Moment, Editor::Arrangement] {
        for destination in [Editor::Task, Editor::Moment, Editor::Arrangement] {
            if source == destination {
                continue;
            }
            open(&mut manager, source);
            open(&mut manager, destination);
            assert!(!manager.cx.has_pending_prompt());
            assert_all_drafts(&manager);
            open(&mut manager, source);
            assert_all_drafts(&manager);
        }
    }
}

#[gpui::test]
fn new_task_close_outside_escape_and_cancel_have_consistent_draft_rules(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Records);
    bind_actions(&mut manager);
    open(&mut manager, Editor::Task);
    fill_draft(&mut manager, Editor::Task);
    manager.click("close-task-dialog");
    assert_eq!(manager.page(), Page::Records);
    manager.cx.simulate_keystrokes("secondary-n");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.task_editor.visible);
        assert_eq!(
            view.task_editor.title_input.read(cx).value(),
            "Keep task draft"
        );
    });
    manager
        .cx
        .simulate_click(Point::new(px(12.), px(600.)), Modifiers::default());
    settle(&mut manager);
    manager
        .view
        .read_with(&manager.cx, |view, _| assert!(!view.task_editor.visible));
    open(&mut manager, Editor::Task);
    manager.cx.simulate_keystrokes("escape");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.task_editor.visible);
        assert_eq!(view.task_editor.timer_input.read(cx).value(), "0");
        assert!(view.task_editor.countdown);
        assert_eq!(view.task_editor.selected_tag, 2);
    });
    open(&mut manager, Editor::Task);
    manager.click("cancel-task-dialog");
    open(&mut manager, Editor::Task);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.task_editor.title_input.read(cx).value().is_empty());
        assert_eq!(view.task_editor.selected_tag, 0);
        assert!(!view.task_editor.countdown);
        assert_eq!(view.task_editor.timer_input.read(cx).value(), "25");
        assert!(view.tasks.is_empty() && view.active.is_empty() && view.agenda.plans.is_empty());
    });
    assert_eq!(manager.page(), Page::Records);
}

#[gpui::test]
fn task_dialog_keyboard_and_activity_menu_do_not_operate_the_background(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    bind_actions(&mut manager);
    open(&mut manager, Editor::Task);
    for key in ["tab", "shift-tab"] {
        for _ in 0..12 {
            manager.cx.simulate_keystrokes(key);
            settle(&mut manager);
            manager.view.update_in(&mut manager.cx, |view, window, cx| {
                assert!(view.task_editor.focus.contains_focused(window, cx));
                assert!(view.task_input.read(cx).value().is_empty());
                assert!(view.tasks.is_empty() && view.active.is_empty());
                assert_eq!(view.page, Page::Agenda);
            });
        }
    }
    manager.click("focus-tag-picker");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.task_editor.tag_picker.read(cx).is_open())
    });
    manager.cx.simulate_keystrokes("escape");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.task_editor.tag_picker.read(cx).is_open());
        assert!(view.task_editor.visible);
    });
    manager.click("focus-tag-picker");
    manager.click("focus-tag-option-2");
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_editor.title_input.update(cx, |input, cx| {
            input.set_value("Save on Return", window, cx);
            input.focus(window, cx);
        });
    });
    settle(&mut manager);
    manager.cx.simulate_keystrokes("enter");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.task_editor.visible);
        assert_eq!(view.page, Page::Focus);
        assert_eq!(view.tasks.len(), 1);
        assert_eq!(view.tasks[0].title, "Save on Return");
        assert_eq!(view.tasks[0].tag_id, 2);
        assert_eq!(view.selected_task, Some(view.tasks[0].id));
        assert!(view.active.is_empty() && view.agenda.plans.is_empty());
        assert!(view.agenda.date == Local::now().date_naive());
        assert!(!view.task_has_unsaved_changes(cx));
    });
}

#[gpui::test]
fn task_dialog_save_cancelled_at_dirty_workspace_prompt_creates_nothing(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let original = manager.view.update_in(&mut manager.cx, |view, window, cx| {
        let task = view
            .store
            .as_mut()
            .unwrap()
            .create_task("Existing task", 1, view.now)
            .unwrap();
        view.refresh(cx);
        view.select_task(task.id, window, cx);
        view.task_input.update(cx, |input, cx| {
            input.set_value("Keep existing task edits", window, cx)
        });
        view.navigate(Page::Records, window, cx);
        task
    });
    settle(&mut manager);
    open(&mut manager, Editor::Task);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_editor.selected_tag = 2;
        view.task_editor.title_input.update(cx, |input, cx| {
            input.set_value("Independent new task", window, cx)
        });
    });
    settle(&mut manager);
    manager.click("save-task-dialog");
    assert!(manager.cx.has_pending_prompt());
    manager
        .view
        .read_with(&manager.cx, |view, _| assert_eq!(view.tasks.len(), 1));
    manager.cx.simulate_prompt_answer("取消");
    settle(&mut manager);
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(view.task_editor.visible);
        assert_eq!(view.page, Page::Records);
        assert_eq!(
            view.task_editor.title_input.read(cx).value(),
            "Independent new task"
        );
        assert_eq!(view.task_input.read(cx).value(), "Keep existing task edits");
        assert_eq!(view.selected_task, Some(original.id));
        assert_eq!(view.tasks.len(), 1);
        assert_eq!(view.tasks[0].title, original.title);
        assert!(view.active.is_empty() && view.agenda.plans.is_empty());
    });
}

#[gpui::test]
fn task_dialog_save_keeps_invalid_timer_draft_and_current_round_running(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    let running = manager.view.update_in(&mut manager.cx, |view, _, cx| {
        let session = view
            .store
            .as_mut()
            .unwrap()
            .create_and_start_task("Current round", 1, view.now - 60, Some(1500))
            .unwrap();
        view.refresh(cx);
        session
    });
    settle(&mut manager);
    open(&mut manager, Editor::Task);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_editor.selected_tag = 2;
        view.task_editor.countdown = true;
        view.task_editor.title_input.update(cx, |input, cx| {
            input.set_value("Save for later", window, cx)
        });
        view.task_editor
            .timer_input
            .update(cx, |input, cx| input.set_value("0", window, cx));
    });
    settle(&mut manager);
    manager.click("save-task-dialog");
    manager.view.read_with(&manager.cx, |view, cx| {
        assert!(!view.task_editor.visible);
        assert_eq!(view.page, Page::Focus);
        let saved = view
            .tasks
            .iter()
            .find(|task| task.title == "Save for later")
            .unwrap();
        assert_eq!(view.selected_task, Some(saved.id));
        assert_eq!(saved.total_seconds, 0);
        assert_eq!(view.timer_input.read(cx).value(), "0");
        assert!(view.countdown);
        assert!(!view.can_start_task(cx));
        assert!(view.timer_duration_error(cx).is_some());
        assert_eq!(view.active, vec![running.clone()]);
        assert_eq!(
            view.store.as_ref().unwrap().active_sessions().unwrap(),
            vec![running]
        );
        assert!(view.agenda.plans.is_empty());
    });
}

#[gpui::test]
fn creation_dialog_tab_cycle_stays_inside_the_active_editor(cx: &mut TestAppContext) {
    let mut manager = ManagerHarness::new_on_page(cx, 780., 620., Page::Agenda);
    manager.view.update_in(&mut manager.cx, |view, window, cx| {
        view.task_input.update(cx, |input, cx| {
            input.set_value("Keep the background field", window, cx)
        });
    });
    settle(&mut manager);
    for editor in [Editor::Task, Editor::Moment, Editor::Arrangement] {
        open(&mut manager, editor);
        for key in ["tab", "shift-tab"] {
            for _ in 0..12 {
                manager.cx.simulate_keystrokes(key);
                settle(&mut manager);
                manager.view.update_in(&mut manager.cx, |view, window, cx| {
                    let focus = match editor {
                        Editor::Task => &view.task_editor.focus,
                        Editor::Moment => &view.moments.focus,
                        Editor::Arrangement => &view.agenda.editor_focus,
                    };
                    assert!(
                        focus.contains_focused(window, cx),
                        "{editor:?} focus escaped after {key}"
                    );
                    assert_eq!(
                        view.task_input.read(cx).value(),
                        "Keep the background field"
                    );
                    assert!(
                        view.tasks.is_empty()
                            && view.active.is_empty()
                            && view.agenda.plans.is_empty()
                    );
                });
            }
        }
    }
}
