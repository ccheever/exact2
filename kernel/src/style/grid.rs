//! CSS grid track and placement values.
//!
//! Taffy's parser supplies CSS tokenization (including escaped identifiers),
//! while this module applies the template-wide rules and keeps the portable
//! subset explicit. The same parser serves literals, runtime writes and wire
//! decode; the web only excludes the few valid forms Taffy cannot run.

use cssparser::{serialize_identifier, Parser, ParserInput, Token};
use taffy::prelude::{
    auto, fit_content, fr, length, line, max_content, min_content, minmax, percent, span,
};
use taffy::style::{
    ExpandedMaxTrackSizingFunction as TaffyMax, ExpandedMinTrackSizingFunction as TaffyMin,
    GridPlacement as TaffyPlacement, GridTemplateComponent as TaffyComponent,
    GridTemplateRepetition as TaffyRepeat, GridTemplateTracks as TaffyTracks,
    MaxTrackSizingFunction, MinTrackSizingFunction, RepetitionCount as TaffyRepeatCount,
    TrackSizingFunction,
};

use super::MAX_GRID_TRACKS;

/// The grid grammars, once linked ([`link`]). Only a wasm artifact reads
/// this table; native hosts and the compiler call the parsers directly.
#[cfg(target_arch = "wasm32")]
struct Parsers {
    tracks: fn(&str) -> Option<GridTracks>,
    placement: fn(&str) -> Option<GridPlacement>,
    idents: fn(&str) -> Option<Vec<String>>,
}

#[cfg(target_arch = "wasm32")]
static LINKED: std::sync::OnceLock<Parsers> = std::sync::OnceLock::new();

/// Link CSS grid's grammars (LLP 1047 D2, linked by use). A web artifact
/// links them when its plan binds any grid row, and admission refuses such a
/// plan before boot when they are absent (D6). Native artifacts and the
/// compiler parse directly.
pub fn link() {
    #[cfg(target_arch = "wasm32")]
    let _ = LINKED.set(Parsers {
        tracks: GridTracks::check,
        placement: GridPlacement::check,
        idents: parse_css_idents,
    });
}

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

/// The limit of `fit-content()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridFitContent {
    /// Layout points.
    Points(f32),
    /// Percent of the grid container, authored as 0–100.
    Percent(f32),
}

/// One non-repeated CSS grid track.
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
    /// `fit-content(<length-percentage>)`.
    FitContent(GridFitContent),
    /// Separate minimum and maximum track breadths.
    MinMax(GridTrackMin, GridTrackMax),
}

/// A `repeat()` count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridRepeatCount {
    /// A fixed positive repetition count.
    Count(u16),
    /// Fill the available space and retain empty tracks.
    AutoFill,
    /// Fill the available space and collapse empty tracks.
    AutoFit,
}

/// A preserved `repeat()` component. Automatic repetition cannot be expanded
/// before Taffy knows the grid container's available size.
#[derive(Debug, Clone, PartialEq)]
pub struct GridRepeat {
    /// Fixed, auto-fill or auto-fit repetition.
    pub count: GridRepeatCount,
    /// The non-repeated track list in the function.
    pub tracks: Vec<GridTrack>,
    /// Names on each line in the repeated fragment.
    pub line_names: Vec<Vec<String>>,
}

/// One component of a grid template.
#[derive(Debug, Clone, PartialEq)]
pub enum GridTrackComponent {
    /// One track.
    Single(GridTrack),
    /// A fixed or automatic repetition.
    Repeat(GridRepeat),
}

/// A grid template, including the names on its explicit lines.
#[derive(Debug, Clone, PartialEq)]
pub struct GridTracks {
    components: Vec<GridTrackComponent>,
    line_names: Vec<Vec<String>>,
    css: String,
}

impl Default for GridTracks {
    fn default() -> Self {
        Self {
            components: Vec::new(),
            line_names: Vec::new(),
            css: "none".into(),
        }
    }
}

impl GridTracks {
    /// Construct a template from individual tracks (the in-process test and
    /// host convenience form).
    pub fn from_tracks(tracks: Vec<GridTrack>) -> Self {
        let css = if tracks.is_empty() {
            "none".into()
        } else {
            tracks.iter().map(track_css).collect::<Vec<_>>().join(" ")
        };
        let line_names = vec![Vec::new(); tracks.len().saturating_add(1)];
        Self {
            components: tracks.into_iter().map(GridTrackComponent::Single).collect(),
            line_names,
            css,
        }
    }

