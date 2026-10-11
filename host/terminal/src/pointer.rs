//! Pointer events against what the terminal last showed (LLP 1101.002 §0
//! P3): one record of the presented painting — its boxes, its scrollers,
//! the open layer's bounds, and where its first row sits on the screen —
//! answers every click and wheel, in both modes. Inline, the origin is the
//! live region's screen row, learned from the terminal's answer to a
//! cursor-position query; until it is known, a pointer event goes nowhere.

use crate::grid::CellRect;
use crate::host::Host;
use crate::paint::Painted;
use exact_kernel::style::cells::ROW;
use exact_kernel::{PropId, ViewId};
use exact_runner::DataSource;

/// What a frame presented.
#[derive(Debug, Clone, Default)]
pub struct Presented {
    /// Interactive nodes and their cells, in paint order.
    pub hits: Vec<(ViewId, CellRect)>,
    /// Scrollers, their visible cells and reach.
    pub scrollers: Vec<(ViewId, CellRect, f32)>,
    /// The open top layer's cells, when one is open.
    pub layer: Option<CellRect>,
    /// Inline runs a click reaches, and their cells.
    pub runs: Vec<(ViewId, CellRect)>,
    /// The screen row of the painting's first row.
    pub origin: Option<i32>,
    /// A layer opened or closed since this was presented: what it shows is
    /// not what a press would reach, so none lands until the next frame.
    pub stale: bool,
}

impl<D: DataSource> Host<D> {
    /// Record what was just presented, and where (full screen: row 0).
    pub fn present(&mut self, painted: &Painted, origin: Option<i32>) {
        self.presented = Presented {
            hits: painted.hits.clone(),
            scrollers: painted.scrollers.clone(),
            layer: painted.layer,
            runs: painted.runs.clone(),
            origin: origin.or(self.presented.origin),
            stale: false,
        };
    }

    /// The presented painting's first row is at this screen row.
    pub fn anchor(&mut self, origin: i32) {
        self.presented.origin = Some(origin);
    }

    /// A screen row in the presented painting's rows.
    fn local(&self, y: i32) -> Option<i32> {
        if self.presented.stale {
            return None;
        }
        self.presented.origin.map(|o| y - o)
    }

