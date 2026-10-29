//! Native button positions and Windows' interval and rectangle for multiple clicks.
//! https://devblogs.microsoft.com/oldnewthing/20041018-00/?p=37543
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    time::{Duration, Instant},
};
use winit::event::{ElementState, MouseButton};

struct ButtonPosition {
    button: MouseButton,
    state: ElementState,
    point: (i32, i32),
}

#[derive(Default)]
pub(super) struct Clicks {
    previous: Option<(Instant, (f64, f64), u32)>,
    size: (i32, i32),
    pending: Rc<RefCell<VecDeque<ButtonPosition>>>,
}

impl Clicks {
    #[allow(unsafe_code)]
    pub(super) fn install(&self, window: isize) -> Result<(), String> {
        use windows_sys::Win32::UI::Shell::SetWindowSubclass;
        let data = Rc::into_raw(self.pending.clone());
        // Winit omits each button message's coordinates and may buffer its
        // callback. Keep the native positions in the same order as those events.
        if unsafe { SetWindowSubclass(window as _, Some(observe), 0, data as usize) } == 0 {
            unsafe { drop(Rc::from_raw(data)) };
            return Err("could not observe native mouse button positions".into());
        }
        Ok(())
    }

    pub(super) fn position(&self, state: ElementState, button: MouseButton) -> Option<(i32, i32)> {
        let mut pending = self.pending.borrow_mut();
        if pending
            .front()
            .is_some_and(|event| event.state == state && event.button == button)
        {
            pending.pop_front().map(|event| event.point)
        } else {
            // Never apply an unmatched position to a later click.
            pending.clear();
            None
        }
    }

    pub(super) fn clear(&mut self) {
        self.previous = None;
    }

    pub(super) fn moved(&mut self, point: (f64, f64)) {
        if self
            .previous
            .is_some_and(|(_, origin, _)| !inside(point, origin, self.size))
        {
            self.clear();
        }
    }

    pub(super) fn down(&mut self, point: (f64, f64), now: Instant) -> u32 {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        // These settings are live user preferences; reading them changes no state.
        #[allow(unsafe_code)]
        let (timeout, size) = unsafe {
            (
                GetDoubleClickTime(),
                (
                    GetSystemMetrics(SM_CXDOUBLECLK),
                    GetSystemMetrics(SM_CYDOUBLECLK),
                ),
            )
        };
        self.at(point, now, Duration::from_millis(u64::from(timeout)), size)
    }

    fn at(&mut self, point: (f64, f64), now: Instant, timeout: Duration, size: (i32, i32)) -> u32 {
        self.size = size;
        let count = self
            .previous
            .filter(|(then, origin, _)| {
                now.saturating_duration_since(*then) <= timeout && inside(point, *origin, size)
            })
            .map_or(1, |(_, _, count)| count % 3 + 1);
        self.previous = Some((now, point, count));
        count
    }
}

#[allow(unsafe_code)]
unsafe extern "system" fn observe(
    window: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
    subclass: usize,
    data: usize,
) -> isize {
    use windows_sys::Win32::UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass},
        WindowsAndMessaging::*,
    };
    use ElementState::{Pressed, Released};
    use MouseButton::{Back, Forward, Left, Middle, Other, Right};
    unsafe {
        let pending = &*(data as *const RefCell<VecDeque<ButtonPosition>>);
        let event = match message {
            WM_LBUTTONDOWN => Some((Left, Pressed)),
            WM_LBUTTONUP => Some((Left, Released)),
            WM_RBUTTONDOWN => Some((Right, Pressed)),
            WM_RBUTTONUP => Some((Right, Released)),
            WM_MBUTTONDOWN => Some((Middle, Pressed)),
            WM_MBUTTONUP => Some((Middle, Released)),
            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                let button = match (wparam >> 16) as u16 {
                    1 => Back,
                    2 => Forward,
                    other => Other(other),
                };
                Some((
                    button,
                    if message == WM_XBUTTONDOWN {
                        Pressed
                    } else {
                        Released
                    },
                ))
            }
            _ => None,
        };
        if let Some((button, state)) = event {
            // Client coordinates are signed, including captured releases outside
            // the window: https://learn.microsoft.com/windows/win32/inputdev/wm-lbuttonup
            let point = (i32::from(lparam as i16), i32::from((lparam >> 16) as i16));
            pending.borrow_mut().push_back(ButtonPosition {
                button,
                state,
                point,
            });
        }
        if message == WM_NCDESTROY {
            RemoveWindowSubclass(window, Some(observe), subclass);
            pending.borrow_mut().clear();
            drop(Rc::from_raw(
                data as *const RefCell<VecDeque<ButtonPosition>>,
            ));
        }
        DefSubclassProc(window, message, wparam, lparam)
    }
}

