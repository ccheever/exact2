//! @ref LLP 1043.000 §3 D6 — Pretext's greedy walker over cached host advances.

use crate::{chrome, finite};
use std::{borrow::Cow, ops::Range};
use unicode_linebreak::{break_property, linebreaks, BreakClass};

/// Fit tolerance added to a requested width before comparing advances.
///
/// Canvas and engine advances are rounded to fractions of a pixel, so a line
/// whose advances sum to at most this far past the offer still fits — the same
/// `lineFitEpsilon` Pretext's engine profile applies (`@chenglou/pretext`
/// `measurement.ts` / `line-break.ts`: 0.005, Safari 1/64).
const FIT_EPSILON: f64 = 0.005;

/// Measure a UTF-8 byte range in the original paragraph, shaped in its context.
/// The caller includes letter spacing and returns a nonnegative finite advance.
/// Collapsible ASCII whitespace is measured as a space, including a tab range;
/// format controls, soft hyphens and zero-width spaces have no ink advance.
pub trait Measure {
    /// Return the advance of this range in the caller's shaping units.
    fn advance(&mut self, range: Range<usize>) -> f32;
}
impl<F: FnMut(Range<usize>) -> f32> Measure for F {
    fn advance(&mut self, range: Range<usize>) -> f32 {
        self(range)
    }
}

/// CSS `overflow-wrap`; emergency breaks affect min-content only for `Anywhere`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverflowWrap {
    /// Unbreakable segments overflow a clear line.
    #[default]
    Normal,
    /// Emergency grapheme breaks; intrinsic min-content still uses whole words.
    BreakWord,
    /// Emergency grapheme breaks also reduce intrinsic min-content width.
    Anywhere,
}
/// CSS `white-space`, the shorthand of `white-space-collapse` × `text-wrap-mode`:
/// `normal` is collapse × wrap, `pre-wrap` preserve × wrap, `nowrap` collapse ×
/// nowrap. @ref LLP 1053 §0 G5 — `pre-line` (preserve-breaks × wrap) is next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WhiteSpace {
    /// Collapse spaces, tabs and segment breaks to one hanging space.
    #[default]
    Normal,
    /// Preserve spaces/tabs and break at segment breaks; trailing spaces hang.
    PreWrap,
    /// Collapse like `normal` with no soft wrap opportunity: one line, however wide.
    Nowrap,
}
impl WhiteSpace {
    /// `white-space-collapse: preserve`: spaces and segment breaks are kept;
    /// otherwise they collapse ([`crate::collapse`] on native engines).
    pub fn preserves(self) -> bool {
        self == WhiteSpace::PreWrap
    }
    /// `text-wrap-mode: wrap`: soft wrap opportunities may end a line.
    pub fn wraps(self) -> bool {
        self != WhiteSpace::Nowrap
    }
}
/// Width-independent preparation options; letter spacing belongs to `Measure`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    /// CSS whitespace policy; defaults to `normal`.
    pub white_space: WhiteSpace,
    /// Emergency breaking policy; defaults to CSS `normal`.
    pub overflow_wrap: OverflowWrap,
    /// Advance of a visible hyphen when a soft-hyphen opportunity is taken.
    pub hyphen_advance: f32,
}
/// A resumable UTF-8 source cursor; begin with `Cursor::default()`.
/// Cursors belong to the `Prepared` that produced them; internal hints avoid searches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    /// Byte offset into the original UTF-8 text, including consumed whitespace.
    pub byte: usize,
    segment: usize,
    atom: usize,
}
/// One line's consumed source range and painted advance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineRange {
    /// Inclusive source cursor (leading collapsible whitespace remains owned).
    pub start: Cursor,
    /// Exclusive continuation cursor (hanging spaces and hard breaks are owned).
    pub end: Cursor,
    /// Advance excluding leading/trailing collapsed whitespace and break controls.
    pub width: f32,
    /// A mandatory break ended this line, including a final newline.
    pub hard_break: bool,
    /// Paint a discretionary hyphen after this range; a terminal SHY stays invisible.
    pub hyphenated: bool,
}
#[derive(Clone, Debug)]
struct Atom {
    end: usize,
    prefix: f64,
    space: bool,
}
#[derive(Clone, Debug)]
struct Segment {
    start: usize,
    width: f32,
    space: f32,
    visible: bool,
    hard: bool,
    hyphen: bool,
    atoms: Range<usize>,
}
/// An owned, immutable (`Send + Sync`) paragraph of cached advances and byte offsets.
///
/// Each visible run is measured once (a UAX #14 segment, split around internal
/// collapsed space runs where needed). `normal` collapses spaces, tabs and
/// segment breaks; `pre-wrap` preserves spaces/tabs and mandatory breaks;
/// `nowrap` collapses like `normal` and never breaks (`overflow-wrap` only
/// applies where wrapping is allowed, so it admits no emergency cut either).
/// A terminal break ends a line without a synthetic empty line.
/// @ref LLP 1043.000 §3 D6 — CSS whitespace processing found in M9 review.
///
/// Emergency modes also eagerly measure each conservative grapheme once, so
/// `next_line` never measures, allocates, or mutates. Partial-word advances sum
/// those cached graphemes; whole words use the caller's whole-segment measurement.
/// This approximation cannot reproduce nonadditive shaping across emergency cuts.
#[derive(Clone, Debug)]
pub struct Prepared {
    segments: Vec<Segment>,
    atoms: Vec<Atom>,
    options: Options,
    len: usize,
    units: usize,
    content_end: usize,
}
impl Prepared {
    /// Accessible advance/segment storage retained by a host's immutable shape.
    /// Excludes allocator overhead and the caller-owned source/shaping.
    pub fn capacity_bytes(&self) -> usize {
        self.segments.capacity() * std::mem::size_of::<Segment>()
            + self.atoms.capacity() * std::mem::size_of::<Atom>()
    }

