//! @ref LLP 1115.004 D16 — Windows list/details.
//! @ref LLP 1115.002 D4 — the root list stays in the page; details have pane blocks.
use super::*;
use exact_kernel::{BlockAxis, Display, HostBlock, HostCover, Kernel, NodeKey};
use exact_runner::Runner;
use std::collections::BTreeSet;

// Microsoft list/details: stacked at 320–640 epx, beside at 641; SplitView's
// OpenPaneLength defaults to 320. The neutral web shell splits at 900 px.
const THRESHOLD: f32 = if cfg!(windows) { 641. } else { 900. };
const LIST_WIDTH: f32 = 320.;
const MIN_LIST: f32 = 200.;
const MIN_DETAIL: f32 = 320.;
pub(crate) const DIVIDER: f32 = 1.;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Split {
    pub root: ViewId,
    pub within: ViewId,
    pub list: Option<ViewId>,
    pub list_width: f32,
    pub authored_width: Option<f32>,
    /// The containing block's padding box, in page coordinates.
    pub rect: Rect4,
    pub routes: Vec<ViewId>,
    pub detail: Option<ViewId>,
    pub base: Option<ViewId>,
    pub row: Option<ViewId>,
    pub placeholder: Option<ViewId>,
    pub inert: bool,
    pub hidden: bool,
    pub chevrons: BTreeSet<ViewId>,
}
impl Split {
    pub(crate) fn pane(&self) -> Rect4 {
        let (x, y, w, h) = self.rect;
        (
            x + self.list_width + DIVIDER,
            y,
            w - self.list_width - DIVIDER,
            h,
        )
    }
    pub(crate) fn block(&self, id: ViewId) -> bool {
        self.routes.contains(&id) || self.placeholder == Some(id)
    }
    fn blocks(&self) -> impl Iterator<Item = ViewId> + '_ {
        self.routes.iter().copied().chain(self.placeholder)
    }
}

/// A route a list row's action pushed: the base of the pane while it is
/// shown, and the row it marks selected until that row is gone or recycled.
struct Association {
    root: NodeKey,
    list: NodeKey,
    row: Option<NodeKey>,
    route: NodeKey,
    location: String,
}
#[derive(Default)]
pub(crate) struct State {
    pub active: Option<Split>,
    retained: BTreeMap<NodeKey, Option<ViewId>>,
    refused: BTreeMap<NodeKey, (exact_kernel::StyleProps, Rect4)>,
    logged: BTreeSet<NodeKey>,
    actions: BTreeMap<ViewId, Association>,
    blocks: BTreeMap<NodeKey, ViewId>,
    covers: BTreeMap<NodeKey, (ViewId, HostCover)>,
    /// The row a press is being dispatched from, and whether its commits
    /// recycled it before the press's route could be associated with it.
    pressing: Option<(NodeKey, bool)>,
}
impl State {
    pub(crate) fn renewed(&mut self, kernel: &Kernel, keys: &[NodeKey]) {
        let renewed = |row: NodeKey| {
            keys.iter().any(|key| {
                *key == row
                    || kernel.node_by_key(*key).is_some_and(|renewed| {
                        kernel.node_by_key(row).is_some_and(|row| {
                            within(kernel, row.id, renewed.id) || within(kernel, renewed.id, row.id)
                        })
                    })
            })
        };
        if let Some((row, was)) = &mut self.pressing {
            *was |= renewed(*row);
        }
        for a in self.actions.values_mut() {
            if a.row.is_some_and(renewed) {
                a.row = None;
            }
        }
    }

    pub(crate) fn visibility(&self, id: ViewId) -> Option<(bool, bool)> {
        let split = self.active.as_ref()?;
        (id == split.root).then_some((split.hidden, split.inert))
    }
}

pub(crate) struct Press {
    root: NodeKey,
    list: NodeKey,
    row: NodeKey,
    before: Option<(NodeKey, String)>,
}

