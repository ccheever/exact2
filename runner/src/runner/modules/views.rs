//! A package's views hear whether they are displayed, and who made them so
//! (LLP 1133.008 D8 item 2). For each node of a tag the plan's `views` table
//! names, the runner keeps its displayed state, by `wakelock`'s rule
//! (`crate::wakelock::displayed`), and the values of the props its
//! declaration marks as binding a device, each with the origin of the commit
//! that changed it (`admit.rs`, D12). Each change goes to the extension whose
//! implementation makes the view, when the host composes it (bound as in
//! production or under the agent), through the native slot:
//!
//! - `{"extension", "view", "tag", "displayed": bool, "origin"}` for a view
//!   declared `"displayed": true`;
//! - `{"extension", "view", "tag", "bound": prop, "origin"}` for each binding
//!   prop whose value changed; the value itself reaches the view as its props
//!   do.
//!
//! `view` is the node's id, which the host also gives the view's factory, so
//! an implementation keys its views' state by it. A view that leaves the tree
//! while shown is reported hidden. The runner keeps each origin it reported:
//! what a device acquisition that follows is admitted with
//! ([`Runner::view_origin`]), never an origin the implementation passes back.
//! An extension the host binds after a view is shown (its module loads after
//! first pixel) is told each of its views' state then, with the kept origins.

use super::super::{DataSource, Runner};
use super::CallOrigin;
use crate::request::Reply;
use exact_kernel::{PropId, ViewId};
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

/// What the runner last saw of each package view.
#[derive(Default)]
pub(in crate::runner) struct Views {
    /// Each package view node, by id.
    seen: BTreeMap<ViewId, Viewed>,
    /// The origin of the last commit applied: what a change found after it
    /// (the host's covers set as it lays the commit out) is reported with.
    pub(in crate::runner) last: CallOrigin,
}

struct Viewed {
    tag: String,
    extension: String,
    shown: bool,
    /// The origin of the commit that last changed `shown`.
    shown_by: CallOrigin,
    /// Each binding prop's value as JSON text, and the origin of the commit
    /// that set it.
    bound: BTreeMap<String, (String, CallOrigin)>,
}

impl<D: DataSource> Runner<D> {
    /// After a commit is applied: its origin is the one a change it made is
    /// reported with.
    pub(in crate::runner) fn views_committed(&mut self) {
        if self.plan.views.is_empty() && !self.fed_any() {
            return;
        }
        self.extensions.views.last = self.call_origin();
        self.report_views();
    }

    /// Report each package view's change since the last report (module doc).
    /// A host calls this again after it changes what is displayed without a
    /// commit (a cover set as it lays the commit out), as it reads the wake
    /// lock's holders.
    pub fn report_views(&mut self) {
        // A canvas a package feeds, by the same rule (`fed.rs`).
        self.report_fed();
        let none_placed = !self.kernel.has_prop(PropId::NativeViewModuleName);
        if self.plan.views.is_empty() || (none_placed && self.extensions.views.seen.is_empty()) {
            return;
        }
        let origin = self.extensions.views.last;
        let kernel = &self.kernel;
        let plan = &self.plan;
        let found = kernel.preorder_where(&kernel.roots(), |_, props| {
            (props.str(PropId::NativeViewModuleName))
                .is_some_and(|tag| plan.view_named(tag).is_some())
        });
        let mut said = Vec::new();
        let mut now: BTreeMap<ViewId, Viewed> = BTreeMap::new();
        for id in found {
            let Some(node) = kernel.node(id) else {
                continue;
            };
            let tag = node
                .props
                .str(PropId::NativeViewModuleName)
                .unwrap_or_default();
            let Some(row) = plan.view_named(tag) else {
                continue;
            };
            let extension = plan.str(row.extension).to_owned();
            let props: Json = (node.props.str(PropId::NativeViewProps))
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or_default();
            let shown = row.displayed && crate::wakelock::displayed(kernel, &self.scrolled, id);
            let mut was = self
                .extensions
                .views
                .seen
                .remove(&id)
                .filter(|v| v.tag == tag);
            let mut viewed = Viewed {
                tag: tag.to_owned(),
                extension,
                shown: false,
                shown_by: origin,
                bound: BTreeMap::new(),
            };
            if let Some(was) = was.as_mut() {
                viewed.shown = was.shown;
                viewed.shown_by = was.shown_by;
                viewed.bound = std::mem::take(&mut was.bound);
            }
            if row.displayed && viewed.shown != shown {
                viewed.shown = shown;
                viewed.shown_by = origin;
                said.push(json!({ "extension": viewed.extension, "view": id.to_string(), "tag": tag, "displayed": shown, "origin": origin.word() }));
            }
            for prop in plan.str(row.binds).split_whitespace() {
                let value = match &props[prop] {
                    Json::Null => continue,
                    v => v.to_string(),
                };
                if viewed.bound.get(prop).is_some_and(|(v, _)| *v == value) {
                    continue;
                }
                viewed.bound.insert(prop.to_owned(), (value, origin));
                said.push(json!({ "extension": viewed.extension, "view": id.to_string(), "tag": tag, "bound": prop, "origin": origin.word() }));
            }
            now.insert(id, viewed);
        }
        for (id, gone) in std::mem::take(&mut self.extensions.views.seen) {
            if gone.shown {
                said.push(json!({ "extension": gone.extension, "view": id.to_string(), "tag": gone.tag, "displayed": false, "origin": origin.word() }));
            }
        }
        self.extensions.views.seen = now;
        for body in said {
            self.tell_view(body);
        }
    }

