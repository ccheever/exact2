//! CSS grid track and placement values.
//!
//! The wire keeps a small, closed subset of CSS Grid: fixed repetitions are
//! expanded into at most 32 tracks, and each track is a breadth or `minmax()`.
//! Keeping the parser here gives Contract literals and runtime writes the same
//! grammar that the kernel lowers to Taffy and the web prints back to CSS.

use taffy::prelude::{auto, fr, length, line, max_content, min_content, minmax, percent, span};
use taffy::style::{MaxTrackSizingFunction, MinTrackSizingFunction, TrackSizingFunction};

use super::MAX_GRID_TRACKS;

/// A minimum breadth in `minmax()` (CSS does not admit a flexible minimum).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrackMin {
    /// Layout points.
    Points(f32),
    /// Percent of the grid container, authored as 0–100.
    Percent(f32),
    /// Auto-sized.
    Auto,
    /// Min-content.
    MinContent,
    /// Max-content.
    MaxContent,
}

/// A maximum breadth in `minmax()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrackMax {
    /// A flexible fraction of the free space.
    Fr(f32),
    /// Layout points.
    Points(f32),
    /// Percent of the grid container, authored as 0–100.
    Percent(f32),
    /// Auto-sized.
    Auto,
    /// Min-content.
    MinContent,
    /// Max-content.
    MaxContent,
}

/// One grid track under the closed portable grammar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrack {
    /// A flexible fraction of the free space.
    Fr(f32),
    /// Layout points.
    Points(f32),
    /// Percent of the grid container, authored as 0–100.
    Percent(f32),
    /// Auto-sized.
    Auto,
    /// Min-content.
    MinContent,
    /// Max-content.
    MaxContent,
    /// Separate minimum and maximum track breadths.
    MinMax(GridTrackMin, GridTrackMax),
}

/// A grid template: an ordered track list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GridTracks(pub Vec<GridTrack>);

impl GridTracks {
    /// Parse the closed CSS grammar used by dynamic style values.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("none") {
            return Some(Self::default());
        }
        let mut tracks = Vec::new();
        parse_list(text, &mut tracks)?;
        (!tracks.is_empty() && tracks.len() <= MAX_GRID_TRACKS).then_some(Self(tracks))
    }

    /// The canonical CSS declaration value.
    pub fn css(&self) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        self.0.iter().map(track_css).collect::<Vec<_>>().join(" ")
    }

    /// Whether every track size is finite.
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|track| match *track {
            GridTrack::Fr(v) | GridTrack::Points(v) | GridTrack::Percent(v) => v.is_finite(),
            GridTrack::MinMax(min, max) => min_finite(min) && max_finite(max),
            GridTrack::Auto | GridTrack::MinContent | GridTrack::MaxContent => true,
        })
    }

    /// Whether every numeric breadth is nonnegative, as CSS requires.
    pub(crate) fn is_valid(&self) -> bool {
        self.0.iter().all(|track| match *track {
            GridTrack::Fr(v) | GridTrack::Points(v) | GridTrack::Percent(v) => v >= 0.0,
            GridTrack::MinMax(min, max) => min_nonnegative(min) && max_nonnegative(max),
            GridTrack::Auto | GridTrack::MinContent | GridTrack::MaxContent => true,
        })
    }

    /// `count` equal `1fr` tracks.
    pub fn equal(count: usize) -> Self {
        Self(vec![GridTrack::Fr(1.0); count.min(MAX_GRID_TRACKS)])
    }
}

fn parse_list(text: &str, out: &mut Vec<GridTrack>) -> Option<()> {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if at == bytes.len() {
            break;
        }
        let start = at;
        let mut depth = 0u8;
        while at < bytes.len() {
            match bytes[at] {
                b'(' => depth = depth.checked_add(1)?,
                b')' => {
                    depth = depth.checked_sub(1)?;
                }
                c if c.is_ascii_whitespace() && depth == 0 => break,
                _ => {}
            }
            at += 1;
        }
        if depth != 0 {
            return None;
        }
        parse_component(&text[start..at], out)?;
        if out.len() > MAX_GRID_TRACKS {
            return None;
        }
    }
    Some(())
}

fn parse_component(text: &str, out: &mut Vec<GridTrack>) -> Option<()> {
    if let Some(args) = function(text, "repeat") {
        let (count, body) = split_comma(args)?;
        let count = count.trim().parse::<u16>().ok().filter(|n| *n > 0)? as usize;
        let mut repeated = Vec::new();
        parse_list(body.trim(), &mut repeated)?;
        if repeated.is_empty() || repeated.len().checked_mul(count)? > MAX_GRID_TRACKS - out.len() {
            return None;
        }
        for _ in 0..count {
            out.extend_from_slice(&repeated);
        }
        return Some(());
    }
    out.push(parse_track(text)?);
    Some(())
}

