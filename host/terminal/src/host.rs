//! The runner wrapped for a cell painter: commits → layout → cells, and the
//! keyboard and mouse back as events.
//!
//! @ref LLP 1101 D6 (input and focus), D7 (the kernel is the display list),
//! D10 (full screen and inline)

use crate::grid::CellRect;
use crate::paint::{paint, Painted, Scene};
use exact_kernel::style::cells::{COLUMN, ROW};
use exact_kernel::{AxisOffer, Kernel, NodeType, Offer, PropId, ViewId};
use exact_plan::{EventKind, Plan};
use exact_runner::{DataSource, Event, Runner, Viewport};
use std::collections::HashMap;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// A key, decoded from the terminal or named by the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// A printable character.
    Char(char),
    /// A named key in Playwright's spelling: `Enter`, `Tab`, `ArrowUp`…
    Named(&'static str),
    /// Control plus a letter.
    Ctrl(char),
    /// Shift+Tab.
    BackTab,
    /// Any other chord, in Playwright's spelling: `Shift+Enter`, `Alt+b`.
    Chord(String),
}

impl Key {
    /// The chord in Playwright's spelling, as `aria-keyshortcuts` and `key`
    /// handlers name it.
    pub fn chord(&self) -> String {
        match self {
            Key::Char(' ') => "Space".into(),
            Key::Char(c) => c.to_string(),
            Key::Named(n) => n.to_string(),
            Key::Ctrl(c) => format!("Control+{c}"),
            Key::BackTab => "Shift+Tab".into(),
            Key::Chord(c) => c.clone(),
        }
    }
}

/// What the loop should do after an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    /// Repaint if anything changed.
    Continue,
    /// Leave.
    Quit,
}

/// How the app occupies the terminal (LLP 1101 D10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The alternate screen, every cell the app's, the mouse captured.
    Fullscreen,
    /// The normal screen: settled transcript rows go to the terminal's own
    /// scrollback once; only the live tail is redrawn. No mouse capture, so
    /// selection, copy and scroll are the terminal's.
    Inline,
}

/// One runner, one grid.
pub struct Host<D: DataSource> {
    pub(crate) runner: Runner<D>,
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    /// How the app occupies the terminal.
    pub mode: Mode,
    pub(crate) focus: Option<ViewId>,
    pub(crate) caret: usize,
    pub(crate) scroll: HashMap<ViewId, f32>,
    /// Open dialogs and popovers, bottom to top, with the focus each took.
    pub(crate) layers: Vec<(ViewId, Option<ViewId>)>,
    /// The appearance `light-dark()` resolves by.
    pub dark: bool,
    painted: Option<Painted>,
    /// Bumped by every change a frame can show.
    pub generation: u64,
    pub(crate) quit: bool,
    /// A handler called `preventDefault()` since this was last cleared.
    pub(crate) prevented: bool,
    /// Decoded images by source.
    pub images: crate::image::Images,
}

impl<D: DataSource> Host<D> {
    /// Boot a plan in a `cols` × `rows` terminal.
    pub fn boot(
        plan: Plan,
        data: D,
        mode: Mode,
        cols: usize,
        rows: usize,
    ) -> Result<Host<D>, String> {
        exact_kernel::style::cells::set_terminal();
        exact_kernel::style::link_segments();
        exact_kernel::timeline::link();
        let kernel = Kernel::new(Box::new(crate::measure::CellMeasurer));
        let viewport = Viewport::sized(cols as f64 * COLUMN as f64, rows as f64 * ROW as f64);
        let runner =
            Runner::boot(plan, data, kernel, viewport, "/").map_err(|e| format!("{e:?}"))?;
        let mut host = Host {
            runner,
            cols,
            rows,
            mode,
            focus: None,
            caret: 0,
            scroll: HashMap::new(),
            layers: Vec::new(),
            dark: true,
            painted: None,
            generation: 0,
            quit: false,
            prevented: false,
            images: crate::image::Images::default(),
        };
        host.after_commit();
        Ok(host)
    }

    /// Wake `waker` whenever the data module announces a change from another
    /// thread (LLP 1016.002): the loop then calls [`Host::announced`].
    pub fn listen(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.runner.listen(waker);
    }

    /// Apply what the data module announced: the watching resources are
    /// asked again.
    pub fn announced(&mut self) {
        let (receipts, _) = self.runner.apply_announced();
        if !receipts.is_empty() {
            self.after_commit();
        }
    }

