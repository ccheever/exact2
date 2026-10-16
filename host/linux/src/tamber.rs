//! The binding core on the Linux host, and so on Android's Canvas host
//! (LLP 1121 D12, stage 5): what `ExactTamber(view)` asks of a session
//! (`host/android/canvas`, `Tamber.kt`), by token name, for a view.
//!
//! Where the Apple entry hands a colour on for the platform to resolve per
//! view (`host/apple/src/tamber.rs`), this host paints its own colours, so
//! the answer is what the painter would draw: the colour resolved for the
//! place's scheme, under the session's role overrides and the accent its
//! ancestors authored.

use crate::Host;
use exact_runner::{DataSource, Place};

impl<D: DataSource> Host<D> {
    /// Answer `{"view": id, "names": [..]}` (`view` 0 or none: the session's
    /// root): `{"inputs": "<u64>", "colors": {name: ARGB}}`, each colour as
    /// the painter resolves it at that view (its scheme, the nearest
    /// `color-scheme` else the window's; a role as the theme or the platform
    /// gives it) as a signed 32-bit ARGB int, Android's `Color`; `null` for a
    /// name the plan lacks or a value that is no colour. `inputs`, a string
    /// since it is wider than a JSON number holds exactly, moves when
    /// anything the answer read does (`observe`). A view that is not live is
    /// refused, as are names that are not strings.
    pub fn tamber_json(&mut self, request: &str) -> String {
        let Ok(request) = serde_json::from_str::<serde_json::Value>(request) else {
            return exact_runner::agent::error("tamber: not a JSON request");
        };
        let runner = self.runner_mut();
        let place = match &request["view"] {
            serde_json::Value::Null => Place::Root,
            view => match view.as_u64().map(u32::try_from) {
                Some(Ok(0)) => Place::Root,
                Some(Ok(id)) if runner.kernel().node(id).is_some() => Place::Node(id),
                _ => return exact_runner::agent::error("tamber: `view` is no live view id"),
            },
        };
        let Some(names) = request["names"]
            .as_array()
            .and_then(|n| n.iter().map(|n| n.as_str()).collect::<Option<Vec<&str>>>())
        else {
            return exact_runner::agent::error("tamber: `names` is not a list of token names");
        };
        let _roles = runner.bind_roles();
        // A role drawn in the accent reads the nearest `accent-color` above,
        // as the painter's walk binds it at the node.
        let accent = match place {
            Place::Node(id) => runner
                .kernel()
                .node(id)
                .and_then(|n| crate::paint::accent_of(&n)),
            Place::Root => None,
        };
        let _accent = exact_kernel::style::roles::authored_accent(accent);
        let mut tamber = runner.tamber();
        let dark = tamber.dark(place);
        // The accent above is an input here too: an answer resolves under it.
        let inputs = match accent {
            Some(accent) => {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                format!("{accent:?}").hash(&mut h);
                format!("{}-{:x}", tamber.inputs(place), h.finish())
            }
            None => tamber.inputs(place).to_string(),
        };
        let mut out = format!("{{\"inputs\":\"{inputs}\",\"colors\":{{");
        for (i, name) in names.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            exact_runner::agent::quote(name, &mut out);
            out.push(':');
            match tamber.color(name, place) {
                Some(colour) => {
                    let [r, g, b, a] = crate::paint::rgba(colour.resolve(dark));
                    let argb = u32::from_be_bytes([a, r, g, b]) as i32;
                    out.push_str(&argb.to_string());
                }
                None => out.push_str("null"),
            }
        }
        out.push_str("}}");
        out
    }
}
