//! One immutable full shape; each width owns independent layout and ink.
use super::*;
use cosmic_text::{Hinting, LayoutLine, LayoutRun, ShapeBuffer, ShapeLine};

#[cfg(test)]
thread_local! { static SHAPE_LINES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(super) fn shape_line_calls() -> usize {
    SHAPE_LINES.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! { static WIDTH_FONT_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(super) fn width_font_lookups() -> usize {
    WIDTH_FONT_LOOKUPS.with(std::cell::Cell::get)
}

// One width-pass slot, never retained by a shape, paragraph or catalog.
// Unscaled scalars only: glyph size/metadata and all CSS arithmetic stay below.
#[derive(Clone, Copy, Debug)]
pub(super) struct RawFontMetrics {
    pub(super) units_per_em: u16,
    pub(super) ascent: f32,
    pub(super) descent: f32,
    pub(super) leading: f32,
}
#[derive(Default)]
pub(super) struct LastFontMetrics {
    last: Option<((fontdb::ID, Weight), Option<RawFontMetrics>)>,
}
impl LastFontMetrics {
    pub(super) fn get(
        &mut self,
        fonts: &mut FontSystem,
        id: fontdb::ID,
        weight: Weight,
    ) -> Option<RawFontMetrics> {
        if let Some((key, metrics)) = self.last {
            if key == (id, weight) {
                return metrics;
            }
        }
        #[cfg(test)]
        WIDTH_FONT_LOOKUPS.with(|n| n.set(n.get() + 1));
        let metrics = fonts.get_font(id, weight).map(|font| {
            let m = font.metrics();
            RawFontMetrics {
                units_per_em: m.units_per_em,
                ascent: m.ascent,
                descent: m.descent,
                leading: m.leading,
            }
        });
        self.last = Some(((id, weight), metrics));
        metrics
    }
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
    pub(super) flow: RefCell<Option<Rc<super::flow::FlowSource>>>,
}
// Only immutable arrays/scalars cross the thread boundary, never the UI lease.
pub(super) struct ShapeData {
    lines: Vec<Line>,
    metrics: Metrics,
    strut: (f32, f32),
    pub(super) run_metrics: Vec<FontMetrics>,
    /// Each line's `(first, rest)` inset, points: a Markdown list item's
    /// head indent, its first line's less the marker hung before it (LLP
    /// 1045 D4). Empty when no run has an indent.
    insets: Vec<(f32, f32)>,
}

/// Each hard line's inset: a line takes the indent of the run its first
/// character (or its newline) is in, and when that run is a hung marker,
/// the marker's shaped advance comes off the first line's.
fn line_insets(spec: &Spec, lines: &[Line], font_size: f32) -> Vec<(f32, f32)> {
    if spec.runs.iter().all(|r| r.indent == 0.0) {
        return Vec::new();
    }
    let mut first_runs = Vec::with_capacity(lines.len());
    let mut open = true;
    for (index, run) in spec.runs.iter().enumerate() {
        for byte in run.text.bytes() {
            if std::mem::take(&mut open) {
                first_runs.push(index);
            }
            open = byte == b'\n';
        }
    }
    lines
        .iter()
        .enumerate()
        .map(|(line, shaped)| {
            let Some(&index) = first_runs.get(line) else {
                return (0.0, 0.0);
            };
            let run = &spec.runs[index];
            let marker: f32 = if run.hang {
                shaped
                    .shape
                    .spans
                    .iter()
                    .flat_map(|span| &span.words)
                    .flat_map(|word| &word.glyphs)
                    .filter(|glyph| glyph.metadata == index)
                    .map(|glyph| glyph.width(font_size))
                    .sum()
            } else {
                0.0
            };
            (run.indent - marker, run.indent)
        })
        .collect()
}
impl ShapedSource {
    pub(super) fn flow_box<'a>(&self, glyphs: impl Iterator<Item = &'a LayoutGlyph>) -> (f32, f32) {
        let (above, below, _) = line_box(
            glyphs,
            &self.spec,
            &mut self.catalog.borrow_mut(),
            self.data.strut,
            &self.data.run_metrics,
        );
        (above, above + below)
    }
    pub(super) fn flow_capacity_bytes(&self) -> usize {
        self.flow
            .borrow()
            .as_ref()
            .map_or(0, |f| f.capacity_bytes())
    }

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
        let align = match spec.align.physical(spec.direction) {
            // Physical: cosmic-text's `None` would align by each line's own bidi
            // direction, not the paragraph's CSS `direction` (LLP 1053).
            TextAlign::Left | TextAlign::Start => Some(Align::Left),
            TextAlign::Center => Some(Align::Center),
            TextAlign::Right | TextAlign::End => Some(Align::Right),
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
        let lines: Vec<Line> = buffer
            .lines
            .into_iter()
            .map(|line| {
                #[cfg(test)]
                SHAPE_LINES.with(|n| n.set(n.get() + 1));
                // `direction: rtl` makes the base direction right-to-left, as
                // CSS does (LLP 1053; vendor/cosmic-text/EXACT-PATCHES.md).
                // Under `ltr` the first strong character still decides
                // (declared in LLP 1001 §1): the text-flow walker cannot yet
                // break an RTL run inside an LTR paragraph.
                let shape = ShapeLine::new_with_base(
                    &mut catalog.fonts,
                    line.text(),
                    line.attrs_list(),
                    Shaping::Advanced,
                    8,
                    (spec.direction == exact_kernel::Direction::Rtl).then_some(true),
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
        let insets = line_insets(&spec, &lines, metrics.font_size);
        let mut source = Self {
            spec,
            catalog: lease,
            data: Arc::new(ShapeData {
                lines,
                metrics,
                strut,
                run_metrics,
                insets,
            }),
            accessible_capacity_bytes: 0,
            flow: RefCell::new(None),
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
            flow: RefCell::new(None),
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
    pub(super) fn layout_line_slots(&self) -> Vec<Vec<LayoutLine>> {
        self.data.lines.iter().map(|_| Vec::new()).collect()
    }
    pub(super) fn layout(self: &Rc<Self>, width: Option<f32>, wrap: Option<Wrap>) -> Paragraph {
        self.layout_as(width, wrap, false)
    }
    /// CSS `text-overflow: ellipsis`: each over-wide line ends in "…" (paint).
    pub(super) fn layout_ellipsized(self: &Rc<Self>, width: f32) -> Paragraph {
        self.layout_as(Some(width), None, true)
    }
    fn layout_as(
        self: &Rc<Self>,
        width: Option<f32>,
        wrap: Option<Wrap>,
        ellipsis: bool,
    ) -> Paragraph {
        let spec = &self.spec;
        let wrap = if !spec.white_space.model().wraps() {
            Wrap::None
        } else {
            wrap.unwrap_or(
                if spec.overflow_wrap == exact_kernel::OverflowWrap::Normal {
                    Wrap::Word
                } else {
                    Wrap::WordOrGlyph
                },
            )
        };
        let ellipsize = if spec.line_clamp > 0 {
            Ellipsize::End(EllipsizeHeightLimit::Lines(spec.line_clamp as usize))
        } else if ellipsis && wrap == Wrap::None {
            Ellipsize::End(EllipsizeHeightLimit::Lines(1))
        } else {
            Ellipsize::None
        };
        let mut layouts: Vec<Vec<LayoutLine>> = {
            let mut scratch = ShapeBuffer::default();
            self.data
                .lines
                .iter()
                .enumerate()
                .map(|(index, line)| {
                    let mut output = Vec::new();
                    // A list item's lines are laid out in what its indent
                    // leaves, then moved over by it; its first line's own
                    // inset (less a hung marker) is cosmic-text's indent.
                    let (first, rest) = self.data.insets.get(index).copied().unwrap_or_default();
                    // CSS `text-indent`: the paragraph's first line only.
                    let indent = if index == 0 { spec.text_indent } else { 0.0 };
                    line.shape.layout_to_buffer_indented(
                        &mut scratch,
                        self.data.metrics.font_size,
                        width.map(|w| (w - rest).max(0.)),
                        wrap,
                        ellipsize,
                        line.align,
                        &mut output,
                        None,
                        Hinting::Disabled,
                        indent + first - rest,
                    );
                    if rest != 0.0 {
                        // From the start edge: `rtl` lays out from the right.
                        let shift = if spec.direction == exact_kernel::Direction::Rtl {
                            0.0
                        } else {
                            rest
                        };
                        for laid in &mut output {
                            laid.w += rest;
                            for glyph in &mut laid.glyphs {
                                glyph.x += shift;
                            }
                        }
                    }
                    output
                })
                .collect()
        }; // Request scratch dies before publication, never accumulates by width.
           // Ordinary paragraphs avoid shrinking allocations for small savings.
           // This is an optimization threshold, not admission or a memory limit.
        let spare = layouts.iter().flatten().fold(0usize, |bytes, line| {
            bytes.saturating_add(
                (line.glyphs.capacity() - line.glyphs.len())
                    .saturating_mul(std::mem::size_of::<cosmic_text::LayoutGlyph>()),
            )
        });
        if spare >= 64 * 1024 {
            for line in layouts.iter_mut().flatten() {
                if line.glyphs.capacity() > line.glyphs.len() {
                    // These vectors are still private. Preserve every glyph
                    // and the public Vec/slice API before Arc/index creation.
                    // A moving shrink may need old + one new line allocation;
                    // an unwrapped giant line is not bounded by the viewport.
                    // Tight capacity does not guarantee allocator/AS release.
                    line.glyphs = std::mem::take(&mut line.glyphs)
                        .into_boxed_slice()
                        .into_vec();
                }
            }
        }
        let mut paragraph = Paragraph {
            source: self.clone(),
            layouts: Arc::new(layouts),
            flow: None,
            #[cfg(test)]
            layout_lifetime: Arc::new(()),
            width: 0.,
            height: 0.,
            first_baseline: 0.,
            baselines: Arc::new(Vec::new()),
            bottoms: Arc::new(Vec::new()),
            ink: RefCell::new(ink::Cache::default()),
            ellipsized: RefCell::new(None),
            resident_capacity_bytes: 0,
            private_text_bytes_estimate: 0,
        };
        let mut catalog = self.catalog.borrow_mut();
        let strut = self.data.strut;
        let run_metrics = &self.data.run_metrics;
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        let mut baselines = Vec::new();
        let mut bottoms = Vec::new();
        let mut explicit = false;
        let mut last_font_metrics = LastFontMetrics::default();
        for run in paragraph.layout_runs() {
            w = w.max(run.line_w);
            let (mut above, mut below) = strut;
            let mut above_explicit = spec.strut.line_height.is_some();
            let mut below_explicit = above_explicit;
            for glyph in run.glyphs {
                if let Some(m) =
                    last_font_metrics.get(&mut catalog.fonts, glyph.font_id, glyph.font_weight)
                {
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
            bottoms.push(h);
        }
        paragraph.width = w.ceil();
        paragraph.height = if explicit { h } else { h.ceil() };
        paragraph.first_baseline = baselines.first().copied().unwrap_or(0.);
        paragraph.baselines = Arc::new(baselines);
        paragraph.bottoms = Arc::new(bottoms);
        paragraph.resident_capacity_bytes = cache::capacities(&paragraph);
        paragraph
    }
    #[cfg(test)]
    pub(super) fn layout_reference(
        self: &Rc<Self>,
        width: Option<f32>,
        wrap: Option<Wrap>,
    ) -> Paragraph {
        let spec = &self.spec;
        let wrap = if !spec.white_space.model().wraps() {
            Wrap::None
        } else {
            wrap.unwrap_or(
                if spec.overflow_wrap == exact_kernel::OverflowWrap::Normal {
                    Wrap::Word
                } else {
                    Wrap::WordOrGlyph
                },
            )
        };
        let ellipsize = if spec.line_clamp > 0 {
            Ellipsize::End(EllipsizeHeightLimit::Lines(spec.line_clamp as usize))
        } else {
            Ellipsize::None
        };
        let mut layouts: Vec<Vec<LayoutLine>> = {
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
           // Ordinary paragraphs avoid shrinking allocations for small savings.
           // This is an optimization threshold, not admission or a memory limit.
        let spare = layouts.iter().flatten().fold(0usize, |bytes, line| {
            bytes.saturating_add(
                (line.glyphs.capacity() - line.glyphs.len())
                    .saturating_mul(std::mem::size_of::<cosmic_text::LayoutGlyph>()),
            )
        });
        if spare >= 64 * 1024 {
            for line in layouts.iter_mut().flatten() {
                if line.glyphs.capacity() > line.glyphs.len() {
                    // These vectors are still private. Preserve every glyph
                    // and the public Vec/slice API before Arc/index creation.
                    // A moving shrink may need old + one new line allocation;
                    // an unwrapped giant line is not bounded by the viewport.
                    // Tight capacity does not guarantee allocator/AS release.
                    line.glyphs = std::mem::take(&mut line.glyphs)
                        .into_boxed_slice()
                        .into_vec();
                }
            }
        }
        let mut paragraph = Paragraph {
            source: self.clone(),
            layouts: Arc::new(layouts),
            flow: None,
            #[cfg(test)]
            layout_lifetime: Arc::new(()),
            width: 0.,
            height: 0.,
            first_baseline: 0.,
            baselines: Arc::new(Vec::new()),
            bottoms: Arc::new(Vec::new()),
            ink: RefCell::new(ink::Cache::default()),
            ellipsized: RefCell::new(None),
            resident_capacity_bytes: 0,
            private_text_bytes_estimate: 0,
        };
        let mut catalog = self.catalog.borrow_mut();
        let strut = self.data.strut;
        let run_metrics = &self.data.run_metrics;
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        let mut baselines = Vec::new();
        let mut bottoms = Vec::new();
        let mut explicit = false;
        for run in paragraph.layout_runs() {
            w = w.max(run.line_w);
            let (above, below, is_explicit) =
                line_box(run.glyphs.iter(), spec, &mut catalog, strut, run_metrics);
            explicit |= is_explicit;
            baselines.push(h + above);
            h += above + below;
            bottoms.push(h);
        }
        paragraph.width = w.ceil();
        paragraph.height = if explicit { h } else { h.ceil() };
        paragraph.first_baseline = baselines.first().copied().unwrap_or(0.);
        paragraph.baselines = Arc::new(baselines);
        paragraph.bottoms = Arc::new(bottoms);
        paragraph.resident_capacity_bytes = cache::capacities(&paragraph);
        paragraph
    }
}

fn line_box<'a>(
    glyphs: impl Iterator<Item = &'a LayoutGlyph>,
    spec: &Spec,
    catalog: &mut catalog::Catalog,
    strut: (f32, f32),
    run_metrics: &[FontMetrics],
) -> (f32, f32, bool) {
    let (mut above, mut below) = strut;
    let mut above_explicit = spec.strut.line_height.is_some();
    let mut below_explicit = above_explicit;
    for glyph in glyphs {
        if let Some(font) = catalog.fonts.get_font(glyph.font_id, glyph.font_weight) {
            let m = font.metrics();
            let scale = glyph.font_size / m.units_per_em as f32;
            // Explicit lengths size the authored inline box; only
            // normal expands to the actual fallback glyph font.
            let (ascent, descent, leading) = if spec.runs[glyph.metadata].line_height.is_some() {
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
    (above, below, above_explicit || below_explicit)
}

pub(super) struct Runs<'a> {
    paragraph: &'a Paragraph,
    source: &'a ShapedSource,
    layouts: &'a [Vec<LayoutLine>],
    line: usize,
    wrapped: usize,
    top: f32,
    index: usize,
}
impl<'a> Runs<'a> {
    pub(super) fn new(paragraph: &'a Paragraph) -> Self {
        Self {
            paragraph,
            source: &paragraph.source,
            layouts: &paragraph.layouts,
            line: 0,
            wrapped: 0,
            top: 0.,
            index: 0,
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
                let (line_top, line_y, line_height) =
                    if let Some(flow) = self.paragraph.flow.as_ref().filter(|f| !f.incomplete) {
                        (
                            flow.fragments[self.index].y,
                            self.paragraph.baselines[self.index],
                            flow.line_height,
                        )
                    } else {
                        (line_top, line_y, line_height)
                    };
                self.index += 1;
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
