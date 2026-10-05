//! Layout viewport facts in CSS pixels (points on Apple), and the user's
//! display preferences — what CSS's `@media` answers an app about the
//! screen it is on.
//! @ref LLP 1039 D1–D4; LLP 1061 D4; LLP 1069.000 D1; LLP 1078 D2

use exact_plan::Value;

/// Reserved resource source, answered before the app data seam.
pub const SOURCE: &str = "exactViewport";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &[
    "width",
    "height",
    "prefersReducedMotion",
    "prefersReducedTransparency",
    "prefersContrast",
    "prefersColorScheme",
    "colorGamut",
    "dynamicRange",
    "devicePosture",
    "horizontalViewportSegments",
    "verticalViewportSegments",
    "pointer",
    "hover",
];

/// The host's layout viewport, before the first settlement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Width in CSS pixels, including any scrollbar.
    pub width: f64,
    /// Height in CSS pixels, under the host's interactive-widget policy.
    pub height: f64,
    /// The user's display preferences; a host that cannot read them says
    /// `no-preference`, as a browser does.
    pub preferences: Preferences,
    /// The device's posture and the segments a fold makes of the viewport
    /// (LLP 1078 D2); a host without a fold says `continuous`, 1 × 1.
    pub fold: Fold,
}

/// The Device Posture API's postures: `folded` while the device forms an
/// angle short of flat (a hinge partially open), `continuous` otherwise —
/// flat, closed on one panel, or a device with no fold (LLP 1078 D1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Posture {
    /// `continuous`.
    #[default]
    Continuous,
    /// `folded`.
    Folded,
}

impl Posture {
    /// CSS's keyword.
    pub fn keyword(self) -> &'static str {
        match self {
            Posture::Continuous => "continuous",
            Posture::Folded => "folded",
        }
    }

    /// The posture by CSS keyword.
    pub fn from_keyword(keyword: &str) -> Option<Posture> {
        match keyword {
            "continuous" => Some(Posture::Continuous),
            "folded" => Some(Posture::Folded),
            _ => None,
        }
    }

    /// The hosts' wire form: 0 continuous, anything else folded.
    pub fn from_bits(bits: u32) -> Posture {
        if bits == 0 {
            Posture::Continuous
        } else {
            Posture::Folded
        }
    }
}

/// `device-posture` and the viewport segment counts (Media Queries 5's
/// `horizontal-viewport-segments` and `vertical-viewport-segments`), as
/// `exactViewport()` answers them (LLP 1078 D1, D2). The segments' rects are
/// the kernel's (`Kernel::set_segments`), not the app's: an app reads them
/// as `env()` lengths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fold {
    /// `devicePosture`.
    pub posture: Posture,
    /// `horizontalViewportSegments`: columns of segments, at least 1.
    pub cols: u32,
    /// `verticalViewportSegments`: rows of segments, at least 1.
    pub rows: u32,
}

impl Default for Fold {
    /// No fold: `continuous`, one segment — the bake's answer and every
    /// flat host's.
    fn default() -> Self {
        Fold::FLAT
    }
}

impl Fold {
    /// `continuous`, 1 × 1.
    pub const FLAT: Fold = Fold {
        posture: Posture::Continuous,
        cols: 1,
        rows: 1,
    };

    /// Refuse a grid with no column or no row.
    pub fn validate(self) -> Result<(), crate::RunnerError> {
        if self.cols >= 1 && self.rows >= 1 {
            Ok(())
        } else {
            Err(crate::RunnerError::InvalidViewport)
        }
    }
}

/// The segments a host without a fold makes for the agent's `prefer
/// segments <cols>x<rows> [gap <points>]` (LLP 1078 D7): the viewport split
/// evenly, the gap centred on each divider, as `[x, y, w, h]` row-major —
/// none for 1 × 1. Refused by name: a zero count, a negative or non-finite
/// gap, a gap wider than the viewport (the dividers leave no room).
pub fn even_segments(
    width: f64,
    height: f64,
    cols: u32,
    rows: u32,
    gap: f64,
) -> Result<Vec<[f64; 4]>, String> {
    if cols == 0 || rows == 0 {
        return Err(format!("segments {cols}x{rows}: each count is at least 1"));
    }
    if !gap.is_finite() || gap < 0.0 {
        return Err(format!("segments: gap {gap} is not a non-negative length"));
    }
    if cols * rows == 1 {
        return Ok(Vec::new());
    }
    let span = |total: f64, n: u32| -> Result<f64, String> {
        let bands = f64::from(n - 1) * gap;
        if bands >= total {
            return Err(format!(
                "segments {cols}x{rows} gap {gap}: the gap is wider than the viewport ({width} × {height})"
            ));
        }
        Ok((total - bands) / f64::from(n))
    };
    let (w, h) = (span(width, cols)?, span(height, rows)?);
    let mut out = Vec::with_capacity((cols * rows) as usize);
    for y in 0..rows {
        for x in 0..cols {
            out.push([f64::from(x) * (w + gap), f64::from(y) * (h + gap), w, h]);
        }
    }
    Ok(out)
}

