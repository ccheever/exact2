#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod native;

fn main() {
    #[cfg(windows)]
    native::run();
    #[cfg(not(windows))]
    eprintln!("Windows Desk requires Windows.");
}
