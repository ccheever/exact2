//! Browser measurement into the shared walker; retained by paragraph id.
//! @ref LLP 1043.000 §3 D1, D6, D7 — shape math and breaks stay in wasm.
//!
//! `textflow_request(op, id, len)` uses the optional artifact's own input
//! and JSON output buffers. All numeric input is little endian:
//! 0 segments: overflow-wrap u32 (0 normal, 1 break-word, 2 anywhere),
//! white-space u32 (0 normal, 1 pre-wrap, 2 nowrap, 3 pre-line), word count u32, that many
//! ascending UTF-16 word boundaries u32 (`Intl.Segmenter`'s between two Thai,
//! Lao, Khmer or Myanmar letters; others are dropped), UTF-8;
//! 1 prepare: hyphen f32, then one f32 per returned measurement range;
//! 2 resolve + flow: width/line-height/font-size f32, max-lines u32,
//! paragraph-height f32, direction u32 (0 ltr, 1 rtl), then
//! (exclusion id u32, width/height/x/y f32) records;
//! 3 free paragraph: empty input;
//! 4 register exclusion: shape-margin f32, shape-outside UTF-8;
//! 5 free exclusion, 6 reset: empty input. The caller supplies paragraph-content-local x/y.
//!
//! Eligibility comes from the core host's textflow contexts; registered shapes
//! are the browser's computed CSS for those IDs. At most 4096 styles are retained.
//! At most 64 live preparations, 64 KiB UTF-8 each, 64 shapes per call, and
//! 4096 bands per flow. Up to 4096 candidate boxes resolve to 63 shapes and
//! one conservative overflow envelope, reported as `limited`. Larger or
//! malformed requests refuse explicitly, leaving
//! ordinary DOM text available; a band-limited flow reports `complete:false`.
//! Preparation/mapping is O(bytes + requested ranges); retained storage O(bytes).
//! Flow inherits the shared walker's bound (with the smaller band ceiling),
//! plus O(bands*(E + I log I) + fragments*I) interval diagnostics, with E geometry
//! query work and I emitted exclusion intervals per band. No history of widths
//! or frames is retained. Serialization is linear in source/geometry output.

use exact_textflow::{FlowOptions, FlowShape, Fragment, Options, OverflowWrap, Prepared};
use std::{collections::BTreeMap, fmt::Write as _, ops::Range};

const MAX_TEXT: usize = 64 * 1024;
const MAX_PARAGRAPHS: usize = 64;
const MAX_SHAPES: usize = 64;
const MAX_GEOMETRY_SHAPES: usize = 4096;
const MAX_BANDS: u32 = 4096;

struct Source {
    text: String,
    utf16: Vec<usize>,
    ranges: Vec<Range<usize>>,
    words: Vec<usize>,
    options: Options,
    prepared: Option<Prepared>,
    fragments: Vec<Fragment>,
}

// At most 64 current paragraphs: a flat store bounds lookup at 64 comparisons
// and avoids retaining a full B-tree specialization in every web application.
#[derive(Default)]
struct Sources(Vec<(u32, Source)>);
impl Sources {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn contains_key(&self, id: &u32) -> bool {
        self.0.iter().any(|(key, _)| key == id)
    }
    fn get_mut(&mut self, id: &u32) -> Option<&mut Source> {
        self.0.iter_mut().find(|(key, _)| key == id).map(|(_, s)| s)
    }
    fn insert(&mut self, id: u32, source: Source) {
        if let Some(s) = self.get_mut(&id) {
            *s = source;
        } else {
            self.0.push((id, source));
        }
    }
    fn remove(&mut self, id: &u32) {
        if let Some(i) = self.0.iter().position(|(key, _)| key == id) {
            self.0.swap_remove(i);
        }
    }
}
#[cfg(test)]
impl std::ops::Index<&u32> for Sources {
    type Output = Source;
    fn index(&self, id: &u32) -> &Source {
        &self.0.iter().find(|(key, _)| key == id).unwrap().1
    }
}