fn within(kernel: &Kernel, mut id: ViewId, ancestor: ViewId) -> bool {
    loop {
        if id == ancestor {
            return true;
        }
        let Some(parent) = kernel.node(id).and_then(|n| n.parent) else {
            return false;
        };
        id = parent;
    }
}
fn owner<D: DataSource>(runner: &Runner<D>, list: ViewId, mut id: ViewId) -> Option<ViewId> {
    let mut press = None;
    while id != list {
        let node = runner.kernel().node(id)?;
        if runner.press_writes_router(id).is_some() || node.props.str(PropId::Href).is_some() {
            // The nearest owner is what receives this press. An owner above it
            // makes it a nested control, not the list's row.
            if press.is_some() {
                return None;
            }
            press = Some(id);
        }
        id = node.parent?;
    }
    press
}

/// The first row under `at` the app says is current (ARIA reads `""` as
/// `false`), its rows found as `Runner::stack_split` finds them.
fn current_row<D: DataSource>(runner: &Runner<D>, at: ViewId) -> Option<ViewId> {
    for child in runner.kernel().node(at)?.children() {
        let Some(row) = runner.kernel().node(child) else {
            continue;
        };
        if row.style.display == Display::None
            || row.props.str(PropId::Popover).is_some()
            || row.props.str(PropId::NavigationSplit) == Some("placeholder")
        {
            continue;
        }
        let href = row.props.str(PropId::Href).filter(|h| !h.is_empty());
        let current = row
            .props
            .str(PropId::AccessibilityCurrent)
            .is_some_and(|c| !matches!(c, "" | "false"));
        match (runner.press_writes_router(child), href) {
            (Some(_), _) | (None, Some(_)) if current => return Some(child),
            (None, None) => {
                if let Some(row) = current_row(runner, child) {
                    return Some(row);
                }
            }
            _ => {}
        }
    }
    None
}

impl<D: DataSource> Presenter<D> {
    pub(crate) fn capture_split_row(&mut self, target: ViewId) -> Option<Press> {
        let press = self.split_press(target);
        self.host.navigation.split.pressing = press.as_ref().map(|p| (p.row, false));
        press
    }

    fn split_press(&self, target: ViewId) -> Option<Press> {
        let root = *self.host.shown_stack()?.first()?;
        let rule = self.host.runner().stack_split(
            root,
            self.host.kernel().node(root).and_then(|n| {
                self.host
                    .navigation
                    .split
                    .retained
                    .get(&n.key)
                    .copied()
                    .flatten()
            }),
        )?;
        let list = rule.list?;
        let row = owner(self.host.runner(), list, target)?;
        if self.host.runner().press_writes_router(row) != Some(true) {
            return None;
        }
        let kernel = self.host.kernel();
        let before = self.host.shown_stack()?.last().and_then(|top| {
            let location = rule.locations.iter().find(|(id, _)| id == top)?.1.clone();
            Some((kernel.node(*top)?.key, location))
        });
        Some(Press {
            root: kernel.node(root)?.key,
            list: kernel.node(list)?.key,
            row: kernel.node(row)?.key,
            before,
        })
    }

    pub(crate) fn associate_split_row(&mut self, pressed: Option<Press>) {
        let recycled = self
            .host
            .navigation
            .split
            .pressing
            .take()
            .is_some_and(|p| p.1);
        let Some(pressed) = pressed else {
            return;
        };
        let kernel = self.host.kernel();
        let Some(root) = kernel.node_by_key(pressed.root) else {
            return;
        };
        let Some(rule) = self.host.runner().stack_split(root.id, None) else {
            return;
        };
        let Some(top) = self.host.shown_stack().and_then(|s| s.last()).copied() else {
            return;
        };
        let Some((_, location)) = rule.locations.iter().find(|(id, _)| *id == top) else {
            return;
        };
        let Some(route) = kernel.node(top).map(|n| n.key) else {
            return;
        };
        if pressed.before.as_ref() == Some(&(route, location.clone())) {
            return;
        }
        self.host.navigation.split.actions.insert(
            top,
            Association {
                root: pressed.root,
                list: pressed.list,
                row: (!recycled).then_some(pressed.row),
                route,
                location: location.clone(),
            },
        );
    }

