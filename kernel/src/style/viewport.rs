//! CSS viewport lengths. Native surfaces have no retractable browser chrome.
use super::compare::{Base, Expr, Op};
use super::{Dimension, Env};

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

impl Dimension {
    /// This length where the viewport's height is unknown (LLP 1075.003
    /// §9.11: a sheet's content, whose viewport is the sheet): `None` when
    /// it reads no height-axis viewport unit (`vh` and its kin, `vmin`,
    /// `vmax`); `Auto` for a bare one, or a comparison of nothing else (no
    /// other term, no points added); a comparison that also holds a length,
    /// its points with those terms at zero, so `max(200px, 80vh)` is 200
    /// and `clamp(100px, 50vh, 400px)` is 100.
    pub(crate) fn without_viewport_height(self, env: &Env) -> Option<Dimension> {
        fn reads(e: &Expr) -> bool {
            match e {
                Expr::Term(Base::Viewport(unit, _), _) => unit.reads_height(),
                Expr::Term(..) => false,
                Expr::Pick(_, args, _) => args.iter().any(reads),
            }
        }
        fn other(e: &Expr) -> bool {
            match e {
                Expr::Term(Base::Viewport(unit, _), plus) => !unit.reads_height() || *plus != 0.0,
                Expr::Term(..) => true,
                Expr::Pick(_, args, plus) => *plus != 0.0 || args.iter().any(other),
            }
        }
        fn heightless(e: &Expr, env: &Env) -> f32 {
            match e {
                Expr::Term(Base::Viewport(unit, _), plus) if unit.reads_height() => *plus,
                Expr::Term(..) => e.value(env),
                Expr::Pick(op, args, plus) => {
                    let v: Vec<f32> = args.iter().map(|a| heightless(a, env)).collect();
                    let at = |i: usize| v.get(i).copied().unwrap_or(0.0);
                    plus + match op {
                        Op::Min => v.iter().copied().fold(f32::INFINITY, f32::min),
                        Op::Max => v.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                        Op::Clamp => at(0).max(at(1).min(at(2))),
                    }
                }
            }
        }
        match self {
            Dimension::Viewport(unit, _) if unit.reads_height() => Some(Dimension::Auto),
            Dimension::Compare(c) => {
                let e = c.expr();
                reads(&e).then(|| {
                    if other(&e) {
                        Dimension::Points(heightless(&e, env))
                    } else {
                        Dimension::Auto
                    }
                })
            }
            _ => None,
        }
    }
}
