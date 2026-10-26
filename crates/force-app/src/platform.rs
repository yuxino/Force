#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicU64, Ordering};

// Force has one management window. New show/hide intent cancels older queued
// AppKit work so a delayed activation cannot reopen a window the user closed.
#[cfg(target_os = "macos")]
static MANAGER_PRESENTATION_GENERATION: AtomicU64 = AtomicU64::new(0);

pub(crate) const STRIP_WIDTH: f32 = 88.;
pub(crate) const STRIP_HEIGHT: f32 = 128.;
pub(crate) const STRIP_EXPANDED_WIDTH: f32 = 264.;
pub(crate) const STRIP_EXPANDED_HEIGHT: f32 = 158.;
pub(crate) const STRIP_DOCKED_WIDTH: f32 = 28.;
pub(crate) const STRIP_DOCKED_HEIGHT: f32 = 80.;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockEdge {
    Left,
    Right,
}

fn floating_geometry(
    current: gpui::Bounds<gpui::Pixels>,
    dimensions: gpui::Size<gpui::Pixels>,
    available: gpui::Bounds<gpui::Pixels>,
    edge: Option<DockEdge>,
) -> gpui::Bounds<gpui::Pixels> {
    use gpui::{Bounds, point, px};

    let inset = if edge.is_some() { px(0.) } else { px(8.) };
    let left = available.left() + inset;
    let top = available.top() + px(8.);
    let right = (available.right() - dimensions.width - inset).max(left);
    let bottom = (available.bottom() - dimensions.height - px(8.)).max(top);
    let x = match edge {
        Some(DockEdge::Left) => left,
        Some(DockEdge::Right) => right,
        None => current.origin.x.clamp(left, right),
    };
    Bounds::new(point(x, current.origin.y.clamp(top, bottom)), dimensions)
}

fn dock_edge_for_bounds(
    bounds: gpui::Bounds<gpui::Pixels>,
    available: gpui::Bounds<gpui::Pixels>,
) -> Option<DockEdge> {
    use gpui::px;

    let left = (bounds.left() - available.left()).abs();
    let right = (available.right() - bounds.right()).abs();
    if left.min(right) > px(32.) {
        None
    } else if left <= right {
        Some(DockEdge::Left)
    } else {
        Some(DockEdge::Right)
    }
}

/// The available screen rectangle in the same coordinate basis as this window.
/// GPUI 0.2.2 normalizes macOS display origins, so use native frame differences
/// rather than mixing its display bounds with its per-screen window bounds.
#[cfg(target_os = "macos")]
fn floating_work_area(window: &gpui::Window) -> Option<gpui::Bounds<gpui::Pixels>> {
    use gpui::{Bounds, point, px, size};

    let native = native_window(window).ok()?;
    let frame = native.frame();
    let visible = native.screen()?.visibleFrame();
    let current = window.bounds();
    Some(Bounds::new(
        point(
            current.origin.x + px((visible.origin.x - frame.origin.x) as f32),
            current.origin.y
                + px(
                    (frame.origin.y + frame.size.height - visible.origin.y - visible.size.height)
                        as f32,
                ),
        ),
        size(
            px(visible.size.width as f32),
            px(visible.size.height as f32),
        ),
    ))
}

