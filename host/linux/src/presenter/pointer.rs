//! `pointerdown`, `pointerup` and `pointermove` (LLP 1005 §3; LLP 1056 §3
//! stage 3): DOM's events beside the contact, which they never take from.
//! The down and the up bubble (Charlie, 2026-10-15): every enabled node
//! from the press up that hears either holds the pointer until it lifts (a
//! cancel is an up), innermost first, and the innermost hearing any of the
//! three also hears its moves; a free pointer's moves go to the innermost
//! node under it that hears them. Each carries the
//! `PointerEvent` record from the node's content box. The pointer is a mouse
//! (evdev, VNC, the agent: id 1) unless the device's shell says what touched
//! the screen ([`Presenter::set_pointer_kind`]: Android's reader names a
//! finger or a pen with each touch, id 2 as on iOS); DOM's 0.5 of pressure
//! while down either way. A wheel over a node is DOM's `wheel` first (studio
//! diary R3).
use super::*;
use exact_runner::PointerEvent;

/// What the pointer is: DOM's `pointerType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerKind {
    /// A mouse: it hovers, and a focus it gives shows its ring.
    #[default]
    Mouse,
    /// A finger on the screen.
    Touch,
    /// A stylus on the screen.
    Pen,
}

impl PointerKind {
    /// DOM's keyword and the contact's id.
    fn wire(self) -> (&'static str, u8) {
        match self {
            PointerKind::Mouse => ("mouse", 1),
            PointerKind::Touch => ("touch", 2),
            PointerKind::Pen => ("pen", 2),
        }
    }
}

const KINDS: [EventKind; 3] = [
    EventKind::Pointerdown,
    EventKind::Pointerup,
    EventKind::Pointermove,
];

impl<D: DataSource> Presenter<D> {
    /// What the pointer is from now on (LLP 1115.001 D7.4): told by a shell
    /// that knows, with each contact. A finger or a pen touches the screen
    /// directly: it is reported as itself, hovers nothing ([`Presenter::direct`]),
    /// and leaves no focus ring on what it presses.
    pub fn set_pointer_kind(&mut self, kind: PointerKind) {
        self.pointer_kind = kind;
    }

    /// Whether the pointer touches the screen itself (a finger, a pen): it
    /// is nowhere between contacts, so nothing is hovered.
    pub(crate) fn direct(&self) -> bool {
        self.pointer_kind != PointerKind::Mouse
    }

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

