//! The inheritance boundary of a platform field or button. @ref LLP 1104 D4.
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

static WEB_BUTTON_START: std::sync::LazyLock<StyleProps> = std::sync::LazyLock::new(|| {
    let mut start = WEB_START.clone();
    start.text_align = crate::TextAlign::Center;
    start.white_space = crate::WhiteSpace::Normal;
    start.mask.set(StyleId::WhiteSpace);
    start
});

// A projection's face is still read as painted (LLP 1069.011.000 D1).
// Existing native tabs inherit a title transform from their tablist; do not
// silently change that projection while adding UIButton's typography reset.
fn projected(mut style: StyleProps) -> StyleProps {
    let mut transform = StyleMask::EMPTY;
    transform.set(StyleId::TextTransform);
    style.mask = style.mask.minus(transform);
    style
}
static WEB_PROJECTED_BUTTON_START: std::sync::LazyLock<StyleProps> =
    std::sync::LazyLock::new(|| projected(WEB_BUTTON_START.clone()));

impl NodeArena {
    /// Native text controls have exactly this predicate (LLP 1104 D1).
    pub fn is_native_text_control(&self, slot: u32) -> bool {
        self.node_type(slot) == NodeType::TextInput
            && self.style(slot).appearance == Appearance::Auto
    }

    pub(crate) fn refresh_control_styles(&mut self) {
        self.control_styles = self.env.control_text_styles.as_ref().map(|styles| {
            let mut button = starting_style(&styles.button);
            button.text_align = crate::TextAlign::Center;
            button.mask.set(StyleId::WhiteSpace);
            button.text_color = crate::ColorValue::Role(
                crate::style::roles::role("ButtonText").expect("schema role"),
            );
            let projection = projected(button.clone());
            Box::new([
                starting_style(&styles.field),
                starting_style(&styles.textarea),
                button,
                projection,
            ])
        });
    }

    pub(crate) fn control_font(&self, slot: u32) -> Option<&ControlFont> {
        if !self.is_native_text_control(slot) && !self.is_native_button(slot) {
            return None;
        }
        let styles = self.env.control_text_styles.as_ref()?;
        Some(if self.is_native_button(slot) {
            &styles.button
        } else if FieldKind::from_props(self.props(slot)) == FieldKind::Textarea {
            &styles.textarea
        } else {
            &styles.field
        })
    }

    /// The stopped rows' starting values, with no authored overrides.
    pub(crate) fn control_text_start(&self, slot: u32) -> Option<&StyleProps> {
        if !self.is_native_text_control(slot) && !self.is_native_button(slot) {
            return None;
        }
        let index = if self.is_native_button(slot) {
            if matches!(
                self.props(slot).str(crate::PropId::AccessibilityRole),
                Some("tab" | "menuitem" | "menuitemcheckbox" | "menuitemradio")
            ) {
                3
            } else {
                2
            }
        } else {
            usize::from(FieldKind::from_props(self.props(slot)) == FieldKind::Textarea)
        };
        Some(self.control_styles.as_ref().map_or_else(
            || {
                if index == 3 {
                    &*WEB_PROJECTED_BUTTON_START
                } else if index == 2 {
                    &*WEB_BUTTON_START
                } else {
                    &*WEB_START
                }
            },
            |s| &s[index],
        ))
    }

    /// Content box in the field's local frame, including no chrome/padding/border.
    /// None before layout, for hidden nodes, and for bare/non-field nodes.
    pub fn field_content_rect(&self, slot: u32) -> Option<Frame> {
        self.is_native_text_control(slot)
            .then(|| self.field_content.get(&slot).copied())
            .flatten()
    }
}