    /// Parse the CSS grammar Taffy can lay out. `subgrid`, non-pixel CSS
    /// lengths and calculated lengths are valid CSS but have no Taffy value.
    pub fn parse(text: &str) -> Option<Self> {
        #[cfg(target_arch = "wasm32")]
        return LINKED.get().and_then(|parsers| (parsers.tracks)(text));
        #[cfg(not(target_arch = "wasm32"))]
        Self::check(text)
    }

    fn check(text: &str) -> Option<Self> {
        if parse_css_idents(text)
            .is_some_and(|words| words.len() == 1 && words[0].eq_ignore_ascii_case("none"))
        {
            return Some(Self::default());
        }
        let parsed = text
            .parse::<TaffyTracks<String, TaffyComponent<String>>>()
            .ok()?;
        let components = parsed
            .tracks
            .into_iter()
            .map(component_from_taffy)
            .collect::<Option<Vec<_>>>()?;
        let line_names = parsed.line_names;
        let css = template_css(&components, &line_names);
        let value = Self {
            components,
            line_names,
            css,
        };
        value.is_valid().then_some(value)
    }

    /// The canonical CSS declaration value.
    pub fn css(&self) -> &str {
        &self.css
    }

    /// Heap-bearing entries used by native host packet sizing. The CSS text
    /// itself is exposed separately by [`Self::css`].
    pub fn storage_len(&self) -> usize {
        self.components.len()
            + self.line_names.iter().map(Vec::len).sum::<usize>()
            + self
                .components
                .iter()
                .map(|component| match component {
                    GridTrackComponent::Single(_) => 0,
                    GridTrackComponent::Repeat(repeat) => {
                        repeat.tracks.len() + repeat.line_names.iter().map(Vec::len).sum::<usize>()
                    }
                })
                .sum::<usize>()
    }

    /// Whether every track size is finite.
    pub fn is_finite(&self) -> bool {
        self.components.iter().all(component_finite)
    }

    /// Whether this is a structurally valid template in Taffy's supported CSS
    /// subset, bounded by the engine's explicit-grid limit.
    pub(crate) fn is_valid(&self) -> bool {
        if self.components.is_empty() {
            return self.css.eq_ignore_ascii_case("none");
        }
        if self.line_names.len() != self.components.len().saturating_add(1)
            || !self
                .line_names
                .iter()
                .flatten()
                .all(|name| valid_grid_ident(name))
            || !self.components.iter().all(component_valid)
        {
            return false;
        }
        let mut explicit_tracks = 0usize;
        for component in &self.components {
            let count = match component {
                GridTrackComponent::Single(_) => 1,
                GridTrackComponent::Repeat(GridRepeat {
                    count: GridRepeatCount::Count(count),
                    tracks,
                    ..
                }) => usize::from(*count).saturating_mul(tracks.len()),
                GridTrackComponent::Repeat(GridRepeat { tracks, .. }) => tracks.len(),
            };
            explicit_tracks = explicit_tracks.saturating_add(count);
            if explicit_tracks > MAX_GRID_TRACKS {
                return false;
            }
        }
        let auto_repeats = self
            .components
            .iter()
            .filter(|component| {
                matches!(
                    component,
                    GridTrackComponent::Repeat(GridRepeat {
                        count: GridRepeatCount::AutoFill | GridRepeatCount::AutoFit,
                        ..
                    })
                )
            })
            .count();
        auto_repeats == 0 || (auto_repeats == 1 && self.components.iter().all(component_is_fixed))
    }

    /// `count` equal `1fr` tracks.
    pub fn equal(count: usize) -> Self {
        Self::from_tracks(vec![GridTrack::Fr(1.0); count.min(MAX_GRID_TRACKS)])
    }

    pub(crate) fn taffy_components(&self) -> Vec<TaffyComponent<String>> {
        self.components.iter().map(component_to_taffy).collect()
    }

    pub(crate) fn line_names(&self) -> Vec<Vec<String>> {
        self.line_names.clone()
    }
}