/// CSS's user-preference media features (Media Queries 5 §11) a host reads
/// from the platform: `prefers-reduced-motion: reduce`,
/// `prefers-reduced-transparency: reduce`, `prefers-contrast` and
/// `prefers-color-scheme`. The runner has no policy of its own
/// (`rules/DEFERRED.md` §Motion): the app reads these and chooses.
/// @ref LLP 1061 D5; LLP 1069.000 D1
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Preferences {
    /// The user asked for less motion.
    pub reduced_motion: bool,
    /// The user asked for less transparency (materials, blur).
    pub reduced_transparency: bool,
    /// `prefers-contrast`: `more` and `less` as read; both at once is
    /// `custom`.
    pub contrast: Contrast,
    /// `prefers-color-scheme: dark` — the system's scheme, beneath any
    /// `setScheme` the app chose (LLP 1034 D3 as amended).
    pub dark: bool,
    /// `pointer`: the primary input's pointing accuracy.
    pub pointer: Pointer,
    /// `hover`: whether the primary input can hover.
    pub hover: Hover,
    /// `color-gamut`: the widest of `srgb`, `p3`, `rec2020` the display
    /// covers (Media Queries 5 §6.4; LLP 1100 D9).
    pub gamut: Gamut,
    /// `dynamic-range: high` (Media Queries 5 §6.5; LLP 1100 D9).
    /// `video-dynamic-range` is the same on every host Exact has.
    pub high_dynamic_range: bool,
}

/// CSS's `pointer` values (Media Queries 4 §7.1): a mouse is `fine`, a
/// finger `coarse`, a TV remote's D-pad `none`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Pointer {
    /// `fine`.
    #[default]
    Fine,
    /// `coarse`.
    Coarse,
    /// `none`.
    None,
}

impl Pointer {
    /// CSS's keyword.
    pub fn keyword(self) -> &'static str {
        match self {
            Pointer::Fine => "fine",
            Pointer::Coarse => "coarse",
            Pointer::None => "none",
        }
    }
}

/// CSS's `hover` values (Media Queries 4 §7.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Hover {
    /// `hover`.
    #[default]
    Hover,
    /// `none`.
    None,
}

impl Hover {
    /// CSS's keyword.
    pub fn keyword(self) -> &'static str {
        match self {
            Hover::Hover => "hover",
            Hover::None => "none",
        }
    }
}

/// Media Queries 5's `color-gamut` values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Gamut {
    /// `srgb`, and what a host that reads nothing says.
    #[default]
    Srgb,
    /// `p3`.
    P3,
    /// `rec2020`.
    Rec2020,
}

impl Gamut {
    /// CSS's keyword.
    pub fn keyword(self) -> &'static str {
        match self {
            Gamut::Srgb => "srgb",
            Gamut::P3 => "p3",
            Gamut::Rec2020 => "rec2020",
        }
    }
}

/// CSS's `prefers-contrast` values (Media Queries 5 §11.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Contrast {
    /// `no-preference`.
    #[default]
    NoPreference,
    /// `more`.
    More,
    /// `less`.
    Less,
    /// `custom`: a specific set of colours the user chose.
    Custom,
}

impl Contrast {
    /// CSS's keyword.
    pub fn keyword(self) -> &'static str {
        match self {
            Contrast::NoPreference => "no-preference",
            Contrast::More => "more",
            Contrast::Less => "less",
            Contrast::Custom => "custom",
        }
    }
}

impl Preferences {
    /// No preference stated, a light system: what a host that reads nothing
    /// reports.
    pub const NONE: Preferences = Preferences {
        reduced_motion: false,
        reduced_transparency: false,
        contrast: Contrast::NoPreference,
        dark: false,
        pointer: Pointer::Fine,
        hover: Hover::Hover,
        gamut: Gamut::Srgb,
        high_dynamic_range: false,
    };

