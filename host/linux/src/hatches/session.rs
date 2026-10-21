//! A session's hatches, host side (@ref LLP 1075.003.000.001 §2.4–§2.5,
//! §3.3): what its handles share with the host ([`Shared`]: the act queue,
//! the frame clock's registrations, the overlays published and not yet
//! taken, the observers, the diagnostics store), and what the host keeps of
//! them ([`Session`]: the module, the nodes told, each word's calls, the
//! overlays the painter shows). The presenter drives it between turns
//! (`presenter/hatches.rs`); nothing here reaches the runner or the painter.

use super::breadcrumb::Breadcrumb;
use super::diagnostics::Store;
use super::{App, Element, Frame, Input, Install, Node, Rect, Window};
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

/// The act queue's bounds (§2.5): acts, bytes of `input` text, a drain's
/// snapshot, and one `input`'s text.
pub(crate) const ACTS: usize = 256;
pub(crate) const ACT_BYTES: usize = 1 << 20;
pub(crate) const SNAPSHOT: usize = 64;
pub(crate) const INPUT_BYTES: usize = 65536;
/// An overlay recording's bounds (§2.2.1).
pub(crate) const OVERLAY_OPS: usize = 4096;
pub(crate) const OVERLAY_BYTES: usize = 256 * 1024;
/// Ticks and `after`s, and drained acts, one agent command runs (§2.4).
pub(crate) const COMMAND_LIMIT: u32 = 4096;

/// The app's value behind the host's own trait: the calls, erased of its type.
pub(crate) trait Module {
    fn app(&mut self, app: &App, ended: bool);
    fn window(&mut self, window: &Window, ended: bool);
    fn element(&mut self, element: &Element, ended: bool);
    fn tick(&mut self, token: u64, frame: Frame);
    fn fired(&mut self, token: u64);
    fn heard(&mut self, token: u64, input: &Input);
    /// A reload: what the old incarnation registered keeps no closure.
    fn forget(&mut self);
}

/// What a hatch asks of an authored node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActKind {
    Click,
    Focus,
    Blur,
    Input,
}

impl ActKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            ActKind::Click => "click",
            ActKind::Focus => "focus",
            ActKind::Blur => "blur",
            ActKind::Input => "input",
        }
    }
}

/// One queued act.
pub(crate) struct Act {
    pub(crate) node: Rc<Node>,
    pub(crate) kind: ActKind,
    pub(crate) text: Option<String>,
}

#[derive(Default)]
struct Acts {
    queue: VecDeque<Act>,
    bytes: usize,
    refused: u64,
    drains: u64,
}

/// A published recording, not yet taken by the host.
pub(crate) struct Published {
    pub(crate) lists: Vec<Vec<u8>>,
    pub(crate) size: (f32, f32),
}

struct After {
    due: f64,
    token: u64,
    order: u64,
}

/// The frame clock's registrations: live tickets in registration order, the
/// `after`s pending, and the virtual display's next frame, `base + k·1000/60`
/// from the session clock's time when the first ticket began.
#[derive(Default)]
struct Clock {
    tickets: BTreeSet<u64>,
    afters: Vec<After>,
    base: f64,
    k: u64,
    order: u64,
}

impl Clock {
    fn frame_due(&self) -> f64 {
        self.base + self.k as f64 * 1000.0 / 60.0
    }
}

/// What a session's handles and the host both reach, on the loop's thread.
pub(crate) struct Shared {
    /// The session clock as of the call being made.
    pub(crate) now: Cell<f64>,
    /// The session is the agent's.
    pub(crate) agent: Cell<bool>,
    store: RefCell<Store>,
    acts: RefCell<Acts>,
    clock: RefCell<Clock>,
    overlays: RefCell<BTreeMap<u32, Published>>,
    /// Observers in registration order: (token, node).
    observers: RefCell<Vec<(u64, u32)>>,
    tokens: Cell<u64>,
    overlay_refused: Cell<u64>,
    stops: Cell<u64>,
    /// A hatch asked for something the host does on its next turn.
    woke: Cell<bool>,
}

