// @ref LLP 1030.002 D4 — platform operations keep the same owned-handle boundary.
#[cfg(unix)]
#[path = "directory.rs"]
mod backend;
#[cfg(windows)]
#[path = "directory_windows.rs"]
mod backend;
pub use backend::*;

pub struct Kind {
    pub directory: bool,
    pub regular: bool,
    pub len: u64,
    pub identity: (u64, u64),
}
