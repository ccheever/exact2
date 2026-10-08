//! The inheritance boundary of a platform field. @ref LLP 1104 D4.
use super::*;
use crate::{Appearance, ControlFont, FieldKind};

/// Contract has no word-spacing row. All its other UA typography rows stop.
pub(crate) fn stopped_rows() -> StyleMask {
    let mut rows = StyleMask::EMPTY;
    for id in [
        StyleId::FontFamily,
        StyleId::FontSize,
        StyleId::FontWeight,
        StyleId::FontStyle,
        StyleId::LineHeight,
        StyleId::LetterSpacing,
        StyleId::TextTransform,
        StyleId::TextIndent,
        StyleId::TextShadow,
        StyleId::TextAlign,
        StyleId::TextColor,
    ] {
        rows.set(id);
    }
    rows
}

fn starting_style(font: &ControlFont) -> StyleProps {
    let mut s = StyleProps::default();
    s.font_family = font.family_id;
    s.font_size = font.size;
    s.font_weight = font.weight;
    s.font_style = font.style;
    s.text_color =
        crate::ColorValue::Role(crate::style::roles::role("FieldText").expect("schema role"));
    // Mark even initial-valued resets: hosts must receive the boundary rather
    // than inherit the ancestor's authored row in their presentation tree.
    s.mask = stopped_rows();
    s
}

// The web reset inherits font/spacing/colour, but all: revert restores these
// four UA rows. This boundary also applies before any environment update.
static WEB_START: std::sync::LazyLock<StyleProps> = std::sync::LazyLock::new(|| {
    let mut start = StyleProps::default();
    for row in [
        StyleId::TextAlign,
        StyleId::TextIndent,
        StyleId::TextShadow,
        StyleId::TextTransform,
    ] {
        start.mask.set(row);
    }
    start
});

impl NodeArena {
    /// Native text controls have exactly this predicate (LLP 1104 D1).
    pub fn is_native_text_control(&self, slot: u32) -> bool {
        self.node_type(slot) == NodeType::TextInput
            && self.style(slot).appearance == Appearance::Auto
    }

    pub(crate) fn refresh_control_styles(&mut self) {
        self.control_styles = self.env.control_text_styles.as_ref().map(|styles| {
            Box::new([
                starting_style(&styles.field),
                starting_style(&styles.textarea),
            ])
        });
    }

    pub(crate) fn control_font(&self, slot: u32) -> Option<&ControlFont> {
        if !self.is_native_text_control(slot) {
            return None;
        }
        let styles = self.env.control_text_styles.as_ref()?;
        Some(
            if FieldKind::from_props(self.props(slot)) == FieldKind::Textarea {
                &styles.textarea
            } else {
                &styles.field
            },
        )
    }

    /// The stopped rows' starting values, with no authored overrides.
    pub(crate) fn control_text_start(&self, slot: u32) -> Option<&StyleProps> {
        if !self.is_native_text_control(slot) {
            return None;
        }
        let index = usize::from(FieldKind::from_props(self.props(slot)) == FieldKind::Textarea);
        Some(
            self.control_styles
                .as_ref()
                .map_or(&*WEB_START, |s| &s[index]),
        )
    }

    /// Content box in the field's local frame, including no chrome/padding/border.
    /// None before layout, for hidden nodes, and for bare/non-field nodes.
    pub fn field_content_rect(&self, slot: u32) -> Option<Frame> {
        self.is_native_text_control(slot)
            .then(|| self.field_content.get(&slot).copied())
            .flatten()
    }
}
