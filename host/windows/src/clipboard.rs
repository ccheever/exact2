//! CF_UNICODETEXT, copied while the clipboard is open and its memory locked.
#![allow(unsafe_code)]
use windows_sys::Win32::{
    Foundation::{GlobalFree, HWND},
    System::{DataExchange::*, Memory::*},
};

const UNICODE_TEXT: u32 = 13;
pub(super) struct Clipboard(pub isize);
struct Open;
impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: constructed only after this thread opened the clipboard.
        unsafe {
            CloseClipboard();
        }
    }
}
fn error() -> String {
    format!("Windows clipboard: {}", std::io::Error::last_os_error())
}
impl Clipboard {
    fn open(&self) -> Result<Open, String> {
        // SAFETY: the carrier keeps this window alive longer than its presenter.
        if unsafe { OpenClipboard(self.0 as HWND) } == 0 {
            return Err(error());
        }
        Ok(Open)
    }
}
impl exact_linux::presenter::clipboard::Clipboard for Clipboard {
    fn read_text(&mut self) -> Result<Option<String>, String> {
        let _open = self.open()?;
        // SAFETY: the clipboard remains open; GlobalSize bounds the UTF-16
        // slice. Unlock before closing, and never free the clipboard's memory.
        unsafe {
            if IsClipboardFormatAvailable(UNICODE_TEXT) == 0 {
                return Ok(None);
            }
            let memory = GetClipboardData(UNICODE_TEXT);
            if memory.is_null() {
                return Err(error());
            }
            let bytes = GlobalSize(memory);
            if bytes < 2 {
                return Ok(Some(String::new()));
            }
            let pointer = GlobalLock(memory).cast::<u16>();
            if pointer.is_null() {
                return Err(error());
            }
            let units = std::slice::from_raw_parts(pointer, bytes / 2);
            let end = units.iter().position(|u| *u == 0).unwrap_or(units.len());
            let text = String::from_utf16_lossy(&units[..end]);
            GlobalUnlock(memory);
            Ok(Some(text))
        }
    }
    fn write_text(&mut self, text: &str) -> Result<(), String> {
        let text = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\n', "\r\n");
        let units: Vec<_> = text.encode_utf16().chain(Some(0)).collect();
        let _open = self.open()?;
        // SAFETY: write only within the allocated block. Ownership passes to
        // Windows only after SetClipboardData succeeds; failures free our block.
        unsafe {
            let memory = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
            if memory.is_null() {
                return Err(error());
            }
            let pointer = GlobalLock(memory).cast::<u16>();
            if pointer.is_null() {
                let e = error();
                GlobalFree(memory);
                return Err(e);
            }
            std::ptr::copy_nonoverlapping(units.as_ptr(), pointer, units.len());
            GlobalUnlock(memory);
            if EmptyClipboard() == 0 || SetClipboardData(UNICODE_TEXT, memory).is_null() {
                let e = error();
                GlobalFree(memory);
                return Err(e);
            }
        }
        Ok(())
    }
}