    fn read_split(&mut self) -> Option<Split> {
        self.validate_split_rows();
        let stack = self.host.shown_stack()?.to_vec();
        let root = *stack.first()?;
        let kernel = self.host.kernel();
        let node = kernel.node(root)?;
        let key = node.key;
        let within_id = node.parent?;
        let retained = self
            .host
            .navigation
            .split
            .retained
            .get(&key)
            .copied()
            .flatten();
        let rule = self.host.runner().stack_split(root, retained);
        if let Some(rule) = &rule {
            self.host.navigation.split.retained.insert(key, rule.list);
        }
        let rule = rule?;
        if self.host.navigation.bars.room.is_some() || self.separate_surface_windows() {
            return None;
        }
        let kernel = self.host.kernel();
        let parent = kernel.node(within_id)?;
        let [t, r, b, l] = parent.style.border_widths();
        let f = parent.frame;
        let rect = (
            f.x + l,
            f.y + t,
            (f.width - l - r).max(0.),
            (f.height - t - b).max(0.),
        );
        if self
            .host
            .navigation
            .split
            .refused
            .get(&key)
            .is_some_and(|(style, allocation)| {
                kernel.node(root).is_some_and(|n| style == n.style) && *allocation == rect
            })
        {
            return None;
        }
        if rect.2 < THRESHOLD || rect.2 < MIN_LIST + DIVIDER + MIN_DETAIL {
            return None;
        }
        let list_width = rule
            .width
            .unwrap_or(LIST_WIDTH)
            .max(MIN_LIST)
            .min(rect.2 - DIVIDER - MIN_DETAIL);
        let routes: Vec<_> = stack
            .iter()
            .skip(1)
            .copied()
            .take_while(|id| {
                !kernel.node(*id).is_some_and(|n| {
                    matches!(
                        n.props.str(PropId::NavigationPresentation),
                        Some("modal" | "fullscreen")
                    )
                })
            })
            .collect();
        let detail = routes.last().copied();
        let actions = &self.host.navigation.split.actions;
        let linked = |id: ViewId| rule.rows.iter().find(|(r, _)| *r == id).map(|r| r.1);
        let base = routes
            .iter()
            .rev()
            .find(|id| linked(**id).is_some() || actions.contains_key(id))
            .copied()
            .or_else(|| routes.first().copied());
        let row = base
            .and_then(|id| {
                linked(id).or_else(|| {
                    let row = actions.get(&id)?.row?;
                    kernel.node_by_key(row).map(|n| n.id)
                })
            })
            .or_else(|| {
                // A detail the app opened itself (at launch, after Back): no
                // row pushed or links a route in the column.
                let pushed =
                    base.is_some_and(|id| linked(id).is_some() || actions.contains_key(&id));
                current_row(
                    self.host.runner(),
                    rule.list.filter(|_| detail.is_some() && !pushed)?,
                )
            });
        let placeholder = detail
            .is_none()
            .then(|| {
                kernel
                    .preorder_where(&[root], |_, p| {
                        p.str(PropId::NavigationSplit) == Some("placeholder")
                    })
                    .into_iter()
                    .find(|id| {
                        kernel
                            .node(*id)
                            .is_some_and(|n| n.node_type == NodeType::View)
                    })
            })
            .flatten();
        let mut chevrons = BTreeSet::new();
        if let Some(list) = rule.list {
            for id in kernel.preorder_where(&[list], |_, p| {
                matches!(
                    p.str(PropId::ImageSource),
                    Some(
                        "symbol:forward-chevron"
                            | "symbol:sf/chevron.forward"
                            | "symbol:sf/chevron.right"
                    )
                )
            }) {
                if owner(self.host.runner(), list, id).is_some() {
                    chevrons.insert(id);
                }
            }
        }
        Some(Split {
            root,
            within: within_id,
            list: rule.list,
            list_width,
            authored_width: rule.width,
            rect,
            detail,
            base,
            row,
            placeholder,
            inert: stack.len() > routes.len() + 1,
            hidden: stack.iter().skip(routes.len() + 1).any(|id| {
                kernel.node(*id).is_some_and(|n| {
                    n.props.str(PropId::NavigationPresentation) == Some("fullscreen")
                })
            }),
            routes,
            chevrons,
        })
    }

