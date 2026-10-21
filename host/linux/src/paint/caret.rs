//! A field's caret and selection use the same visual lines as its text.
use super::{field::PaintedField, Shape};
use exact_runner::FieldSelection;
use tiny_skia::Transform;

const HIGHLIGHT: [[u8; 4]; 2] = [[0xb4, 0xd5, 0xfe, 0xff], [0x3f, 0x63, 0x8b, 0xff]];

impl super::Painter {
    pub(super) fn field_highlight(&mut self, field: &PaintedField, selection: FieldSelection) {
        if selection.start == selection.end {
            return;
        }
        for rect in field.geometry.highlight(selection.start, selection.end) {
            self.backend.fill(
                &Shape::rect(field.place(rect)),
                HIGHLIGHT[usize::from(self.dark)],
                field.transform,
            );
        }
    }

    pub(super) fn field_caret(
        &mut self,
        field: &PaintedField,
        color: [u8; 4],
        selection: FieldSelection,
        upstream: bool,
    ) {
        if selection.start != selection.end {
            return;
        }
        let rect = field.geometry.caret(crate::text::field::Cursor {
            offset: selection.start,
            upstream,
        });
        self.backend
            .fill(&Shape::rect(field.place(rect)), color, field.transform);
    }

    /// A field's painted focus: a two-point ring in the accent
    /// colour over its chrome or bare outer edge (LLP 1104 D6–D7).
    pub(super) fn field_ring(&mut self, outer: &Shape, accent: [u8; 4], ts: Transform) {
        for part in super::border::border_fills(outer, [2.0; 4], [accent; 4]) {
            self.backend.fill_border(&part, ts);
        }
    }
}
