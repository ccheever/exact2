//! Native control measurements and platform fonts. @ref LLP 1104 D4–D5.
//! Buttons will use the same host seam later; only field chrome is measured here.

use crate::{FontStyle, PropId, PropList};

/// A platform font, with the id under which the host registered its family.
/// The existing computed `font-family` row and text runs carry that id; the
/// host resolves it to `family`, including a platform-only font name.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlFont {
    /// Family name resolved by the host (possibly a platform font name).
    pub family: String,
    /// Host font registry/plan stack id, as in `TextStyle::font_family`.
    pub family_id: u16,
    /// Logical pixels, finite and positive.
    pub size: f32,
    /// CSS font weight.
    pub weight: u16,
    /// Normal or italic.
    pub style: FontStyle,
}

/// The platform's starting font for each kind. Secure/search use `field`.
/// The button font is carried now but has no kernel behaviour yet.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlTextStyles {
    /// Single-line field, including secure and search fields.
    pub field: ControlFont,
    /// Multi-line field.
    pub textarea: ControlFont,
    /// Native button, reserved for the button implementation.
    pub button: ControlFont,
}

impl ControlTextStyles {
    /// Every supplied size is finite and strictly positive.
    pub fn is_valid(&self) -> bool {
        [&self.field, &self.textarea, &self.button]
            .into_iter()
            .all(|f| f.size.is_finite() && f.size > 0.0)
    }
}

/// The native text control whose chrome the host measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// Ordinary single-line field (typed keyboards included).
    Field,
    /// Password field.
    SecureField,
    /// Search field.
    SearchField,
    /// Multi-line field.
    Textarea,
}

impl FieldKind {
    /// Kind from the text control's props, without deciding its appearance.
    pub fn from_props(props: &PropList) -> Self {
        if props.str(PropId::SemanticTag) == Some("textarea") {
            return Self::Textarea;
        }
        match props.str(PropId::Type) {
            Some("password") => Self::SecureField,
            Some("search") => Self::SearchField,
            _ => Self::Field,
        }
    }
}

/// The resolved font and kind; a control-size axis can be added beside them
/// when that row lands. The host includes its current traits in its own cache.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldChromeRequest {
    /// Secure and search controls have distinct chrome/cache entries.
    pub kind: FieldKind,
    /// Computed font. An authored family id resolves through the host's
    /// existing catalog; `family` is empty when the kernel has no name for it.
    pub font: ControlFont,
}

/// Chrome in logical pixels, outside authored content, padding and borders.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FieldChrome {
    /// Top inset.
    pub top: f32,
    /// Right inset.
    pub right: f32,
    /// Bottom inset.
    pub bottom: f32,
    /// Left inset.
    pub left: f32,
    /// Minimum frame height, including chrome; used only for single-line fields.
    pub minimum_height: f32,
    /// A stand-in answer; the host must measure and request relayout before paint.
    pub provisional: bool,
}

impl FieldChrome {
    /// All geometry is finite and nonnegative.
    pub fn is_valid(self) -> bool {
        [
            self.top,
            self.right,
            self.bottom,
            self.left,
            self.minimum_height,
        ]
        .into_iter()
        .all(|v| v.is_finite() && v >= 0.0)
    }
}