    fn validate_split_rows(&mut self) {
        let live: BTreeSet<_> = self
            .host
            .navigation
            .split
            .retained
            .keys()
            .copied()
            .filter(|key| self.host.kernel().node_by_key(*key).is_some())
            .collect();
        self.host
            .navigation
            .split
            .retained
            .retain(|key, _| live.contains(key));
        self.host
            .navigation
            .split
            .refused
            .retain(|key, _| live.contains(key));
        self.host
            .navigation
            .split
            .logged
            .retain(|key| live.contains(key));
        let runner = self.host.runner();
        let kernel = runner.kernel();
        // A route stays the pane's base while it is shown at the location its
        // row's press gave it; the row stays selected while it is that row.
        let valid: BTreeMap<_, _> = self
            .host
            .navigation
            .split
            .actions
            .iter()
            .filter_map(|(&id, a)| {
                let list = kernel.node_by_key(a.list)?;
                let root = kernel.node_by_key(a.root)?;
                let rule = runner.stack_split(root.id, Some(list.id))?;
                let route = kernel.node(id).is_some_and(|n| n.key == a.route)
                    && rule.list == Some(list.id)
                    && rule
                        .locations
                        .iter()
                        .any(|(r, url)| *r == id && *url == a.location);
                let row = a
                    .row
                    .and_then(|key| kernel.node_by_key(key))
                    .is_some_and(|row| {
                        owner(runner, list.id, row.id) == Some(row.id)
                            && runner.press_writes_router(row.id) == Some(true)
                    });
                route.then_some((id, row))
            })
            .collect();
        let actions = &mut self.host.navigation.split.actions;
        actions.retain(|id, _| valid.contains_key(id));
        for (id, a) in actions.iter_mut() {
            if valid.get(id) == Some(&false) {
                a.row = None;
            }
        }
    }

    pub(super) fn sync_split(&mut self) {
        if let Err(error) = self.apply_split() {
            self.host.log(error);
        }
    }

    fn split_placeholder(
        &mut self,
        id: ViewId,
        show: bool,
    ) -> Result<Option<exact_runner::Timed>, String> {
        let at_ms = self.host.now();
        self.host
            .runner_mut()
            .show_split_placeholder(id, show)
            .map(|r| r.map(|receipt| exact_runner::Timed { at_ms, receipt }))
            .map_err(|e| format!("split placeholder: {e:?}"))
    }

    fn apply_split(&mut self) -> Result<(), String> {
        let next = self.read_split();
        self.apply_split_to(next)
    }

