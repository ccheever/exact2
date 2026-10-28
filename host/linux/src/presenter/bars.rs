//! Structure as the platform's bars (LLP 1115.001 D11): where a shell has
//! bars, a route's `header` is Material 3's top app bar and the navigation
//! root's `tablist` its navigation bar. What counts as either is the
//! kernel's rule (`exact_kernel::bars`), which the iOS host reads too; the
//! host takes what a bar stands for out of the page (`crate::bars`); and
//! what each bar looks like is Material's (`bars/look.rs`), never UIKit's.
//!
//! **Who has them.** A shell that shows bars says so
//! ([`Presenter::serve_bars`]: the Canvas host's reader, as it boots).
//! With no such shell (desktop Linux, a headless run, the agent) nothing
//! here runs: the header and the tablist are laid out and painted in the
//! page as written, as they always were.
//!
//! **Where they are.** Not in the page. The top bar is at the window's top
//! under the status bar, whose area is the bar's own colour; the navigation
//! bar is at the window's bottom over the gesture bar, likewise. Each is a
//! picture of its own that the shell shows in a view of its own over the
//! page, as it shows the top layer's surfaces (`top.rs`), so a keyboard
//! that ends the page's viewport does not move the navigation bar: it
//! covers it, as it covers a Material app's. The page's routes are inset by
//! the room the bars take of them ([`Presenter::fit_bars`]), which is none
//! for a navigation bar a keyboard covers.
//!
//! **What crosses.** Out, with each stream and not on the channel, so that
//! a bar and the page under it change in one frame
//! ([`Presenter::reader_bars`]): each bar's frame in the window and its
//! picture, when either is not what the shell was last told. In, one call
//! ([`Presenter::bar_event`]): a touch in a bar at its own points.
//!
//! **What a touch does.** The Up arrow goes Back, in Back's order (D6,
//! [`Presenter::system_back`]). A header's button is pressed as a touch in
//! the page would press it, a popover's invoker included. A destination is
//! pressed too; one tapped again while it is selected and its stack is at
//! its root scrolls that route's content to its top, as iOS does (LLP
//! 1115.000 D11.3).
use super::top::shape;
use super::top::SurfacePainter;
use super::*;

mod look;
pub use look::{Bottom, Button, Tab, Target, Top, What};
use look::{Inputs, Layout};

/// The top app bar, as the shell names it.
pub const TOP: i32 = 1;
/// The navigation bar.
pub const BOTTOM: i32 = 2;

/// A bar as it shows: what it says, how it was measured, and its frame in
/// the window (x, y, width, height; points).
struct Shown<P> {
    plan: P,
    layout: Layout,
    frame: [f32; 4],
}

/// What a bar's picture was made from besides what it says: when none of
/// it changed, a bar that says the same is the same.
#[derive(Debug, Clone, PartialEq)]
struct Seen {
    dark: bool,
    font_scale: f32,
    window: (f32, f32),
    edges: [f32; 4],
    /// The colours a bar is drawn in, which a new palette changes.
    colors: [[u8; 4]; 8],
}

/// What the presenter keeps of its shell's bars.
#[derive(Default)]
pub(super) struct State {
    /// The shell's painter: `Some` on a host whose shell shows bars.
    painter: Option<Box<dyn SurfacePainter>>,
    /// The kernel epoch the bars were last read at, whether the content
    /// had then scrolled, and what else they were measured for.
    read: Option<(u64, bool)>,
    seen: Option<Seen>,
    /// The scroller the top bar's "on scroll" colour follows.
    scroller: Option<ViewId>,
    top: Option<Shown<Top>>,
    bottom: Option<Shown<Bottom>>,
    /// The target a finger holds: the bar, and its place among the targets.
    pressed: Option<(i32, usize)>,
    /// The target a TV remote's focus is on, lit as a held one is (LLP
    /// 1115.001.003 D4): the bar, and its place among the targets.
    focused: Option<(i32, usize)>,
    /// Moved on whenever a bar's picture or frame changes.
    revision: u32,
    /// The revision the shell was last told.
    told: Option<u32>,
}

