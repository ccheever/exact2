//! The focused text field's selection, painted (x2apps codeedit #2): a
//! caret where it is collapsed, else a highlight behind the selected text,
//! as Chrome hides the caret over a range. Positions come from measuring
//! the text before an offset on its line; a textarea's line is a hard one
//! (between newlines), which a soft wrap would put elsewhere.

use super::{text_spec, Shape};
use exact_kernel::StyleProps;
use exact_runner::FieldSelection;
use tiny_skia::Transform;

/// Chrome's selection highlight on a focused field, light and dark.
const HIGHLIGHT: [[u8; 4]; 2] = [[0xb4, 0xd5, 0xfe, 0xff], [0x3f, 0x63, 0x8b, 0xff]];

/// A field's painted text up to UTF-16 offset `at` of its value: a password
/// is one bullet a character, as it is painted.
fn painted_prefix(value: &str, at: u32, masked: bool) -> String {
    let mut units = 0u32;
    let end = value
        .char_indices()
        .find(|(_, c)| {
            units += c.len_utf16() as u32;
            units > at
        })
        .map_or(value.len(), |(i, _)| i);
    if masked {
        "\u{2022}".repeat(value[..end].chars().count())
    } else {
        value[..end].to_owned()
    }
}

/// A field's painted text, as its selection is placed on it.
pub(super) struct FieldText<'a> {
    /// Its node: whose caret texts these are.
    pub node: u32,
    /// The field's inherited style, its value never collapsed.
    pub style: &'a StyleProps,
    /// Its value, whose UTF-16 units the selection counts.
    pub value: &'a str,
    /// A password's, painted one bullet a character.
    pub masked: bool,
    /// The painted text's top left.
    pub origin: (f32, f32),
}

impl super::Painter {
    /// Where offset `at` falls, in the viewport: its x, the top of its
    /// line, and the line's height.
    fn field_point(&self, field: &FieldText<'_>, at: u32, end: u8) -> (f32, f32, f32) {
        let prefix = painted_prefix(field.value, at, field.masked);
        let style = field.style;
        let mut text = self.text.borrow_mut();
        let line = text.paragraph(&text_spec(style, " "), None).height;
        let line = if line > 0.0 {
            line
        } else {
            style.font_size * 1.2
        };
        let last = prefix.rsplit('\n').next().unwrap_or("");
        // The text up to an end of the selection: each replaces the last one
        // measured for that end (1 its start or the caret, 2 its end).
        let x = text
            .paragraph_replacing((field.node, end), &text_spec(style, last), None)
            .width;
        let y = prefix.matches('\n').count() as f32 * line;
        (field.origin.0 + x, field.origin.1 + y, line)
    }

    /// The highlight behind a selected range, where both ends are on one
    /// line; drawn before the text.
    pub(super) fn field_highlight(
        &mut self,
        field: &FieldText<'_>,
        selection: FieldSelection,
        ts: Transform,
    ) {
        if selection.start == selection.end {
            return;
        }
        let (x0, y0, line) = self.field_point(field, selection.start, 1);
        let (x1, y1, _) = self.field_point(field, selection.end, 2);
        if y0 != y1 {
            return;
        }
        let color = HIGHLIGHT[usize::from(self.dark)];
        self.backend
            .fill(&Shape::rect((x0, y0, x1 - x0, line)), color, ts);
    }

    /// The caret at a collapsed selection, one point wide and a line tall.
    pub(super) fn field_caret(
        &mut self,
        field: &FieldText<'_>,
        color: [u8; 4],
        selection: FieldSelection,
        ts: Transform,
    ) {
        if selection.start != selection.end {
            return;
        }
        let (x, y, line) = self.field_point(field, selection.start, 1);
        self.backend
            .fill(&Shape::rect((x, y, 1.0, line)), color, ts);
    }

    /// A field's painted focus: a two-point ring in the accent
    /// colour over its chrome or bare outer edge (LLP 1104 D6–D7).
    pub(super) fn field_ring(&mut self, outer: &Shape, accent: [u8; 4], ts: Transform) {
        for part in super::border::border_fills(outer, [2.0; 4], [accent; 4]) {
            self.backend.fill_border(&part, ts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::painted_prefix;

    #[test]
    fn a_fields_prefix_counts_utf16_and_masks_a_password() {
        assert_eq!(painted_prefix("a😀b", 3, false), "a😀");
        assert_eq!(painted_prefix("a😀b", 2, false), "a", "inside the pair");
        assert_eq!(painted_prefix("abc", 9, false), "abc");
        assert_eq!(painted_prefix("abc", 2, true), "\u{2022}\u{2022}");
    }
}
