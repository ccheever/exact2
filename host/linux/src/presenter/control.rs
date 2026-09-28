//! Form controls, activated (LLP 1069.001 D4, D7, D9): a press on a
//! checkbox toggles it, as a click does on the web, and reports the new
//! state as HTML's `input` then `change`.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// Toggle `id` if it is an enabled checkbox; false if it is not one. A
    /// bound checkbox draws the committed `checked`, so an action that
    /// refuses the toggle leaves it where it was; an unbound one keeps its
    /// own state here.
    pub(crate) fn toggle_control(&mut self, id: ViewId, now_ms: f64) -> bool {
        let Some(node) = self.host.kernel().node(id) else {
            return false;
        };
        if node.node_type != NodeType::Control {
            return false;
        }
        // A visible file input's press opens its picker (LLP 1069.002 D1).
        if node.props.str(PropId::Type) == Some("file") {
            if node.props.bool(PropId::Disabled) != Some(true) {
                match node.props.str(PropId::Id).map(str::to_owned) {
                    Some(name) => self.show_picker(&name),
                    None => self.host.log("picker: refused: a file input needs an id"),
                }
            }
            return true;
        }
        if node.props.bool(PropId::Disabled) == Some(true) {
            return true;
        }
        let bound = node.props.bool(PropId::Checked);
        let on = !bound
            .or_else(|| self.controls.get(&id).copied())
            .unwrap_or(false);
        if bound.is_none() {
            self.controls.insert(id, on);
        }
        self.dirty = true;
        let mut dispatched = false;
        for (event, kind) in [
            (Event::Input(on.into()), EventKind::Input),
            (Event::Change(on.into()), EventKind::Change),
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
        true
    }
}
