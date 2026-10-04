//! `pointerdown`, `pointerup` and `pointermove` (LLP 1005 §3; LLP 1056 §3
//! stage 3): DOM's events beside the contact, which they never take from.
//! The innermost enabled node under the press hearing any of them holds the
//! pointer until it lifts (a cancel is an up), and hears its moves; a free
//! pointer's moves go to the innermost such node under it. Each carries the
//! `PointerEvent` record from the node's content box. Linux's pointer is a
//! mouse (evdev, VNC, the agent): id 1, DOM's 0.5 of pressure while down.
use super::*;
use exact_runner::PointerEvent;

const KINDS: [EventKind; 3] = [
    EventKind::Pointerdown,
    EventKind::Pointerup,
    EventKind::Pointermove,
];

impl<D: DataSource> Presenter<D> {
    /// The innermost enabled node from `hit` up that hears one of `kinds`.
    fn pointer_node(&self, hit: ViewId, kinds: &[EventKind]) -> Option<ViewId> {
        if self.host.route_visibility(hit).1 {
            return None;
        }
        let kernel = self.host.kernel();
        let mut at = Some(hit);
        while let Some(n) = at {
            let node = kernel.node(n)?;
            let hears = self.host.runner().handlers_of(n);
            if node.props.bool(PropId::Disabled) != Some(true)
                && kinds.iter().any(|k| hears.contains(k))
            {
                return Some(n);
            }
            at = self.display.parent(kernel, n);
        }
        None
    }

    /// The record of a viewport point as `view` sees it, from its content box.
    fn pointer_record(&mut self, view: ViewId, x: f32, y: f32, down: bool) -> Option<PointerEvent> {
        let (ox, oy, _, _) = self.rect_of(view)?;
        let (left, top, _, _) =
            exact_kernel::svg::scene::content_box(&self.host.kernel().node(view)?);
        let mut record = PointerEvent::parse(&format!(
            "{},{},{},{},mouse,1",
            x - ox - left,
            y - oy - top,
            u8::from(down),
            if down { 0.5 } else { 0.0 }
        ))?;
        // The modifiers held (gallery F20), as a `MouseEvent` has them.
        record.held = self.modifiers();
        Some(record)
    }

    fn pointer_dispatch(
        &mut self,
        view: ViewId,
        kind: EventKind,
        event: Event,
        now_ms: f64,
    ) -> Option<String> {
        if !self.host.runner().handlers_of(view).contains(&kind) {
            return None;
        }
        self.host
            .dispatch_at(view, event, now_ms)
            .or(self.after_commit())
    }

    /// The button went down at a viewport point.
    pub(super) fn pointer_pressed(&mut self, x: f32, y: f32, now_ms: f64) -> Option<String> {
        let view = self
            .hit(x, y)
            .and_then(|hit| self.pointer_node(hit, &KINDS))?;
        self.pointer_held = self.host.kernel().node(view).map(|n| n.key);
        let record = self.pointer_record(view, x, y, true)?;
        self.pointer_dispatch(
            view,
            EventKind::Pointerdown,
            Event::Pointerdown(record),
            now_ms,
        )
    }

    /// The pointer moved: the held node's move, or a free pointer's over
    /// the innermost node hearing one.
    pub(super) fn pointer_moved_over(&mut self, x: f32, y: f32, now_ms: f64) -> Option<String> {
        let (view, down) = match self.pointer_held {
            Some(key) => (self.host.kernel().node_by_key(key)?.id, true),
            None if self.contact.is_none() => {
                let hit = self.hit(x, y)?;
                (self.pointer_node(hit, &[EventKind::Pointermove])?, false)
            }
            None => return None,
        };
        let record = self.pointer_record(view, x, y, down)?;
        self.pointer_dispatch(
            view,
            EventKind::Pointermove,
            Event::Pointermove(record),
            now_ms,
        )
    }

    /// The held pointer lifted, or its contact was cancelled: its `pointerup`.
    pub(super) fn pointer_lifted(&mut self, at: Option<(f32, f32)>, now_ms: f64) -> Option<String> {
        let key = self.pointer_held.take()?;
        let view = self.host.kernel().node_by_key(key)?.id;
        let (x, y) = at.or(self.contact_position()).or(self.pointer)?;
        let record = self.pointer_record(view, x, y, false)?;
        self.pointer_dispatch(view, EventKind::Pointerup, Event::Pointerup(record), now_ms)
    }
}