fn parse_track(text: &str) -> Option<GridTrack> {
    if let Some(args) = function(text, "minmax") {
        let (min, max) = split_comma(args)?;
        return Some(GridTrack::MinMax(
            parse_min(min.trim())?,
            parse_max(max.trim())?,
        ));
    }
    match parse_max(text)? {
        GridTrackMax::Fr(v) => Some(GridTrack::Fr(v)),
        GridTrackMax::Points(v) => Some(GridTrack::Points(v)),
        GridTrackMax::Percent(v) => Some(GridTrack::Percent(v)),
        GridTrackMax::Auto => Some(GridTrack::Auto),
        GridTrackMax::MinContent => Some(GridTrack::MinContent),
        GridTrackMax::MaxContent => Some(GridTrack::MaxContent),
    }
}

fn function<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')
}

fn split_comma(text: &str) -> Option<(&str, &str)> {
    let mut depth = 0u8;
    let mut comma = None;
    for (i, byte) in text.bytes().enumerate() {
        match byte {
            b'(' => depth = depth.checked_add(1)?,
            b')' => depth = depth.checked_sub(1)?,
            b',' if depth == 0 && comma.replace(i).is_some() => return None,
            _ => {}
        }
    }
    let comma = comma?;
    Some((&text[..comma], &text[comma + 1..]))
}

fn number(text: &str, suffix: &str) -> Option<f32> {
    let value = exact_num::parse_f32(text.strip_suffix(suffix)?).ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn parse_min(text: &str) -> Option<GridTrackMin> {
    if let Some(v) = number(text, "px") {
        Some(GridTrackMin::Points(v))
    } else if let Some(v) = number(text, "%") {
        Some(GridTrackMin::Percent(v))
    } else if text == "0" {
        Some(GridTrackMin::Points(0.0))
    } else {
        Some(match text {
            "auto" => GridTrackMin::Auto,
            "min-content" => GridTrackMin::MinContent,
            "max-content" => GridTrackMin::MaxContent,
            _ => return None,
        })
    }
}

fn parse_max(text: &str) -> Option<GridTrackMax> {
    if let Some(v) = number(text, "fr") {
        Some(GridTrackMax::Fr(v))
    } else if let Some(v) = number(text, "px") {
        Some(GridTrackMax::Points(v))
    } else if let Some(v) = number(text, "%") {
        Some(GridTrackMax::Percent(v))
    } else if text == "0" {
        Some(GridTrackMax::Points(0.0))
    } else {
        Some(match text {
            "auto" => GridTrackMax::Auto,
            "min-content" => GridTrackMax::MinContent,
            "max-content" => GridTrackMax::MaxContent,
            _ => return None,
        })
    }
}

fn scalar(value: f32, suffix: &str) -> String {
    format!("{}{suffix}", exact_num::Shortest32(value))
}

fn min_css(value: GridTrackMin) -> String {
    match value {
        GridTrackMin::Points(v) => scalar(v, "px"),
        GridTrackMin::Percent(v) => scalar(v, "%"),
        GridTrackMin::Auto => "auto".into(),
        GridTrackMin::MinContent => "min-content".into(),
        GridTrackMin::MaxContent => "max-content".into(),
    }
}

fn max_css(value: GridTrackMax) -> String {
    match value {
        GridTrackMax::Fr(v) => scalar(v, "fr"),
        GridTrackMax::Points(v) => scalar(v, "px"),
        GridTrackMax::Percent(v) => scalar(v, "%"),
        GridTrackMax::Auto => "auto".into(),
        GridTrackMax::MinContent => "min-content".into(),
        GridTrackMax::MaxContent => "max-content".into(),
    }
}

fn track_css(value: &GridTrack) -> String {
    match *value {
        GridTrack::Fr(v) => scalar(v, "fr"),
        GridTrack::Points(v) => scalar(v, "px"),
        GridTrack::Percent(v) => scalar(v, "%"),
        GridTrack::Auto => "auto".into(),
        GridTrack::MinContent => "min-content".into(),
        GridTrack::MaxContent => "max-content".into(),
        GridTrack::MinMax(min, max) => format!("minmax({}, {})", min_css(min), max_css(max)),
    }
}

fn min_finite(value: GridTrackMin) -> bool {
    match value {
        GridTrackMin::Points(v) | GridTrackMin::Percent(v) => v.is_finite(),
        GridTrackMin::Auto | GridTrackMin::MinContent | GridTrackMin::MaxContent => true,
    }
}

fn max_finite(value: GridTrackMax) -> bool {
    match value {
        GridTrackMax::Fr(v) | GridTrackMax::Points(v) | GridTrackMax::Percent(v) => v.is_finite(),
        GridTrackMax::Auto | GridTrackMax::MinContent | GridTrackMax::MaxContent => true,
    }
}

fn min_nonnegative(value: GridTrackMin) -> bool {
    match value {
        GridTrackMin::Points(v) | GridTrackMin::Percent(v) => v >= 0.0,
        GridTrackMin::Auto | GridTrackMin::MinContent | GridTrackMin::MaxContent => true,
    }
}

fn max_nonnegative(value: GridTrackMax) -> bool {
    match value {
        GridTrackMax::Fr(v) | GridTrackMax::Points(v) | GridTrackMax::Percent(v) => v >= 0.0,
        GridTrackMax::Auto | GridTrackMax::MinContent | GridTrackMax::MaxContent => true,
    }
}

pub(crate) fn track(value: GridTrack) -> TrackSizingFunction {
    match value {
        GridTrack::Fr(v) => fr(v),
        GridTrack::Points(v) => length(v),
        GridTrack::Percent(v) => percent(v / 100.0),
        GridTrack::Auto => auto(),
        GridTrack::MinContent => min_content(),
        GridTrack::MaxContent => max_content(),
        GridTrack::MinMax(min, max) => minmax(min_track(min), max_track(max)),
    }
}

fn min_track(value: GridTrackMin) -> MinTrackSizingFunction {
    match value {
        GridTrackMin::Points(v) => length(v),
        GridTrackMin::Percent(v) => percent(v / 100.0),
        GridTrackMin::Auto => auto(),
        GridTrackMin::MinContent => min_content(),
        GridTrackMin::MaxContent => max_content(),
    }
}

fn max_track(value: GridTrackMax) -> MaxTrackSizingFunction {
    match value {
        GridTrackMax::Fr(v) => fr(v),
        GridTrackMax::Points(v) => length(v),
        GridTrackMax::Percent(v) => percent(v / 100.0),
        GridTrackMax::Auto => auto(),
        GridTrackMax::MinContent => min_content(),
        GridTrackMax::MaxContent => max_content(),
    }
}

/// One edge of a grid placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridLine {
    /// Auto-placed.
    #[default]
    Auto,
    /// A 1-based line index (negative counts from the end).
    Line(i16),
    /// Span this many tracks.
    Span(u16),
}

