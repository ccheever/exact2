//! Layout viewport facts in CSS pixels (points on Apple).
//! @ref LLP 1039 D1–D4

use exact_plan::Value;

/// Reserved resource source, answered before the app data seam.
pub const SOURCE: &str = "exactViewport";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &["width", "height"];

/// The host's layout viewport, before the first settlement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Width in CSS pixels, including any scrollbar.
    pub width: f64,
    /// Height in CSS pixels, under the host's interactive-widget policy.
    pub height: f64,
}

impl Default for Viewport {
    /// The bake's LINT_VIEWPORT: 390 × 844 (LLP 1039 D3).
    fn default() -> Self {
        Self {
            width: 390.0,
            height: 844.0,
        }
    }
}

impl Viewport {
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
            _ => None,
        }
    }
}
