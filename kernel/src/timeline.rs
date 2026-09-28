//! Animation timelines a drag drives (LLP 1057.003 D1, D4).
//!
//! CSS scroll-driven animations (CSS Animations 2, Scroll-driven Animations
//! §3) name a timeline on a scroller (`scroll-timeline: <name> <axis>`) and
//! bind a node's `animation`s to it (`animation-timeline: <name>`), over a
//! stretch of it (`animation-range`). A drag has no CSS timeline; this is
//! the same shape with a drag as the source, a deviation LLP 1001 declares:
//!
//! - `drag-timeline: none | <dashed-ident> [x | y]?` on the node whose held
//!   translate a drag moves: the timeline's position is that translate on
//!   the axis (`y` if unsaid), as presented, so a release's spring moves it
//!   too.
//! - `animation-timeline: auto | <dashed-ident>` on a consumer: its
//!   `animation`s follow the named timeline instead of the clock.
//! - `animation-range: normal | <length> <length>` on the consumer: the
//!   positions where its animations are at 0% and 100%; outside them the
//!   progress clamps.
//! - `timeline-scope: none | all | <dashed-ident>#`, CSS's: names declared
//!   below a node, in scope for the node's subtree.
//!
//! The engine evaluates a consumer in the frame its source moves (D2); no
//! app code runs per frame. `animation-range` takes lengths only: a drag
//! has no scroll range for CSS's `cover` or percentages to name. Names
//! resolve as CSS resolves them, in the kernel ([`lookup`], D4), so the
//! engine hears a node, never a name.
//!
//! The grammar and the lookup are linked by use (LLP 1047 D2): until a host
//! calls [`link`], a text value for any of the four rows is refused as a
//! bad value and no name resolves. The compiler and the native hosts link
//! it at start; a web artifact links it when its plan sets one of the rows,
//! which a plan that sets none never reaches (D6).

mod lookup;
/// What a name resolves to, as the engine takes it ([`crate::Kernel::timeline_of`]).
pub use exact_motion::NamedTimeline;
pub(crate) use lookup::{refresh, rows, Registry};

use std::fmt::Write;

/// The four rows' grammar and the name lookup, once linked ([`link`]).
static LINKED: std::sync::OnceLock<Grammar> = std::sync::OnceLock::new();

struct Grammar {
    drag: fn(&str) -> Option<DragTimeline>,
    timeline: fn(&str) -> Option<AnimationTimeline>,
    range: fn(&str) -> Option<AnimationRange>,
    scope: fn(&str) -> Option<TimelineScope>,
    resolve: lookup::Resolve,
}

/// Link the four rows' grammar and the name lookup into this artifact.
pub fn link() {
    let _ = LINKED.set(Grammar {
        drag: DragTimeline::parse_text,
        timeline: AnimationTimeline::parse_text,
        range: AnimationRange::parse_text,
        scope: TimelineScope::parse_text,
        resolve: lookup::resolve,
    });
}

/// The axis of a held translate a drag timeline reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Axis {
    /// The horizontal translate.
    X,
    /// The vertical translate.
    #[default]
    Y,
}

/// `drag-timeline`: the timeline a node's held translate drives.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DragTimeline {
    /// The timeline's name, a `<dashed-ident>`; `None` is `none`.
    pub name: Option<String>,
    /// The axis it reads.
    pub axis: Axis,
}

