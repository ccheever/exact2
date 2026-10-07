//! `key`'s and `keyup`'s record (#140): DOM's `KeyboardEvent` as a host
//! reports a keydown or a keyup — the key's name, the modifiers held, the
//! physical key and whether a keydown is the platform's auto-repeat.
//!
//! The wire is the chord every host already writes (`Event::key`: the
//! modifiers before the key in Playwright's spelling, `Shift+Meta+b`), then,
//! where the host knows them, `\n` and the `code`, `\n` and `true` or
//! `false` for `repeat`. A chord alone is `code` "" (DOM's value for a key
//! with no physical position, such as a synthesized one) and no repeat.

use super::KeyModifiers;
use exact_plan::Value;

/// A key going down (`key`, DOM's `keydown`) or coming up (`keyup`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyboardEvent {
    /// `KeyboardEvent.key`: what the key types (`"a"`, `"A"`, `"7"`) or its
    /// name (`"Enter"`, `"ArrowDown"`, `"Meta"`).
    pub key: String,
    /// `KeyboardEvent.code`: the physical key (`"KeyB"`, `"Digit1"`,
    /// `"MetaLeft"`), whatever the layout types there; "" when unknown.
    pub code: String,
    /// `KeyboardEvent.repeat`: a keydown the platform repeats while the key
    /// is held. Never true on a keyup.
    pub repeat: bool,
    /// The modifiers held as the event fires. A modifier's own keydown
    /// holds it (`Meta`'s says `metaKey`) and its keyup no longer does.
    pub held: KeyModifiers,
}

impl KeyboardEvent {
    /// A bare chord ([`KeyModifiers::split`]): no `code`, no repeat.
    pub fn chord(chord: &str) -> Self {
        let (held, key) = KeyModifiers::split(chord);
        Self {
            key: key.into(),
            held,
            ..Self::default()
        }
    }

    /// Decode the wire: a chord, or a chord, its `code` (ASCII letters and
    /// digits, or "") and `true` or `false` on three lines. `None` for any
    /// other shape, which a host reports as the batch's error.
    pub fn parse(payload: &str) -> Option<Self> {
        let mut lines = payload.split('\n');
        let mut event = Self::chord(lines.next()?);
        let Some(code) = lines.next() else {
            return Some(event);
        };
        if !code.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return None;
        }
        event.code = code.into();
        event.repeat = match lines.next()? {
            "true" => true,
            "false" => false,
            _ => return None,
        };
        lines.next().is_none().then_some(event)
    }

    /// The `KeyboardEvent` record, its fields in the compiler's order
    /// (`contract/types/src/selection.rs`).
    pub fn value(&self) -> Value {
        Value::record(vec![
            Value::str(&self.key),
            Value::Bool(self.held.shift),
            Value::Bool(self.held.ctrl),
            Value::Bool(self.held.alt),
            Value::Bool(self.held.meta),
            Value::str(&self.code),
            Value::Bool(self.repeat),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyModifiers, KeyboardEvent};

    #[test]
    fn a_chord_alone_has_no_code_and_no_repeat() {
        let e = KeyboardEvent::parse("Shift+Meta+b").unwrap();
        assert_eq!(e.key, "b");
        assert_eq!(e.code, "");
        assert!(!e.repeat);
        let held = KeyModifiers {
            shift: true,
            meta: true,
            ..KeyModifiers::default()
        };
        assert_eq!(e.held, held);
    }

    #[test]
    fn the_code_and_repeat_ride_after_the_chord() {
        let e = KeyboardEvent::parse("Meta+b\nKeyB\ntrue").unwrap();
        assert_eq!(
            (e.key.as_str(), e.code.as_str(), e.repeat),
            ("b", "KeyB", true)
        );
        assert!(e.held.meta);
        // A modifier's release: its own flag is no longer held.
        let up = KeyboardEvent::parse("Meta\nMetaLeft\nfalse").unwrap();
        assert_eq!((up.key.as_str(), up.code.as_str()), ("Meta", "MetaLeft"));
        assert_eq!(up.held, KeyModifiers::default());
        // A key whose name is a modifier's word with `+` stays a key.
        let plus = KeyboardEvent::parse("Shift++\nEqual\nfalse").unwrap();
        assert_eq!((plus.key.as_str(), plus.held.shift), ("+", true));
        // An unknown physical key is DOM's "".
        assert_eq!(KeyboardEvent::parse("a\n\nfalse").unwrap().code, "");
    }

    #[test]
    fn a_malformed_wire_is_refused() {
        for refused in [
            "a\nKeyA",
            "a\nKeyA\nyes",
            "a\nKey A\nfalse",
            "a\nKeyA\nfalse\nx",
        ] {
            assert_eq!(KeyboardEvent::parse(refused), None, "{refused:?}");
        }
    }
}
