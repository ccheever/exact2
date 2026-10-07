//! Access hatches on a painting host.
//!
//! @ref LLP 1075.003.000.001 §2.1–§2.5, §3.2, §5 (Linux and Windows)
//!
//! An app's hatches are one type that implements [`Hatches`], in
//! `modules/linux/*.rs`, named once (`pub type ExactHatches = App;`) and
//! included by the app's own Linux crate: statically linked, no `dlopen`, no
//! C table. The entry holds only the type ([`install`]); the host makes the
//! value with `Default::default()` after its first frame is presented, so no
//! hatch code runs before first pixel, and none inside the painter's walk.
//!
//! This host has no native objects, so a handle carries what a painter has:
//! the node's laid-out box, its declared `data-*` words, an [`Overlay`] the
//! hatch replaces whole, and the input that lands in the box, to observe
//! only. Into the Contract app a hatch goes as a person would: `click`,
//! `focus`, `blur` and `input` on an authored node, each queued, journaled
//! and run between turns.
//!
//! Every call is made on the host loop's thread. The handles are not `Send`:
//! what another thread learns reaches a hatch through its own channel.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

pub(crate) mod breadcrumb;
pub(crate) mod diagnostics;
pub(crate) mod session;
#[cfg(test)]
mod tests;

/// The recorder an overlay is drawn with: Canvas 2D by web-sys's names (LLP 1056).
pub use exact_canvas::Context2d;
pub(crate) use session::{ActKind, Module, Session, Shared};

