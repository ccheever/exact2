//! `pointerdown`, `pointerup` and `pointermove`'s record (LLP 1005 §3; LLP
//! 1056 §3 stage 3, as built): a subset of DOM's `PointerEvent`, by its own
//! names, which an action takes as an optional last parameter (the paint
//! diary's F1: a drawing app had no coordinates, movement or pressure).
//! A `contextmenu` offers the same record (UI Events makes it a
//! `PointerEvent`): where the secondary click was (studio diary R22).
//! `wheel`'s `WheelEvent` and `drop`'s `DragEvent` are the same family,
//! DOM's `MouseEvent`s (studio diary R3, R19).
//!
//! The wire is one UTF-8 line every host writes the same way:
//! `offsetX,offsetY,buttons,pressure,pointerType,pointerId`; a wheel's is
//! `offsetX,offsetY,deltaX,deltaY,deltaMode`; a drop's is `offsetX,offsetY`
//! and then one `doc:` handle per line. Each may end in the modifiers held.

use super::Event;
use exact_plan::Value;

/// One pointer sample, as the node that hears it sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct PointerEvent {
    /// From the left of the node's content box, CSS px (DOM measures from
    /// the target's padding edge; a canvas draws in its content box).
    pub offset_x: f64,
    /// From the top of the node's content box, CSS px.
    pub offset_y: f64,
    /// DOM's bit set: 1 primary (or a touch, or a pen in contact), 2
    /// secondary, 4 auxiliary. 0 on `pointerup` and on a hover move.
    pub buttons: f64,
    /// 0 to 1: a pen's or a pressed touch's force where the platform
    /// measures one, else DOM's defaults, 0.5 while pressed and 0 while not.
    pub pressure: f64,
    /// `mouse`, `pen` or `touch`.
    pub pointer_type: String,
    /// The contact's id: 1 for the mouse, as browsers number it; a touch
    /// or pen gets its own for as long as it is down.
    pub pointer_id: f64,
    /// The modifier keys held, `MouseEvent`'s `shiftKey`… (gallery F20).
    pub held: super::KeyModifiers,
}

impl PointerEvent {
    /// Decode the wire line — six fields, then optionally the modifiers
    /// held as a chord prefix (`Shift+Meta`, [`KeyModifiers::held`]);
    /// `None` for a malformed one, a non-finite number, a pressure outside
    /// 0 to 1, another pointer type or a modifier DOM does not name.
    pub fn parse(payload: &str) -> Option<PointerEvent> {
        let mut parts = payload.split(',');
        let mut number = || {
            exact_num::parse_f64(parts.next()?)
                .ok()
                .filter(|v| v.is_finite())
        };
        let (offset_x, offset_y, buttons, pressure) = (number()?, number()?, number()?, number()?);
        let pointer_type = parts.next()?;
        let pointer_id = exact_num::parse_f64(parts.next()?).ok()?;
        let held = super::KeyModifiers::held(parts.next().unwrap_or(""))?;
        let valid = parts.next().is_none()
            && (0.0..=1.0).contains(&pressure)
            && buttons >= 0.0
            && pointer_id.is_finite()
            && matches!(pointer_type, "mouse" | "pen" | "touch");
        valid.then(|| PointerEvent {
            offset_x,
            offset_y,
            buttons,
            pressure,
            pointer_type: pointer_type.into(),
            pointer_id,
            held,
        })
    }

    /// The `PointerEvent` record, its fields in the compiler's order.
    pub fn value(&self) -> Value {
        Value::record(vec![
            Value::Number(self.offset_x),
            Value::Number(self.offset_y),
            Value::Number(self.buttons),
            Value::Number(self.pressure),
            Value::str(&self.pointer_type),
            Value::Number(self.pointer_id),
            Value::Bool(self.held.shift),
            Value::Bool(self.held.ctrl),
            Value::Bool(self.held.alt),
            Value::Bool(self.held.meta),
        ])
    }
}

