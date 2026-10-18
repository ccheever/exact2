//! App-computed decorations over a field's plain string (LLP 1114.007).
//! Hosts share these UTF-16 reconciliation rules; painting and IME ownership
//! stay with the host's editor.

/// A half-open source range in UTF-16 code units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Range {
    /// The first code unit.
    pub start: u32,
    /// The code unit after the range.
    pub end: u32,
}
impl Range {
    /// Keep a nonempty range inside a string of `length` code units.
    pub fn clamped(self, length: u32) -> Option<Self> {
        let range = Self {
            start: self.start.min(length),
            end: self.end.min(length),
        };
        (range.start < range.end).then_some(range)
    }

    /// Whether the selection touches this range, including a caret at an edge.
    pub fn touches(self, selection: Self) -> bool {
        selection.start <= self.end && selection.end >= self.start
    }
}

/// A named inline style over source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// A declaration visible to this field.
    pub name: String,
    /// The source characters it styles.
    pub range: Range,
}

/// A block's optional marker, independent of its source grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// A bullet.
    Bullet,
    /// The marker written in the source.
    Source,
    /// An unchecked checkbox.
    CheckboxOff,
    /// A checked checkbox.
    CheckboxOn,
    /// A horizontal rule.
    Rule,
}
impl Marker {
    /// The record's marker vocabulary; an empty or unknown word draws none.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "bullet" => Self::Bullet,
            "source" => Self::Source,
            "checkbox:off" => Self::CheckboxOff,
            "checkbox:on" => Self::CheckboxOn,
            "rule" => Self::Rule,
            _ => return None,
        })
    }

    /// The spelling carried by the serialized record.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bullet => "bullet",
            Self::Source => "source",
            Self::CheckboxOff => "checkbox:off",
            Self::CheckboxOn => "checkbox:on",
            Self::Rule => "rule",
        }
    }
}

/// A named block style over whole source lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The declaration and source range.
    pub style: Span,
    /// The optional marker drawn beside or over the block.
    pub marker: Option<Marker>,
}

/// Source that occupies no advance and is omitted from accessibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hidden {
    /// The source characters to hide.
    pub range: Range,
    /// Touching this interval reveals the characters dimmed, without an app turn.
    pub reveal: Option<Range>,
}
impl Hidden {
    /// Whether this hidden source is revealed by the field's current selection.
    pub fn revealed(self, selection: Range) -> bool {
        self.reveal.is_some_and(|range| range.touches(selection))
    }
}

/// A source range edited and traversed as one labelled unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atom {
    /// The declaration and source range.
    pub style: Span,
    /// The chip's visible and accessible label.
    pub label: String,
}

/// One app record. Its complete source is part of its identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decorations {
    /// The exact string for which the app produced these ranges.
    pub value: String,
    /// Inline ranges, in precedence order; later ranges win overlaps.
    pub spans: Vec<Span>,
    /// Whole-line ranges, in precedence order.
    pub blocks: Vec<Block>,
    /// Hidden source runs.
    pub hidden: Vec<Hidden>,
    /// Atomic source runs.
    pub atoms: Vec<Atom>,
}

/// Ranges reconciled against the host's current text. This does not replace the
/// app record: retain that record to reconcile again after another keystroke.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolved {
    /// Whether the record names an older or different source string.
    pub provisional: bool,
    /// Reconciled inline ranges, with original precedence preserved.
    pub spans: Vec<Span>,
    /// Reconciled block ranges.
    pub blocks: Vec<Block>,
    /// Hidden runs that are still safe to hide.
    pub hidden: Vec<Hidden>,
    /// Atomic runs that are still safe to edit as one unit.
    pub atoms: Vec<Atom>,
}

/// The single replacement between two strings' common prefix and suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edit {
    /// The unchanged prefix's UTF-16 length.
    pub start: u32,
    /// The end of the replaced source in the old string.
    pub old_end: u32,
    /// The end of its replacement in the current string.
    pub new_end: u32,
}
impl Edit {
    /// Find the replacement without splitting an astral character at a matching
    /// high or low surrogate. Offsets remain UTF-16, as field selections are.
    pub fn between(before: &str, after: &str) -> Self {
        let mut prefix_bytes = 0;
        let mut start = 0;
        for (a, b) in before.chars().zip(after.chars()) {
            if a != b {
                break;
            }
            prefix_bytes += a.len_utf8();
            start += a.len_utf16() as u32;
        }
        let suffix = before[prefix_bytes..]
            .chars()
            .rev()
            .zip(after[prefix_bytes..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf16() as u32)
            .sum::<u32>();
        Self {
            start,
            old_end: utf16_len(before) - suffix,
            new_end: utf16_len(after) - suffix,
        }
    }

    /// Map one old boundary. Replacement-interior boundaries collapse to its
    /// start; an unchanged suffix moves by the replacement's length difference.
    pub fn boundary(self, offset: u32) -> u32 {
        if offset <= self.start {
            offset
        } else if offset >= self.old_end {
            (i64::from(offset) + i64::from(self.new_end) - i64::from(self.old_end))
                .clamp(0, i64::from(u32::MAX)) as u32
        } else {
            self.start
        }
    }

    /// A stale hidden/atomic run touching inserted text is unsafe even at an
    /// edge: its mapped range must never hide or absorb the new characters.
    pub fn intersects(self, range: Range) -> bool {
        if self.start == self.old_end {
            range.start <= self.start && self.start <= range.end
        } else {
            range.start < self.old_end && range.end > self.start
        }
    }

    fn range(self, range: Range, length: u32) -> Option<Range> {
        Range {
            start: self.boundary(range.start),
            end: self.boundary(range.end),
        }
        .clamped(length)
    }
}

impl Decorations {
    /// Resolve the current field text without changing it or making an undo
    /// entry. A host must defer applying the result while it owns marked text.
    pub fn resolve(&self, value: &str) -> Resolved {
        let provisional = self.value != value;
        let edit = Edit::between(&self.value, value);
        let old_length = utf16_len(&self.value);
        let length = utf16_len(value);
        let range = |r: Range| edit.range(r.clamped(old_length)?, length);
        let span = |s: &Span| {
            Some(Span {
                name: s.name.clone(),
                range: range(s.range)?,
            })
        };
        Resolved {
            provisional,
            spans: self.spans.iter().filter_map(span).collect(),
            blocks: self
                .blocks
                .iter()
                .filter_map(|block| {
                    Some(Block {
                        style: span(&block.style)?,
                        marker: block.marker,
                    })
                })
                .collect(),
            hidden: self
                .hidden
                .iter()
                .filter(|run| !provisional || !edit.intersects(run.range))
                .filter_map(|run| {
                    Some(Hidden {
                        range: range(run.range)?,
                        reveal: run.reveal.and_then(range),
                    })
                })
                .collect(),
            atoms: self
                .atoms
                .iter()
                .filter(|atom| !provisional || !edit.intersects(atom.style.range))
                .filter_map(|atom| {
                    Some(Atom {
                        style: span(&atom.style)?,
                        label: atom.label.clone(),
                    })
                })
                .collect(),
        }
    }
}

fn utf16_len(value: &str) -> u32 {
    value.encode_utf16().count().min(u32::MAX as usize) as u32
}

#[cfg(test)]
#[path = "decorations_tests.rs"]
mod tests;
