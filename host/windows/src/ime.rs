//! Bind Windows' composition stream to the field that began it.
use exact_linux::{presenter::ime::Target, Presenter};
use exact_runner::DataSource;
use std::{cell::Cell, rc::Rc};
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::Ime,
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

#[derive(Default)]
pub(super) struct InputMethod {
    allowed: Option<Target>,
    session: Option<Target>,
    composing: bool,
    hwnd: Rc<Cell<isize>>,
    area: Option<[f64; 5]>,
}

impl InputMethod {
    pub(super) fn message_hook(&self) -> impl Fn(*const std::ffi::c_void) + 'static {
        let hwnd = self.hwnd.clone();
        move |message| {
            complete_before_pointer(message, hwnd.get());
        }
    }

    pub(super) fn sync<D: DataSource>(&mut self, window: &Window, presenter: &Presenter<D>) {
        if self.hwnd.get() == 0 {
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                    self.hwnd.set(handle.hwnd.get());
                    install_composition_refresh(handle.hwnd.get());
                }
            }
        }
        let target = presenter.ime_target();
        if self.allowed != target {
            self.area = None;
            if self.allowed.take().is_some() {
                // Disassociating an active IME cancels its preedit. Native
                // Windows editors complete it when their window loses focus.
                complete(self.hwnd.get());
                window.set_ime_allowed(false);
            }
            // Closing an old context can deliver its final result. Its owner
            // must not be replaced before Disabled has drained that result.
            if !self.composing && target.is_some() {
                window.set_ime_allowed(true);
                self.allowed = target;
            }
        }
        let area = presenter.ime_cursor_area().map(|(x, y, width, height)| {
            [
                x.into(),
                y.into(),
                width.into(),
                height.into(),
                window.scale_factor(),
            ]
        });
        if area != self.area {
            self.area = area;
            if let Some([x, y, width, height, _]) = area {
                window.set_ime_cursor_area(
                    LogicalPosition::new(x, y),
                    LogicalSize::new(width, height),
                );
            }
        }
    }

    pub(super) fn event<D: DataSource>(&mut self, presenter: &mut Presenter<D>, event: Ime) {
        match event {
            Ime::Enabled => {
                self.reposition();
                self.session = self.allowed;
                self.composing = true;
                if let Some(target) = self.session {
                    presenter.ime_begin(target);
                }
            }
            Ime::Preedit(text, cursor) => {
                if self
                    .session
                    .is_some_and(|target| presenter.ime_begin(target))
                {
                    presenter.ime_preedit(&text, cursor);
                    presenter.ime_unconverted(&unconverted(self.hwnd.get(), &text));
                }
            }
            Ime::Commit(text) => {
                if self
                    .session
                    .is_some_and(|target| presenter.ime_begin(target))
                {
                    // Some IMEs commit one clause and immediately compose
                    // another without a new Enabled event.
                    presenter.ime_commit(&text);
                }
            }
            Ime::Disabled => {
                presenter.ime_end();
                self.composing = false;
                self.session = None;
            }
        }
    }

    pub(super) fn takes_keys(&self) -> bool {
        // Winit 0.30's Windows backend still emits hardware keys while the
        // IME handles them (upstream #4508). Enter must never also send a chat.
        self.composing
    }

    pub(super) fn reposition(&mut self) {
        self.area = None;
    }
}

fn composition_flags(flags: isize) -> isize {
    use windows_sys::Win32::UI::Input::Ime::{
        GCS_COMPATTR, GCS_COMPCLAUSE, GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR,
    };
    // Winit 0.30 emits Preedit only for GCS_COMPSTR. IMM can change the
    // selection or decoration without changing text; ask Winit to read both.
    // A result-only message must not resurrect the composition it committed.
    if flags & GCS_RESULTSTR as isize == 0
        && flags & (GCS_COMPATTR | GCS_COMPCLAUSE | GCS_CURSORPOS) as isize != 0
    {
        flags | GCS_COMPSTR as isize
    } else {
        flags
    }
}