/// A bar for a driver and a test: what it is and what can be touched.
pub struct Bar<'a> {
    /// Its frame in the window: x, y, width, height, points.
    pub frame: [f32; 4],
    /// What can be touched, each box in the bar's own points.
    pub targets: &'a [Target],
    /// Each text it shows, by the node it stands for, with its box in the
    /// bar's own points.
    pub texts: Vec<(ViewId, Rect4)>,
}

/// Whether a node scrolls up and down by its own saying: its `overflow-y`,
/// or, with none written, its kind (a `scroll`, a `list`). A sideways
/// scroller, whose other axis only follows it, is not one.
fn scrolls_down(node: &exact_kernel::NodeRef<'_>) -> bool {
    match node.style.mask.has(exact_kernel::StyleId::OverflowY) {
        true => matches!(node.style.overflow_y, Overflow::Scroll | Overflow::Auto),
        false => node.node_type.scrolls_by_default(),
    }
}

impl<D: DataSource> Presenter<D> {
    /// A shell shows this presenter's bars from now on, drawn through
    /// `painter` (LLP 1115.001 D11): a route's header and the root's
    /// tablist leave the page for them. A host with no such shell keeps
    /// painting both in the page.
    pub fn serve_bars(&mut self, painter: Box<dyn SurfacePainter>) -> Option<String> {
        self.bars.painter = Some(painter);
        self.fit_bars()
    }

    /// Whether a shell shows bars.
    pub fn has_bars(&self) -> bool {
        self.bars.painter.is_some()
    }

    /// The shell was made again: it is told its bars anew.
    pub(super) fn bars_reset(&mut self) {
        self.bars.told = None;
    }

    /// The window, the system's bars at its top and bottom, and its safe
    /// sides (left, right), with the page's frame in it; a host whose shell
    /// told no window has the viewport and no insets.
    fn bar_window(&self) -> ((f32, f32), [f32; 4], [f32; 4]) {
        let frame = self.chrome().frame;
        match self.window() {
            Some(w) => {
                // A TV's overscan margin too, as the page's (LLP 1115.001.003 D7).
                let overscan = self.tv_margin(w.size);
                let safe: [f32; 4] =
                    std::array::from_fn(|i| w.bars[i].max(w.cutout[i]).max(overscan[i]));
                (w.size, safe, frame)
            }
            None => (self.viewport, [0.0; 4], frame),
        }
    }

