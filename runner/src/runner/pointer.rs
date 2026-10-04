//! `pointerdown`, `pointerup` and `pointermove`'s record (LLP 1005 §3; LLP
//! 1056 §3 stage 3, as built): a subset of DOM's `PointerEvent`, by its own
//! names, which an action takes as an optional last parameter (the paint
//! diary's F1: a drawing app had no coordinates, movement or pressure).
//!
//! The wire is one UTF-8 line every host writes the same way:
//! `offsetX,offsetY,buttons,pressure,pointerType,pointerId`.

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
}

impl PointerEvent {
    /// Decode the wire line; `None` for a malformed one, a non-finite
    /// number, a pressure outside 0 to 1 or another pointer type.
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
            "1,2,1,0.5,mouse,1,9",
        ] {
            assert!(PointerEvent::parse(bad).is_none(), "{bad}");
        }
        assert!(matches!(
            Event::pointer_payload(31, "0,0,0,0,mouse,1"),
            Some(Event::Pointermove(_))
        ));
    }
}
