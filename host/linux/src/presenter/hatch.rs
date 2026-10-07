//! The presenter's half of the hatches (@ref LLP 1075.003.000.001 §2.1–§2.5,
//! §4.1): when each moment runs, and what a hatch's asks become.
//!
//! Hatch code runs on the loop's thread, between turns, never inside a
//! commit or the painter's walk. The hatches connect as first pixel is
//! acknowledged. A turn ([`Presenter::hatch_turn`]) drains one snapshot of
//! the act queue, each act a commit through the path a person's input
//! takes, then tells the moments the tree now holds: every scope's `ended`
//! first, then `built` and `changed` in the tree's order. The display loop's
//! turn is its own; the agent's is a command. Under the agent's clock a seek
//! stops at each hatch instant ([`Presenter::hatch_fire`]): that instant's
//! moments, its ticks, its due `after`s, and what they asked, drained there.
use super::*;
use crate::hatches::session::{Act, Shown, Tracked, COMMAND_LIMIT, SNAPSHOT};
use crate::hatches::{ActKind, App, Element, Frame, Input, Node, Phase, Rect, Window};
use exact_runner::ControlValue;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Where Exact stood before it dispatched an observed event, and who hears it.
pub(crate) struct Mark {
    seq: u64,
    scrolled: (f64, f64),
    chain: Vec<u32>,
}

/// A `dataset` prop's words: one JSON object of strings.
fn words(dataset: &str) -> BTreeMap<String, String> {
    serde_json::from_str::<BTreeMap<String, String>>(dataset).unwrap_or_default()
}

impl<D: DataSource> Presenter<D> {
    /// The app's hatches, from its entry: connected after first pixel.
    pub fn set_hatches(&mut self, install: Option<crate::hatches::Install>) {
        self.hatches.install = install;
    }

    /// First pixel is acknowledged: the app's value is made and its scopes
    /// are told. Once.
    pub(crate) fn connect_hatches(&mut self) {
        if std::mem::replace(&mut self.hatches.asked, true) {
            return;
        }
        if self.hatches.install.is_none() {
            if self.host.hatched {
                self.host.log("hatch: this host has no native objects, and this binary no hatches; hatched nodes are shown and never called");
            }
            return;
        }
        // `EXACT_HATCHES=off` (§2.6), a development build's switch: a
        // production one has dropped the name (`app.rs`).
        let production = exact_runner::delivery::production(&self.compat);
        if !production && std::env::var("EXACT_HATCHES").as_deref() == Ok("off") {
            self.host
                .log("hatch hatches: off (EXACT_HATCHES=off); no hatch is connected or called");
            return;
        }
        let agent = self.agent || std::env::var("EXACT_AGENT").as_deref() == Ok("1");
        let app_id = self.host.runner().data_ref().app_id().to_owned();
        self.hatches.connect(!production, agent, &app_id);
        for line in self.hatches.last_ends.clone() {
            self.host.log(format!("hatch: {line}"));
        }
        self.host.log("hatch: connected (this host has no native objects: a hatch gets its node's box, an overlay and the input that lands there)");
        self.hatch_moments();
    }

    /// One turn of the loop, or one agent command: a snapshot of the act
    /// queue runs, then the moments the tree now holds.
    pub fn hatch_turn(&mut self) {
        if self.hatches.connected() {
            self.hatch_drain();
            self.hatch_moments();
        }
    }

    /// The moments the tree now holds, and what the hatches published.
    pub fn hatch_moments(&mut self) {
        if self.hatches.connected() {
            self.hatch_scopes();
            self.hatch_nodes();
            self.hatch_requests();
        }
    }

    fn hatch_log(&mut self) {
        for line in self.hatches.take_lines() {
            self.host.log(format!("hatch {line}"));
        }
    }

