//! Layout viewport facts in CSS pixels (points on Apple), and the user's
//! display preferences — what CSS's `@media` answers an app about the
//! screen it is on.
//! @ref LLP 1039 D1–D4; LLP 1061 D4

use exact_plan::Value;

/// Reserved resource source, answered before the app data seam.
pub const SOURCE: &str = "exactViewport";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &[
    "width",
    "height",
    "prefersReducedMotion",
    "prefersReducedTransparency",
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
/// from the platform: `prefers-reduced-motion: reduce` and
/// `prefers-reduced-transparency: reduce`. The runner has no policy of its
/// own (`rules/DEFERRED.md` §Motion): the app reads these and chooses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Preferences {
    /// The user asked for less motion.
    pub reduced_motion: bool,
    /// The user asked for less transparency (materials, blur).
    pub reduced_transparency: bool,
}

impl Preferences {
    /// The hosts' wire form: bit 0 reduced motion, bit 1 reduced
    /// transparency; other bits are ignored.
    pub fn from_bits(bits: u32) -> Self {
        Self {
            reduced_motion: bits & 1 != 0,
            reduced_transparency: bits & 2 != 0,
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
            _ => None,
        }
    }
}