fn inside(point: (f64, f64), origin: (f64, f64), size: (i32, i32)) -> bool {
    point.0 >= origin.0 - f64::from(size.0 / 2)
        && point.0 < origin.0 + f64::from(size.0 / 2)
        && point.1 >= origin.1 - f64::from(size.1 / 2)
        && point.1 < origin.1 + f64::from(size.1 / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(unsafe_code)]
    fn native_button_positions_survive_buffering_and_preserve_captured_coordinates() {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        use ElementState::{Pressed, Released};
        use MouseButton::{Back, Forward, Left, Middle, Right};
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        unsafe {
            let window = CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            assert!(!window.is_null());
            let clicks = Clicks::default();
            clicks.install(window as isize).unwrap();
            assert_eq!(Rc::strong_count(&clicks.pending), 2);
            let cases = [
                (WM_LBUTTONDOWN, 0, Left, Pressed, (1066i16, 26i16)),
                (WM_LBUTTONUP, 0, Left, Released, (-40, 900)),
                (WM_RBUTTONDOWN, 0, Right, Pressed, (12, -9)),
                (WM_RBUTTONUP, 0, Right, Released, (17, 20)),
                (WM_MBUTTONDOWN, 0, Middle, Pressed, (100, 40)),
                (WM_MBUTTONUP, 0, Middle, Released, (101, 45)),
                (WM_XBUTTONDOWN, 1usize << 16, Back, Pressed, (19, 23)),
                (WM_XBUTTONUP, 1usize << 16, Back, Released, (20, 24)),
                (WM_XBUTTONDOWN, 2usize << 16, Forward, Pressed, (30, 34)),
                (WM_XBUTTONUP, 2usize << 16, Forward, Released, (31, 35)),
            ];
            for (message, wparam, _, _, (x, y)) in cases {
                let packed = (u32::from(x as u16) | (u32::from(y as u16) << 16)) as isize;
                SendMessageW(window, message, wparam, packed);
            }
            for (_, _, button, state, (x, y)) in cases {
                assert_eq!(
                    clicks.position(state, button),
                    Some((i32::from(x), i32::from(y)))
                );
            }
            assert!(clicks.pending.borrow().is_empty());
            SendMessageW(window, WM_LBUTTONDOWN, 0, 17);
            assert_eq!(clicks.position(Released, Left), None);
            assert_eq!(clicks.position(Pressed, Left), None);
            SendMessageW(window, WM_LBUTTONDOWN, 0, 26);
            assert_ne!(DestroyWindow(window), 0);
            assert!(clicks.pending.borrow().is_empty());
            assert_eq!(Rc::strong_count(&clicks.pending), 1);
        }
    }

    #[test]
    fn clicks_follow_system_time_and_rectangle_and_reset_after_interruption() {
        let mut clicks = Clicks::default();
        let start = Instant::now();
        let limit = Duration::from_millis(400);
        assert_eq!(clicks.at((10., 10.), start, limit, (4, 6)), 1);
        assert_eq!(clicks.at((11., 12.), start + limit, limit, (4, 6)), 2);
        assert_eq!(clicks.at((11., 12.), start + limit * 2, limit, (4, 6)), 3);
        assert_eq!(clicks.at((11., 12.), start + limit * 2, limit, (4, 6)), 1);
        assert_eq!(clicks.at((13., 12.), start + limit * 2, limit, (4, 6)), 1);
        assert_eq!(clicks.at((13., 12.), start + limit * 4, limit, (4, 6)), 1);
        clicks.clear();
        assert_eq!(clicks.at((13., 12.), start + limit * 4, limit, (4, 6)), 1);
        clicks.moved((20., 12.));
        assert_eq!(clicks.at((13., 12.), start + limit * 4, limit, (4, 6)), 1);
    }
}
