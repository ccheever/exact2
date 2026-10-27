//! A content owner's values (@ref LLP 1123 D9): the safe area, the layout
//! regions and the fold's segments that the platform defines for a route,
//! a sheet or a pane, which an author reads as `var(--exact-…)`. `env()`
//! stays the document's ([`super::env`]); these are the owner's, in its
//! content root's coordinates, set by the host on each owner and inherited
//! by its subtree. A node in no owner reads the document's: `env()`'s
//! values.

use super::env::{Edge, Env, Rect, Region, SegmentVar};

/// Which owner value a `var(--exact-…)` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OwnerVar {
    /// `--exact-safe-area-inset-<edge>`.
    Safe(Edge),
    /// `--exact-<region>-inset-<edge>`, one of D10's five regions.
    Region(Region, Edge),
    /// `--exact-viewport-segment-<field>-<x>-<y>`, `x` and `y` each 0 or 1.
    Segment(SegmentVar, u8, u8),
}

/// The largest segment index an owner property names (LLP 1123 D9: a grid
/// up to 2 × 2).
pub const MAX_SEGMENT_INDEX: u8 = 1;

impl OwnerVar {
    /// The variable a custom property's name names, without `var(`:
    /// `--exact-safe-area-inset-top`. `None` for any other name.
    pub fn from_name(name: &str) -> Option<OwnerVar> {
        let rest = name.strip_prefix("--exact-")?;
        if let Some(edge) = rest.strip_prefix("safe-area-inset-") {
            return Edge::from_name(edge).map(OwnerVar::Safe);
        }
        if let Some(seg) = rest.strip_prefix("viewport-segment-") {
            let mut parts = seg.rsplitn(3, '-');
            let y = parts.next()?.parse::<u8>().ok()?;
            let x = parts.next()?.parse::<u8>().ok()?;
            let var = SegmentVar::from_name(parts.next()?)?;
            return (x <= MAX_SEGMENT_INDEX && y <= MAX_SEGMENT_INDEX)
                .then_some(OwnerVar::Segment(var, x, y));
        }
        // A region's name is the `env()` one with one more hyphen.
        Region::from_name(&name[1..]).map(|(r, e)| OwnerVar::Region(r, e))
    }

    /// The custom property's name: `--exact-safe-area-inset-top`.
    pub fn name(self) -> String {
        match self {
            OwnerVar::Safe(edge) => format!("--exact-safe-area-inset-{}", edge.name()),
            OwnerVar::Region(region, edge) => format!("-{}{}", region.prefix(), edge.name()),
            OwnerVar::Segment(var, x, y) => {
                format!("--exact-viewport-segment-{}-{x}-{y}", var.name())
            }
        }
    }

    /// Its wire form: a tag (0 safe area, 1 region, 2 segment) and three bytes.
    pub fn bytes(self) -> [u8; 4] {
        match self {
            OwnerVar::Safe(edge) => [0, edge as u8, 0, 0],
            OwnerVar::Region(region, edge) => [1, region as u8, edge as u8, 0],
            OwnerVar::Segment(var, x, y) => [2, var as u8, x, y],
        }
    }

    /// The variable its wire form names.
    pub fn from_bytes(b: [u8; 4]) -> Option<OwnerVar> {
        match b[0] {
            0 => Edge::from_index(b[1]).map(OwnerVar::Safe),
            1 => Some(OwnerVar::Region(
                Region::from_index(b[1])?,
                Edge::from_index(b[2])?,
            )),
            2 => {
                let var = SegmentVar::from_index(b[1])?;
                (b[2] <= MAX_SEGMENT_INDEX && b[3] <= MAX_SEGMENT_INDEX)
                    .then_some(OwnerVar::Segment(var, b[2], b[3]))
            }
            _ => None,
        }
    }

    /// The points it reads under `env`: the owner's when `env` carries one,
    /// the document's otherwise. `None` for a segment that does not exist,
    /// whose property the host leaves invalid so `var()`'s fallback applies.
    pub fn value(self, env: &Env) -> Option<f64> {
        let owner = env.owner.as_deref();
        match self {
            OwnerVar::Safe(edge) => Some(f64::from(
                owner.map_or_else(|| env.inset(edge), |o| o.safe[edge as usize]),
            )),
            OwnerVar::Region(region, edge) => Some(f64::from(match owner {
                Some(o) => o.region_inset(region, edge),
                None => env.region_inset(region, edge),
            })),
            OwnerVar::Segment(var, x, y) => {
                let rect = match owner {
                    Some(o) => o.segment(x, y),
                    None => env.segment(x, y),
                }?;
                Some(f64::from(var.of(rect)))
            }
        }
    }
}