    /// The hosts' wire form: bit 0 reduced motion, bit 1 reduced
    /// transparency, bit 2 contrast `more`, bit 3 contrast `less` (both is
    /// `custom`), bit 4 a dark system scheme, bit 5 pointer `coarse`, bit 6
    /// pointer `none` (over bit 5), bit 7 hover `none`, bits 8–9 the gamut
    /// (0 sRGB, 1 P3, 2 rec2020), bit 10 a high dynamic range; other bits
    /// are ignored. Zero is a mouse's `fine` and `hover`.
    pub fn from_bits(bits: u32) -> Self {
        Self {
            reduced_motion: bits & 1 != 0,
            reduced_transparency: bits & 2 != 0,
            contrast: match (bits & 4 != 0, bits & 8 != 0) {
                (false, false) => Contrast::NoPreference,
                (true, false) => Contrast::More,
                (false, true) => Contrast::Less,
                (true, true) => Contrast::Custom,
            },
            dark: bits & 16 != 0,
            pointer: match (bits & 32 != 0, bits & 64 != 0) {
                (false, false) => Pointer::Fine,
                (true, false) => Pointer::Coarse,
                (_, true) => Pointer::None,
            },
            hover: if bits & 128 != 0 {
                Hover::None
            } else {
                Hover::Hover
            },
            gamut: match (bits >> 8) & 3 {
                1 => Gamut::P3,
                2 | 3 => Gamut::Rec2020,
                _ => Gamut::Srgb,
            },
            high_dynamic_range: bits & 1024 != 0,
        }
    }

    /// The inverse of [`Preferences::from_bits`].
    pub fn bits(self) -> u32 {
        let contrast = match self.contrast {
            Contrast::NoPreference => 0,
            Contrast::More => 4,
            Contrast::Less => 8,
            Contrast::Custom => 12,
        };
        u32::from(self.reduced_motion)
            | (u32::from(self.reduced_transparency) << 1)
            | contrast
            | (u32::from(self.dark) << 4)
            | match self.pointer {
                Pointer::Fine => 0,
                Pointer::Coarse => 32,
                Pointer::None => 64,
            }
            | (u32::from(self.hover == Hover::None) << 7)
            | (match self.gamut {
                Gamut::Srgb => 0,
                Gamut::P3 => 1,
                Gamut::Rec2020 => 2,
            } << 8)
            | (u32::from(self.high_dynamic_range) << 10)
    }

    /// `"light"` or `"dark"`, CSS's words for the system's scheme.
    pub fn color_scheme(self) -> &'static str {
        if self.dark {
            "dark"
        } else {
            "light"
        }
    }
}

impl Default for Viewport {
    /// The bake's LINT_VIEWPORT: 390 × 844 (LLP 1039 D3), no preference.
    fn default() -> Self {
        Self::sized(390.0, 844.0)
    }
}

impl Viewport {
    /// A size with no stated preference.
    pub fn sized(width: f64, height: f64) -> Self {
        Self {
            width,
            height,
            preferences: Preferences::default(),
            fold: Fold::default(),
        }
    }

    /// Refuse invalid sizes before changing any runner state.
    pub fn validate(self) -> Result<(), crate::RunnerError> {
        if self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
        {
            Ok(())
        } else {
            Err(crate::RunnerError::InvalidViewport)
        }
    }

