//! Native control measurements and platform fonts. @ref LLP 1104 D4–D5.
//! Field chrome and native button faces share TextMeasurer's defaulted hooks.

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
/// Apple supplies these before the first layout; the web inherits the page.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlTextStyles {
    /// Single-line field, including secure and search fields.
    pub field: ControlFont,
    /// Multi-line field.
    pub textarea: ControlFont,
    /// Native button.
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

/// Optional platform fonts for each native button size. Missing entries use
/// `ControlTextStyles::button`; hosts obtain these from their platform.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ButtonFonts {
    /// Mini control font.
    pub mini: Option<ControlFont>,
    /// Small control font.
    pub small: Option<ControlFont>,
    /// Medium (default) control font.
    pub medium: Option<ControlFont>,
    /// Large control font.
    pub large: Option<ControlFont>,
}

impl ButtonFonts {
    pub(crate) fn get(&self, size: crate::ControlSize) -> Option<&ControlFont> {
        match size {
            crate::ControlSize::Mini => &self.mini,
            crate::ControlSize::Small => &self.small,
            crate::ControlSize::Medium => &self.medium,
            crate::ControlSize::Large => &self.large,
        }
        .as_ref()
    }

    pub(crate) fn is_valid(&self) -> bool {
        [&self.mini, &self.small, &self.medium, &self.large]
            .into_iter()
            .flatten()
            .all(|f| f.size.is_finite() && f.size > 0.0)
    }
}

impl ControlTextStyles {
    /// Select the size's platform font, retaining the original fallback.
    pub fn button_font<'a>(
        &'a self,
        size: crate::ControlSize,
        fonts: Option<&'a ButtonFonts>,
    ) -> &'a ControlFont {
        fonts.and_then(|f| f.get(size)).unwrap_or(&self.button)
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

/// Image side resolved from the face axis and child order (D3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonImagePlacement {
    /// Before the title in a row.
    #[default]
    Leading,
    /// After the title in a row.
    Trailing,
    /// Before the title in a column.
    Top,
    /// After the title in a column.
    Bottom,
}

/// Resolved values, with each StyleProps mask marking only rows authored on
/// the button or that semantic child (D1–D2). Values are computed, including
/// em resolution and the web's inheritance; an unmarked row is the platform's
/// own answer on Apple, not an instruction to write the computed initial.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonFaceStyle {
    /// Title typography; its own rows win over the button's.
    pub title: crate::StyleProps,
    /// Subtitle typography, resolved independently in the same way.
    pub subtitle: Option<crate::StyleProps>,
    /// Symbol tint/font; only rows on the image are marked authored (D14).
    pub symbol: crate::StyleProps,
    /// Button padding, radii and platform size/corners, per-side masks included.
    pub button: crate::StyleProps,
    /// Image-to-text gap on the face axis. None asks the host's system spacing.
    pub image_gap: Option<f32>,
}

impl ButtonFaceStyle {
    pub(crate) fn resolve_geometry(
        &mut self,
        env: &crate::Env,
        containing_width: Option<f32>,
        frame: crate::Frame,
    ) {
        let length = |d: crate::Dimension, basis: Option<f32>| {
            crate::Dimension::Points(
                match d.resolve(env) {
                    crate::Dimension::Points(n) => n,
                    crate::Dimension::Percent(p) => basis.map_or(0.0, |b| b * p / 100.0),
                    crate::Dimension::Calc(p, n) => basis.map_or(0.0, |b| b * p / 100.0 + n),
                    _ => 0.0,
                }
                .max(0.0),
            )
        };
        let s = &mut self.button;
        for side in [
            &mut s.padding_top,
            &mut s.padding_right,
            &mut s.padding_bottom,
            &mut s.padding_left,
        ] {
            *side = length(*side, containing_width);
        }
        // Native configurations and the painted button use circular corners;
        // retain their scalar-radius approximation for percentage corners.
        let basis = Some(frame.width.min(frame.height));
        for corner in [
            &mut s.border_radius_top_left,
            &mut s.border_radius_top_right,
            &mut s.border_radius_bottom_right,
            &mut s.border_radius_bottom_left,
        ] {
            *corner = length(*corner, basis);
        }
    }
}

/// A native button's semantic face and resolved configuration (D11).
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonMeasureRequest {
    /// Title, subtitle, symbol and placement.
    pub face: crate::PressFace,
    /// Resolved rows with authored masks.
    pub style: ButtonFaceStyle,
    /// Name from buttonStyles; bordered when absent.
    pub button_style: String,
    /// The button is a direct grouped-list row; the host includes its slot inset.
    pub grouped_row: bool,
    /// Offered border-box width; hosts answer height-for-width.
    pub width: crate::AxisOffer,
}

/// The host's border-box measurement in logical pixels (D11).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonMeasure {
    /// Fitting width.
    pub width: f32,
    /// Fitting height at the offered width.
    pub height: f32,
    /// A stand-in; the host must remeasure before it presents a frame.
    pub provisional: bool,
}

impl ButtonMeasure {
    /// Both axes must be finite and nonnegative.
    pub fn is_valid(self) -> bool {
        self.width.is_finite() && self.width >= 0.0 && self.height.is_finite() && self.height >= 0.0
    }
}