fn component_from_taffy(value: TaffyComponent<String>) -> Option<GridTrackComponent> {
    match value {
        TaffyComponent::Single(track) => track_from_taffy(track).map(GridTrackComponent::Single),
        TaffyComponent::Repeat(repeat) => Some(GridTrackComponent::Repeat(GridRepeat {
            count: match repeat.count {
                TaffyRepeatCount::Count(n) => GridRepeatCount::Count(n),
                TaffyRepeatCount::AutoFill => GridRepeatCount::AutoFill,
                TaffyRepeatCount::AutoFit => GridRepeatCount::AutoFit,
            },
            tracks: repeat
                .tracks
                .into_iter()
                .map(track_from_taffy)
                .collect::<Option<Vec<_>>>()?,
            line_names: repeat.line_names,
        })),
    }
}

fn component_to_taffy(value: &GridTrackComponent) -> TaffyComponent<String> {
    match value {
        GridTrackComponent::Single(track) => TaffyComponent::Single(track_to_taffy(*track)),
        GridTrackComponent::Repeat(repeat) => TaffyComponent::Repeat(TaffyRepeat {
            count: match repeat.count {
                GridRepeatCount::Count(n) => TaffyRepeatCount::Count(n),
                GridRepeatCount::AutoFill => TaffyRepeatCount::AutoFill,
                GridRepeatCount::AutoFit => TaffyRepeatCount::AutoFit,
            },
            tracks: repeat.tracks.iter().copied().map(track_to_taffy).collect(),
            line_names: repeat.line_names.clone(),
        }),
    }
}

#[derive(Clone, Copy)]
enum GridTrackMaxExpanded {
    Track(GridTrackMax),
    FitContentPoints(f32),
    FitContentPercent(f32),
}

fn track_from_taffy(value: TrackSizingFunction) -> Option<GridTrack> {
    let min = min_from_taffy(value.min_sizing_function().expand())?;
    let max = max_from_taffy(value.max_sizing_function().expand())?;
    Some(match (min, max) {
        (_, GridTrackMaxExpanded::FitContentPoints(v)) => {
            GridTrack::FitContent(GridFitContent::Points(v))
        }
        (_, GridTrackMaxExpanded::FitContentPercent(v)) => {
            GridTrack::FitContent(GridFitContent::Percent(v))
        }
        (GridTrackMin::Auto, GridTrackMaxExpanded::Track(GridTrackMax::Fr(v))) => GridTrack::Fr(v),
        (GridTrackMin::Auto, GridTrackMaxExpanded::Track(GridTrackMax::Auto)) => GridTrack::Auto,
        (GridTrackMin::MinContent, GridTrackMaxExpanded::Track(GridTrackMax::MinContent)) => {
            GridTrack::MinContent
        }
        (GridTrackMin::MaxContent, GridTrackMaxExpanded::Track(GridTrackMax::MaxContent)) => {
            GridTrack::MaxContent
        }
        (GridTrackMin::Points(a), GridTrackMaxExpanded::Track(GridTrackMax::Points(b)))
            if a == b =>
        {
            GridTrack::Points(a)
        }
        (GridTrackMin::Percent(a), GridTrackMaxExpanded::Track(GridTrackMax::Percent(b)))
            if a == b =>
        {
            GridTrack::Percent(a)
        }
        (min, GridTrackMaxExpanded::Track(max)) => GridTrack::MinMax(min, max),
    })
}

fn min_from_taffy(value: TaffyMin) -> Option<GridTrackMin> {
    Some(match value {
        TaffyMin::Length(v) => GridTrackMin::Points(v),
        TaffyMin::Percent(v) => GridTrackMin::Percent(v * 100.0),
        TaffyMin::Auto => GridTrackMin::Auto,
        TaffyMin::MinContent => GridTrackMin::MinContent,
        TaffyMin::MaxContent => GridTrackMin::MaxContent,
        TaffyMin::Calc(_) => return None,
    })
}