    /// Fill a declared field; unknown names are refused.
    pub fn field(self, name: &str) -> Option<Value> {
        match name {
            "width" => Some(Value::Number(self.width)),
            "height" => Some(Value::Number(self.height)),
            "prefersReducedMotion" => Some(Value::Bool(self.preferences.reduced_motion)),
            "prefersReducedTransparency" => {
                Some(Value::Bool(self.preferences.reduced_transparency))
            }
            "prefersContrast" => Some(Value::str(self.preferences.contrast.keyword())),
            "prefersColorScheme" => Some(Value::str(self.preferences.color_scheme())),
            "colorGamut" => Some(Value::str(self.preferences.gamut.keyword())),
            "dynamicRange" => Some(Value::str(if self.preferences.high_dynamic_range {
                "high"
            } else {
                "standard"
            })),
            "devicePosture" => Some(Value::str(self.fold.posture.keyword())),
            "horizontalViewportSegments" => Some(Value::Number(f64::from(self.fold.cols))),
            "verticalViewportSegments" => Some(Value::Number(f64::from(self.fold.rows))),
            "pointer" => Some(Value::str(self.preferences.pointer.keyword())),
            "hover" => Some(Value::str(self.preferences.hover.keyword())),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        even_segments, Contrast, Fold, Pointer, Posture, Preferences, Value, Viewport, FIELDS,
    };

    #[test]
    fn even_segments_split_the_viewport_with_the_gap_centred() {
        assert!(even_segments(951.0, 669.0, 1, 1, 40.0).unwrap().is_empty());
        assert_eq!(
            even_segments(951.0, 669.0, 2, 1, 40.0).unwrap(),
            vec![[0.0, 0.0, 455.5, 669.0], [495.5, 0.0, 455.5, 669.0]]
        );
        assert_eq!(
            even_segments(100.0, 100.0, 2, 2, 0.0).unwrap(),
            vec![
                [0.0, 0.0, 50.0, 50.0],
                [50.0, 0.0, 50.0, 50.0],
                [0.0, 50.0, 50.0, 50.0],
                [50.0, 50.0, 50.0, 50.0]
            ]
        );
        assert!(even_segments(100.0, 100.0, 0, 1, 0.0).is_err());
        assert!(even_segments(100.0, 100.0, 1, 0, 0.0).is_err());
        assert!(even_segments(100.0, 100.0, 2, 1, 100.0).is_err());
        assert!(even_segments(100.0, 100.0, 2, 1, -1.0).is_err());
        assert!(even_segments(100.0, 100.0, 2, 1, f64::NAN).is_err());
    }

    /// LLP 1078 D2: the three fold fields fill by name; the bake answers
    /// `continuous`, 1, 1.
    #[test]
    fn fold_fields_fill_by_name_and_default_flat() {
        use exact_plan::Value;
        let v = Viewport::default();
        assert_eq!(v.fold, Fold::FLAT);
        assert_eq!(v.field("devicePosture"), Some(Value::str("continuous")));
        assert_eq!(
            v.field("horizontalViewportSegments"),
            Some(Value::Number(1.0))
        );
        assert_eq!(
            v.field("verticalViewportSegments"),
            Some(Value::Number(1.0))
        );
        let folded = Viewport {
            fold: Fold {
                posture: Posture::Folded,
                cols: 2,
                rows: 1,
            },
            ..v
        };
        assert_eq!(folded.field("devicePosture"), Some(Value::str("folded")));
        assert_eq!(
            folded.field("horizontalViewportSegments"),
            Some(Value::Number(2.0))
        );
        for name in [
            "devicePosture",
            "horizontalViewportSegments",
            "verticalViewportSegments",
        ] {
            assert!(FIELDS.contains(&name));
        }
        assert_eq!(Posture::from_keyword("folded"), Some(Posture::Folded));
        assert_eq!(Posture::from_keyword("open"), None);
        assert_eq!(Posture::from_bits(0), Posture::Continuous);
        assert_eq!(Posture::from_bits(1).keyword(), "folded");
        assert!(Fold {
            cols: 0,
            ..Fold::FLAT
        }
        .validate()
        .is_err());
        assert!(Fold {
            rows: 0,
            ..Fold::FLAT
        }
        .validate()
        .is_err());
    }

    #[test]
    fn bits_round_trip() {
        assert_eq!(Preferences::from_bits(0), Preferences::NONE);
        for bits in 0..32 {
            assert_eq!(Preferences::from_bits(bits).bits(), bits);
        }
        assert_eq!(Preferences::from_bits(4).contrast, Contrast::More);
        assert_eq!(Preferences::from_bits(8).contrast.keyword(), "less");
        assert_eq!(Preferences::from_bits(12).contrast.keyword(), "custom");
        assert_eq!(Preferences::from_bits(16).color_scheme(), "dark");
        // LLP 1100 D9: the display's gamut and range.
        for bits in [0, 256, 512, 1024, 256 | 1024] {
            assert_eq!(Preferences::from_bits(bits).bits(), bits);
        }
        let v = |bits| Viewport {
            preferences: Preferences::from_bits(bits),
            ..Viewport::default()
        };
        assert_eq!(
            v(0).field("colorGamut"),
            Some(exact_plan::Value::str("srgb"))
        );
        assert_eq!(
            v(256).field("colorGamut"),
            Some(exact_plan::Value::str("p3"))
        );
        assert_eq!(
            v(768).field("colorGamut"),
            Some(exact_plan::Value::str("rec2020")),
            "a rec2020 panel covers p3 too"
        );
        assert_eq!(
            v(0).field("dynamicRange"),
            Some(exact_plan::Value::str("standard"))
        );
        assert_eq!(
            v(1024).field("dynamicRange"),
            Some(exact_plan::Value::str("high"))
        );
    }

    #[test]
    fn pointer_and_hover_bits_fill_their_fields() {
        let field = |bits: u32, name: &str| {
            Viewport {
                preferences: Preferences::from_bits(bits),
                ..Viewport::default()
            }
            .field(name)
        };
        assert_eq!(field(0, "pointer"), Some(Value::str("fine")));
        assert_eq!(field(0, "hover"), Some(Value::str("hover")));
        assert_eq!(field(32 | 128, "pointer"), Some(Value::str("coarse")));
        assert_eq!(field(64 | 128, "pointer"), Some(Value::str("none")));
        assert_eq!(field(64 | 128, "hover"), Some(Value::str("none")));
        assert_eq!(
            Preferences::from_bits(96).pointer,
            Pointer::None,
            "none wins over coarse"
        );
        for bits in [32, 64, 128, 32 | 128, 64 | 128 | 16] {
            assert_eq!(Preferences::from_bits(bits).bits(), bits);
        }
    }
}
