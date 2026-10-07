//! A text field's selection, and the `InputEvent` record (x2apps codeedit
//! #2, survey #2): what `input` and `change` hand an action that takes one
//! more parameter, and what a text field's `select` carries — the target's
//! own fields as the event fires, by the DOM's names
//! (`contract/types/src/selection.rs` declares the record).

use super::{ControlValue, DataSource, Event, Runner};
use exact_kernel::{ControlKind, PropId, ViewId};
use exact_plan::Value;

/// A text field's selection as the DOM reports it: `selectionStart`,
/// `selectionEnd` in UTF-16 code units of its value, and
/// `selectionDirection`; a caret is a selection whose ends are equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSelection {
    /// `selectionStart`.
    pub start: u32,
    /// `selectionEnd`, never before `start`.
    pub end: u32,
    /// `selectionDirection`.
    pub direction: SelectionDirection,
}

/// HTML's `selectionDirection`: which end of a selection the person moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SelectionDirection {
    /// `forward`: the end moves.
    Forward,
    /// `backward`: the start moves.
    Backward,
    /// `none`: the platform keeps no direction (a caret).
    #[default]
    None,
}

impl SelectionDirection {
    /// The DOM's word.
    pub fn name(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
            Self::None => "none",
        }
    }

    /// The direction a word names, as `setSelectionRange` reads its third
    /// argument: anything but `forward` or `backward` is `none`.
    pub fn of(word: &str) -> Self {
        match word {
            "forward" => Self::Forward,
            "backward" => Self::Backward,
            _ => Self::None,
        }
    }
}

impl FieldSelection {
    /// A caret at `at`.
    pub fn caret(at: u32) -> Self {
        Self {
            start: at,
            end: at,
            direction: SelectionDirection::None,
        }
    }

    /// The caret after the last character of `text`: where an edit a host
    /// reports no selection for leaves it (typing at the end).
    pub fn at_end(text: &str) -> Self {
        Self::caret(utf16_len(text))
    }

    /// `setSelectionRange(start, end, direction)` on a value of `len` UTF-16
    /// units, as HTML clamps it: each end an `unsigned long` (WebIDL's
    /// ToUint32, so `-1` is past the end), then to the length, and an end
    /// before the start moves the start to it.
    pub fn clamped(start: f64, end: f64, direction: SelectionDirection, len: u32) -> Self {
        let at = |n: f64| {
            let n = if n.is_finite() {
                (n.trunc() % 4_294_967_296.0 + 4_294_967_296.0) % 4_294_967_296.0
            } else {
                0.0
            };
            n.min(f64::from(len)) as u32
        };
        let end = at(end);
        Self {
            start: at(start).min(end),
            end,
            direction,
        }
    }

    /// Decode `start,end,direction,` then the field's text verbatim: what
    /// ABI kinds 40 to 42 carry. The offsets are whole, `start <= end`, each
    /// within the text's UTF-16 length, the direction a DOM word.
    pub fn parse(payload: &str) -> Option<(Self, &str)> {
        let mut parts = payload.splitn(4, ',');
        let start: u32 = parts.next()?.parse().ok()?;
        let end: u32 = parts.next()?.parse().ok()?;
        let direction = match parts.next()? {
            "forward" => SelectionDirection::Forward,
            "backward" => SelectionDirection::Backward,
            "none" => SelectionDirection::None,
            _ => return None,
        };
        let text = parts.next()?;
        (start <= end && end <= utf16_len(text)).then_some((
            Self {
                start,
                end,
                direction,
            },
            text,
        ))
    }
}

/// A string's length in UTF-16 code units, the DOM's offsets' unit.
pub fn utf16_len(text: &str) -> u32 {
    text.encode_utf16().count().try_into().unwrap_or(u32::MAX)
}

impl Event {
    /// Decode ABI kind 40 (a text field's `input`), 41 (its `change`) or 42
    /// (its `select`), each carrying its selection then its text
    /// ([`FieldSelection::parse`]).
    pub fn field_payload(kind: u32, payload: &str) -> Option<Self> {
        let (selection, text) = FieldSelection::parse(payload)?;
        let value = ControlValue::Field(text.into(), selection);
        match kind {
            40 => Some(Event::Input(value)),
            41 => Some(Event::Change(value)),
            42 => Some(Event::FieldSelect(text.into(), selection)),
            _ => None,
        }
    }
}

