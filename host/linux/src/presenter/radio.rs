//! `input type="radio"` (x2apps survey #2), as Chrome runs one: a click
//! checks it and unchecks the rest of its group (`Kernel::radio_group`) at
//! once, firing `input` then `change` with its value; a checked one takes
//! no click. With one focused, the arrows move the focus and the check to
//! `Kernel::radio_step`, and Space checks it. Controlled as the checkbox is:
//! a bound radio draws its `checked` after the commit, so an action that
//! writes nothing snaps the group back; an unbound one keeps its state in
//! `controls`, exclusive within its group.
use super::*;
use exact_kernel::ControlKind;

impl<D: DataSource> Presenter<D> {
    /// Whether `id` is a radio.
    pub(crate) fn is_radio(&self, id: ViewId) -> bool {
        self.host
            .kernel()
            .node(id)
            .is_some_and(|n| ControlKind::of(n.node_type, n.props) == Some(ControlKind::Radio))
    }

    /// A radio's state: its bound `checked`, else the one kept here.
    pub(crate) fn radio_checked(&self, id: ViewId) -> bool {
        self.host
            .kernel()
            .node(id)
            .and_then(|n| n.props.bool(PropId::Checked))
            .or_else(|| self.controls.get(&id).copied())
            .unwrap_or(false)
    }

    /// Check `id`, a radio, as a click does: nothing when it is checked or
    /// disabled; else its group's unbound radios show it alone checked, and
    /// its `input` then `change` carry its value.
    pub(crate) fn check_radio(&mut self, id: ViewId, now_ms: f64) {
        let disabled = self
            .host
            .kernel()
            .node(id)
            .is_none_or(|n| n.props.bool(PropId::Disabled) == Some(true));
        if disabled || self.radio_checked(id) {
            return;
        }
        let kernel = self.host.kernel();
        for member in kernel.radio_group(id) {
            let bound = kernel
                .node(member)
                .is_some_and(|n| n.props.bool(PropId::Checked).is_some());
            if !bound {
                self.controls.insert(member, member == id);
            }
        }
        let value = kernel.radio_value(id);
        self.dirty = true;
        let mut dispatched = false;
        for (event, kind) in [
            (Event::Input(value.as_str().into()), EventKind::Input),
            (Event::Change(value.as_str().into()), EventKind::Change),
        ] {
            if self.host.runner().handlers_of(id).contains(&kind) {
                if let Some(e) = self.host.dispatch_at(id, event, now_ms) {
                    eprintln!("exact: {e}");
                }
                dispatched = true;
            }
        }
        if dispatched {
            if let Some(e) = self.after_commit() {
                eprintln!("exact: {e}");
            }
        }
    }

    /// A key's default action at a focused radio: an arrow moves the focus
    /// and the check to the next enabled radio of its group (Down, Right)
    /// or the previous (Up, Left), wrapping; Space checks it. False when
    /// `id` is no radio or the key is none of these.
    pub(crate) fn radio_key(&mut self, id: ViewId, name: &str, now_ms: f64) -> bool {
        if !self.is_radio(id) {
            return false;
        }
        let forward = match name {
            " " => {
                self.check_radio(id, now_ms);
                return true;
            }
            "ArrowDown" | "ArrowRight" => true,
            "ArrowUp" | "ArrowLeft" => false,
            _ => return false,
        };
        if let Some(next) = self.host.kernel().radio_step(id, forward) {
            if let Some(e) = self.set_focus(Some(next), now_ms) {
                eprintln!("exact: {e}");
            }
            self.check_radio(next, now_ms);
        }
        true
    }

    /// HTML's Tab stop in a radio group: its checked radio, or with none
    /// checked its first enabled one; anything else is a stop of its own.
    pub(crate) fn radio_tab_stop(&self, id: ViewId) -> bool {
        if !self.is_radio(id) {
            return true;
        }
        let kernel = self.host.kernel();
        let group = kernel.radio_group(id);
        match group.iter().find(|r| self.radio_checked(**r)) {
            Some(checked) => *checked == id,
            None => group
                .iter()
                .find(|r| {
                    kernel
                        .node(**r)
                        .is_some_and(|n| n.props.bool(PropId::Disabled) != Some(true))
                })
                .is_none_or(|first| *first == id),
        }
    }
}