impl Shared {
    pub(crate) fn new(measuring: bool) -> Shared {
        Shared {
            now: Cell::new(0.0),
            agent: Cell::new(false),
            store: RefCell::new(Store::new(measuring)),
            acts: RefCell::default(),
            clock: RefCell::default(),
            overlays: RefCell::default(),
            observers: RefCell::default(),
            tokens: Cell::new(0),
            overlay_refused: Cell::new(0),
            stops: Cell::new(0),
            woke: Cell::new(false),
        }
    }

    pub(crate) fn store(&self) -> RefMut<'_, Store> {
        self.store.borrow_mut()
    }

    fn say(&self, line: String) {
        self.store().say(line);
    }

    fn token(&self) -> u64 {
        self.tokens.set(self.tokens.get() + 1);
        self.tokens.get()
    }

    /// Whether the host has something of the hatches' to do on a turn of its own.
    pub(crate) fn take_wake(&self) -> bool {
        self.woke.replace(false)
    }

    // The act queue (§2.5).

    /// Queue an act: one FIFO a session, at most 256 acts and 1 MB of text;
    /// an act past either is refused by name and counted.
    pub(crate) fn ask(&self, node: &Rc<Node>, kind: ActKind, text: Option<String>) {
        let what = format!("element {} #{}", node.word, node.id);
        if !node.live.get() {
            return self.say(format!(
                "{what}: {}() after its end does nothing",
                kind.name()
            ));
        }
        let size = text.as_ref().map_or(0, String::len);
        let mut acts = self.acts.borrow_mut();
        if size > INPUT_BYTES {
            acts.refused += 1;
            return self.say(format!("{what}: input refused: {size} bytes is over 64 KB"));
        }
        if acts.queue.len() >= ACTS || acts.bytes + size > ACT_BYTES {
            acts.refused += 1;
            return self.say(format!(
                "{what}: {} refused: the act queue is full",
                kind.name()
            ));
        }
        acts.bytes += size;
        acts.queue.push_back(Act {
            node: node.clone(),
            kind,
            text,
        });
        self.woke.set(true);
    }

    /// The acts queued now, at most 64, for one drain. What they queue waits
    /// for a later one.
    pub(crate) fn snapshot(&self) -> Vec<Act> {
        let mut acts = self.acts.borrow_mut();
        let n = acts.queue.len().min(SNAPSHOT);
        if n > 0 {
            acts.drains += 1;
        }
        let taken: Vec<Act> = acts.queue.drain(..n).collect();
        acts.bytes -= taken
            .iter()
            .map(|a| a.text.as_ref().map_or(0, String::len))
            .sum::<usize>();
        taken
    }

    /// An act the host could not run where it stood is counted with the refused.
    pub(crate) fn refused(&self) {
        self.acts.borrow_mut().refused += 1;
    }

    /// Acts still queued: what `clock settle` waits for.
    pub(crate) fn in_flight(&self) -> usize {
        self.acts.borrow().queue.len()
    }

    // The overlay (§2.2.1).

    /// Publish a recording for `node`, replacing one not yet taken: at most
    /// 4,096 ops and 256 KB, a larger one refused by name. False when refused.
    pub(crate) fn publish(&self, node: &Rc<Node>, lists: Vec<Vec<u8>>, frame: Rect) -> bool {
        let what = format!("element {} #{}", node.word, node.id);
        let refuse = |why: String| {
            self.overlay_refused.set(self.overlay_refused.get() + 1);
            self.say(format!("{what}: overlay refused: {why}"));
            false
        };
        if !node.live.get() {
            self.say(format!("{what}: overlay after its end does nothing"));
            return false;
        }
        let bytes: usize = lists.iter().map(Vec::len).sum();
        if bytes > OVERLAY_BYTES {
            return refuse(format!("{bytes} bytes is over 256 KB"));
        }
        let (mut ops, mut ids) = (0, Vec::new());
        for list in &lists {
            match exact_canvas::list::check(list, &mut ids) {
                Ok(n) => ops += n,
                Err(e) => return refuse(format!("{e}")),
            }
        }
        if ops > OVERLAY_OPS {
            return refuse(format!("{ops} ops is over 4,096"));
        }
        self.overlays.borrow_mut().insert(
            node.id,
            Published {
                lists,
                size: (frame.width, frame.height),
            },
        );
        self.woke.set(true);
        true
    }

    /// The recordings published since the last take, by node: the latest of each.
    pub(crate) fn take_overlays(&self) -> BTreeMap<u32, Published> {
        std::mem::take(&mut *self.overlays.borrow_mut())
    }

    // The frame clock (§2.4).

    pub(crate) fn frames(&self) -> u64 {
        let token = self.token();
        let mut clock = self.clock.borrow_mut();
        if clock.tickets.is_empty() {
            clock.base = self.now.get();
            clock.k = 1;
        }
        // A later token is a later registration: the set's order is theirs.
        clock.tickets.insert(token);
        self.woke.set(true);
        token
    }

    pub(crate) fn after(&self, ms: f64) -> u64 {
        let token = self.token();
        let mut clock = self.clock.borrow_mut();
        clock.order += 1;
        let order = clock.order;
        let ms = if ms.is_finite() { ms.max(0.0) } else { 0.0 };
        clock.afters.push(After {
            due: self.now.get() + ms,
            token,
            order,
        });
        self.woke.set(true);
        token
    }

    pub(crate) fn observe(&self, node: &Rc<Node>) -> u64 {
        let token = self.token();
        if node.live.get() {
            self.observers.borrow_mut().push((token, node.id));
        }
        token
    }

    /// A ticket, an `after` or an observer is stopped.
    pub(crate) fn stop(&self, token: u64) {
        let mut clock = self.clock.borrow_mut();
        clock.tickets.remove(&token);
        clock.afters.retain(|a| a.token != token);
        self.observers.borrow_mut().retain(|(t, _)| *t != token);
        self.stops.set(self.stops.get() + 1);
    }

    /// How many times something registered has been stopped or dropped: a
    /// module prunes its closures when this has moved.
    pub(crate) fn stops(&self) -> u64 {
        self.stops.get()
    }

    /// Whether `token` still names something to call.
    pub(crate) fn live(&self, token: u64) -> bool {
        let clock = self.clock.borrow();
        clock.tickets.contains(&token)
            || clock.afters.iter().any(|a| a.token == token)
            || self.observers.borrow().iter().any(|(t, _)| *t == token)
    }

    /// Live tickets, in registration order.
    pub(crate) fn tickets(&self) -> Vec<u64> {
        self.clock.borrow().tickets.iter().copied().collect()
    }

    /// A ticket is live.
    pub(crate) fn ticking(&self) -> bool {
        !self.clock.borrow().tickets.is_empty()
    }

    /// The next hatch instant, for a seek to stop at: the earliest live
    /// ticket's next frame or pending `after`.
    pub(crate) fn next_instant(&self) -> Option<f64> {
        let clock = self.clock.borrow();
        let after = clock.afters.iter().map(|a| a.due).reduce(f64::min);
        match clock.tickets.is_empty() {
            true => after,
            false => Some(clock.frame_due().min(after.unwrap_or(f64::INFINITY))),
        }
    }

    /// The earliest pending `after`: what the wall's loop sleeps until.
    pub(crate) fn after_due(&self) -> Option<f64> {
        let clock = self.clock.borrow();
        clock.afters.iter().map(|a| a.due).reduce(f64::min)
    }

    /// Whether the tickets' next virtual frame is due at `now`; it is then
    /// moved past `now`.
    pub(crate) fn frame_due(&self, now: f64) -> bool {
        let mut clock = self.clock.borrow_mut();
        if clock.tickets.is_empty() || now < clock.frame_due() {
            return false;
        }
        while clock.frame_due() <= now {
            clock.k += 1;
        }
        true
    }

    /// The `after`s due at `now`, taken, in due order, ties by registration.
    pub(crate) fn due_afters(&self, now: f64) -> Vec<u64> {
        let mut clock = self.clock.borrow_mut();
        let mut due: Vec<(f64, u64, u64)> = clock
            .afters
            .iter()
            .filter(|a| a.due <= now)
            .map(|a| (a.due, a.order, a.token))
            .collect();
        clock.afters.retain(|a| a.due > now);
        due.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        due.into_iter().map(|(_, _, token)| token).collect()
    }

    /// A reload: what was registered before it is dropped. How many of each.
    fn reset_clock(&self) -> (usize, usize) {
        let mut clock = self.clock.borrow_mut();
        let dropped = (clock.tickets.len(), clock.afters.len());
        *clock = Clock::default();
        self.stops.set(self.stops.get() + 1);
        dropped
    }

    /// The observers on `node`, in registration order.
    pub(crate) fn observers_on(&self, node: u32) -> Vec<u64> {
        let observers = self.observers.borrow();
        observers
            .iter()
            .filter(|(_, n)| *n == node)
            .map(|(t, _)| *t)
            .collect()
    }

    /// Whether any node is observed.
    pub(crate) fn observed(&self) -> bool {
        !self.observers.borrow().is_empty()
    }

    fn forget_node(&self, node: u32) {
        self.observers.borrow_mut().retain(|(_, n)| *n != node);
        self.overlays.borrow_mut().remove(&node);
        self.stops.set(self.stops.get() + 1);
    }
}

