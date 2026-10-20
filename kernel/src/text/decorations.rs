//! App-computed paint over a field's plain string (LLP 1114.007). Hosts
//! share these UTF-16 reconciliation rules; painting, and deferring it while
//! text is composed, stay with the host's field.

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
}

/// A named range painted over source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// A declaration visible to this field.
    pub name: String,
    /// The source characters it styles.
    pub range: Range,
}

/// One app record. Its complete source is part of its identity: an equal
/// span list computed for new text is still a new record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decorations {
    /// The exact string for which the app produced these ranges.
    pub value: String,
    /// Named ranges, in precedence order; later ranges win overlaps.
    pub spans: Vec<Span>,
}

/// Ranges reconciled against the host's current text. This does not replace the
/// app record: retain that record to reconcile again after another keystroke.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolved {
    /// Whether the record names an older or different source string.
    pub provisional: bool,
    /// Reconciled ranges, with original precedence preserved.
    pub spans: Vec<Span>,
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

    fn range(self, range: Range, length: u32) -> Option<Range> {
        Range {
            start: self.boundary(range.start),
            end: self.boundary(range.end),
        }
        .clamped(length)
    }
}

impl Decorations {
    /// Resolve the current field text without changing it. A record for
    /// that text paints as given; one for other text paints provisionally,
    /// each range adjusted as the DOM adjusts a live `Range` across the one
    /// replacement between the two strings, until the app's next record.
    pub fn resolve(&self, value: &str) -> Resolved {
        let edit = Edit::between(&self.value, value);
        let old_length = utf16_len(&self.value);
        let length = utf16_len(value);
        Resolved {
            provisional: self.value != value,
            spans: self
                .spans
                .iter()
                .filter_map(|s| {
                    Some(Span {
                        name: s.name.clone(),
                        range: edit.range(s.range.clamped(old_length)?, length)?,
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
