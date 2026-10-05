//! Keys, the text focus and system Back for the Canvas host's reader
//! (Android). The reader's view takes the window's key events (a hardware
//! keyboard's, the IME's committed text sent as keys, a driver's injected
//! ones) and hands them here; they go where the display's keyboard goes on
//! Linux ([`Presenter::key`]): the focused node, as a keydown and then its
//! default action (a character typed into a field, Backspace, Enter).
//!
//! While an editable field holds the focus the reader shows the platform's
//! software keyboard ([`CanvasHost::editing`]), as a focused `<input>` does in
//! a browser on Android. System Back presses the shown `id="back"` control
//! (LLP 1038 D6's rule for the selected route's Back), or answers that the
//! app has none, and the activity finishes.
//!
//! @ref LLP 1076 §3.3 (Exact plus Android Canvas)

use super::CanvasHost;
use exact_kernel::{NodeType, PropId};
use exact_runner::DataSource;

impl<D: DataSource + Default> CanvasHost<D> {
    /// A key typed at the focused node: a character, Enter (`'\n'`), or
    /// Backspace (`backspace`). Nothing when nothing holds the focus.
    pub fn key(&mut self, ch: Option<char>, backspace: bool) {
        let now = self.now();
        self.p.key(ch, backspace, now);
        self.force = true;
    }

    /// Whether an editable text field (`input`, `textarea`) holds the focus.
    pub fn editing(&self) -> bool {
        self.p
            .focus
            .and_then(|id| self.p.host().kernel().node(id))
            .is_some_and(|n| {
                n.node_type == NodeType::TextInput
                    && n.props.bool(PropId::Editable) != Some(false)
                    && n.props.bool(PropId::Disabled) != Some(true)
            })
    }

    /// Whether the focused field is a `textarea` (the keyboard offers a
    /// newline rather than a submit).
    pub fn multiline(&self) -> bool {
        self.p
            .focus
            .and_then(|id| self.p.host().kernel().node(id))
            .is_some_and(|n| n.props.str(PropId::SemanticTag) == Some("textarea"))
    }

    /// System Back: press the topmost painted, enabled, interactive control
    /// whose `id` is `back`. Whether there was one (else the app has no
    /// Back to take and the activity goes).
    pub fn back(&mut self) -> bool {
        let now = self.now();
        let ids: Vec<_> = self.p.boxes().iter().rev().map(|b| b.id).collect();
        let host = self.p.host();
        let target = ids.into_iter().find(|&id| {
            host.kernel().node(id).is_some_and(|n| {
                n.props.str(PropId::Id) == Some("back")
                    && n.props.bool(PropId::Disabled) != Some(true)
            }) && !host.route_visibility(id).1
        });
        let Some(id) = target else { return false };
        self.p.dispatch_press(id, now, false);
        self.force = true;
        true
    }
}