impl<D: DataSource> Runner<D> {
    /// The `InputEvent` an `input`, `change` or text field's `select` at
    /// `view` hands its action, its fields in the compiler's order: the
    /// value the event carries (a checkbox's or a radio's own `value`, `on`
    /// when it has none; a range's number written as JavaScript would),
    /// whether it is checked, and a text field's selection (where a host
    /// reported none, the caret after its text, as typing leaves it); a
    /// control without a text selection reports 0, 0 and `none`.
    pub(super) fn input_record(
        &self,
        view: ViewId,
        payload: &Value,
        reported: Option<FieldSelection>,
    ) -> Value {
        let node = self.kernel.node(view);
        let kind = node.and_then(|n| ControlKind::of(n.node_type, n.props));
        let own = || {
            node.and_then(|n| n.props.str(PropId::Value))
                .unwrap_or("on")
                .to_owned()
        };
        let (value, checked) = match (kind, payload) {
            (Some(ControlKind::Checkbox | ControlKind::Switch), Value::Bool(on)) => (own(), *on),
            (Some(ControlKind::Radio), _) => (own(), true),
            (_, Value::Number(n)) => (exact_num::Shortest(*n).to_string(), false),
            (_, v @ exact_plan::str_value!()) => (v.text().to_owned(), false),
            _ => (String::new(), false),
        };
        let selection = match kind {
            None => Some(reported.unwrap_or_else(|| FieldSelection::at_end(&value))),
            Some(_) => None,
        };
        let (start, end, direction) = selection.map_or((0, 0, SelectionDirection::None), |s| {
            (s.start, s.end, s.direction)
        });
        Value::record(vec![
            Value::str(&value),
            Value::Bool(checked),
            Value::Number(f64::from(start)),
            Value::Number(f64::from(end)),
            Value::str(direction.name()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fields_selection_decodes_and_clamps_as_htmls() {
        let (s, text) = FieldSelection::parse("1,3,backward,a,b\nc").unwrap();
        assert_eq!(
            (s.start, s.end, s.direction),
            (1, 3, SelectionDirection::Backward)
        );
        assert_eq!(text, "a,b\nc");
        // Offsets are UTF-16: an emoji is two.
        assert!(FieldSelection::parse("0,2,none,😀").is_some());
        for bad in [
            "3,1,none,abcd",
            "0,9,none,abc",
            "0,0,sideways,",
            "0,0,none",
            "x,0,none,",
        ] {
            assert!(FieldSelection::parse(bad).is_none(), "{bad}");
        }
        assert!(matches!(
            Event::field_payload(42, "0,1,forward,ab"),
            Some(Event::FieldSelect(text, s)) if text == "ab" && s.end == 1
        ));
        assert!(matches!(
            Event::field_payload(40, "2,2,none,ab"),
            Some(Event::Input(ControlValue::Field(text, s))) if text == "ab" && s.start == 2
        ));
        let c = FieldSelection::clamped(9.0, 4.0, SelectionDirection::of("sideways"), 6);
        assert_eq!(
            (c.start, c.end, c.direction),
            (4, 4, SelectionDirection::None)
        );
        let c = FieldSelection::clamped(0.5, 99.0, SelectionDirection::of("forward"), 6);
        assert_eq!(
            (c.start, c.end, c.direction),
            (0, 6, SelectionDirection::Forward)
        );
        // As the DOM converts it, `-1` is the largest offset: the end.
        let c = FieldSelection::clamped(-1.0, -1.0, SelectionDirection::None, 6);
        assert_eq!((c.start, c.end), (6, 6));
        let c = FieldSelection::clamped(f64::NAN, f64::INFINITY, SelectionDirection::None, 6);
        assert_eq!((c.start, c.end), (0, 0));
        assert_eq!(FieldSelection::at_end("a😀").end, 3);
    }
}
