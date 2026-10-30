//! Keep the client reachable when a display's scale changes.
#![allow(unsafe_code)]
use windows_sys::Win32::{
    Foundation::RECT,
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    UI::WindowsAndMessaging::{
        GetClientRect, GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
    },
};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

pub(super) fn fit(window: &Window, minimum: (f32, f32)) -> bool {
    if window.fullscreen().is_some() || window.is_maximized() || window.is_minimized() == Some(true)
    {
        return false;
    }
    let Ok(handle) = window.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return false;
    };
    let hwnd = handle.hwnd.get() as _;
    let mut outer = RECT::default();
    let mut client = RECT::default();
    let mut monitor = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // All coordinates are physical pixels in this per-monitor-aware process.
    if unsafe {
        GetWindowRect(hwnd, &mut outer) == 0
            || GetClientRect(hwnd, &mut client) == 0
            || GetMonitorInfoW(
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            ) == 0
    } {
        return false;
    }
    let scale = window.scale_factor();
    let minimum = (
        (f64::from(minimum.0) * scale).ceil() as i32 + outer.right - outer.left - client.right,
        (f64::from(minimum.1) * scale).ceil() as i32 + outer.bottom - outer.top - client.bottom,
    );
    let fitted = fit_rect(outer, monitor.rcWork, minimum);
    if (fitted.left, fitted.top, fitted.right, fitted.bottom)
        == (outer.left, outer.top, outer.right, outer.bottom)
    {
        return false;
    }
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            fitted.left,
            fitted.top,
            fitted.right - fitted.left,
            fitted.bottom - fitted.top,
            SWP_NOACTIVATE | SWP_NOZORDER,
        ) != 0
    }
}

fn fit_rect(rect: RECT, work: RECT, minimum: (i32, i32)) -> RECT {
    let width = (rect.right - rect.left).min((work.right - work.left).max(minimum.0));
    let height = (rect.bottom - rect.top).min((work.bottom - work.top).max(minimum.1));
    let left = rect
        .left
        .clamp(work.left, (work.right - width).max(work.left));
    let top = rect
        .top
        .clamp(work.top, (work.bottom - height).max(work.top));
    RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect([left, top, right, bottom]: [i32; 4]) -> RECT {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn scale_change_keeps_content_above_the_taskbar() {
        let work = rect([0, 0, 5120, 1344]);
        let fitted = fit_rect(rect([78, 0, 2304, 1592]), work, (746, 1072));
        assert_eq!(
            [fitted.left, fitted.top, fitted.right, fitted.bottom],
            [78, 0, 2304, 1344]
        );

        let small = fit_rect(rect([80, 100, 1180, 900]), work, (360, 500));
        assert_eq!(
            [small.left, small.top, small.right, small.bottom],
            [80, 100, 1180, 900]
        );

        let other = fit_rect(
            rect([-900, 100, 100, 1300]),
            rect([-1920, 40, 0, 1080]),
            (360, 500),
        );
        assert_eq!(
            [other.left, other.top, other.right, other.bottom],
            [-1000, 40, 0, 1080]
        );

        // An authored minimum can exceed the display; retain it and keep the title bar reachable.
        let minimum = fit_rect(
            rect([20, 40, 1020, 1040]),
            rect([0, 0, 800, 600]),
            (900, 700),
        );
        assert_eq!(
            [minimum.left, minimum.top, minimum.right, minimum.bottom],
            [0, 0, 900, 700]
        );
    }
}