#[allow(unsafe_code)]
fn install_composition_refresh(window: isize) {
    use windows_sys::Win32::{Foundation::HWND, UI::Shell::SetWindowSubclass};
    // Called on the HWND's owning thread. The callback owns no app data and
    // removes itself at WM_NCDESTROY, before the HWND can be reused.
    if unsafe { SetWindowSubclass(window as HWND, Some(composition_proc), 0, 0) } == 0 {
        eprintln!("exact-windows: could not install IME composition refresh");
    }
}

#[allow(unsafe_code)]
unsafe extern "system" fn composition_proc(
    window: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
    subclass: usize,
    _: usize,
) -> isize {
    use windows_sys::Win32::UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass},
        WindowsAndMessaging::{WM_IME_COMPOSITION, WM_NCDESTROY},
    };
    // Subclassing catches sent messages too; the event-loop hook sees only
    // queued messages. Forward once through the remaining subclass chain.
    unsafe {
        if message == WM_NCDESTROY {
            RemoveWindowSubclass(window, Some(composition_proc), subclass);
        }
        let flags = if message == WM_IME_COMPOSITION {
            composition_flags(lparam)
        } else {
            lparam
        };
        DefSubclassProc(window, message, wparam, flags)
    }
}

fn input_ranges(text: &str, attrs: &[u8]) -> Vec<(u32, u32)> {
    use windows_sys::Win32::UI::Input::Ime::ATTR_INPUT;
    if attrs.len() != text.encode_utf16().count() {
        return Vec::new();
    }
    let mut result: Vec<(u32, u32)> = Vec::new();
    let mut offset = 0;
    for ch in text.chars() {
        let end = offset + ch.len_utf16();
        if attrs[offset..end]
            .iter()
            .all(|a| u32::from(*a & 15) == ATTR_INPUT)
        {
            if let Some(last) = result.last_mut().filter(|last| last.1 == offset as u32) {
                last.1 = end as u32;
            } else {
                result.push((offset as u32, end as u32));
            }
        }
        offset = end;
    }
    result
}

#[allow(unsafe_code)]
fn unconverted(window: isize, text: &str) -> Vec<(u32, u32)> {
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::Input::Ime::{
            ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext, GCS_COMPATTR,
        },
    };
    let Ok(length) = u32::try_from(text.encode_utf16().count()) else {
        return Vec::new();
    };
    if window == 0 || length == 0 {
        return Vec::new();
    }
    // IMM's Unicode attributes have one byte per UTF-16 unit. Read the context
    // while handling its preedit and release it before returning to the event loop.
    unsafe {
        let context = ImmGetContext(window as HWND);
        if context.is_null() {
            return Vec::new();
        }
        let mut attrs = vec![0; length as usize];
        let copied =
            ImmGetCompositionStringW(context, GCS_COMPATTR, attrs.as_mut_ptr().cast(), length);
        ImmReleaseContext(window as HWND, context);
        if copied == length as i32 {
            input_ranges(text, &attrs)
        } else {
            Vec::new()
        }
    }
}

// The hook runs before Winit dispatches the mouse message, outside an app
// callback. The IME's synchronous result therefore reaches the editor before
// the pointer changes its selection or opens an editing menu.
#[allow(unsafe_code)]
fn complete_before_pointer(message: *const std::ffi::c_void, window: isize) {
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            MSG, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_XBUTTONDOWN,
        },
    };
    // Winit supplies a live MSG for the duration of this callback. Only the
    // app's own window is eligible; an IME candidate window handles its clicks.
    unsafe {
        let Some(message) = message.cast::<MSG>().as_ref() else {
            return;
        };
        if window == 0
            || message.hwnd != window as HWND
            || !matches!(
                message.message,
                WM_LBUTTONDOWN
                    | WM_LBUTTONDBLCLK
                    | WM_RBUTTONDOWN
                    | WM_MBUTTONDOWN
                    | WM_XBUTTONDOWN
            )
        {
            return;
        }
        complete(window);
    }
}