/// Current sources only; a boot replaces the store, destruction frees a source.
#[derive(Default)]
pub struct TextFlow {
    sources: Sources,
    shapes: BTreeMap<u32, (exact_textflow::ShapeOutside, f32)>,
}

fn number(bytes: &[u8], at: usize) -> Result<f32, &'static str> {
    let n = f32::from_bits(integer(bytes, at)?);
    n.is_finite().then_some(n).ok_or("nonfinite textflow input")
}
fn integer(bytes: &[u8], at: usize) -> Result<u32, &'static str> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or("short textflow input")?
            .try_into()
            .unwrap(),
    ))
}
fn mapping(text: &str) -> Vec<usize> {
    let mut map = vec![0; text.len() + 1];
    let mut units = 0;
    for (at, ch) in text.char_indices() {
        map[at..at + ch.len_utf8()].fill(units);
        units += ch.len_utf16();
    }
    map[text.len()] = units;
    map
}
fn ranges_json(ranges: impl IntoIterator<Item = Range<usize>>, utf16: &[usize]) -> String {
    let mut out = String::from("[");
    for (i, r) in ranges.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "[{},{},{},{}]",
            r.start, r.end, utf16[r.start], utf16[r.end]
        );
    }
    out.push(']');
    out
}

impl TextFlow {
    /// Empty, allocation-free store for one optional browser instance.
    pub const fn new() -> Self {
        Self {
            sources: Sources(Vec::new()),
            shapes: BTreeMap::new(),
        }
    }

    /// The native-testable implementation of the exported protocol.
    pub fn request(&mut self, op: u32, id: u32, input: &[u8]) -> String {
        self.execute(op, id, input)
            .unwrap_or_else(|error| format!("{{\"error\":\"{error}\"}}"))
    }

