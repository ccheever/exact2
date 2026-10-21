//! Typed node properties.
//!
//! A prop is a [`PropId`] from the generated table with a value of its declared
//! [`PropKind`]. Booleans are booleans — never `"true"`. A node's props live in
//! a [`PropList`]: a small vector sorted by id, contiguous per node, with no
//! hashing on the read path.

use crate::generated::{PropId, PropKind};
use smol_str::SmolStr;

/// A typed prop value.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    /// Immutable UTF-8 text: short strings inline, longer strings shared.
    Str(SmolStr),
    /// A boolean.
    Bool(bool),
    /// A signed integer.
    Int(i64),
    /// A double.
    Float(f64),
}

impl PropValue {
    /// The value's kind.
    pub fn kind(&self) -> PropKind {
        match self {
            PropValue::Str(_) => PropKind::Str,
            PropValue::Bool(_) => PropKind::Bool,
            PropValue::Int(_) => PropKind::Int,
            PropValue::Float(_) => PropKind::Float,
        }
    }

    /// The string, if this is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropValue::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The boolean, if this is a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PropValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The integer, if this is an integer.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            PropValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// The double, if this is a double.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            PropValue::Float(f) => Some(*f),
            _ => None,
        }
    }
}

impl From<&str> for PropValue {
    fn from(s: &str) -> Self {
        PropValue::Str(s.into())
    }
}

impl From<String> for PropValue {
    fn from(s: String) -> Self {
        PropValue::Str(s.into())
    }
}

impl From<bool> for PropValue {
    fn from(b: bool) -> Self {
        PropValue::Bool(b)
    }
}

impl From<i64> for PropValue {
    fn from(i: i64) -> Self {
        PropValue::Int(i)
    }
}

impl From<f64> for PropValue {
    fn from(f: f64) -> Self {
        PropValue::Float(f)
    }
}

/// One node's props, sorted by [`PropId`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropList {
    entries: Vec<(PropId, PropValue)>,
}

impl PropList {
    /// An empty list.
    pub const fn new() -> Self {
        PropList {
            entries: Vec::new(),
        }
    }

    fn position(&self, id: PropId) -> Result<usize, usize> {
        self.entries.binary_search_by_key(&id, |(k, _)| *k)
    }

    /// The value for `id`, if set.
    pub fn get(&self, id: PropId) -> Option<&PropValue> {
        self.position(id).ok().map(|i| &self.entries[i].1)
    }

    /// The string value for `id`, if set and a string.
    pub fn str(&self, id: PropId) -> Option<&str> {
        self.get(id).and_then(PropValue::as_str)
    }

    /// The boolean value for `id`, if set and a boolean.
    pub fn bool(&self, id: PropId) -> Option<bool> {
        self.get(id).and_then(PropValue::as_bool)
    }

    /// Where the node links to: its `href`, unless that turns out empty.
    /// @ref LLP 1115 §3 — an empty `href` is no link on any host: no link
    /// colour, role or pointer, and nothing to follow. It is HTML's `<a>`
    /// with no `href`: the node is still that element, which is what
    /// `Arena::is_button` reads from the row's presence, so its box is laid
    /// out as it was. Every host asks this and not the row.
    pub fn link(&self) -> Option<&str> {
        self.str(PropId::Href).filter(|href| !href.is_empty())
    }

    /// The level of a heading: its `aria-level`, else ARIA's default 2,
    /// which lowering also writes. `None` when the node is no heading.
    /// @ref LLP 1115.000 D12.5 — only `role="heading"` makes one, to
    /// assistive technology as to the bar and the heading's style: an
    /// `aria-level` alone is not a heading on any host (`treeitem`, `row`
    /// and `listitem` take a level of their own). Every host asks this.
    pub fn heading(&self) -> Option<i64> {
        (self.str(PropId::AccessibilityRole) == Some("heading")).then(|| {
            match self.get(PropId::AccessibilityHeadingLevel) {
                Some(PropValue::Int(level)) if *level >= 1 => *level,
                _ => 2,
            }
        })
    }

    /// Whether `id` is set.
    pub fn contains(&self, id: PropId) -> bool {
        self.position(id).is_ok()
    }

    /// Insert or replace. Returns the previous value.
    pub fn set(&mut self, id: PropId, value: PropValue) -> Option<PropValue> {
        match self.position(id) {
            Ok(i) => Some(std::mem::replace(&mut self.entries[i].1, value)),
            Err(i) => {
                // A node sets a few props, once: room for exactly them, not
                // the four a growing vector starts with.
                self.entries.reserve_exact(1);
                self.entries.insert(i, (id, value));
                None
            }
        }
    }

    /// Remove. Returns the previous value.
    pub fn remove(&mut self, id: PropId) -> Option<PropValue> {
        match self.position(id) {
            Ok(i) => Some(self.entries.remove(i).1),
            Err(_) => None,
        }
    }

