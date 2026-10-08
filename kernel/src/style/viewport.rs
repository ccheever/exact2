//! CSS viewport lengths. Native surfaces have no retractable browser chrome.
use super::compare::{Base, Expr, Op};
use super::{Dimension, Env, SegmentVar};
use std::cell::Cell;

/// The viewport dimension a CSS length uses (CSS Values 4 §6.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ViewportUnit {
    /// Large viewport width (`vw`).
    Vw,
    /// Large viewport height (`vh`).
    Vh,
    /// The smaller large viewport dimension.
    Vmin,
    /// The larger large viewport dimension.
    Vmax,
    /// Small viewport width.
    Svw,
    /// Small viewport height.
    Svh,
    /// Large viewport width.
    Lvw,
    /// Large viewport height.
    Lvh,
    /// Dynamic viewport width.
    Dvw,
    /// Dynamic viewport height.
    Dvh,
}

impl ViewportUnit {
    /// Every supported unit, in wire order (kinds 14–23).
    pub const ALL: [Self; 10] = [
        Self::Vw,
        Self::Vh,
        Self::Vmin,
        Self::Vmax,
        Self::Svw,
        Self::Svh,
        Self::Lvw,
        Self::Lvh,
        Self::Dvw,
        Self::Dvh,
    ];

    /// The CSS suffix.
    pub fn name(self) -> &'static str {
        [
            "vw", "vh", "vmin", "vmax", "svw", "svh", "lvw", "lvh", "dvw", "dvh",
        ][self as usize]
    }

    /// Whether the unit reads the viewport's height: the height units,
    /// and `vmin`/`vmax`, which may (LLP 1075.003 §9.11).
    pub fn reads_height(self) -> bool {
        !matches!(self, Self::Vw | Self::Svw | Self::Lvw | Self::Dvw)
    }

    /// The viewport's length in points. Native windows have no browser chrome.
    pub fn basis(self, env: &Env) -> f32 {
        match self {
            Self::Vw | Self::Svw | Self::Lvw | Self::Dvw => env.viewport_width,
            Self::Vh | Self::Svh | Self::Lvh | Self::Dvh => env.viewport_height,
            Self::Vmin => env.viewport_width.min(env.viewport_height),
            Self::Vmax => env.viewport_width.max(env.viewport_height),
        }
    }
}

pub(super) fn parse(text: &str) -> Option<Dimension> {
    let text = text.trim().to_ascii_lowercase();
    ViewportUnit::ALL.into_iter().find_map(|unit| {
        text.strip_suffix(unit.name())
            .and_then(|n| exact_num::parse_f32(n).ok())
            .filter(|n| n.is_finite())
            .map(|n| Dimension::Viewport(unit, n))
    })
}

thread_local! {
    static HEIGHTLESS: Cell<bool> = const { Cell::new(false) };
}

/// While this lives, every length on this thread resolves without the
/// viewport's height ([`Dimension::without_viewport_height`]): a sheet's
/// content laid out alone, whose viewport is the sheet (LLP 1075.003 §9.11).
/// Lowering, field chrome, button and multi-column measurement all resolve
/// through [`Dimension::resolve`], so none reads the sheet back.
pub(crate) struct Heightless(bool);

impl Heightless {
    pub(crate) fn enter() -> Heightless {
        Heightless(HEIGHTLESS.with(|h| h.replace(true)))
    }

    /// Whether a scope is open on this thread.
    pub(crate) fn active() -> bool {
        HEIGHTLESS.with(Cell::get)
    }
}

impl Drop for Heightless {
    fn drop(&mut self) {
        HEIGHTLESS.with(|h| h.set(self.0));
    }
}

/// Where a term sits in a comparison: under `max()` (or `clamp()`'s low
/// and preferred values) it is a floor, under `min()` (or `clamp()`'s high
/// value) a cap.
#[derive(Clone, Copy)]
enum Slot {
    Floor,
    Cap,
    Bare,
}

/// Whether a term reads the viewport's height: a height-axis unit with a
/// nonzero coefficient (`0vh` reads nothing).
fn reads(e: &Expr) -> bool {
    match e {
        Expr::Term(Base::Viewport(unit, n), _) => unit.reads_height() && *n != 0.0,
        Expr::Term(..) => false,
        Expr::Pick(_, args, _) => args.iter().any(reads),
    }
}

