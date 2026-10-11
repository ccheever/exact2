//! Keys and system Back for the Canvas host's reader (Android). The
//! reader's view takes the window's key events (a hardware keyboard's, the
//! IME's committed text sent as keys, a driver's injected ones) and hands
//! them here; they go where the display's keyboard goes on Linux
//! ([`Presenter::key`]): the focused node, as a keydown and then its default
//! action (a character typed into a field, Backspace, Enter).
//!
//! While an editable field holds the focus the reader shows the platform's
//! software keyboard: the presenter tells it so, as a request
//! (`requests.rs`, LLP 1115.001 D5). System Back goes where the presenter's order sends it
//! (LLP 1115.001 D6, [`crate::presenter::Presenter::system_back`]): the
//! navigation root's authored Back control, else its `navigate`, else the
//! runner's own `back`. The reader takes Back only while the presenter says
//! it has somewhere to go, so at the first screen the system's own Back
//! leaves the app; a Back that arrives and is not taken finishes the
//! activity, as it did.
//!
//! @ref LLP 1076 §3.3 (Exact plus Android Canvas)

use super::CanvasHost;
use exact_runner::DataSource;

impl<D: DataSource + Default> CanvasHost<D> {
    /// A key typed at the focused node: a character, Enter (`'\n'`), or
    /// Backspace (`backspace`). Nothing when nothing holds the focus.
    pub fn key(&mut self, ch: Option<char>, backspace: bool) {
        let now = self.now();
        self.p.key(ch, backspace, now);
        self.force = true;
    }

    /// System Back, in the presenter's order (LLP 1115.001 D6). Whether the
    /// app took it; when it did not, it has nowhere to go back to and the
    /// activity goes.
    pub fn back(&mut self) -> bool {
        let now = self.now();
        let taken = self.p.system_back(now);
        self.force |= taken;
        taken
    }
}