/// A hatched node the module has been told of.
pub(crate) struct Tracked {
    pub(crate) element: Element,
    pub(crate) key: exact_kernel::NodeKey,
    pub(crate) dataset: String,
    pub(crate) site: Option<u32>,
    pub(crate) in_list: bool,
}

/// An overlay as the painter shows it: its recording, the box size it was
/// recorded at, and the bitmap it replays to at a device scale.
pub(crate) struct Shown {
    pub(crate) pixels: Arc<tiny_skia::Pixmap>,
    pub(crate) size: (f32, f32),
    pub(crate) lists: Vec<Vec<u8>>,
    pub(crate) scale: f32,
}

/// A word's overlay protocol events, for `state.hatches`.
#[derive(Default, Clone, Copy)]
struct OverlayCounts {
    published: u64,
    dropped: u64,
}

/// One session's hatches.
#[derive(Default)]
pub(crate) struct Session {
    /// What the entry handed over; `None` for an app with no hatches module.
    pub(crate) install: Option<Install>,
    /// The first frame was presented, and what followed was decided.
    pub(crate) asked: bool,
    module: Option<Box<dyn Module>>,
    shared: Option<Rc<Shared>>,
    /// The nodes told, by id.
    pub(crate) nodes: BTreeMap<u32, Tracked>,
    /// Views a commit renewed for another item (LLP 1078): ended and built.
    pub(crate) renewed: Vec<u32>,
    /// The tree or its layout may have moved since the nodes were last read.
    pub(crate) stale: bool,
    calls: BTreeMap<String, BTreeMap<&'static str, u64>>,
    said: BTreeSet<String>,
    warned: BTreeSet<String>,
    /// Each word met and not called, with why: the plan does not give it to
    /// this platform, or the module was not built to handle it.
    unhandled: std::collections::BTreeMap<String, &'static str>,
    overlaid: BTreeMap<String, OverlayCounts>,
    /// What the painter shows over each node.
    pub(crate) overlays: BTreeMap<u32, Shown>,
    pub(crate) app_told: Option<App>,
    pub(crate) window_told: Option<Window>,
    crumbs: Option<(Breadcrumb, usize)>,
    incarnation: u32,
    /// What earlier runs left in their breadcrumbs.
    pub(crate) last_ends: Vec<String>,
    /// What a pointer that went down is captured by, until it lifts: the
    /// observed nodes along its hit chain, innermost first.
    pub(crate) capture: Option<Vec<u32>>,
    /// An observed event is being dispatched: a nested entry is part of it.
    pub(crate) marking: bool,
    /// Ticks and `after`s fired, and acts drained, in the agent command running.
    pub(crate) fires: u32,
    pub(crate) acted: u32,
}

