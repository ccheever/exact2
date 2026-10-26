//! Which stacks a host shows as a list beside its detail (LLP 1115.002 D4;
//! the rule is LLP 1116.004 D1.2's, written once for every host).
//!
//! **A stack splits when a row of its root route's list pushes.** The rule
//! reads the push and never the picture: a row's disclosure chevron is paint
//! (the Mac's table draws none), and what says that a row shows a detail is
//! where its press goes.
//!
//! - *The list* is a `list` in the stack's root route whose rows go down the
//!   page. One that lays its items out sideways (a flex row, a virtualized
//!   row list) is a carousel, and a `list` hidden as authored is none.
//! - *A row's press* is the outermost thing in the row that a press reaches:
//!   a link, or a node that hears `press`. A button inside a row that
//!   presses (a star in a link) is not the row's.
//! - *It pushes* when it is a link to one of the app's routes (its `href`
//!   matches a pattern the route table declares), or when the action its
//!   `press` runs writes the router. An action that writes the router may
//!   also go back or open a sheet: the rule cannot see which, and the author
//!   says `navigationSplit="none"` for a list whose rows only do those.
//! - *`navigationSplit` on the root route* still wins: `"none"` keeps the
//!   stack pushed, and a positive number names the list column's width and
//!   splits a stack whose list this rule does not find.
//!
//! **Whose row a route is.** A route above the root whose location a row of
//! the list links to is that row's detail: the row is the selected one while
//! the route shows, and a route that is some row's detail was not pushed
//! from inside the detail (no Back leads from it to the row's last detail).
//! A row whose `press` is an action names no location; its host knows which
//! row it pressed.
//!
//! When a window is wide enough for two columns, and what the columns look
//! like, are each host's: the Mac splits at every width (LLP 1115.002 D4),
//! the web at 900 px.

use super::{DataSource, Runner, RunnerError};
use crate::instance::InstanceError;
use exact_kernel::{
    CommitReceipt, Display, FlexDirection, NodeType, Op, PropId, PropValue, StyleId, StyleProps,
    ViewId,
};
use exact_plan::{EventKind, NodesId, Plan, Value};

/// The press handler's router write-set fact, shared with web projection.
/// A handler that writes the router may push, go back or open a sheet.
pub fn press_writes_router(plan: &Plan, site: NodesId) -> Option<bool> {
    let handler = plan
        .node(site)
        .handlers
        .iter()
        .map(|h| plan.handler(h))
        .find(|h| h.event == EventKind::Press)?;
    Some(plan.router.is_some_and(|slot| {
        plan.action(handler.action)
            .writes
            .iter()
            .any(|w| plan.write(w).slot == slot)
    }))
}

/// A stack that splits, read from its root route.
#[derive(Debug, Clone, PartialEq)]
pub struct StackSplit {
    /// The list whose row pushes; `None` where only `navigationSplit` asked
    /// for the split.
    pub list: Option<ViewId>,
    /// The list column's width as `navigationSplit` names it, in CSS px.
    pub width: Option<f32>,
    /// Each route above the root in its stack whose location a row of the
    /// list links to, with that row's press: `(route, row)`, bottom to top.
    pub rows: Vec<(ViewId, ViewId)>,
    /// Each route above the root in its stack with its visit's location,
    /// bottom to top: a row's association with a route holds while the
    /// route keeps the location it had, as `replace` keeps a visit's id
    /// (LLP 1127 D3).
    pub locations: Vec<(ViewId, String)>,
}