    /// Prepare Chrome's break opportunities, source offsets and advances.
    pub fn new(text: &str, options: Options, measure: &mut dyn Measure) -> Self {
        Self::with_words(text, options, &[], measure)
    }

    /// [`Prepared::new`], with the word boundaries a host's segmenter found
    /// (ascending UTF-8 byte offsets). The walker has no dictionary: between two
    /// characters of Thai, Lao, Khmer or Myanmar (UAX #14 class SA) these are its
    /// only opportunities, as ICU's dictionaries are the browser's and CoreText's.
    /// Elsewhere they are ignored. Without them an SA run breaks only at spaces.
    pub fn with_words(
        text: &str,
        options: Options,
        words: &[usize],
        measure: &mut dyn Measure,
    ) -> Self {
        let options = Options {
            hyphen_advance: advance(options.hyphen_advance),
            overflow_wrap: if options.white_space == WhiteSpace::Nowrap {
                OverflowWrap::Normal
            } else {
                options.overflow_wrap
            },
            ..options
        };
        let mut result = Self {
            segments: Vec::new(),
            atoms: Vec::new(),
            options,
            len: text.len(),
            units: 0,
            content_end: 0,
        };
        let preserve = options.white_space == WhiteSpace::PreWrap;
        let whitespace = |ch| space(ch) || (!preserve && hard_break(ch));
        let mut start = 0;
        for end in opportunities(text, words) {
            if end == start {
                continue;
            }
            // Keep even ordinary UAX opportunities out of conservative clusters
            // (notably a combining mark after an ASCII space or a newer mark).
            let previous = text[..end].chars().next_back().unwrap();
            if !hard_break(previous)
                && text[end..].chars().next().is_some_and(|ch| {
                    joins_previous(ch, previous) && !(complex(ch) && complex(previous))
                })
            {
                continue;
            }
            // One collapsed run can cross several UAX mandatory boundaries.
            if !preserve
                && whitespace(previous)
                && text[end..].chars().next().is_some_and(whitespace)
            {
                continue;
            }
            let source = &text[start..end];
            let hard = preserve && source.chars().next_back().is_some_and(hard_break);
            let without_break = if preserve {
                source.trim_end_matches(hard_break)
            } else {
                source
            };
            let without_space = without_break.trim_end_matches(whitespace);
            let hyphen = without_break.ends_with('\u{ad}') && !hard && end < text.len();
            let content = without_space.trim_end_matches(['\u{ad}', '\u{200b}']);
            let content = if preserve {
                content
            } else {
                content.trim_start_matches(whitespace)
            };
            let content_start = if preserve {
                start
            } else {
                start + without_space.len() - without_space.trim_start_matches(whitespace).len()
            };
            let content_end = content_start + content.len();
            let width = if content.is_empty() {
                0.0
            } else {
                if preserve {
                    advance(measure.advance(content_start..content_end))
                } else {
                    collapsed_advance(text, content_start..content_end, measure)
                }
            };
            let trailing = without_space.len();
            let space_width = if trailing < without_break.len() {
                let len = without_break[trailing..].chars().next().unwrap().len_utf8();
                advance(measure.advance(
                    start + trailing..if preserve {
                        start + without_break.len()
                    } else {
                        start + trailing + len
                    },
                ))
            } else {
                0.0
            };
            let atom_start = result.atoms.len();
            if options.overflow_wrap != OverflowWrap::Normal && !content.is_empty() {
                let mut prefix = 0.0;
                for cluster in grapheme_ranges(content) {
                    let range = content_start + cluster.start..content_start + cluster.end;
                    let is_space = text[range.clone()].chars().all(space);
                    let measured = if is_space && !preserve {
                        range.start..range.start + 1
                    } else {
                        range.clone()
                    };
                    prefix += if cluster.start == 0 && cluster.end == content.len() {
                        width as f64
                    } else {
                        advance(measure.advance(measured)) as f64
                    };
                    result.atoms.push(Atom {
                        end: range.end,
                        prefix,
                        space: is_space,
                    });
                }
            }
            result.units += (result.atoms.len() - atom_start).max(1);
            if !content.is_empty()
                || without_space.contains('\u{200b}')
                || hard
                || (preserve && !without_break.is_empty())
            {
                result.content_end = result.segments.len() + 1;
            }
            result.segments.push(Segment {
                start,
                width,
                space: space_width,
                hard,
                hyphen,
                visible: !content.is_empty()
                    || without_space.contains('\u{200b}')
                    || (preserve && !without_break.is_empty()),
                atoms: atom_start..result.atoms.len(),
            });
            start = end;
        }
        result
    }