impl Session {
    /// The hatches are connected: a module is there to call.
    pub(crate) fn connected(&self) -> bool {
        self.module.is_some()
    }

    pub(crate) fn shared(&self) -> Option<&Rc<Shared>> {
        self.shared.as_ref()
    }

    /// The words this platform's module handles.
    pub(crate) fn words(&self) -> &'static [&'static str] {
        self.install.map_or(&[], |i| i.words)
    }

    /// Connect: read what earlier runs left, take this run's breadcrumb, and
    /// make the app's value. After first pixel, never before.
    pub(crate) fn connect(&mut self, measuring: bool, agent: bool, app_id: &str) {
        let Some(install) = self.install else { return };
        let directory = super::breadcrumb::directory(app_id, agent);
        self.last_ends = Breadcrumb::read(&directory);
        self.crumbs = Breadcrumb::open(&directory, std::process::id())
            .and_then(|mut crumbs| crumbs.take("").map(|slot| (crumbs, slot)));
        let shared = Rc::new(Shared::new(measuring));
        shared.agent.set(agent);
        self.shared = Some(shared.clone());
        self.incarnation = 1;
        self.stale = true;
        // The app's own code from here: its `Default`.
        self.push("module", "built");
        self.module = Some((install.make)(shared));
        self.pop();
    }

    fn push(&mut self, scope: &str, moment: &str) {
        if let Some((crumbs, slot)) = &mut self.crumbs {
            crumbs.push(*slot, scope, moment, self.incarnation);
        }
    }

    fn pop(&mut self) {
        if let Some((crumbs, slot)) = &mut self.crumbs {
            crumbs.pop(*slot);
        }
    }

    /// One hatch call at session clock `now`: under the crash breadcrumb,
    /// production included (§4.4), and timed in a development build (§3.1).
    pub(crate) fn call(
        &mut self,
        now: f64,
        scope: &str,
        site: Option<u32>,
        moment: &'static str,
        counts: bool,
        body: impl FnOnce(&mut dyn Module),
    ) {
        let (Some(shared), true) = (self.shared.clone(), self.module.is_some()) else {
            return;
        };
        shared.now.set(now);
        self.push(scope, moment);
        let started = shared.store().begin_call();
        if let Some(module) = self.module.as_mut() {
            body(module.as_mut());
        }
        shared
            .store()
            .end_call(started, scope, site, moment, counts);
        self.pop();
    }

    /// The journal lines hatch code and the queue left, each without its prefix.
    pub(crate) fn take_lines(&mut self) -> Vec<String> {
        match &self.shared {
            Some(shared) => std::mem::take(&mut shared.store().lines),
            None => Vec::new(),
        }
    }

    /// Count a node's call by word and moment. True when the journal names
    /// it: in a list's row only the first of each moment is, so a fling
    /// does not flood it.
    pub(crate) fn count(&mut self, word: &str, moment: &'static str, in_list: bool) -> bool {
        let n = self
            .calls
            .entry(word.to_owned())
            .or_default()
            .entry(moment)
            .or_default();
        *n += 1;
        !in_list || *n == 1
    }

    /// The first time `word` is told: what a hatched node gives up here.
    pub(crate) fn first_said(&mut self, word: &str) -> bool {
        self.said.insert(word.to_owned())
    }

    /// The first time `word` is found in a list's row.
    pub(crate) fn first_warned(&mut self, word: &str) -> bool {
        self.warned.insert(word.to_owned())
    }

    /// The first time a word the module does not handle is met.
    pub(crate) fn first_unhandled(&mut self, word: &str, reason: &'static str) -> bool {
        self.unhandled.insert(word.to_owned(), reason).is_none()
    }

    /// A recording was shown (`dropped` false) or dropped at a new size.
    pub(crate) fn overlay_event(&mut self, word: &str, dropped: bool) {
        let counts = self.overlaid.entry(word.to_owned()).or_default();
        match dropped {
            true => counts.dropped += 1,
            false => counts.published += 1,
        }
    }

    /// A node has ended, its hatch told: nothing of it is kept.
    pub(crate) fn forget(&mut self, id: u32) {
        self.nodes.remove(&id);
        self.overlays.remove(&id);
        if let Some(shared) = &self.shared {
            shared.forget_node(id);
            shared.store().ended(id);
        }
    }

    /// A reload (§4.3): once every scope has ended, the old incarnation's
    /// registrations are dropped, with one journal line, and its counts
    /// start at nothing.
    pub(crate) fn reset(&mut self) -> Option<String> {
        let shared = self.shared.clone()?;
        let (tickets, afters) = shared.reset_clock();
        shared.observers.borrow_mut().clear();
        shared.overlays.borrow_mut().clear();
        *shared.acts.borrow_mut() = Acts::default();
        if let Some(module) = self.module.as_mut() {
            module.forget();
        }
        self.nodes.clear();
        self.overlays.clear();
        self.calls.clear();
        self.overlaid.clear();
        self.app_told = None;
        self.window_told = None;
        self.capture = None;
        self.stale = true;
        self.incarnation = self.incarnation.wrapping_add(1);
        shared.store().reset();
        (tickets + afters > 0).then(|| {
            format!("frames: {tickets} ticket(s) and {afters} after(s) from before the reload were dropped")
        })
    }

    /// The session's end, every scope told: the module goes, and a clean
    /// run's breadcrumb with it.
    pub(crate) fn end(&mut self) {
        self.module = None;
        if let Some((mut crumbs, slot)) = self.crumbs.take() {
            crumbs.free(slot);
            crumbs.close(true);
        }
    }

    /// `state.hatches` (§3.3), in the shape every host answers.
    pub(crate) fn state(&self) -> Value {
        let mut live: BTreeMap<&str, u64> = BTreeMap::new();
        for tracked in self.nodes.values() {
            *live.entry(tracked.element.hatch()).or_default() += 1;
        }
        let mut words = Map::new();
        let names: BTreeSet<&str> = live
            .keys()
            .copied()
            .chain(self.calls.keys().map(String::as_str))
            .collect();
        for word in names {
            let mut entry = json!({ "live": live.get(word).copied().unwrap_or(0), "reusable": 0, "lost": [],
                "calls": self.calls.get(word).cloned().unwrap_or_default() });
            if let Some(o) = self.overlaid.get(word) {
                let shown = self
                    .nodes
                    .iter()
                    .filter(|(id, t)| t.element.hatch() == word && self.overlays.contains_key(*id))
                    .count();
                entry["overlay"] =
                    json!({ "shown": shown, "published": o.published, "dropped": o.dropped });
            }
            words.insert(word.to_owned(), entry);
        }
        let mut reply = match &self.shared {
            Some(shared) => shared.store().state(words),
            None => Map::from_iter([("words".to_owned(), Value::Object(words))]),
        };
        let (refused, in_flight, overlay_refused) = match &self.shared {
            Some(shared) => (
                shared.acts.borrow().refused,
                shared.in_flight(),
                shared.overlay_refused.get(),
            ),
            None => (0, 0, 0),
        };
        if self.install.is_some() {
            reply.insert("platform".into(), json!(self.words()));
        }
        let unhandled: Vec<Value> = self
            .unhandled
            .iter()
            .map(|(word, reason)| json!({ "word": word, "reason": reason }))
            .collect();
        reply.insert("unhandled".into(), unhandled.into());
        reply.insert("refused".into(), refused.into());
        reply.insert("inFlight".into(), in_flight.into());
        if overlay_refused > 0 {
            reply.insert("overlaysRefused".into(), overlay_refused.into());
        }
        if !self.last_ends.is_empty() {
            reply.insert("lastEnd".into(), json!(self.last_ends));
        }
        Value::Object(super::diagnostics::fit(
            reply,
            &["words", "scopes", "unhandled"],
        ))
    }

    /// `perf hatches` (§3.3), with `perf`'s own tags.
    pub(crate) fn perf(&self, tags: Map<String, Value>) -> Value {
        match &self.shared {
            Some(shared) => shared.store().perf(tags, shared.tickets().len()),
            None => Store::new(false).perf(tags, 0),
        }
    }
}
