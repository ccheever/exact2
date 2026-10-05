//! @ref LLP 1043.000 §3 D5–D7 — retained shaping, logical ranges, visual glyphs.
//! Only the accepted painter owns the current flowed layout per leaf. Flow layouts
//! never enter the width cache; replacing that owner releases the previous frame.
//! Preparation is O(G log G); a frame adds O(G + F log G + sum(g_f log g_f)) to
//! textflow's documented band bound, with O(G + F + S) storage, never frame history.
use super::*;
use cosmic_text::LayoutLine;
use exact_textflow::{FlowOptions, FlowShape, Fragment, Options, Prepared};
use std::ops::Range;

pub(super) struct FlowLayout {
    pub fragments: Vec<Fragment>,
    pub line_height: f32,
    width: f32,
    shapes: Vec<FlowShape>,
    pub(super) incomplete: bool,
}
impl FlowLayout {
    pub fn capacity_bytes(&self) -> usize {
        self.fragments.capacity() * std::mem::size_of::<Fragment>()
            + self.shapes.capacity() * std::mem::size_of::<FlowShape>()
    }
}
struct Cluster {
    range: Range<usize>,
    line: usize,
    glyphs: Vec<LayoutGlyph>,
    width: f32,
}
pub(super) struct FlowSource {
    text: String,
    clusters: Vec<Cluster>,
    prepared: Prepared,
    baseline: f32,
    height: f32,
    ellipses: Vec<Vec<LayoutGlyph>>,
    hyphen: Vec<LayoutGlyph>,
}
impl FlowSource {
    pub(super) fn capacity_bytes(&self) -> usize {
        self.hyphen.capacity() * std::mem::size_of::<LayoutGlyph>()
            + self
                .ellipses
                .iter()
                .map(|g| g.capacity() * std::mem::size_of::<LayoutGlyph>())
                .sum::<usize>()
            + self.ellipses.capacity() * std::mem::size_of::<Vec<LayoutGlyph>>()
            + self.text.capacity()
            + self.prepared.capacity_bytes()
            + self.clusters.capacity() * std::mem::size_of::<Cluster>()
            + self
                .clusters
                .iter()
                .map(|c| c.glyphs.capacity() * std::mem::size_of::<LayoutGlyph>())
                .sum::<usize>()
    }

    // Synthetic ink uses the same run font and shaper as ordinary text.
    fn symbol(source: &Rc<ShapedSource>, text: &str, run: usize) -> Vec<LayoutGlyph> {
        let mut spec = (*source.spec).clone();
        spec.runs = vec![spec.runs[run].clone()];
        spec.runs[0].text = text.into();
        spec.line_clamp = 0;
        let shaped = Rc::new(ShapedSource::new(source.catalog.clone(), Arc::new(spec)));
        let p = shaped.layout(None, Some(Wrap::None));
        p.layout_runs()
            .flat_map(|r| r.glyphs.iter().cloned())
            .map(|mut g| {
                g.metadata = run;
                g
            })
            .collect()
    }