/// A comparison's points with its height terms unknown, and whether the
/// result still rests on one. An unknown term loses where it can: under a
/// cap it is unbounded, under a floor its least (its added points, or no
/// bound for a negative coefficient). A pick a height term wins or ties
/// rests on it, as does one with no finite value.
fn reduce(e: &Expr, env: &Env, slot: Slot) -> (f32, bool) {
    match e {
        Expr::Term(Base::Viewport(unit, n), plus) if unit.reads_height() && *n != 0.0 => {
            let v = match slot {
                Slot::Cap => f32::INFINITY,
                Slot::Floor if *n < 0.0 => f32::NEG_INFINITY,
                Slot::Floor | Slot::Bare => *plus,
            };
            (v, true)
        }
        Expr::Term(..) => (e.value(env), false),
        Expr::Pick(op, args, plus) => {
            let arg = |i: usize, slot| args.get(i).map_or((0.0, false), |a| reduce(a, env, slot));
            let (v, rests) = match op {
                Op::Min => pick(args.iter().map(|a| reduce(a, env, Slot::Cap)), true),
                Op::Max => pick(args.iter().map(|a| reduce(a, env, Slot::Floor)), false),
                Op::Clamp => {
                    let inner = pick([arg(1, Slot::Floor), arg(2, Slot::Cap)], true);
                    pick([arg(0, Slot::Floor), inner], false)
                }
            };
            (v + plus, rests || !v.is_finite())
        }
    }
}

/// The least (`min`) or greatest value, and whether a term resting on a
/// height wins it or ties.
fn pick(items: impl IntoIterator<Item = (f32, bool)>, min: bool) -> (f32, bool) {
    let items: Vec<_> = items.into_iter().collect();
    let best = items.iter().map(|i| i.0).fold(
        if min {
            f32::INFINITY
        } else {
            f32::NEG_INFINITY
        },
        |a, b| {
            if min {
                a.min(b)
            } else {
                a.max(b)
            }
        },
    );
    (best, items.iter().any(|&(v, rests)| rests && v == best))
}

impl Dimension {
    /// This length where the viewport's height is unknown (LLP 1075.003
    /// §9.11: a sheet's content, whose viewport is the sheet): `None` when
    /// it reads none (`vh` and its kin, `vmin`, `vmax` with a nonzero
    /// coefficient, `env(viewport-segment-height|top|bottom)`); `Auto` when
    /// it rests on one; else its points with the height terms unknown —
    /// `max(200px, 80vh)` is 200, `min(80vh, 300px)` 300,
    /// `clamp(100px, 50vh, 400px)` 100, `clamp(10vh, 200px, 40vh)` 200, and
    /// `max(80vh, 0px)`, `calc(80vh + 16px)` rest on it.
    pub(crate) fn without_viewport_height(self, env: &Env) -> Option<Dimension> {
        match self {
            Dimension::Viewport(unit, n) if unit.reads_height() && n != 0.0 => {
                Some(Dimension::Auto)
            }
            Dimension::Segment(SegmentVar::Height | SegmentVar::Top | SegmentVar::Bottom, ..) => {
                Some(Dimension::Auto)
            }
            Dimension::Compare(c) => {
                let e = c.expr();
                reads(&e).then(|| match reduce(&e, env, Slot::Bare) {
                    (v, false) => Dimension::Points(v),
                    (_, true) => Dimension::Auto,
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod heightless_tests {
    use super::*;

    fn at(text: &str) -> Option<Dimension> {
        let d = super::super::compare::parse(text)
            .ok()
            .flatten()
            .or_else(|| parse(text))
            .unwrap_or_else(|| panic!("{text}"));
        d.without_viewport_height(&Env::default())
    }

    /// LLP 1075.003 §9.11, Astra's and Grok's r6: a height term loses
    /// where it can, so a cap or floor beside it stands, and a value that
    /// rests on it is unknown.
    #[test]
    fn a_height_term_loses_where_it_can() {
        use Dimension::{Auto, Points};
        for (text, want) in [
            ("80vh", Some(Auto)),
            ("100vmax", Some(Auto)),
            ("10vw", None),
            ("0vh", None),
            ("max(200px, 80vh)", Some(Points(200.0))),
            ("min(80vh, 300px)", Some(Points(300.0))),
            ("clamp(100px, 50vh, 400px)", Some(Points(100.0))),
            ("clamp(10vh, 200px, 40vh)", Some(Points(200.0))),
            ("max(80vh, 0px)", Some(Auto)),
            ("max(50vh, 30dvh)", Some(Auto)),
            ("max(calc(80vh + 16px), 10px)", Some(Auto)),
            ("max(-40px, -10vh)", Some(Points(-40.0))),
            ("max(-40px, 10vh)", Some(Auto)),
            ("max(20px, 0vh)", None),
            ("min(10vw, 300px)", None),
        ] {
            assert_eq!(at(text), want, "{text}");
        }
        // A segment's height and vertical edges follow the sheet; its
        // width and horizontal edges do not.
        let env = Env::default();
        for (var, want) in [
            (SegmentVar::Height, Some(Auto)),
            (SegmentVar::Top, Some(Auto)),
            (SegmentVar::Bottom, Some(Auto)),
            (SegmentVar::Width, None),
            (SegmentVar::Left, None),
        ] {
            let d = Dimension::Segment(var, 0, 0, 12.0);
            assert_eq!(d.without_viewport_height(&env), want, "{var:?}");
        }
    }
}