    /// The kernel, for the agent's questions.
    pub fn kernel(&self) -> &Kernel {
        self.runner.kernel()
    }

    /// The grid size.
    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    /// Whether an action asked the host to close.
    pub fn quitting(&self) -> bool {
        self.quit
    }

    /// Lay out again, take the commands actions emitted, keep the focus on
    /// a node that still exists, and mark the frame stale.
    pub(crate) fn after_commit(&mut self) {
        let trace = std::env::var_os("EXACT_TERMINAL_TRACE").is_some();
        let t0 = std::time::Instant::now();
        let width = AxisOffer::Definite(self.cols as f32 * COLUMN);
        let offer = match self.mode {
            Mode::Fullscreen => Offer::definite(self.cols as f32 * COLUMN, self.rows as f32 * ROW),
            Mode::Inline => Offer {
                width,
                height: AxisOffer::MaxContent,
            },
        };
        for root in self.runner.roots() {
            let _ = self.runner.kernel_mut().compute_layout(root, offer);
        }
        let t_layout = t0.elapsed();
        for command in self.runner.take_commands() {
            match command.name.as_str() {
                "close" => self.quit = true,
                "preventDefault" => self.prevented = true,
                _ => {}
            }
        }
        self.layers
            .retain(|(id, _)| self.runner.kernel().node(*id).is_some());
        if self.focus.is_some_and(|f| !self.focusables().contains(&f)) {
            self.focus = None;
        }
        if self.focus.is_none() {
            self.focus = self.autofocus();
            if let Some(f) = self.focus {
                self.caret = self.value(f).graphemes(true).count();
            }
        }
        let t_focus = t0.elapsed();
        self.follow_ends();
        self.images.load(self.runner.kernel());
        self.changed();
        if trace {
            eprintln!(
                "commit: layout {:?}, focus {:?}, walks {:?}",
                t_layout,
                t_focus - t_layout,
                t0.elapsed() - t_focus
            );
        }
    }

    pub(crate) fn changed(&mut self) {
        self.painted = None;
        self.generation += 1;
    }

