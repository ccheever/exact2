//! A held state (LLP 1122 D8, LLP 1121 stage 8 E3): the build channel's
//! `hold` and the agent's `holdState` hold one state on one node as this
//! host's own, on its own tracking, with no handler told. `active` is the
//! press of the node the press resolves to (the innermost at or above it
//! with a `press` handler, as theme2's 07:48 says): its press paint and scale
//! ease as a real press's, at once on the agent's clock, and a real press's
//! release leaves it (`Host::press_feedback`). `hover` and `focus-visible`
//! are that node's look, which the held node keeps beside the pointer's and
//! the focus's (`Shown::on`). One at a time (`Host::looks_shown`, so a new
//! generation's host holds none and nothing is said); a disabled node
//! refuses one. Real input on the node (a press, a key, the pointer coming
//! or going for a hover, the focus coming or going for a focus), its
//! disabling and its going let it go, each said why to the build channel
//! when it was a hold frame's.
use super::*;
use crate::build_channel::HoldTarget;
use crate::host::press::{Held, HoldState};
use serde_json::{json, Value};

/// The press rows a held `active` shows, by wire name (LLP 1061 D7, which
/// `:active` lowers to).
const PRESS_ROWS: [exact_kernel::StyleId; 5] = {
    use exact_kernel::StyleId::*;
    [
        PressScale,
        PressOpacity,
        PressOpacityTo,
        PressBackgroundColor,
        PressColor,
    ]
};

impl<D: DataSource> Presenter<D> {
    /// The state held, if any.
    pub(crate) fn held(&self) -> Option<Held> {
        self.host.looks_shown.held
    }

