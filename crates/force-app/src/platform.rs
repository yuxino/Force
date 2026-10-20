pub(crate) const STRIP_WIDTH: f32 = 328.;
pub(crate) const STRIP_HEIGHT: f32 = 68.;
pub(crate) const MANAGER_WIDTH: f32 = 1040.;
pub(crate) const MANAGER_HEIGHT: f32 = 780.;
pub(crate) const MANAGER_MIN_WIDTH: f32 = 780.;
pub(crate) const MANAGER_MIN_HEIGHT: f32 = 620.;

#[cfg(target_os = "macos")]
pub(crate) fn prepare_application() -> anyhow::Result<()> {
    use anyhow::{Context as _, ensure};
    use objc2::{
        MainThreadMarker, msg_send,
        runtime::{AnyClass, AnyObject},
    };

    let _main_thread = MainThreadMarker::new()
        .context("AppKit initialization requires the application main thread")?;
    // GPUI 0.2.2 queries TIS while constructing Application, before creating
    // NSApplication. That order disables marked-text composition in nonactivating
    // panels. Initialize its registered subclass first; creating NSApplication
    // directly would leave GPUI with the wrong singleton and missing ivars.
    let class = AnyClass::get(c"GPUIApplication")
        .context("GPUI's native application class is unavailable")?;
    // GPUI registers this NSApplication subclass before main. sharedApplication
    // returns the process-owned singleton; no ownership is transferred here.
    let application: *mut AnyObject = unsafe { msg_send![class, sharedApplication] };
    ensure!(
        !application.is_null(),
        "Could not initialize the native application"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn set_always_on_top(window: &gpui::Window, enabled: bool) -> anyhow::Result<()> {
    use anyhow::{Context as _, bail};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSFloatingWindowLevel, NSNormalWindowLevel, NSView};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let _main_thread = MainThreadMarker::new()
        .context("Changing the window level requires the application main thread.")?;
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("The native window handle is unavailable: {error:?}"))?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        bail!("Unsupported native window handle for macOS.");
    };

    // GPUI owns this NSView. The borrowed handle keeps it valid for this call,
    // and the main-thread check above permits AppKit access.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let native_window = view
        .window()
        .context("The native view is not attached to a window.")?;
    native_window.setLevel(if enabled {
        NSFloatingWindowLevel
    } else {
        NSNormalWindowLevel
    });
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn set_always_on_top(window: &gpui::Window, enabled: bool) -> anyhow::Result<()> {
    use anyhow::{Context as _, bail};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };

    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("The native window handle is unavailable: {error:?}"))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        bail!("Unsupported native window handle for Windows.");
    };

    // GPUI owns the HWND. Only its stacking order changes; its position, size
    // and activation state stay intact.
    let result = unsafe {
        SetWindowPos(
            handle.hwnd.get() as _,
            if enabled {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            },
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        )
    };
    if result == 0 {
        return Err(std::io::Error::last_os_error())
            .context("The operating system could not change the window stacking order.");
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn set_always_on_top(_window: &gpui::Window, _enabled: bool) -> anyhow::Result<()> {
    anyhow::bail!(
        "Unsupported: always-on-top is controlled by the window manager on this platform."
    )
}

#[cfg(target_os = "macos")]
fn native_window(
    window: &gpui::Window,
) -> anyhow::Result<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    use anyhow::{Context as _, bail};
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let _main_thread = objc2::MainThreadMarker::new().context("AppKit requires the main thread")?;
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("Native window is unavailable: {error:?}"))?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        bail!("Unsupported native window handle");
    };
    // The GPUI-owned view is live for the borrowed window handle.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    view.window().context("The native view has no window")
}

#[cfg(target_os = "macos")]
pub(crate) fn configure_floating(window: &mut gpui::Window) -> anyhow::Result<()> {
    use anyhow::{Context as _, ensure};
    use objc2_app_kit::{
        NSAccessibility, NSAccessibilityFloatingWindowSubrole, NSAccessibilityWindowRole, NSPanel,
        NSWindowStyleMask,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    window.set_window_title("Force");
    let native = native_window(window)?;
    let panel = native
        .downcast_ref::<NSPanel>()
        .context("A floating window requires an NSPanel")?;
    // GPUI's titlebar=None still supplies a titled, full-size content window.
    // Preserve the nonactivating bit while removing every native titlebar bit.
    native.setStyleMask(NSWindowStyleMask::NonactivatingPanel);
    native.setHidesOnDeactivate(false);
    native.setMovableByWindowBackground(false);
    native.setHasShadow(false);
    panel.setFloatingPanel(true);
    // GPUI inputs share a custom NSView, which does not implement AppKit's
    // needsPanelToBecomeKey opt-in. Let clicks reacquire keyboard focus, also
    // after dismissing a native save/confirmation dialog.
    panel.setBecomesKeyOnlyIfNeeded(false);
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("Native window is unavailable: {error:?}"))?;
    if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
        // Removing the titled style resets AppKit's first responder to the
        // panel. Restore GPUI's text-input view: logical focus and paste still
        // work without it, but ordinary typing and IME composition do not.
        // GPUI owns this live view for the duration of the window handle.
        let view = unsafe { handle.ns_view.cast::<objc2_app_kit::NSView>().as_ref() };
        ensure!(
            native.makeFirstResponder(Some(view)),
            "The floating window could not restore its text input responder"
        );
    }
    // A nonactivating tool panel is an application window, with floating-window
    // semantics exposed to assistive technology rather than an anonymous dialog.
    native.setAccessibilityRole(Some(unsafe { NSAccessibilityWindowRole }));
    native.setAccessibilitySubrole(Some(unsafe { NSAccessibilityFloatingWindowSubrole }));
    set_always_on_top(window, true)
}

