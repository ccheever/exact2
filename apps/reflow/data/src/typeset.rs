//! Pretext's `layout` half, on the hosts' own walker: line counts, column
//! cuts and shape-aware flow over the advance tables of `font.rs`.
use crate::font::FaceData;
use exact_textflow::{
    flow, Cursor, Direction, FlowOptions, FlowShape, Fragment, Options, Prepared, WhiteSpace,
    MIN_FRAGMENT_EM,
};

/// A face at a size with a fixed line height, as the Contract sets it.
#[derive(Clone, Copy)]
pub struct Type {
    /// The face's advance table.
    pub face: &'static FaceData,
    /// `font-size`, px.
    pub size: f32,
    /// `line-height`, px (a fixed length in the Contract).
    pub line_height: f32,
}

impl Type {
    /// Prepare `text` under `white-space: pre-wrap`, the paragraphs' setting.
    pub fn prepare(&self, text: &str) -> Prepared {
        let options = Options {
            white_space: WhiteSpace::PreWrap,
            ..Options::default()
        };
        Prepared::new(text, options, &mut self.face.measure(text, self.size))
    }

    /// How many lines `text` takes at `width`.
    pub fn lines(&self, text: &str, width: f32) -> usize {
        self.prepare(text).count_lines(width)
    }

    /// Consume up to `max_lines` lines of `text` at `width`: the byte length
    /// taken and the lines used. Zero `max_lines` means no cap.
    pub fn take_lines(&self, text: &str, width: f32, max_lines: usize) -> (usize, usize) {
        let prepared = self.prepare(text);
        let mut cursor = Cursor::default();
        let mut lines = 0;
        let mut end = 0;
        while max_lines == 0 || lines < max_lines {
            let Some(line) = prepared.next_line(cursor, width) else {
                break;
            };
            lines += 1;
            end = line.end.byte;
            cursor = line.end;
        }
        (end, lines)
    }

    /// Flow `text` at `width` around `shapes` (in the paragraph's own
    /// coordinates) for at most `max_bands` line bands (zero: uncapped):
    /// the byte length consumed, the height used, and whether all of it fit.
    pub fn take_flow(
        &self,
        text: &str,
        width: f32,
        shapes: &[FlowShape],
        max_bands: u32,
    ) -> (usize, f32, bool) {
        let prepared = self.prepare(text);
        let options = FlowOptions {
            direction: Direction::Ltr,
            width,
            line_height: self.line_height,
            min_fragment: MIN_FRAGMENT_EM * self.size,
            max_lines: max_bands,
        };
        let mut out: Vec<Fragment> = Vec::new();
        let result = flow(&prepared, shapes, &options, &mut out);
        let end = out.last().map_or(0, |f| f.end);
        (end, result.height, result.complete)
    }
}

/// One column's share of a longer text.
pub struct Chunk {
    /// The text set in this column, trimmed of the whitespace it owns at
    /// either end (a paragraph break at a column boundary carries no line).
    pub text: String,
    /// Lines the walker used.
    pub lines: usize,
}

/// Cut `text` into `columns` chunks of `lines_per_column` lines at `width`;
/// returns the chunks and the words that did not fit.
pub fn columns(
    ty: &Type,
    text: &str,
    width: f32,
    lines_per_column: usize,
    columns: usize,
) -> (Vec<Chunk>, usize) {
    let mut rest = text.trim_start();
    let mut out = Vec::with_capacity(columns);
    for _ in 0..columns {
        if rest.is_empty() {
            break;
        }
        let (end, lines) = ty.take_lines(rest, width, lines_per_column);
        out.push(Chunk {
            text: rest[..end].trim().to_owned(),
            lines,
        });
        rest = rest[end..].trim_start();
    }
    (out, rest.split_whitespace().count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::REGULAR;
    const TY: Type = Type {
        face: &REGULAR,
        size: 16.0,
        line_height: 24.0,
    };
    #[test]
    fn lines_grow_as_width_shrinks_and_chunks_cover_the_text() {
        let text = crate::prose::ARTICLE;
        let wide = TY.lines(text, 700.0);
        let narrow = TY.lines(text, 300.0);
        assert!(narrow > wide * 2, "{narrow} vs {wide}");
        let (chunks, left) = columns(&TY, text, 300.0, 20, 40);
        assert_eq!(left, 0);
        let words: usize = chunks
            .iter()
            .map(|c| c.text.split_whitespace().count())
            .sum();
        assert_eq!(words, text.split_whitespace().count());
        assert!(chunks.iter().all(|c| c.lines <= 20));
        // A column cut at the line the walker chose has no partial word.
        for c in &chunks {
            assert!(!c.text.starts_with(' ') && !c.text.ends_with(' '));
        }
    }
    #[test]
    fn a_shape_costs_lines() {
        let text = crate::prose::BALLS;
        let (end, clear, complete) = TY.take_flow(text, 500.0, &[], 0);
        assert!(complete && end == text.len());
        let ball = FlowShape::Circle {
            cx: 250.0,
            cy: 80.0,
            r: 70.0,
        };
        let (_, blocked, complete) = TY.take_flow(text, 500.0, &[ball], 0);
        assert!(complete && blocked > clear, "{blocked} vs {clear}");
    }
}
