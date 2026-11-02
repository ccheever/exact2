//! Windows owns the editor menu's layout, input, clipping and dismissal.
#![allow(unsafe_code)]
use exact_linux::presenter::edit_menu::{EditAction, EditMenu};
use std::ptr;
use windows_sys::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ClientToScreen,
    System::DataExchange::IsClipboardFormatAvailable,
    UI::WindowsAndMessaging::*,
};

const ITEMS: [(EditAction, &str); 7] = [
    (EditAction::Undo, "&Undo\tCtrl+Z"),
    (EditAction::Redo, "&Redo\tCtrl+Y"),
    (EditAction::Cut, "Cu&t\tCtrl+X"),
    (EditAction::Copy, "&Copy\tCtrl+C"),
    (EditAction::Paste, "&Paste\tCtrl+V"),
    (EditAction::Delete, "&Delete\tDel"),
    (EditAction::SelectAll, "Select &All\tCtrl+A"),
];

struct Menu(HMENU);
impl Drop for Menu {
    fn drop(&mut self) {
        // SAFETY: created here, destroyed after TrackPopupMenuEx releases it.
        unsafe {
            DestroyMenu(self.0);
        }
    }
}

pub(super) fn show(hwnd: isize, spec: &EditMenu, scale: f32) -> Result<Option<EditAction>, String> {
    let error = || format!("Windows text menu: {}", std::io::Error::last_os_error());
    // SAFETY: called on the window's UI thread while its HWND remains alive.
    // AppendMenuW copies the labels; the menu owns no application pointers.
    unsafe {
        let menu = Menu(CreatePopupMenu());
        if menu.0.is_null() {
            return Err(error());
        }
        for (index, (action, label)) in ITEMS.iter().enumerate() {
            if matches!(index, 2 | 6) && AppendMenuW(menu.0, MF_SEPARATOR, 0, ptr::null()) == 0 {
                return Err(error());
            }
            let available = spec.enabled(*action)
                && (*action != EditAction::Paste || IsClipboardFormatAvailable(13) != 0);
            let label: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            if AppendMenuW(
                menu.0,
                MF_STRING | if available { MF_ENABLED } else { MF_GRAYED },
                *action as usize,
                label.as_ptr(),
            ) == 0
            {
                return Err(error());
            }
        }
        let mut point = POINT {
            x: (spec.point.0 * scale).round() as i32,
            y: (spec.point.1 * scale).round() as i32,
        };
        if ClientToScreen(hwnd as HWND, &mut point) == 0 {
            return Err(error());
        }
        let align = if GetSystemMetrics(SM_MENUDROPALIGNMENT) != 0 {
            TPM_RIGHTALIGN
        } else {
            TPM_LEFTALIGN
        };
        let picked = TrackPopupMenuEx(
            menu.0,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | align,
            point.x,
            point.y,
            hwnd as HWND,
            ptr::null(),
        );
        Ok(ITEMS
            .iter()
            .find(|(action, _)| *action as i32 == picked)
            .map(|(action, _)| *action))
    }
}
