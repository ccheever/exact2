//! A canvas a package feeds hears whether it is displayed, and who made it
//! so (LLP 1133.013 D6, D12 item 3: LLP 1133.008 D8 item 2's report, extended
//! from a package's views to a kernel `canvas`). A send that declares
//! `"feeds"` names a surface; from an input's start until the surface's last
//! input stops, the runner keeps whether any canvas of that surface is
//! displayed, by `wakelock`'s rule (`crate::wakelock::displayed`), and the
//! origin of the commit that changed it, and keeps the input's lease:
//!
//! - hidden (`display: none`, a screen pushed over it, not yet mounted), the
//!   input is paused;
//! - shown again by a commit of the app's own code or a person's input, it
//!   resumes; shown by a tool's or a wake's commit, it stays paused, as LLP
//!   1133.008 D3 rules for a viewfinder. A new start takes the state then.
//!
//! The substitute's inputs follow the lease (`samples.rs`). An
//! implementation the host binds, in production or under `--bind`, is told
//! each change through the native slot, and keeps the same rule:
//! `{"extension", "surface", "displayed": bool, "origin"}`. A start on a
//! surface that is hidden is told hidden at once, before the start reaches
//! the implementation.

use super::super::{DataSource, Runner};
use super::{CallOrigin, Json, ModulesRow, Route};
use exact_kernel::ViewId;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(in crate::runner) struct Fed {
    /// Each GPU canvas's surface name, by its kernel view, as published.
    canvases: BTreeMap<ViewId, String>,
    /// Each surface an input feeds, by name.
    surfaces: BTreeMap<String, Surface>,
}

struct Surface {
    extension: String,
    /// The sensors whose inputs run on it.
    sensors: BTreeSet<String>,
    shown: bool,
    /// The origin of the commit that last changed `shown`.
    by: CallOrigin,
    /// The lease: whether its inputs wait for an app's commit to show it.
    paused: bool,
}

impl<D: DataSource> Runner<D> {
    /// The GPU canvases a commit published: their surface names, kept by view.
    pub(in crate::runner) fn fed_canvases(&mut self, updates: &[crate::SurfaceUpdate]) {
        for u in updates {
            (self.extensions.fed.canvases).insert(u.view, u.name.clone());
        }
    }

    /// Whether a live canvas of surface `name` is displayed.
    fn surface_shown(&self, name: &str) -> bool {
        let kernel = &self.kernel;
        (self.extensions.fed.canvases.iter())
            .filter(|(_, n)| *n == name)
            .any(|(v, _)| crate::wakelock::displayed(kernel, &self.scrolled, *v))
    }

    /// A send that `feeds` a canvas, admitted: its input starts or stops on
    /// the surface it names. Before the substitute keeps it, or the host
    /// hands it to the implementation.
    pub(in crate::runner) fn fed_send(
        &mut self,
        row: &ModulesRow,
        args: &Json,
        origin: CallOrigin,
    ) {
        let feeds = self.substitute(row)["feeds"].clone();
        let field = |k: &str| {
            (feeds[k].as_str())
                .and_then(|o| args[o].as_str())
                .unwrap_or_default()
                .to_owned()
        };
        let (surface, sensor) = (field("surface"), field("sensor"));
        if feeds["stop"] == true {
            let surfaces = &mut self.extensions.fed.surfaces;
            if let Some(s) = surfaces.get_mut(&surface) {
                s.sensors.remove(&sensor);
                if s.sensors.is_empty() {
                    surfaces.remove(&surface);
                }
            }
            return;
        }
        let shown = self.surface_shown(&surface);
        let extension = self.plan.str(row.extension).to_owned();
        let fed = &mut self.extensions.fed.surfaces;
        let started = !fed.contains_key(&surface);
        let s = fed.entry(surface.clone()).or_insert_with(|| Surface {
            extension,
            sensors: BTreeSet::new(),
            shown,
            by: origin,
            paused: !shown,
        });
        s.sensors.insert(sensor);
        // A new start takes the canvas as it is now (D6).
        (s.shown, s.paused) = (shown, !shown);
        if started && !shown && self.route(row) != Route::Substitute {
            self.tell_fed(&surface, false, origin);
        }
    }

    /// After a commit, or a host's change to what is displayed: each fed
    /// surface whose displayed state changed takes the lease's rule, and an
    /// implementation is told.
    pub(in crate::runner) fn report_fed(&mut self) {
        if self.extensions.fed.surfaces.is_empty() {
            return;
        }
        let kernel = &self.kernel;
        (self.extensions.fed.canvases).retain(|v, _| kernel.node(*v).is_some());
        let origin = self.extensions.views.last;
        let names: Vec<String> = self.extensions.fed.surfaces.keys().cloned().collect();
        for name in names {
            let shown = self.surface_shown(&name);
            let Some(s) = self.extensions.fed.surfaces.get_mut(&name) else {
                continue;
            };
            if s.shown == shown {
                continue;
            }
            (s.shown, s.by) = (shown, origin);
            s.paused = !shown || (s.paused && origin != CallOrigin::App);
            let paused = s.paused;
            self.log(format!(
                "surface {name}: displayed {shown} ({} origin), its input {}",
                origin.word(),
                if paused { "paused" } else { "resumed" }
            ));
            self.tell_fed(&name, shown, origin);
        }
    }

    /// Whether any input feeds a canvas, so a commit is reported.
    pub(in crate::runner) fn fed_any(&self) -> bool {
        !self.extensions.fed.surfaces.is_empty()
    }

    /// Whether surface `name`'s inputs are paused by the lease.
    pub(in crate::runner) fn fed_paused(&self, name: &str) -> bool {
        self.extensions
            .fed
            .surfaces
            .get(name)
            .is_some_and(|s| s.paused)
    }

    /// `state.extensions`' `surfaces` for `extension`: each surface its
    /// inputs feed, whether it is displayed, by whose commit, and the lease.
    pub(in crate::runner) fn fed_json(&self, extension: &str) -> Option<Json> {
        let surfaces: serde_json::Map<String, Json> = (self.extensions.fed.surfaces.iter())
            .filter(|(_, s)| s.extension == extension)
            .map(|(n, s)| {
                let v = json!({ "displayed": s.shown, "origin": s.by.word(), "paused": s.paused });
                (n.clone(), v)
            })
            .collect();
        (!surfaces.is_empty()).then_some(Json::Object(surfaces))
    }

    /// Tell the implementation that feeds surface `name`, when the host binds one.
    fn tell_fed(&mut self, name: &str, shown: bool, origin: CallOrigin) {
        let Some(s) = self.extensions.fed.surfaces.get(name) else {
            return;
        };
        let extension = s.extension.clone();
        if !(self.extensions.bound.contains(&extension)
            || self.extensions.agent.contains(&extension))
        {
            return;
        }
        let body = json!({ "extension": extension, "surface": name, "displayed": shown, "origin": origin.word() });
        if let Some(handler) = self.native_slot().handler() {
            handler(
                body.to_string().into_bytes(),
                crate::request::Reply::new(|_| {}),
            );
        }
    }
}
