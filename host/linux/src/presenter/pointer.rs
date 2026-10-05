//! `pointerdown`, `pointerup` and `pointermove` (LLP 1005 §3; LLP 1056 §3
//! stage 3): DOM's events beside the contact, which they never take from.
//! The innermost enabled node under the press hearing any of them holds the
//! pointer until it lifts (a cancel is an up), and hears its moves; a free
//! pointer's moves go to the innermost such node under it. Each carries the
//! `PointerEvent` record from the node's content box. Linux's pointer is a
//! mouse (evdev, VNC, the agent): id 1, DOM's 0.5 of pressure while down.
//! A wheel over a node is DOM's `wheel` first (studio diary R3).
use super::*;
use exact_runner::PointerEvent;

const KINDS: [EventKind; 3] = [
    EventKind::Pointerdown,
    EventKind::Pointerup,
    EventKind::Pointermove,
];

impl<D: DataSource> Presenter<D> {
    /// The innermost enabled node from `hit` up that hears one of `kinds`.
    /// A `wheel` is heard by a disabled box too, as by a `<div disabled>`
    /// in Chrome; only a disabled form control is out (review b5-b 4).
    fn pointer_node(&self, hit: ViewId, kinds: &[EventKind]) -> Option<ViewId> {
        if self.host.route_visibility(hit).1 {
            return None;
        }
        let kernel = self.host.kernel();
        let mut at = Some(hit);
        while let Some(n) = at {
            let node = kernel.node(n)?;
            let hears = self.host.runner().handlers_of(n);
            let out = if kinds == [EventKind::Wheel] {
                crate::surfaces::controls::disabled_control(&node)
            } else {
                node.props.bool(PropId::Disabled) == Some(true)
            };
            if !out && kinds.iter().any(|k| hears.contains(k)) {
                return Some(n);
            }
            at = self.display.parent(kernel, n);
        }
        None
    }

    /// The record of a viewport point as `view` sees it, from its content box.
    fn pointer_record(
        &mut self,
        view: ViewId,
        x: f32,
        y: f32,
        buttons: u8,
    ) -> Option<PointerEvent> {
        let (ox, oy, _, _) = self.rect_of(view)?;
        let (left, top, _, _) =
            exact_kernel::svg::scene::content_box(&self.host.kernel().node(view)?);
        let mut record = PointerEvent::parse(&format!(
            "{},{},{},{},mouse,1",
            x - ox - left,
            y - oy - top,
            buttons,
            if buttons != 0 { 0.5 } else { 0.0 }
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
        self.pointer_pressed_with(x, y, 1, now_ms)
    }

    /// A button went down with no other held: the innermost node hearing
    /// the pointer holds it, any button, as in a browser (review b5-b 1).
    pub(crate) fn pointer_pressed_with(
        &mut self,
        x: f32,
        y: f32,
        buttons: u8,
        now_ms: f64,
    ) -> Option<String> {
        let view = self
            .hit(x, y)
            .and_then(|hit| self.pointer_node(hit, &KINDS))?;
        self.pointer_held = self.host.kernel().node(view).map(|n| n.key);
        self.pointer_buttons = buttons;
        let record = self.pointer_record(view, x, y, buttons)?;
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
        let (view, buttons) = match self.pointer_held {
            Some(key) => (
                self.host.kernel().node_by_key(key)?.id,
                self.pointer_buttons,
            ),
            None if self.contact.is_none() => {
                let hit = self.hit(x, y)?;
                (self.pointer_node(hit, &[EventKind::Pointermove])?, 0)
            }
            None => return None,
        };
        let record = self.pointer_record(view, x, y, buttons)?;
        self.pointer_dispatch(
            view,
            EventKind::Pointermove,
            Event::Pointermove(record),
            now_ms,
        )
    }

    /// A secondary or middle button over no canvas: the first button down
    /// with none held is a `pointerdown`, the last up a `pointerup`, and a
    /// chord changes the held buttons the moves report.
    pub(crate) fn aux_pointer(&mut self, before: u32, after: u32, x: f32, y: f32, now_ms: f64) {
        let primary = self.pointer_buttons & 1;
        let buttons = primary | after as u8;
        let error = if self.pointer_held.is_none() && before == 0 && after != 0 {
            self.pointer_pressed_with(x, y, after as u8, now_ms)
        } else if self.pointer_held.is_some() && buttons == 0 {
            self.pointer_lifted(Some((x, y)), now_ms)
        } else {
            if self.pointer_held.is_some() {
                self.pointer_buttons = buttons;
            }
            None
        };
        if let Some(e) = error {
            eprintln!("exact: {e}");
        }
    }

    /// The held pointer lifted, or its contact was cancelled: its `pointerup`.
    pub(super) fn pointer_lifted(&mut self, at: Option<(f32, f32)>, now_ms: f64) -> Option<String> {
        let key = self.pointer_held.take()?;
        self.pointer_buttons = 0;
        let view = self.host.kernel().node_by_key(key)?.id;
        let (x, y) = at.or(self.contact_position()).or(self.pointer)?;
        let record = self.pointer_record(view, x, y, 0)?;
        self.pointer_dispatch(view, EventKind::Pointerup, Event::Pointerup(record), now_ms)
    }

    /// DOM's `wheel` at a viewport point (studio diary R3): every enabled
    /// node from the hit up that hears it, innermost first, as it bubbles,
    /// each with its `WheelEvent` (pixels, the web's sign). True when one
    /// called `preventDefault()`: the caller does not scroll.
    pub(crate) fn wheel_event(&mut self, x: f32, y: f32, dx: f32, dy: f32) -> bool {
        let mut path = Vec::new();
        let mut at = self.hit(x, y);
        while let Some(hit) = at.and_then(|hit| self.pointer_node(hit, &[EventKind::Wheel])) {
            path.push(hit);
            at = self.display.parent(self.host.kernel(), hit);
        }
        let mut prevented = false;
        for view in path {
            let Some(point) = self.pointer_record(view, x, y, 0) else {
                continue;
            };
            let wheel = exact_runner::WheelEvent {
                offset_x: point.offset_x,
                offset_y: point.offset_y,
                delta_x: f64::from(dx),
                delta_y: f64::from(dy),
                delta_mode: 0.0,
                held: point.held,
            };
            let now = self.pointer_now();
            if let Some(error) =
                self.pointer_dispatch(view, EventKind::Wheel, Event::Wheel(wheel), now)
            {
                self.host.log(error);
            }
            let queued = self.commands.len();
            self.commands.retain(|c| c.name != "preventDefault");
            prevented |= self.commands.len() != queued;
        }
        prevented
    }
}
