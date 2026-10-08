//! CSS viewport lengths. Native surfaces have no retractable browser chrome.
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

    /// The viewport's length in points. Native windows have no browser chrome.
    pub fn basis(self, env: &Env) -> f32 {
        match self {
            Self::Vw | Self::Svw | Self::Lvw | Self::Dvw => env.viewport_width,
            Self::Vh | Self::Svh | Self::Lvh | Self::Dvh => {
                env.screen.map_or(env.viewport_height, |s| s.1)
            }
            Self::Vmin => {
                let (w, h) = env
                    .screen
                    .unwrap_or((env.viewport_width, env.viewport_height));
                w.min(h)
            }
            Self::Vmax => {
                let (w, h) = env
                    .screen
                    .unwrap_or((env.viewport_width, env.viewport_height));
                w.max(h)
            }
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
