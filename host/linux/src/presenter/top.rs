//! The top layer (LLP 1115.001 D10): on a host whose shell makes windows, a
//! confirmation and a menu are separate windows of the platform's, with
//! their content painted here. On Android an alert-shaped `alertdialog` is
//! Material 3's basic dialog in a `Dialog` window and a menu popover is
//! Material 3's menu in a `PopupWindow` (`host/android/README.md`).
//!
//! **Who has one.** A shell that makes windows says so
//! ([`Presenter::serve_windows`]: the Canvas host's reader, as it boots).
//! Windows draws an alert-shaped dialog and a menu as WinUI's ContentDialog
//! and MenuFlyout over the window (`top/overlay.rs`). Every other `dialog`
//! and popover on Windows, and every one on the Linux desktop, is the
//! author's subtree, laid out by the kernel and painted in the window
//! (`alert.rs`, LLP 1115.004 D10.7); Android refuses those for now.
//!
//! **How a surface is painted.** Not by the kernel's layout of the authored
//! rows, which no native host shows either. The shape rule (`top/shape.rs`)
//! reads what the subtree says (a title, a message, the buttons; a menu's
//! items), `top/look.rs` measures and paints Material's component from it,
//! and the shell's painter records that as an op stream of its own, whole
//! each time, which the shell's window replays. One surface is open at a
//! time, and it is host state: opening one commits nothing.
//!
//! **What crosses.** Out, on the one channel (`requests.rs`, kind 7): the
//! surface as a statement, told when it is not what the shell was last told
//! (it opened, a press lit a row, the scheme or the font scale changed, it
//! closed), with its size, its place and its stream. In, one call
//! ([`Presenter::surface_event`]): a touch in the window at the surface's
//! own points, a key, that the system dismissed the window (a tap outside a
//! menu, Back, Escape), or that the shell could not make it.
//!
//! **Its behaviour.** While a surface is open the page is inert: nothing of
//! it is hit, and keys go to the surface. The focus moves in (the page's
//! node loses it) and returns, when it closes, to the node that held it. A
//! row's tap closes the surface and then runs the row's `press`; an alert's
//! cancel closes it and runs nothing; Back and Escape are a close request,
//! which fires a modal dialog's `cancel` first (HTML) and closes it unless
//! that was prevented; a tap outside closes a menu, and does nothing to an
//! alert, which has no outside (LLP 1115.000 D13.4), unless its author wrote
//! `closedby="any"`: then it dismisses it, as it does a Material dialog left
//! to itself. System Back closes it before any route is asked
//! ([`Presenter::system_back`]).
use super::*;
use exact_kernel::invocation::Action;
use exact_kernel::NodeKey;

pub(super) mod area;
pub(super) mod look;
mod overlay;
pub(crate) mod shape;
pub use look::Target;
use look::{Inputs, Layout};
pub use shape::{Plan, Row};

/// What a shell's windows are drawn from: it records what `paint` draws
/// for a surface of `size` points at `scale` pixels a point, and returns
/// the words its window replays. A recording is whole: it defines every
/// face it draws with.
pub trait SurfacePainter {
    /// Record one surface's picture.
    fn record(
        &mut self,
        size: (f32, f32),
        scale: f32,
        paint: &mut dyn FnMut(&mut dyn Backend),
    ) -> Vec<u32>;
}

/// What a surface is, and the window its shell makes for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    /// Material's basic dialog, in a dialog window: modal, over a scrim.
    Alert = 1,
    /// Material's menu, in a popup window at its invoker.
    Menu = 2,
}

/// The top layer's surface as its shell is told it (request kind 7).
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    /// Names this opening. Never reused; what the shell says back names it.
    pub serial: u32,
    /// What to show, or `None`: this surface closed.
    pub kind: Option<SurfaceKind>,
    /// Bit 0: Back and Escape dismiss it. Bit 1: a tap outside dismisses it.
    /// Bits 2 and 3: the corner of a menu its motion grows from, the one
    /// at its anchor (bit 2 its right side, bit 3 its bottom).
    pub flags: u32,
    /// Where its window goes in the page's window, and its size, in pixels
    /// (x, y, width, height). An alert's place is the platform's: centred.
    pub frame: [i32; 4],
    /// Its container's corner radius and its elevation, in pixels.
    pub shape: (f32, f32),
    /// Its picture, as the shell's painter recorded it.
    pub stream: Vec<u32>,
}

impl Surface {
    /// The words after a request's count: the serial, what it is (0 closed,
    /// 1 an alert, 2 a menu), the flags, the frame, the radius and the
    /// elevation as float bits, then the stream's count and its words.
    pub(super) fn encode(&self, out: &mut Vec<i32>) {
        let what = self.kind.map_or(0, |kind| kind as i32);
        out.extend([self.serial as i32, what, self.flags as i32]);
        out.extend(self.frame);
        out.extend([self.shape.0.to_bits() as i32, self.shape.1.to_bits() as i32]);
        out.push(self.stream.len() as i32);
        out.extend(self.stream.iter().map(|word| *word as i32));
    }
}

/// What the shell says happened in a surface's window, beside a touch
/// (0 down, 1 up, 2 move, 3 cancel): the numbers [`Presenter::surface_event`]
/// takes.
pub mod event {
    /// The system dismissed the window: a tap outside a menu, Back, Escape.
    pub const DISMISSED: i32 = 8;
    /// The shell could not make the window.
    pub const REFUSED: i32 = 9;
    /// Enter, on a hardware keyboard.
    pub const ENTER: i32 = 16;
    /// Tab, and Shift+Tab.
    pub const TAB: i32 = 17;
    /// See `TAB`.
    pub const TAB_BACK: i32 = 18;
    /// The arrows: up, down, left, right.
    pub const UP: i32 = 19;
    /// See `UP`.
    pub const DOWN: i32 = 20;
    /// See `UP`.
    pub const LEFT: i32 = 21;
    /// See `UP`.
    pub const RIGHT: i32 = 22;
    /// Space.
    pub const SPACE: i32 = 23;
}

/// The surface that is open.
struct Open {
    serial: u32,
    /// Moved on whenever its picture or its place changes.
    revision: u32,
    key: NodeKey,
    node: ViewId,
    kind: SurfaceKind,
    /// Opened by `show-modal`, not as a popover: HTML's popover commands
    /// leave it open, and its dialog `close` closes only this mode.
    modal: bool,
    /// What opened it: where a menu hangs.
    invoker: Option<ViewId>,
    /// Where a context menu was asked for, viewport points.
    point: Option<(f32, f32)>,
    /// The node that held the focus before it opened.
    restore: Option<ViewId>,
    plan: Plan,
    inputs: Inputs,
    /// A menu's anchor in the window, points.
    anchor: Rect4,
    layout: Layout,
    /// The shell's frame in window pixels, its display scale and motion origin.
    frame: [i32; 4],
    scale: f32,
    origin: u32,
    /// The row under the finger, and the row a keyboard focused.
    pressed: Option<usize>,
    focused: Option<usize>,
    hovered: Option<usize>,
    /// The kernel epoch its plan was read at.
    epoch: u64,
}