    fn execute(&mut self, op: u32, id: u32, input: &[u8]) -> Result<String, &'static str> {
        match op {
            0 => self.segments(id, input),
            1 => self.prepare(id, input),
            2 => self.layout(id, input),
            3 if input.is_empty() => {
                self.sources.remove(&id);
                Ok("{}".into())
            }
            4 => self.exclusion(id, input),
            5 if input.is_empty() => {
                self.shapes.remove(&id);
                Ok("{}".into())
            }
            6 if input.is_empty() => {
                *self = Self::new();
                Ok("{}".into())
            }
            _ => Err("unknown textflow operation"),
        }
    }

    fn exclusion(&mut self, id: u32, input: &[u8]) -> Result<String, &'static str> {
        if input.len() > 16 * 1024 {
            return Err("textflow shape exceeds 16 KiB");
        }
        if self.shapes.len() >= MAX_GEOMETRY_SHAPES && !self.shapes.contains_key(&id) {
            return Err("textflow exceeds 4096 registered exclusions");
        }
        let margin = number(input, 0)?;
        if margin < 0.0 {
            return Err("negative shape-margin");
        }
        let css = std::str::from_utf8(&input[4..]).map_err(|_| "invalid shape UTF-8")?;
        let shape = exact_textflow::ShapeOutside::parse(css).ok_or("invalid shape-outside")?;
        self.shapes.insert(id, (shape, margin));
        Ok("{}".into())
    }

    fn segments(&mut self, id: u32, input: &[u8]) -> Result<String, &'static str> {
        if self.sources.len() >= MAX_PARAGRAPHS && !self.sources.contains_key(&id) {
            return Err("textflow exceeds 64 live paragraphs");
        }
        let overflow_wrap = match integer(input, 0)? {
            0 => OverflowWrap::Normal,
            1 => OverflowWrap::BreakWord,
            2 => OverflowWrap::Anywhere,
            _ => return Err("invalid overflow-wrap"),
        };
        let count = integer(input, 8)? as usize;
        let header = count
            .checked_mul(4)
            .and_then(|n| n.checked_add(12))
            .ok_or("short textflow word list")?;
        let source = input.get(header..).ok_or("short textflow word list")?;
        if source.len() > MAX_TEXT {
            return Err("textflow exceeds 64 KiB source limit");
        }
        let text = std::str::from_utf8(source).map_err(|_| "invalid textflow UTF-8")?;
        let words = (0..count)
            .map(|i| integer(input, 12 + 4 * i).map(|w| w as usize))
            .collect::<Result<Vec<_>, _>>()?;
        let words = exact_textflow::utf16_words(text, words);
        let options = Options {
            white_space: match integer(input, 4)? {
                0 => exact_textflow::WhiteSpace::Normal,
                1 => exact_textflow::WhiteSpace::PreWrap,
                2 => exact_textflow::WhiteSpace::Nowrap,
                3 => exact_textflow::WhiteSpace::PreLine,
                4 => exact_textflow::WhiteSpace::Pre,
                _ => return Err("invalid white-space"),
            },
            overflow_wrap,
            hyphen_advance: 0.0,
        };
        let mut ranges = Vec::new();
        // Ask the actual walker for every measurement, including spaces and
        // emergency clusters. Replaying this list must consume it exactly.
        Prepared::with_words(text, options, &words, &mut |r| {
            ranges.push(r);
            0.0
        });
        let utf16 = mapping(text);
        let out = format!(
            "{{\"ranges\":{},\"graphemes\":{},\"utf16_len\":{}}}",
            ranges_json(ranges.iter().cloned(), &utf16),
            ranges_json(exact_textflow::grapheme_ranges(text), &utf16),
            utf16[text.len()]
        );
        self.sources.insert(
            id,
            Source {
                text: text.into(),
                utf16,
                ranges,
                words,
                options,
                prepared: None,
                fragments: Vec::new(),
            },
        );
        Ok(out)
    }

    fn prepare(&mut self, id: u32, input: &[u8]) -> Result<String, &'static str> {
        let source = self
            .sources
            .get_mut(&id)
            .ok_or("textflow source is absent")?;
        if input.len() != 4 + source.ranges.len() * 4 {
            return Err("textflow advance count mismatch");
        }
        let hyphen = number(input, 0)?;
        if input.chunks_exact(4).any(|b| {
            let n = f32::from_le_bytes(b.try_into().unwrap());
            !n.is_finite() || n < 0.0
        }) {
            return Err("textflow advances must be finite and nonnegative");
        }
        let mut at = 0;
        let mut matched = true;
        let prepared = Prepared::with_words(
            &source.text,
            Options {
                hyphen_advance: hyphen,
                ..source.options
            },
            &source.words,
            &mut |r| {
                matched &= source.ranges.get(at) == Some(&r);
                at += 1;
                number(input, at * 4).unwrap_or(0.0)
            },
        );
        if !matched || at != source.ranges.len() {
            return Err("textflow measurement replay mismatch");
        }
        source.prepared = Some(prepared);
        Ok("{\"prepared\":true}".into())
    }

    fn layout(&mut self, id: u32, input: &[u8]) -> Result<String, &'static str> {
        if input.len() < 24
            || !(input.len() - 24).is_multiple_of(20)
            || (input.len() - 24) / 20 > MAX_GEOMETRY_SHAPES
        {
            return Err("textflow expects geometry and at most 4096 candidate exclusions");
        }
        let options = FlowOptions {
            direction: match integer(input, 20)? {
                0 => exact_textflow::Direction::Ltr,
                1 => exact_textflow::Direction::Rtl,
                _ => return Err("invalid direction"),
            },
            width: number(input, 0)?,
            line_height: number(input, 4)?,
            min_fragment: number(input, 8)? * exact_textflow::MIN_FRAGMENT_EM,
            max_lines: match integer(input, 12)? {
                0 => MAX_BANDS,
                n => n.min(MAX_BANDS),
            },
        };
        let paragraph_height = number(input, 16)?;
        if options.width < 0.0 || options.line_height <= 0.0 || paragraph_height < 0.0 {
            return Err("invalid textflow paragraph geometry");
        }
        let mut shapes: Vec<FlowShape> = Vec::new();
        let mut overflow: Option<(f32, f32, f32, f32)> = None;
        for at in (24..input.len()).step_by(20) {
            let (shape, margin) = self
                .shapes
                .get(&integer(input, at)?)
                .ok_or("textflow exclusion is stale")?;
            let width = number(input, at + 4)?;
            let height = number(input, at + 8)?;
            if width < 0.0 || height < 0.0 {
                return Err("negative exclusion size");
            }
            let shape = shape
                .resolve(width, height)
                .grow(*margin)
                .translate(number(input, at + 12)?, number(input, at + 16)?);
            if exact_textflow::meets(
                std::slice::from_ref(&shape),
                0.0,
                0.0,
                options.width,
                paragraph_height,
            ) {
                if shapes.len() == MAX_SHAPES {
                    overflow = shapes.pop().map(|s| s.bounds());
                }
                if let Some(bounds) = &mut overflow {
                    let b = shape.bounds();
                    *bounds = (
                        bounds.0.min(b.0),
                        bounds.1.min(b.1),
                        bounds.2.max(b.2),
                        bounds.3.max(b.3),
                    );
                } else {
                    shapes.push(shape);
                }
            }
        }
        // @ref LLP 1043.000 §3 D7 — overload may reserve extra whitespace,
        // never overlap an omitted exclusion. At most 64 shapes reach the walker.
        if let Some((x, y, right, bottom)) = overflow {
            shapes.push(FlowShape::RoundRect {
                x,
                y,
                width: right - x,
                height: bottom - y,
                radius: 0.,
            });
        }
        let mut reply = self.flow(id, &shapes, options)?;
        if overflow.is_some() {
            reply.insert_str(1, "\"limited\":true,");
        }
        Ok(reply)
    }

    fn flow(
        &mut self,
        id: u32,
        shapes: &[FlowShape],
        options: FlowOptions,
    ) -> Result<String, &'static str> {
        let source = self
            .sources
            .get_mut(&id)
            .ok_or("textflow source is absent")?;
        let prepared = source
            .prepared
            .as_ref()
            .ok_or("textflow source is not prepared")?;
        let result = exact_textflow::flow(prepared, shapes, &options, &mut source.fragments);
        // The walker can finish before a space-only segment following a hard
        // break. Keep those source units on the final DOM fragment: no extra
        // ink or band, and no false guard-exhaustion report.
        if let Some(last) = source.fragments.last_mut() {
            if source.text[last.end..]
                .chars()
                .all(|c| matches!(c, ' ' | '\t'))
            {
                last.end = source.text.len();
            }
        }
        let height = result.height;
        let complete = result.complete;
        let clamped = result.clamped && options.max_lines < MAX_BANDS;
        let mut out = format!(
            "{{\"height\":{height},\"line_height\":{},\"complete\":{complete},\"clamped\":{clamped},\"shapes\":[",
            options.line_height
        );
        for (i, shape) in shapes.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            shape.write_json(&mut out);
        }
        out.push_str("],\"fragments\":[");
        let mut slots = Vec::new();
        let mut band = u32::MAX;
        for (i, f) in source.fragments.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            if band != f.line {
                exact_textflow::intervals(
                    shapes,
                    f.y,
                    f.y + options.line_height,
                    options.width,
                    options.min_fragment,
                    &mut slots,
                );
                band = f.line;
            }
            let available = slots
                .iter()
                .find(|(x, _)| *x == f.x)
                .map_or(options.width, |(a, b)| b - a);
            let paint_range = prepared.paint_range(&source.text, f.start..f.end);
            let (start, end) = (paint_range.start, paint_range.end);
            let hyphenated = f.hyphenated;
            let _ = write!(out, "{{\"start\":{},\"end\":{},\"utf16_start\":{},\"utf16_end\":{},\"paint_start\":{},\"paint_end\":{},\"x\":{},\"y\":{},\"width\":{},\"available\":{},\"line\":{},\"hyphenated\":{hyphenated}}}",
                f.start, f.end, source.utf16[f.start], source.utf16[f.end], source.utf16[start], source.utf16[end], f.x, f.y, f.width, available, f.line);
        }
        out.push_str("]}");
        Ok(out)
    }
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
