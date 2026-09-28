//! Layout viewport facts in CSS pixels (points on Apple), and the user's
//! display preferences — what CSS's `@media` answers an app about the
//! screen it is on.
//! @ref LLP 1039 D1–D4; LLP 1061 D4; LLP 1069.000 D1

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
    };

    /// The hosts' wire form: bit 0 reduced motion, bit 1 reduced
    /// transparency, bit 2 contrast `more`, bit 3 contrast `less` (both is
    /// `custom`), bit 4 a dark system scheme; other bits are ignored.
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
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Contrast, Preferences};

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
    }
}