impl Open {
    /// Whether a tap outside dismisses it: a menu always, an alert when its
    /// `closedby` is `any`.
    fn outside(&self) -> bool {
        match &self.plan {
            Plan::Alert { outside, .. } => *outside,
            Plan::Menu(_) => true,
        }
    }
}

/// What the presenter keeps of the top layer.
#[derive(Default)]
pub(super) struct State {
    overlay: overlay::State,
    /// The shell's painter: `Some` on a host whose shell makes windows.
    painter: Option<Box<dyn SurfacePainter>>,
    open: Option<Open>,
    toggles: Vec<(NodeKey, bool, bool)>,
    closes: Vec<NodeKey>,
    /// The last serial given.
    last: u32,
    /// The surface the shell shows as it was last told: serial and revision.
    told: Option<(u32, u32)>,
    /// How many long presses a node has taken ([`Presenter::long_press`]):
    /// a reader that compares it across a contact knows whether its own was.
    long_presses: u32,
}

/// The open surface, for a driver and a test.
pub struct Shown<'a> {
    /// Its serial.
    pub serial: u32,
    /// Changes when the surface's presentation or item geometry changes.
    pub revision: u32,
    /// What it is.
    pub kind: SurfaceKind,
    /// The authored `dialog` or popover.
    pub node: ViewId,
    /// What it says.
    pub plan: &'a Plan,
    /// Its size, points.
    pub size: (f32, f32),
    /// The visible content of an overflowing desktop menu.
    pub clip: Option<Rect4>,
    /// The menu's current and maximum vertical scroll offsets.
    pub scroll: Option<(f32, f32)>,
    /// Its rows' boxes in the window, in the plan's order: how a node
    /// inside a window is addressed.
    pub targets: &'a [Target],
    /// The row a finger holds, if one does.
    pub pressed: Option<usize>,
    /// Each text it shows, by the node it stands for (an alert's headline
    /// and supporting text by the surface's own), with its box, points.
    pub texts: Vec<(ViewId, Rect4)>,
}

impl<D: DataSource> Presenter<D> {
    pub(super) fn next_surface_serial(&mut self) -> u32 {
        self.top.last += 1;
        self.top.last
    }

    /// A shell makes this presenter's top layer as windows from now on,
    /// drawn through `painter` (LLP 1115.001 D10). A host with no such
    /// shell keeps what it did: no top layer.
    pub fn serve_windows(&mut self, painter: Box<dyn SurfacePainter>) {
        self.top.painter = Some(painter);
        self.host.top_layer(true);
    }

    /// Whether a shell makes windows.
    pub(crate) fn has_windows(&self) -> bool {
        self.top.painter.is_some() || self.top.overlay.enabled
    }

    pub(super) fn separate_surface_windows(&self) -> bool {
        self.top.painter.is_some()
    }

    pub(super) fn desktop_overlays(&self) -> bool {
        self.top.overlay.enabled
    }

