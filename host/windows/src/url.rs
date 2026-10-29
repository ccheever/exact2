//! Leave the app through the user's registered URL handler.
#![allow(unsafe_code)]
use std::ptr;
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
};

pub(super) fn open(window: isize, url: &str) -> Result<(), String> {
    if url.contains('\0') {
        return Err("a URL cannot contain a null character".into());
    }
    let url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    // The presenter admits only http, https, mailto and tel. Pass the URL as
    // the file argument, never as a command line. Winit owns this thread's STA
    // and keeps the parent window alive throughout this callback.
    let result = unsafe {
        ShellExecuteW(
            window as HWND,
            windows_sys::core::w!("open"),
            url.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if result > 32 {
        Ok(())
    } else {
        Err(format!(
            "Windows could not open the link (ShellExecute error {result})"
        ))
    }
}