    /// Source ink range; trailing hanging spaces carry no ink, leading preserved spaces remain.
    pub fn paint_range(&self, text: &str, range: Range<usize>) -> Range<usize> {
        let raw = &text[range.clone()];
        if self.options.white_space == WhiteSpace::PreWrap {
            range.start..range.start + raw.trim_end_matches(|ch| space(ch) || hard_break(ch)).len()
        } else {
            let trim = |ch| space(ch) || hard_break(ch);
            let leading = raw.len() - raw.trim_start_matches(trim).len();
            let start = range.start + leading;
            start..start + raw.trim_matches(trim).len()
        }
    }

    /// Greedily consume one line at `width`; no allocation or measurement.
    /// Negative/NaN widths act as zero; positive infinity requests max-content.
    /// Returns `None` after exhaustion, for only spaces, or an invalid cursor.
    pub fn next_line(&self, start: Cursor, width: f32) -> Option<LineRange> {
        if !self.valid_cursor(start) {
            return None;
        }
        let fit = self.fit(width);
        let mut total = 0.0;
        let mut visible = false;
        let mut best = None;
        let mut fallback = None;
        for i in start.segment..self.segments.len() {
            let segment = &self.segments[i];
            let mut atom = if i == start.segment { start.atom } else { 0 };
            // Emergency continuation may begin at an internal collapsed space.
            // Own those bytes but give them no width at the new line's start.
            while self.options.white_space != WhiteSpace::PreWrap
                && atom > 0
                && atom < segment.atoms.len()
                && self.atoms[segment.atoms.start + atom].space
            {
                atom += 1;
            }
            let segment_width = self.remaining_width(segment, atom);
            let paint = total + segment_width;
            let end = self.cursor(i + 1, 0);
            let line = LineRange {
                start,
                end,
                width: finite(paint),
                hard_break: segment.hard,
                hyphenated: false,
            };
            if paint > fit && (visible || segment.visible) {
                if let Some(best) = best {
                    return Some(best);
                }
                if let Some(fallback) = fallback {
                    return Some(fallback);
                }
                if self.options.overflow_wrap != OverflowWrap::Normal && !segment.atoms.is_empty() {
                    let first = segment.atoms.start + atom;
                    let base = if atom == 0 {
                        0.0
                    } else {
                        self.atoms[first - 1].prefix
                    };
                    let mut last = first;
                    let mut painted = 0.0;
                    for j in first..segment.atoms.end {
                        let candidate = if self.atoms[j].space {
                            // A space run hangs, even after an overflowing cluster.
                            painted
                        } else {
                            self.atoms[j].prefix - base
                        };
                        if j > first && !self.atoms[j].space && candidate > fit {
                            break;
                        }
                        last = j;
                        painted = candidate;
                    }
                    let consumed = last + 1 - segment.atoms.start;
                    let end = if last + 1 == segment.atoms.end {
                        end
                    } else {
                        self.cursor(i, consumed)
                    };
                    let complete = last + 1 == segment.atoms.end;
                    let hyphenated = complete && segment.hyphen;
                    return Some(LineRange {
                        start,
                        end,
                        width: finite(
                            painted
                                + if hyphenated {
                                    self.options.hyphen_advance as f64
                                } else {
                                    0.0
                                },
                        ),
                        hard_break: complete && segment.hard,
                        hyphenated,
                    });
                }
                return Some(LineRange {
                    width: finite(
                        paint
                            + if segment.hyphen {
                                self.options.hyphen_advance as f64
                            } else {
                                0.0
                            },
                    ),
                    hyphenated: segment.hyphen,
                    ..line
                });
            }
            visible |= segment.visible;
            if segment.hard {
                return Some(line);
            }
            if i + 1 == self.segments.len() {
                return visible.then_some(line);
            }
            let candidate = LineRange {
                width: finite(
                    paint
                        + if segment.hyphen {
                            self.options.hyphen_advance as f64
                        } else {
                            0.0
                        },
                ),
                hyphenated: segment.hyphen,
                ..line
            };
            if visible {
                fallback = Some(candidate);
                if candidate.width as f64 <= fit {
                    best = Some(candidate);
                }
            }
            total = paint + if visible { segment.space as f64 } else { 0.0 };
        }
        None
    }

