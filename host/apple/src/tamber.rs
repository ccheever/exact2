//! The binding core on the Apple hosts (LLP 1121 D12, stage 5): what
//! `ExactTamber(view)` asks of a session, by token name, for a view.

use super::Host;
use exact_runner::{DataSource, Place};

impl<D: DataSource> Host<D> {
    /// Answer `{"view": id, "names": [..]}` (`view` 0 or none: the session's
    /// root): `{"inputs", "colors": {name: colour}}`, each colour as a row
    /// crosses it (a role the theme replaces as what replaces it, a system
    /// colour by its name), `null` for a name the plan lacks; `inputs` moves
    /// when anything the answer read does (`observe`).
    pub fn tamber_json(&mut self, request: &str) -> String {
        let Ok(request) = serde_json::from_str::<serde_json::Value>(request) else {
            return exact_runner::agent::error("tamber: not a JSON request");
        };
        let place = match request["view"].as_u64().unwrap_or(0) {
            0 => Place::Root,
            id => Place::Node(id as exact_kernel::ViewId),
        };
        let names: Vec<&str> = request["names"]
            .as_array()
            .map(|n| n.iter().filter_map(|n| n.as_str()).collect())
            .unwrap_or_default();
        let _roles = self.runner.bind_roles();
        let mut tamber = self.runner.tamber();
        let mut out = format!("{{\"inputs\":\"{}\",\"colors\":{{", tamber.inputs(place));
        for (i, name) in names.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            exact_runner::agent::quote(name, &mut out);
            out.push(':');
            match tamber.color(name, place) {
                Some(colour) => crate::style::push_color_value(&mut out, colour),
                None => out.push_str("null"),
            }
        }
        out.push_str("}}");
        out
    }
}
