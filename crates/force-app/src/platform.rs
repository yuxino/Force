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