    /// Count lines and their largest advance using exactly the streaming walker.
    ///
    /// Under `overflow-wrap: normal` no grapheme atoms exist, so every line
    /// starts and ends on a segment boundary; the arithmetic below then makes
    /// the same break decisions as [`Prepared::next_line`] without cursor or
    /// range bookkeeping — the hot path Pretext's `layout()` optimizes for.
    pub fn line_stats(&self, width: f32) -> (usize, f32) {
        if self.options.overflow_wrap == OverflowWrap::Normal {
            return self.count_and_max(width);
        }
        let mut cursor = Cursor::default();
        let mut count = 0;
        let mut max: f32 = 0.0;
        while let Some(line) = self.next_line(cursor, width) {
            count += 1;
            max = max.max(line.width);
            cursor = line.end;
        }
        (count, max)
    }

    /// Count wrapped lines at `width`, mirroring Pretext's `layout()` helper.
    pub fn count_lines(&self, width: f32) -> usize {
        self.line_stats(width).0
    }

    /// Segment-boundary fast path behind [`Prepared::line_stats`].
    ///
    /// Break decisions match [`Prepared::next_line`] exactly (same fit
    /// tolerance, same best/fallback rule, same hyphen and hard-break handling);
    /// `tests/walker.rs` checks both agree over the corpus and fuzz text.
    fn count_and_max(&self, width: f32) -> (usize, f32) {
        let fit = self.fit(width);
        let hyphen = self.options.hyphen_advance as f64;
        let n = self.segments.len();
        let mut start = 0;
        let mut count = 0;
        let mut max: f32 = 0.0;
        while start < n {
            let mut total = 0.0;
            let mut visible = false;
            let mut best: Option<(usize, f32)> = None;
            let mut fallback: Option<(usize, f32)> = None;
            let mut i = start;
            // Every arm ends past `start` or ends the walk, so this terminates.
            let line: Option<(usize, f32)> = loop {
                let segment = &self.segments[i];
                let paint = total + segment.width as f64;
                if paint > fit && (visible || segment.visible) {
                    if let Some(found) = best {
                        break Some(found);
                    }
                    if let Some(found) = fallback {
                        break Some(found);
                    }
                    break Some((i + 1, hyphenated(paint, segment.hyphen, hyphen)));
                }
                visible |= segment.visible;
                if segment.hard {
                    break Some((i + 1, finite(paint)));
                }
                if i + 1 == n {
                    break visible.then_some((n, finite(paint)));
                }
                let candidate = hyphenated(paint, segment.hyphen, hyphen);
                if visible {
                    fallback = Some((i + 1, candidate));
                    if candidate as f64 <= fit {
                        best = Some((i + 1, candidate));
                    }
                }
                total = paint + if visible { segment.space as f64 } else { 0.0 };
                i += 1;
            };
            let Some((end, w)) = line else {
                break;
            };
            count += 1;
            max = max.max(w);
            start = end;
        }
        (count, max)
    }
    /// Max-content width: the widest mandatory-break-delimited line.
    pub fn natural_width(&self) -> f32 {
        self.line_stats(f32::INFINITY).1
    }