fn max_from_taffy(value: TaffyMax) -> Option<GridTrackMaxExpanded> {
    Some(match value {
        TaffyMax::Fr(v) => GridTrackMaxExpanded::Track(GridTrackMax::Fr(v)),
        TaffyMax::Length(v) => GridTrackMaxExpanded::Track(GridTrackMax::Points(v)),
        TaffyMax::Percent(v) => GridTrackMaxExpanded::Track(GridTrackMax::Percent(v * 100.0)),
        TaffyMax::Auto => GridTrackMaxExpanded::Track(GridTrackMax::Auto),
        TaffyMax::MinContent => GridTrackMaxExpanded::Track(GridTrackMax::MinContent),
        TaffyMax::MaxContent => GridTrackMaxExpanded::Track(GridTrackMax::MaxContent),
        TaffyMax::FitContentPx(v) => GridTrackMaxExpanded::FitContentPoints(v),
        TaffyMax::FitContentPercent(v) => GridTrackMaxExpanded::FitContentPercent(v * 100.0),
        TaffyMax::Calc(_) => return None,
    })
}

fn component_finite(value: &GridTrackComponent) -> bool {
    match value {
        GridTrackComponent::Single(track) => track_finite(*track),
        GridTrackComponent::Repeat(repeat) => repeat.tracks.iter().copied().all(track_finite),
    }
}

fn component_valid(value: &GridTrackComponent) -> bool {
    match value {
        GridTrackComponent::Single(track) => track_valid(*track),
        GridTrackComponent::Repeat(repeat) => {
            !repeat.tracks.is_empty()
                && repeat.line_names.len() == repeat.tracks.len().saturating_add(1)
                && repeat
                    .line_names
                    .iter()
                    .flatten()
                    .all(|name| valid_grid_ident(name))
                && !matches!(repeat.count, GridRepeatCount::Count(0))
                && repeat.tracks.iter().copied().all(track_valid)
        }
    }
}

fn template_css(components: &[GridTrackComponent], line_names: &[Vec<String>]) -> String {
    if components.is_empty() {
        return "none".into();
    }
    track_list_css(components, line_names)
}

fn track_list_css(components: &[GridTrackComponent], line_names: &[Vec<String>]) -> String {
    let mut parts = Vec::with_capacity(components.len().saturating_mul(2).saturating_add(1));
    for (index, component) in components.iter().enumerate() {
        if let Some(names) = line_names.get(index).filter(|names| !names.is_empty()) {
            parts.push(line_names_css(names));
        }
        parts.push(component_css(component));
    }
    if let Some(names) = line_names.last().filter(|names| !names.is_empty()) {
        parts.push(line_names_css(names));
    }
    parts.join(" ")
}

fn component_css(value: &GridTrackComponent) -> String {
    match value {
        GridTrackComponent::Single(track) => track_css(track),
        GridTrackComponent::Repeat(repeat) => {
            let count = match repeat.count {
                GridRepeatCount::Count(count) => count.to_string(),
                GridRepeatCount::AutoFill => "auto-fill".into(),
                GridRepeatCount::AutoFit => "auto-fit".into(),
            };
            let components = repeat
                .tracks
                .iter()
                .copied()
                .map(GridTrackComponent::Single)
                .collect::<Vec<_>>();
            format!(
                "repeat({count}, {})",
                track_list_css(&components, &repeat.line_names)
            )
        }
    }
}

fn line_names_css(names: &[String]) -> String {
    let mut out = String::from("[");
    for (index, name) in names.iter().enumerate() {
        if index != 0 {
            out.push(' ');
        }
        serialize_identifier(name, &mut out).expect("writing CSS to a String cannot fail");
    }
    out.push(']');
    out
}

fn component_is_fixed(value: &GridTrackComponent) -> bool {
    match value {
        GridTrackComponent::Single(track) => track_is_fixed(*track),
        GridTrackComponent::Repeat(repeat) => repeat.tracks.iter().copied().all(track_is_fixed),
    }
}

fn track_is_fixed(value: GridTrack) -> bool {
    match value {
        GridTrack::Points(_) | GridTrack::Percent(_) => true,
        GridTrack::MinMax(min, max) => {
            matches!(min, GridTrackMin::Points(_) | GridTrackMin::Percent(_))
                || matches!(max, GridTrackMax::Points(_) | GridTrackMax::Percent(_))
        }
        GridTrack::Fr(_)
        | GridTrack::Auto
        | GridTrack::MinContent
        | GridTrack::MaxContent
        | GridTrack::FitContent(_) => false,
    }
}

fn track_finite(value: GridTrack) -> bool {
    match value {
        GridTrack::Fr(v) | GridTrack::Points(v) | GridTrack::Percent(v) => v.is_finite(),
        GridTrack::FitContent(GridFitContent::Points(v) | GridFitContent::Percent(v)) => {
            v.is_finite()
        }
        GridTrack::MinMax(min, max) => min_finite(min) && max_finite(max),
        GridTrack::Auto | GridTrack::MinContent | GridTrack::MaxContent => true,
    }
}

