//! The platform's own text stack (LLP 1076): with `EXACT_TEXT=platform`, a
//! host that installs a [`Shaper`] (the Android Canvas host, over JNI to
//! Minikin, whose fonts the zygote has already loaded) measures with it
//! instead of cosmic-text's shaping over a fontdb catalog of every system
//! face. cosmic-text still finds the words and breaks the lines, so wrapping
//! follows the same rules: each word is a run of pseudo glyphs, one per
//! cluster of the line's text, carrying the platform's advances, and a
//! painter hands the platform each line fragment's text to draw with the
//! same stack ([`super::TextEngine::platform_fragments`]): what was measured
//! is what is drawn. Each pseudo glyph's "font" is a metrics entry (ascent,
//! descent, line gap over every font its segment used, fallbacks included),
//! so `line-height: normal` grows for a fallback face as it does in Rust.
use super::*;
use cosmic_text::{BufferLine, ShapeGlyph, ShapeLine, ShapeWord};
use shaping::RawFontMetrics;
use std::sync::OnceLock;

/// The glyph id of a platform paragraph's ellipsis: its text is `…`, not the line's.
pub(crate) const ELLIPSIS: u16 = u16::MAX;

/// A family the platform resolves: a generic, or a plan stack declared with
/// [`Shaper::declare`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    /// `sans-serif` (and `system-ui`).
    SansSerif,
    /// `serif`.
    Serif,
    /// `monospace`.
    Monospace,
    /// A plan stack's declared faces.
    Declared(u16),
}

impl Family {
    /// 0 sans-serif, 1 serif, 2 monospace, 3 + stack for a declared one.
    pub fn code(self) -> i32 {
        match self {
            Family::SansSerif => 0,
            Family::Serif => 1,
            Family::Monospace => 2,
            Family::Declared(stack) => 3 + i32::from(stack),
        }
    }
}

/// What a run asks the platform's font matching for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// The family.
    pub family: Family,
    /// CSS 100–900.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// Letter spacing in ems.
    pub letter_spacing: f32,
    /// `font-variant-numeric: tabular-nums` (the face's `tnum`).
    pub tabular: bool,
}

/// One styled run of text measured in its own context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// First UTF-16 unit.
    pub start: u32,
    /// UTF-16 units.
    pub len: u32,
    /// Right-to-left.
    pub rtl: bool,
    /// The text may fall back past the style's first font: its metrics are
    /// measured over the fonts it used. Otherwise they are the first font's
    /// ([`Shaper::font_metrics`]), and `measure` leaves them.
    pub fallback: bool,
    /// A key [`Shaper::style`] gave.
    pub style: u32,
    /// Points.
    pub size: f32,
}

/// The platform's text measurement. Calls come from whichever thread runs
/// the host; an implementation reaches the platform from each.
pub trait Shaper: Send + Sync {
    /// Make plan stack `stack` a family of these faces (file, weight,
    /// italic), falling back to the system's; whether it could.
    fn declare(&self, stack: u16, faces: &[(String, u16, bool)]) -> bool;
    /// A key for `style`, the same for the process's life.
    fn style(&self, style: &Style) -> u32;
    /// Each segment's advances into `advances` (one per UTF-16 unit of
    /// `chars`, in points; a cluster's whole advance on its first unit,
    /// zero on the rest), and, for a segment that may fall back, its
    /// ascent, descent and line gap (points, positive) over every font it
    /// used into `metrics[3 * i..]`.
    fn measure(
        &self,
        chars: &[u16],
        segments: &[Segment],
        advances: &mut [f32],
        metrics: &mut [f32],
    );
    /// The ascent, descent and line gap of `style`'s first font at `size`.
    fn font_metrics(&self, style: u32, size: f32) -> [f32; 3];
}

static SHAPER: OnceLock<Box<dyn Shaper>> = OnceLock::new();

/// Install the process's shaper (once; a later one is ignored).
pub fn install(shaper: Box<dyn Shaper>) {
    let _ = SHAPER.set(shaper);
}

/// Whether `EXACT_TEXT=platform` asks for the platform's text stack.
pub fn requested() -> bool {
    std::env::var("EXACT_TEXT").is_ok_and(|v| v == "platform")
}