    fn apply_split_to(&mut self, next: Option<Split>) -> Result<(), String> {
        let old = self.host.navigation.split.active.clone();
        let mut changed = old != next;
        let mut receipts = Vec::new();
        let placeholder = next.as_ref().and_then(|s| s.placeholder);
        if let Some(id) = old
            .as_ref()
            .and_then(|s| s.placeholder)
            .filter(|id| Some(*id) != placeholder)
        {
            receipts.extend(self.split_placeholder(id, false)?);
        }
        if let Some(id) = placeholder {
            receipts.extend(self.split_placeholder(id, true)?);
        }
        let kernel = self.host.kernel();
        let blocks: BTreeMap<_, _> = next
            .iter()
            .flat_map(|s| s.blocks())
            .filter_map(|id| Some((kernel.node(id)?.key, id)))
            .collect();
        let mut covers = BTreeMap::new();
        if let Some(s) = &next {
            let root = kernel.node(s.root).ok_or("split list removed")?;
            // The cover takes the route's content in from its right edge to the
            // column's, so authored padding and border stay inside the column.
            // The edge is where an auto width puts it in the containing block,
            // not the route's frame: a cover wider than the box widens it. A
            // width that does not end there is refused below.
            let (env, style) = (kernel.arena().env(), root.style);
            let edge = if root.layout_position() == exact_kernel::PositionType::Absolute {
                s.rect.2
                    - style.right.at_basis(env, s.rect.2)
                    - style.margin_right.at_basis(env, s.rect.2)
            } else {
                let parent = kernel.node(s.within).map(|n| n.key);
                let (pl, _, pr, _) =
                    (parent.and_then(|key| kernel.layout_padding(key))).unwrap_or_default();
                let basis = (s.rect.2 - pl - pr).max(0.);
                pl + basis - style.margin_right.at_basis(env, basis)
            };
            let right = (edge - s.list_width).max(0.);
            covers.insert(root.key, (s.root, HostCover::Edges([0., right, 0., 0.])));
            if let Some(base) = s.base {
                let mut at = Some(s.root);
                let name = loop {
                    let Some(node) = at.and_then(|id| kernel.node(id)) else {
                        break "";
                    };
                    if let Some(name) = node.props.str(PropId::NavigationBack) {
                        break name;
                    }
                    at = node.parent;
                };
                if !name.is_empty() {
                    for key in kernel.find_by_id(name) {
                        if let Some(n) = kernel
                            .node_by_key(key)
                            .filter(|n| within(kernel, n.id, base))
                        {
                            covers.insert(n.key, (n.id, HostCover::Whole));
                        }
                    }
                }
            }
            // A list row's chevron takes no room, as the web's `display: none`
            // has it, so what is beside it reaches the row's end.
            for id in &s.chevrons {
                if let Some(n) = kernel.node(*id) {
                    covers.insert(n.key, (n.id, HostCover::Whole));
                }
            }
        }
        let previous_blocks =
            std::mem::replace(&mut self.host.navigation.split.blocks, blocks.clone());
        let previous_covers =
            std::mem::replace(&mut self.host.navigation.split.covers, covers.clone());
        for (key, id) in previous_blocks {
            if !blocks.contains_key(&key) {
                if self.host.kernel().node_by_key(key).is_some() {
                    self.host
                        .runner_mut()
                        .kernel_mut()
                        .set_host_block(id, None)
                        .map_err(|e| format!("split block: {e:?}"))?;
                }
                self.host.runner_mut().place_blocks(&[(id, None)]);
                changed = true;
            }
        }
        for (key, (id, cover)) in previous_covers {
            if cover == HostCover::Whole
                && self.host.navigation.bars.covers.get(&id) != Some(&HostCover::Whole)
            {
                self.host.navigation.lifted.remove(&id);
            }
            if !covers.contains_key(&key)
                && self.host.kernel().node_by_key(key).is_some()
                && !self.host.navigation.bars.covers.contains_key(&id)
            {
                self.host
                    .runner_mut()
                    .kernel_mut()
                    .set_host_cover(id, None)
                    .map_err(|e| format!("split cover: {e:?}"))?;
                changed = true;
            }
        }
        for (id, cover) in covers.values() {
            let k = self.host.kernel();
            if k.arena().slot_of(*id).and_then(|s| k.arena().cover(s)) != Some(*cover) {
                self.host
                    .runner_mut()
                    .kernel_mut()
                    .set_host_cover(*id, Some(*cover))
                    .map_err(|e| format!("split cover: {e:?}"))?;
                changed = true;
            }
            if *cover == HostCover::Whole {
                self.host.navigation.lifted.insert(*id);
            }
        }
        if let Some(s) = &next {
            let pane = s.pane();
            let block = HostBlock {
                width: BlockAxis::Is(pane.2),
                height: BlockAxis::Is(pane.3),
            };
            for id in blocks.values() {
                if self.host.kernel().host_block(*id) != Some(block) {
                    self.host
                        .runner_mut()
                        .kernel_mut()
                        .set_host_block(*id, Some(block))
                        .map_err(|e| format!("split block: {e:?}"))?;
                    changed = true;
                }
            }
        }
        self.host.navigation.split.active = next.clone();
        self.brush.split_changed(next.clone());
        if !receipts.is_empty() {
            self.dirty = true;
            if let Some(error) = self.host.commit(&receipts, None) {
                return Err(error);
            }
            self.host.navigation.lifted.extend(
                covers
                    .values()
                    .filter_map(|(id, cover)| (*cover == HostCover::Whole).then_some(*id)),
            );
        } else if changed {
            self.dirty = true;
            self.host.layout()?;
        }
        if let Some(s) = next {
            let kernel = self.host.kernel();
            let root = kernel.node(s.root).ok_or("split list removed")?;
            let arena = kernel.arena();
            let cover = match arena.slot_of(s.root).and_then(|t| arena.cover(t)) {
                Some(HostCover::Edges([_, right, _, _])) => right,
                _ => 0.,
            };
            let content = root.frame.x + root.frame.width - cover - s.rect.0;
            if (content - s.list_width).abs() > 0.5 {
                let key = root.key;
                self.host
                    .navigation
                    .split
                    .refused
                    .insert(key, (root.style.clone(), s.rect));
                if self.host.navigation.split.logged.insert(key) {
                    self.host.log(format!(
                        "split: route {} cannot narrow to its {} px column (its content ends at {content} px)",
                        s.root, s.list_width
                    ));
                }
                return self.apply_split_to(None);
            }
        }
        self.place_split();
        Ok(())
    }