impl<D: DataSource> Runner<D> {
    /// Whether the stack whose root route is `route` shows that route as a
    /// list beside the route above it (the module's rule), read from the
    /// tree as it now stands. `None` for a stack that pushes, and for a node
    /// that is no stack's root route. A host may retain a previously split
    /// list while it has no actionable rows (static placeholders are allowed);
    /// its current eligibility is still this rule's.
    pub fn stack_split(&self, route: ViewId, retained: Option<ViewId>) -> Option<StackSplit> {
        let node = self.kernel.node(route)?;
        node.props.str(PropId::NavigationKey)?;
        if matches!(
            node.props.str(PropId::NavigationPresentation),
            Some("modal" | "fullscreen")
        ) {
            return None;
        }
        let said = node.props.str(PropId::NavigationSplit);
        if said == Some("none") {
            return None;
        }
        let width = said
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|w| w.is_finite() && *w > 0.0);
        let found = self.pushing_list(route, retained);
        if found.is_none() && width.is_none() {
            return None;
        }
        let (list, links) = match found {
            Some((list, links)) => (Some(list), links),
            None => (None, Vec::new()),
        };
        let locations = self.pane_locations(route);
        // A route whose location a row's link names is that row's detail.
        let rows = locations
            .iter()
            .filter_map(|(route, url)| {
                let (row, _) = links.iter().find(|(_, href)| href == url)?;
                Some((*route, *row))
            })
            .collect();
        Some(StackSplit {
            list,
            width,
            rows,
            locations,
        })
    }

    /// @ref LLP 1116.006 D2 — show `view`, a box written
    /// `navigationSplit="placeholder"`, as a split shows it in its detail
    /// pane while nothing is selected, or hide it again: the lowerer's
    /// `display: none` gives way to the web shell's sheet for it
    /// (`host/web/index.html`). A flex box filling the containing block the
    /// host gives it, with no margin; and, where neither the author nor the
    /// tag set them, a column centred both ways inside 16 px. Shown, these
    /// rows win over the author's, as the shell's `!important` ones do, so a
    /// host asks again after a commit restyles the box. `None` where `view`
    /// is no placeholder or already is as asked.
    pub fn show_split_placeholder(
        &mut self,
        view: ViewId,
        shown: bool,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(node) = self.kernel.node(view) else {
            return Ok(None);
        };
        if node.props.str(PropId::NavigationSplit) != Some("placeholder") {
            return Ok(None);
        }
        let zero = || Value::Number(0.0);
        let mut rows = vec![("display", Value::str(if shown { "flex" } else { "none" }))];
        if shown {
            rows.extend([
                ("position_type", Value::str("absolute")),
                ("box_sizing", Value::str("border-box")),
                ("width", Value::str("auto")),
                ("height", Value::str("auto")),
            ]);
            for side in ["top", "right", "bottom", "left"] {
                rows.push((side, zero()));
            }
            for side in ["margin_top", "margin_right", "margin_bottom", "margin_left"] {
                rows.push((side, zero()));
            }
            // Neither set nor written: a row the author cleared (`unset`) is said.
            let said = node.style.mask.union(node.style.written);
            let unsaid = |name: &str| StyleId::from_name(name).is_some_and(|id| !said.has(id));
            for (name, value) in [
                ("flex_direction", "column"),
                ("align_items", "center"),
                ("justify_content", "center"),
            ] {
                if unsaid(name) {
                    rows.push((name, Value::str(value)));
                }
            }
            for side in [
                "padding_top",
                "padding_right",
                "padding_bottom",
                "padding_left",
            ] {
                if unsaid(side) {
                    rows.push((side, Value::Number(16.0)));
                }
            }
        }
        let mut patch = StyleProps::default();
        for (name, value) in rows {
            let id = StyleId::from_name(name).expect("a kernel style");
            crate::bridge::set_plan_style(&mut patch, id as u16, &value, &self.plan)
                .map_err(|e| RunnerError::from(InstanceError::Bridge(e)))?;
        }
        let mut after = node.style.clone();
        after.apply_patch(&patch);
        if after == *node.style {
            return Ok(None);
        }
        let op = Op::SetStyle {
            id: view,
            patch: Box::new(patch),
        };
        self.apply(vec![op]).map(Some)
    }

    /// The first list under `route`, in tree order, with a row that pushes,
    /// and that list's rows that are links to the app's routes.
    fn pushing_list(
        &self,
        route: ViewId,
        retained: Option<ViewId>,
    ) -> Option<(ViewId, Vec<(ViewId, String)>)> {
        let mut empty = None;
        let mut stack: Vec<ViewId> = self.kernel.node(route)?.children();
        stack.reverse();
        while let Some(id) = stack.pop() {
            let Some(node) = self.kernel.node(id) else {
                continue;
            };
            // Another route's, a popover's, a hidden box's and the split's own
            // placeholder's (shown or not) are not this route's rows.
            if node.props.str(PropId::NavigationKey).is_some()
                || node.props.str(PropId::Popover).is_some()
                || node.props.str(PropId::NavigationSplit) == Some("placeholder")
                || node.style.display == Display::None
            {
                continue;
            }
            if node.node_type == NodeType::List && !sideways(&node) {
                let (mut pushes, mut actionable, mut links) = (false, false, Vec::new());
                self.row_presses(id, &mut pushes, &mut actionable, &mut links);
                if pushes {
                    return Some((id, links));
                }
                if retained == Some(id) && !actionable {
                    empty = Some((id, links));
                }
                continue;
            }
            stack.extend(node.children().into_iter().rev());
        }
        empty
    }

    /// Under `at`, each outermost node a press reaches: whether any pushes,
    /// and those that are links to the app's routes, with their locations.
    fn row_presses(
        &self,
        at: ViewId,
        pushes: &mut bool,
        actionable: &mut bool,
        links: &mut Vec<(ViewId, String)>,
    ) {
        let Some(node) = self.kernel.node(at) else {
            return;
        };
        for child in node.children() {
            let Some(row) = self.kernel.node(child) else {
                continue;
            };
            if row.style.display == Display::None
                || row.props.str(PropId::Popover).is_some()
                || row.props.str(PropId::NavigationSplit) == Some("placeholder")
            {
                continue;
            }
            let press = self.press_writes_router(child);
            let href = row.props.str(PropId::Href).filter(|h| !h.is_empty());
            match (press, href) {
                // A node that hears `press` is pressed, whatever it links to.
                (Some(writes), _) => {
                    *actionable = true;
                    *pushes |= writes;
                }
                (None, Some(href)) => {
                    *actionable = true;
                    if href.starts_with('/') && !href.starts_with("//") && self.route_matches(href)
                    {
                        *pushes = true;
                        links.push((child, href.to_string()));
                    }
                }
                (None, None) => self.row_presses(child, pushes, actionable, links),
            }
        }
    }

    /// For a view that hears `press`, whether the action it runs writes the
    /// router; `None` for a view that hears none.
    pub fn press_writes_router(&self, view: ViewId) -> Option<bool> {
        press_writes_router(&self.plan, self.ids.site(view)?)
    }

    /// The routes after `root` among its parent's children, each with its
    /// visit's location.
    fn pane_locations(&self, root: ViewId) -> Vec<(ViewId, String)> {
        let Some((_, router)) = self.carry_router() else {
            return Vec::new();
        };
        let siblings = self
            .kernel
            .node(root)
            .and_then(|n| n.parent)
            .and_then(|p| self.kernel.node(p))
            .map(|p| p.children())
            .unwrap_or_default();
        let location = |route: ViewId| -> Option<String> {
            let id: u64 = self
                .kernel
                .node(route)?
                .props
                .str(PropId::NavigationKey)?
                .parse()
                .ok()?;
            router
                .tabs
                .iter()
                .flat_map(|tab| tab.stack.iter())
                .find(|e| e.id == id)
                .map(|e| e.url.clone())
        };
        siblings
            .into_iter()
            .skip_while(|id| *id != root)
            .skip(1)
            .filter_map(|route| Some((route, location(route)?)))
            .collect()
    }
}

/// Whether a list lays its items out across the page: a flex row, or one
/// the runner windows by item width.
fn sideways(list: &exact_kernel::NodeRef<'_>) -> bool {
    let row = list.style.display == Display::Flex
        && matches!(
            list.style.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
    row || matches!(
        list.props.get(PropId::EstimatedItemWidth),
        Some(PropValue::Float(w)) if *w > 0.0
    )
}
