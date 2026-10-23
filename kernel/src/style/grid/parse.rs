//! Extend Taffy's track grammar with the kernel's length calculations.

use super::*;
use taffy::{CssParseResult, FromCss};

// Carry the wire's calculation limits through Taffy's generic parser.
#[derive(Debug)]
struct Parsed<T, const CANONICAL: bool>(T);

pub(super) fn tracks(
    text: &str,
    canonical: bool,
) -> Option<TaffyTracks<String, GridTrackComponent>> {
    if canonical {
        template::<true>(text)
    } else {
        template::<false>(text)
    }
}

fn template<const CANONICAL: bool>(text: &str) -> Option<TaffyTracks<String, GridTrackComponent>> {
    let parsed = text
        .parse::<TaffyTracks<String, Parsed<GridTrackComponent, CANONICAL>>>()
        .ok()?;
    Some(TaffyTracks {
        tracks: parsed.tracks.into_iter().map(|p| p.0).collect(),
        line_names: parsed.line_names,
    })
}

impl<const CANONICAL: bool> FromCss for Parsed<GridTrackComponent, CANONICAL> {
    fn from_css<'i>(parser: &mut Parser<'i, '_>) -> CssParseResult<'i, Self> {
        if parser
            .try_parse(|p| p.expect_function_matching("repeat"))
            .is_ok()
        {
            return parser.parse_nested_block(|p| {
                let count = match TaffyRepeatCount::from_css(p)? {
                    TaffyRepeatCount::Count(n) => GridRepeatCount::Count(n),
                    TaffyRepeatCount::AutoFill => GridRepeatCount::AutoFill,
                    TaffyRepeatCount::AutoFit => GridRepeatCount::AutoFit,
                };
                p.expect_comma()?;
                let tracks = TaffyTracks::<String, Parsed<GridTrack, CANONICAL>>::from_css(p)?;
                Ok(Self(GridTrackComponent::Repeat(GridRepeat {
                    count,
                    tracks: tracks.tracks.into_iter().map(|p| p.0).collect(),
                    line_names: tracks.line_names,
                })))
            });
        }
        Parsed::<GridTrack, CANONICAL>::from_css(parser)
            .map(|p| Self(GridTrackComponent::Single(p.0)))
    }
}

impl<const CANONICAL: bool> FromCss for Parsed<GridTrack, CANONICAL> {
    fn from_css<'i>(parser: &mut Parser<'i, '_>) -> CssParseResult<'i, Self> {
        if parser
            .try_parse(|p| p.expect_function_matching("minmax"))
            .is_ok()
        {
            return parser.parse_nested_block(|p| {
                let min = if let Ok(c) = p.try_parse(calculation::<CANONICAL>) {
                    GridTrackMin::Calc(c)
                } else {
                    min_from_taffy(taffy::MinTrackSizingFunction::from_css(p)?.expand())
                        .ok_or_else(|| p.new_custom_error("unsupported minimum"))?
                };
                p.expect_comma()?;
                let max = if let Ok(c) = p.try_parse(calculation::<CANONICAL>) {
                    GridTrackMax::Calc(c)
                } else {
                    match max_from_taffy(taffy::MaxTrackSizingFunction::from_css(p)?.expand()) {
                        Some(GridTrackMaxExpanded::Track(max)) => max,
                        _ => return Err(p.new_custom_error("unsupported maximum")),
                    }
                };
                Ok(Self(GridTrack::MinMax(min, max)))
            });
        }
        if let Ok(c) = parser.try_parse(calculation::<CANONICAL>) {
            return Ok(Self(GridTrack::Calc(c)));
        }
        track_from_taffy(TrackSizingFunction::from_css(parser)?)
            .map(Self)
            .ok_or_else(|| parser.new_custom_error("unsupported track"))
    }
}

fn calculation<'i, const CANONICAL: bool>(
    p: &mut Parser<'i, '_>,
) -> CssParseResult<'i, Comparison> {
    let start = p.position();
    let name = p.expect_function()?.to_ascii_lowercase();
    if !matches!(name.as_str(), "calc" | "min" | "max" | "clamp") {
        return Err(p.new_custom_error("expected a length calculation"));
    }
    p.parse_nested_block(|p| {
        while p.next_including_whitespace_and_comments().is_ok() {}
        Ok(())
    })?;
    super::super::calc::grid(p.slice_from(start), CANONICAL)
        .ok_or_else(|| p.new_custom_error("expected a px or % calculation"))
}