    /// `app` and `window`: built as the hatches connect and after a reload,
    /// changed when a fact or the surface's size has moved since they were
    /// last told, once however many moved.
    fn hatch_scopes(&mut self) {
        let (page, preferences) = (
            self.host.runner().page(),
            self.host.runner().viewport().preferences,
        );
        let app = App {
            visibility_state: if page.hidden { "hidden" } else { "visible" },
            on_line: page.on_line,
            prefers_color_scheme: if preferences.dark { "dark" } else { "light" },
            prefers_contrast: preferences.contrast.keyword(),
            prefers_reduced_motion: preferences.reduced_motion,
            prefers_reduced_transparency: preferences.reduced_transparency,
            // The Linux and Windows entries' one session owns the process
            // and its window (§2.1.1).
            process_owner: true,
            is_new: false,
            is_live: true,
        };
        let now = self.host.now();
        if self.hatches.app_told.as_ref() != Some(&app) {
            let is_new = self.hatches.app_told.replace(app.clone()).is_none();
            let moment = if is_new { "built" } else { "changed" };
            self.host.log(format!("hatch app: {moment}"));
            let app = App { is_new, ..app };
            self.hatches
                .call(now, "app", None, moment, true, |m| m.app(&app, false));
            self.hatch_log();
        }
        let window = Window {
            frame: Rect {
                x: 0.0,
                y: 0.0,
                width: self.viewport.0,
                height: self.viewport.1,
            },
            scale: self.brush.scale,
            exclusive: true,
            is_new: false,
            is_live: true,
        };
        if self.hatches.window_told.as_ref() != Some(&window) {
            let is_new = self.hatches.window_told.replace(window.clone()).is_none();
            let moment = if is_new { "built" } else { "changed" };
            self.host.log(format!("hatch window: {moment}"));
            let window = Window { is_new, ..window };
            self.hatches.call(now, "window", None, moment, true, |m| {
                m.window(&window, false)
            });
            self.hatch_log();
        }
    }

