//! One immutable full shape; each width owns independent layout and ink.
use super::*;
use cosmic_text::{Hinting, LayoutLine, LayoutRun, ShapeBuffer, ShapeLine};

#[cfg(test)]
thread_local! { static SHAPE_LINES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(super) fn shape_line_calls() -> usize {
    SHAPE_LINES.with(std::cell::Cell::get)
}

struct Line {
    text: String,
    shape: ShapeLine,
    align: Option<Align>,
}
pub(super) struct ShapedSource {
    pub(super) spec: Arc<Spec>,
    pub(super) catalog: catalog::Lease,
    pub(super) data: Arc<ShapeData>,
    pub(super) accessible_capacity_bytes: usize,
}
// Only immutable arrays/scalars cross the thread boundary, never the UI lease.
pub(super) struct ShapeData {
    lines: Vec<Line>,
    metrics: Metrics,
    strut: (f32, f32),
    run_metrics: Vec<FontMetrics>,
}
impl ShapedSource {
    pub(super) fn new(lease: catalog::Lease, spec: Arc<Spec>) -> Self {
        let mut catalog = lease.borrow_mut();
        let minimum = catalog.line_height(&spec.strut);
        let (ascent, descent, leading) = catalog.font_metrics(&spec.strut);
        let half = (minimum - ascent - descent - leading) / 2.0;
        let strut = (ascent + half, descent + leading + half);
        let line_heights: Vec<f32> = spec.runs.iter().map(|r| catalog.line_height(r)).collect();
        let run_metrics: Vec<_> = spec.runs.iter().map(|r| catalog.font_metrics(r)).collect();
        let base = spec.runs.first().map_or(16.0, |r| r.size.max(0.5));
        let tallest = line_heights.iter().copied().fold(minimum, f32::max);
        let metrics = Metrics::new(base, tallest.max(f32::EPSILON));
        let mut buffer = Buffer::new_empty(metrics);
        let align = match spec.align {
            TextAlign::Left => None,
            TextAlign::Center => Some(Align::Center),
            TextAlign::Right => Some(Align::Right),
            TextAlign::Justify => Some(Align::Justified),
        };
        let weights: Vec<u16> = spec
            .runs
            .iter()
            .map(|r| catalog.snap_weight(r.family, r.weight, r.italic))
            .collect();
        let families: Vec<FamilyChoice> = spec
            .runs
            .iter()
            .map(|r| {
                catalog
                    .families
                    .get(r.family as usize)
                    .cloned()
                    .unwrap_or(FamilyChoice::SansSerif)
            })
            .collect();
        let spans: Vec<(&str, Attrs<'_>)> = spec
            .runs
            .iter()
            .zip(line_heights.iter())
            .zip(weights.iter())
            .zip(families.iter())
            .enumerate()
            .map(|(index, (((r, lh), w), family))| {
                (
                    r.text.as_str(),
                    // cosmic-text's scrolling loop requires positive pitch.
                    // This shaping-only pitch never escapes: CSS placement below
                    // uses the original lengths, including zero, by run metadata.
                    catalog::Catalog::attrs(r, *w, family.cosmic())
                        .metadata(index)
                        .metrics(Metrics::new(r.size.max(0.5), lh.max(1.0))),
                )
            })
            .collect();
        let default = spec
            .runs
            .first()
            .zip(weights.first())
            .zip(families.first())
            .map(|((r, w), family)| catalog::Catalog::attrs(r, *w, family.cosmic()))
            .unwrap_or_else(Attrs::new);
        buffer.set_rich_text(spans, &default, Shaping::Advanced, align);
        let lines = buffer
            .lines
            .into_iter()
            .map(|line| {
                #[cfg(test)]
                SHAPE_LINES.with(|n| n.set(n.get() + 1));
                let shape = ShapeLine::new(
                    &mut catalog.fonts,
                    line.text(),
                    line.attrs_list(),
                    Shaping::Advanced,
                    8,
                );
                let align = line.align();
                Line {
                    text: line.into_text(),
                    shape,
                    align,
                }
            })
            .collect();
        drop(catalog);
        let mut source = Self {
            spec,
            catalog: lease,
            data: Arc::new(ShapeData {
                lines,
                metrics,
                strut,
                run_metrics,
            }),
            accessible_capacity_bytes: 0,
        };
        source.accessible_capacity_bytes = source.capacities();
        source
    }
    pub(super) fn attach(
        catalog: catalog::Lease,
        spec: Arc<Spec>,
        data: Arc<ShapeData>,
        bytes: usize,
    ) -> Self {
        Self {
            spec,
            catalog,
            data,
            accessible_capacity_bytes: bytes,
        }
    }
    fn capacities(&self) -> usize {
        fn vec<T>(v: &Vec<T>) -> usize {
            v.capacity() * std::mem::size_of::<T>()
        }
        let mut bytes = vec(&self.data.lines) + vec(&self.data.run_metrics);
        for line in &self.data.lines {
            bytes += line.text.capacity() + vec(&line.shape.spans);
            for span in &line.shape.spans {
                bytes += vec(&span.words) + vec(&span.decoration_spans);
                for word in &span.words {
                    bytes += vec(&word.glyphs);
                }
            }
        }
        bytes
    }
    pub(super) fn layout(self: &Rc<Self>, width: Option<f32>, wrap: Option<Wrap>) -> Paragraph {
        let spec = &self.spec;
        let wrap = wrap.unwrap_or(
            if spec.overflow_wrap == exact_kernel::OverflowWrap::Normal {
                Wrap::Word
            } else {
                Wrap::WordOrGlyph
            },
        );
        let ellipsize = if spec.line_clamp > 0 {
            Ellipsize::End(EllipsizeHeightLimit::Lines(spec.line_clamp as usize))
        } else {
            Ellipsize::None
        };
        let layouts = {
            let mut scratch = ShapeBuffer::default();
            self.data
                .lines
                .iter()
                .map(|line| {
                    let mut output = Vec::new();
                    line.shape.layout_to_buffer(
                        &mut scratch,
                        self.data.metrics.font_size,
                        width.map(|w| w.max(0.)),
                        wrap,
                        ellipsize,
                        line.align,
                        &mut output,
                        None,
                        Hinting::Disabled,
                    );
                    output
                })
                .collect()
        }; // Request scratch dies before publication, never accumulates by width.
        let mut paragraph = Paragraph {
            source: self.clone(),
            layouts,
            #[cfg(test)]
            layout_lifetime: Arc::new(()),
            width: 0.,
            height: 0.,
            first_baseline: 0.,
            baselines: Vec::new(),
            ink: RefCell::new(ink::Cache::default()),
            resident_capacity_bytes: 0,
            private_text_bytes_estimate: 0,
        };
        let mut catalog = self.catalog.borrow_mut();
        let strut = self.data.strut;
        let run_metrics = &self.data.run_metrics;
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        let mut baselines = Vec::new();
        let mut explicit = false;
        for run in paragraph.layout_runs() {
            w = w.max(run.line_w);
            let (mut above, mut below) = strut;
            let mut above_explicit = spec.strut.line_height.is_some();
            let mut below_explicit = above_explicit;
            for glyph in run.glyphs {
                if let Some(font) = catalog.fonts.get_font(glyph.font_id, glyph.font_weight) {
                    let m = font.metrics();
                    let scale = glyph.font_size / m.units_per_em as f32;
                    // Explicit lengths size the authored inline box; only
                    // normal expands to the actual fallback glyph font.
                    let (ascent, descent, leading) =
                        if spec.runs[glyph.metadata].line_height.is_some() {
                            run_metrics[glyph.metadata]
                        } else {
                            (m.ascent * scale, m.descent.abs() * scale, m.leading * scale)
                        };
                    let height = spec.runs[glyph.metadata]
                        .line_height
                        .unwrap_or(ascent + descent + leading);
                    let half = (height - ascent - descent - leading) / 2.0;
                    let run_explicit = spec.runs[glyph.metadata].line_height.is_some();
                    let (a, b) = (ascent + half, descent + leading + half);
                    if a > above {
                        above = a;
                        above_explicit = run_explicit;
                    } else if a == above {
                        above_explicit &= run_explicit;
                    }
                    if b > below {
                        below = b;
                        below_explicit = run_explicit;
                    } else if b == below {
                        below_explicit &= run_explicit;
                    }
                }
            }
            explicit |= above_explicit || below_explicit;
            baselines.push(h + above);
            h += above + below;
        }
        paragraph.width = w.ceil();
        paragraph.height = if explicit { h } else { h.ceil() };
        paragraph.first_baseline = baselines.first().copied().unwrap_or(0.);
        paragraph.baselines = baselines;
        paragraph.resident_capacity_bytes = cache::capacities(&paragraph);
        paragraph
    }
}

pub(super) struct Runs<'a> {
    source: &'a ShapedSource,
    layouts: &'a [Vec<LayoutLine>],
    line: usize,
    wrapped: usize,
    top: f32,
}
impl<'a> Runs<'a> {
    pub(super) fn new(source: &'a ShapedSource, layouts: &'a [Vec<LayoutLine>]) -> Self {
        Self {
            source,
            layouts,
            line: 0,
            wrapped: 0,
            top: 0.,
        }
    }
}
impl<'a> Iterator for Runs<'a> {
    type Item = LayoutRun<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(line) = self.source.data.lines.get(self.line) {
            while let Some(layout) = self.layouts[self.line].get(self.wrapped) {
                self.wrapped += 1;
                let line_height = layout
                    .line_height_opt
                    .unwrap_or(self.source.data.metrics.line_height);
                let line_top = self.top;
                let centering = (line_height - (layout.max_ascent + layout.max_descent)) / 2.;
                let line_y = line_top + centering + layout.max_ascent;
                self.top += line_height;
                if line_y + layout.max_descent < 0. {
                    continue;
                }
                return Some(LayoutRun {
                    line_i: self.line,
                    text: &line.text,
                    rtl: line.shape.rtl,
                    glyphs: &layout.glyphs,
                    decorations: &layout.decorations,
                    line_y,
                    line_top,
                    line_height,
                    line_w: layout.w,
                });
            }
            self.line += 1;
            self.wrapped = 0;
        }
        None
    }
}