#[cfg(target_os = "windows")]
fn floating_work_area(window: &gpui::Window) -> Option<gpui::Bounds<gpui::Pixels>> {
    use gpui::{Bounds, point, px, size};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        },
        UI::WindowsAndMessaging::GetWindowRect,
    };

    let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    let hwnd = handle.hwnd.get() as _;
    let mut monitor: MONITORINFO = unsafe { std::mem::zeroed() };
    monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    let mut frame: RECT = unsafe { std::mem::zeroed() };
    // Both rectangles are physical pixels on the window's current monitor.
    if unsafe {
        GetMonitorInfoW(
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        )
    } == 0
        || unsafe { GetWindowRect(hwnd, &mut frame) } == 0
    {
        return None;
    }
    let scale = window.scale_factor();
    let current = window.bounds();
    Some(Bounds::new(
        point(
            current.origin.x + px((monitor.rcWork.left - frame.left) as f32 / scale),
            current.origin.y + px((monitor.rcWork.top - frame.top) as f32 / scale),
        ),
        size(
            px((monitor.rcWork.right - monitor.rcWork.left) as f32 / scale),
            px((monitor.rcWork.bottom - monitor.rcWork.top) as f32 / scale),
        ),
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn floating_work_area(_window: &gpui::Window) -> Option<gpui::Bounds<gpui::Pixels>> {
    None
}

/// Detect after a native drag has finished, before resizing the floating object.
pub(crate) fn floating_dock_edge(window: &gpui::Window) -> Option<DockEdge> {
    dock_edge_for_bounds(window.bounds(), floating_work_area(window)?)
}

/// Preserve the top and free position, moving inward only to fit. A docked
/// object keeps the chosen screen edge through expansion and contraction.
/// Native mutations are queued after the current GPUI window borrow ends.
pub(crate) fn reshape_floating(
    window: &mut gpui::Window,
    dimensions: gpui::Size<gpui::Pixels>,
    edge: Option<DockEdge>,
    cx: &gpui::App,
) -> anyhow::Result<()> {
    use anyhow::ensure;

    ensure!(
        f32::from(dimensions.width).is_finite()
            && f32::from(dimensions.height).is_finite()
            && dimensions.width > gpui::px(0.)
            && dimensions.height > gpui::px(0.),
        "The floating window requires finite, positive dimensions"
    );
    let available = floating_work_area(window)
        .or_else(|| window.display(cx).map(|display| display.bounds()))
        .unwrap_or_else(|| window.bounds());
    let bounds = floating_geometry(window.bounds(), dimensions, available, edge);
    set_floating_bounds(window, bounds, cx)
}

#[cfg(target_os = "macos")]
fn set_floating_bounds(
    window: &mut gpui::Window,
    bounds: gpui::Bounds<gpui::Pixels>,
    cx: &gpui::App,
) -> anyhow::Result<()> {
    let native = match native_window(window) {
        Ok(native) => native,
        Err(error) => {
            #[cfg(test)]
            {
                let _ = error;
                window.resize(bounds.size);
                return Ok(());
            }
            #[cfg(not(test))]
            return Err(error);
        }
    };
    let current = window.bounds();
    let mut frame = native.frame();
    frame.origin.x += f64::from(f32::from(bounds.origin.x - current.origin.x));
    // AppKit uses a bottom-left origin; keep the top stable when height changes.
    frame.origin.y -= f64::from(f32::from(
        bounds.origin.y - current.origin.y + bounds.size.height - current.size.height,
    ));
    frame.size.width = f64::from(f32::from(bounds.size.width));
    frame.size.height = f64::from(f32::from(bounds.size.height));
    cx.foreground_executor()
        .spawn(async move {
            native.setFrame_display(frame, true);
        })
        .detach();
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_floating_bounds(
    window: &mut gpui::Window,
    bounds: gpui::Bounds<gpui::Pixels>,
    cx: &gpui::App,
) -> anyhow::Result<()> {
    use anyhow::{Context as _, bail};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::{
        Foundation::RECT,
        UI::WindowsAndMessaging::{
            GetClientRect, GetWindowRect, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
        },
    };

    let handle = window
        .window_handle()
        .map_err(|error| anyhow::anyhow!("Native window is unavailable: {error:?}"))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        bail!("Unsupported native window handle");
    };
    let hwnd = handle.hwnd.get() as _;
    let mut frame: RECT = unsafe { std::mem::zeroed() };
    let mut client: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(hwnd, &mut frame) } == 0
        || unsafe { GetClientRect(hwnd, &mut client) } == 0
    {
        return Err(std::io::Error::last_os_error()).context("Could not read the floating frame");
    }
    let current = window.bounds();
    let scale = window.scale_factor();
    let x = frame.left + (f32::from(bounds.origin.x - current.origin.x) * scale).round() as i32;
    let y = frame.top + (f32::from(bounds.origin.y - current.origin.y) * scale).round() as i32;
    let width = (f32::from(bounds.size.width) * scale).round() as i32 + frame.right
        - frame.left
        - (client.right - client.left);
    let height = (f32::from(bounds.size.height) * scale).round() as i32 + frame.bottom
        - frame.top
        - (client.bottom - client.top);
    let hwnd = handle.hwnd.get();
    cx.foreground_executor()
        .spawn(async move {
            let result = unsafe {
                SetWindowPos(
                    hwnd as _,
                    std::ptr::null_mut(),
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
            };
            if result == 0 {
                eprintln!(
                    "Force could not reshape the floating window: {}",
                    std::io::Error::last_os_error()
                );
            }
        })
        .detach();
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn set_floating_bounds(
    window: &mut gpui::Window,
    bounds: gpui::Bounds<gpui::Pixels>,
    _cx: &gpui::App,
) -> anyhow::Result<()> {
    // Wayland controls placement. Keep a functional resize without claiming
    // screen-edge placement that the compositor does not expose through GPUI.
    window.resize(bounds.size);
    Ok(())
}
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
    let generation = MANAGER_PRESENTATION_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    // AppKit notifications may synchronously enter GPUI. Hide this window
    // after its current borrow ends, leaving the independent timer visible.
    cx.foreground_executor()
        .spawn(async move {
            if MANAGER_PRESENTATION_GENERATION.load(Ordering::Relaxed) == generation {
                native.orderOut(None);
            }
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

#[cfg(target_os = "macos")]
pub(crate) fn show_manager(window: &gpui::Window, cx: &gpui::App) -> anyhow::Result<()> {
    use anyhow::{Context as _, ensure};
    use objc2::{
        msg_send,
        runtime::{AnyClass, AnyObject},
    };
    use std::time::{Duration, Instant};

    let native = native_window(window)?;
    let class = AnyClass::get(c"NSApplication").context("AppKit is unavailable")?;
    // NSApplication owns this singleton for the lifetime of the process.
    let application: *mut AnyObject = unsafe { msg_send![class, sharedApplication] };
    ensure!(
        !application.is_null(),
        "The native application is unavailable"
    );
    let background = cx.background_executor().clone();
    let generation = MANAGER_PRESENTATION_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    cx.activate(true);
    // Activation completes on a later AppKit turn. GPUI 0.2.2 can receive a
    // spurious key notification while an inactive app has a popup, then call
    // resignKeyWindow while holding its window mutex and deadlock on reentry.
    // Wait for the application before requesting the manager's key status.
    // Keep its native input view, responder and window style untouched.
    cx.foreground_executor()
        .spawn(async move {
            let deadline = Instant::now() + Duration::from_millis(500);
            loop {
                if MANAGER_PRESENTATION_GENERATION.load(Ordering::Relaxed) != generation {
                    return;
                }
                let active: bool = unsafe { msg_send![application, isActive] };
                if active {
                    // Make visibility explicit before requesting key status.
                    // macOS can otherwise notify while isKeyWindow is still false.
                    native.orderFront(None);
                    if MANAGER_PRESENTATION_GENERATION.load(Ordering::Relaxed) == generation {
                        native.makeKeyWindow();
                    }
                    return;
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    native.orderFront(None);
                    eprintln!("Force could not activate management within 500 ms; showing it without keyboard focus.");
                    return;
                }
                background.timer(remaining.min(Duration::from_millis(16))).await;
            }
        })
        .detach();
    Ok(())
}

/// Show the nonactivating clock only after the normal manager is genuinely key.
/// Showing an inactive popup first can trigger GPUI 0.2.2's reentrant key-status
/// workaround. Never activate the popup or replace its text-input responder.
#[cfg(target_os = "macos")]
pub(crate) fn show_floating_after_manager(
    window: &gpui::Window,
    cx: &gpui::App,
) -> anyhow::Result<()> {
    use anyhow::{Context as _, ensure};
    use objc2::{
        msg_send,
        runtime::{AnyClass, AnyObject},
    };
    use std::time::{Duration, Instant};

    let native = native_window(window)?;
    let floating = std::ptr::from_ref(&*native).cast::<AnyObject>();
    let class = AnyClass::get(c"NSApplication").context("AppKit is unavailable")?;
    let application: *mut AnyObject = unsafe { msg_send![class, sharedApplication] };
    ensure!(
        !application.is_null(),
        "The native application is unavailable"
    );
    let background = cx.background_executor().clone();
    let generation = MANAGER_PRESENTATION_GENERATION.load(Ordering::Relaxed);
    cx.foreground_executor()
        .spawn(async move {
            let deadline = Instant::now() + Duration::from_millis(500);
            loop {
                if MANAGER_PRESENTATION_GENERATION.load(Ordering::Relaxed) != generation {
                    return;
                }
                let active: bool = unsafe { msg_send![application, isActive] };
                let key: *mut AnyObject = unsafe { msg_send![application, keyWindow] };
                if active && !key.is_null() && key.cast_const() != floating {
                    let visible: bool = unsafe { msg_send![key, isVisible] };
                    let is_key: bool = unsafe { msg_send![key, isKeyWindow] };
                    if visible && is_key {
                        native.orderFront(None);
                        return;
                    }
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    eprintln!("Force could not safely show its floating clock within 500 ms; reopening management will retry.");
                    return;
                }
                background.timer(remaining.min(Duration::from_millis(16))).await;
            }
        })
        .detach();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
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
pub(crate) struct WindowDragEvent(objc2::rc::Retained<objc2_app_kit::NSEvent>);

/// Capture while processing mouseDown; currentEvent is already mouseDragged
/// once GPUI dispatches the movement that crosses the drag threshold.
#[cfg(target_os = "macos")]
pub(crate) fn capture_window_drag(window: &gpui::Window) -> anyhow::Result<WindowDragEvent> {
    use anyhow::{Context as _, ensure};
    use objc2_app_kit::NSEventType;

    let event = native_window(window)?
        .currentEvent()
        .context("A native mouse-down event is required")?;
    ensure!(
        event.r#type() == NSEventType::LeftMouseDown,
        "Window dragging requires the original left mouse-down event"
    );
    Ok(WindowDragEvent(event))
}

#[cfg(target_os = "macos")]
pub(crate) fn start_window_drag(
    window: &gpui::Window,
    event: WindowDragEvent,
) -> anyhow::Result<()> {
    // AppKit hands movement to WindowServer and returns immediately. The owned
    // initiating event is released after this handoff, rather than retained for
    // the duration of the drag or substituted with a later movement event.
    native_window(window)?.performWindowDragWithEvent(&event.0);
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn window_drag_button_pressed() -> bool {
    objc2_app_kit::NSEvent::pressedMouseButtons() & 1 != 0
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

// Synthetic timer completion in unit/headless tests must stay silent.
pub(crate) fn play_completion_sound() {
    #[cfg(all(target_os = "macos", not(test)))]
    objc2_app_kit::NSBeep();
    #[cfg(all(target_os = "windows", not(test)))]
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

#[cfg(test)]
mod floating_geometry_tests {
    use super::{DockEdge, dock_edge_for_bounds, floating_geometry};
    use gpui::{Bounds, Pixels, point, px, size};

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    #[test]
    fn free_expansion_preserves_the_top_and_only_moves_inward_when_needed() {
        let available = rect(0., 24., 1440., 856.);
        let dimensions = size(px(240.), px(192.));
        assert_eq!(
            floating_geometry(rect(600., 200., 88., 128.), dimensions, available, None),
            rect(600., 200., 240., 192.)
        );
        assert_eq!(
            floating_geometry(rect(1330., 780., 88., 128.), dimensions, available, None),
            rect(1192., 680., 240., 192.)
        );
    }

    #[test]
    fn both_docked_shapes_keep_the_same_edge_and_stay_inside_the_work_area() {
        let available = rect(-1920., 50., 1920., 1030.);
        for (edge, expected_x, closed_x) in [
            (DockEdge::Left, -1920., -1920.),
            (DockEdge::Right, -240., -28.),
        ] {
            let expanded = floating_geometry(
                rect(-600., 200., 88., 128.),
                size(px(240.), px(192.)),
                available,
                Some(edge),
            );
            assert_eq!(expanded, rect(expected_x, 200., 240., 192.));
            let folded = floating_geometry(expanded, size(px(28.), px(80.)), available, Some(edge));
            assert_eq!(folded, rect(closed_x, 200., 28., 80.));
        }
    }

    #[test]
    fn docking_detects_the_objects_edge_on_either_side_with_a_32_point_threshold() {
        let available = rect(-1920., 50., 1920., 1030.);
        assert_eq!(
            dock_edge_for_bounds(rect(-1888., 200., 88., 128.), available),
            Some(DockEdge::Left)
        );
        assert_eq!(
            dock_edge_for_bounds(rect(-120., 200., 88., 128.), available),
            Some(DockEdge::Right)
        );
        assert_eq!(
            dock_edge_for_bounds(rect(-1887., 200., 88., 128.), available),
            None
        );
        assert_eq!(
            dock_edge_for_bounds(rect(-121., 200., 88., 128.), available),
            None
        );
    }
}
