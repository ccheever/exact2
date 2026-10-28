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

    /// A gamepad's state (`Pads.kt`): to the canvas a key would reach.
    pub fn pad(&mut self, index: u8, connected: bool, buttons: [f32; 17], axes: [f32; 4]) {
        let now = self.now();
        self.p.hardware_pad(index, connected, buttons, axes, now);
        self.force = true;
    }

    /// A remote's key by its web names (LLP 1115.001.003 D3): the D-pad's
    /// arrows and Select, down and up. They take the Linux display's
    /// named-key path ([`crate::presenter::Presenter::hardware_key`]).
    pub fn named_key(&mut self, code: &str, key: &str, down: bool, repeat: bool) {
        self.p.hardware_key(code, key, down, repeat);
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

    /// A URL the activity was given while the app runs (`onNewIntent`, or
    /// an activity made again in a running process), as a link's location
    /// (LLP 1133.023 D15 item 3). The one at launch is `EXACT_LAUNCH_URL`.
    pub fn open(&mut self, href: &str) {
        let now = self.now();
        if let Some(error) = self.p.open_location(href, now) {
            eprintln!("exact: {error}");
        }
        self.force = true;
    }

    /// A share the activity was given while the app runs (`onNewIntent`,
    /// LLP 1133.024 D5): `{"target", "share"}`, its files descriptors the
    /// reader opened. The one at launch is `EXACT_SHARE`.
    pub fn share(&mut self, request: &str) {
        let now = self.now();
        let received = serde_json::from_str(request)
            .map_err(|e| e.to_string())
            .and_then(|q| self.p.receive_platform_share(&q, now));
        if let Err(error) = received {
            eprintln!("exact: share: {error}");
        }
        self.force = true;
    }
}