    /// The terminal changed size.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        if (cols, rows) == (self.cols, self.rows) {
            return;
        }
        (self.cols, self.rows) = (cols, rows);
        let _ = self
            .runner
            .set_viewport(cols as f64 * COLUMN as f64, rows as f64 * ROW as f64);
        self.after_commit();
    }

    fn scene(&self) -> Scene<'_> {
        Scene {
            kernel: self.runner.kernel(),
            focus: self.focus,
            scroll: &self.scroll,
            caret: self.caret,
            dark: self.dark,
            layers: self.layers.iter().map(|(id, _)| *id).collect(),
            images: &self.images,
            center: self.mode == Mode::Fullscreen,
        }
    }

    /// The current full-screen frame, painted once per change.
    pub fn frame(&mut self) -> &Painted {
        if self.painted.is_none() {
            let roots = self.runner.roots();
            let painted = paint(&self.scene(), &roots, self.cols, self.rows, 0);
            self.painted = Some(painted);
        }
        self.painted.as_ref().expect("painted")
    }

    /// Rows `top..top + count` of the whole document, painted (inline mode).
    pub fn render(&self, top: usize, count: usize) -> Painted {
        let roots = self.runner.roots();
        paint(&self.scene(), &roots, self.cols, count, top)
    }

    /// The document's height in rows (inline mode lays out to content).
    pub fn document_rows(&self) -> usize {
        let kernel = self.runner.kernel();
        self.runner
            .roots()
            .iter()
            .filter_map(|r| kernel.node(*r))
            .chain(self.layers.iter().filter_map(|(id, _)| kernel.node(*id)))
            .map(|n| ((n.frame.y + n.frame.height) / ROW).round().max(0.0) as usize)
            .max()
            .unwrap_or(0)
    }

    /// The settled transcript (LLP 1101 D10): the bottom row of each child
    /// of the `role="log"` node, in order, and how many lead children are
    /// settled — those before the first `aria-busy` one. Settled children
    /// never change again, so an inline terminal prints them once into its
    /// scrollback.
    pub fn transcript(&self) -> (Vec<usize>, usize) {
        let kernel = self.runner.kernel();
        let Some(log) = self.find(|n| n.props.str(PropId::AccessibilityRole) == Some("log")) else {
            return (Vec::new(), 0);
        };
        let Some(node) = kernel.node(log) else {
            return (Vec::new(), 0);
        };
        let mut bottoms = Vec::new();
        let mut settled = None;
        for child in node.children() {
            let Some(c) = kernel.node(child) else {
                continue;
            };
            if c.style.display == exact_kernel::Display::None {
                continue;
            }
            if settled.is_none() && c.props.bool(PropId::AccessibilityBusy) == Some(true) {
                settled = Some(bottoms.len());
            }
            bottoms.push(((c.frame.y + c.frame.height) / ROW).round().max(0.0) as usize);
        }
        let settled = settled.unwrap_or(bottoms.len());
        (bottoms, settled)
    }

    /// The first node in document order that `pred` accepts.
    pub(crate) fn find(&self, pred: impl Fn(&exact_kernel::NodeRef<'_>) -> bool) -> Option<ViewId> {
        let kernel = self.runner.kernel();
        let mut stack: Vec<ViewId> = self.runner.roots().into_iter().rev().collect();
        while let Some(id) = stack.pop() {
            let Some(n) = kernel.node(id) else { continue };
            if pred(&n) {
                return Some(id);
            }
            if n.node_type != NodeType::Text {
                stack.extend(n.children().into_iter().rev());
            }
        }
        None
    }

    /// Advance the app's clock: timers fire.
    pub fn tick(&mut self, now_ms: f64) {
        if !self.runner.advance_timed(now_ms).receipts.is_empty() {
            self.after_commit();
        }
    }

    /// When the next timer is due, in the app's clock.
    pub fn timer_due_ms(&self) -> Option<f64> {
        self.runner.timer_due_ms()
    }

    /// Dispatch, then lay out; whether the commit changed the tree.
    pub(crate) fn dispatch(&mut self, view: ViewId, event: Event) -> bool {
        let changed = match self.runner.dispatch(view, event) {
            Ok(r) => !(r.created.is_empty() && r.destroyed.is_empty() && r.touched.is_empty()),
            Err(_) => false,
        };
        self.after_commit();
        changed
    }

    /// The nearest node at or above `view` with a handler for `kind`.
    pub(crate) fn handler(&self, view: ViewId, kind: EventKind) -> Option<ViewId> {
        let mut at = Some(view);
        while let Some(id) = at {
            if self.runner.handlers_of(id).contains(&kind) {
                return Some(id);
            }
            at = self.kernel().node(id)?.parent;
        }
        None
    }

    pub(crate) fn is_field(&self, id: ViewId) -> bool {
        self.kernel()
            .node(id)
            .is_some_and(|n| n.node_type == NodeType::TextInput)
    }

    pub(crate) fn is_textarea(&self, id: ViewId) -> bool {
        self.kernel()
            .node(id)
            .is_some_and(|n| n.props.str(PropId::SemanticTag) == Some("textarea"))
    }

    pub(crate) fn value(&self, id: ViewId) -> String {
        self.kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value).map(str::to_string))
            .unwrap_or_default()
    }

    /// A dialog or popover: painted only while open, above everything.
    pub(crate) fn is_layer(n: &exact_kernel::NodeRef<'_>) -> bool {
        n.props.str(PropId::SemanticTag) == Some("dialog") || n.props.contains(PropId::Popover)
    }

    /// Interactive nodes in document order, from the semantic tree, not the
    /// paint: a row scrolled out of view is still reachable, and a closed
    /// dialog's are not. With a dialog open, only its own.
    pub(crate) fn focusables(&self) -> Vec<ViewId> {
        let kernel = self.runner.kernel();
        let mut out = Vec::new();
        let starts: Vec<ViewId> = match self.layers.last() {
            Some((top, _)) => vec![*top],
            None => self.runner.roots(),
        };
        let mut stack: Vec<ViewId> = starts.into_iter().rev().collect();
        let open: Vec<ViewId> = self.layers.iter().map(|(id, _)| *id).collect();
        while let Some(id) = stack.pop() {
            let Some(n) = kernel.node(id) else { continue };
            if n.style.display == exact_kernel::Display::None
                || n.props.bool(PropId::Disabled) == Some(true)
                || (Self::is_layer(&n) && !open.contains(&id))
            {
                continue;
            }
            if matches!(
                n.node_type,
                NodeType::Pressable | NodeType::TextInput | NodeType::Control
            ) {
                out.push(id);
            }
            if n.node_type != NodeType::Text {
                stack.extend(n.children().into_iter().rev());
            }
        }
        out
    }

    fn autofocus(&self) -> Option<ViewId> {
        let kernel = self.runner.kernel();
        self.focusables().into_iter().find(|id| {
            kernel
                .node(*id)
                .is_some_and(|n| n.props.bool(PropId::Autofocus) == Some(true))
        })
    }

    /// Move the focus to a node (or nowhere), telling the app.
    pub fn focus(&mut self, to: Option<ViewId>) {
        if to == self.focus {
            return;
        }
        if let Some(old) = self.focus.take() {
            if self.runner.handlers_of(old).contains(&EventKind::Blur) {
                self.dispatch(old, Event::Blur);
            }
        }
        self.focus = to;
        if let Some(new) = to {
            self.caret = self.value(new).graphemes(true).count();
            if self.runner.handlers_of(new).contains(&EventKind::Focus) {
                self.dispatch(new, Event::Focus);
            }
            self.reveal(new);
        }
        self.changed();
    }

    pub(crate) fn step_focus(&mut self, back: bool) {
        let all = self.focusables();
        if all.is_empty() {
            return;
        }
        let at = self.focus.and_then(|f| all.iter().position(|id| *id == f));
        let next = match (at, back) {
            (None, false) => 0,
            (None, true) => all.len() - 1,
            (Some(i), false) => (i + 1) % all.len(),
            (Some(i), true) => (i + all.len() - 1) % all.len(),
        };
        self.focus(Some(all[next]));
    }

    fn is_scroller(n: &exact_kernel::NodeRef<'_>) -> bool {
        n.node_type == NodeType::ScrollView
            || matches!(
                n.style.overflow_y,
                exact_kernel::Overflow::Scroll | exact_kernel::Overflow::Auto
            )
    }

    /// Scroll every scroller above `id` so its frame is in view.
    fn reveal(&mut self, id: ViewId) {
        let kernel = self.runner.kernel();
        let Some(target) = kernel.node(id).map(|n| n.frame) else {
            return;
        };
        let mut at = kernel.node(id).and_then(|n| n.parent);
        let mut changes = Vec::new();
        while let Some(p) = at {
            let Some(n) = kernel.node(p) else { break };
            if Self::is_scroller(&n) {
                let [bt, _, bb, _] = n.style.border_widths();
                let top = n.frame.y + bt;
                let height = n.frame.height - bt - bb;
                let offset = self.scroll.get(&p).copied().unwrap_or(0.0);
                let y = target.y - top;
                let next = if y < offset {
                    y
                } else if y + target.height > offset + height {
                    y + target.height - height
                } else {
                    offset
                };
                changes.push((p, next.max(0.0)));
            }
            at = n.parent;
        }
        self.scroll.extend(changes);
    }

    /// Keep a `scrollFollowEnd` scroller at its end while it was there.
    fn follow_ends(&mut self) {
        let kernel = self.runner.kernel();
        let mut stack: Vec<ViewId> = self.runner.roots();
        let mut ends = Vec::new();
        while let Some(id) = stack.pop() {
            let Some(n) = kernel.node(id) else { continue };
            if Self::is_scroller(&n) && n.props.bool(PropId::ScrollFollowEnd) == Some(true) {
                let [bt, _, bb, _] = n.style.border_widths();
                let reach = (n.content.1 - (n.frame.height - bt - bb)).max(0.0);
                ends.push((id, reach));
            }
            if n.node_type != NodeType::Text {
                stack.extend(n.children());
            }
        }
        for (id, reach) in ends {
            let at = self.scroll.entry(id).or_insert(f32::MAX);
            // Within a row of the end counts as at the end.
            if *at >= reach - ROW || *at == f32::MAX {
                *at = reach;
            }
        }
    }

    /// Press a node as a click or Enter would: a field takes the focus, an
    /// invoker opens or closes its dialog, a button runs its handler.
    pub fn press(&mut self, id: ViewId) {
        if self.is_field(id) {
            self.focus(Some(id));
            return;
        }
        self.invoke(id);
        if let Some(target) = self.handler(id, EventKind::Press) {
            if self.layers.is_empty() || self.focusables().contains(&id) {
                self.focus(Some(id));
            }
            self.dispatch(target, Event::Press);
        }
    }

    /// `commandfor`/`command` and `popovertarget`, before the press's own
    /// handler runs (as the Mac host does, LLP 1021).
    fn invoke(&mut self, id: ViewId) {
        let kernel = self.runner.kernel();
        let Some(n) = kernel.node(id) else { return };
        let (target, action) = if let Some(t) = n.props.str(PropId::Commandfor) {
            (
                t.to_string(),
                n.props
                    .str(PropId::Command)
                    .unwrap_or("show-modal")
                    .to_string(),
            )
        } else if let Some(t) = n.props.str(PropId::Popovertarget) {
            let action = n.props.str(PropId::Popovertargetaction).unwrap_or("toggle");
            (t.to_string(), action.to_string())
        } else {
            return;
        };
        let Some(layer) = self.find(|n| n.props.str(PropId::Id) == Some(target.as_str())) else {
            return;
        };
        let open = self.layers.iter().any(|(l, _)| *l == layer);
        match (action.as_str(), open) {
            ("show-modal" | "show" | "show-popover", false) | ("toggle", false) => {
                self.open_layer(layer)
            }
            ("close" | "hide" | "hide-popover", true) | ("toggle", true) => self.close_layer(layer),
            _ => {}
        }
    }

    /// Open a dialog or popover: on top, its first autofocus or focusable
    /// descendant focused, the focus it took remembered.
    pub fn open_layer(&mut self, layer: ViewId) {
        self.layers.push((layer, self.focus));
        let kernel = self.runner.kernel();
        let all = self.focusables();
        let first = all
            .iter()
            .copied()
            .find(|id| {
                kernel
                    .node(*id)
                    .is_some_and(|n| n.props.bool(PropId::Autofocus) == Some(true))
            })
            .or_else(|| all.first().copied());
        self.focus = None;
        self.focus(first);
        self.changed();
    }

    /// Close a dialog or popover, giving back the focus it took.
    pub fn close_layer(&mut self, layer: ViewId) {
        let Some(at) = self.layers.iter().position(|(l, _)| *l == layer) else {
            return;
        };
        let (_, restore) = self.layers.remove(at);
        self.focus = None;
        self.focus(restore.filter(|r| self.runner.kernel().node(*r).is_some()));
        self.changed();
    }

    /// The topmost interactive node at a cell.
    pub fn hit(&mut self, x: i32, y: i32) -> Option<ViewId> {
        self.frame()
            .hits
            .iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(id, _)| *id)
    }

    /// A mouse press at a cell. With a dialog open, a press outside it
    /// closes it only when `closedby="any"`.
    pub fn click(&mut self, x: i32, y: i32) {
        let hit = self.hit(x, y);
        if let Some((top, _)) = self.layers.last().copied() {
            let inside = hit.is_some_and(|h| self.focusables().contains(&h));
            if !inside {
                let any = self
                    .kernel()
                    .node(top)
                    .and_then(|n| n.props.str(PropId::Closedby).map(str::to_string));
                if any.as_deref() == Some("any") {
                    self.close_layer(top);
                }
                return;
            }
        }
        match hit {
            Some(id) => self.press(id),
            None => self.focus(None),
        }
    }

    /// A wheel at a cell: the innermost scroller under it moves by rows.
    pub fn wheel(&mut self, x: i32, y: i32, rows: i32) {
        let found = self
            .frame()
            .scrollers
            .iter()
            .rev()
            .find(|(_, r, _)| r.contains(x, y))
            .map(|(id, _, reach)| (*id, *reach));
        if let Some((id, reach)) = found {
            let at = self.scroll.entry(id).or_insert(0.0);
            *at = (at.min(reach) + rows as f32 * ROW).clamp(0.0, reach);
            self.changed();
        }
    }

    /// The node an agent names by `testId`.
    pub fn by_test_id(&self, id: &str) -> Option<ViewId> {
        let kernel = self.kernel();
        kernel
            .find_by_test_id(id)
            .first()
            .and_then(|k| kernel.node_by_key(*k))
            .map(|n| n.id)
    }

    /// A node's box in cells, as the painter snapped it.
    pub fn cells_of(&self, id: ViewId) -> Option<CellRect> {
        let f = self.kernel().node(id)?.frame;
        let x = (f.x / COLUMN).round() as i32;
        let y = (f.y / ROW).round() as i32;
        Some(CellRect {
            x,
            y,
            w: ((f.x + f.width) / COLUMN).round() as i32 - x,
            h: ((f.y + f.height) / ROW).round() as i32 - y,
        })
    }
}
