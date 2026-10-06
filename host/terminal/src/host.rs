//! The runner wrapped for a cell painter: commits → layout → cells, and the
//! keyboard and mouse back as events.
//!
//! @ref LLP 1101 D6 (input and focus), D7 (the kernel is the display list)

use crate::grid::CellRect;
use crate::paint::{paint, Painted, Scene};
use exact_kernel::style::cells::{COLUMN, ROW};
use exact_kernel::{Kernel, NodeType, Offer, PropId, ViewId};
use exact_plan::{EventKind, Plan};
use exact_runner::{ControlValue, Event, Runner, Viewport};
use std::collections::HashMap;
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
}

/// What the loop should do after an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    /// Repaint if anything changed.
    Continue,
    /// Leave.
    Quit,
}

/// One runner, one grid.
pub struct Host {
    runner: Runner<()>,
    cols: usize,
    rows: usize,
    focus: Option<ViewId>,
    caret: usize,
    scroll: HashMap<ViewId, f32>,
    /// The appearance `light-dark()` resolves by.
    pub dark: bool,
    painted: Option<Painted>,
}

impl Host {
    /// Boot a plan in a `cols` × `rows` terminal.
    pub fn boot(plan: Plan, cols: usize, rows: usize) -> Result<Host, String> {
        exact_kernel::style::cells::set_terminal();
        exact_kernel::style::link_segments();
        exact_kernel::timeline::link();
        let kernel = Kernel::new(Box::new(crate::measure::CellMeasurer));
        let viewport = Viewport::sized(cols as f64 * COLUMN as f64, rows as f64 * ROW as f64);
        let runner = Runner::boot(plan, (), kernel, viewport, "/").map_err(|e| format!("{e:?}"))?;
        let mut host = Host {
            runner,
            cols,
            rows,
            focus: None,
            caret: 0,
            scroll: HashMap::new(),
            dark: true,
            painted: None,
        };
        host.layout();
        Ok(host)
    }

    /// The kernel, for the agent's questions.
    pub fn kernel(&self) -> &Kernel {
        self.runner.kernel()
    }

    /// The grid size.
    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    fn layout(&mut self) {
        let offer = Offer::definite(self.cols as f32 * COLUMN, self.rows as f32 * ROW);
        for root in self.runner.roots() {
            let _ = self.runner.kernel_mut().compute_layout(root, offer);
        }
        self.painted = None;
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
        self.layout();
    }

    /// The current frame, painted once per change.
    pub fn frame(&mut self) -> &Painted {
        if self.painted.is_none() {
            let roots = self.runner.roots();
            let scene = Scene {
                kernel: self.runner.kernel(),
                focus: self.focus,
                scroll: &self.scroll,
                caret: self.caret,
                dark: self.dark,
            };
            self.painted = Some(paint(&scene, &roots, self.cols, self.rows));
        }
        self.painted.as_ref().expect("painted")
    }

    /// Advance the app's clock: timers fire.
    pub fn tick(&mut self, now_ms: f64) {
        if !self.runner.advance_timed(now_ms).receipts.is_empty() {
            self.layout();
        }
    }

    fn dispatch(&mut self, view: ViewId, event: Event) -> bool {
        let ok = self.runner.dispatch(view, event).is_ok();
        self.layout();
        ok
    }

    /// The nearest node at or above `view` with a handler for `kind`.
    fn handler(&self, view: ViewId, kind: EventKind) -> Option<ViewId> {
        let mut at = Some(view);
        while let Some(id) = at {
            if self.runner.handlers_of(id).contains(&kind) {
                return Some(id);
            }
            at = self.kernel().node(id)?.parent;
        }
        None
    }

    fn is_field(&self, id: ViewId) -> bool {
        self.kernel()
            .node(id)
            .is_some_and(|n| n.node_type == NodeType::TextInput)
    }

