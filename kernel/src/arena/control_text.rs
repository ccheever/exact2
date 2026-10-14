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

/// The neutral look's sheet says more on a platform field or button than the
/// reset does (LLP 1115.000 D4 part B; `host/web/neutral.css`), and what it
/// says stops at the control as the reset's rows do: its text sits on a line
/// of the look's height (1.25 of its size, so a control is 36 px in every
/// engine), a field's ink is the label's whatever the page's `color`, and a
/// button's title is the look's weight. Each is a row of the schema's
/// `metrics`, so a kernel that lays the control out (a host that paints its
/// own, D17) sizes the box the page draws. `[field, button, projected]`.
static NEUTRAL_STARTS: std::sync::LazyLock<[StyleProps; 3]> = std::sync::LazyLock::new(|| {
    use crate::generated::{metric, MetricValue, METRICS};
    let number = |index: usize| match METRICS[index].neutral {
        MetricValue::Number(n) => n as f32,
        _ => unreachable!("the neutral look has this row"),
    };
    let on_line = |mut start: StyleProps| {
        start.line_height = crate::LineHeight::Number(number(metric::CONTROL_LINE_HEIGHT));
        start.mask.set(StyleId::LineHeight);
        start
    };
    let mut field = on_line(WEB_START.clone());
    field.text_color =
        crate::ColorValue::Role(crate::style::roles::role("-exact-label").expect("schema role"));
    field.mask.set(StyleId::TextColor);
    let titled = |start: &StyleProps| {
        let mut start = on_line(start.clone());
        start.font_weight = number(metric::CONTROL_WEIGHT) as u16;
        start.mask.set(StyleId::FontWeight);
        start
    };
    [
        field,
        titled(&WEB_BUTTON_START),
        titled(&WEB_PROJECTED_BUTTON_START),
    ]
});

impl NodeArena {
    /// Native text controls have exactly this predicate (LLP 1104 D1). A
    /// `textfield` is the platform's field too, without a number field's
    /// stepper (LLP 1116.007 D4); only `none` is the author's box.
    pub fn is_native_text_control(&self, slot: u32) -> bool {
        self.node_type(slot) == NodeType::TextInput
            && self.style(slot).appearance != Appearance::None
    }

    pub(crate) fn refresh_control_styles(&mut self) {
        self.control_styles = self.env.control_text_styles.as_ref().map(|styles| {
            Box::new(std::array::from_fn(|index| {
                if index < 2 {
                    return starting_style(if index == 0 {
                        &styles.field
                    } else {
                        &styles.textarea
                    });
                }
                let size = [
                    crate::ControlSize::Mini,
                    crate::ControlSize::Small,
                    crate::ControlSize::Medium,
                    crate::ControlSize::Large,
                ][(index - 2) % 4];
                let mut button =
                    starting_style(styles.button_font(size, self.env.button_fonts.as_ref()));
                button.text_align = crate::TextAlign::Center;
                button.mask.set(StyleId::WhiteSpace);
                button.text_color = crate::ColorValue::Role(
                    crate::style::roles::role("ButtonText").expect("schema role"),
                );
                if index >= 6 {
                    projected(button)
                } else {
                    button
                }
            }))
        });
    }

    pub(crate) fn control_font(&self, slot: u32) -> Option<&ControlFont> {
        if !self.is_native_text_control(slot) && !self.is_native_button(slot) {
            return None;
        }
        let styles = self.env.control_text_styles.as_ref()?;
        Some(if self.is_native_button(slot) {
            styles.button_font(
                self.style(slot).control_size,
                self.env.button_fonts.as_ref(),
            )
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
                6 + self.button_font_index(slot)
            } else {
                2 + self.button_font_index(slot)
            }
        } else {
            usize::from(FieldKind::from_props(self.props(slot)) == FieldKind::Textarea)
        };
        Some(self.control_styles.as_ref().map_or_else(
            || {
                let kind = if index >= 6 {
                    2
                } else if index >= 2 {
                    1
                } else {
                    0
                };
                if crate::style::metrics::column() == crate::style::metrics::Column::Neutral {
                    &NEUTRAL_STARTS[kind]
                } else {
                    [
                        &*WEB_START,
                        &*WEB_BUTTON_START,
                        &*WEB_PROJECTED_BUTTON_START,
                    ][kind]
                }
            },
            |s| &s[index],
        ))
    }

    fn button_font_index(&self, slot: u32) -> usize {
        match self.style(slot).control_size {
            crate::ControlSize::Mini => 0,
            crate::ControlSize::Small => 1,
            crate::ControlSize::Medium => 2,
            crate::ControlSize::Large => 3,
        }
    }

    /// Content box in the field's local frame, including no chrome/padding/border.
    /// None before layout, for hidden nodes, and for bare/non-field nodes.
    pub fn field_content_rect(&self, slot: u32) -> Option<Frame> {
        self.is_native_text_control(slot)
            .then(|| self.field_content.get(&slot).copied())
            .flatten()
    }
}