    fn new(source: &Rc<ShapedSource>) -> Self {
        // This is layout of the retained ShapeLines, NOT another shaping call.
        // It also supplies exactly the ordinary CSS strut/fallback-font baseline.
        let plain = source.layout(None, Some(Wrap::None));
        let text: String = source.spec.runs.iter().map(|r| r.text.as_str()).collect();
        let mut clusters: Vec<Cluster> = Vec::new();
        let mut offset = 0;
        let (baseline, height) = source.flow_box(plain.layout_runs().flat_map(|r| r.glyphs));
        for run in plain.layout_runs() {
            let start = offset + text[offset..].find(run.text).unwrap_or(0);
            let mut glyphs = run.glyphs.to_vec();
            // Stable sort preserves the shaper's order inside multi-glyph clusters.
            glyphs.sort_by_key(|g| g.start);
            for glyph in glyphs {
                let range = start + glyph.start..start + glyph.end;
                if let Some(last) = clusters.last_mut().filter(|c| c.range == range) {
                    last.width += glyph.w;
                    last.glyphs.push(glyph);
                } else {
                    clusters.push(Cluster {
                        range,
                        line: run.line_i,
                        width: glyph.w,
                        glyphs: vec![glyph],
                    });
                }
            }
            offset = start + run.text.len();
        }
        if !source.spec.white_space.model().preserves()
            && text.chars().any(|ch| {
                matches!(
                    ch,
                    '\t' | '\n' | '\r' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
                )
            })
        {
            let mut space_spec = (*source.spec).clone();
            space_spec.runs.truncate(1);
            space_spec.runs[0].text = " ".into();
            let space_source = Rc::new(ShapedSource::new(
                source.catalog.clone(),
                Arc::new(space_spec),
            ));
            let space = space_source.layout(None, Some(Wrap::None));
            let template: Vec<_> = space
                .layout_runs()
                .flat_map(|r| r.glyphs.iter().cloned())
                .collect();
            let width = template.iter().map(|g| g.w).sum();
            let whitespace = |ch| {
                matches!(
                    ch,
                    ' ' | '\t' | '\n' | '\r' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
                )
            };
            let spaces: Vec<_> = text
                .char_indices()
                .filter(|(_, ch)| whitespace(*ch))
                .map(|(at, ch)| {
                    let pos = clusters.partition_point(|c| c.range.start < at);
                    let line = clusters
                        .get(pos)
                        .or_else(|| clusters.last())
                        .map_or(0, |c| c.line);
                    Cluster {
                        range: at..at + ch.len_utf8(),
                        line,
                        glyphs: template.clone(),
                        width,
                    }
                })
                .collect();
            clusters.retain(|c| !text[c.range.clone()].chars().all(whitespace));
            clusters.extend(spaces);
            clusters.sort_by_key(|c| c.range.start);
        }

        let mut prefix = Vec::with_capacity(clusters.len() + 1);
        prefix.push(0.0_f64);
        for c in &mut clusters {
            let left = c.glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
            for g in &mut c.glyphs {
                g.x -= left;
            }
            prefix.push(prefix.last().unwrap() + c.width as f64);
        }
        let mut measure = |range: Range<usize>| {
            let a = clusters.partition_point(|c| c.range.start < range.start);
            let b = clusters.partition_point(|c| c.range.start < range.end);
            (prefix[b] - prefix[a]) as f32
        };
        let overflow_wrap = match source.spec.overflow_wrap {
            exact_kernel::OverflowWrap::Normal => exact_textflow::OverflowWrap::Normal,
            exact_kernel::OverflowWrap::BreakWord => exact_textflow::OverflowWrap::BreakWord,
            exact_kernel::OverflowWrap::Anywhere => exact_textflow::OverflowWrap::Anywhere,
        };
        let hyphen = if text.contains('\u{ad}') {
            Self::symbol(source, "-", 0)
        } else {
            Vec::new()
        };
        let prepared = Prepared::new(
            &text,
            Options {
                white_space: source.spec.white_space.model(),
                overflow_wrap,
                hyphen_advance: hyphen.iter().map(|g| g.w).sum(),
            },
            &mut measure,
        );
        let ellipses = if source.spec.line_clamp > 0 {
            (0..source.spec.runs.len())
                .map(|i| Self::symbol(source, "…", i))
                .collect()
        } else {
            Vec::new()
        };
        Self {
            hyphen,
            ellipses,
            text,
            clusters,
            prepared,
            baseline,
            height,
        }
    }
}

