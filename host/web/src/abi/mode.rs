//! The window's app modes (LLP 1121 D5): the glue's `prefer mode`.

use super::Bridge;
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// App modes, `axis=value` pins comma-separated in the input buffer
    /// (LLP 1121 D5), set in one batch; after a `?`, only checked.
    pub fn set_mode(&mut self, len: usize) -> u32 {
        let Ok(text) = std::str::from_utf8(&self.input[..len.min(self.input.len())]) else {
            return self.refuse("mode: invalid UTF-8");
        };
        // `?` first: only ask whether they would be taken.
        let (check, text) = match text.strip_prefix('?') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let pins: Option<Vec<(String, String)>> = text
            .split(',')
            .filter(|pin| !pin.is_empty())
            .map(|pin| {
                pin.split_once('=')
                    .map(|(a, v)| (a.to_owned(), v.to_owned()))
            })
            .collect();
        let Some(pins) = pins else {
            return self.refuse("mode: write axis=value pins");
        };
        let pins: Vec<(&str, &str)> = pins.iter().map(|(a, v)| (a.as_str(), v.as_str())).collect();
        let out = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("not booted"),
            |h| match check {
                true => h.check_modes(&pins),
                false => h.set_modes(&pins),
            },
        );
        self.emit(out)
    }
}

impl<D: DataSource> Bridge<D> {
    /// A `tamber-batch` or `tamber-snapshot` frame from the dev stream in the
    /// input buffer (LLP 1121 stage 4): applied in place; returns the batch.
    pub fn design_rows(&mut self, len: usize) -> u32 {
        let Ok(text) = std::str::from_utf8(&self.input[..len.min(self.input.len())]) else {
            return self.refuse("live: invalid UTF-8");
        };
        let text = text.to_owned();
        let out = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("not booted"),
            |h| h.apply_live(&text),
        );
        self.emit(out)
    }
}

/// The app modes' exports, invoked inside [`crate::host!`].
#[macro_export]
macro_rules! mode_exports {
    () => {
        /// App modes, `axis=value` pins in the input buffer (LLP 1121 D5);
        /// returns the batch length.
        #[no_mangle]
        pub extern "C" fn exact_set_mode(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().set_mode(len as usize))
        }

        /// A live design batch or snapshot in the input buffer (LLP 1121
        /// stage 4); returns the batch length. Development builds only.
        #[no_mangle]
        pub extern "C" fn exact_design_rows(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().design_rows(len as usize))
        }
    };
}