/// The installed shaper, when `EXACT_TEXT=platform` asks for it.
pub(super) fn chosen() -> Option<&'static dyn Shaper> {
    if requested() {
        SHAPER.get().map(|s| &**s)
    } else {
        None
    }
}

/// A catalog's platform state: the shaper, the keys of the styles and the
/// metrics entries it has seen.
#[derive(Clone)]
pub(super) struct Platform {
    pub(super) shaper: &'static dyn Shaper,
    styles: HashMap<(u16, u16, bool, u32, bool), u32>,
    declared: std::collections::HashSet<u16>,
    ids: fontdb::Database,
    by_metrics: HashMap<[u32; 3], fontdb::ID>,
    metrics: HashMap<fontdb::ID, RawFontMetrics>,
    primary: HashMap<(u32, u32), FontMetrics>,
}

impl Platform {
    pub(super) fn new(shaper: &'static dyn Shaper) -> Self {
        Self {
            shaper,
            styles: HashMap::new(),
            declared: Default::default(),
            ids: fontdb::Database::new(),
            by_metrics: HashMap::new(),
            metrics: HashMap::new(),
            primary: HashMap::new(),
        }
    }

    /// Declare plan stack `stack`'s faces; whether the platform took them.
    pub(super) fn declare(&mut self, stack: u16, faces: &[(String, u16, bool)]) -> bool {
        let ok = self.shaper.declare(stack, faces);
        if ok {
            self.declared.insert(stack);
        }
        ok
    }

    /// The style key for a run.
    pub(super) fn style(&mut self, families: &[FamilyChoice], run: &Run) -> u32 {
        let tabular = run.font_variant_numeric & 1 != 0;
        let spacing = if run.size > 0.0 {
            run.letter_spacing / run.size
        } else {
            0.0
        };
        let key = (
            run.family,
            run.weight,
            run.italic,
            spacing.to_bits(),
            tabular,
        );
        if let Some(k) = self.styles.get(&key) {
            return *k;
        }
        let family = if self.declared.contains(&run.family) {
            Family::Declared(run.family)
        } else {
            match families.get(run.family as usize) {
                Some(FamilyChoice::Serif) => Family::Serif,
                Some(FamilyChoice::Monospace) => Family::Monospace,
                _ => Family::SansSerif,
            }
        };
        let k = self.shaper.style(&Style {
            family,
            weight: run.weight,
            italic: run.italic,
            letter_spacing: spacing,
            tabular,
        });
        self.styles.insert(key, k);
        k
    }

    /// The run's first font's ascent, descent and line gap.
    pub(super) fn font_metrics(&mut self, families: &[FamilyChoice], run: &Run) -> FontMetrics {
        let style = self.style(families, run);
        let size = run.size.max(0.5);
        *self
            .primary
            .entry((style, size.to_bits()))
            .or_insert_with(|| {
                let [a, d, l] = self.shaper.font_metrics(style, size);
                (a, d, l)
            })
    }

    /// The metrics entry standing for a segment's fonts, per em.
    fn metrics_id(&mut self, per_em: [f32; 3]) -> fontdb::ID {
        let key = per_em.map(f32::to_bits);
        if let Some(id) = self.by_metrics.get(&key) {
            return *id;
        }
        let id = self.ids.push_face_info(fontdb::FaceInfo {
            id: fontdb::ID::dummy(),
            source: fontdb::Source::Binary(Arc::new(Vec::<u8>::new())),
            index: 0,
            families: Vec::new(),
            post_script_name: String::new(),
            style: fontdb::Style::Normal,
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            monospaced: false,
        });
        self.by_metrics.insert(key, id);
        self.metrics.insert(
            id,
            RawFontMetrics {
                units_per_em: 1,
                ascent: per_em[0],
                descent: per_em[1],
                leading: per_em[2],
            },
        );
        id
    }

    /// A pseudo glyph's font metrics (per em: `units_per_em` is 1).
    pub(super) fn raw_metrics(&self, id: fontdb::ID) -> Option<RawFontMetrics> {
        self.metrics.get(&id).copied()
    }
}