    /// Hold `state` on `view` (or let any go: `None`), for the hold frame
    /// `revision` (none: the agent's). The state held, or why not: `gone`
    /// (no node, or no press there for `active`) or `disabled`.
    pub(crate) fn hold(
        &mut self,
        view: ViewId,
        state: Option<HoldState>,
        revision: Option<u64>,
    ) -> Result<Option<HoldState>, &'static str> {
        // (The same frame taken again, a resent one or `rehold`'s, replaces nothing.)
        let again = revision.is_some() && self.held().is_some_and(|h| h.revision == revision);
        self.let_hold_go((!again).then_some("replaced"));
        let Some(state) = state else { return Ok(None) };
        let asked = view;
        let kernel = self.host.kernel();
        let mut at = kernel.node(view).map(|n| n.id);
        if state == HoldState::Active {
            while let Some(id) = at {
                let node = kernel.node(id).ok_or("gone")?;
                // A disabled node stops the press there, as a real one's does.
                if node.props.bool(PropId::Disabled) == Some(true) {
                    return Err("disabled");
                }
                let handlers = self.host.runner().handlers_of(id);
                if handlers.contains(&EventKind::Press) {
                    break;
                }
                at = self.display.parent(kernel, id);
            }
        }
        let node = at.and_then(|id| kernel.node(id)).ok_or("gone")?;
        let (view, key) = (node.id, node.key);
        if self.disabled_at(view) {
            return Err("disabled");
        }
        let over = self.pointer_in(view);
        let focused = self.focus.is_some_and(|f| self.within(f, view));
        self.host.looks_shown.held = Some(Held {
            view,
            asked,
            key,
            state,
            revision,
            over,
            focused,
        });
        if state == HoldState::Active {
            let now = self.host.now();
            self.host.press_feedback(key, true, now);
            if self.agent {
                self.host.land_presses();
            }
        }
        self.repaint_looks(vec![view]);
        self.held_feedback();
        Ok(Some(state))
    }

    /// Let the hold go, said why (`None`: unsaid). A held press is taken
    /// off, unless a real press holds the node down still, whose own
    /// release then takes it off.
    pub(super) fn let_hold_go(&mut self, reason: Option<&str>) {
        let Some(held) = self.host.looks_shown.held.take() else {
            return;
        };
        if held.state == HoldState::Active && self.contact_press() != Some(held.key) {
            let now = self.host.now();
            self.host.press_feedback(held.key, false, now);
            if self.agent {
                self.host.land_presses();
            }
        }
        self.repaint_looks(vec![held.view]);
        self.held_feedback();
        let (Some(reason), Some(revision)) = (reason, held.revision) else {
            return;
        };
        let Some(link) = self.dev.as_mut().and_then(|dev| dev.link.as_mut()) else {
            return;
        };
        // Let go, it is not taken again under a new generation, nor when its frame comes again.
        if link.hold.as_ref().is_some_and(|h| h.0 == revision) {
            link.hold = None;
        }
        if reason != "replaced" {
            link.end(revision);
        }
        link.report(json!({"kind": "held", "state": null, "reason": reason, "revision": revision}));
    }

    /// The painted control's pressed look, as the hold now stands.
    fn held_feedback(&mut self) {
        let pressed = self.pressed_feedback();
        if self.brush.feedback.1 != pressed {
            self.brush.feedback.1 = pressed;
            self.dirty = true;
        }
    }

    /// Before the looks are read again: a held node gone (or mounted again)
    /// or disabled lets its hold go, and so does the pointer coming over it
    /// or leaving it for a hover (`at`: what it hovers, after it moved) or
    /// the focus coming or going for a focus.
    pub(super) fn check_hold(&mut self, at: Option<Option<ViewId>>) {
        let Some(mut held) = self.held() else { return };
        let node = self.host.kernel().node_by_key(held.key);
        let gone = node.is_none_or(|n| n.id != held.view)
            || held.state == HoldState::Active && !self.host.pressed(held.key);
        if gone || self.disabled_at(held.view) {
            return self.let_hold_go(Some(if gone { "gone" } else { "disabled" }));
        }
        let over = at.map(|at| at.is_some_and(|id| self.within(id, held.view)));
        let focused = self.focus.is_some_and(|f| self.within(f, held.view));
        let crossed = match held.state {
            HoldState::Hover => over.is_some_and(|o| o != held.over),
            HoldState::FocusVisible => focused != held.focused,
            HoldState::Active => false,
        };
        if crossed {
            return self.let_hold_go(Some("input"));
        }
        held.over = over.unwrap_or(held.over);
        held.focused = focused;
        self.host.looks_shown.held = Some(held);
    }

    /// Real input at `at` (a press's hit, a key's focus): on the held node
    /// or inside it, it lets the hold go.
    pub(crate) fn hold_input(&mut self, at: Option<ViewId>) {
        let held = self.held();
        if held.is_some_and(|h| at.is_some_and(|id| self.within(id, h.view))) {
            self.let_hold_go(Some("input"));
        }
    }

    /// Whether `id` is `outer` or inside it.
    fn within(&self, id: ViewId, outer: ViewId) -> bool {
        let kernel = self.host.kernel();
        let mut at = Some(id);
        while let Some(id) = at {
            if id == outer {
                return true;
            }
            at = kernel.node(id).and_then(|n| n.parent);
        }
        false
    }

    /// Whether the mouse rests over `view` (a finger hovers nothing).
    fn pointer_in(&mut self, view: ViewId) -> bool {
        let hit = match self.hover_point {
            Some((x, y)) if !self.direct() => self.hit(x, y),
            _ => None,
        };
        hit.is_some_and(|id| self.within(id, view))
    }

    /// Whether `view` or a node above it is disabled.
    fn disabled_at(&self, view: ViewId) -> bool {
        let kernel = self.host.kernel();
        let mut at = Some(view);
        while let Some(id) = at {
            let Some(node) = kernel.node(id) else {
                return false;
            };
            if node.props.bool(PropId::Disabled) == Some(true) {
                return true;
            }
            at = node.parent;
        }
        false
    }

    /// The build channel's `hold {revision, hold}`: the one view of this
    /// build's plan at the frame's site and instance path held, or any let
    /// go (null). Reported `held` with the frame's revision: the state held,
    /// or `state: null` and why not; no single view there lets the one held
    /// before go too. The frame is kept, to be taken again after a new
    /// generation of the same plan (`rehold`).
    pub(crate) fn hold_frame(&mut self, revision: u64, target: Option<HoldTarget>) {
        if let Some(link) = self.dev.as_mut().and_then(|dev| dev.link.as_mut()) {
            // A revision ended here is not taken again; a lower one is a new server's.
            if link.ended.iter().any(|&r| r > revision) {
                link.ended.clear();
            }
            if target.is_some() && link.ended.contains(&revision) {
                return;
            }
        }
        let digest = self.host.runner().inspection_digest().to_string();
        // A hold addressing another plan (one installed since) is nothing, unsaid: the server's replay after the
        // status that names this plan corresponds it (and `rehold` takes it should that plan come back).
        let addressed = target.as_ref().and_then(|t| t.plan.clone());
        if let Some(link) = self.dev.as_mut().and_then(|dev| dev.link.as_mut()) {
            link.hold = (target.clone())
                .map(|t| (revision, t, addressed.clone().unwrap_or(digest.clone())));
        }
        if addressed.is_some_and(|plan| plan != digest) {
            return;
        }
        let views = (target.as_ref()).map_or(Vec::new(), |t| self.views_at(t.site, &t.instance));
        let state = target.as_ref().map(|t| HoldState::parse(&t.state));
        let (report, holds) = match (target, state, &views[..]) {
            (Some(_), Some(Some(state)), &[view]) => {
                match self.hold(view, Some(state), Some(revision)) {
                    Ok(_) => (json!({"kind": "held", "state": state.name()}), true),
                    Err(reason) => (
                        json!({"kind": "held", "state": null, "reason": reason}),
                        false,
                    ),
                }
            }
            (target, state, _) => {
                self.let_hold_go(Some("replaced"));
                let report = match (target, state) {
                    (None, _) => json!({"kind": "held", "state": null}),
                    // A state this host does not know is said so by name.
                    (Some(t), Some(None)) => json!({"kind": "unsupported", "what": "hold",
                        "state": t.state, "detail": "this host holds no such state"}),
                    _ => json!({"kind": "held", "state": null, "reason": "gone"}),
                };
                (report, false)
            }
        };
        let Some(link) = self.dev.as_mut().and_then(|dev| dev.link.as_mut()) else {
            return;
        };
        if !holds {
            link.hold = None;
        }
        if report["reason"].is_string() || report["kind"] == "unsupported" {
            link.end(revision);
        }
        let mut report = report;
        report["revision"] = revision.into();
        link.report(report);
    }

    /// After a new generation's host (whose tree holds nothing, unsaid): the
    /// last hold frame taken again under the same plan. Under another the
    /// server replays it, its site corresponded, once the status names it.
    pub(crate) fn rehold(&mut self) {
        let kept = (self.dev.as_ref())
            .and_then(|dev| dev.link.as_ref())
            .and_then(|link| link.hold.clone());
        let Some((revision, target, digest)) = kept else {
            return;
        };
        if self.held().is_none() && digest == self.host.runner().inspection_digest() {
            self.hold_frame(revision, Some(target));
        }
    }

    /// The live views at plan site `site` whose instance path is `instance`
    /// (compared as JSON values: an object's keys in any order).
    fn views_at(&self, site: Option<u64>, instance: &Value) -> Vec<ViewId> {
        let empty = Value::Array(Vec::new());
        let instance = if instance.is_null() { &empty } else { instance };
        let sites: Value =
            serde_json::from_str(&self.host.agent(r#"{"op":"sites"}"#)).unwrap_or_default();
        let at = |pair: &Value| Some((pair[0].as_u64()? as ViewId, pair[1].as_u64()?));
        let mut views: Vec<ViewId> = (sites["sites"].as_array().into_iter().flatten())
            .filter_map(at)
            .filter(|(_, s)| Some(*s) == site)
            .map(|(view, _)| view)
            .filter(|view| {
                let node: Value = serde_json::from_str(
                    &(self.host).agent(&format!(r#"{{"op":"node","id":{view},"plan":true}}"#)),
                )
                .unwrap_or_default();
                node.get("error").is_none()
                    && node["site"].as_u64() == site
                    && node.get("instance").unwrap_or(&empty) == instance
            })
            .collect();
        views.sort_unstable();
        views.dedup();
        views
    }

    /// The agent's `{"op":"tap","id":V,"holdState":…}` (LLP 1012, LLP 1122
    /// D8): the state held on the node as the real one, one hold shared with
    /// the build channel's, or let go (null).
    pub(crate) fn hold_tap(&mut self, id: Option<u32>, state: &Value) -> Value {
        let state = match state {
            Value::Null => None,
            Value::String(s) if HoldState::parse(s).is_some() => HoldState::parse(s),
            _ => {
                return json!({"error": "holdState is active, hover, focus-visible or null",
                    "malformed": true})
            }
        };
        let Some(id) = id else {
            return json!({"error": "tap needs an id", "malformed": true});
        };
        if self.host.kernel().node(id).is_none() {
            return json!({"error": format!("no view {id}")});
        }
        match self.hold(id, state, None) {
            Ok(held) => json!({"tapped": id, "holdState": held.map(HoldState::name),
                "delivery": "recognized"}),
            Err("disabled") => {
                json!({"error": format!("view {id} is disabled: it holds no state")})
            }
            Err(_) => json!({"error": format!("view {id}: no press there, nor any around it")}),
        }
    }

    /// The agent's `layout` (`layout_json`), its node answer carrying
    /// `held {state, rows}` when a state is held on it (LLP 1122 D8): the
    /// press rows the node has for `active`, else its look's rows for that
    /// state, by wire name.
    pub(crate) fn layout_held(&mut self, node: Option<u32>, include_plan: bool) -> String {
        let mut layout = self.layout_json(node, include_plan);
        // (Named on a node inside the press, it answers for that node too.)
        let held = self
            .held()
            .filter(|h| Some(h.view) == node || Some(h.asked) == node);
        let Some(held) = held.filter(|_| layout.ends_with("}}")) else {
            return layout;
        };
        let Some(n) = self.host.kernel().node(held.view) else {
            return layout;
        };
        let value = |row: exact_kernel::StyleId, style: &exact_kernel::StyleProps| {
            let mut out = String::new();
            exact_runner::agent::row_json(row, style.get(row), &mut out);
            json!({"name": row.name(), "value": serde_json::from_str::<Value>(&out).unwrap_or_default()})
        };
        let rows: Vec<Value> = match held.state {
            HoldState::Active => (PRESS_ROWS.iter())
                .filter(|row| n.style.mask.has(**row))
                .map(|row| value(*row, n.style))
                .collect(),
            state => {
                let look = match state {
                    HoldState::Hover => exact_kernel::LOOK_HOVER,
                    _ => exact_kernel::LOOK_FOCUS_VISIBLE,
                };
                let looks = n.looks().iter().filter(|(s, _)| *s == look);
                looks
                    .flat_map(|(_, l)| {
                        (exact_kernel::StyleId::ALL.into_iter())
                            .filter(|row| l.mask.union(l.written).has(*row))
                            .map(|row| value(row, l))
                    })
                    .collect()
            }
        };
        let held = json!({"state": held.state.name(), "rows": rows});
        layout.truncate(layout.len() - 2);
        layout.push_str(&format!(",\"held\":{held}}}}}"));
        layout
    }
}

#[cfg(test)]
#[path = "held_tests.rs"]
mod held_tests;
