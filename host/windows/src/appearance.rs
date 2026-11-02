//! Read contrast colors and observe changes, including sent window messages.
#![allow(unsafe_code)]
use exact_linux::presenter::ContrastPalette;
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use windows::{Foundation::TypedEventHandler, UI::ViewManagement::UISettings};
use windows_sys::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{
        GetSysColor, COLOR_BTNFACE, COLOR_BTNTEXT, COLOR_GRAYTEXT, COLOR_HIGHLIGHT,
        COLOR_HIGHLIGHTTEXT, COLOR_HOTLIGHT, COLOR_WINDOW, COLOR_WINDOWTEXT,
    },
    UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            PostMessageW, SystemParametersInfoW, SPI_GETHIGHCONTRAST, WM_NCDESTROY, WM_NULL,
            WM_SETTINGCHANGE, WM_SYSCOLORCHANGE, WM_THEMECHANGED,
        },
    },
};
use winit::event_loop::EventLoopProxy;

pub(super) fn contrast() -> Option<ContrastPalette> {
    let mut settings = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    // Both APIs read the current user's settings without changing them.
    if unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            settings.cbSize,
            (&mut settings as *mut HIGHCONTRASTW).cast(),
            0,
        )
    } == 0
        || settings.dwFlags & HCF_HIGHCONTRASTON == 0
    {
        return None;
    }
    let color = |index| {
        let [r, g, b, _] = unsafe { GetSysColor(index) }.to_le_bytes();
        [r, g, b, 255]
    };
    Some(ContrastPalette {
        window: color(COLOR_WINDOW),
        text: color(COLOR_WINDOWTEXT),
        link: color(COLOR_HOTLIGHT),
        disabled: color(COLOR_GRAYTEXT),
        highlight: color(COLOR_HIGHLIGHT),
        highlighted_text: color(COLOR_HIGHLIGHTTEXT),
        button: color(COLOR_BTNFACE),
        button_text: color(COLOR_BTNTEXT),
    })
}

#[derive(Default)]
pub(super) struct Changes {
    sent: Rc<Cell<bool>>,
    text_changed: Arc<AtomicBool>,
    text: Option<(UISettings, i64)>,
}

impl Changes {
    pub(super) fn install(
        &mut self,
        window: isize,
        proxy: EventLoopProxy<crate::accessibility::Event>,
    ) {
        let state = Rc::into_raw(self.sent.clone());
        // The HWND owns this reference until WM_NCDESTROY; all access is on
        // its event-loop thread. The subclass does not retain the presenter.
        if unsafe { SetWindowSubclass(window as HWND, Some(changed), 0, state as usize) } == 0 {
            unsafe { drop(Rc::from_raw(state)) };
            eprintln!("exact-windows: could not observe system appearance changes");
        }
        let subscribe = || -> windows::core::Result<_> {
            exact_linux::com::ensure_mta().map_err(windows::core::Error::from_hresult)?;
            let settings = UISettings::new()?;
            let changed = self.text_changed.clone();
            let token = settings.TextScaleFactorChanged(&TypedEventHandler::new(move |_, _| {
                changed.store(true, Ordering::Release);
                let _ = proxy.send_event(crate::accessibility::Event::Wake);
                Ok(())
            }))?;
            Ok((settings, token))
        };
        match subscribe() {
            Ok(subscription) => self.text = Some(subscription),
            Err(error) => eprintln!("exact-windows: text-size notifications: {error}"),
        }
    }

    pub(super) fn text_scale(&self) -> Option<f32> {
        self.text
            .as_ref()?
            .0
            .TextScaleFactor()
            .ok()
            .map(|scale| scale as f32)
    }

    pub(super) fn take(&self) -> bool {
        let sent = self.sent.replace(false);
        self.text_changed.swap(false, Ordering::AcqRel) || sent
    }
}

impl Drop for Changes {
    fn drop(&mut self) {
        if let Some((settings, token)) = self.text.take() {
            let _ = settings.RemoveTextScaleFactorChanged(token);
        }
    }
}

unsafe extern "system" fn changed(
    window: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
    subclass: usize,
    data: usize,
) -> isize {
    unsafe {
        if message == WM_NCDESTROY {
            RemoveWindowSubclass(window, Some(changed), subclass);
            drop(Rc::from_raw(data as *const Cell<bool>));
        } else if matches!(
            message,
            WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED
        ) && !(*(data as *const Cell<bool>)).replace(true)
        {
            // Sent broadcasts need a queued message to wake a waiting loop.
            PostMessageW(window, WM_NULL, 0, 0);
        }
        DefSubclassProc(window, message, wparam, lparam)
    }
}