    pub(super) fn place_split(&mut self) {
        let Some(s) = &self.host.navigation.split.active else {
            return;
        };
        let Some(parent) = self.host.kernel().node(s.within) else {
            return;
        };
        let pane = s.pane();
        let f = parent.frame;
        let (sx, sy) = self.scroll.get(&s.within).copied().unwrap_or_default();
        let rect = [pane.0 - f.x - sx, pane.1 - f.y - sy, pane.2, pane.3].map(f64::from);
        let placements: Vec<_> = s
            .blocks()
            .map(|id| (id, Some((rect, s.within, None))))
            .collect();
        self.host.runner_mut().place_blocks(&placements);
    }

    pub(crate) fn split_json(&self) -> serde_json::Value {
        let Some(s) = &self.host.navigation.split.active else {
            return serde_json::Value::Null;
        };
        let empty = if s.detail.is_some() {
            None
        } else if s.placeholder.is_some() {
            Some("placeholder")
        } else if cfg!(windows) {
            None
        } else {
            Some("No Selection")
        };
        serde_json::json!({"list": s.root, "listWidth": s.list_width, "detailWidth": s.pane().2,
            "authoredWidth": s.authored_width, "routes": s.routes, "detail": s.detail,
            "base": s.base, "row": s.row, "empty": empty})
    }

    pub(super) fn pane_root(&self, mut id: ViewId) -> Option<ViewId> {
        let split = self.host.navigation.split.active.as_ref()?;
        loop {
            if split.block(id) {
                return Some(id);
            }
            if self.host.kernel().host_block(id).is_some() {
                return None;
            }
            id = self.host.kernel().node(id)?.parent?;
        }
    }

    pub(super) fn pane_frame(&self, id: ViewId) -> Option<exact_kernel::Frame> {
        let mut frame = self.host.kernel().node(id)?.frame;
        if self.pane_root(id).is_some() {
            let pane = self.host.navigation.split.active.as_ref()?.pane();
            frame.x += pane.0;
            frame.y += pane.1;
        }
        Some(frame)
    }

    pub(super) fn geometry_parent(&self, id: ViewId) -> Option<ViewId> {
        if let Some(s) = self
            .host
            .navigation
            .split
            .active
            .as_ref()
            .filter(|s| s.block(id))
        {
            return Some(s.within);
        }
        self.host.kernel().node(id)?.parent
    }

    pub(super) fn scroll_parent(&self, id: ViewId) -> Option<ViewId> {
        if self.display.attached() {
            self.display.scroll_parent(self.host.kernel(), id)
        } else {
            self.geometry_parent(id)
        }
    }
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