/// What a host publishes for one owner, in its content root's points.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OwnerValues {
    /// The safe area left at the content root: 0 on an edge the host clears,
    /// the owner's inset on an edge the author takes (LLP 1123 D4).
    pub safe: [f32; 4],
    /// D10's five regions, each top right bottom left, translated into the
    /// content root; `None` where the host reports none (each is then the
    /// safe area).
    pub regions: Option<[[f32; 4]; 5]>,
    /// Columns and rows of the owner's segments; 1 × 1 has none.
    pub cols: u8,
    /// See `cols`.
    pub rows: u8,
    /// The segments, row-major, in the content root's coordinates.
    pub segments: Vec<Rect>,
    /// The content root's width and height: the container units' basis
    /// (LLP 1123 D12).
    pub size: (f32, f32),
}

impl OwnerValues {
    /// A region's inset at `edge`: the region's, or the safe area's on a host
    /// that reports none.
    pub fn region_inset(&self, region: Region, edge: Edge) -> f32 {
        self.regions.map_or(self.safe[edge as usize], |r| {
            r[region as usize][edge as usize]
        })
    }

    /// Segment `(x, y)`, when the owner has that many.
    pub fn segment(&self, x: u8, y: u8) -> Option<&Rect> {
        (x < self.cols
            && y < self.rows
            && self.segments.len() == usize::from(self.cols) * usize::from(self.rows))
        .then(|| &self.segments[usize::from(y) * usize::from(self.cols) + usize::from(x)])
    }

    /// Whether every number is finite and the segment grid is whole.
    pub fn is_valid(&self) -> bool {
        self.safe.iter().all(|v| v.is_finite())
            && self
                .regions
                .is_none_or(|r| r.iter().flatten().all(|v| v.is_finite()))
            && self.size.0.is_finite()
            && self.size.1.is_finite()
            && self.size.0 >= 0.0
            && self.size.1 >= 0.0
            && self.cols >= 1
            && self.rows >= 1
            && ((self.cols == 1 && self.rows == 1 && self.segments.is_empty())
                || self.segments.len() == usize::from(self.cols) * usize::from(self.rows))
            && self.segments.iter().all(Rect::is_finite)
    }
}

