//! One width's lines as the host keeps them (LLP 1085.000 §4, the storage
//! spike's arrangement (b)): extracted from the paragraph's Parley layouts
//! once they are broken at that width, owned by the host and independent of
//! them, so a shaped layout is broken again for the next width while every
//! painted width keeps its own glyphs.
//!
//! Glyphs are walked by run and cluster, never through Parley's glyph-run
//! iterator (6–20× slower per glyph; the spike), and placed as its
//! `positioned_glyphs` places them: each glyph at the pen plus its offset,
//! the pen moved by its advance.
use super::catalog::FaceId;
use parley::FontData;
use std::sync::Arc;

/// A laid-out glyph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutGlyph {
    /// Start of its cluster in the hard line's text, bytes.
    pub start: u32,
    /// End of its cluster (a ligature's covers every component).
    pub end: u32,
    /// Pen position from the paragraph's left, points, its offset included.
    pub x: f32,
    /// Offset below the baseline, points (y down).
    pub y: f32,
    /// Advance, points.
    pub w: f32,
    /// Points.
    pub font_size: f32,
    /// In its face.
    pub glyph_id: u32,
    /// The canonical run index ([`super::Spec`]'s runs), kept across font
    /// fallback and wrapping.
    pub metadata: u32,
    /// Its face, an index into [`Lines::faces`].
    pub face: u16,
    /// Its bidi embedding level.
    pub level: u8,
}

impl LayoutGlyph {
    /// The canonical run index.
    pub fn run(&self) -> usize {
        self.metadata as usize
    }
    /// The cluster's byte range in its hard line's text.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
    /// Where the CPU raster places it with the pen at `offset` (device
    /// pixels) at `scale`: whole pixel x, its quarter-pixel phase, and
    /// whole pixel y.
    pub fn pixel(&self, offset: (f32, f32), scale: f32) -> (i32, u8, i32) {
        let (x, bin) = super::catalog::subpixel(self.x.mul_add(scale, offset.0));
        let (y, _) = super::catalog::subpixel(self.y.mul_add(scale, offset.1).trunc());
        (x, bin, y)
    }
}

/// A face as the glyphs of one width name it.
#[derive(Clone, Debug)]
pub struct Face {
    /// The face's data and collection index.
    pub font: FontData,
    /// Normalized variation coordinates (a variable face's axes, as Parley
    /// set them for the run's weight).
    pub coords: Arc<[i16]>,
    /// An oblique Parley synthesized: the face has no italic.
    pub skew: bool,
    /// The CSS weight shaped with.
    pub weight: u16,
}

impl Face {
    /// The face's identity.
    pub fn id(&self) -> FaceId {
        FaceId::of(&self.font)
    }
}

/// One visual line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutLine {
    /// The hard line (text between line feeds) it belongs to.
    pub hard: u32,
    /// Its glyphs, a range of [`Lines::glyphs`], in visual order.
    pub glyphs: (u32, u32),
    /// Its width, points: what its content takes, indent included.
    pub w: f32,
    /// A flowed line's band (LLP 1043.000 §3 D7): ascent, descent and line
    /// height; zero for ordinary lines, whose box is the host's line pass.
    pub max_ascent: f32,
    /// See `max_ascent`.
    pub max_descent: f32,
    /// See `max_ascent`.
    pub line_height: Option<f32>,
}

/// One width's lines, glyphs and faces.
#[derive(Clone, Debug, Default)]
pub struct Lines {
    /// Visual lines, top to bottom.
    pub lines: Vec<LayoutLine>,
    /// Every line's glyphs.
    pub glyphs: Vec<LayoutGlyph>,
    /// The faces the glyphs name.
    pub faces: Vec<Face>,
}

impl Lines {
    /// A line's glyphs.
    pub fn glyphs_of(&self, line: &LayoutLine) -> &[LayoutGlyph] {
        &self.glyphs[line.glyphs.0 as usize..line.glyphs.1 as usize]
    }
    /// Exact accessible capacities, bytes.
    pub fn capacity_bytes(&self) -> usize {
        use std::mem::size_of;
        self.lines.capacity() * size_of::<LayoutLine>()
            + self.glyphs.capacity() * size_of::<LayoutGlyph>()
            + self.faces.capacity() * size_of::<Face>()
            + self
                .faces
                .iter()
                .map(|f| f.coords.len() * size_of::<i16>())
                .sum::<usize>()
    }
    /// The index of `face` among these lines' faces, added when new.
    pub(super) fn face(&mut self, face: Face) -> u16 {
        if let Some(i) = self
            .faces
            .iter()
            .position(|f| f.font == face.font && f.coords == face.coords && f.skew == face.skew)
        {
            return i as u16;
        }
        self.faces.push(face);
        (self.faces.len() - 1) as u16
    }
    /// Release spare capacity once the width is complete. Ordinary
    /// paragraphs keep a little spare; this is not a memory limit.
    pub(super) fn tighten(&mut self) {
        use std::mem::size_of;
        let spare = (self.glyphs.capacity() - self.glyphs.len()) * size_of::<LayoutGlyph>()
            + (self.lines.capacity() - self.lines.len()) * size_of::<LayoutLine>();
        if spare >= 4 * 1024 {
            self.glyphs.shrink_to_fit();
            self.lines.shrink_to_fit();
        }
        self.faces.shrink_to_fit();
    }
}

/// One Parley line's glyphs, appended in visual order with `shift` added
/// to every x (`styles` is the layout's, whose brushes are run indices).
/// Returns the line's summed cluster advance (justified where it is).
pub(super) fn extract(
    line: &parley::Line<'_, u32>,
    styles: &[parley::Style<u32>],
    shift: f32,
    out: &mut Lines,
) -> f32 {
    let m = line.metrics();
    let mut pen = m.offset + m.inline_min_coord + shift;
    let mut content = 0.0f32;
    for run in line.runs() {
        let synthesis = run.synthesis();
        let face = out.face(Face {
            font: run.font().clone(),
            coords: Arc::from(run.normalized_coords()),
            skew: synthesis.skew().is_some(),
            weight: run.font_attrs().weight.value().round().clamp(1.0, 1000.0) as u16,
        });
        let size = run.font_size();
        let level = run.bidi_level();
        for cluster in run.visual_clusters() {
            content += cluster.advance();
            let mut range = cluster.text_range();
            if cluster.is_ligature_start() {
                let mut next = cluster.next_logical();
                while let Some(c) = next.filter(|c| c.is_ligature_continuation()) {
                    range.end = range.end.max(c.text_range().end);
                    next = c.next_logical();
                }
            }
            for glyph in cluster.glyphs() {
                let style = styles.get(glyph.style_index()).map_or(0, |s| s.brush);
                out.glyphs.push(LayoutGlyph {
                    start: range.start as u32,
                    end: range.end as u32,
                    x: pen + glyph.x,
                    y: glyph.y,
                    w: glyph.advance,
                    font_size: size,
                    glyph_id: glyph.id,
                    metadata: style,
                    face,
                    level,
                });
                pen += glyph.advance;
            }
        }
    }
    content
}