    /// Iterate in id order.
    pub fn iter(&self) -> impl Iterator<Item = (PropId, &PropValue)> {
        self.entries.iter().map(|(k, v)| (*k, v))
    }

    /// Number of props set.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no prop is set.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove every prop.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_inline_and_long_text_clones_share_storage() {
        for text in ["", "a", "界é", "12345678901234567890123"] {
            let PropValue::Str(value) = PropValue::from(text) else {
                unreachable!()
            };
            assert!(!value.is_heap_allocated());
            assert_eq!(value.as_str(), text);
            assert_eq!(PropValue::from(text.to_string()).as_str(), Some(text));
        }
        for text in ["123456789012345678901234", "界".repeat(1024).as_str()] {
            let value = PropValue::from(text);
            let kept = value.clone();
            assert_eq!(
                value.as_str().unwrap().as_ptr(),
                kept.as_str().unwrap().as_ptr()
            );
            drop(value);
            assert_eq!(kept.as_str(), Some(text));
        }
    }

    #[test]
    fn text_storage_preserves_wire_exports_and_independent_kernel_forks() {
        use crate::{wire, Kernel, MonospaceMeasurer, NodeType, Op};
        for text in [
            "".to_string(),
            "12345678901234567890123".to_string(),
            "123456789012345678901234".to_string(),
            "界é\0e\u{301}\r\n".repeat(300),
            "\n\n          ".repeat(8),
        ] {
            let set = Op::SetProp {
                id: 1,
                prop: PropId::Text,
                value: text.as_str().into(),
            };
            let mut encoded = wire::codec::Writer::new();
            set.encode_payload(&mut encoded);
            let mut expected = wire::codec::Writer::new();
            expected.u16(PropId::Text as u16);
            expected.u8(PropKind::Str as u8);
            expected.string(&text);
            assert_eq!(encoded.into_vec(), expected.into_vec());
            let ops = [
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::Text,
                },
                set,
                Op::AttachRoot { id: 1 },
            ];
            let mut structured = Kernel::with_monospace();
            structured.apply(0, 1, &ops).unwrap();
            let mut decoded = Kernel::with_monospace();
            decoded.apply_frame(&wire::encode(0, 1, &ops)).unwrap();
            drop(ops);
            assert_eq!(
                structured.export(None).unwrap(),
                decoded.export(None).unwrap()
            );
            let snapshot = crate::export::decode(&decoded.export(None).unwrap()).unwrap();
            assert_eq!(snapshot.props[0].str(PropId::Text), Some(text.as_str()));
            let fork = structured.rehydrate(Box::new(MonospaceMeasurer::default()));
            structured
                .apply(
                    0,
                    2,
                    &[Op::SetProp {
                        id: 1,
                        prop: PropId::Text,
                        value: "changed".into(),
                    }],
                )
                .unwrap();
            assert_eq!(
                fork.node(1).unwrap().props.str(PropId::Text),
                Some(text.as_str())
            );
            assert_eq!(
                decoded.node(1).unwrap().props.str(PropId::Text),
                Some(text.as_str())
            );
        }
    }

    #[test]
    fn set_get_remove_keep_sorted_order() {
        let mut list = PropList::new();
        assert!(list.set(PropId::TestId, "b".into()).is_none());
        assert!(list.set(PropId::Text, "a".into()).is_none());
        assert!(list.set(PropId::Disabled, true.into()).is_none());
        let ids: Vec<_> = list.iter().map(|(id, _)| id).collect();
        assert_eq!(ids, vec![PropId::Text, PropId::Disabled, PropId::TestId]);
        assert_eq!(list.str(PropId::Text), Some("a"));
        assert_eq!(list.bool(PropId::Disabled), Some(true));
        assert_eq!(
            list.set(PropId::Text, "c".into()),
            Some(PropValue::Str("a".into()))
        );
        assert_eq!(list.remove(PropId::Disabled), Some(PropValue::Bool(true)));
        assert!(list.remove(PropId::Disabled).is_none());
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn kinds_match_declared_kinds() {
        assert_eq!(PropValue::from("x").kind(), PropKind::Str);
        assert_eq!(PropValue::from(true).kind(), PropKind::Bool);
        assert_eq!(PropValue::from(3i64).kind(), PropKind::Int);
        assert_eq!(PropValue::from(1.5f64).kind(), PropKind::Float);
        assert_eq!(PropId::Text.kind(), PropKind::Str);
        assert_eq!(PropId::Disabled.kind(), PropKind::Bool);
        assert_eq!(PropId::TabIndex.kind(), PropKind::Int);
        assert_eq!(PropId::HitSlop.kind(), PropKind::Float);
    }
}