#[allow(unsafe_code)]
fn complete(window: isize) {
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::Input::Ime::{
            ImmGetCompositionStringW, ImmGetContext, ImmNotifyIME, ImmReleaseContext, CPS_COMPLETE,
            GCS_COMPSTR, NI_COMPOSITIONSTR,
        },
    };
    if window == 0 {
        return;
    }
    // The context belongs to this thread's live window and is released after
    // the synchronous completion request, including when no text is composing.
    unsafe {
        let context = ImmGetContext(window as HWND);
        if !context.is_null() {
            if ImmGetCompositionStringW(context, GCS_COMPSTR, std::ptr::null_mut(), 0) > 0 {
                ImmNotifyIME(context, NI_COMPOSITIONSTR, CPS_COMPLETE, 0);
            }
            ImmReleaseContext(window as HWND, context);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[allow(unsafe_code)]
    fn composition_subclass_forwards_each_sent_message_once() {
        use windows_sys::Win32::{
            Foundation::HWND,
            UI::{
                Input::Ime::{GCS_COMPATTR, GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR},
                Shell::{DefSubclassProc, SetWindowSubclass},
                WindowsAndMessaging::{
                    CreateWindowExW, DestroyWindow, SendMessageW, HWND_MESSAGE, WM_IME_COMPOSITION,
                },
            },
        };
        thread_local! {
            static RECEIVED: Cell<(usize, isize)> = const { Cell::new((0, 0)) };
        }
        unsafe extern "system" fn receiver(
            window: HWND,
            message: u32,
            wparam: usize,
            lparam: isize,
            _: usize,
            _: usize,
        ) -> isize {
            if message == WM_IME_COMPOSITION {
                RECEIVED.set((RECEIVED.get().0 + 1, lparam));
                177
            } else {
                unsafe { DefSubclassProc(window, message, wparam, lparam) }
            }
        }
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // A message-only window exercises native dispatch without showing UI or
        // changing the user's input context. Its receiver consumes the IME data.
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
            assert_ne!(SetWindowSubclass(window, Some(receiver), 0, 0), 0);
            install_composition_refresh(window as isize);
            let flags = [GCS_COMPATTR, GCS_CURSORPOS, GCS_RESULTSTR, 0];
            for (i, flag) in flags.into_iter().enumerate() {
                assert_eq!(
                    SendMessageW(window, WM_IME_COMPOSITION, 0, flag as isize),
                    177
                );
                let expected = if i < 2 { flag | GCS_COMPSTR } else { flag };
                assert_eq!(RECEIVED.get(), (i + 1, expected as isize));
            }
            assert_ne!(DestroyWindow(window), 0);
        }
    }

    #[test]
    fn composition_refresh_preserves_results_and_cancellation() {
        use windows_sys::Win32::UI::Input::Ime::{
            GCS_COMPATTR, GCS_COMPCLAUSE, GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR,
        };
        for update in [
            GCS_COMPATTR,
            GCS_COMPCLAUSE,
            GCS_CURSORPOS,
            GCS_COMPATTR | GCS_COMPCLAUSE | GCS_CURSORPOS,
        ] {
            assert_eq!(
                composition_flags(update as isize),
                (update | GCS_COMPSTR) as isize
            );
            let result = (update | GCS_RESULTSTR) as isize;
            assert_eq!(composition_flags(result), result);
            let both = result | GCS_COMPSTR as isize;
            assert_eq!(composition_flags(both), both);
        }
        assert_eq!(composition_flags(0), 0);
        assert_eq!(
            composition_flags(GCS_RESULTSTR as isize),
            GCS_RESULTSTR as isize
        );
        assert_eq!(
            composition_flags(GCS_COMPSTR as isize),
            GCS_COMPSTR as isize
        );
    }

    #[test]
    fn unconverted_attributes_preserve_utf16_boundaries_and_converted_clauses() {
        assert_eq!(input_ranges("あ🦀いう", &[0, 0, 0, 1, 0]), [(0, 3), (4, 5)]);
        assert!(input_ranges("🦀", &[0, 1]).is_empty());
        assert!(input_ranges("あ", &[]).is_empty());
        assert!(input_ranges("あ", &[1, 0]).is_empty());
        assert!(input_ranges("あいうえお", &[1, 2, 3, 4, 5]).is_empty());
    }
}