/// An app's hatches. Every method has a default that does nothing.
///
/// ```ignore
/// #[derive(Default)]
/// pub struct App { taps: u32 }
/// impl exact_linux::Hatches for App {
///     fn element(&mut self, element: &Element, context: &mut Context<'_, Self>) {
///         element.overlay().draw(|c, w, h| { c.set_fill_style_str("#f00"); c.fill_rect(0., 0., w, h); });
///     }
/// }
/// pub type ExactHatches = App;
/// ```
pub trait Hatches: Default + 'static {
    /// The app: built as the hatches connect, changed when a fact moves.
    fn app(&mut self, _app: &App, _context: &mut Context<'_, Self>) {}
    /// The session is ending, or its plan is being replaced.
    fn app_ended(&mut self, _app: &App, _context: &mut Context<'_, Self>) {}
    /// The surface the session presents into: built, then changed on a new size.
    fn window(&mut self, _window: &Window, _context: &mut Context<'_, Self>) {}
    /// The session is leaving its surface.
    fn window_ended(&mut self, _window: &Window, _context: &mut Context<'_, Self>) {}
    /// A node marked `hatch="word"`: built after the commit that mounts it,
    /// changed when its `data-*` words or its box's size change.
    fn element(&mut self, _element: &Element, _context: &mut Context<'_, Self>) {}
    /// The node has left the tree. Its handle keeps its last box and words.
    fn element_ended(&mut self, _element: &Element, _context: &mut Context<'_, Self>) {}
}

/// What an entry hands the host: the hatches' type and the words `app.json`
/// gives this platform. Nothing of the app's is constructed by it.
#[derive(Clone, Copy)]
pub struct Install {
    pub(crate) make: fn(Rc<Shared>) -> Box<dyn Module>,
    pub(crate) words: &'static [&'static str],
}

/// `H` as an app's hatches, handling `words` on this platform.
pub fn install<H: Hatches>(words: &'static [&'static str]) -> Install {
    Install {
        make: make::<H>,
        words,
    }
}

fn make<H: Hatches>(shared: Rc<Shared>) -> Box<dyn Module> {
    Box::new(Holder {
        module: H::default(),
        callbacks: Callbacks::default(),
        shared,
        pruned: 0,
    })
}

/// A box in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// The app scope's handle: the facts, by the web's names (§2.1).
#[derive(Clone, Debug, PartialEq)]
pub struct App {
    /// `document.visibilityState`: `visible` or `hidden`.
    pub visibility_state: &'static str,
    /// `navigator.onLine`.
    pub on_line: bool,
    /// `prefers-color-scheme`: `light` or `dark`.
    pub prefers_color_scheme: &'static str,
    /// `prefers-contrast`: `no-preference`, `more`, `less` or `custom`.
    pub prefers_contrast: &'static str,
    /// `prefers-reduced-motion: reduce`.
    pub prefers_reduced_motion: bool,
    /// `prefers-reduced-transparency: reduce`.
    pub prefers_reduced_transparency: bool,
    /// This session may set process-wide state: the entry's one session does.
    pub process_owner: bool,
    /// This is the built moment.
    pub is_new: bool,
    /// False in `app_ended`.
    pub is_live: bool,
}

/// The window scope's handle: the surface the session paints (§2.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    /// The surface in its own coordinates, points.
    pub frame: Rect,
    /// Device pixels per point.
    pub scale: f32,
    /// The window is this session's alone: the entry's one session's is.
    pub exclusive: bool,
    /// This is the built moment.
    pub is_new: bool,
    /// False in `window_ended`.
    pub is_live: bool,
}

/// What a node's handle shares with the host.
pub(crate) struct Node {
    pub(crate) id: u32,
    pub(crate) word: String,
    pub(crate) html_id: String,
    pub(crate) scope: Rc<str>,
    pub(crate) shared: Rc<Shared>,
    pub(crate) live: Cell<bool>,
    pub(crate) new: Cell<bool>,
    pub(crate) frame: Cell<Rect>,
    pub(crate) data: RefCell<BTreeMap<String, String>>,
}

/// A hatched node's handle (§2.2). Cheap to clone and keep: a kept handle
/// reads the node's current box and words, and after the node's end its last.
#[derive(Clone)]
pub struct Element {
    pub(crate) node: Rc<Node>,
}

impl Element {
    /// The node's `hatch` word.
    pub fn hatch(&self) -> &str {
        &self.node.word
    }
    /// The node's authored `id`, empty when it has none.
    pub fn id(&self) -> &str {
        &self.node.html_id
    }
    /// The node's number, as `tree` and the journal name it (`#130`).
    pub fn node(&self) -> u32 {
        self.node.id
    }
    /// The border box Exact laid out, in the document's coordinates, as of
    /// the last commit. Scroll offsets and transforms are not folded in.
    pub fn frame(&self) -> Rect {
        self.node.frame.get()
    }
    /// A declared `data-*` word's value (`data("tone")` for `data-tone`).
    pub fn data(&self, word: &str) -> Option<String> {
        self.node.data.borrow().get(word).cloned()
    }
    /// Every declared `data-*` word the node carries.
    pub fn words(&self) -> BTreeMap<String, String> {
        self.node.data.borrow().clone()
    }
    /// This is the built moment.
    pub fn is_new(&self) -> bool {
        self.node.new.get()
    }
    /// The node is in the tree.
    pub fn is_live(&self) -> bool {
        self.node.live.get()
    }
    /// Press the node, as a person would: queued, run between turns (§2.5).
    pub fn click(&self) {
        self.node.shared.ask(&self.node, ActKind::Click, None);
    }
    /// Give the node the focus.
    pub fn focus(&self) {
        self.node.shared.ask(&self.node, ActKind::Focus, None);
    }
    /// Take the focus from the node, if it has it.
    pub fn blur(&self) {
        self.node.shared.ask(&self.node, ActKind::Blur, None);
    }
    /// Replace an authored text field's whole value, without moving focus.
    /// The journal holds its length, never its text.
    pub fn input(&self, text: &str) {
        self.node
            .shared
            .ask(&self.node, ActKind::Input, Some(text.to_owned()));
    }
    /// The node's overlay: its top-most content, clipped to its border box.
    pub fn overlay(&self) -> Overlay<'_> {
        Overlay { node: &self.node }
    }
    /// Diagnostics scoped to the node's word (`element <word>`).
    pub fn diagnostics(&self) -> Diagnostics {
        Diagnostics {
            shared: self.node.shared.clone(),
            scope: self.node.scope.clone(),
            node: self.node.id,
        }
    }
}

/// A node's overlay (§2.2.1): a protocol, not a canvas. A recording replaces
/// the last one whole; nothing is carried from one to the next.
pub struct Overlay<'a> {
    node: &'a Rc<Node>,
}

impl Overlay<'_> {
    /// Record a fresh Canvas 2D list against the box's current size (the
    /// width and height `record` is given, points, the origin its top left)
    /// and publish it. False when the recording is refused: past 4,096 ops
    /// or 256 KB, or the node has ended. The last good one then stays.
    pub fn draw(&self, record: impl FnOnce(&Context2d, f64, f64)) -> bool {
        let frame = self.node.frame.get();
        let context = Context2d::new();
        record(&context, f64::from(frame.width), f64::from(frame.height));
        self.node
            .shared
            .publish(self.node, context.take_lists(), frame)
    }
    /// Publish the empty list: the overlay shows nothing.
    pub fn clear(&self) {
        self.node
            .shared
            .publish(self.node, Vec::new(), self.node.frame.get());
    }
}

