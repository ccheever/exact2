//! The focus a commit leaves: its `autofocus`, then the route's.
//!
//! **The focus follows the route** (LLP 1115.000 D4.2, and §5's ruling 5 of
//! 2026-10-12: the painted hosts take the web's focus rule). The web's rule
//! is `arrive` in `host/web/input-glue.js`, which both web targets run; this
//! is that rule over the kernel's tree, for every host this presenter
//! paints, and conformance compares the two step by step
//! (`host/web-js/conform.mjs --linux`). A route is a page, so:
//!
//! - on a push (or a replace) the focus goes to the new route's header's
//!   heading, else to the route itself;
//! - on Back it returns to the control whose press pushed the route that
//!   left (the row, the link, the button that opened a sheet), when that
//!   can still take it, else as on a push;
//! - a focus the commit set is left alone: an `autofocus` field, anything
//!   now inside the route, or anywhere that is neither the pressed control
//!   nor where the focus was as the press began. So a route that arrives
//!   with no press never takes the focus from where a person put it;
//! - another tab's stack is no arrival, nor is the first sight of the page;
//! - nothing scrolls, and the ring is as the web's `:focus-visible`: none
//!   after a pointer's press, one after a key's.
//!
//! The stack is the first navigation root's, as the host's navigation last
//! projected it (`Host::shown_stack`). A pointer press is noted at activation
//! (`press_at` or a press-scale contact's lift), a canvas contact at its down,
//! and a key at its press (`typing.rs`); system Back is no press, as the
//! browser's own Back is none. A commit's `focus()` command runs after its
//! commit here (`run_commands`), so it takes the focus from where this put
//! it, which leaves it where the web leaves it.
use super::*;
use std::time::{Duration, Instant};

/// How long after it began a press still explains a route's arrival: the
/// web's two seconds, on the wall's clock as there (an agent's clock moves
/// the app's time, not a person's).
const RECENT: Duration = Duration::from_secs(2);

/// A press, as it began.
#[derive(Debug, Clone, Copy)]
struct Press {
    /// The control pressed: the nearest node at or above the hit that is a
    /// button, a link or a tab stop (the web's `button, a[href],
    /// [tabindex]`).
    control: Option<ViewId>,
    /// Where the focus was.
    focus: Option<ViewId>,
    /// A key's press, which shows the focus it moves; a pointer's does not.
    key: bool,
    at: Instant,
}

/// What the focus's following of the route keeps.
#[derive(Default)]
pub(super) struct State {
    /// The stack as it last stood: its routes, through the one that shows.
    shown: Option<Vec<ViewId>>,
    /// The press that began last, until a route arrives.
    pressed: Option<Press>,
    /// The control whose press pushed each route.
    origins: BTreeMap<ViewId, Option<ViewId>>,
    /// The node this commit took the focus from because its route made it
    /// inert: the page still names it as the focus while its routes change
    /// (a browser blurs an inert element later), so it is what the route's
    /// arrival finds there.
    inert: Option<ViewId>,
}

impl<D: DataSource> Presenter<D> {
    /// The focus leaves a node that is gone, or inert under its route
    /// (`clear_removed_focus` sends its `focusout` first, #283).
    pub(super) fn release_focus(&mut self) {
        let Some(focus) = self.focus else {
            return;
        };
        let live = self.host.kernel().node(focus).is_some();
        if !live || self.host.route_visibility(focus).1 {
            self.focus = None;
            self.dirty = true;
            self.route_focus.inert = live.then_some(focus);
        }
    }

    /// The first `autofocus` node not yet offered that can take the focus
    /// takes it, when nothing has it or a surface's focus handed it on
    /// (LLP 1035.000 D9), with its `focus` and bubbling `focusin` (#283).
    pub(super) fn autofocus(&mut self) -> Option<String> {
        // Only autofocus nodes matter: a walk that keeps those, in
        // preorder, not an export row per node of every mounted row.
        let kernel = self.host.kernel();
        if !self.autofocus_processed.is_empty() {
            self.autofocus_processed.retain(|id| attached(kernel, *id));
        }
        if !kernel.has_prop(PropId::Autofocus) {
            return None;
        }
        let candidates = kernel.preorder_where(&kernel.roots(), |_, props| {
            props.bool(PropId::Autofocus) == Some(true)
        });
        for id in candidates {
            if self.autofocus_processed.contains(&id)
                || !self.focusable(id)
                || self.host.route_visibility(id).1
            {
                continue;
            }
            if !self
                .box_of(id)
                .is_some_and(|b| b.rect.2 > 0.0 && b.rect.3 > 0.0)
            {
                continue;
            }
            self.autofocus_processed.insert(id);
            if self.focus.is_none() || self.focus == self.autofocus_from {
                return self.set_focus(Some(id), self.host.now());
            }
            break;
        }
        None
    }