/// The owner variable a `var(--exact-…)` text names, alone, or `None` when the
/// text is not one. Any other `var()` is not this module's.
pub fn parse(text: &str) -> Option<OwnerVar> {
    let inner = text.trim().strip_prefix("var(")?.strip_suffix(')')?.trim();
    OwnerVar::from_name(inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_refuse_what_d9_does_not_name() {
        let all = Edge::ALL
            .iter()
            .map(|e| OwnerVar::Safe(*e))
            .chain(
                Region::ALL
                    .iter()
                    .flat_map(|r| Edge::ALL.iter().map(move |e| OwnerVar::Region(*r, *e))),
            )
            .chain(SegmentVar::ALL.iter().flat_map(|v| {
                [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(x, y)| OwnerVar::Segment(*v, x, y))
            }))
            .collect::<Vec<_>>();
        assert_eq!(all.len(), 4 + 20 + 24);
        for var in all {
            assert_eq!(
                OwnerVar::from_name(&var.name()),
                Some(var),
                "{}",
                var.name()
            );
            assert_eq!(OwnerVar::from_bytes(var.bytes()), Some(var));
        }
        assert_eq!(
            OwnerVar::from_name("--exact-viewport-segment-width-2-0"),
            None
        );
        assert_eq!(OwnerVar::from_name("--exact-press"), None);
        assert_eq!(OwnerVar::from_name("--exact-safe-area-inset-middle"), None);
        assert_eq!(
            parse(" var( --exact-readable-content-inset-left ) "),
            Some(OwnerVar::Region(Region::ReadableContent, Edge::Left))
        );
        assert_eq!(parse("var(--exact-safe-area-inset-top, 0px)"), None);
    }

    #[test]
    fn an_owner_reads_its_own_values_and_no_owner_reads_the_documents() {
        let mut env = Env::new(59.0, 0.0, 34.0, 0.0).with_segments(
            2,
            1,
            vec![
                Rect::new(0.0, 0.0, 455.5, 669.0),
                Rect::new(495.5, 0.0, 455.5, 669.0),
            ],
        );
        let top = OwnerVar::Safe(Edge::Top);
        let lower = OwnerVar::Segment(SegmentVar::Top, 0, 1);
        let left = OwnerVar::Segment(SegmentVar::Left, 1, 0);
        assert_eq!(top.value(&env), Some(59.0));
        assert_eq!(left.value(&env), Some(495.5));
        assert_eq!(lower.value(&env), None);
        // A route under a 134-point bar on a laptop fold (LLP 1123 D9's worked example).
        env.owner = Some(std::sync::Arc::new(OwnerValues {
            safe: [0.0; 4],
            regions: None,
            cols: 1,
            rows: 2,
            segments: vec![
                Rect::new(0.0, 0.0, 669.0, 321.5),
                Rect::new(0.0, 361.5, 669.0, 372.5),
            ],
            size: (669.0, 734.0),
        }));
        assert_eq!(top.value(&env), Some(0.0));
        assert_eq!(lower.value(&env), Some(361.5));
        assert_eq!(left.value(&env), None);
    }

    #[test]
    fn a_length_reads_an_owner_value_alone_and_in_a_calculation() {
        use crate::style::{uses_env, Dimension};
        use crate::{StyleId, StyleProps, StyleValue};
        fn text(t: &str) -> StyleValue<'_> {
            StyleValue::Text(t.into())
        }
        let mut s = StyleProps::default();
        s.set_dynamic(
            StyleId::Width,
            &text("var(--exact-viewport-segment-width-0-0)"),
        )
        .unwrap();
        assert_eq!(
            s.width,
            Dimension::Owner(OwnerVar::Segment(SegmentVar::Width, 0, 0), 0.0)
        );
        assert!(uses_env(&s));
        // One owner term and points reduce to the compact form, as env() does.
        s.set_dynamic(
            StyleId::Height,
            &text("calc(var(--exact-viewport-segment-top-0-1) - 40px)"),
        )
        .unwrap();
        assert_eq!(
            s.height,
            Dimension::Owner(OwnerVar::Segment(SegmentVar::Top, 0, 1), -40.0)
        );
        let mut env = Env::new(0.0, 0.0, 0.0, 0.0);
        env.owner = Some(std::sync::Arc::new(OwnerValues {
            cols: 1,
            rows: 2,
            segments: vec![
                Rect::new(0.0, 0.0, 669.0, 321.5),
                Rect::new(0.0, 361.5, 669.0, 372.5),
            ],
            size: (669.0, 734.0),
            ..OwnerValues::default()
        }));
        assert_eq!(s.height.resolve(&env), Dimension::Points(321.5));
        // A tree keeps the term, and reads it against the owner too.
        s.set_dynamic(
            StyleId::Height,
            &text("max(10px, var(--exact-viewport-segment-height-0-0))"),
        )
        .unwrap();
        assert!(matches!(s.height, Dimension::Compare(_)));
        assert_eq!(s.height.resolve(&env), Dimension::Points(321.5));
        let mut css = String::new();
        if let Dimension::Compare(c) = s.height {
            c.css(&mut css);
        }
        assert_eq!(css, "max(10px, var(--exact-viewport-segment-height-0-0))");
        // Any other var() is refused.
        for refused in [
            "var(--exact-press)",
            "calc(var(--exact-press) + 1px)",
            "var(--exact-press, 1px)",
        ] {
            assert!(
                s.set_dynamic(StyleId::Width, &text(refused)).is_err(),
                "{refused}"
            );
        }
        // A fallback stands in for a segment the owner does not have, as
        // env()'s does (CSS Variables 1 §3).
        let fallback = "calc(var(--exact-viewport-segment-left-1-0, 7px) + 1px)";
        s.set_dynamic(StyleId::Width, &text(fallback)).unwrap();
        assert_eq!(s.width.resolve(&env), Dimension::Points(8.0));
        let defined = "var(--exact-viewport-segment-top-0-1, 7px)";
        s.set_dynamic(StyleId::Width, &text(defined)).unwrap();
        assert_eq!(s.width.resolve(&env), Dimension::Points(361.5));
    }
}