/// What landed in a node's box, heard after Exact's own dispatch (§2.2.2).
/// `handled` says whether an authored handler ran or a scroller moved for it.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// A pointer event. A pointer that went down in the box stays with its
    /// observers until it lifts or is cancelled, wherever it goes.
    Pointer {
        /// Down, a move, up, or a cancel.
        phase: Phase,
        /// The point in the window, points.
        x: f32,
        /// The point in the window, points.
        y: f32,
        /// The point from the box's top left as painted.
        local: (f32, f32),
        /// Exact's dispatch ran a handler or moved a scroller.
        handled: bool,
    },
    /// A key at the focus, which is this node or inside it.
    Key {
        /// The web's `key`.
        key: String,
        /// The web's `code`, empty when unknown.
        code: String,
        /// Down, or a release.
        down: bool,
        /// An auto-repeat.
        repeat: bool,
        /// Exact's dispatch ran a handler for it.
        handled: bool,
    },
}

/// A pointer event's phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The button went down.
    Down,
    /// The pointer moved.
    Move,
    /// The button lifted.
    Up,
    /// The contact was cancelled.
    Cancel,
}

/// A frame ticket's instant (§2.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The session clock at the instant, milliseconds.
    pub now: f64,
    /// The commit the instant's state ends at.
    pub seq: u64,
}

/// What `frames`, `after` and `observe` return: `stop` ends it.
pub struct Ticket {
    shared: Rc<Shared>,
    token: u64,
}

impl Ticket {
    /// No further call is made for it. A stopped `after` never fires.
    pub fn stop(&self) {
        self.shared.stop(self.token);
    }
}

/// What hatch code says of itself (§3.2): development only, bounded, on
/// the session clock. In a production build every call returns at once.
#[derive(Clone)]
pub struct Diagnostics {
    shared: Rc<Shared>,
    scope: Rc<str>,
    node: u32,
}

impl Diagnostics {
    /// A journal line: at most 256 bytes, 20 a second of session clock a scope.
    pub fn log(&self, text: &str) {
        let now = self.shared.now.get();
        self.shared.store().log(&self.scope, text, now);
    }
    /// A counter, cumulative.
    pub fn count(&self, name: &str, by: u64) {
        self.shared.store().count(&self.scope, name, by);
    }
    /// One sample of a timing the hatch measured itself.
    pub fn measure(&self, name: &str, ms: f64) {
        self.shared.store().sample(&self.scope, name, ms, true);
    }
    /// A span on the session clock, timed when it ends.
    pub fn begin(&self, name: &str) -> Span {
        let now = self.shared.now.get();
        let id = self.shared.store().begin(&self.scope, name, self.node, now);
        Span {
            shared: self.shared.clone(),
            id,
        }
    }
    /// A snapshot, JSON text, the latest kept: at most 4 KB, refused whole past it.
    pub fn publish(&self, name: &str, json: &str) {
        self.shared.store().publish(&self.scope, name, json);
    }
    /// Ask for Save Trace (LLP 1079 D5), at most once a second of wall
    /// time: the display loop writes it; a headless run has none to save.
    pub fn save_trace(&self) {
        self.shared.store().ask_trace();
    }
}

/// An open span. One still open when its node ends is counted abandoned.
pub struct Span {
    shared: Rc<Shared>,
    id: u64,
}

impl Span {
    /// The span's time, on the session clock, joins its timing.
    pub fn end(&self) {
        let now = self.shared.now.get();
        self.shared.store().end(self.id, now);
    }
}

