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
    /// Container width (LLP 1123 D12): the content owner's, else the small viewport's.
    Cqw,
    /// Container height.
    Cqh,
    /// Container inline size: its width, in a horizontal writing mode.
    Cqi,
    /// Container block size: its height.
    Cqb,
    /// The smaller container dimension.
    Cqmin,
    /// The larger container dimension.
    Cqmax,
}

impl ViewportUnit {
    /// Every supported unit: the viewport's in wire order (kinds 14–23),
    /// then the container's (kinds 27–32).
    pub const ALL: [Self; 16] = [
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
        Self::Cqw,
        Self::Cqh,
        Self::Cqi,
        Self::Cqb,
        Self::Cqmin,
        Self::Cqmax,
    ];

    /// Whether it is a container unit, which an owner's size sets.
    pub fn is_container(self) -> bool {
        self as u8 >= Self::Cqw as u8
    }

    /// The CSS suffix.
    pub fn name(self) -> &'static str {
        [
            "vw", "vh", "vmin", "vmax", "svw", "svh", "lvw", "lvh", "dvw", "dvh", "cqw", "cqh",
            "cqi", "cqb", "cqmin", "cqmax",
        ][self as usize]
    }

    /// The viewport's length in points. Native windows have no browser chrome.
    pub fn basis(self, env: &Env) -> f32 {
        // LLP 1075.003 §9.11: the window's, when the host lays the page out
        // in a sheet's smaller viewport.
        let (w, h) = env
            .screen
            .unwrap_or((env.viewport_width, env.viewport_height));
        // An owner's content root is a size container (LLP 1123 D12); with
        // none, CSS uses the small viewport, which a native window's is.
        let (cw, ch) = env.container.unwrap_or((w, h));
        match self {
            Self::Cqw | Self::Cqi => cw,
            Self::Cqh | Self::Cqb => ch,
            Self::Cqmin => cw.min(ch),
            Self::Cqmax => cw.max(ch),
            Self::Vw | Self::Svw | Self::Lvw | Self::Dvw => w,
            Self::Vh | Self::Svh | Self::Lvh | Self::Dvh => h,
            Self::Vmin => w.min(h),
            Self::Vmax => w.max(h),
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