    /// A press on an inline run at a cell of the presented painting: its
    /// `press` handler, or else its link (`Host::follow`). Whether a run
    /// was there.
    fn press_run_at(&mut self, x: i32, y: i32) -> bool {
        let found = self
            .presented
            .runs
            .iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(id, _)| *id);
        found.is_some_and(|id| self.press_run(id))
    }

    /// A press on an inline run: its `press` handler, or else the link it
    /// or its paragraph names. Whether anything took it.
    pub fn press_run(&mut self, id: ViewId) -> bool {
        if self.handler(id, exact_plan::EventKind::Press).is_some() {
            self.press_with(id, false);
            return true;
        }
        let kernel = self.kernel();
        let mut at = Some(id);
        while let Some(n) = at.and_then(|a| kernel.node(a)) {
            if let Some(href) = n.props.str(PropId::Href).filter(|h| !h.is_empty()) {
                let href = href.to_string();
                self.follow(&href);
                return true;
            }
            if n.node_type != exact_kernel::NodeType::Text {
                break;
            }
            at = n.parent;
        }
        false
    }

    /// Whether `id` is an inline run a click can reach on the screen.
    pub fn is_presented_run(&self, id: ViewId) -> bool {
        self.presented.runs.iter().any(|(r, c)| *r == id && c.w > 0)
    }

    /// The topmost interactive node at a screen cell.
    pub fn hit(&self, x: i32, y: i32) -> Option<ViewId> {
        let y = self.local(y)?;
        self.presented
            .hits
            .iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(id, _)| *id)
    }

    /// A mouse press at a screen cell. With a layer open, a press anywhere
    /// in its bounds (its title, its padding) is inside it; one outside
    /// closes it only when `closedby="any"`, and presses nothing beneath.
    pub fn click(&mut self, x: i32, y: i32) {
        let Some(local) = self.local(y) else { return };
        let hit = self.hit(x, y);
        if let Some((top, _)) = self.layers.last().copied() {
            let inside = self.presented.layer.is_some_and(|r| r.contains(x, local));
            if !inside {
                let any = self
                    .kernel()
                    .node(top)
                    .and_then(|n| n.props.str(PropId::Closedby).map(str::to_string));
                if any.as_deref() == Some("any") {
                    self.dismiss(top);
                }
                return;
            }
            match hit {
                Some(id) => self.press(id),
                None => {
                    self.press_run_at(x, local);
                }
            }
            return;
        }
        match hit {
            Some(id) => self.press(id),
            None => {
                if !self.press_run_at(x, local) {
                    self.focus(None);
                }
            }
        }
    }

    /// A wheel at a screen cell: the innermost presented scroller under it
    /// moves by rows (with a layer open, only the layer's are presented).
    pub fn wheel(&mut self, x: i32, y: i32, rows: i32) {
        let Some(y) = self.local(y) else { return };
        let found = self
            .presented
            .scrollers
            .iter()
            .rev()
            .find(|(_, r, _)| r.contains(x, y))
            .map(|(id, _, reach)| (*id, *reach));
        if let Some((id, reach)) = found {
            self.scroll_by(id, reach, rows);
        }
    }

    /// Page Up or Page Down: the first presented scroller (the outermost;
    /// with a layer open, the layer's) by its own height less a row kept in
    /// view.
    pub fn page(&mut self, down: bool) {
        if let Some((id, r, reach)) = self.presented.scrollers.first().copied() {
            let rows = (r.h - 1).max(1);
            self.scroll_by(id, reach, if down { rows } else { -rows });
        }
    }

    /// The wheel over `id`: the innermost presented scroller that is `id` or
    /// holds it moves by rows. False when none was presented.
    pub fn wheel_on(&mut self, id: ViewId, rows: i32) -> bool {
        let mut at = Some(id);
        while let Some(n) = at {
            let found = self.presented.scrollers.iter().find(|(s, _, _)| *s == n);
            if let Some((s, _, reach)) = found.copied() {
                self.scroll_by(s, reach, rows);
                return true;
            }
            at = self.kernel().node(n).and_then(|n| n.parent);
        }
        false
    }

    /// The first presented scroller (with a layer open, the layer's) by
    /// rows: the arrows, with nothing focused.
    pub fn scroll_rows(&mut self, rows: i32) {
        if let Some((id, _, reach)) = self.presented.scrollers.first().copied() {
            self.scroll_by(id, reach, rows);
        }
    }

    /// The first presented scroller to its top or its end: Home and End.
    pub fn scroll_edge(&mut self, end: bool) {
        if let Some((id, _, reach)) = self.presented.scrollers.first().copied() {
            self.scroll.insert(id, if end { reach } else { 0.0 });
            self.changed();
        }
    }

    /// A scroller by `y` pixels, rounded to whole rows and kept within its
    /// reach; a node that does not scroll does not move (the web's
    /// `Element.scrollBy` on an element without overflow).
    pub(crate) fn scroll_node_by(&mut self, id: ViewId, y: f32) {
        let kernel = self.kernel();
        let Some(n) = kernel.node(id) else { return };
        let scrolls = n.node_type == exact_kernel::NodeType::ScrollView
            || matches!(
                n.style.overflow_y,
                exact_kernel::Overflow::Scroll | exact_kernel::Overflow::Auto
            );
        if !scrolls {
            return;
        }
        let [bt, _, bb, _] = n.style.border_widths_in(&kernel.env());
        let reach = (n.content.1 - (n.frame.height - bt - bb)).max(0.0);
        self.scroll_by(id, reach, (y / ROW).round() as i32);
    }

    fn scroll_by(&mut self, id: ViewId, reach: f32, rows: i32) {
        let at = self.scroll.entry(id).or_insert(0.0);
        *at = (at.min(reach) + rows as f32 * ROW).clamp(0.0, reach);
        self.changed();
    }

    /// Whether `id`, or the interactive node it sits in, has cells in what
    /// was presented: the agent's `tap` presses only what a person could.
    pub fn on_screen(&self, id: ViewId) -> bool {
        if self.is_presented_run(id) {
            return true;
        }
        let kernel = self.kernel();
        let mut at = Some(id);
        while let Some(n) = at {
            if self
                .presented
                .hits
                .iter()
                .any(|(h, r)| *h == n && r.w > 0 && r.h > 0)
            {
                return true;
            }
            // A scroller is on the screen as itself, not by what it holds:
            // an item clipped out of it is not.
            if n == id
                && self
                    .presented
                    .scrollers
                    .iter()
                    .any(|(s, r, _)| *s == n && r.w > 0 && r.h > 0)
            {
                return true;
            }
            at = kernel.node(n).and_then(|n| n.parent);
        }
        false
    }
}