    /// The surface that is open, if one is.
    pub fn surface(&self) -> Option<Shown<'_>> {
        self.top.open.as_ref().map(|open| Shown {
            serial: open.serial,
            revision: open.revision,
            kind: open.kind,
            node: open.node,
            plan: &open.plan,
            size: open.layout.size,
            clip: open.layout.menu_clip(),
            scroll: open.layout.menu_scroll(),
            targets: &open.layout.targets,
            pressed: open.pressed,
            texts: open.layout.texts(),
        })
    }

    /// Whether a surface is open: the page is then inert.
    pub(crate) fn top_open(&self) -> bool {
        self.shape_surface_open() || self.alert.is_some()
    }

    pub(super) fn shape_surface_open(&self) -> bool {
        self.top.open.is_some()
    }

    /// The host was replaced (a reload): its tree is another, and the new
    /// one is told again whether a top layer is presented.
    pub(super) fn top_replaced(&mut self) {
        self.alert = None;
        self.brush.alert = None;
        self.host.top_layer(self.has_windows());
        if self.top.open.take().is_some() {
            self.surface_changed();
        }
    }

    /// The shell was made again: it is told the open surface anew.
    pub(super) fn surface_reset(&mut self) {
        self.top.told = None;
    }

    fn surface_changed(&mut self) {
        self.dirty |= self.top.overlay.enabled;
        self.report_top_layer();
        self.back_changed();
        self.holds_changed();
        // The shell comes for the statement whether or not a frame follows.
        self.executor.notify();
    }

    /// What `pop` is to the top layer, shown modally or as a popover: an
    /// alert, a menu, or nothing it shows.
    fn surface_plan(&self, pop: ViewId, modal: bool) -> Option<Plan> {
        let runner = self.host.runner();
        let pressed = |id: ViewId| runner.handlers_of(id).contains(&EventKind::Press);
        let kernel = self.host.kernel();
        shape::alert(kernel, pop, &pressed, modal).or_else(|| {
            if self.top.overlay.enabled {
                let role = kernel.node(pop)?.props.str(PropId::AccessibilityRole);
                let menu = role == Some("menu")
                    || role.is_none()
                        && kernel
                            .preorder_where(&[pop], |kind, _| kind == NodeType::TextInput)
                            .is_empty();
                if !menu {
                    return None;
                }
            }
            shape::menu(kernel, pop, &pressed)
        })
    }

    /// The window the shell told, and how far a menu keeps from its top and
    /// bottom: Material's 48, or the system's bars where they are taller.
    fn surface_window(&self) -> ((f32, f32), (f32, f32)) {
        if self.top.overlay.enabled {
            return (self.viewport, (4.0, 4.0));
        }
        match self.window() {
            Some(w) => {
                let inset = |i: usize| w.bars[i].max(w.cutout[i]).max(48.0);
                (w.size, (inset(0), inset(2)))
            }
            None => (self.viewport, (48.0, 48.0)),
        }
    }

    fn surface_inputs(&self) -> Inputs {
        let scale = self.text_scale;
        Inputs {
            dark: self.dark(),
            contrast: self.colors.contrast.palette,
            font_scale: if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            },
            window: self.surface_window().0,
        }
    }

    /// Where the open menu hangs, in the window's points: the point a
    /// context menu was asked at, else its invoker's box as last painted.
    fn surface_anchor(&mut self) -> Rect4 {
        let Some((invoker, point)) = self.top.open.as_ref().map(|o| (o.invoker, o.point)) else {
            return (0.0, 0.0, 0.0, 0.0);
        };
        let [px, py, ..] = self.chrome().frame;
        if let Some((x, y)) = point {
            return (px + x, py + y, 0.0, 0.0);
        }
        // A button its shell's top bar shows hangs its menu from the bar.
        if let Some(at) = invoker.and_then(|id| self.bar_box(id)) {
            return at;
        }
        let b = invoker.and_then(|id| self.box_of(id));
        b.map_or((px, py, 0.0, 0.0), |b| {
            (px + b.rect.0, py + b.rect.1, b.rect.2, b.rect.3)
        })
    }

    /// How `target` is open: `Some(true)` modally, `Some(false)` as a
    /// popover, `None` when it is not the open surface.
    fn open_mode(&self, target: NodeKey) -> Option<bool> {
        self.alert_mode(target).or_else(|| {
            (self.top.open.as_ref())
                .filter(|open| open.key == target)
                .map(|open| open.modal)
        })
    }

    /// The element an action's command names by `id`, by a button's rule
    /// (`exact_kernel::invocation::element`).
    pub(super) fn element(&self, name: &str) -> Option<NodeKey> {
        exact_kernel::invocation::element(self.host.kernel(), name)
    }

    /// Present `target`, the dialog or popover `name` names, modally or as a
    /// popover: whether it was. A menu is never presented modally.
    pub(super) fn present(
        &mut self,
        target: Option<NodeKey>,
        name: &str,
        invoker: Option<ViewId>,
        point: Option<(f32, f32)>,
        modal: bool,
        now_ms: f64,
    ) -> bool {
        // Its session's roles (LLP 1121 stage 5): the surface's look resolves them.
        let _roles = self.host.runner().bind_roles();
        // A popover its author placed by `left` or `top` opens at that point,
        // as a context menu opens at the pointer (LLP 1021 D2).
        let point = point.or_else(|| {
            let node = self.host.kernel().node_by_key(target?)?;
            let style = node.computed_style(exact_kernel::StyleMask::INHERITED);
            let popover = !modal && node.props.str(PropId::Popover).is_some();
            popover
                .then(|| area::inset_point(&style, &self.host.kernel().env(), self.viewport))
                .flatten()
        });
        if self.neutral_alerts() {
            return self.alert_present(target, name, invoker, point, modal, now_ms);
        }
        let kernel = self.host.kernel();
        let pop = (target.and_then(|key| kernel.node_by_key(key)))
            .map(|node| (node.id, node.key, node.parent));
        let plan = pop.and_then(|(id, ..)| self.surface_plan(id, modal));
        if self.top.overlay.enabled && plan.is_none() {
            return self.alert_present(target, name, invoker, point, modal, now_ms);
        }
        let shows =
            |parent: Option<ViewId>| !parent.is_some_and(|p| self.host.route_visibility(p).1);
        let refusal = match (pop, &plan) {
            (None, _) => Some("no live node has that id"),
            (Some((_, _, parent)), _) if !shows(parent) => Some("it is on a route that does not show"),
            (_, Some(Plan::Menu(_))) if modal => Some("it is a menu, which its invoker opens"),
            (_, None) => Some("this host presents an alert-shaped alertdialog (text, one cancel, at most three buttons) and a menu popover (button rows); authored dialogs and popovers are presented on desktop hosts, but not yet in Android's windows (LLP 1115.001 D10)"),
            _ => None,
        };
        let (Some((id, key, _)), Some(plan), None) = (pop, plan, refusal) else {
            let what = if modal { "showModal" } else { "popover" };
            let why = refusal.unwrap_or_default();
            self.host.log(format!("{what} \"{name}\" refused: {why}"));
            return false;
        };
        let restore = self
            .replace_surface(invoker, now_ms)
            .and_then(|key| self.host.kernel().node_by_key(key).map(|n| n.id));
        self.report_top_layer();
        if let Err(e) = self.pointer_cancel(now_ms) {
            self.host.log(e);
        }
        // The focus moves in: the page's node loses it.
        if let Some(e) = self.set_focus(None, now_ms) {
            self.host.log(e);
        }
        let serial = self.next_surface_serial();
        let inputs = self.surface_inputs();
        let measure = if self.top.overlay.enabled {
            Layout::desktop
        } else {
            Layout::of
        };
        let layout = measure(&mut self.text.borrow_mut(), &plan, id, &inputs);
        self.top.open = Some(Open {
            serial,
            revision: 0,
            key,
            node: id,
            modal,
            kind: match plan {
                Plan::Alert { .. } => SurfaceKind::Alert,
                Plan::Menu(_) => SurfaceKind::Menu,
            },
            invoker,
            point,
            restore,
            plan,
            inputs,
            anchor: (0.0, 0.0, 0.0, 0.0),
            layout,
            frame: [0; 4],
            scale: self.brush.scale,
            origin: 0,
            pressed: None,
            focused: None,
            hovered: None,
            epoch: self.host.kernel().epoch(),
        });
        let anchor = self.surface_anchor();
        if let Some(open) = self.top.open.as_mut() {
            open.anchor = anchor;
        }
        self.place_surface();
        // Alerts need an initial action even when a pointer or a response
        // opens them. TV surfaces also start at their first row (LLP 1115.001.003 D3).
        let desktop_alert = self.top.overlay.enabled
            && self
                .top
                .open
                .as_ref()
                .is_some_and(|open| open.kind == SurfaceKind::Alert);
        if self.remote() || desktop_alert {
            self.surface_step(false);
        }
        self.queue_toggle(key, true);
        self.surface_changed();
        true
    }

    /// Close the previous surface and carry its original focus return target.
    pub(super) fn replace_surface(&mut self, invoker: Option<ViewId>, now: f64) -> Option<NodeKey> {
        let restore = if let Some(open) = &self.alert {
            open.restore
        } else {
            self.top
                .open
                .as_ref()
                .map_or(self.focus.or(invoker), |o| o.restore)
                .and_then(|id| self.host.kernel().node(id).map(|n| n.key))
        };
        self.close_surface(now);
        restore
    }

    pub(super) fn close_surface(&mut self, now_ms: f64) {
        if self.alert.is_some() {
            self.alert_close(now_ms);
            return;
        }
        let Some(open) = self.top.open.take() else {
            return;
        };
        self.report_top_layer();
        self.queue_surface_closed(open.key, open.modal);
        let back = open.restore.filter(|id| {
            self.host.kernel().node(*id).is_some()
                && !self.host.route_visibility(*id).1
                && self.focusable(*id)
        });
        if self.focus.is_none() && back.is_some() {
            if let Some(e) = self.set_focus(back, now_ms) {
                self.host.log(e);
            }
        }
        self.surface_changed();
    }

    /// Back, Escape or a tap outside, where the surface's own rule lets it
    /// go: a menu always, an alert unless its `closedby` is `none`. Whether
    /// a surface was open to take it.
    pub(super) fn dismiss_surface(&mut self, now_ms: f64) -> bool {
        if self.alert.is_some() {
            return self.alert_close_request(now_ms);
        }
        let Some(open) = &self.top.open else {
            return false;
        };
        let may = match &open.plan {
            Plan::Alert { cancellable, .. } => *cancellable,
            Plan::Menu(_) => true,
        };
        let (serial, node, modal) = (open.serial, open.node, open.modal);
        if may
            && (!modal || !self.dialog_close_prevented(node, now_ms))
            && self.top.open.as_ref().is_some_and(|o| o.serial == serial)
        {
            self.close_surface(now_ms);
        }
        true
    }

    /// HTML's cancel is sent only for a person's close request, before close.
    pub(super) fn dialog_close_prevented(&mut self, id: ViewId, now: f64) -> bool {
        let error = self.host.dispatch_at(id, Event::Cancel, now);
        if let Some(error) = error.or(self.after_commit()) {
            self.host.log(error);
        }
        let prevented = self.commands.iter().any(|c| c.name == "preventDefault");
        self.commands
            .retain(|c| c.name != "preventDefault" && c.name != "stopPropagation");
        prevented
    }

    /// A person chose row `index`: the surface closes, then the row's
    /// `press` runs (an alert's cancel has none), then the menu it opens,
    /// where it opens one, takes the first one's place.
    /// A point of the open surface's own space in the page's, where its
    /// row's `MouseEvent` has `clientX`/`clientY` (LLP 1021 D2).
    /// Its frame is in the window's pixels, the rest in points.
    fn surface_client(&self, (x, y): (f32, f32)) -> Option<(f32, f32)> {
        let open = self.top.open.as_ref()?;
        let [px, py, ..] = self.chrome().frame;
        Some((
            open.frame[0] as f32 / open.scale + x - px,
            open.frame[1] as f32 / open.scale + y - py,
        ))
    }

    /// Row `index` chosen; `pressed`, where a pointer chose it, in the page.
    fn choose(&mut self, index: usize, now_ms: f64, pressed: Option<(f32, f32)>) {
        let Some(open) = &self.top.open else {
            return;
        };
        let Some(row) = open.plan.rows().nth(index).filter(|r| r.enabled).cloned() else {
            return;
        };
        let (invoker, point, menu) = (open.invoker, open.point, open.key);
        let key = self.host.kernel().node(row.node).map(|node| node.key);
        self.close_surface(now_ms);
        if self
            .host
            .runner()
            .handlers_of(row.node)
            .contains(&EventKind::Press)
            && !row.cancel
        {
            let (x, y) = pressed.unwrap_or_default();
            let press = Event::pressed(exact_runner::MouseEvent {
                client_x: x.into(),
                client_y: y.into(),
                held: self.modifiers(),
            });
            let error = self.host.dispatch_at(row.node, press, now_ms);
            if let Some(e) = error.or(self.after_commit()) {
                eprintln!("exact: {e}");
            }
        }
        // What the surviving row opens once its `press` has run, as HTML
        // resolves a button's invocation after its listeners: a submenu, or
        // a dialog by `show-modal`, in the menu's place.
        let kernel = self.host.kernel();
        let opens = (key.and_then(|key| kernel.node_by_key(key)))
            .filter(|row| row.props.bool(PropId::Disabled) != Some(true))
            .and_then(|row| exact_kernel::Invocation::of(kernel, &row))
            .filter(|i| i.opens() && i.target != menu);
        if let Some(i) = opens {
            let name = self.name_of(i.target);
            let modal = i.action == Action::ShowModal;
            self.present(Some(i.target), &name, invoker, point, modal, now_ms);
        }
    }

    /// What the shell says happened in the window of surface `serial`
    /// ([`event`]): a touch at (`x`, `y`) of the surface, in points from
    /// its top left; a key; a dismissal; a refusal. What it says of a
    /// surface that is no longer the one open is dropped.
    pub fn surface_event(&mut self, serial: u32, what: i32, x: f32, y: f32, now_ms: f64) {
        if self.top.open.as_ref().map(|open| open.serial) != Some(serial) {
            return;
        }
        match what {
            0..=3 => self.surface_touch(what, x, y, now_ms),
            event::DISMISSED => {
                let open = self.top.open.as_ref().unwrap();
                let (node, modal) = (open.node, open.modal);
                // The shell's window is already gone. HTML permits ignoring
                // cancelation when the UA cannot honor it; cancel still fires.
                if modal {
                    self.dialog_close_prevented(node, now_ms);
                }
                if self.top.open.as_ref().is_some_and(|o| o.serial == serial) {
                    self.close_surface(now_ms);
                }
            }
            event::REFUSED => {
                self.host.log(format!(
                    "the top layer's surface {serial} was refused: the shell could not make its window"
                ));
                self.close_surface(now_ms);
            }
            event::ENTER | event::SPACE => {
                self.surface_key("Enter", now_ms);
            }
            event::TAB | event::DOWN | event::RIGHT => self.surface_step(false),
            event::TAB_BACK | event::UP | event::LEFT => self.surface_step(true),
            _ => {}
        }
    }

    /// A touch in the surface: a row is lit while a finger holds it, and
    /// chosen when the finger lifts in it. A finger that leaves its row
    /// lets go of it.
    fn surface_touch(&mut self, action: i32, x: f32, y: f32, now_ms: f64) {
        let Some(open) = self.top.open.as_mut() else {
            return;
        };
        let at = (open.layout.target_at(x, y)).filter(|i| open.layout.targets[*i].enabled);
        let before = open.pressed;
        let mut chosen = None;
        match action {
            0 => open.pressed = at,
            2 if open.pressed != at => open.pressed = None,
            2 => {}
            1 => chosen = open.pressed.take().filter(|pressed| Some(*pressed) == at),
            _ => open.pressed = None,
        }
        if open.pressed != before {
            open.revision += 1;
            self.executor.notify();
        }
        if let Some(index) = chosen {
            let point = self.surface_client((x, y));
            self.choose(index, now_ms, point);
        }
    }

    /// A key while a surface is open goes to it and to nothing of the page:
    /// Escape dismisses it, Tab and the arrows move its own focus, Enter
    /// and Space choose the focused row. Whether a surface took the key.
    /// Whether a surface is open to take the keys ([`Self::surface_key`]).
    pub(crate) fn surface_takes_keys(&self) -> bool {
        self.alert.is_some() || self.top.open.is_some()
    }

    #[cfg(windows)]
    pub(super) fn desktop_surface_access(&self) -> Option<(Option<ViewId>, (f32, f32))> {
        let open = self.top.open.as_ref()?;
        let focused = open
            .focused
            .and_then(|i| open.layout.targets.get(i))
            .map(|t| t.node);
        Some((
            focused,
            (
                open.frame[0] as f32 / open.scale,
                open.frame[1] as f32 / open.scale,
            ),
        ))
    }

    pub(super) fn surface_key(&mut self, name: &str, now_ms: f64) -> bool {
        if !matches!(
            name,
            "" | "Unidentified"
                | "Shift"
                | "Control"
                | "Alt"
                | "Meta"
                | "CapsLock"
                | "NumLock"
                | "ScrollLock"
        ) {
            self.overlay_input(true);
        }
        if self.top.open.is_none() {
            return false;
        }
        if self.overlay_scroll_key(name) {
            return true;
        }
        match name {
            "Escape" => {
                self.dismiss_surface(now_ms);
            }
            "Tab" => self.surface_step(self.modifiers().shift),
            "ArrowDown" | "ArrowRight" => self.surface_step(false),
            "ArrowUp" | "ArrowLeft" => self.surface_step(true),
            "Home" | "End" if self.top.overlay.enabled => self.surface_edge(name == "End"),
            "Enter" | " " => {
                if let Some(index) = self.top.open.as_ref().and_then(|open| open.focused) {
                    self.choose(index, now_ms, None);
                }
            }
            _ => {}
        }
        true
    }

    /// An authored focus command selects the row in its shell window,
    /// leaving the page's focus set aside until the surface closes.
    pub(super) fn surface_focus(&mut self, id: ViewId) -> bool {
        let Some(open) = self.top.open.as_mut() else {
            return false;
        };
        let Some(index) = open
            .layout
            .targets
            .iter()
            .position(|row| row.node == id && row.enabled)
        else {
            return false;
        };
        let revealed = open.layout.reveal_menu_target(index);
        if open.focused != Some(index) || revealed {
            open.focused = Some(index);
            open.revision += 1;
            self.executor.notify();
        }
        true
    }

    /// Move the surface's own focus to the next row that can be chosen
    /// (`backward`: the one before), from none to the first (the last).
    fn surface_step(&mut self, backward: bool) {
        let Some(open) = self.top.open.as_mut() else {
            return;
        };
        let targets = &open.layout.targets;
        let n = targets.len();
        let from = open.focused;
        let order = if from.is_some() {
            open.layout.focus_order()
        } else {
            (0..n).collect()
        };
        let from = from.and_then(|i| order.iter().position(|v| *v == i));
        let next = (1..=n)
            .map(|step| match (from, backward) {
                (Some(at), false) => (at + step) % n,
                (Some(at), true) => (at + n - step) % n,
                (None, false) => step - 1,
                (None, true) => n - step,
            })
            .map(|i| order[i])
            .find(|i| targets[*i].enabled);
        let revealed = next.is_some_and(|i| open.layout.reveal_menu_target(i));
        if next.is_some() && (next != open.focused || revealed) {
            open.focused = next;
            open.revision += 1;
            self.executor.notify();
        }
    }

    fn surface_edge(&mut self, last: bool) {
        let Some(open) = self.top.open.as_mut() else {
            return;
        };
        if open.kind != SurfaceKind::Menu {
            return;
        }
        let targets = &open.layout.targets;
        let next = if last {
            targets.iter().rposition(|row| row.enabled)
        } else {
            targets.iter().position(|row| row.enabled)
        };
        let revealed = next.is_some_and(|i| open.layout.reveal_menu_target(i));
        if next != open.focused || revealed {
            open.focused = next;
            open.revision += 1;
            self.executor.notify();
        }
    }

    /// A touch reached the page while a surface is open: it is outside the
    /// surface, since the page is inert under it. A menu closes (the
    /// platform's own popup does this itself and the touch never arrives),
    /// and so does an alert whose `closedby` is `any`; any other alert has
    /// no outside. Whether the touch was taken from the page.
    pub(super) fn top_takes_pointer(&mut self, now_ms: f64) -> bool {
        match self.top.open.as_ref().map(|open| open.outside()) {
            Some(true) => {
                self.dismiss_surface(now_ms);
            }
            Some(false) => {}
            None => return false,
        }
        true
    }

    /// Whether a surface of the top layer (a popover's, a menu's) is open over the page: nothing under it is the
    /// agent's to scroll clear of a sticky box (`clear_of_sticky`).
    pub(super) fn surface_open(&self) -> bool {
        self.top.open.is_some()
    }

    /// Whether `id` is a row of the open surface, by its authored node: it
    /// shows in the surface's window, so the agent's `reveal` has nothing
    /// to scroll into view before a tap on it.
    pub(super) fn surface_row(&self, id: ViewId) -> bool {
        (self.top.open.as_ref())
            .is_some_and(|open| open.layout.targets.iter().any(|t| t.node == id))
    }

    /// The agent's `tap` while a surface is open: a row of the surface is
    /// chosen, by its authored node; any other node is under the surface,
    /// where a tap closes a menu and does nothing through an alert.
    pub(super) fn surface_tap(&mut self, id: ViewId) -> Option<Result<String, String>> {
        if self.alert.is_some() {
            return self.alert_outside_tap(id);
        }
        let open = self.top.open.as_ref()?;
        let (serial, pop, kind, outside) = (open.serial, open.node, open.kind, open.outside());
        let row = open.layout.targets.iter().position(|t| t.node == id);
        let enabled = row.is_some_and(|index| open.layout.targets[index].enabled);
        let now = self.host.now();
        Some(match (row, kind) {
            (Some(_), _) if !enabled => Err(format!(
                "view {id} is disabled in the open surface (view {pop})"
            )),
            (Some(index), _) => {
                // The agent's tap is a pointer's, at the row's middle.
                let (x, y, w, h) = open.layout.targets[index].rect;
                let point = self.surface_client((x + w / 2., y + h / 2.));
                self.choose(index, now, point);
                Ok(format!("{{\"tapped\":{id},\"surface\":{serial}}}"))
            }
            (None, _) if outside => {
                self.dismiss_surface(now);
                let dismissed = if self.top.open.as_ref().is_some_and(|o| o.serial == serial) {
                    "null".into()
                } else {
                    serial.to_string()
                };
                Ok(format!("{{\"tapped\":{id},\"dismissed\":{dismissed}}}"))
            }
            (None, _) => Err(format!(
                "view {id} is under the open alert (view {pop}): the page is inert until it closes"
            )),
        })
    }

    /// The button a press at `hit` invokes the top layer through: the
    /// nearest button at or above it, when it names something (a non-empty
    /// `commandfor` or `popovertarget`). With no top layer, `hit` itself where
    /// it is a popover's invoker, which is then refused
    /// ([`Presenter::invoke`]), as it always was.
    pub(crate) fn top_invoker(&self, hit: ViewId) -> Option<ViewId> {
        let kernel = self.host.kernel();
        if !self.has_windows() && !self.neutral_alerts() {
            return crate::navigation::popover_invoker(kernel, hit).then_some(hit);
        }
        let mut at = Some(hit);
        while let Some(id) = at {
            let node = kernel.node(id)?;
            if node.props.bool(PropId::Disabled) == Some(true) {
                return None;
            }
            if shape::is_button(&node) {
                let names = |prop| node.props.str(prop).is_some_and(|name| !name.is_empty());
                return (names(PropId::Commandfor) || names(PropId::Popovertarget)).then_some(id);
            }
            at = node.parent;
        }
        None
    }

    /// A press on `invoker` ([`Presenter::top_invoker`]): its own `press`
    /// runs first, as a click's listeners run before its default action,
    /// then what the surviving button invokes ([`Presenter::follow_invocation`]).
    /// The node pressed, or `None` on a host with no top layer, which
    /// refuses it.
    pub(crate) fn invoke(&mut self, invoker: ViewId, now_ms: f64, pointer: bool) -> Option<ViewId> {
        self.overlay_input(!pointer);
        if !self.has_windows() && !self.neutral_alerts() {
            self.host.log(crate::navigation::POPOVER_UNSUPPORTED);
            return None;
        }
        let key = self.host.kernel().node(invoker)?.key;
        if (self.host.runner().handlers_of(invoker)).contains(&EventKind::Press) {
            self.dispatch_press(invoker, now_ms, pointer);
        }
        self.follow_invocation(key, invoker, now_ms);
        Some(invoker)
    }

    /// After a button's `press` has run: what the button `key` holds invokes
    /// then, if it is still that node, enabled and shown (a bar's button is
    /// out of the page and still its route's to press). A popover opens or
    /// closes by its action, a dialog opens modally by `show-modal` and
    /// closes by `close`; a popover command leaves a surface open modally
    /// alone, and `close` one open as a popover.
    pub(crate) fn follow_invocation(&mut self, key: NodeKey, invoker: ViewId, now_ms: f64) {
        if !self.has_windows() && !self.neutral_alerts() {
            return;
        }
        let kernel = self.host.kernel();
        let Some(node) = kernel.node_by_key(key).filter(|node| {
            node.props.bool(PropId::Disabled) != Some(true)
                && node.style.display != exact_kernel::Display::None
                && !self.host.authored_visibility(node.id).1
        }) else {
            return;
        };
        let Some(invocation) = exact_kernel::Invocation::of(kernel, &node) else {
            return;
        };
        let (target, name) = (invocation.target, self.name_of(invocation.target));
        match (invocation.action, self.open_mode(target)) {
            (Action::Show | Action::Toggle, None) => {
                self.present(Some(target), &name, Some(invoker), None, false, now_ms);
            }
            (Action::ShowModal, None) => {
                self.present(Some(target), &name, Some(invoker), None, true, now_ms);
            }
            (Action::Hide | Action::Toggle, Some(false)) | (Action::Close, Some(true)) => {
                self.close_surface(now_ms)
            }
            _ => {}
        }
    }

    /// The `id` of the node `key` holds, for what the log says.
    fn name_of(&self, key: NodeKey) -> String {
        let node = self.host.kernel().node_by_key(key);
        node.and_then(|node| node.props.str(PropId::Id))
            .unwrap_or_default()
            .to_string()
    }

    /// A node pressed by a contact that kept its own press feedback: the
    /// top layer's invoker where it is one, else its `press`.
    pub(super) fn activate(&mut self, id: ViewId, now_ms: f64) {
        self.activate_with(id, now_ms, true);
    }

    /// `id` activated by a pointer or by a key (Enter, Space, a shortcut):
    /// an invoker's `press` and then what it invokes, else its `press`.
    pub(super) fn activate_with(&mut self, id: ViewId, now_ms: f64, pointer: bool) {
        self.overlay_input(!pointer);
        let serial = self.top.last;
        if (self.has_windows() || self.neutral_alerts()) && self.top_invoker(id) == Some(id) {
            self.invoke(id, now_ms, pointer);
        } else {
            self.press_then_follow(id, now_ms, pointer);
        }
        if !pointer
            && self.top.overlay.enabled
            && self
                .top
                .open
                .as_ref()
                .is_some_and(|open| open.serial != serial && open.focused.is_none())
        {
            self.surface_edge(false);
        }
    }

    /// `id`'s `press`, then what it invokes if it is a button its handler
    /// gave something to invoke.
    pub(crate) fn press_then_follow(&mut self, id: ViewId, now_ms: f64, pointer: bool) {
        let key = self.host.kernel().node(id).map(|node| node.key);
        self.dispatch_press(id, now_ms, pointer);
        if let Some(key) = key {
            self.follow_invocation(key, id, now_ms);
        }
    }

    pub(super) fn queue_surface_closed(&mut self, key: NodeKey, modal: bool) {
        self.queue_toggle(key, false);
        if modal
            && self
                .host
                .kernel()
                .node_by_key(key)
                .is_some_and(|n| n.props.str(PropId::SemanticTag) == Some("dialog"))
        {
            self.top.closes.push(key);
        }
    }

    pub(super) fn queue_toggle(&mut self, key: NodeKey, open: bool) {
        if let Some((_, _, new)) = self.top.toggles.iter_mut().find(|(k, _, _)| *k == key) {
            *new = open;
        } else {
            self.top.toggles.push((key, !open, open));
        }
    }

    pub(super) fn dispatch_toggles(&mut self) {
        for key in std::mem::take(&mut self.top.closes) {
            let Some(id) = self.host.kernel().node_by_key(key).map(|n| n.id) else {
                continue;
            };
            if let Some(error) = self
                .host
                .dispatch_at(id, Event::Close, self.host.now())
                .or(self.after_commit())
            {
                self.host.log(error);
            }
        }
        for (key, old_open, new_open) in std::mem::take(&mut self.top.toggles) {
            let Some(node) = self.host.kernel().node_by_key(key) else {
                continue;
            };
            if node.props.str(PropId::Popover).is_none()
                || !self
                    .host
                    .runner()
                    .handlers_of(node.id)
                    .contains(&EventKind::Toggle)
            {
                continue;
            }
            let id = node.id;
            if let Some(error) = self
                .host
                .dispatch_at(id, Event::Toggle { old_open, new_open }, self.host.now())
                .or(self.after_commit())
            {
                self.host.log(error);
            }
        }
    }

    pub(super) fn popover_command(&mut self, command: &str, args: &[exact_plan::Value]) {
        if !self.has_windows() && !self.neutral_alerts() {
            self.host.log(format!(
                "{command} unsupported: this host has no popover presentation"
            ));
            return;
        }
        let Some(name) = args.first().and_then(exact_plan::Value::as_str) else {
            return;
        };
        let target = self.element(name).filter(|key| {
            (self.host.kernel().node_by_key(*key))
                .is_some_and(|n| n.props.str(PropId::Popover).is_some())
        });
        let Some(target) = target else {
            self.host
                .log(format!("{command} refused: no popover has that id"));
            return;
        };
        let source_name = args.get(1).and_then(exact_plan::Value::as_str);
        let source = source_name
            .and_then(|name| self.element(name))
            .and_then(|key| self.host.kernel().node_by_key(key).map(|n| n.id));
        if source_name.is_some() && source.is_none() {
            self.host
                .log(format!("{command} refused: no source has that id"));
            return;
        }
        match self.open_mode(target) {
            Some(true) => self.host.log(format!(
                "{command} ignored: \"{name}\" is open as a modal dialog, which popover commands leave alone"
            )),
            Some(false) if command == "hidePopover" || command == "togglePopover" => {
                self.close_surface(self.host.now())
            }
            None if command != "hidePopover" => {
                self.present(Some(target), name, source, None, false, self.host.now());
            }
            _ => {}
        }
    }

    /// `showModal(id)` from an action (LLP 1115 D6): the confirmation whose
    /// `id` that is, with no invoker (`source`: the node whose input ran
    /// the action, where the focus returns).
    pub(super) fn show_modal(&mut self, args: &[exact_plan::Value], source: Option<ViewId>) {
        let Some(name) = args.first().and_then(exact_plan::Value::as_str) else {
            return self.host.log("showModal requires an element id");
        };
        let target = self.element(name);
        if target.is_some_and(|key| self.open_mode(key).is_some()) {
            return self
                .host
                .log(format!("showModal \"{name}\" ignored: it is open"));
        }
        let now = self.host.now();
        self.present(target, name, source, None, true, now);
    }

    /// `close(id)` from an action: the open surface, when it is that node
    /// open modally, closes as its cancel closes it (HTML's `close` does
    /// nothing to a dialog shown as a popover).
    pub(super) fn close_modal(&mut self, args: &[exact_plan::Value]) {
        let name = args.first().and_then(exact_plan::Value::as_str);
        let target = name.and_then(|name| self.element(name));
        if target.is_some_and(|key| self.open_mode(key) == Some(true)) {
            let now = self.host.now();
            self.close_surface(now);
        }
    }

    /// A finger held still at (`x`, `y`) for the platform's long-press
    /// time, which the shell measures: the nearest node at or above it that
    /// names a context menu (`contextPopover`) or hears `contextmenu` takes
    /// it. Its `contextmenu` runs first, then its menu opens where the
    /// finger is, and the touch presses nothing when it lifts. Whether a
    /// node took it. Nothing on a host with no top layer.
    pub fn long_press(&mut self, x: f32, y: f32, now_ms: f64) -> bool {
        if (!self.has_windows() && !self.neutral_alerts()) || self.page_inert() {
            return false;
        }
        let Some(hit) = self.hit(x, y) else {
            return false;
        };
        let (kernel, runner) = (self.host.kernel(), self.host.runner());
        let mut at = Some(hit);
        let mut found = None;
        while let Some(node) = at.and_then(|id| kernel.node(id)) {
            if node.props.bool(PropId::Disabled) == Some(true) {
                break;
            }
            let names = node
                .props
                .str(PropId::ContextPopover)
                .filter(|n| !n.is_empty());
            let hears = runner
                .handlers_of(node.id)
                .contains(&EventKind::Contextmenu);
            if names.is_some() || hears {
                found = Some((node.id, names.map(str::to_string), hears));
                break;
            }
            at = node.parent;
        }
        let Some((id, names, hears)) = found else {
            return false;
        };
        self.top.long_presses = self.top.long_presses.wrapping_add(1);
        if let Err(e) = self.pointer_cancel(now_ms) {
            self.host.log(e);
        }
        // The platform's own long-press click.
        self.request(Request::Haptic(Haptic::ImpactHeavy));
        if hears {
            let error = self.host.dispatch_at(id, Event::Contextmenu, now_ms);
            if let Some(e) = error.or(self.after_commit()) {
                eprintln!("exact: {e}");
            }
        }
        if let Some(name) = names {
            let menu = self.element(&name);
            self.present(menu, &name, Some(id), Some((x, y)), false, now_ms);
        }
        true
    }

    /// How many long presses a node has taken, wrapping.
    #[cfg(any(target_os = "android", test))]
    pub(crate) fn long_presses(&self) -> u32 {
        self.top.long_presses
    }

    /// Before the shell is told: an open surface whose node is gone, whose
    /// route no longer shows or whose rows are no longer its shape closes,
    /// running nothing; one that says something else, or is shown under
    /// another scheme, font scale or window, is measured again.
    pub(super) fn sync_surface(&mut self) {
        let Some(open) = &self.top.open else {
            return;
        };
        let (key, pop, kind, read) = (open.key, open.node, open.kind, open.epoch);
        let kernel = self.host.kernel();
        let epoch = kernel.epoch();
        let node = kernel.node_by_key(key).filter(|node| node.id == pop);
        let shows = node.is_some_and(|node| {
            !(node.parent).is_some_and(|parent| self.host.route_visibility(parent).1)
        });
        let plan = match (shows, epoch == read) {
            (false, _) => None,
            (true, true) => Some(open.plan.clone()),
            (true, false) => self
                .surface_plan(pop, open.modal)
                .filter(|plan| matches!(plan, Plan::Alert { .. }) == (kind == SurfaceKind::Alert)),
        };
        let Some(plan) = plan else {
            let now = self.host.now();
            self.close_surface(now);
            return;
        };
        let inputs = self.surface_inputs();
        let anchor = self.surface_anchor();
        let Some(open) = self.top.open.as_mut() else {
            return;
        };
        open.epoch = epoch;
        if plan != open.plan || inputs != open.inputs {
            let measure = if self.top.overlay.enabled {
                Layout::desktop
            } else {
                Layout::of
            };
            open.layout = measure(&mut self.text.borrow_mut(), &plan, pop, &inputs);
            (open.plan, open.inputs) = (plan, inputs);
            open.pressed = None;
            open.hovered = None;
            open.focused = open.focused.filter(|i| *i < open.layout.targets.len());
            if let Some(index) = open.focused {
                open.layout.reveal_menu_target(index);
            }
            open.revision += 1;
        }
        if anchor != open.anchor {
            open.anchor = anchor;
            open.revision += 1;
        }
        self.place_surface();
        self.report_top_layer();
    }

    fn place_surface(&mut self) {
        let (window, margins) = self.surface_window();
        let scale = self.brush.scale;
        let Some(open) = self.top.open.as_mut() else {
            return;
        };
        let (at, origin) = match open.kind {
            SurfaceKind::Alert if self.top.overlay.enabled => (
                (
                    (window.0 - open.layout.size.0).max(0.) / 2.,
                    (window.1 - open.layout.size.1).max(0.) / 2.,
                ),
                0,
            ),
            SurfaceKind::Alert => ((0.0, 0.0), 0),
            SurfaceKind::Menu => {
                let at = look::place(open.anchor, open.layout.size, window, margins);
                (at, look::origin(open.anchor, at, open.layout.size))
            }
        };
        let px = |v: f32| (v * scale).round() as i32;
        let frame = [
            px(at.0),
            px(at.1),
            px(open.layout.size.0),
            px(open.layout.size.1),
        ];
        if (frame, scale, origin) != (open.frame, open.scale, open.origin) {
            (open.frame, open.scale, open.origin) = (frame, scale, origin);
            open.revision += 1;
        }
    }

    /// Report the shell's frame in viewport points and whether it opened
    /// modally. A popover leaves the document reachable to structural reads.
    /// Native alerts, as on Apple, report no authored geometry.
    pub(super) fn report_top_layer(&mut self) {
        let Some(open) = &self.top.open else {
            // An open authored popover is the page's own subtree, drawn at its
            // box (alert.rs): reads see it, as the web's do.
            let frames: Vec<_> = self.alert_popover_frame().into_iter().collect();
            self.host.runner_mut().top_layer_frames(&frames, None);
            return;
        };
        let rect = match open.kind {
            SurfaceKind::Alert => [0.0; 4],
            SurfaceKind::Menu => {
                let [px, py, ..] = self.chrome().frame;
                let [x, y, w, h] = open.frame.map(|v| f64::from(v) / f64::from(open.scale));
                [x - f64::from(px), y - f64::from(py), w, h]
            }
        };
        let frames = [(open.node, rect, None)];
        let modal = open.modal.then_some(0);
        self.host.runner_mut().top_layer_frames(&frames, modal);
    }

    /// The open surface as the shell is told it: its frame in the window
    /// and its picture, recorded by the shell's painter.
    fn surface_statement(&mut self) -> Option<Surface> {
        let text = self.text.clone();
        let open = self.top.open.as_ref()?;
        let scale = open.scale;
        let layout = &open.layout;
        let feedback = look::Feedback {
            pressed: open.pressed,
            focused: open.focused,
            hovered: open.hovered,
        };
        let painter = self.top.painter.as_mut()?;
        let stream = painter.record(layout.size, scale, &mut |backend| {
            layout.paint(backend, &mut text.borrow_mut(), feedback)
        });
        let cancellable = match &open.plan {
            Plan::Alert { cancellable, .. } => *cancellable,
            Plan::Menu(_) => true,
        };
        Some(Surface {
            serial: open.serial,
            kind: Some(open.kind),
            flags: u32::from(cancellable) | u32::from(open.outside()) << 1 | open.origin << 2,
            frame: open.frame,
            shape: (layout.radius * scale, layout.elevation * scale),
            stream,
        })
    }

    /// Tell the shell the top layer's surface when it is not what it was
    /// last told: the one it shows closed, another opened, or the one open
    /// changed its picture or its place. Queued with the shell's other
    /// statements (`tell_shell`), on the one channel out.
    pub(super) fn tell_surface(&mut self) {
        if self.top.painter.is_none() {
            return;
        }
        self.sync_surface();
        let now = self
            .top
            .open
            .as_ref()
            .map(|open| (open.serial, open.revision));
        if now == self.top.told {
            return;
        }
        let gone = self
            .top
            .told
            .filter(|told| now.map(|n| n.0) != Some(told.0));
        if let Some((serial, _)) = gone {
            self.queue(Request::Surface(Surface {
                serial,
                kind: None,
                flags: 0,
                frame: [0; 4],
                shape: (0.0, 0.0),
                stream: Vec::new(),
            }));
        }
        if let Some(surface) = self.surface_statement() {
            self.queue(Request::Surface(surface));
        }
        self.top.told = now;
    }

    /// The open surface for the agent's `state`: what it is, the authored
    /// node, what it says, and each row's node and box in the window.
    pub fn surface_json(&self) -> serde_json::Value {
        if self.alert.is_some() {
            return self.alert_json();
        }
        let Some(shown) = self.surface() else {
            return serde_json::Value::Null;
        };
        let rows: Vec<_> = (shown.plan.rows().zip(shown.targets))
            .map(|(row, target)| {
                let (x, y, w, h) = target.rect;
                serde_json::json!({"id": row.node, "title": row.title, "enabled": row.enabled,
                    "destructive": row.destructive, "cancel": row.cancel,
                    "x": r2(x), "y": r2(y), "w": r2(w), "h": r2(h)})
            })
            .collect();
        let (title, message) = match shown.plan {
            Plan::Alert { title, message, .. } => (title.clone(), message.clone()),
            Plan::Menu(_) => (None, None),
        };
        serde_json::json!({
            "serial": shown.serial,
            "kind": match shown.kind { SurfaceKind::Alert => "alert", SurfaceKind::Menu => "menu" },
            "node": shown.node, "title": title, "message": message,
            "w": r2(shown.size.0), "h": r2(shown.size.1), "rows": rows,
        })
    }
}

#[cfg(test)]
#[path = "top_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "top_invocation_tests.rs"]
mod invocation_tests;