type Tick<H> = Box<dyn FnMut(&mut H, Frame, &mut Context<'_, H>)>;
type Fired<H> = Box<dyn FnOnce(&mut H, &mut Context<'_, H>)>;
type Heard<H> = Box<dyn FnMut(&mut H, &Input, &mut Context<'_, H>)>;

struct Callbacks<H: Hatches> {
    ticks: BTreeMap<u64, Tick<H>>,
    afters: BTreeMap<u64, Fired<H>>,
    heard: BTreeMap<u64, Heard<H>>,
}

impl<H: Hatches> Default for Callbacks<H> {
    fn default() -> Self {
        Callbacks {
            ticks: BTreeMap::new(),
            afters: BTreeMap::new(),
            heard: BTreeMap::new(),
        }
    }
}

/// What a moment is given beside its handle: the session's clock, the
/// module's diagnostics, and the registrations that outlive the call.
pub struct Context<'a, H: Hatches> {
    shared: &'a Rc<Shared>,
    callbacks: &'a mut Callbacks<H>,
}

impl<H: Hatches> Context<'_, H> {
    /// The session clock, milliseconds: the agent's under a drive.
    pub fn now(&self) -> f64 {
        self.shared.now.get()
    }
    /// The session is driven by the agent.
    pub fn agent(&self) -> bool {
        self.shared.agent.get()
    }
    /// Diagnostics scoped to the module (`module`).
    pub fn diagnostics(&self) -> Diagnostics {
        Diagnostics {
            shared: self.shared.clone(),
            scope: Rc::from("module"),
            node: 0,
        }
    }
    /// A frame ticket (§2.4): `tick` runs at each instant LLP 1073's frame
    /// tasks fire, after that frame's tasks and timers; each presented frame
    /// on the wall, the virtual display's under the agent.
    pub fn frames(
        &mut self,
        tick: impl FnMut(&mut H, Frame, &mut Context<'_, H>) + 'static,
    ) -> Ticket {
        let token = self.shared.frames();
        self.callbacks.ticks.insert(token, Box::new(tick));
        self.ticket(token)
    }
    /// `fired` runs once, `ms` of session clock from now.
    pub fn after(
        &mut self,
        ms: f64,
        fired: impl FnOnce(&mut H, &mut Context<'_, H>) + 'static,
    ) -> Ticket {
        let token = self.shared.after(ms);
        self.callbacks.afters.insert(token, Box::new(fired));
        self.ticket(token)
    }
    /// Observe the input that lands in `element`'s box (§2.2.2): `heard`
    /// runs after Exact's own dispatch, and can claim nothing. It ends with
    /// the node.
    pub fn observe(
        &mut self,
        element: &Element,
        heard: impl FnMut(&mut H, &Input, &mut Context<'_, H>) + 'static,
    ) -> Ticket {
        let token = self.shared.observe(&element.node);
        self.callbacks.heard.insert(token, Box::new(heard));
        self.ticket(token)
    }
    fn ticket(&self, token: u64) -> Ticket {
        Ticket {
            shared: self.shared.clone(),
            token,
        }
    }
}

/// The app's value and what it registered, behind the host's own trait.
struct Holder<H: Hatches> {
    module: H,
    callbacks: Callbacks<H>,
    shared: Rc<Shared>,
    /// The host's count of stops when the closures were last pruned.
    pruned: u64,
}

impl<H: Hatches> Holder<H> {
    /// What was stopped, or ended with its node, keeps no closure.
    fn prune(&mut self) {
        let shared = &self.shared;
        if std::mem::replace(&mut self.pruned, shared.stops()) == shared.stops() {
            return;
        }
        self.callbacks.ticks.retain(|t, _| shared.live(*t));
        self.callbacks.afters.retain(|t, _| shared.live(*t));
        self.callbacks.heard.retain(|t, _| shared.live(*t));
    }
}

impl<H: Hatches> Module for Holder<H> {
    fn app(&mut self, app: &App, ended: bool) {
        let mut context = Context {
            shared: &self.shared,
            callbacks: &mut self.callbacks,
        };
        match ended {
            true => self.module.app_ended(app, &mut context),
            false => self.module.app(app, &mut context),
        }
        self.prune();
    }
    fn window(&mut self, window: &Window, ended: bool) {
        let mut context = Context {
            shared: &self.shared,
            callbacks: &mut self.callbacks,
        };
        match ended {
            true => self.module.window_ended(window, &mut context),
            false => self.module.window(window, &mut context),
        }
        self.prune();
    }
    fn element(&mut self, element: &Element, ended: bool) {
        let mut context = Context {
            shared: &self.shared,
            callbacks: &mut self.callbacks,
        };
        match ended {
            true => self.module.element_ended(element, &mut context),
            false => self.module.element(element, &mut context),
        }
        self.prune();
    }
    fn tick(&mut self, token: u64, frame: Frame) {
        // Out of the table for its own call, so it may register and stop.
        let Some(mut tick) = self.callbacks.ticks.remove(&token) else {
            return;
        };
        let mut context = Context {
            shared: &self.shared,
            callbacks: &mut self.callbacks,
        };
        tick(&mut self.module, frame, &mut context);
        if self.shared.live(token) {
            self.callbacks.ticks.insert(token, tick);
        }
        self.prune();
    }
    fn fired(&mut self, token: u64) {
        let Some(fired) = self.callbacks.afters.remove(&token) else {
            return;
        };
        let mut context = Context {
            shared: &self.shared,
            callbacks: &mut self.callbacks,
        };
        fired(&mut self.module, &mut context);
        self.prune();
    }
    fn heard(&mut self, token: u64, input: &Input) {
        let Some(mut heard) = self.callbacks.heard.remove(&token) else {
            return;
        };
        let mut context = Context {
            shared: &self.shared,
            callbacks: &mut self.callbacks,
        };
        heard(&mut self.module, input, &mut context);
        if self.shared.live(token) {
            self.callbacks.heard.insert(token, heard);
        }
        self.prune();
    }
    fn forget(&mut self) {
        self.callbacks = Callbacks::default();
    }
}