impl GridLine {
    pub(crate) fn is_valid(self) -> bool {
        !matches!(self, GridLine::Line(0) | GridLine::Span(0))
    }
}

/// An item's placement on one grid axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridPlacement {
    /// Start edge.
    pub start: GridLine,
    /// End edge.
    pub end: GridLine,
}

impl GridPlacement {
    /// Parse `<line> [ / <line> ]?`, including `auto`, negative lines and spans.
    pub fn parse(text: &str) -> Option<Self> {
        let mut sides = text.split('/');
        let start = parse_line(sides.next()?.trim())?;
        let end = match sides.next() {
            Some(value) => parse_line(value.trim())?,
            None => GridLine::Auto,
        };
        sides.next().is_none().then_some(Self { start, end })
    }

    /// The canonical CSS declaration value.
    pub fn css(self) -> String {
        format!("{} / {}", line_css(self.start), line_css(self.end))
    }

    pub(crate) fn is_valid(self) -> bool {
        self.start.is_valid() && self.end.is_valid()
    }
}

fn parse_line(text: &str) -> Option<GridLine> {
    if text == "auto" {
        return Some(GridLine::Auto);
    }
    if let Some(span) = text.strip_prefix("span ") {
        return span
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|n| *n > 0)
            .map(GridLine::Span);
    }
    text.parse::<i16>()
        .ok()
        .filter(|n| *n != 0)
        .map(GridLine::Line)
}

fn line_css(line: GridLine) -> String {
    match line {
        GridLine::Auto => "auto".into(),
        GridLine::Line(n) => n.to_string(),
        GridLine::Span(n) => format!("span {n}"),
    }
}

pub(crate) fn grid_line(value: GridLine) -> taffy::style::GridPlacement {
    match value {
        GridLine::Auto => taffy::style::GridPlacement::Auto,
        GridLine::Line(i) => line(i),
        GridLine::Span(n) => span(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_track_grammar_is_bounded_and_canonical() {
        let tracks =
            GridTracks::parse("repeat(2, 1fr minmax(80px, 25%)) auto min-content max-content")
                .unwrap();
        assert_eq!(tracks.0.len(), 7);
        assert_eq!(
            tracks.css(),
            "1fr minmax(80px, 25%) 1fr minmax(80px, 25%) auto min-content max-content"
        );
        assert!(GridTracks::parse("repeat(33, 1fr)").is_none());
        assert!(GridTracks::parse("minmax(1fr, 20px)").is_none());
        assert!(GridTracks::parse("-1px").is_none());
    }

    #[test]
    fn css_placement_carries_lines_spans_and_negative_lines() {
        assert_eq!(
            GridPlacement::parse("2 / span 3").unwrap().css(),
            "2 / span 3"
        );
        assert_eq!(GridPlacement::parse("-2").unwrap().css(), "-2 / auto");
        assert!(GridPlacement::parse("0 / auto").is_none());
        assert!(GridPlacement::parse("span 0").is_none());
    }
}