    /// Min-content width; only `Anywhere` counts emergency grapheme opportunities,
    /// and `nowrap` has none at all, so its min-content is its max-content.
    pub fn min_content_width(&self) -> f32 {
        if self.options.white_space == WhiteSpace::Nowrap {
            return self.natural_width();
        }
        let mut max: f32 = 0.0;
        for s in &self.segments {
            if self.options.overflow_wrap == OverflowWrap::Anywhere {
                let mut previous = 0.0;
                for a in &self.atoms[s.atoms.clone()] {
                    if !a.space {
                        max = max.max(finite(a.prefix - previous));
                    }
                    previous = a.prefix;
                }
            } else {
                max = max.max(finite(
                    s.width as f64
                        + if s.hyphen {
                            self.options.hyphen_advance as f64
                        } else {
                            0.0
                        },
                ));
            }
        }
        max
    }
    pub(crate) fn work_units(&self) -> usize {
        self.units
    }
    /// The advance a line may paint before it breaks; `nowrap` never breaks.
    fn fit(&self, width: f32) -> f64 {
        if self.options.white_space == WhiteSpace::Nowrap {
            return f64::INFINITY;
        }
        (if width.is_nan() { 0.0 } else { width.max(0.0) } as f64) + FIT_EPSILON
    }
    pub(crate) fn has_remaining(&self, cursor: Cursor) -> bool {
        cursor.segment < self.content_end && self.valid_cursor(cursor)
    }
    fn remaining_width(&self, s: &Segment, atom: usize) -> f64 {
        if atom == 0 {
            s.width as f64
        } else {
            self.atoms[s.atoms.end - 1].prefix - self.atoms[s.atoms.start + atom - 1].prefix
        }
    }
    fn cursor(&self, segment: usize, atom: usize) -> Cursor {
        let byte = if segment == self.segments.len() {
            self.len
        } else if atom == 0 {
            self.segments[segment].start
        } else {
            self.atoms[self.segments[segment].atoms.start + atom - 1].end
        };
        Cursor {
            byte,
            segment,
            atom,
        }
    }
    fn valid_cursor(&self, c: Cursor) -> bool {
        c.segment < self.segments.len()
            && c.atom <= self.segments[c.segment].atoms.len()
            && (c.atom == 0 || c.atom < self.segments[c.segment].atoms.len())
            && c == self.cursor(c.segment, c.atom)
    }
}
// UAX can keep spaces inside a segment (e.g. after an opener). Preserve shaping
// of its visible runs while measuring each collapsed whitespace run just once.
fn collapsed_advance(text: &str, range: Range<usize>, measure: &mut dyn Measure) -> f32 {
    let source = &text[range.clone()];
    if !source
        .as_bytes()
        .windows(2)
        .any(|w| w.iter().all(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r')))
    {
        return advance(measure.advance(range));
    }
    let mut total = 0.0;
    let mut from = range.start;
    let mut chars = source.char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        if !(space(ch) || hard_break(ch)) {
            continue;
        }
        let at = range.start + offset;
        if from < at {
            total += advance(measure.advance(from..at)) as f64;
        }
        total += advance(measure.advance(at..at + ch.len_utf8())) as f64;
        from = at + ch.len_utf8();
        while chars
            .peek()
            .is_some_and(|(_, ch)| space(*ch) || hard_break(*ch))
        {
            let (offset, ch) = chars.next().unwrap();
            from = range.start + offset + ch.len_utf8();
        }
    }
    if from < range.end {
        total += advance(measure.advance(from..range.end)) as f64;
    }
    finite(total)
}
/// UTF-16 word boundaries, as `Intl.Segmenter` and `CFStringTokenizer` give
/// them, to the UTF-8 byte offsets [`Prepared::with_words`] takes. A boundary
/// inside a surrogate pair, out of order, or past the end is dropped.
pub fn utf16_words(text: &str, words: impl IntoIterator<Item = usize>) -> Vec<usize> {
    let mut result = Vec::new();
    let (mut byte, mut units, mut chars) = (0, 0, text.chars());
    for word in words {
        while units < word {
            let Some(ch) = chars.next() else { break };
            byte += ch.len_utf8();
            units += ch.len_utf16();
        }
        if units == word && result.last() < Some(&byte) {
            result.push(byte);
        }
    }
    result
}

fn complex(ch: char) -> bool {
    break_property(ch as u32) == BreakClass::ComplexContext
}

/// Where a line may start, in the order Blink decides it (`NextBreakablePosition`,
/// `text_break_iterator.cc`): never before a space, always after one, Chromium's
/// table between two Latin-1 characters, and otherwise UAX #14 under CSS
/// `line-break: normal`, where small kana and `ー` (class CJ) break as ideographs,
/// and inside SA runs the host's word boundaries stand in for ICU's dictionaries.
/// A break beside a mandatory break stays UAX's. The last entry is `text.len()`.
/// @ref LLP 1043 §4 C — measured against Chrome by `tests/it/corpus.rs`.
fn opportunities(text: &str, words: &[usize]) -> Vec<usize> {
    let starter = |c: char| break_property(c as u32) == BreakClass::ConditionalJapaneseStarter;
    // Swap each CJ character for an ideograph of the same UTF-8 length.
    let normal = if text.chars().any(starter) {
        Cow::Owned(
            text.chars()
                .map(|c| match (starter(c), c.len_utf8()) {
                    (true, 3) => '\u{4e00}',
                    (true, _) => '\u{20000}',
                    _ => c,
                })
                .collect(),
        )
    } else {
        Cow::Borrowed(text)
    };
    let mut uax = linebreaks(&normal).map(|(at, _)| at).peekable();
    let latin1 = |c: char| ('\u{21}'..='\u{ff}').contains(&c);
    let mut result = Vec::new();
    let mut words = words.iter().copied().peekable();
    let (mut before_before, mut before) = ('\0', None);
    for (at, ch) in text.char_indices() {
        while uax.next_if(|&b| b < at).is_some() {}
        let listed = uax.peek() == Some(&at);
        while words.next_if(|&w| w < at).is_some() {}
        if let Some(previous) = before {
            let open = if hard_break(previous) || hard_break(ch) {
                listed
            } else if space(ch) {
                false
            } else if space(previous) {
                true
            } else if complex(previous) && complex(ch) {
                words.peek() == Some(&at)
            } else if previous == '-' && ch.is_ascii_digit() {
                before_before.is_ascii_alphanumeric()
            } else if latin1(previous) && latin1(ch) && !(previous == '-' && !ch.is_ascii()) {
                chrome::breaks(previous, ch)
            } else {
                listed
            };
            if open {
                result.push(at);
            }
            before_before = previous;
        }
        before = Some(ch);
    }
    result.push(text.len());
    result
}
fn advance(n: f32) -> f32 {
    if n.is_finite() {
        n.max(0.0)
    } else {
        0.0
    }
}
/// A candidate line width, adding the visible hyphen when a soft-hyphen
/// opportunity ends the line — shared by both walk paths.
fn hyphenated(paint: f64, hyphen: bool, hyphen_advance: f64) -> f32 {
    finite(paint + if hyphen { hyphen_advance } else { 0.0 })
}
fn space(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}
fn hard_break(ch: char) -> bool {
    matches!(
        ch,
        '\n' | '\r' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

// Small conservative rule using the already-linked UAX table. An SA run is
// glued as a whole (no dictionary segmentation), but only to SA before it: a
// space or zero-width space still separates Thai, Khmer or Myanmar words.
// GL includes several spacing marks.
// Unknown code points are conservatively glued, covering marks newer than the
// dependency's Unicode table; Tibetan U+0F7F is a spacing mark with class BA.
// UTF-8 Rust chars cannot contain either surrogate half. Emoji modifiers/tags,
// regional pairs, ZWJ on either side, and virama continuations stay attached.
fn joins_previous(ch: char, previous: char) -> bool {
    let class = break_property(ch as u32);
    matches!(
        class,
        BreakClass::CombiningMark
            | BreakClass::ZeroWidthJoiner
            | BreakClass::NonBreakingGlue
            | BreakClass::Unknown
    ) || (class == BreakClass::ComplexContext
        && matches!(
            break_property(previous as u32),
            BreakClass::ComplexContext | BreakClass::CombiningMark
        ))
        || previous == '\u{200d}'
        || matches!(ch as u32, 0x0f7f | 0xfe00..=0xfe0f | 0xe0100..=0xe01ef | 0x1f3fb..=0x1f3ff | 0xe0020..=0xe007f)
        || matches!(
            previous as u32,
            0x094d | 0x09cd | 0x0a4d | 0x0acd | 0x0b4d | 0x0bcd | 0x0c4d | 0x0ccd | 0x0d4d | 0x0dca
        )
}

/// Conservative grapheme ranges used by emergency breaking and host spacing.
/// Uses the walker's UAX table, including emoji ZWJ, flags and Indic joins;
/// hosts must not substitute a different segmenter's boundaries.
/// @ref LLP 1043.000 §3 D6 — one source of break/cluster boundaries.
pub fn grapheme_ranges(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut chars = text.char_indices().peekable();
    std::iter::from_fn(move || {
        let (start, mut previous) = chars.next()?;
        let mut regional = usize::from(matches!(previous as u32, 0x1f1e6..=0x1f1ff));
        while let Some(&(_, ch)) = chars.peek() {
            let regional_here = matches!(ch as u32, 0x1f1e6..=0x1f1ff);
            if !joins_previous(ch, previous)
                && !(regional_here && regional % 2 == 1)
                && !(space(ch) && space(previous))
            {
                break;
            }
            chars.next();
            regional = if regional_here { regional + 1 } else { 0 };
            previous = ch;
        }
        Some(start..chars.peek().map_or(text.len(), |(at, _)| *at))
    })
}