/// A declared face's file: the asset's own, or (an update's bytes) a copy
/// in `$HOME/.exact-fonts/` named by their hash.
pub(super) fn asset_file(assets: &Assets, name: &str) -> Option<String> {
    if let Some(path) = assets.path(name) {
        return Some(path.to_string_lossy().into_owned());
    }
    let bytes = assets.read(name)?;
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    });
    let dir = std::path::Path::new(&std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
        .join(".exact-fonts");
    let path = dir.join(format!("{hash:016x}.ttf"));
    let ok = path.exists()
        || (std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&path, &bytes).is_ok());
    ok.then(|| path.to_string_lossy().into_owned())
}

/// The run indices (`Attrs::metadata`) and sizes a line's text is styled with.
fn run_at(line: &BufferLine, byte: usize) -> usize {
    line.attrs_list().get_span(byte).metadata
}

/// Shape a paragraph's lines with the platform: one call measures every
/// styled segment of every line and the ellipsis. Returns the shaped lines
/// and each spec run's style key.
pub(super) fn shape(
    catalog: &mut catalog::Catalog,
    spec: &Spec,
    lines: &[BufferLine],
) -> (Vec<ShapeLine>, Vec<u32>) {
    let families = catalog.families.clone();
    let platform = catalog.platform.as_mut().expect("platform catalog");
    let declared = platform.declared.clone();
    let styles: Vec<u32> = spec
        .runs
        .iter()
        .map(|r| platform.style(&families, r))
        .collect();
    let size_of = |run: usize| spec.runs.get(run).map_or(16.0, |r| r.size.max(0.5));
    let base = (spec.direction == exact_kernel::Direction::Rtl).then_some(true);
    let mut chars: Vec<u16> = Vec::new();
    let mut segments: Vec<Segment> = Vec::new();
    // Per segment: its run. Per UTF-16 unit: its segment.
    let mut runs: Vec<usize> = Vec::new();
    let mut unit_segment: Vec<u32> = Vec::new();
    // Per line: direction, level runs and words, first unit, unit of each byte.
    let mut planned = Vec::with_capacity(lines.len());
    for line in lines {
        let text = line.text();
        let (rtl, spans) = ShapeLine::segment(text, base);
        let base16 = chars.len();
        let mut units = vec![0u32; text.len() + 1];
        let mut buf = [0u16; 2];
        for (b, c) in text.char_indices() {
            units[b] = (chars.len() - base16) as u32;
            chars.extend_from_slice(c.encode_utf16(&mut buf));
        }
        units[text.len()] = (chars.len() - base16) as u32;
        unit_segment.resize(chars.len(), 0);
        for (level, words) in &spans {
            let (Some(first), Some(last)) = (words.first(), words.last()) else {
                continue;
            };
            let mut start = first.0.start;
            let mut run = run_at(line, start);
            let mut push = |from: usize, to: usize, run: usize| {
                let (a, b) = (base16 + units[from] as usize, base16 + units[to] as usize);
                if a >= b {
                    return;
                }
                for u in &mut unit_segment[a..b] {
                    *u = segments.len() as u32;
                }
                // Latin, Greek, Cyrillic and common punctuation are the
                // generic families' own (Roboto's); anything else, or a
                // declared family, may fall back.
                let fallback = declared.contains(&spec.runs.get(run).map_or(0, |r| r.family))
                    || chars[a..b].iter().any(|&c| {
                        !(c < 0x0530
                            || (0x1E00..0x2000).contains(&c)
                            || (0x2000..0x20D0).contains(&c))
                    });
                segments.push(Segment {
                    start: a as u32,
                    len: (b - a) as u32,
                    rtl: level.is_rtl(),
                    fallback,
                    style: styles.get(run).copied().unwrap_or(0),
                    size: size_of(run),
                });
                runs.push(run);
            };
            let from = start;
            for (b, _) in text[from..last.0.end].char_indices() {
                let at = from + b;
                let r = run_at(line, at);
                if r != run {
                    push(start, at, run);
                    start = at;
                    run = r;
                }
            }
            push(start, last.0.end, run);
        }
        planned.push((rtl, spans, base16, units));
    }
    // The ellipsis, in the first run's style (cosmic-text's choice).
    let ellipsis_at = chars.len();
    chars.push('\u{2026}' as u16);
    segments.push(Segment {
        start: ellipsis_at as u32,
        len: 1,
        rtl: false,
        fallback: true,
        style: styles.first().copied().unwrap_or(0),
        size: size_of(0),
    });
    runs.push(0);
    let mut advances = vec![0.0f32; chars.len()];
    let mut metrics = vec![0.0f32; 3 * segments.len()];
    platform
        .shaper
        .measure(&chars, &segments, &mut advances, &mut metrics);
    for (i, s) in segments.iter().enumerate() {
        if !s.fallback {
            let run = &spec.runs[runs[i]];
            let (a, d, l) = platform.font_metrics(&families, run);
            metrics[3 * i..3 * i + 3].copy_from_slice(&[a, d, l]);
        }
    }
    let ids: Vec<fontdb::ID> = segments
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let m = &metrics[3 * i..3 * i + 3];
            platform.metrics_id([m[0] / s.size, m[1] / s.size, m[2] / s.size])
        })
        .collect();
    let glyph =
        |run: usize, segment: usize, start: usize, end: usize, advance: f32, line: &BufferLine| {
            let segment_size = segments[segment].size;
            ShapeGlyph {
                start,
                end,
                x_advance: advance / segment_size,
                y_advance: 0.0,
                x_offset: 0.0,
                y_offset: 0.0,
                ascent: metrics[3 * segment] / segment_size,
                descent: metrics[3 * segment + 1] / segment_size,
                font_monospace_em_width: None,
                font_id: ids[segment],
                font_weight: Weight(spec.runs.get(run).map_or(400, |r| r.weight)),
                glyph_id: 0,
                color_opt: None,
                metadata: run,
                cache_key_flags: CacheKeyFlags::empty(),
                metrics_opt: line
                    .attrs_list()
                    .get_span(start)
                    .metrics_opt
                    .map(Into::into),
            }
        };
    let shaped = lines
        .iter()
        .zip(planned)
        .map(|(line, (rtl, spans, base16, units))| {
            let text = line.text();
            let spans = spans
                .into_iter()
                .map(|(level, words)| {
                    let words = words
                        .into_iter()
                        .map(|(range, blank)| {
                            let mut glyphs: Vec<ShapeGlyph> = Vec::new();
                            for (b, c) in text[range.clone()].char_indices() {
                                let at = range.start + b;
                                let u = base16 + units[at] as usize;
                                let n = c.len_utf16();
                                let advance: f32 = advances[u..u + n].iter().sum();
                                let segment = unit_segment[u] as usize;
                                let run = runs[segment];
                                // A zero advance continues the cluster before it
                                // (a mark, a ligature's tail, an emoji sequence).
                                if let Some(last) = glyphs.last_mut() {
                                    if advance == 0.0 && last.metadata == run {
                                        last.end = at + c.len_utf8();
                                        continue;
                                    }
                                }
                                glyphs.push(glyph(
                                    run,
                                    segment,
                                    at,
                                    at + c.len_utf8(),
                                    advance,
                                    line,
                                ));
                            }
                            // A shaper's output order: visual for a right-to-left level.
                            if level.is_rtl() {
                                glyphs.reverse();
                            }
                            ShapeWord { blank, glyphs }
                        })
                        .collect();
                    (level, words)
                })
                .collect();
            let last = segments.len() - 1;
            let mut ellipsis = glyph(
                0,
                last,
                0,
                '\u{2026}'.len_utf8(),
                advances[ellipsis_at],
                line,
            );
            ellipsis.glyph_id = ELLIPSIS;
            ellipsis.metrics_opt = line.attrs_list().get_span(0).metrics_opt.map(Into::into);
            ShapeLine::from_words(
                text,
                rtl,
                spans,
                line.attrs_list().defaults().metrics_opt.map(Into::into),
                vec![ellipsis],
                8,
            )
        })
        .collect();
    (shaped, styles)
}