/// A wheel's turn or a trackpad's scroll over a node (DOM's `WheelEvent`),
/// a pinch delivered as one with Control held, as browsers do.
#[derive(Debug, Clone, PartialEq)]
pub struct WheelEvent {
    /// From the left of the node's content box, CSS px.
    pub offset_x: f64,
    /// From the top of the node's content box, CSS px.
    pub offset_y: f64,
    /// How far it scrolls along x, in `delta_mode`'s units.
    pub delta_x: f64,
    /// How far it scrolls along y; positive scrolls down, as the DOM's.
    pub delta_y: f64,
    /// DOM's `deltaMode`: 0 pixels, 1 lines, 2 pages.
    pub delta_mode: f64,
    /// The modifier keys held (`ctrlKey` for a pinch).
    pub held: super::KeyModifiers,
}

/// Files dropped on a node from outside the app (DOM's `drop` with its
/// `DataTransfer`'s files): each a `doc:` handle the host minted, as a
/// picker's choice is (LLP 1069.010 D1).
#[derive(Debug, Clone, PartialEq)]
pub struct DropEvent {
    /// From the left of the node's content box, CSS px.
    pub offset_x: f64,
    /// From the top of the node's content box, CSS px.
    pub offset_y: f64,
    /// The dropped files' handles, in the order the platform gave them.
    pub files: Vec<String>,
    /// The modifier keys held.
    pub held: super::KeyModifiers,
}

/// `n` finite numbers, then the optional modifiers, from comma-separated
/// fields; `None` for anything else.
fn numbers<const N: usize>(line: &str) -> Option<([f64; N], super::KeyModifiers)> {
    let mut parts = line.split(',');
    let mut out = [0.0; N];
    for slot in &mut out {
        *slot = exact_num::parse_f64(parts.next()?)
            .ok()
            .filter(|v| v.is_finite())?;
    }
    let held = super::KeyModifiers::held(parts.next().unwrap_or(""))?;
    parts.next().is_none().then_some((out, held))
}

impl WheelEvent {
    /// Decode the wire line; `None` for a malformed one, a non-finite
    /// number or a `deltaMode` DOM does not name.
    pub fn parse(payload: &str) -> Option<WheelEvent> {
        let ([offset_x, offset_y, delta_x, delta_y, delta_mode], held) = numbers(payload)?;
        [0.0, 1.0, 2.0].contains(&delta_mode).then_some(WheelEvent {
            offset_x,
            offset_y,
            delta_x,
            delta_y,
            delta_mode,
            held,
        })
    }

    /// The `WheelEvent` record, its fields in the compiler's order.
    pub fn value(&self) -> Value {
        Value::record(vec![
            Value::Number(self.offset_x),
            Value::Number(self.offset_y),
            Value::Number(self.delta_x),
            Value::Number(self.delta_y),
            Value::Number(self.delta_mode),
            Value::Bool(self.held.shift),
            Value::Bool(self.held.ctrl),
            Value::Bool(self.held.alt),
            Value::Bool(self.held.meta),
        ])
    }
}

impl DropEvent {
    /// Decode the point line and the handles under it; `None` without a
    /// file, or for a line that is not a `doc:` handle — a host mints every
    /// dropped file before it is delivered.
    pub fn parse(payload: &str) -> Option<DropEvent> {
        let mut lines = payload.split('\n');
        let ([offset_x, offset_y], held) = numbers(lines.next()?)?;
        let files: Vec<String> = lines.map(str::to_owned).collect();
        let valid = !files.is_empty() && files.iter().all(|f| f.starts_with("doc:/"));
        valid.then_some(DropEvent {
            offset_x,
            offset_y,
            files,
            held,
        })
    }

