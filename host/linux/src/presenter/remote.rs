//! A TV remote's D-pad (LLP 1115.001.003 D3, D4). A TV is a device with no
//! pointer at all (`pointer: none`, D2). Its page is one view the reader
//! draws, so Android's own focus search cannot see the nodes: the arrows
//! move the focus by the rules of Android's `FocusFinder`
//! (`frameworks/base/core/java/android/view/FocusFinder.java`), and Select
//! presses the focus, as tvOS's Select does (LLP 1008 §9).
use super::*;
use std::collections::BTreeSet;

/// The way an arrow moves the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// The direction of an arrow key, by the web's `key`.
    pub(crate) fn of(key: &str) -> Option<Self> {
        match key {
            "ArrowUp" => Some(Self::Up),
            "ArrowDown" => Some(Self::Down),
            "ArrowLeft" => Some(Self::Left),
            "ArrowRight" => Some(Self::Right),
            _ => None,
        }
    }

    fn across(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// A box's left, top, right and bottom.
type Edges = (f32, f32, f32, f32);

fn edges(r: Rect4) -> Edges {
    (r.0, r.1, r.0 + r.2, r.1 + r.3)
}

/// `t` lies in `d` from `s` (`isCandidate`).
fn candidate(d: Direction, s: Edges, t: Edges) -> bool {
    match d {
        Direction::Left => (s.2 > t.2 || s.0 >= t.2) && s.0 > t.0,
        Direction::Right => (s.0 < t.0 || s.2 <= t.0) && s.2 < t.2,
        Direction::Up => (s.3 > t.3 || s.1 >= t.3) && s.1 > t.1,
        Direction::Down => (s.1 < t.1 || s.3 <= t.1) && s.3 < t.3,
    }
}

/// `t` overlaps `s`'s beam: its extent across `d` (`beamsOverlap`).
fn in_beam(d: Direction, s: Edges, t: Edges) -> bool {
    match d.across() {
        true => t.3 > s.1 && t.1 < s.3,
        false => t.2 > s.0 && t.0 < s.2,
    }
}

/// `t` lies wholly beyond `s`'s edge in `d` (`isToDirectionOf`).
fn beyond(d: Direction, s: Edges, t: Edges) -> bool {
    match d {
        Direction::Left => s.0 >= t.2,
        Direction::Right => s.2 <= t.0,
        Direction::Up => s.1 >= t.3,
        Direction::Down => s.3 <= t.1,
    }
}

/// The gap from `s`'s edge to `t`'s near edge along `d` (`majorAxisDistance`).
fn major(d: Direction, s: Edges, t: Edges) -> f32 {
    let gap = match d {
        Direction::Left => s.0 - t.2,
        Direction::Right => t.0 - s.2,
        Direction::Up => s.1 - t.3,
        Direction::Down => t.1 - s.3,
    };
    gap.max(0.0)
}

/// The distance from `s`'s edge to `t`'s far edge along `d`
/// (`majorAxisDistanceToFarEdge`).
fn major_far(d: Direction, s: Edges, t: Edges) -> f32 {
    let gap = match d {
        Direction::Left => s.0 - t.0,
        Direction::Right => t.2 - s.2,
        Direction::Up => s.1 - t.1,
        Direction::Down => t.3 - s.3,
    };
    gap.max(1.0)
}

/// The distance between the centres across `d` (`minorAxisDistance`).
fn minor(d: Direction, s: Edges, t: Edges) -> f32 {
    match d.across() {
        true => ((s.1 + s.3) / 2.0 - (t.1 + t.3) / 2.0).abs(),
        false => ((s.0 + s.2) / 2.0 - (t.0 + t.2) / 2.0).abs(),
    }
}

/// `a`, in `s`'s beam, beats `b`, outside it (`beamBeats`): always across,
/// and up or down unless `b` is nearer than `a`'s near edge.
fn beam_beats(d: Direction, s: Edges, a: Edges, b: Edges) -> bool {
    if in_beam(d, s, b) || !in_beam(d, s, a) {
        return false;
    }
    !beyond(d, s, b) || d.across() || major(d, s, a) < major_far(d, s, b)
}

/// `a` is a better next focus than `b` (`isBetterCandidate`).
fn better(d: Direction, s: Edges, a: Edges, b: Edges) -> bool {
    if !candidate(d, s, a) {
        return false;
    }
    if !candidate(d, s, b) || beam_beats(d, s, a, b) {
        return true;
    }
    if beam_beats(d, s, b, a) {
        return false;
    }
    let weighted = |t| {
        let (m, n) = (major(d, s, t), minor(d, s, t));
        13.0 * m * m + n * n
    };
    weighted(a) < weighted(b)
}

/// Of `candidates`, the one the focus moves to from `from` in `d`.
pub(crate) fn nearest(
    d: Direction,
    from: Rect4,
    candidates: impl IntoIterator<Item = (ViewId, Rect4)>,
) -> Option<ViewId> {
    let s = edges(from);
    let mut best: Option<(ViewId, Edges)> = None;
    for (id, r) in candidates {
        let t = edges(r);
        if candidate(d, s, t) && best.is_none_or(|(_, b)| better(d, s, t, b)) {
            best = Some((id, t));
        }
    }
    best.map(|(id, _)| id)
}

impl<D: DataSource> Presenter<D> {
    /// The device has no pointer at all (D2): a TV, whose remote's arrows
    /// move the focus.
    pub(crate) fn remote(&self) -> bool {
        self.host.runner().viewport().preferences.pointer == exact_runner::Pointer::None
    }

    /// A remote's key at the focused node `id`, after its `key` handlers
    /// left the default (D3): Select presses, once for a held press, as a
    /// held Enter presses a button once; an arrow moves the focus, its
    /// repeats too. A text field keeps both, as its caret and Enter.
    /// Whether it was the remote's.
    pub(super) fn remote_key(
        &mut self,
        id: ViewId,
        (name, code, repeat): (&str, &str, bool),
        now_ms: f64,
    ) -> bool {
        if !self.remote()
            || self
                .host
                .kernel()
                .node(id)
                .is_none_or(|n| n.node_type == NodeType::TextInput)
        {
            return false;
        }
        if code == "Select" {
            if !repeat {
                self.remote_select(id, now_ms);
            }
            return true;
        }
        let Some(d) = Direction::of(name) else {
            return false;
        };
        self.move_focus_toward(d, now_ms);
        true
    }

    /// Select at `id`: a bar's target is chosen, a radio is checked, a
    /// checkbox or switch toggled, anything else pressed.
    fn remote_select(&mut self, id: ViewId, now_ms: f64) {
        if let Some(Err(e)) = self.bar_tap(id) {
            self.host.log(e);
            return;
        }
        if self.bar_node(id) {
            return;
        }
        if self.is_radio(id) {
            self.check_radio(id, now_ms);
            return;
        }
        let toggles = self.host.kernel().node(id).is_some_and(|n| {
            matches!(
                exact_kernel::ControlKind::of(n.node_type, n.props),
                Some(exact_kernel::ControlKind::Checkbox | exact_kernel::ControlKind::Switch)
            )
        });
        if toggles {
            self.toggle_control(id, now_ms);
            return;
        }
        self.note_press(id, true);
        self.activate_with(id, now_ms, false);
    }

    /// What a TV's Back with no route to go back from presses (D3, LLP 1008
    /// §9's Menu): the first shown, enabled button that answers Escape
    /// (`aria-keyshortcuts="Escape"`), in the frontmost `aria-modal` view
    /// where one shows, as a page shortcut is found; a bar's target is
    /// shown. A TV's remote has no Escape key, so its Back is one.
    pub(super) fn remote_back_target(&self) -> Option<ViewId> {
        if !self.remote() {
            return None;
        }
        let kernel = self.host.kernel();
        let preorder = self.host.preorder();
        let shown = |id: ViewId| {
            self.host.route_visibility(id) == (false, false) && self.display.allows(kernel, id)
        };
        let modal = preorder.iter().rev().copied().find(|&id| {
            kernel
                .node(id)
                .is_some_and(|n| n.props.bool(PropId::AccessibilityModal) == Some(true))
                && shown(id)
        });
        preorder.into_iter().find(|&id| {
            kernel.node(id).is_some_and(|n| {
                n.props
                    .str(PropId::AccessibilityKeyShortcuts)
                    .is_some_and(|chords| chords.split_whitespace().any(|c| c == "Escape"))
                    && n.props.bool(PropId::Disabled) != Some(true)
            }) && self.alert_allows(id)
                && modal.is_none_or(|m| id == m || self.is_inside(id, m))
                && (shown(id) || self.bar_target(id).is_some())
        })
    }

    /// A TV's Back with no route to go back from: the press of
    /// [`Self::remote_back_target`], as Select presses it. Whether there
    /// was one.
    pub(super) fn remote_back(&mut self, now_ms: f64) -> bool {
        let Some(id) = self.remote_back_target() else {
            return false;
        };
        self.remote_select(id, now_ms);
        true
    }

    /// A remote's arrow at a focus that a commit disabled: the page's keys
    /// refuse it, and the arrow moves the focus off it all the same (D4).
    /// Whether it did.
    pub(super) fn remote_leaves_disabled(&mut self, id: ViewId, name: &str, now_ms: f64) -> bool {
        let Some(d) = Direction::of(name).filter(|_| self.remote()) else {
            return false;
        };
        let disabled = (self.host.kernel().node(id))
            .is_none_or(|n| crate::surfaces::controls::disabled_control(&n));
        if disabled {
            self.move_focus_toward(d, now_ms);
        }
        disabled
    }

    /// The page's remote stops in Tab order, with their boxes: its Tab
    /// stops, every enabled radio of a group among them, so the remote
    /// reaches each option (Select checks one). A stop that holds another
    /// is none: a column with a `key` handler is focusable, and the remote
    /// reaches the controls in it instead, since none of them lies beyond
    /// its edges in any direction.
    fn page_stops(&mut self) -> Vec<(ViewId, Rect4)> {
        let boxed = self.shown_boxes();
        let mut rects = BTreeMap::new();
        for b in &self.boxes {
            rects.entry(b.id).or_insert(b.rect);
        }
        let mut order: Vec<(i64, usize, ViewId)> = Vec::new();
        for (at, id) in self.host.preorder().into_iter().enumerate() {
            if self.focus_stop(id, &boxed) {
                let index = (self.host.kernel().node(id))
                    .and_then(|n| n.props.get(PropId::TabIndex).and_then(|v| v.as_int()))
                    .filter(|&i| i > 0)
                    .unwrap_or(i64::MAX);
                order.push((index, at, id));
            }
        }
        let stops: BTreeSet<ViewId> = order.iter().map(|o| o.2).collect();
        let kernel = self.host.kernel();
        let mut holders = BTreeSet::new();
        for &id in &stops {
            let mut at = kernel.node(id).and_then(|n| n.parent);
            while let Some(p) = at {
                if stops.contains(&p) {
                    holders.insert(p);
                }
                at = kernel.node(p).and_then(|n| n.parent);
            }
        }
        order.sort_unstable();
        (order.into_iter())
            .filter(|o| !holders.contains(&o.2))
            .filter_map(|o| rects.get(&o.2).map(|r| (o.2, *r)))
            .collect()
    }

    /// The focus moves to the nearest stop in `d` (D4), and that stop is
    /// scrolled into view. The stops are the page's ([`Self::page_stops`])
    /// and the targets of the shell's bars. From no focus, or from one that
    /// holds stops (a column the app focused), it moves to the first stop in
    /// Tab order there, else to the bars' first. With none in `d`, the focus
    /// stays.
    pub(crate) fn move_focus_toward(&mut self, d: Direction, now_ms: f64) {
        let bars = self.bar_stops();
        let page = self.page_stops();
        let focus = self.focus;
        let from = focus.and_then(|f| {
            (self.box_of(f).map(|b| b.rect))
                .or_else(|| bars.iter().find(|(id, _)| *id == f).map(|(_, r)| *r))
        });
        let next = match (focus, from) {
            (Some(f), Some(from)) => match page.iter().find(|(id, _)| self.is_inside(*id, f)) {
                Some((inner, _)) => Some(*inner),
                None => nearest(
                    d,
                    from,
                    (page.iter().chain(&bars).copied()).filter(|(id, _)| *id != f),
                ),
            },
            _ => page.first().or(bars.first()).map(|(id, _)| *id),
        };
        let Some(next) = next else {
            return;
        };
        if let Some(e) = self.set_focus(Some(next), now_ms) {
            eprintln!("exact: {e}");
        }
        // A bar's target is in view in its bar.
        if !bars.iter().any(|(id, _)| *id == next) {
            self.keep_in_view(next, reveal::Cause::FocusMove);
        }
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