fn dashed(token: &str) -> Option<String> {
    let rest = token.strip_prefix("--")?;
    (!rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    .then(|| token.to_string())
}

impl DragTimeline {
    /// `none`, or a `<dashed-ident>` and an optional axis; `None` also
    /// while the grammar is unlinked ([`link`]).
    pub fn parse(css: &str) -> Option<Self> {
        (LINKED.get()?.drag)(css)
    }

    fn parse_text(css: &str) -> Option<Self> {
        let mut tokens = css.split_ascii_whitespace();
        let first = tokens.next()?;
        if first.eq_ignore_ascii_case("none") {
            return tokens.next().is_none().then(Self::default);
        }
        let name = dashed(first)?;
        let axis = match tokens.next() {
            None => Axis::Y,
            Some(a) if a.eq_ignore_ascii_case("y") || a.eq_ignore_ascii_case("block") => Axis::Y,
            Some(a) if a.eq_ignore_ascii_case("x") || a.eq_ignore_ascii_case("inline") => Axis::X,
            Some(_) => return None,
        };
        tokens.next().is_none().then_some(Self {
            name: Some(name),
            axis,
        })
    }

    /// The declaration's value.
    pub fn css(&self) -> String {
        match &self.name {
            None => "none".into(),
            Some(n) => format!("{n} {}", if self.axis == Axis::X { "x" } else { "y" }),
        }
    }
}

/// `animation-timeline`: `auto` (the clock) or a named timeline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnimationTimeline(pub Option<String>);

impl AnimationTimeline {
    /// `auto` or a `<dashed-ident>`; `None` also while unlinked.
    pub fn parse(css: &str) -> Option<Self> {
        (LINKED.get()?.timeline)(css)
    }

    fn parse_text(css: &str) -> Option<Self> {
        let t = css.trim();
        if t.eq_ignore_ascii_case("auto") {
            return Some(Self(None));
        }
        dashed(t).map(|n| Self(Some(n)))
    }

    /// The declaration's value.
    pub fn css(&self) -> String {
        self.0.clone().unwrap_or_else(|| "auto".into())
    }
}

/// `animation-range`: where on the timeline the animations run from 0% to
/// 100%, in points; `None` is `normal`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AnimationRange(pub Option<[f32; 2]>);

fn length(token: &str) -> Option<f32> {
    let n = token.strip_suffix("px").unwrap_or(token);
    let v: f32 = n.parse().ok()?;
    v.is_finite().then_some(v)
}

impl AnimationRange {
    /// `normal`, or two lengths (px, or unitless points); `None` also
    /// while unlinked.
    pub fn parse(css: &str) -> Option<Self> {
        (LINKED.get()?.range)(css)
    }

    fn parse_text(css: &str) -> Option<Self> {
        let mut tokens = css.split_ascii_whitespace();
        let first = tokens.next()?;
        if first.eq_ignore_ascii_case("normal") {
            return tokens.next().is_none().then(Self::default);
        }
        let start = length(first)?;
        let end = length(tokens.next()?)?;
        (tokens.next().is_none() && start != end).then_some(Self(Some([start, end])))
    }

    /// The declaration's value.
    pub fn css(&self) -> String {
        match self.0 {
            None => "normal".into(),
            Some([a, b]) => {
                let mut out = String::new();
                let _ = write!(
                    out,
                    "{}px {}px",
                    exact_num::Shortest32(a),
                    exact_num::Shortest32(b)
                );
                out
            }
        }
    }

    /// The progress, 0 to 1, at timeline position `at`; `None` for `normal`.
    pub fn progress(&self, at: f32) -> Option<f32> {
        let [a, b] = self.0?;
        Some(((at - a) / (b - a)).clamp(0.0, 1.0))
    }
}

/// `timeline-scope` (Scroll-driven Animations 1 §4.2): the timeline names a
/// node declares in scope for its subtree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TimelineScope {
    /// `none`: no change in scope.
    #[default]
    None,
    /// `all`: every name a descendant declares.
    All,
    /// A list of `<dashed-ident>`s, as its canonical text: `--a, --b`. One
    /// string, not a list of them, is less code in every artifact that
    /// carries the row and sets none (LLP 1047 D9).
    Names(String),
}

impl TimelineScope {
    /// `none`, `all`, or a comma-separated list of `<dashed-ident>`s;
    /// `None` also while unlinked.
    pub fn parse(css: &str) -> Option<Self> {
        (LINKED.get()?.scope)(css)
    }

    fn parse_text(css: &str) -> Option<Self> {
        let t = css.trim();
        if t.eq_ignore_ascii_case("none") {
            return Some(Self::None);
        }
        if t.eq_ignore_ascii_case("all") {
            return Some(Self::All);
        }
        let mut names = String::new();
        for name in t.split(',') {
            if !names.is_empty() {
                names.push_str(", ");
            }
            names.push_str(&dashed(name.trim())?);
        }
        Some(Self::Names(names))
    }

    /// The declaration's value.
    pub fn css(&self) -> String {
        match self {
            Self::None => "none".into(),
            Self::All => "all".into(),
            Self::Names(names) => names.clone(),
        }
    }

    /// Whether it scopes `name`: `all` scopes every name.
    fn scopes(&self, name: &str) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Names(names) => names.split(", ").any(|n| n == name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_rows_round_trip() {
        link();
        let d = DragTimeline::parse("--dismiss y").unwrap();
        assert_eq!(d.name.as_deref(), Some("--dismiss"));
        assert_eq!(DragTimeline::parse(&d.css()), Some(d));
        assert_eq!(DragTimeline::parse("--pan x").unwrap().axis, Axis::X);
        assert_eq!(DragTimeline::parse("none"), Some(DragTimeline::default()));
        assert_eq!(DragTimeline::parse("dismiss"), None);
        assert_eq!(
            AnimationTimeline::parse("--dismiss").unwrap().css(),
            "--dismiss"
        );
        assert_eq!(
            AnimationTimeline::parse("auto"),
            Some(AnimationTimeline(None))
        );
        let r = AnimationRange::parse("0px 300px").unwrap();
        assert_eq!(r.css(), "0px 300px");
        assert_eq!(r.progress(150.0), Some(0.5));
        assert_eq!(r.progress(-20.0), Some(0.0));
        assert_eq!(r.progress(900.0), Some(1.0));
        assert_eq!(AnimationRange::parse("10 10"), None);
        let s = TimelineScope::parse(" --a,--b ").unwrap();
        assert_eq!(s.css(), "--a, --b");
        assert_eq!(TimelineScope::parse(&s.css()), Some(s));
        assert_eq!(TimelineScope::parse("ALL"), Some(TimelineScope::All));
        assert_eq!(TimelineScope::parse("none"), Some(TimelineScope::None));
        for bad in ["", "--a --b", "--a,", "a", "--a, all", "none, --a"] {
            assert_eq!(TimelineScope::parse(bad), None, "{bad:?}");
        }
    }
}