#[cfg(target_os = "windows")]
pub(crate) fn configure_floating(window: &gpui::Window) -> anyhow::Result<()> {
    set_always_on_top(window, true)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn configure_floating(_window: &gpui::Window) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn hide_manager(window: &gpui::Window, cx: &gpui::App) -> anyhow::Result<()> {
    let native = native_window(window)?;
    // AppKit notifications may synchronously enter GPUI. Hide this window
    // after its current borrow ends, leaving the independent timer visible.
    cx.foreground_executor()
        .spawn(async move {
            native.orderOut(None);
        })
        .detach();
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn hide_manager(window: &gpui::Window, cx: &gpui::App) -> anyhow::Result<()> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("Native window is unavailable: {error:?}"))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        anyhow::bail!("Unsupported native window handle");
    };
    let hwnd = handle.hwnd.get() as _;
    cx.foreground_executor()
        .spawn(async move {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        })
        .detach();
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn hide_manager(window: &gpui::Window, _cx: &gpui::App) -> anyhow::Result<()> {
    window.minimize_window();
    Ok(())
}

pub(crate) fn show_manager(window: &gpui::Window, cx: &gpui::App) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::UI::WindowsAndMessaging::{SW_SHOW, ShowWindow};
        let handle = HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("Native window is unavailable: {error:?}"))?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            anyhow::bail!("Unsupported native window handle");
        };
        let hwnd = handle.hwnd.get() as _;
        cx.foreground_executor()
            .spawn(async move {
                unsafe {
                    ShowWindow(hwnd, SW_SHOW);
                }
            })
            .detach();
    }
    cx.activate(true);
    window.activate_window();
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn start_window_drag(window: &gpui::Window) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let native = native_window(window)?;
    let event = native
        .currentEvent()
        .context("A native mouse-down event is required")?;
    native.performWindowDragWithEvent(&event);
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn start_window_drag(window: &gpui::Window) -> anyhow::Result<()> {
    use anyhow::bail;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{HTCAPTION, SendMessageW, WM_NCLBUTTONDOWN},
    };
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("Native window is unavailable: {error:?}"))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        bail!("Unsupported native window handle")
    };
    // This is called only for the dedicated drag region, never an interactive child.
    unsafe {
        ReleaseCapture();
        SendMessageW(
            handle.hwnd.get() as _,
            WM_NCLBUTTONDOWN,
            HTCAPTION as usize,
            0,
        );
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn start_window_drag(window: &gpui::Window) -> anyhow::Result<()> {
    window.start_window_move();
    Ok(())
}

pub(crate) fn play_completion_sound() {
    #[cfg(target_os = "macos")]
    objc2_app_kit::NSBeep();
    #[cfg(target_os = "windows")]
    unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(0);
    }
}

pub(crate) fn restored_bounds(
    directory: &std::path::Path,
    cx: &gpui::App,
) -> Option<gpui::Bounds<gpui::Pixels>> {
    let text = std::fs::read_to_string(directory.join("window-state")).ok()?;
    let mut values = text.split_whitespace();
    if values.next()? != "v1" {
        return None;
    }
    let x: f32 = values.next()?.parse().ok()?;
    let y: f32 = values.next()?.parse().ok()?;
    if !x.is_finite() || !y.is_finite() || x.abs() > 1_000_000. || y.abs() > 1_000_000. {
        return None;
    }
    let mut best = None;
    for display in cx.displays() {
        let bounds = display.bounds();
        let left = f32::from(bounds.origin.x) + 8.;
        let top = f32::from(bounds.origin.y) + if cfg!(target_os = "macos") { 28. } else { 8. };
        let right = (f32::from(bounds.origin.x) + f32::from(bounds.size.width) - STRIP_WIDTH - 8.)
            .max(left);
        let bottom =
            (f32::from(bounds.origin.y) + f32::from(bounds.size.height) - STRIP_HEIGHT - 8.)
                .max(top);
        let clamped_x = x.clamp(left, right);
        let clamped_y = y.clamp(top, bottom);
        let distance = (clamped_x - x).powi(2) + (clamped_y - y).powi(2);
        if best.is_none_or(|(_, _, previous)| distance < previous) {
            best = Some((clamped_x, clamped_y, distance));
        }
    }
    best.map(|(x, y, _)| {
        gpui::Bounds::new(
            gpui::point(gpui::px(x), gpui::px(y)),
            gpui::size(gpui::px(STRIP_WIDTH), gpui::px(STRIP_HEIGHT)),
        )
    })
}

pub(crate) fn persist_position(
    directory: &std::path::Path,
    position: (f32, f32),
) -> std::io::Result<()> {
    write_private_state(
        directory,
        "window-state",
        format!("v1 {} {}\n", position.0, position.1).as_bytes(),
    )
}

pub(crate) fn write_private_state(
    directory: &std::path::Path,
    name: &str,
    content: &[u8],
) -> std::io::Result<()> {
    use std::io::Write as _;
    std::fs::create_dir_all(directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = directory.join(format!(".{name}-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, directory.join(name))?;
        #[cfg(unix)]
        std::fs::File::open(directory)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