    /// The `DragEvent` record, its fields in the compiler's order.
    pub fn value(&self) -> Value {
        Value::record(vec![
            Value::Number(self.offset_x),
            Value::Number(self.offset_y),
            Value::list(self.files.iter().map(|f| Value::str(f)).collect()),
            Value::Bool(self.held.shift),
            Value::Bool(self.held.ctrl),
            Value::Bool(self.held.alt),
            Value::Bool(self.held.meta),
        ])
    }
}

impl Event {
    /// Decode ABI kind 29 (`pointerdown`), 30 (`pointerup`) or 31
    /// (`pointermove`).
    pub fn pointer_payload(kind: u32, payload: &str) -> Option<Event> {
        let pointer = PointerEvent::parse(payload)?;
        match kind {
            29 => Some(Event::Pointerdown(pointer)),
            30 => Some(Event::Pointerup(pointer)),
            31 => Some(Event::Pointermove(pointer)),
            _ => None,
        }
    }

    /// Decode ABI kind 10 (`contextmenu`): no payload when the platform
    /// asked for a menu without a point (a keyboard's menu key), else the
    /// secondary click's pointer line.
    pub fn contextmenu_payload(payload: &str) -> Option<Event> {
        if payload.is_empty() {
            return Some(Event::Contextmenu);
        }
        PointerEvent::parse(payload).map(Event::ContextmenuAt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_line_decodes_and_refuses_what_dom_never_sends() {
        let p = PointerEvent::parse("12.5,-3,1,0.25,pen,7").unwrap();
        assert_eq!(
            (p.offset_x, p.offset_y, p.buttons, p.pressure),
            (12.5, -3.0, 1.0, 0.25)
        );
        assert_eq!((p.pointer_type.as_str(), p.pointer_id), ("pen", 7.0));
        for bad in [
            "",
            "1,2,1,0.5,mouse",
            "1,2,1,1.5,mouse,1",
            "1,NaN,1,0.5,mouse,1",
            "1,2,1,0.5,stylus,1",
            "1,2,1,0.5,mouse,1,Hyper",
            "1,2,1,0.5,mouse,1,Shift,9",
        ] {
            assert!(PointerEvent::parse(bad).is_none(), "{bad}");
        }
        let held = PointerEvent::parse("1,2,1,0.5,mouse,1,Shift+Meta")
            .unwrap()
            .held;
        assert!(held.shift && held.meta && !held.ctrl && !held.alt);
        assert!(matches!(
            Event::pointer_payload(31, "0,0,0,0,mouse,1"),
            Some(Event::Pointermove(_))
        ));
        assert!(matches!(
            Event::contextmenu_payload(""),
            Some(Event::Contextmenu)
        ));
        assert!(matches!(
            Event::contextmenu_payload("40,8,2,0.5,mouse,1,Control"),
            Some(Event::ContextmenuAt(p)) if p.offset_x == 40.0 && p.held.ctrl
        ));
        assert!(Event::contextmenu_payload("x").is_none());
    }

    #[test]
    fn a_wheel_and_a_drop_decode_and_refuse_what_dom_never_sends() {
        let w = WheelEvent::parse("10,20,0,-120.5,0,Control").unwrap();
        assert_eq!((w.delta_x, w.delta_y, w.delta_mode), (0.0, -120.5, 0.0));
        assert!(w.held.ctrl && !w.held.meta);
        for bad in ["", "1,2,3,4", "1,2,3,4,3", "1,2,NaN,4,0", "1,2,3,4,0,Hyper"] {
            assert!(WheelEvent::parse(bad).is_none(), "{bad}");
        }
        let d = DropEvent::parse("5,6,Shift\ndoc:/1/a.board\ndoc:/2/b, c.board").unwrap();
        assert_eq!(d.files, ["doc:/1/a.board", "doc:/2/b, c.board"]);
        assert!(d.held.shift);
        for bad in ["5,6", "5,6\n/tmp/a.board", "x\ndoc:/1/a"] {
            assert!(DropEvent::parse(bad).is_none(), "{bad}");
        }
    }
}
