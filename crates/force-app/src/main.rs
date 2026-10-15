#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod model;
mod platform;
mod view;

use gpui::{prelude::*, *};
use gpui_component::{Root, Theme, ThemeMode};
use model::ForceView;
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

struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path == "force-logo.svg" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/logo.svg"
            ))));
        }
        let custom: Option<&'static [u8]> = match path {
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
    let data_dir = data_directory()?;
    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            let theme = Theme::global_mut(cx);
            theme.primary = rgb(0x252525).into();
            theme.primary_hover = rgb(0x414141).into();
            theme.primary_active = rgb(0x111111).into();
            theme.primary_foreground = rgb(0xffffff).into();
            theme.ring = rgb(0x999995).into();
            theme.caret = rgb(0x252525).into();
            theme.background = rgba(0x00000000).into();
            theme.popover = rgb(0xfdfdfc).into();
            theme.popover_foreground = rgb(0x202020).into();
            theme.foreground = rgb(0x202020).into();
            theme.font_size = px(14.);
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
                        MenuItem::action("新任务", NewTask),
                        MenuItem::separator(),
                        MenuItem::action("开始 / 暂停 / 继续", PrimaryAction),
                        MenuItem::action("结束本段", FinishSegment),
                        MenuItem::action("休息 5 分钟", StartBreak),
                    ],
                },
                Menu {
                    name: "显示".into(),
                    items: vec![
                        MenuItem::action("展开 / 收起", ToggleCompact),
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
            let bounds = platform::restored_bounds(&data_dir, cx)
                .unwrap_or_else(|| Bounds::centered(None, size(px(352.), px(76.)), cx));
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    kind: WindowKind::PopUp,
                    focus: false,
                    is_resizable: false,
                    window_background: WindowBackgroundAppearance::Transparent,
                    window_decorations: Some(WindowDecorations::Client),
                    window_min_size: Some(size(px(352.), px(76.))),
                    app_id: Some("app.yuxino.force".into()),
                    ..Default::default()
                },
                move |window, cx| {
                    if platform::configure_floating(window).is_err() {
                        eprintln!("Force could not configure the floating window.");
                    }
                    let view = cx.new(|cx| ForceView::new(data_dir, window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
            match result {
                Ok(handle) => {
                    cx.on_action(move |_: &NewTask, cx| {
                        with_force(handle, cx, |this, window, cx| this.new_task(window, cx))
                    });
                    cx.on_action(move |_: &ToggleCompact, cx| {
                        with_force(handle, cx, |this, window, cx| this.compact(window, cx))
                    });
                    cx.on_action(move |_: &PrimaryAction, cx| {
                        with_force(handle, cx, |this, window, cx| {
                            this.primary_action(window, cx)
                        })
                    });
                    cx.on_action(move |_: &FinishSegment, cx| {
                        with_force(handle, cx, |this, _, cx| {
                            if let Some(id) = this.selected_session {
                                this.finish(id, cx);
                            }
                        })
                    });
                    cx.on_action(move |_: &StartBreak, cx| {
                        with_force(handle, cx, |this, window, cx| this.start_break(window, cx))
                    });
                    cx.on_action(move |_: &ShowRecords, cx| {
                        with_force(handle, cx, |this, window, cx| {
                            if this.compact {
                                this.compact(window, cx);
                            }
                            this.navigate(model::Page::Records, cx);
                        })
                    });
                    cx.on_action(move |_: &ShowSettings, cx| {
                        with_force(handle, cx, |this, window, cx| {
                            if this.compact {
                                this.compact(window, cx);
                            }
                            this.navigate(model::Page::Settings, cx);
                        })
                    });
                    cx.on_action(move |_: &ClosePanel, cx| {
                        with_force(handle, cx, |this, window, cx| {
                            if !this.compact {
                                this.compact(window, cx);
                            }
                        })
                    });
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
    operation: impl FnOnce(&mut ForceView, &mut Window, &mut Context<ForceView>),
) {
    let _ = handle.update(cx, |root, window, cx| {
        if let Ok(view) = root.view().clone().downcast::<ForceView>() {
            view.update(cx, |this, cx| operation(this, window, cx));
        }
    });
}
