//! Keep scheduled work alive in Windows' menu and move/resize loops.
#![allow(unsafe_code)]
use std::{cell::Cell, rc::Rc, time::Instant};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            KillTimer, SetTimer, WM_ENTERMENULOOP, WM_ENTERSIZEMOVE, WM_EXITMENULOOP,
            WM_EXITSIZEMOVE, WM_NCDESTROY, WM_TIMER,
        },
    },
};
use winit::event_loop::EventLoopProxy;

use crate::accessibility::Event;

#[derive(Default)]
pub(super) struct Schedule(Option<Rc<State>>);

struct State {
    window: Cell<HWND>,
    depth: Cell<u32>,
    deadline: Cell<Option<Instant>>,
    armed: Cell<Option<Instant>>,
    proxy: EventLoopProxy<Event>,
}

impl Schedule {
    pub fn install(&mut self, window: isize, proxy: EventLoopProxy<Event>) {
        let state = Rc::new(State {
            window: Cell::new(window as HWND),
            depth: Cell::new(0),
            deadline: Cell::new(None),
            armed: Cell::new(None),
            proxy,
        });
        let data = Rc::into_raw(state.clone());
        // The HWND holds this reference through WM_NCDESTROY. Both owners and
        // the subclass run on the window thread; no presenter is borrowed here.
        if unsafe { SetWindowSubclass(window as HWND, Some(observe), 0, data as usize) } == 0 {
            unsafe { drop(Rc::from_raw(data)) };
            eprintln!("exact-windows: could not observe native modal loops");
        } else {
            self.0 = Some(state);
        }
    }

    pub fn active(&self) -> bool {
        self.0.as_ref().is_some_and(|state| state.depth.get() > 0)
    }

    pub fn wait_until(&self, deadline: Option<Instant>) {
        if let Some(state) = &self.0 {
            state.deadline.set(deadline);
            state.arm();
        }
    }
}

fn delay_ms(deadline: Instant, now: Instant) -> u32 {
    // SetTimer uses whole milliseconds with a 10 ms floor. Round up so a
    // submillisecond remainder cannot wake a timer before its deadline.
    deadline
        .saturating_duration_since(now)
        .as_nanos()
        .div_ceil(1_000_000)
        .clamp(10, 0x7fff_ffff) as u32
}

impl State {
    fn cancel(&self) {
        if self.armed.take().is_some() && !self.window.get().is_null() {
            unsafe { KillTimer(self.window.get(), self as *const Self as usize) };
        }
    }

    fn arm(&self) {
        let deadline = self.deadline.get().filter(|_| self.depth.get() > 0);
        if self.armed.get() == deadline || self.window.get().is_null() {
            return;
        }
        self.cancel();
        if let Some(deadline) = deadline {
            let timer = unsafe {
                SetTimer(
                    self.window.get(),
                    self as *const Self as usize,
                    delay_ms(deadline, Instant::now()),
                    None,
                )
            };
            if timer != 0 {
                self.armed.set(Some(deadline));
            } else {
                eprintln!("exact-windows: could not schedule a native modal wake");
            }
        }
    }

    fn wake(&self) {
        let _ = self.proxy.send_event(Event::ModalWake);
    }

    fn fire(&self) {
        self.cancel();
        // KillTimer does not remove queued WM_TIMER messages. A message from
        // an earlier deadline must not consume the replacement deadline.
        if self.depth.get() > 0 {
            if self.deadline.get().is_some_and(|due| due <= Instant::now()) {
                self.wake();
            } else {
                self.arm();
            }
        }
    }
}

unsafe extern "system" fn observe(
    window: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
    subclass: usize,
    data: usize,
) -> isize {
    unsafe {
        let state = &*(data as *const State);
        match message {
            WM_ENTERMENULOOP | WM_ENTERSIZEMOVE => {
                state.depth.set(state.depth.get().saturating_add(1));
                state.arm();
            }
            WM_EXITMENULOOP | WM_EXITSIZEMOVE => {
                state.depth.set(state.depth.get().saturating_sub(1));
                state.arm();
                // A posted wake also resumes the outer loop when Windows has
                // consumed the input that dismissed its modal loop.
                state.wake();
            }
            WM_TIMER if wparam == data => {
                state.fire();
                return 0;
            }
            WM_NCDESTROY => {
                state.cancel();
                state.window.set(std::ptr::null_mut());
                RemoveWindowSubclass(window, Some(observe), subclass);
                drop(Rc::from_raw(data as *const State));
            }
            _ => {}
        }
        DefSubclassProc(window, message, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn modal_deadlines_round_up_and_stay_within_windows_timer_limits() {
        let now = Instant::now();
        for (delay, expected) in [
            (Duration::ZERO, 10),
            (Duration::from_micros(10_001), 11),
            (Duration::from_millis(1000), 1000),
            (Duration::from_secs(u32::MAX as u64), 0x7fff_ffff),
        ] {
            assert_eq!(delay_ms(now + delay, now), expected);
        }
        assert_eq!(delay_ms(now, now + Duration::from_secs(1)), 10);
    }

    #[test]
    fn native_modal_and_carrier_deadlines_cancel_rearm_and_release() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SendMessageW, HWND_MESSAGE,
        };
        use winit::{event_loop::EventLoop, platform::windows::EventLoopBuilderExtWindows};

        let event_loop = EventLoop::<Event>::with_user_event()
            .with_any_thread(true)
            .build()
            .unwrap();
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
            let mut schedule = Schedule::default();
            schedule.install(window as isize, event_loop.create_proxy());
            let state = schedule.0.as_ref().unwrap();
            let future = Instant::now() + Duration::from_secs(3600);
            let timer = Rc::as_ptr(state) as usize;
            schedule.wait_until(Some(future));
            assert!(!schedule.active());
            assert_eq!(state.armed.get(), None);
            SendMessageW(window, WM_ENTERMENULOOP, 0, 0);
            SendMessageW(window, WM_ENTERSIZEMOVE, 0, 0);
            assert!(schedule.active());
            assert_eq!(state.armed.get(), Some(future));
            schedule.wait_until(None);
            assert_eq!(state.armed.get(), None);
            schedule.wait_until(Some(future));
            SendMessageW(window, WM_TIMER, timer, 0);
            assert_eq!(state.armed.get(), Some(future));
            schedule.wait_until(Some(Instant::now()));
            SendMessageW(window, WM_TIMER, timer, 0);
            assert_eq!(state.armed.get(), None);
            schedule.wait_until(Some(future));
            SendMessageW(window, WM_EXITSIZEMOVE, 0, 0);
            assert!(schedule.active());
            assert_eq!(state.armed.get(), Some(future));
            SendMessageW(window, WM_EXITMENULOOP, 0, 0);
            assert!(!schedule.active());
            assert_eq!(state.armed.get(), None);
            assert_eq!(Rc::strong_count(state), 2);
            assert_ne!(DestroyWindow(window), 0);
            assert!(state.window.get().is_null());
            assert_eq!(Rc::strong_count(state), 1);
        }
        // Winit permits one event loop per process; both deadline paths use this one.
        crate::window::tests::about_to_wait_honors_retry_pending_redraw_and_earlier_timer(
            event_loop,
        );
    }
}