    fn value(&self, id: ViewId) -> String {
        self.kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value).map(str::to_string))
            .unwrap_or_default()
    }

    /// Interactive nodes, in document (paint) order, each once.
    fn focusables(&mut self) -> Vec<ViewId> {
        let mut seen = Vec::new();
        for (id, rect) in &self.frame().hits {
            if rect.w > 0 && rect.h > 0 && !seen.contains(id) {
                seen.push(*id);
            }
        }
        let kernel = self.kernel();
        seen.retain(|id| {
            kernel
                .node(*id)
                .is_some_and(|n| n.props.str(PropId::Disabled) != Some("true"))
        });
        seen
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
        self.painted = None;
    }

    fn step_focus(&mut self, back: bool) {
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
            let scrolls = n.node_type == NodeType::ScrollView
                || matches!(
                    n.style.overflow_y,
                    exact_kernel::Overflow::Scroll | exact_kernel::Overflow::Auto
                );
            if scrolls {
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

    /// Press a node as a click or Enter would: the field takes the focus, a
    /// button its handler.
    pub fn press(&mut self, id: ViewId) {
        if self.is_field(id) {
            self.focus(Some(id));
            return;
        }
        if let Some(target) = self.handler(id, EventKind::Press) {
            self.focus(Some(id));
            self.dispatch(target, Event::Press);
        }
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

    /// A mouse press at a cell.
    pub fn click(&mut self, x: i32, y: i32) {
        match self.hit(x, y) {
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
            *at = (*at + rows as f32 * ROW).clamp(0.0, reach);
            self.painted = None;
        }
    }

    /// Nodes whose `aria-keyshortcuts` name this chord.
    fn shortcut(&mut self, chord: &str) -> Option<ViewId> {
        let ids: Vec<ViewId> = self.frame().hits.iter().map(|(id, _)| *id).collect();
        let kernel = self.kernel();
        ids.into_iter().find(|id| {
            kernel.node(*id).is_some_and(|n| {
                n.props
                    .str(PropId::AccessibilityKeyShortcuts)
                    .is_some_and(|s| s.split_whitespace().any(|k| k.eq_ignore_ascii_case(chord)))
            })
        })
    }

    fn edit(&mut self, id: ViewId, change: impl FnOnce(&mut Vec<String>, &mut usize)) {
        let mut clusters: Vec<String> =
            self.value(id).graphemes(true).map(str::to_string).collect();
        let mut caret = self.caret.min(clusters.len());
        change(&mut clusters, &mut caret);
        let next = clusters.concat();
        self.caret = caret;
        // `input=` hears every keystroke; `change=` hears the commit (Enter, blur).
        self.dispatch(id, Event::Input(ControlValue::Text(next)));
    }

    /// A key, routed as LLP 1101 D6 says: the focused field first, then
    /// shortcuts, then the focused node, then navigation.
    pub fn key(&mut self, key: Key) -> After {
        let focus = self.focus;
        let field = focus.filter(|f| self.is_field(*f));
        if let Some(f) = field {
            match &key {
                Key::Char(c) => {
                    let c = *c;
                    self.edit(f, |v, at| {
                        v.insert(*at, c.to_string());
                        *at += 1;
                    });
                    return After::Continue;
                }
                Key::Named("Backspace") => {
                    self.edit(f, |v, at| {
                        if *at > 0 {
                            v.remove(*at - 1);
                            *at -= 1;
                        }
                    });
                    return After::Continue;
                }
                Key::Named("Delete") => {
                    self.edit(f, |v, at| {
                        if *at < v.len() {
                            v.remove(*at);
                        }
                    });
                    return After::Continue;
                }
                Key::Named("ArrowLeft") => {
                    self.caret = self.caret.saturating_sub(1);
                    self.painted = None;
                    return After::Continue;
                }
                Key::Named("ArrowRight") => {
                    self.caret = (self.caret + 1).min(self.value(f).graphemes(true).count());
                    self.painted = None;
                    return After::Continue;
                }
                Key::Named("Home") | Key::Ctrl('a') => {
                    self.caret = 0;
                    self.painted = None;
                    return After::Continue;
                }
                Key::Named("End") | Key::Ctrl('e') => {
                    self.caret = self.value(f).graphemes(true).count();
                    self.painted = None;
                    return After::Continue;
                }
                Key::Named("Enter") => {
                    if self.runner.handlers_of(f).contains(&EventKind::Submit) {
                        self.dispatch(f, Event::Submit);
                        self.caret = self.value(f).graphemes(true).count();
                    }
                    return After::Continue;
                }
                Key::Named("Escape") => {
                    self.focus(None);
                    return After::Continue;
                }
                _ => {}
            }
        }
        let chord = match &key {
            Key::Char(c) => c.to_string(),
            Key::Named(n) => n.to_string(),
            Key::Ctrl(c) => format!("Control+{c}"),
            Key::BackTab => "Shift+Tab".into(),
        };
        if let Some(id) = self.shortcut(&chord) {
            self.press(id);
            return After::Continue;
        }
        if let Some(f) = focus {
            if self.runner.handlers_of(f).contains(&EventKind::Key)
                && self.dispatch(f, Event::key(&chord))
            {
                return After::Continue;
            }
        }
        match key {
            Key::Ctrl('c') | Key::Ctrl('\\') | Key::Ctrl('d') => return After::Quit,
            Key::Named("Tab") | Key::Named("ArrowDown") => self.step_focus(false),
            Key::BackTab | Key::Named("ArrowUp") => self.step_focus(true),
            Key::Named("Enter") | Key::Char(' ') => {
                if let Some(f) = focus {
                    self.press(f);
                }
            }
            Key::Named("Escape") => self.focus(None),
            Key::Named("PageDown") | Key::Named("PageUp") => {
                let rows = self.rows as i32 - 2;
                let sign = if key == Key::Named("PageDown") { 1 } else { -1 };
                let target = self.frame().scrollers.first().map(|(_, r, _)| (r.x, r.y));
                if let Some((x, y)) = target {
                    self.wheel(x, y, sign * rows);
                }
            }
            _ => {}
        }
        After::Continue
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
