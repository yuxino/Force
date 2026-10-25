#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod activity_view;
mod agenda_model;
mod agenda_view;
mod control_style;
mod focus_view;
mod label;
mod model;
mod platform;
mod records_view;
mod settings_view;
mod tag_picker;
mod view;

#[cfg(all(test, feature = "ui-test-support"))]
mod agenda_ui_tests;
#[cfg(all(test, feature = "ui-test-support"))]
mod headless_ui_tests;

use gpui::{prelude::*, *};
use gpui_component::{Root, Theme, ThemeMode};
use model::{FloatingStrip, ForceView};
use std::{borrow::Cow, path::PathBuf};

actions!(
    force,
    [
        Quit,
        NewTask,
        ToggleCompact,
        PrimaryAction,
        FinishSegment,
        StartBreak,
        ShowRecords,
        ShowSettings,
        ClosePanel
    ]
);

struct ManagerWindow(WindowHandle<Root>);

impl Global for ManagerWindow {}

struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path == "force-logo.svg" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/logo.svg"
            ))));
        }
        let custom: Option<&'static [u8]> = match path {
            "force-wordmark.svg" => Some(include_bytes!("../../../assets/wordmark.svg")),
            "force-icons/trash.svg" => Some(include_bytes!("../../../assets/icons/trash.svg")),
            "force-icons/pencil.svg" => Some(include_bytes!("../../../assets/icons/pencil.svg")),
            "force-icons/pin.svg" => Some(include_bytes!("../../../assets/icons/pin.svg")),
            "force-icons/play.svg" => Some(include_bytes!("../../../assets/icons/play.svg")),
            "force-icons/pause.svg" => Some(include_bytes!("../../../assets/icons/pause.svg")),
            "force-icons/stop.svg" => Some(include_bytes!("../../../assets/icons/stop.svg")),
            _ => None,
        };
        if let Some(bytes) = custom {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}

fn data_directory() -> anyhow::Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "--data-dir" {
            let path = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--data-dir requires a path"))?;
            anyhow::ensure!(args.next().is_none(), "Unexpected argument");
            return Ok(PathBuf::from(path));
        }
        anyhow::bail!("Usage: force [--data-dir PATH] or force --version");
    }
    let executable = std::env::current_exe().ok();
    let is_preview = executable.as_ref().is_some_and(|path| {
        path.ancestors().any(|part| {
            part.file_name()
                .is_some_and(|name| name == "Force Preview.app")
        })
    });
    let is_dev = cfg!(debug_assertions)
        || executable.as_ref().is_some_and(|path| {
            path.ancestors()
                .any(|part| part.file_name().is_some_and(|name| name == "Force Dev.app"))
        });
    let name = if is_preview {
        "Force Preview"
    } else if is_dev {
        "Force Dev"
    } else {
        "Force"
    };
    let dirs = directories::ProjectDirs::from("app", "yuxino", name)
        .ok_or_else(|| anyhow::anyhow!("Local data directory is unavailable"))?;
    Ok(dirs.data_local_dir().to_path_buf())
}

fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("Force {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    platform::prepare_application()?;
    let data_dir = data_directory()?;
    let application = Application::new().with_assets(Assets);
    application.on_reopen(|cx| {
        if let Some(handle) = cx.try_global::<ManagerWindow>().map(|manager| manager.0) {
            with_force(handle, cx, |this, window, cx| this.show_manager(window, cx));
        }
    });
    application.run(move |cx: &mut App| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        let theme = Theme::global_mut(cx);
        theme.primary = rgb(view::INK).into();
        theme.primary_hover = rgb(0x414141).into();
        theme.primary_active = rgb(0x111111).into();
        theme.primary_foreground = rgb(0xffffff).into();
        theme.ring = rgb(0x9a9a9a).into();
        theme.caret = rgb(view::INK).into();
        theme.background = rgba(0x00000000).into();
        theme.popover = rgb(view::PAPER).into();
        theme.popover_foreground = rgb(view::INK).into();
        theme.foreground = rgb(view::INK).into();
        theme.muted_foreground = rgb(view::MUTED).into();
        theme.border = rgb(view::LINE).into();
        theme.input = rgb(view::LINE).into();
        theme.secondary = rgb(view::SURFACE).into();
        theme.secondary_hover = rgb(0xe9e9e9).into();
        theme.secondary_active = rgb(0xe0e0e0).into();
        theme.accent = rgb(0xededed).into();
        theme.accent_foreground = rgb(view::INK).into();
        theme.selection = rgb(0xdcdcdc).into();
        theme.font_size = px(13.);
        theme.radius = px(6.);
        theme.radius_lg = px(8.);
        theme.shadow = false;
        if cfg!(target_os = "windows") {
            theme.font_family = "Segoe UI".into();
        }
        if cfg!(target_os = "linux") {
            theme.font_family = "Noto Sans".into();
        }
        cx.bind_keys([
            KeyBinding::new("secondary-q", Quit, None),
            KeyBinding::new("secondary-n", NewTask, None),
            KeyBinding::new("secondary-p", PrimaryAction, None),
            KeyBinding::new("secondary-shift-p", FinishSegment, None),
            KeyBinding::new("secondary-,", ShowSettings, None),
            KeyBinding::new("escape", ClosePanel, Some("Force")),
            KeyBinding::new("secondary-shift-m", ToggleCompact, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus(vec![
            Menu {
                name: "Force".into(),
                items: vec![
                    MenuItem::action("设置…", ShowSettings),
                    MenuItem::separator(),
                    MenuItem::action("退出 Force", Quit),
                ],
            },
            Menu {
                name: "计时".into(),
                items: vec![
                    MenuItem::action("新建…", NewTask),
                    MenuItem::separator(),
                    MenuItem::action("开始 / 暂停 / 继续", PrimaryAction),
                    MenuItem::action("结束本次计时", FinishSegment),
                    MenuItem::action("休息 5 分钟", StartBreak),
                ],
            },
            Menu {
                name: "显示".into(),
                items: vec![
                    MenuItem::action("显示 / 隐藏主窗口", ToggleCompact),
                    MenuItem::action("计时记录", ShowRecords),
                ],
            },
        ]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let strip_bounds = platform::restored_bounds(&data_dir, cx).unwrap_or_else(|| {
            Bounds::centered(
                None,
                size(px(platform::STRIP_WIDTH), px(platform::STRIP_HEIGHT)),
                cx,
            )
        });
        // Create the one model and every input inside the normal manager
        // window. The timer window only observes and dispatches to it.
        let manager = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(platform::MANAGER_WIDTH), px(platform::MANAGER_HEIGHT)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Force".into()),
                    ..Default::default()
                }),
                kind: WindowKind::Normal,
                focus: false,
                show: false,
                is_resizable: true,
                window_background: WindowBackgroundAppearance::Opaque,
                window_min_size: Some(size(
                    px(platform::MANAGER_MIN_WIDTH),
                    px(platform::MANAGER_MIN_HEIGHT),
                )),
                app_id: Some("app.yuxino.force.manager".into()),
                ..Default::default()
            },
            move |window, cx| {
                let model = cx.new(|cx| ForceView::new(data_dir, window, cx));
                let close_model = model.clone();
                window.on_window_should_close(cx, move |window, cx| {
                    close_model.update(cx, |model, cx| model.hide_manager(window, cx));
                    false
                });
                cx.new(|cx| Root::new(model, window, cx))
            },
        );
        let Ok(handle) = manager else {
            eprintln!("Force could not open its management window.");
            cx.quit();
            return;
        };
        let Some(model) = handle
            .update(cx, |root, _, _| root.view().clone().downcast::<ForceView>())
            .ok()
            .and_then(Result::ok)
        else {
            cx.quit();
            return;
        };
        cx.set_global(ManagerWindow(handle));
        model.update(cx, |model, _| model.manager_window = Some(handle));
        let strip_model = model.clone();
        let strip = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(strip_bounds)),
                titlebar: None,
                kind: WindowKind::PopUp,
                focus: false,
                is_resizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                window_decorations: Some(WindowDecorations::Client),
                window_min_size: Some(size(px(platform::STRIP_WIDTH), px(platform::STRIP_HEIGHT))),
                app_id: Some("app.yuxino.force".into()),
                ..Default::default()
            },
            move |window, cx| {
                if platform::configure_floating(window).is_err() {
                    eprintln!("Force could not configure the floating timer.");
                }
                let strip = cx.new(|cx| FloatingStrip::new(strip_model, handle, window, cx));
                cx.new(|cx| Root::new(strip, window, cx))
            },
        );
        match strip {
            Ok(strip) => {
                model.update(cx, |model, _| model.strip_window = Some(strip));
                cx.on_action(move |_: &NewTask, cx| {
                    with_force(handle, cx, |this, window, cx| {
                        if this.page == model::Page::Agenda {
                            this.begin_plan(None, window, cx);
                        } else {
                            this.new_task(window, cx);
                        }
                    })
                });
                cx.on_action(move |_: &ToggleCompact, cx| {
                    with_force(handle, cx, |this, window, cx| this.compact(window, cx))
                });
                cx.on_action(move |_: &PrimaryAction, cx| {
                    with_force(handle, cx, |this, window, cx| {
                        this.strip_primary_action(window, cx)
                    })
                });
                cx.on_action(move |_: &FinishSegment, cx| {
                    with_force(handle, cx, |this, _, cx| {
                        if this.completed.is_some()
                            && !this.active.iter().any(|session| session.is_running())
                        {
                            return;
                        }
                        if let Some(id) = this.strip_session().map(|session| session.id) {
                            this.finish(id, cx);
                        }
                    })
                });
                cx.on_action(move |_: &StartBreak, cx| {
                    with_force(handle, cx, |this, window, cx| {
                        this.start_global_break(window, cx)
                    })
                });
                cx.on_action(move |_: &ShowRecords, cx| {
                    with_force(handle, cx, |this, window, cx| {
                        this.navigate(model::Page::Records, window, cx);
                    })
                });
                cx.on_action(move |_: &ShowSettings, cx| {
                    with_force(handle, cx, |this, window, cx| {
                        this.navigate(model::Page::Settings, window, cx);
                    })
                });
                cx.on_action(move |_: &ClosePanel, cx| {
                    with_force(handle, cx, |this, window, cx| {
                        if !this.compact && this.page == model::Page::Agenda && this.agenda.editing
                        {
                            this.cancel_plan_edit(window, cx);
                        } else if !this.compact && this.editing_duration.is_some() {
                            this.cancel_duration_edit(window, cx);
                        } else if !this.compact
                            && this.page == model::Page::Settings
                            && this.editing_tag.is_some()
                        {
                            this.cancel_tag_edit(window, cx);
                        } else if !this.compact
                            && this.page == model::Page::Records
                            && this.record_details.is_some()
                        {
                            this.record_details = None;
                            cx.notify();
                        } else if !this.compact
                            && this.page == model::Page::Records
                            && this.record_help_open
                        {
                            this.record_help_open = false;
                            cx.notify();
                        } else if !this.compact {
                            this.compact(window, cx);
                        }
                    })
                });
                with_force(handle, cx, |this, window, cx| this.show_manager(window, cx));
            }
            Err(_) => {
                eprintln!("Force could not open a graphics window.");
                cx.quit();
            }
        }
    });
    Ok(())
}

fn with_force(
    handle: WindowHandle<Root>,
    cx: &mut App,
    operation: impl FnOnce(&mut ForceView, &mut Window, &mut Context<ForceView>) + 'static,
) {
    // Keyboard actions run while GPUI is borrowing the dispatching window.
    // Wait until that borrow ends before accessing it through its handle.
    cx.defer(move |cx| {
        let _ = handle.update(cx, |root, window, cx| {
            if let Ok(view) = root.view().clone().downcast::<ForceView>() {
                view.update(cx, |this, cx| operation(this, window, cx));
            }
        });
    });
}
