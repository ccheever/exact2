//! The binding core on the Apple hosts (LLP 1121 D12, stage 5): what
//! `ExactTamber(view)` asks of a session, by token name, for a view.

use super::Host;
use exact_runner::{DataSource, Place};

impl<D: DataSource> Host<D> {
    /// Answer `{"view": id, "names": [..]}` (`view` 0 or none: the session's
    /// root): `{"inputs", "colors": {name: colour}}`, each colour as a row
    /// crosses it (a role the theme replaces as what replaces it, a system
    /// colour by its name), `null` for a name the plan lacks or a value that
    /// is no colour; `inputs` moves when anything the answer read does
    /// (`observe`). A view that is not live is refused.
    pub fn tamber_json(&mut self, request: &str) -> String {
        let Ok(request) = serde_json::from_str::<serde_json::Value>(request) else {
            return exact_runner::agent::error("tamber: not a JSON request");
        };
        let place = match &request["view"] {
            serde_json::Value::Null => Place::Root,
            view => match view.as_u64().map(u32::try_from) {
                Some(Ok(0)) => Place::Root,
                Some(Ok(id)) if self.runner.kernel().node(id).is_some() => Place::Node(id),
                _ => return exact_runner::agent::error("tamber: `view` is no live view id"),
            },
        };
        let Some(names) = request["names"]
            .as_array()
            .and_then(|n| n.iter().map(|n| n.as_str()).collect::<Option<Vec<&str>>>())
        else {
            return exact_runner::agent::error("tamber: `names` is not a list of token names");
        };
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