fn track_valid(value: GridTrack) -> bool {
    track_finite(value)
        && match value {
            GridTrack::Fr(v) | GridTrack::Points(v) | GridTrack::Percent(v) => v >= 0.0,
            GridTrack::FitContent(GridFitContent::Points(v) | GridFitContent::Percent(v)) => {
                v >= 0.0
            }
            GridTrack::MinMax(min, max) => min_nonnegative(min) && max_nonnegative(max),
            GridTrack::Auto | GridTrack::MinContent | GridTrack::MaxContent => true,
        }
}

fn scalar(value: f32, suffix: &str) -> String {
    format!("{}{suffix}", exact_num::Shortest32(value))
}

fn track_css(value: &GridTrack) -> String {
    match *value {
        GridTrack::Fr(v) => scalar(v, "fr"),
        GridTrack::Points(v) => scalar(v, "px"),
        GridTrack::Percent(v) => scalar(v, "%"),
        GridTrack::Auto => "auto".into(),
        GridTrack::MinContent => "min-content".into(),
        GridTrack::MaxContent => "max-content".into(),
        GridTrack::FitContent(GridFitContent::Points(v)) => {
            format!("fit-content({})", scalar(v, "px"))
        }
        GridTrack::FitContent(GridFitContent::Percent(v)) => {
            format!("fit-content({})", scalar(v, "%"))
        }
        GridTrack::MinMax(min, max) => format!("minmax({}, {})", min_css(min), max_css(max)),
    }
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

fn track_to_taffy(value: GridTrack) -> TrackSizingFunction {
    match value {
        GridTrack::Fr(v) => fr(v),
        GridTrack::Points(v) => length(v),
        GridTrack::Percent(v) => percent(v / 100.0),
        GridTrack::Auto => auto(),
        GridTrack::MinContent => min_content(),
        GridTrack::MaxContent => max_content(),
        GridTrack::FitContent(GridFitContent::Points(v)) => fit_content(length(v)),
        GridTrack::FitContent(GridFitContent::Percent(v)) => fit_content(percent(v / 100.0)),
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GridLine {
    /// Auto-placed.
    #[default]
    Auto,
    /// A 1-based line index (negative counts from the end).
    Line(i16),
    /// The nth line with a custom name (`0` means the first).
    NamedLine(String, i16),
    /// Span this many tracks.
    Span(u16),
    /// Span to the nth line with a custom name (`0` means the first).
    NamedSpan(String, u16),
}

impl GridLine {
    pub(crate) fn is_valid(&self) -> bool {
        match self {
            GridLine::Line(line) => *line != 0 && line.unsigned_abs() as usize <= MAX_GRID_TRACKS,
            GridLine::Span(span) => *span != 0 && usize::from(*span) <= MAX_GRID_TRACKS,
            GridLine::NamedLine(name, line) => {
                valid_grid_ident(name) && line.unsigned_abs() as usize <= MAX_GRID_TRACKS
            }
            GridLine::NamedSpan(name, span) => {
                valid_grid_ident(name) && usize::from(*span) <= MAX_GRID_TRACKS
            }
            GridLine::Auto => true,
        }
    }
}

/// An item's placement on one grid axis.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GridPlacement {
    /// Start edge.
    pub start: GridLine,
    /// End edge.
    pub end: GridLine,
}

impl GridPlacement {
    /// Parse `<grid-line> [ / <grid-line> ]?`, including named lines.
    pub fn parse(text: &str) -> Option<Self> {
        #[cfg(target_arch = "wasm32")]
        return LINKED.get().and_then(|parsers| (parsers.placement)(text));
        #[cfg(not(target_arch = "wasm32"))]
        Self::check(text)
    }

    fn check(text: &str) -> Option<Self> {
        let (start, end) = placement_tokens(text)?;
        let end = match end {
            Some(value) => value,
            // CSS Grid §7.3.1: an omitted end copies a bare custom-ident;
            // every other start defaults the end to auto.
            None => match &start {
                GridLine::NamedLine(name, 0) => GridLine::NamedLine(name.clone(), 0),
                _ => GridLine::Auto,
            },
        };
        let value = Self { start, end };
        value.is_valid().then_some(value)
    }

    /// Construct a placement from its typed lines.
    pub fn from_lines(start: GridLine, end: GridLine) -> Self {
        Self { start, end }
    }

    /// The canonical CSS declaration value.
    pub fn css(&self) -> String {
        let omission_keeps_meaning = match (&self.start, &self.end) {
            (GridLine::NamedLine(start, 0), GridLine::NamedLine(end, 0)) => start == end,
            (GridLine::NamedLine(_, 0), GridLine::Auto) => false,
            (_, GridLine::Auto) => true,
            _ => false,
        };
        if omission_keeps_meaning {
            line_css(&self.start)
        } else {
            format!("{} / {}", line_css(&self.start), line_css(&self.end))
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.start.is_valid() && self.end.is_valid()
    }
}

fn valid_grid_ident(name: &str) -> bool {
    ![
        "auto",
        "span",
        "default",
        "inherit",
        "initial",
        "unset",
        "revert",
        "revert-layer",
    ]
    .iter()
    .any(|keyword| name.eq_ignore_ascii_case(keyword))
}

#[derive(Default)]
struct PlacementPart {
    span: bool,
    number: Option<i32>,
    ident: Option<String>,
}

impl PlacementPart {
    fn push(&mut self, token: Token<'_>) -> Option<()> {
        match token {
            Token::Ident(ident) if ident.eq_ignore_ascii_case("auto") => {
                if self.span || self.number.is_some() || self.ident.is_some() {
                    return None;
                }
                self.ident = Some("auto".into());
            }
            Token::Ident(ident) if ident.eq_ignore_ascii_case("span") => {
                if self.span || self.ident.as_deref() == Some("auto") {
                    return None;
                }
                self.span = true;
            }
            Token::Ident(ident) => {
                if self.ident.is_some() || !valid_grid_ident(&ident) {
                    return None;
                }
                self.ident = Some(ident.as_ref().to_string());
            }
            Token::Number {
                int_value: Some(value),
                ..
            } if value != 0 && self.number.is_none() => self.number = Some(value),
            _ => return None,
        }
        Some(())
    }

    fn finish(self) -> Option<GridLine> {
        if self.ident.as_deref() == Some("auto") {
            return (!self.span && self.number.is_none()).then_some(GridLine::Auto);
        }
        let number = self.number.unwrap_or(0);
        if self.span {
            let count = u16::try_from(number).ok()?;
            if usize::from(count) > MAX_GRID_TRACKS {
                return None;
            }
            return match (count, self.ident) {
                (0, None) => None,
                (count, None) => Some(GridLine::Span(count)),
                (count, Some(name)) => Some(GridLine::NamedSpan(name, count)),
            };
        }
        if number.unsigned_abs() as usize > MAX_GRID_TRACKS {
            return None;
        }
        match (number, self.ident) {
            (0, None) => None,
            (number, None) => Some(GridLine::Line(i16::try_from(number).ok()?)),
            (number, Some(name)) => Some(GridLine::NamedLine(name, i16::try_from(number).ok()?)),
        }
    }
}

fn placement_tokens(text: &str) -> Option<(GridLine, Option<GridLine>)> {
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let mut parts = [PlacementPart::default(), PlacementPart::default()];
    let mut side = 0usize;
    while !parser.is_exhausted() {
        let token = parser.next().ok()?.clone();
        if token == Token::Delim('/') {
            if side != 0 || parts[0].finish_ref().is_none() {
                return None;
            }
            side = 1;
        } else {
            parts[side].push(token)?;
        }
    }
    let [start, end] = parts;
    let end = if side == 1 { Some(end.finish()?) } else { None };
    Some((start.finish()?, end))
}

impl PlacementPart {
    fn finish_ref(&self) -> Option<()> {
        (self.span || self.number.is_some() || self.ident.is_some()).then_some(())
    }
}

pub(super) fn css_idents(text: &str) -> Option<Vec<String>> {
    #[cfg(target_arch = "wasm32")]
    return LINKED.get().and_then(|parsers| (parsers.idents)(text));
    #[cfg(not(target_arch = "wasm32"))]
    parse_css_idents(text)
}

fn parse_css_idents(text: &str) -> Option<Vec<String>> {
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let mut words = Vec::new();
    while !parser.is_exhausted() {
        match parser.next().ok()?.clone() {
            Token::Ident(ident) => words.push(ident.as_ref().to_string()),
            _ => return None,
        }
    }
    (!words.is_empty()).then_some(words)
}

fn css_ident(name: &str) -> String {
    let mut out = String::new();
    serialize_identifier(name, &mut out).expect("writing CSS to a String cannot fail");
    out
}

fn line_css(value: &GridLine) -> String {
    match value {
        GridLine::Auto => "auto".into(),
        GridLine::Line(n) => n.to_string(),
        GridLine::NamedLine(name, 0) => css_ident(name),
        GridLine::NamedLine(name, n) => format!("{n} {}", css_ident(name)),
        GridLine::Span(n) => format!("span {n}"),
        GridLine::NamedSpan(name, 0) => format!("span {}", css_ident(name)),
        GridLine::NamedSpan(name, n) => format!("span {n} {}", css_ident(name)),
    }
}

pub(crate) fn grid_line(value: &GridLine) -> TaffyPlacement<String> {
    match value {
        GridLine::Auto => TaffyPlacement::Auto,
        GridLine::Line(i) => line(*i),
        GridLine::NamedLine(name, i) => TaffyPlacement::NamedLine(name.clone(), *i),
        GridLine::Span(n) => span(*n),
        GridLine::NamedSpan(name, n) => TaffyPlacement::NamedSpan(name.clone(), *n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_track_grammar_carries_taffys_css_forms() {
        for value in [
            "auto",
            "25%",
            "100PX",
            "fit-content(40px)",
            "fit-content(0)",
            "minmax(0, 1fr)",
            "repeat(auto-fit, minmax(80px, 1fr))",
            "repeat(auto-fit, minmax(0, 1fr))",
            "[start] 40px [middle] 1fr [end]",
            "[em] 40px",
            "1e2%",
            "minmax(1e2px, 1fr)",
        ] {
            assert!(GridTracks::parse(value).is_some(), "{value}");
        }
        for value in [
            "repeat(2, repeat(2, 40px))",
            "minmax(1fr, 20px)",
            "minmax(auto, fit-content(40px))",
            "1fr repeat(auto-fit, 40px)",
            "subgrid",
            "[auto] 1fr",
            "repeat(10001, 1px)",
            "repeat(6000, 1px) repeat(6000, 1px)",
        ] {
            assert!(GridTracks::parse(value).is_none(), "{value}");
        }
        assert_eq!(
            GridTracks::parse("[\\31 foo] 100PX repeat(2, [mid] minmax(0, 1FR))")
                .unwrap()
                .css(),
            "[\\31 foo] 100px repeat(2, [mid] minmax(0px, 1fr))"
        );
    }

    #[test]
    fn an_over_u16_repeat_fragment_is_refused_without_reaching_layout() {
        let tracks = std::iter::repeat_n("1px", usize::from(u16::MAX) + 1)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(GridTracks::parse(&format!("repeat(1, {tracks})")).is_none());
    }

    #[test]
    fn css_placement_carries_numbers_spans_and_named_lines() {
        for value in [
            "2",
            "AUTO",
            "auto / SPAN 2",
            "2 rail / span 3 rail",
            "rail / span rail",
            "2 /* gap */ / span 2",
        ] {
            assert!(GridPlacement::parse(value).is_some(), "{value}");
        }
        assert!(GridPlacement::parse("0 / auto").is_none());
        assert!(GridPlacement::parse("span 0").is_none());
        assert!(GridPlacement::parse("inherit").is_none());
        assert!(GridPlacement::parse("2 inherit").is_none());
        assert!(GridPlacement::parse("1 / 2 / 3").is_none());
        assert!(GridPlacement::parse("10001").is_none());
        assert!(GridPlacement::parse("span 10001").is_none());
        assert_eq!(GridPlacement::parse("rail").unwrap().css(), "rail");
        assert_eq!(
            GridPlacement::parse("rail / auto").unwrap().css(),
            "rail / auto"
        );
        assert_eq!(
            GridPlacement::parse("\\31 foo / 2").unwrap().css(),
            "\\31 foo / 2"
        );
        assert_eq!(GridPlacement::parse("2 rail").unwrap().css(), "2 rail");
    }
}