    /// The hatched nodes the tree holds against those told: a node gone, or
    /// renewed for another item, ends; a new one is built; one whose words
    /// or box size moved is changed, its overlay dropped at a new size.
    fn hatch_nodes(&mut self) {
        if !std::mem::take(&mut self.hatches.stale) {
            return;
        }
        let kernel = self.host.kernel();
        let ids = match kernel.has_prop(PropId::Hatch) {
            true => kernel.preorder_where(&self.host.roots(), |_, props| {
                props.str(PropId::Hatch).is_some()
            }),
            false => Vec::new(),
        };
        let mut seen = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(node) = kernel.node(id) else {
                continue;
            };
            let frame = Rect {
                x: node.frame.x,
                y: node.frame.y,
                width: node.frame.width,
                height: node.frame.height,
            };
            let word = node.props.str(PropId::Hatch).unwrap_or("").to_owned();
            let dataset = node.props.str(PropId::Dataset).unwrap_or("").to_owned();
            seen.push((id, node.key, word, dataset, frame));
        }
        let renewed = std::mem::take(&mut self.hatches.renewed);
        let gone: Vec<u32> = self
            .hatches
            .nodes
            .iter()
            .filter(|(id, told)| {
                renewed.contains(*id)
                    || !seen
                        .iter()
                        .any(|s| s.0 == **id && s.1 == told.key && s.2 == told.element.hatch())
            })
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            self.hatch_ended(id);
        }
        for (id, key, word, dataset, frame) in seen {
            let Some(told) = self.hatches.nodes.get_mut(&id) else {
                self.hatch_built(id, key, word, dataset, frame);
                continue;
            };
            let before = told.element.frame();
            told.element.node.frame.set(frame);
            let resized = (before.width, before.height) != (frame.width, frame.height);
            let reworded = told.dataset != dataset;
            if !resized && !reworded {
                continue;
            }
            if reworded {
                *told.element.node.data.borrow_mut() = words(&dataset);
                told.dataset = dataset;
            }
            told.element.node.new.set(false);
            let (element, site, in_list) = (told.element.clone(), told.site, told.in_list);
            // A recording is for the size it was recorded at: never stretched.
            if resized && self.hatches.overlays.remove(&id).is_some() {
                self.hatches.overlay_event(&word, true);
                self.host.row_dirty.node(key);
                self.dirty = true;
            }
            self.hatch_element(&element, site, in_list, "changed");
        }
    }

    fn hatch_built(
        &mut self,
        id: ViewId,
        key: exact_kernel::NodeKey,
        word: String,
        dataset: String,
        frame: Rect,
    ) {
        // A word the module was not built to handle (§4.3): shown, never
        // called, journaled once and listed.
        if !self.hatches.words().contains(&word.as_str()) {
            if self.hatches.first_unhandled(&word) {
                self.host.log(format!("hatch element {word}: not handled by this build's module; its nodes are shown and never called"));
            }
            return;
        }
        let Some(shared) = self.hatches.shared().cloned() else {
            return;
        };
        let kernel = self.host.kernel();
        let html_id = kernel
            .node(id)
            .and_then(|n| n.props.str(PropId::Id))
            .unwrap_or("")
            .to_owned();
        let list = self.hatch_list(id);
        let site = self.host.runner().site_of(id).map(|(site, _)| site.0);
        let element = Element {
            node: Rc::new(Node {
                id,
                scope: Rc::from(format!("element {word}")),
                word: word.clone(),
                html_id,
                shared,
                live: Cell::new(true),
                new: Cell::new(true),
                frame: Cell::new(frame),
                data: RefCell::new(words(&dataset)),
            }),
        };
        let told = Tracked {
            element: element.clone(),
            key,
            dataset,
            site,
            in_list: list.is_some(),
        };
        self.hatches.nodes.insert(id, told);
        self.hatch_element(&element, site, list.is_some(), "built");
        // What a hatched node gives up here (LLP 1075.003.000 §3.5), once a word.
        if self.hatches.first_said(&word) {
            self.host.log(format!("hatch element {word}: nothing beyond the call on this host; a node with an overlay is painted each frame, outside its row's kept picture (LLP 1075.003.000)"));
        }
        if let Some(name) = list.filter(|_| self.hatches.first_warned(&word)) {
            self.host.log(format!("hatch element {word} is in a row of {name}: each row's mount calls its hatch on the host loop's thread"));
        }
    }

    /// The list whose row holds `id`, as the journal names it.
    fn hatch_list(&self, id: ViewId) -> Option<String> {
        let kernel = self.host.kernel();
        let mut at = kernel.node(id);
        let mut in_row = false;
        while let Some(node) = at {
            if in_row {
                return Some(match node.props.str(PropId::TestId) {
                    Some(name) => format!("list {name}"),
                    None => format!("list #{}", node.id),
                });
            }
            in_row = node.props.get(PropId::ListItemKey).is_some();
            at = node.parent.and_then(|parent| kernel.node(parent));
        }
        None
    }

    /// `element` (built, changed) or `element_ended` for one node.
    fn hatch_element(
        &mut self,
        element: &Element,
        site: Option<u32>,
        in_list: bool,
        moment: &'static str,
    ) {
        let (word, id) = (element.hatch().to_owned(), element.node());
        if self.hatches.count(&word, moment, in_list) {
            self.host
                .log(format!("hatch element {word} #{id}: {moment}"));
        }
        let now = self.host.now();
        let scope = format!("element {word}");
        self.hatches.call(now, &scope, site, moment, false, |m| {
            m.element(element, moment == "ended")
        });
        self.hatch_log();
    }

    /// A node left the tree: its hatch hears `ended`, its handle no longer
    /// live, and nothing of it is kept.
    fn hatch_ended(&mut self, id: u32) {
        let Some(told) = self.hatches.nodes.get(&id) else {
            return;
        };
        let (element, site, in_list) = (told.element.clone(), told.site, told.in_list);
        element.node.live.set(false);
        self.hatch_element(&element, site, in_list, "ended");
        self.dirty |= self.hatches.overlays.contains_key(&id);
        self.hatches.forget(id);
    }

    /// What the hatches published since the last turn: each node's latest
    /// recording replayed whole, in place of the one shown, and the box
    /// dirty. A publish wakes an idle loop, as a commit would.
    fn hatch_requests(&mut self) {
        let Some(shared) = self.hatches.shared().cloned() else {
            return;
        };
        let scale = self.brush.scale;
        // A new device scale: what is shown is replayed at it.
        let mut again: Vec<_> = (self.hatches.overlays.iter())
            .filter(|(_, shown)| shown.scale != scale)
            .map(|(id, shown)| (*id, shown.lists.clone(), shown.size, false))
            .collect();
        for (id, published) in shared.take_overlays() {
            again.retain(|(shown, ..)| *shown != id);
            again.push((id, published.lists, published.size, true));
        }
        for (id, lists, size, published) in again {
            let Some(told) = self.hatches.nodes.get(&id) else {
                continue;
            };
            let (word, key, frame) = (
                told.element.hatch().to_owned(),
                told.key,
                told.element.frame(),
            );
            // Recorded at a size the box no longer has: dropped, and its
            // hatch has heard `changed` to record again.
            if size != (frame.width, frame.height) {
                self.hatches.overlay_event(&word, true);
                continue;
            }
            let pixels = match lists.is_empty() {
                true => None,
                false => match self.host.replay_overlay(&lists, size, scale) {
                    Ok(pixels) => pixels,
                    Err(e) => {
                        self.host
                            .log(format!("hatch element {word} #{id}: overlay refused: {e}"));
                        continue;
                    }
                },
            };
            match pixels {
                Some(pixels) => {
                    let shown = Shown {
                        pixels,
                        size,
                        lists,
                        scale,
                    };
                    self.hatches.overlays.insert(id, shown);
                }
                None => {
                    self.hatches.overlays.remove(&id);
                }
            }
            if published {
                self.hatches.overlay_event(&word, false);
            }
            self.host.row_dirty.node(key);
            self.dirty = true;
        }
        if shared.take_wake() || shared.in_flight() > 0 {
            self.executor.notify();
        }
    }

    // Acts (§2.5).

    /// Run the acts queued now, at most 64, in order. What they queue (a
    /// `changed` that clicks again) waits for a later drain. How many ran.
    pub(crate) fn hatch_drain(&mut self) -> usize {
        let Some(shared) = self.hatches.shared().cloned() else {
            return 0;
        };
        let acts = shared.snapshot();
        let ran = acts.len();
        for act in acts {
            self.hatch_act(act);
        }
        self.hatch_log();
        ran
    }

    /// Acts still queued: what `clock settle` waits for.
    pub fn hatch_in_flight(&self) -> usize {
        (self.hatches.shared()).map_or(0, |shared| shared.in_flight())
    }

    /// One act, as a person's: a press at the node's own handler, the focus
    /// moved, or a field's whole value replaced through the field model,
    /// cut to the field's limits and reported as typing is, without moving
    /// the focus. Journaled before it runs, an `input` by length only.
    fn hatch_act(&mut self, act: Act) {
        let id = act.node.id;
        let what = format!("hatch element {} #{id}", act.node.word);
        let name = act.kind.name();
        let now = self.host.now();
        let facts = (self.host.kernel().node(id))
            .filter(|_| act.node.live.get())
            .map(|n| {
                (
                    n.node_type == NodeType::TextInput,
                    n.props.bool(PropId::Disabled) == Some(true),
                    n.props.bool(PropId::Editable) == Some(false),
                    n.props.str(PropId::Type) == Some("password"),
                )
            });
        let Some((field, disabled, readonly, protected)) = facts else {
            self.host
                .log(format!("{what}: {name}() after its end does nothing"));
            return;
        };
        let refusal = match act.kind {
            ActKind::Click if disabled => Some("the node is disabled"),
            ActKind::Input if !field => Some("not an editable text field"),
            ActKind::Input if disabled => Some("the field is disabled"),
            ActKind::Input if readonly => Some("the field is readonly"),
            _ => None,
        };
        if let Some(why) = refusal {
            if let Some(shared) = self.hatches.shared() {
                shared.refused();
            }
            self.host.log(format!("{what}: {name} refused: {why}"));
            return;
        }
        let error = match act.kind {
            ActKind::Click => {
                self.host.log(format!("{what}: click (delivery: hatch)"));
                if !self.toggle_control(id, now) {
                    if let Some(target) = self.handler_target(id, EventKind::Press) {
                        self.dispatch_press(target, now, false);
                    }
                }
                None
            }
            ActKind::Focus => {
                self.host.log(format!("{what}: focus (delivery: hatch)"));
                match self.focusable(id) {
                    true => self.set_focus(Some(id), now),
                    false => None,
                }
            }
            ActKind::Blur => {
                self.host.log(format!("{what}: blur (delivery: hatch)"));
                match self.focus == Some(id) {
                    true => self.set_focus(None, now),
                    false => None,
                }
            }
            ActKind::Input => {
                let text = act.text.unwrap_or_default();
                let value = match self.host.kernel().node(id) {
                    Some(n) => exact_kernel::control::limit_text(n.props, &text).to_owned(),
                    None => return,
                };
                let said = match protected {
                    true => "protected".to_owned(),
                    false => format!("{} chars", value.chars().count()),
                };
                self.host
                    .log(format!("{what}: input ({said}, delivery: hatch)"));
                // As `type` replaces a value (typing.rs), the focus where it is.
                let caret = exact_runner::FieldSelection::at_end(&value);
                self.mark_field(id, &value, caret, false);
                self.forget_replaced_choices();
                let bound = self.bound_text(id);
                self.host.values.watch(id, Some(bound.clone()));
                let heard = (self.host.runner().handlers_of(id)).contains(&EventKind::Input);
                let error = match heard {
                    true => {
                        let event = Event::Input(ControlValue::Field(value.clone(), caret));
                        self.host.dispatch_at(id, event, now)
                    }
                    false => None,
                };
                let after = self.after_commit();
                self.keep_typed(id, value, bound);
                error.or(after)
            }
        };
        if let Some(error) = error {
            self.host.log(format!("{what}: {name}: {error}"));
        }
    }

    // The frame clock (§2.4).

    /// The loop's next timer: the runner's, or a hatch's `after`.
    pub fn timer_due_ms(&self) -> Option<f64> {
        let after = self.hatches.shared().and_then(|shared| shared.after_due());
        (self.host.timer_due_ms().into_iter().chain(after)).reduce(f64::min)
    }

    /// Whether every display frame is wanted: a frame task's, or a ticket's.
    pub fn wants_frames(&self) -> bool {
        self.host.wants_frames() || self.hatch_ticking()
    }

    /// A frame ticket is live.
    pub(crate) fn hatch_ticking(&self) -> bool {
        (self.hatches.shared()).is_some_and(|shared| shared.ticking())
    }

    /// The next hatch instant at or before `to`, for an agent's seek to stop at.
    pub fn hatch_instant(&self, to: f64) -> Option<f64> {
        (self.hatches.shared())
            .filter(|_| self.hatches.connected())
            .and_then(|shared| shared.next_instant())
            .filter(|instant| *instant <= to)
    }

    /// An agent command begins: its caps start at nothing.
    pub fn hatch_command(&mut self) {
        self.hatches.fires = 0;
        self.hatches.acted = 0;
    }

    fn hatch_limit(&mut self, name: &'static str) -> Option<&'static str> {
        self.host.log(format!(
            "hatch refused advance: {name} ({COMMAND_LIMIT} in one command)"
        ));
        Some(name)
    }

    /// One tick or `after`, counted against the command's cap under the agent.
    fn hatch_fired(&mut self) -> bool {
        self.hatches.fires = self.hatches.fires.saturating_add(1);
        !self.agent || self.hatches.fires <= COMMAND_LIMIT
    }

    fn hatch_ticks(&mut self, now: f64) -> bool {
        let Some(shared) = self.hatches.shared().cloned() else {
            return true;
        };
        let frame = Frame {
            now,
            seq: self.host.runner().seq(),
        };
        for token in shared.tickets() {
            // One a tick before it stopped is not called.
            if !shared.live(token) {
                continue;
            }
            if !self.hatch_fired() {
                return false;
            }
            self.hatches
                .call(now, "frames", None, "tick", false, |m| m.tick(token, frame));
            self.hatch_log();
        }
        true
    }

    fn hatch_afters(&mut self, now: f64) -> bool {
        let Some(shared) = self.hatches.shared().cloned() else {
            return true;
        };
        for token in shared.due_afters(now) {
            if !self.hatch_fired() {
                return false;
            }
            self.hatches
                .call(now, "after", None, "fired", false, |m| m.fired(token));
            self.hatch_log();
        }
        true
    }

    /// Under the agent, at the instant the clock stands at, its commits in
    /// the tree (§2.4 steps 1–3): the moments they caused, each due tick in
    /// registration order, each due `after` in due order, then the acts
    /// those asked, snapshot after snapshot with the moments their commits
    /// cause and any `after` now due, until nothing is queued. The limit's
    /// name when a cap is passed.
    pub fn hatch_fire(&mut self) -> Option<&'static str> {
        let shared = self.hatches.shared().cloned()?;
        let now = self.host.now();
        self.hatch_moments();
        if shared.frame_due(now) && !self.hatch_ticks(now) {
            return self.hatch_limit("HatchFireLimit");
        }
        loop {
            if !self.hatch_afters(now) {
                return self.hatch_limit("HatchFireLimit");
            }
            let queued = shared.in_flight();
            if queued == 0 {
                if shared.after_due().is_some_and(|due| due <= now) {
                    continue;
                }
                return None;
            }
            self.hatches.acted += queued.min(SNAPSHOT) as u32;
            if self.hatches.acted > COMMAND_LIMIT {
                return self.hatch_limit("HatchActLimit");
            }
            self.hatch_drain();
            self.hatch_moments();
        }
    }

    /// A presented frame on the wall, after its tasks and timers: each
    /// ticket ticks, then each `after` now due fires. What they ask waits
    /// for the loop's next turn.
    pub(crate) fn hatch_presented(&mut self, now: f64) {
        if self.hatch_ticking() {
            self.hatch_moments();
            self.hatch_ticks(now);
        }
        self.hatch_timers(now);
    }

    /// The `after`s due on the wall.
    pub(crate) fn hatch_timers(&mut self, now: f64) {
        let due = (self.hatches.shared()).and_then(|shared| shared.after_due());
        if self.hatches.connected() && due.is_some_and(|due| due <= now) {
            self.hatch_afters(now);
            self.hatch_requests();
        }
    }

    // Observed input (§2.2.2).

    /// The observed nodes from `hit` up, innermost first.
    fn hatch_chain(&self, hit: Option<ViewId>) -> Vec<u32> {
        let Some(shared) = self.hatches.shared() else {
            return Vec::new();
        };
        let kernel = self.host.kernel();
        let mut chain = Vec::new();
        let mut at = hit.and_then(|id| kernel.node(id));
        while let Some(node) = at {
            if !shared.observers_on(node.id).is_empty() {
                chain.push(node.id);
            }
            at = node.parent.and_then(|parent| kernel.node(parent));
        }
        chain
    }

    fn hatch_progress(&self) -> (u64, (f64, f64)) {
        let page = (f64::from(self.page.0), f64::from(self.page.1));
        let scrolled = (self.scroll.values()).fold(page, |sum, offset| {
            (sum.0 + f64::from(offset.0), sum.1 + f64::from(offset.1))
        });
        (self.host.runner().seq(), scrolled)
    }

    fn hatch_mark(&mut self, chain: Vec<u32>) -> Option<Mark> {
        if chain.is_empty() {
            return None;
        }
        self.hatches.marking = true;
        let (seq, scrolled) = self.hatch_progress();
        Some(Mark {
            seq,
            scrolled,
            chain,
        })
    }

    /// Before Exact dispatches a pointer event at a window point: who will
    /// hear it afterwards. The hit chain Exact computes, innermost first; a
    /// pointer that went down keeps the chain of its down until it lifts.
    /// `None` when no observer hears it, or it is part of an event marked
    /// already.
    pub(crate) fn hatch_pointer_mark(&mut self, phase: Phase, x: f32, y: f32) -> Option<Mark> {
        let observed = (self.hatches.shared()).is_some_and(|shared| shared.observed());
        if self.hatches.marking || !(observed || self.hatches.capture.is_some()) {
            return None;
        }
        let chain = match (self.hatches.capture.take(), phase) {
            (Some(held), Phase::Move) => {
                self.hatches.capture = Some(held.clone());
                held
            }
            (Some(held), Phase::Up | Phase::Cancel) => held,
            (_, Phase::Down) => {
                let hit = self.hit(x, y);
                let chain = self.hatch_chain(hit);
                self.hatches.capture = Some(chain.clone());
                chain
            }
            (None, Phase::Cancel) => Vec::new(),
            (None, _) => {
                let hit = self.hit(x, y);
                self.hatch_chain(hit)
            }
        };
        self.hatch_mark(chain)
    }

    /// After Exact's dispatch: the marked chain hears the event, with
    /// whether a handler ran or a scroller moved for it.
    pub(crate) fn hatch_pointer(&mut self, mark: Option<Mark>, phase: Phase, x: f32, y: f32) {
        let Some(mark) = mark else { return };
        self.hatches.marking = false;
        let handled = self.hatch_progress() != (mark.seq, mark.scrolled);
        for id in mark.chain {
            let origin = (self.boxes.iter().rev().find(|b| b.id == id)).map(|b| b.rect);
            let local = origin.map_or((x, y), |rect| (x - rect.0, y - rect.1));
            let input = Input::Pointer {
                phase,
                x,
                y,
                local,
                handled,
            };
            self.hatch_heard(id, &input);
        }
    }

    /// The agent's `tap` is a press at a point with no down or up of its
    /// own: its observers hear both, the up with what the press did.
    pub(crate) fn hatch_tapped(&mut self, mark: Option<Mark>, x: f32, y: f32) {
        // No pointer is down after a tap, whoever heard it.
        self.hatches.capture = None;
        let Some(mark) = mark else { return };
        let unmoved = Mark {
            seq: self.hatch_progress().0,
            scrolled: self.hatch_progress().1,
            chain: mark.chain.clone(),
        };
        self.hatch_pointer(Some(unmoved), Phase::Down, x, y);
        self.hatch_pointer(Some(mark), Phase::Up, x, y);
    }

    /// Before a key is dispatched at the focus: the observers on the
    /// focused node and its ancestors, innermost first.
    pub(crate) fn hatch_key_mark(&mut self) -> Option<Mark> {
        let observed = (self.hatches.shared()).is_some_and(|shared| shared.observed());
        if self.hatches.marking || !observed {
            return None;
        }
        self.hatch_mark(self.hatch_chain(self.focus))
    }

    /// After the key's dispatch and its default action.
    pub(crate) fn hatch_key(
        &mut self,
        mark: Option<Mark>,
        key: &str,
        code: &str,
        down: bool,
        repeat: bool,
    ) {
        let Some(mark) = mark else { return };
        self.hatches.marking = false;
        let input = Input::Key {
            key: key.to_owned(),
            code: code.to_owned(),
            down,
            repeat,
            handled: self.hatch_progress() != (mark.seq, mark.scrolled),
        };
        for id in mark.chain {
            self.hatch_heard(id, &input);
        }
    }

    fn hatch_heard(&mut self, id: u32, input: &Input) {
        let (Some(shared), Some(told)) =
            (self.hatches.shared().cloned(), self.hatches.nodes.get(&id))
        else {
            return;
        };
        let (scope, site) = (format!("element {}", told.element.hatch()), told.site);
        let now = self.host.now();
        // In registration order; one stopped by an earlier is not called.
        for token in shared.observers_on(id) {
            if shared.live(token) {
                self.hatches
                    .call(now, &scope, site, "observed", false, |m| {
                        m.heard(token, input)
                    });
            }
        }
        self.hatch_log();
        self.hatch_requests();
    }

    // A reload, and the session's end (§4.3).

    /// Every hatched node, then the window, then the app, ends.
    fn hatch_close(&mut self) {
        let ids: Vec<u32> = self.hatches.nodes.keys().copied().collect();
        for id in ids {
            self.hatch_ended(id);
        }
        let now = self.host.now();
        if let Some(window) = self.hatches.window_told.take() {
            self.host.log("hatch window: ended");
            let window = Window {
                is_live: false,
                ..window
            };
            self.hatches.call(now, "window", None, "ended", true, |m| {
                m.window(&window, true)
            });
        }
        if let Some(app) = self.hatches.app_told.take() {
            self.host.log("hatch app: ended");
            let app = App {
                is_live: false,
                ..app
            };
            self.hatches
                .call(now, "app", None, "ended", true, |m| m.app(&app, true));
        }
        self.hatch_log();
    }

    /// Another plan took over: every scope ends with the handle it had,
    /// then the old incarnation's registrations are dropped. All are built
    /// again, with new handles, on the next turn.
    pub(crate) fn hatch_reset(&mut self) {
        if self.hatches.connected() {
            self.hatch_close();
            if let Some(line) = self.hatches.reset() {
                self.host.log(format!("hatch {line}"));
            }
        }
    }

    /// The session's end: every scope ends before the module goes, and a
    /// run that ends here leaves no breadcrumb.
    pub fn end_hatches(&mut self) {
        if self.hatches.connected() {
            self.hatch_close();
            self.hatches.end();
        }
    }

    // Reads, which change nothing (§3.3).

    /// `state.hatches`, for an app that has hatches.
    pub fn hatch_state(&self) -> Option<serde_json::Value> {
        self.hatches.install.map(|_| self.hatches.state())
    }

    /// `perf hatches`, with the tags of the runner's own `perf` read.
    pub fn hatch_perf(&self) -> serde_json::Value {
        let perf: serde_json::Value =
            serde_json::from_str(&self.host.agent("{\"op\":\"perf\"}")).unwrap_or_default();
        let tags = ["seq", "plan", "incarnation", "clock", "epoch"]
            .into_iter()
            .filter_map(|key| Some((key.to_owned(), perf.get(key)?.clone())))
            .collect();
        self.hatches.perf(tags)
    }

    /// `perf <target>`: a hatched site's row names its hatch's calls and time.
    pub fn hatch_sites(&self, reply: String) -> String {
        let Some(shared) = self.hatches.shared() else {
            return reply;
        };
        let Ok(mut perf) = serde_json::from_str::<serde_json::Value>(&reply) else {
            return reply;
        };
        let mut found = false;
        for row in perf["sites"].as_array_mut().into_iter().flatten() {
            let site = row["site"]
                .as_u64()
                .and_then(|site| u32::try_from(site).ok());
            if let Some((calls, ms)) = site.and_then(|site| shared.store().site(site)) {
                row["hatch"] = serde_json::json!({ "calls": calls, "ms": ms });
                found = true;
            }
        }
        match found {
            true => perf.to_string(),
            false => reply,
        }
    }

    /// Whether a hatch asked for Save Trace, a second since the last (§3.2).
    pub fn hatch_trace_asked(&self) -> bool {
        (self.hatches.shared()).is_some_and(|shared| shared.store().take_trace())
    }
}

#[cfg(test)]
#[path = "hatch_tests.rs"]
mod tests;
