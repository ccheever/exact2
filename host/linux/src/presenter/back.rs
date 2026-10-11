//! System Back, in order (LLP 1115.001 D6; LLP 1115 D5, "Back is always
//! there"): the route's part of it, which is the rule the iOS host follows
//! (`NavigationRules.back`), over the same runner.
//!
//! From the navigation root's selected route, Back goes to the first of:
//!
//! 1. **the authored Back control**: an element of that route whose `id` is
//!    the root's `navigationBack`, that takes `press` and is enabled (the
//!    lowest view id among several). It is pressed. A route that declares
//!    one that does not resolve (disabled, or no `press`) refuses Back while
//!    there is a visit beneath it: the author's say;
//! 2. **the root's `navigate`**, with the location of the visit beneath, for
//!    a route that declares no control;
//! 3. **the runner's own `back`** of the router, only for a root with no
//!    `navigate` handler.
//!
//! With no navigation root, or no visit beneath and no control, nothing
//! takes it, and the shell leaves the app. A node that merely has
//! `id="back"` is not a Back control: the root names it.
//!
//! Before the route come the soft keyboard, which Android closes itself, and
//! the top layer (an open menu, dialog or sheet closes: D6 step 2), which
//! this presenter does not have yet (D10): it attaches at the head of
//! [`Presenter::system_back`]. [`Presenter::back_goes`] is what the shell's
//! back callback will be enabled by, once the host can tell the shell (D5).
use super::*;

/// How Back goes from the selected route.
#[derive(Debug, Clone, PartialEq)]
enum Back {
    /// Press this authored control.
    Press(ViewId),
    /// Deliver this location to this navigation root's `navigate`.
    Navigate(ViewId, String),
    /// The runner's `back` of the router, from this visit.
    Pop(u64),
    /// The route declares a Back control that does not resolve.
    Refused,
}

impl<D: DataSource> Presenter<D> {
    /// The navigation root that shows: the first node in tree order that
    /// carries `navigationBack` (lowering gives the first root with a
    /// `navigationKey` one, empty where the author named no control).
    fn navigation_root(&self) -> Option<ViewId> {
        let kernel = self.host.kernel();
        if !kernel.has_prop(PropId::NavigationBack) {
            return None;
        }
        kernel
            .preorder_where(&self.host.runner().roots(), |_, props| {
                props.str(PropId::NavigationBack).is_some()
            })
            .into_iter()
            .find(|id| !self.host.route_visibility(*id).1)
    }

    /// How Back goes from the selected route now, resolved at use.
    fn back(&self) -> Option<Back> {
        let kernel = self.host.kernel();
        let root = self.navigation_root()?;
        let node = kernel.node(root)?;
        let name = node.props.str(PropId::NavigationBack)?;
        let selected = node.props.str(PropId::NavigationKey)?;
        // The selected route: the route under the root (its child, or a
        // tabpanel's) whose key is the root's.
        let route = kernel
            .preorder_where(&[root], |_, props| {
                props.str(PropId::NavigationKey) == Some(selected)
            })
            .into_iter()
            .find(|id| *id != root)?;
        let within = |mut id: ViewId| loop {
            if id == route {
                break true;
            }
            match kernel.node(id).and_then(|n| n.parent) {
                Some(parent) => id = parent,
                None => break false,
            }
        };
        let declared: Vec<ViewId> = match name.is_empty() {
            true => Vec::new(),
            false => kernel
                .find_by_id(name)
                .iter()
                .filter_map(|key| kernel.node_by_key(*key))
                .map(|n| n.id)
                .filter(|id| within(*id))
                .collect(),
        };
        let control = declared
            .iter()
            .copied()
            .filter(|id| {
                let pressable = (self.host.runner().handlers_of(*id)).contains(&EventKind::Press);
                let disabled = kernel
                    .node(*id)
                    .is_some_and(|n| n.props.bool(PropId::Disabled) == Some(true));
                pressable && !disabled && !self.host.route_visibility(*id).1
            })
            .min();
        if let Some(control) = control {
            return Some(Back::Press(control));
        }
        let visit = selected.parse::<u64>().ok()?;
        let beneath = self
            .host
            .runner()
            .location_beneath(visit)
            .filter(|location| !location.is_empty())?;
        if !declared.is_empty() {
            return Some(Back::Refused);
        }
        let hears = (self.host.runner().handlers_of(root)).contains(&EventKind::Navigate);
        Some(match hears {
            true => Back::Navigate(root, beneath),
            false => Back::Pop(visit),
        })
    }

    /// Whether system Back would be taken now: what a shell's back callback
    /// is enabled by, so that Back with nowhere to go leaves the app with
    /// the system's own animation (D6).
    pub fn back_goes(&self) -> bool {
        self.back().is_some()
    }

    /// System Back at `now_ms`: whether the app took it. `false` is the
    /// shell's to act on (Android finishes the activity).
    pub fn system_back(&mut self, now_ms: f64) -> bool {
        // D6 step 2, when this presenter has a top layer (D10): an open
        // menu, dialog or sheet closes here, before any route is asked.
        let Some(back) = self.back() else {
            return false;
        };
        let error = match back {
            Back::Press(control) => {
                self.dispatch_press(control, now_ms, false);
                None
            }
            Back::Navigate(root, location) => self
                .host
                .dispatch_at(root, Event::Navigate(location), now_ms)
                .or(self.after_commit()),
            Back::Pop(visit) => match self.host.host_back(visit, now_ms) {
                Ok(_) => self.after_commit(),
                Err(error) => Some(error),
            },
            Back::Refused => Some(
                "system Back refused: the route's navigationBack control is disabled or has no press handler"
                    .into(),
            ),
        };
        if let Some(error) = error {
            self.host.log(error);
        }
        true
    }
}

#[cfg(test)]
#[path = "back_tests.rs"]
mod tests;