    /// Whether `id` is what a press presses: a button, a link or a tab
    /// stop, and no field or other control (which a press edits).
    fn presses(&self, id: ViewId) -> bool {
        self.host.kernel().node(id).is_some_and(|n| {
            n.props.get(PropId::TabIndex).is_some()
                || self.focusable(id)
                    && n.node_type != NodeType::TextInput
                    && !matches!(
                        exact_kernel::ControlKind::of(n.node_type, n.props),
                        Some(kind) if kind != exact_kernel::ControlKind::Button
                    )
        })
    }

    /// A press on `hit` begins, a key's or a pointer's, with the focus
    /// where it is now: what a route that arrives next came from.
    pub(crate) fn note_press(&mut self, hit: ViewId, key: bool) {
        let mut control = Some(hit);
        while let Some(id) = control {
            if self.presses(id) {
                break;
            }
            control = self.host.kernel().node(id).and_then(|n| n.parent);
        }
        self.route_focus.pressed = Some(Press {
            control,
            focus: self.focus,
            key,
            at: Instant::now(),
        });
    }

    /// Whether `id` can take the focus back: live, shown, and not disabled.
    fn lands(&self, id: ViewId) -> bool {
        (self.host.kernel().node(id))
            .is_some_and(|n| !crate::surfaces::controls::disabled_control(&n))
            && self.host.route_visibility(id) == (false, false)
    }

    /// The heading of `route`'s header, where it shows: what the focus
    /// goes to as the route arrives.
    fn route_title(&self, route: ViewId) -> Option<ViewId> {
        let kernel = self.host.kernel();
        let header = kernel.node(route)?.children().into_iter().find(|id| {
            kernel
                .node(*id)
                .is_some_and(|n| n.props.str(PropId::SemanticTag) == Some("header"))
        })?;
        kernel
            .preorder_where(&[header], |_, props| props.heading().is_some())
            .into_iter()
            .find(|id| *id != header && self.host.route_visibility(*id) == (false, false))
    }

    /// After a commit's tree and its `autofocus`: the focus follows the
    /// route, where the route moved (the module's rule).
    pub(super) fn arrive(&mut self) {
        let inert = self.route_focus.inert.take();
        let Some(stack) = self.host.shown_stack().map(<[ViewId]>::to_vec) else {
            return;
        };
        let Some(&top) = stack.last() else {
            return;
        };
        let state = &mut self.route_focus;
        let by = state.pressed.filter(|p| p.at.elapsed() < RECENT);
        let Some(was) = state.shown.replace(stack.clone()) else {
            return;
        };
        if was.first() != stack.first() || was.last() == Some(&top) {
            return;
        }
        let kernel = self.host.kernel();
        let left = was.iter().position(|route| *route == top);
        if left.is_none() {
            // A replace keeps where its route came from.
            let from = match was.len() == stack.len() {
                true => was.last().and_then(|r| state.origins.get(r).copied()),
                false => by.map(|p| p.control),
            };
            state.origins.insert(top, from.flatten());
        }
        state.pressed = None;
        let origin = left.and_then(|at| state.origins.get(was.get(at + 1)?).copied().flatten());
        state
            .origins
            .retain(|route, _| kernel.node(*route).is_some());
        if let Some(active) = self.focus.or(inert) {
            let mut inside = Some(active);
            while inside.is_some_and(|id| id != top) {
                inside = inside.and_then(|id| kernel.node(id)?.parent);
            }
            let pressed = by.is_some_and(|p| p.control == Some(active) || p.focus == Some(active));
            if inside.is_some() || !pressed {
                return;
            }
        }
        let to = origin
            .filter(|id| self.lands(*id))
            .or_else(|| self.route_title(top))
            .unwrap_or(top);
        // None after a pointer's press, one after a key's; with no press,
        // as the pointer is: a finger leaves no ring (LLP 1115.001 D7.6).
        let ring = by.map_or(!self.direct(), |p| p.key);
        if !ring || self.brush.ringless == Some(to) {
            self.brush.ringless = (!ring).then_some(to);
        }
        if let Some(e) = self.set_focus(Some(to), self.host.now()) {
            eprintln!("exact: {e}");
        }
    }
}

#[cfg(test)]
#[path = "arrive_tests.rs"]
mod tests;