    /// The extensions `extensions` were just bound: each is told its views'
    /// state as the runner keeps it, with the kept origins.
    pub(in crate::runner) fn retell_views(&mut self, extensions: &[&str]) {
        let mut said = Vec::new();
        for (id, v) in &self.extensions.views.seen {
            if !extensions.contains(&v.extension.as_str()) {
                continue;
            }
            if v.shown {
                said.push(json!({ "extension": v.extension, "view": id.to_string(), "tag": v.tag, "displayed": true, "origin": v.shown_by.word() }));
            }
            for (prop, (_, by)) in &v.bound {
                said.push(json!({ "extension": v.extension, "view": id.to_string(), "tag": v.tag, "bound": prop, "origin": by.word() }));
            }
        }
        for body in said {
            self.tell_view(body);
        }
    }

    /// The origin the runner reported for view `id`'s displayed state, or for
    /// its binding `prop`: what an acquisition the view makes is admitted
    /// with (LLP 1133.008 D3).
    pub fn view_origin(&self, id: ViewId, prop: Option<&str>) -> Option<CallOrigin> {
        let v = self.extensions.views.seen.get(&id)?;
        match prop {
            Some(prop) => v.bound.get(prop).map(|(_, by)| *by),
            None => v.shown.then_some(v.shown_by),
        }
    }

    /// `state.extensions`' `views` for `extension`: each view, whether it is
    /// displayed, and the origin of the commit that made it so.
    pub(in crate::runner) fn views_json(&self, extension: &str) -> Vec<Json> {
        (self.extensions.views.seen.iter())
            .filter(|(_, v)| v.extension == extension)
            .map(|(id, v)| {
                let bound: serde_json::Map<String, Json> = (v.bound.iter())
                    .map(|(p, (_, by))| (p.clone(), json!(by.word())))
                    .collect();
                json!({ "view": id, "tag": v.tag, "displayed": v.shown, "origin": v.shown_by.word(), "bound": bound })
            })
            .collect()
    }

    fn tell_view(&mut self, body: Json) {
        let extension = body["extension"].as_str().unwrap_or_default();
        if !(self.extensions.bound.contains(extension) || self.extensions.agent.contains(extension))
        {
            return;
        }
        self.log(format!(
            "view {} {}: {} ({} origin)",
            body["tag"].as_str().unwrap_or_default(),
            body["view"].as_str().unwrap_or_default(),
            match &body["displayed"] {
                Json::Bool(shown) => format!("displayed {shown}"),
                _ => format!("bound {}", body["bound"].as_str().unwrap_or_default()),
            },
            body["origin"].as_str().unwrap_or_default()
        ));
        if let Some(handler) = self.native_slot().handler() {
            handler(body.to_string().into_bytes(), Reply::new(|_| {}));
        }
    }
}