    /// The record of a viewport point as `view` sees it, from its content
    /// box, and the viewport point itself (`clientX`, LLP 1094 D11).
    fn pointer_record(
        &mut self,
        view: ViewId,
        x: f32,
        y: f32,
        buttons: u8,
    ) -> Option<PointerEvent> {
        let (ox, oy, _, _) = self.rect_of(view)?;
        let kernel = self.host.kernel();
        let (left, top, _, _) = exact_kernel::svg::scene::content_box(kernel, &kernel.node(view)?);
        let (kind, id) = self.pointer_kind.wire();
        let mut record = PointerEvent::parse(&format!(
            "{},{},{},{},{kind},{id},{x},{y}",
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

    /// A button went down with no other held, any button, as in a browser
    /// (review b5-b 1): the innermost node hearing the pointer holds it,
    /// and its `pointerdown` bubbles to every node above that hears it.
    pub(crate) fn pointer_pressed_with(
        &mut self,
        x: f32,
        y: f32,
        buttons: u8,
        now_ms: f64,
    ) -> Option<String> {
        // A press on a held node lets its hold go (LLP 1122 D8).
        let hit = self.hit(x, y);
        self.hold_input(hit);
        let held = hit.and_then(|hit| self.pointer_node(hit, &KINDS))?;
        let mut path = Vec::new();
        let mut at = Some(held);
        while let Some(view) =
            at.and_then(|n| self.pointer_node(n, &[EventKind::Pointerdown, EventKind::Pointerup]))
        {
            path.push(view);
            at = self.display.parent(self.host.kernel(), view);
        }
        let kernel = self.host.kernel();
        self.pointer_held = kernel.node(held).map(|n| n.key);
        self.pointer_path = path
            .iter()
            .filter_map(|&v| kernel.node(v).map(|n| n.key))
            .collect();
        self.pointer_buttons = buttons;
        let mut error = None;
        let outer = std::mem::replace(&mut self.pointer_walking, true);
        self.stopped();
        for key in self.pointer_path.clone() {
            // A handler may remove or disable a node further up before its
            // turn, or stop this down; the up's path and the hold stay.
            let Some(node) = self.host.kernel().node_by_key(key) else {
                continue;
            };
            if node.props.bool(PropId::Disabled) == Some(true) {
                continue;
            }
            let view = node.id;
            let Some(record) = self.pointer_record(view, x, y, buttons) else {
                continue;
            };
            let failed = self.pointer_dispatch(
                view,
                EventKind::Pointerdown,
                Event::Pointerdown(record),
                now_ms,
            );
            error = error.or(failed);
            if self.stopped() {
                break;
            }
        }
        self.walked(outer);
        error
    }

    /// A walk ended: a contact one of its handlers retired is cancelled now.
    fn walked(&mut self, outer: bool) {
        self.pointer_walking = outer;
        self.retire_pointer();
    }

    /// Whether the dispatch just made called `stopPropagation()`, which ends
    /// this event's bubbling there, as the DOM's does; the command is the
    /// event's, never one for the host to run.
    fn stopped(&mut self) -> bool {
        let queued = self.commands.len();
        self.commands.retain(|c| c.name != "stopPropagation");
        self.commands.len() != queued
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

    /// The held pointer lifted, or its contact was cancelled: `pointerup`
    /// for every node that held it still in the tree, innermost first.
    pub(super) fn pointer_lifted(&mut self, at: Option<(f32, f32)>, now_ms: f64) -> Option<String> {
        self.pointer_held.take()?;
        self.pointer_buttons = 0;
        let path = std::mem::take(&mut self.pointer_path);
        let (x, y) = at.or(self.contact_position()).or(self.pointer)?;
        let mut error = None;
        // The caller retires the contact after the up's walk.
        let outer = std::mem::replace(&mut self.pointer_walking, true);
        self.stopped();
        for key in path {
            let Some(view) = self.host.kernel().node_by_key(key).map(|n| n.id) else {
                continue;
            };
            let Some(record) = self.pointer_record(view, x, y, 0) else {
                continue;
            };
            let failed =
                self.pointer_dispatch(view, EventKind::Pointerup, Event::Pointerup(record), now_ms);
            error = error.or(failed);
            if self.stopped() {
                break;
            }
        }
        self.pointer_walking = outer;
        error
    }

    /// DOM's `wheel` at a viewport point (studio diary R3): every enabled
    /// node from the hit up that hears it, innermost first, as it bubbles,
    /// each with its `WheelEvent` (pixels, the web's sign), until one calls
    /// `stopPropagation()`. True when one called `preventDefault()`: the
    /// caller does not scroll.
    pub(crate) fn wheel_event(&mut self, x: f32, y: f32, dx: f32, dy: f32) -> bool {
        let mut path = Vec::new();
        let mut at = self.hit(x, y);
        while let Some(hit) = at.and_then(|hit| self.pointer_node(hit, &[EventKind::Wheel])) {
            path.push(hit);
            at = self.display.parent(self.host.kernel(), hit);
        }
        let mut prevented = false;
        let outer = std::mem::replace(&mut self.pointer_walking, true);
        self.stopped();
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
            if self.stopped() {
                break;
            }
        }
        self.walked(outer);
        prevented
    }
}

#[cfg(test)]
#[path = "touch_tests.rs"]
mod touch_tests;