impl Paragraph {
    /// An incomplete flow was replaced by ordinary engine layout.
    pub fn flow_incomplete(&self) -> bool {
        self.flow.as_ref().is_some_and(|f| f.incomplete)
    }
    /// Flowed source ranges in logical order, including consumed whitespace.
    /// Empty for ordinary engine-wrapped paragraphs.
    pub fn fragments(&self) -> &[Fragment] {
        self.flow.as_ref().map_or(&[], |f| &f.fragments)
    }
    /// Fixed CSS line band for a flowed paragraph (zero for ordinary paragraphs).
    pub fn flow_line_height(&self) -> f32 {
        self.flow.as_ref().map_or(0.0, |f| f.line_height)
    }
}
impl TextEngine {
    /// Layout from the same full shape as ordinary text. The caller may hand back
    /// its current snapshot; no shape/frame history is retained by this method.
    /// Justify falls back to start; clamp counts bands, including obstructed ones.
    pub fn paragraph_flow(
        &mut self,
        spec: &Spec,
        width: f32,
        shapes: &[FlowShape],
        previous: Option<&Rc<Paragraph>>,
    ) -> Rc<Paragraph> {
        let key = self.paragraphs.identity(spec);
        self.flow_for(key, width, shapes, previous)
    }
    pub(crate) fn flow_identified(
        &mut self,
        stamp: &ParagraphStamp,
        width: f32,
        shapes: &[FlowShape],
        previous: Option<&Rc<Paragraph>>,
        build: impl FnOnce() -> Spec,
    ) -> Option<Rc<Paragraph>> {
        if shapes.is_empty() {
            return self.paragraph_identified(stamp, Some(width), build);
        }
        let (key, spec) = self.identified_spec(stamp, build);
        (!spec.is_empty()).then(|| self.flow_for(key, width, shapes, previous))
    }
    pub(super) fn flow_for(
        &mut self,
        key: (u64, u64),
        width: f32,
        shapes: &[FlowShape],
        previous: Option<&Rc<Paragraph>>,
    ) -> Rc<Paragraph> {
        if shapes.is_empty() {
            let spec = self.paragraphs.spec(key).unwrap();
            return self.paragraph_for(&spec, Some(width), key);
        }
        // Retain the source even if the cold index evicted its identity meanwhile.
        let source = if let Some(p) = previous.filter(|p| {
            Rc::ptr_eq(&self.catalog, &p.source.catalog)
                && self.paragraphs.spec(key).as_deref() == Some(&*p.source.spec)
        }) {
            p.source.clone()
        } else {
            self.source(key)
        };
        if let Some(p) = previous.filter(|p| {
            Rc::ptr_eq(&p.source, &source)
                && p.flow
                    .as_ref()
                    .is_some_and(|f| f.width.to_bits() == width.to_bits() && f.shapes == shapes)
        }) {
            return p.clone();
        }
        let started = Instant::now();
        if source.flow.borrow().is_none() {
            let prepared = Rc::new(FlowSource::new(&source));
            *source.flow.borrow_mut() = Some(prepared);
        }
        let data = source.flow.borrow().as_ref().unwrap().clone();
        self.layout_calls += 1;
        let options = FlowOptions {
            direction: if source.spec.direction == exact_kernel::Direction::Rtl {
                exact_textflow::Direction::Rtl
            } else {
                exact_textflow::Direction::Ltr
            },
            width,
            line_height: data.height,
            min_fragment: source.spec.strut.size * exact_textflow::MIN_FRAGMENT_EM,
            max_lines: source.spec.line_clamp,
        };
        let mut fragments = Vec::new();
        let walk_started = Instant::now();
        let result = exact_textflow::flow(&data.prepared, shapes, &options, &mut fragments);
        self.flow_walk += walk_started.elapsed();
        if !result.complete && !result.clamped {
            let mut ordinary = source.layout(Some(width), None);
            ordinary.flow = Some(FlowLayout {
                fragments: Vec::new(),
                line_height: data.height,
                width,
                shapes: shapes.to_vec(),
                incomplete: true,
            });
            ordinary.resident_capacity_bytes = cache::capacities(&ordinary);
            return Rc::new(ordinary);
        }
        let height = result.height;
        let mut layouts: Vec<Vec<LayoutLine>> = source.layout_line_slots();
        let mut baselines = Vec::new();
        let mut bottoms = Vec::new();
        let mut slots = Vec::new();
        let mut band = u32::MAX;
        let fragment_count = fragments.len();
        for (fragment_index, f) in fragments.iter_mut().enumerate() {
            if band != f.line {
                exact_textflow::intervals(
                    shapes,
                    f.y,
                    f.y + data.height,
                    width,
                    options.min_fragment,
                    &mut slots,
                );
                band = f.line;
            }
            let available = slots
                .iter()
                .find(|s| s.0 == f.x)
                .map_or(width, |s| s.1 - s.0);
            let a = data.clusters.partition_point(|c| c.range.start < f.start);
            let b = data.clusters.partition_point(|c| c.range.start < f.end);
            let mut visible = Vec::new();
            let mut pending_space = None;
            for c in &data.clusters[a..b] {
                let value = &data.text[c.range.clone()];
                if value.chars().all(|ch| {
                    matches!(
                        ch,
                        ' ' | '\t' | '\r' | '\n' | '\u{85}' | '\u{c}' | '\u{2028}' | '\u{2029}'
                    )
                }) {
                    if source.spec.white_space.model().preserves() {
                        if value.chars().all(|ch| matches!(ch, ' ' | '\t')) {
                            visible.push(c);
                        }
                        continue;
                    }
                    if !visible.is_empty() && pending_space.is_none() {
                        pending_space = Some(c);
                    }
                } else if !value.chars().all(|ch| {
                    matches!(
                        ch,
                        '\r' | '\n'
                            | '\u{85}'
                            | '\u{c}'
                            | '\u{2028}'
                            | '\u{2029}'
                            | '\u{200b}'
                            | '\u{ad}'
                    )
                }) {
                    if let Some(space) = pending_space.take() {
                        visible.push(space);
                    }
                    visible.push(c);
                }
            }
            // @ref LLP 1043.000 §3 D7 — reserve the run's real ellipsis ink.
            let mut suffix: Option<&[LayoutGlyph]> = f.hyphenated.then_some(&data.hyphen);
            if result.clamped && fragment_index + 1 == fragment_count {
                f.hyphenated = false;
                while visible
                    .last()
                    .is_some_and(|c| data.text[c.range.clone()].chars().all(char::is_whitespace))
                {
                    visible.pop();
                }
                let mut total: f32 = visible.iter().map(|c| c.width).sum();
                loop {
                    let run = visible
                        .last()
                        .and_then(|c| c.glyphs.first())
                        .map_or(0, |g| g.metadata);
                    let ink = &data.ellipses[run];
                    let advance: f32 = ink.iter().map(|g| g.w).sum();
                    if total + advance <= available || visible.is_empty() {
                        suffix = (advance <= available).then_some(ink);
                        f.width = total + if suffix.is_some() { advance } else { 0.0 };
                        break;
                    }
                    total -= visible.pop().unwrap().width;
                }
            }
            // UAX #9 L2 on whole clusters, preserving glyph order inside a cluster.
            let max = visible
                .iter()
                .map(|c| c.glyphs[0].level.number())
                .max()
                .unwrap_or(0);
            let min_odd = visible
                .iter()
                .map(|c| c.glyphs[0].level.number())
                .filter(|l| l % 2 == 1)
                .min()
                .unwrap_or(max + 1);
            for level in (min_odd..=max).rev() {
                let mut i = 0;
                while i < visible.len() {
                    if visible[i].glyphs[0].level.number() < level {
                        i += 1;
                        continue;
                    }
                    let start = i;
                    while i < visible.len() && visible[i].glyphs[0].level.number() >= level {
                        i += 1;
                    }
                    visible[start..i].reverse();
                }
            }
            let shift = match source.spec.align {
                TextAlign::Center => (available - f.width) / 2.0,
                TextAlign::Right => available - f.width,
                _ => 0.0,
            }
            .max(0.0);
            f.x += shift;
            let mut x = f.x;
            let mut glyphs = Vec::new();
            if source.spec.direction == exact_kernel::Direction::Rtl {
                if let Some(ink) = suffix {
                    for g in ink {
                        let mut g = g.clone();
                        g.x += x;
                        x += g.w;
                        glyphs.push(g);
                    }
                }
            }
            for c in visible {
                glyphs.extend(c.glyphs.iter().cloned().map(|mut g| {
                    g.x += x;
                    g
                }));
                x += c.width;
            }
            if source.spec.direction != exact_kernel::Direction::Rtl {
                if let Some(ink) = suffix {
                    for g in ink {
                        let mut g = g.clone();
                        g.x += x;
                        x += g.w;
                        glyphs.push(g);
                    }
                }
            }
            // Trailing hard breaks have no next cluster. Keep their empty
            // layouts after the final source line, never back in line zero.
            let line = data
                .clusters
                .get(a)
                .or_else(|| data.clusters.last())
                .map_or(0, |c| c.line);
            layouts[line].push(LayoutLine {
                w: f.width,
                max_ascent: data.baseline,
                max_descent: data.height - data.baseline,
                line_height_opt: Some(data.height),
                glyphs,
                decorations: Vec::new(),
            });
            baselines.push(f.y + data.baseline);
            bottoms.push(f.y + data.height);
        }
        let measured_width = fragments
            .iter()
            .map(|f| f.x + f.width)
            .fold(0.0_f32, f32::max)
            .ceil();
        let mut p = Paragraph {
            source,
            layouts: Arc::new(layouts),
            flow: Some(FlowLayout {
                fragments,
                line_height: data.height,
                width,
                shapes: shapes.to_vec(),
                incomplete: false,
            }),
            #[cfg(test)]
            layout_lifetime: Arc::new(()),
            width: measured_width,
            height,
            first_baseline: baselines.first().copied().unwrap_or(0.0),
            baselines: Arc::new(baselines),
            bottoms: Arc::new(bottoms),
            ellipsized: RefCell::new(None),
            ink: RefCell::new(ink::Cache::default()),
            resident_capacity_bytes: 0,
            private_text_bytes_estimate: 0,
        };
        p.resident_capacity_bytes = cache::capacities(&p);
        self.flowing += started.elapsed();
        Rc::new(p)
    }
}