    fn bar_font_scale(&self) -> f32 {
        let scale = self.text_scale;
        if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        }
    }

    /// Tell the host the room the bars take of a route, as the window, the
    /// page's place in it and the keyboard now have it: the top bar's 64
    /// under the status bar, less what of that is outside the page; the
    /// navigation bar's height over the gesture bar, less what of it is
    /// under the page's end (all of it, where a keyboard ended the page
    /// above the bar). The tree is laid out again where that changed.
    pub(super) fn fit_bars(&mut self) -> Option<String> {
        self.bars.painter.as_ref()?;
        let (window, safe, frame) = self.bar_window();
        let above = (safe[0] + look::top_height() - frame[1]).max(0.0);
        let nav = look::nav_height(self.bar_font_scale());
        let below = (frame[1] + frame[3] - (window.1 - safe[2] - nav)).max(0.0);
        match self.host.set_bar_room(Some((above, below))) {
            Ok(true) => self.after_commit(),
            Ok(false) => None,
            Err(e) => Some(e),
        }
    }

    /// The route that shows: the one of the root's that is neither hidden
    /// nor inert.
    fn bar_route(&self) -> Option<ViewId> {
        let bars = self.host.bars();
        let live = |id: &ViewId| self.host.route_visibility(*id) == (false, false);
        bars.stacks.iter().flatten().copied().find(live)
    }

    /// A route's content scroller (LLP 1115.000 D11.1, the iOS host's
    /// `contentScroller`): the one its `navigationScroll` names, else the
    /// shallowest node under it that scrolls up and down, in tree order at
    /// each depth, eight levels down at most.
    fn content_scroller(&self, route: ViewId) -> Option<ViewId> {
        let kernel = self.host.kernel();
        let vertical = |id: ViewId| kernel.node(id).is_some_and(|n| scrolls_down(&n));
        let named = kernel
            .node(route)?
            .props
            .str(PropId::NavigationScroll)
            .filter(|name| !name.is_empty());
        if let Some(name) = named {
            return (kernel.find_by_id(name).into_iter())
                .filter_map(|key| kernel.node_by_key(key))
                .map(|n| n.id)
                .find(|id| vertical(*id) && self.is_inside(*id, route));
        }
        // A screen's scroller is near its top: a level wider than a
        // screen's own boxes (a list's rows) holds none worth the walk.
        let mut level = vec![route];
        for _ in 0..8 {
            if let Some(hit) = level.iter().copied().find(|id| vertical(*id)) {
                return Some(hit);
            }
            level = (level.iter())
                .filter_map(|id| kernel.node(*id))
                .filter(|n| n.style.display != exact_kernel::Display::None)
                .flat_map(|n| n.children())
                .collect();
            if level.is_empty() || level.len() > 256 {
                break;
            }
        }
        None
    }

    /// Whether the content has scrolled under the top bar: its route's
    /// scroller is off its top, or, with none, the page itself is.
    fn bar_scrolled(&self) -> bool {
        match self.bars.scroller {
            Some(id) => self.scroll.get(&id).is_some_and(|at| at.1 > 0.5),
            None => self.page.1 > 0.5,
        }
    }

    /// A header's button as the top bar shows it.
    fn bar_button(&self, id: ViewId, dark: bool) -> Option<Button> {
        let kernel = self.host.kernel();
        let node = kernel.node(id)?;
        let row = shape::row(kernel, &node, false);
        // The colour its author wrote on it wins (LLP 1115: author >
        // platform); one it only inherits leaves Material's.
        let _accent = crate::paint::accent_scope(&node);
        let tint = (node.style.written.has(exact_kernel::StyleId::TextColor))
            .then(|| crate::paint::rgba(node.text_color().resolve(dark)));
        Some(Button {
            node: id,
            title: row.title,
            symbol: row.symbol,
            enabled: row.enabled,
            destructive: row.destructive,
            tint,
        })
    }

    /// What the bars say now, from the host's projection: the top bar of
    /// the route that shows, less whether it has scrolled, and the
    /// navigation bar.
    fn read_bars(&self, dark: bool) -> (Option<Top>, Option<Bottom>) {
        let kernel = self.host.kernel();
        let bars = self.host.bars();
        let route = self.bar_route();
        let top = (bars.routes.iter())
            .find(|r| Some(r.route) == route)
            .map(|r| {
                // A readout (`role="status"`, LLP 1075.003 §9.15) is no
                // button, and Material's top app bar has no item for it.
                let buttons = |ids: &[ViewId]| -> Vec<Button> {
                    (ids.iter())
                        .filter(|id| r.shape.items.contains(id))
                        .filter_map(|id| self.bar_button(*id, dark))
                        .collect()
                };
                Top {
                    route: r.route,
                    heading: r.shape.heading,
                    title: kernel.bar_text(r.shape.heading).unwrap_or_default(),
                    up: r.pushed,
                    leading: buttons(&r.shape.leading),
                    trailing: buttons(&r.shape.trailing),
                    scrolled: false,
                }
            });
        let bottom = bars.tabs.as_ref().map(|tabs| Bottom {
            tablist: tabs.tablist,
            tabs: (tabs.tabs.iter())
                .filter_map(|id| {
                    let node = kernel.node(*id)?;
                    let row = shape::row(kernel, &node, false);
                    Some(Tab {
                        node: *id,
                        label: row.title,
                        symbol: row.symbol,
                        selected: node.props.bool(PropId::AccessibilitySelected) == Some(true),
                        enabled: row.enabled,
                    })
                })
                .collect(),
        });
        (top, bottom)
    }

    /// Bring the bars up to date with the tree, the window, the device and
    /// how far the content has scrolled. Nothing is read or measured again
    /// while none of those changed.
    fn sync_bars(&mut self) {
        if self.bars.painter.is_none() {
            return;
        }
        let _roles = self.host.runner().bind_roles(); // The bars' look resolves them.
        let (window, safe, _) = self.bar_window();
        let (dark, font_scale) = (self.dark(), self.bar_font_scale());
        let epoch = self.host.kernel().epoch();
        if self.bars.read.map(|read| read.0) != Some(epoch) {
            // Only a route with a top bar has a scroller the bar follows.
            let route = self.bar_route();
            let barred = (self.host.bars().routes.iter()).any(|r| Some(r.route) == route);
            self.bars.scroller = route
                .filter(|_| barred)
                .and_then(|route| self.content_scroller(route));
        }
        use super::palette::{material, Material};
        use super::top::look::Role;
        let [a, b, c, d] = [
            Role::Surface,
            Role::SurfaceContainer,
            Role::OnSurface,
            Role::Primary,
        ]
        .map(|role| role.color(dark));
        // And those the navigation bar reads by Material's names.
        let [e, f, g, h] = [
            Material::OnSurfaceVariant,
            Material::SecondaryContainer,
            Material::Secondary,
            Material::OnSecondaryContainer,
        ]
        .map(|role| material(role, dark).unwrap_or_default());
        let colors = [a, b, c, d, e, f, g, h];
        let seen = Seen {
            dark,
            font_scale,
            window,
            edges: safe,
            colors,
        };
        let read = (epoch, self.bar_scrolled());
        // The same look as before: a bar that says the same is not
        // measured again, so a commit elsewhere in the tree shapes no text.
        let same = self.bars.seen.as_ref() == Some(&seen);
        if same && self.bars.read == Some(read) {
            return;
        }
        let (top, bottom) = self.read_bars(dark);
        let top = top.map(|top| Top {
            scrolled: read.1,
            ..top
        });
        let inputs = |edge: f32| Inputs {
            dark,
            font_scale,
            width: window.0,
            edge,
            sides: (safe[3], safe[1]),
        };
        let text = self.text.clone();
        let mut changed = false;
        if !same || self.bars.top.as_ref().map(|s| &s.plan) != top.as_ref() {
            self.bars.top = top.map(|plan| {
                let layout = Layout::top(&mut text.borrow_mut(), &plan, &inputs(safe[0]));
                let frame = [0.0, 0.0, layout.size.0, layout.size.1];
                Shown {
                    plan,
                    layout,
                    frame,
                }
            });
            changed = true;
        }
        if !same || self.bars.bottom.as_ref().map(|s| &s.plan) != bottom.as_ref() {
            self.bars.bottom = bottom.map(|plan| {
                let layout = Layout::bottom(&mut text.borrow_mut(), &plan, &inputs(safe[2]));
                let frame = [0.0, window.1 - layout.size.1, layout.size.0, layout.size.1];
                Shown {
                    plan,
                    layout,
                    frame,
                }
            });
            changed = true;
        }
        (self.bars.read, self.bars.seen) = (Some(read), Some(seen));
        if changed {
            // A target measured again is another target: nothing is held.
            self.bars.pressed = None;
            self.bars.focused = None;
            self.bars.revision += 1;
        }
    }

    /// The bars as they now show, for a driver and a test.
    pub fn bars(&mut self) -> (Option<Bar<'_>>, Option<Bar<'_>>) {
        self.sync_bars();
        let top = self.bars.top.as_ref().map(|s| Bar {
            frame: s.frame,
            targets: &s.layout.targets,
            texts: s.layout.texts(),
        });
        let bottom = self.bars.bottom.as_ref().map(|s| Bar {
            frame: s.frame,
            targets: &s.layout.targets,
            texts: s.layout.texts(),
        });
        (top, bottom)
    }

    /// Moved on whenever a bar's picture or frame changes.
    pub(super) fn bars_revision(&mut self) -> u32 {
        self.sync_bars();
        self.bars.revision
    }

    /// What the top bar says, and the navigation bar.
    pub fn bar_plans(&mut self) -> (Option<&Top>, Option<&Bottom>) {
        self.sync_bars();
        (
            self.bars.top.as_ref().map(|s| &s.plan),
            self.bars.bottom.as_ref().map(|s| &s.plan),
        )
    }

    fn bar_targets(&self, which: i32) -> Option<(&[Target], [f32; 4])> {
        match which {
            TOP => (self.bars.top.as_ref()).map(|s| (&s.layout.targets[..], s.frame)),
            BOTTOM => (self.bars.bottom.as_ref()).map(|s| (&s.layout.targets[..], s.frame)),
            _ => None,
        }
    }

    /// What the shell is told of its bars with a stream, when that is not
    /// what it was last told (`None`), in its pixels (`scale` to a point):
    /// a count, then for each bar which it is ([`TOP`], [`BOTTOM`]), its
    /// frame in the window (x, y, width, height), a count of words and
    /// that many: its picture, an op stream of its own, whole. A bar a
    /// statement leaves out is gone.
    pub fn reader_bars(&mut self, scale: f32) -> Option<Vec<i32>> {
        self.bars.painter.as_ref()?;
        self.sync_bars();
        self.sync_bar_focus();
        if self.bars.told == Some(self.bars.revision) {
            return None;
        }
        self.bars.told = Some(self.bars.revision);
        let text = self.text.clone();
        let pressed = self.bars.pressed.or(self.bars.focused);
        let lit = |which: i32| pressed.filter(|p| p.0 == which).map(|p| p.1);
        let px = |v: f32| (v * scale).round() as i32;
        let painter = self.bars.painter.as_mut()?;
        let mut out = vec![0];
        let shown = [
            (TOP, self.bars.top.as_ref().map(|s| (&s.layout, s.frame))),
            (
                BOTTOM,
                self.bars.bottom.as_ref().map(|s| (&s.layout, s.frame)),
            ),
        ];
        for (which, bar) in shown {
            let Some((layout, frame)) = bar else {
                continue;
            };
            let stream = painter.record(layout.size, scale, &mut |backend| {
                layout.paint(backend, &mut text.borrow_mut(), lit(which))
            });
            out[0] += 1;
            out.extend([
                which,
                px(frame[0]),
                px(frame[1]),
                px(frame[2]),
                px(frame[3]),
            ]);
            out.push(stream.len() as i32);
            out.extend(stream.iter().map(|word| *word as i32));
        }
        Some(out)
    }

    /// The tab's route to scroll to its top when its destination is tapped
    /// again: the selected tab's, whose stack is at its root.
    fn reselected_root(&self, tab: ViewId) -> Option<ViewId> {
        let bars = self.host.bars();
        let index = bars.tabs.as_ref()?.tabs.iter().position(|t| *t == tab)?;
        let stack = bars.stacks.get(index)?;
        let shows = self.bar_route()?;
        (stack.len() == 1 && stack[0] == shows).then_some(shows)
    }

    /// A person chose what `what` is. Its press carries no point: the bar is
    /// the shell's, outside the page, and a popover it opens hangs from the
    /// item (`surface_anchor`), as the Mac's toolbar item's (LLP 1021 D2).
    fn bar_choose(&mut self, what: What, now_ms: f64) {
        self.press_point = None;
        match what {
            What::Up => {
                self.system_back(now_ms);
            }
            What::Item(node) => self.activate(node, now_ms),
            What::Tab(node) => {
                // Read before the tab's press, scrolled after it.
                let root = self.reselected_root(node);
                if (self.host.runner().handlers_of(node)).contains(&EventKind::Press) {
                    self.press_then_follow(node, now_ms, true);
                } else if let Some(key) = self.host.kernel().node(node).map(|n| n.key) {
                    self.follow_invocation(key, node, now_ms);
                }
                let scroller = root
                    .filter(|route| self.reselected_root(node) == Some(*route))
                    .and_then(|route| self.content_scroller(route));
                if let Some(id) = scroller {
                    let x = self.scroll.get(&id).map_or(0.0, |at| at.0);
                    self.scroll_to(id, (x, 0.0));
                }
            }
        }
    }

    /// What the shell says happened in bar `which` ([`TOP`], [`BOTTOM`]):
    /// a touch (0 down, 1 up, 2 move, 3 cancel) at (`x`, `y`), points from
    /// the bar's own top left. A target is lit while a finger holds it and
    /// chosen when the finger lifts in it; a finger that leaves it lets go.
    pub fn bar_event(&mut self, which: i32, action: i32, x: f32, y: f32, now_ms: f64) {
        self.sync_bars();
        let at = match which {
            TOP => self.bars.top.as_ref().map(|s| &s.layout),
            BOTTOM => self.bars.bottom.as_ref().map(|s| &s.layout),
            _ => None,
        }
        .and_then(|layout| {
            let index = layout.target_at(x, y)?;
            let target = layout.targets[index];
            target.enabled.then_some((index, target.what))
        });
        let held = self.bars.pressed.filter(|p| p.0 == which).map(|p| p.1);
        let before = self.bars.pressed;
        let mut chosen = None;
        match action {
            0 => self.bars.pressed = at.map(|(index, _)| (which, index)),
            2 if held != at.map(|a| a.0) => self.bars.pressed = None,
            2 => {}
            1 => {
                chosen = at.filter(|(index, _)| held == Some(*index)).map(|a| a.1);
                self.bars.pressed = None;
            }
            _ => self.bars.pressed = None,
        }
        if self.bars.pressed != before {
            self.bars.revision += 1;
            self.executor.notify();
        }
        if let Some(what) = chosen {
            self.bar_choose(what, now_ms);
        }
    }

    /// The bar's target that stands for the authored node `id`: a header's
    /// button or a tab by its own node (or a node inside it), and the Up
    /// arrow by the Back control it stands for on a pushed route (the item
    /// of the header that is in neither of the bar's lists).
    pub(super) fn bar_target(&self, id: ViewId) -> Option<(i32, Target)> {
        let stands = |node: ViewId| node == id || self.is_inside(id, node);
        let up = (self.bars.top.as_ref())
            .filter(|s| s.plan.up)
            .and_then(|s| {
                let bars = self.host.bars();
                (bars.routes.iter()).find(|r| r.route == s.plan.route)
            })
            .is_some_and(|route| {
                let shape = &route.shape;
                (shape.items.iter())
                    .filter(|i| !shape.leading.contains(i) && !shape.trailing.contains(i))
                    .any(|i| stands(*i))
            });
        [TOP, BOTTOM].into_iter().find_map(|which| {
            let (targets, _) = self.bar_targets(which)?;
            (targets.iter().copied())
                .find(|t| match t.what {
                    What::Up => up,
                    What::Item(node) | What::Tab(node) => stands(node),
                })
                .map(|t| (which, t))
        })
    }

    /// Whether `id` shows in a bar and not in the page: the agent's
    /// `reveal` has nothing to scroll into view before a tap on it.
    pub(crate) fn bar_node(&mut self, id: ViewId) -> bool {
        self.sync_bars();
        self.bar_target(id).is_some()
    }

    /// The box in the window, points, of the bar's target that stands for
    /// `id`: where a menu its button opens hangs.
    pub(super) fn bar_box(&mut self, id: ViewId) -> Option<Rect4> {
        self.sync_bars();
        let (which, target) = self.bar_target(id)?;
        let (_, frame) = self.bar_targets(which)?;
        let (x, y, w, h) = target.rect;
        Some((frame[0] + x, frame[1] + y, w, h))
    }

    /// The bars' targets a TV remote can move the focus to (LLP 1115.001.003
    /// D4): each enabled header button and destination, by its authored
    /// node, with its box in the page's coordinates. The Up arrow has no
    /// node; Back does what it does.
    pub(super) fn bar_stops(&mut self) -> Vec<(ViewId, Rect4)> {
        self.sync_bars();
        let page = self.page_origin();
        let mut stops = Vec::new();
        for which in [TOP, BOTTOM] {
            let Some((targets, frame)) = self.bar_targets(which) else {
                continue;
            };
            for t in targets.iter().filter(|t| t.enabled) {
                if let What::Item(node) | What::Tab(node) = t.what {
                    let (x, y, w, h) = t.rect;
                    stops.push((node, (frame[0] + x - page.0, frame[1] + y - page.1, w, h)));
                }
            }
        }
        stops
    }

    /// Light the bar target the focus is on, and tell the shell the bar
    /// again when that changed.
    fn sync_bar_focus(&mut self) {
        let focused = self.focus.filter(|_| self.remote()).and_then(|id| {
            [TOP, BOTTOM].into_iter().find_map(|which| {
                let (targets, _) = self.bar_targets(which)?;
                (targets.iter())
                    .position(|t| matches!(t.what, What::Item(n) | What::Tab(n) if n == id))
                    .map(|index| (which, index))
            })
        });
        if focused != self.bars.focused {
            self.bars.focused = focused;
            self.bars.revision += 1;
        }
    }

    /// The agent's `tap` on a node a bar stands for: its target is chosen,
    /// as a finger lifting in it chooses it. `None` for any other node.
    pub(super) fn bar_tap(&mut self, id: ViewId) -> Option<Result<String, String>> {
        self.sync_bars();
        let (which, target) = self.bar_target(id)?;
        let name = if which == TOP { "top" } else { "bottom" };
        if !target.enabled {
            return Some(Err(format!("view {id} is disabled in the {name} bar")));
        }
        let now = self.host.now();
        self.bar_choose(target.what, now);
        Some(Ok(format!("{{\"tapped\":{id},\"bar\":\"{name}\"}}")))
    }

    /// The bars for the agent's `state`: each one's frame in the window,
    /// what it says, and each target's authored node and box in the window.
    pub fn bars_json(&mut self) -> serde_json::Value {
        self.sync_bars();
        let targets = |shown: &[Target], frame: [f32; 4]| -> Vec<serde_json::Value> {
            (shown.iter())
                .map(|t| {
                    let (x, y, w, h) = t.rect;
                    let (kind, id) = match t.what {
                        What::Up => ("up", serde_json::Value::Null),
                        What::Item(node) => ("item", node.into()),
                        What::Tab(node) => ("tab", node.into()),
                    };
                    serde_json::json!({"kind": kind, "id": id, "enabled": t.enabled,
                        "x": r2(frame[0] + x), "y": r2(frame[1] + y), "w": r2(w), "h": r2(h)})
                })
                .collect()
        };
        let frame = |f: [f32; 4]| serde_json::json!({"x": r2(f[0]), "y": r2(f[1]), "w": r2(f[2]), "h": r2(f[3])});
        let top = self.bars.top.as_ref().map(|s| {
            serde_json::json!({"route": s.plan.route, "title": s.plan.title, "heading": s.plan.heading,
                "up": s.plan.up, "scrolled": s.plan.scrolled, "frame": frame(s.frame),
                "targets": targets(&s.layout.targets, s.frame)})
        });
        let bottom = self.bars.bottom.as_ref().map(|s| {
            let tabs: Vec<_> = (s.plan.tabs.iter())
                .map(
                    |t| serde_json::json!({"id": t.node, "label": t.label, "selected": t.selected}),
                )
                .collect();
            serde_json::json!({"tablist": s.plan.tablist, "tabs": tabs, "frame": frame(s.frame),
                "targets": targets(&s.layout.targets, s.frame)})
        });
        serde_json::json!({"top": top, "bottom": bottom})
    }
}

#[cfg(test)]
#[path = "bars_tests.rs"]
mod tests;
