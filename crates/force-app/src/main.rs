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

actions!(force, [Quit, NewTask, ToggleCompact]);

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
    let is_dev = cfg!(debug_assertions)
        || std::env::current_exe().ok().is_some_and(|path| {
            path.ancestors()
                .any(|part| part.file_name().is_some_and(|name| name == "Force Dev.app"))
        });
    let name = if is_dev { "Force Dev" } else { "Force" };
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
            theme.background = rgb(0xfafaf8).into();
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
                KeyBinding::new("secondary-n", NewTask, Some("Force")),
                KeyBinding::new("secondary-shift-m", ToggleCompact, Some("Force")),
            ]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.set_menus(vec![Menu {
                name: "Force".into(),
                items: vec![
                    MenuItem::action("新任务", NewTask),
                    MenuItem::action("切换小窗", ToggleCompact),
                    MenuItem::separator(),
                    MenuItem::action("退出 Force", Quit),
                ],
            }]);
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(960.), px(740.)), cx);
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Force".into()),
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(360.), px(110.))),
                    app_id: Some("app.yuxino.force".into()),
                    ..Default::default()
                },
                move |window, cx| {
                    let view = cx.new(|cx| ForceView::new(data_dir, window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
            if result.is_err() {
                eprintln!("Force could not open a graphics window.");
                cx.quit();
            }
            cx.activate(true);
        });
    Ok(())
}
