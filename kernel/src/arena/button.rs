//! Semantic button faces, shared by host queries and layout. LLP 1069.011.001 D1–D3.
use super::*;
use crate::{ButtonFaceStyle, ButtonImagePlacement, ControlKind, PressFace};

const TITLE_ROWS: &[StyleId] = &[
    StyleId::FontSize,
    StyleId::FontWeight,
    StyleId::TextColor,
    StyleId::WhiteSpace,
    StyleId::LineClamp,
    StyleId::TextAlign,
];

fn mask(rows: &[StyleId]) -> StyleMask {
    let mut mask = StyleMask::EMPTY;
    for &row in rows {
        mask.set(row);
    }
    mask
}

impl NodeArena {
    pub(crate) fn is_native_button(&self, slot: u32) -> bool {
        ControlKind::of(self.node_type(slot), self.props(slot)) == Some(ControlKind::Button)
    }

    pub(crate) fn press_face(&self, slot: u32) -> Option<PressFace> {
        if !self.is_native_button(slot) && self.node_type(slot) != NodeType::Pressable {
            return None;
        }
        let mut face = PressFace {
            label: self
                .props(slot)
                .str(PropId::AccessibilityLabel)
                .map(str::to_owned),
            fits: true,
            ..PressFace::default()
        };
        let (mut texts, mut images) = (0, 0);
        for &child in self.children(slot) {
            match self.node_type(child) {
                NodeType::Text => {
                    texts += 1;
                    if texts <= 2 {
                        let mut runs = Vec::new();
                        self.text_runs(child, &mut runs);
                        let text: String = runs.iter().map(|r| &*r.text).collect();
                        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                        let text = (!text.is_empty()).then_some(text);
                        if texts == 1 {
                            face.title = text;
                        } else {
                            face.subtitle = text;
                        }
                    }
                }
                NodeType::Image => {
                    images += 1;
                    if images == 1 {
                        let source = self.props(child).str(PropId::ImageSource);
                        match source.and_then(|s| s.strip_prefix("symbol:")) {
                            Some(symbol) => face.symbol = Some(symbol.to_owned()),
                            None => face.raster = source.is_some_and(|s| !s.is_empty()),
                        }
                        face.leading = face.title.is_none() && face.subtitle.is_none();
                    }
                }
                _ => face.fits = false,
            }
        }
        face.fits &= texts <= 2 && images <= 1;
        face.placement = match (self.style(slot).flex_direction, face.leading) {
            (crate::FlexDirection::Column | crate::FlexDirection::ColumnReverse, true) => {
                ButtonImagePlacement::Top
            }
            (crate::FlexDirection::Column | crate::FlexDirection::ColumnReverse, false) => {
                ButtonImagePlacement::Bottom
            }
            (_, true) => ButtonImagePlacement::Leading,
            (_, false) => ButtonImagePlacement::Trailing,
        };
        Some(face)
    }

    pub(crate) fn button_face_style(&self, slot: u32) -> Option<ButtonFaceStyle> {
        if !self.is_native_button(slot) {
            return None;
        }
        let own = self.style(slot);
        let title_mask = mask(TITLE_ROWS);
        let text_style = |child: Option<u32>| {
            let at = child.unwrap_or(slot);
            let mut style = self.computed_style(at, title_mask);
            // Non-inheriting rows (line-clamp) also map from button to title.
            for &row in TITLE_ROWS {
                if !self.style(at).mask.has(row) && own.mask.has(row) {
                    let mut one = StyleMask::EMPTY;
                    one.set(row);
                    style.copy_rows(own, one);
                }
            }
            style.mask = own.mask.union(self.style(at).mask).intersect(title_mask);
            style
        };
        let texts: Vec<_> = self
            .children(slot)
            .iter()
            .copied()
            .filter(|&c| self.node_type(c) == NodeType::Text)
            .collect();
        let title = text_style(texts.first().copied());
        let image = self
            .children(slot)
            .iter()
            .copied()
            .find(|&c| self.node_type(c) == NodeType::Image);
        let mut symbol = image.map_or_else(StyleProps::default, |c| {
            self.computed_style(c, StyleMask::INHERITED)
        });
        symbol.mask = image
            .map_or(StyleMask::EMPTY, |c| self.style(c).mask)
            .intersect(mask(&[
                StyleId::TintColor,
                StyleId::FontSize,
                StyleId::FontWeight,
            ]));
        if !symbol.mask.has(StyleId::FontSize) {
            symbol.font_size = title.font_size;
        }
        if !symbol.mask.has(StyleId::FontWeight) {
            symbol.font_weight = title.font_weight;
        }
        let mut button = self.computed_style(slot, StyleMask::INHERITED);
        button.mask = own.mask.intersect(mask(&[
            StyleId::PaddingTop,
            StyleId::PaddingRight,
            StyleId::PaddingBottom,
            StyleId::PaddingLeft,
            StyleId::BorderRadiusTopLeft,
            StyleId::BorderRadiusTopRight,
            StyleId::BorderRadiusBottomRight,
            StyleId::BorderRadiusBottomLeft,
            StyleId::ControlSize,
            StyleId::ControlCornerStyle,
            StyleId::RowGap,
            StyleId::ColumnGap,
            StyleId::FlexDirection,
        ]));
        let column = matches!(
            own.flex_direction,
            crate::FlexDirection::Column | crate::FlexDirection::ColumnReverse
        );
        let (gap_row, gap) = if column {
            (StyleId::RowGap, own.row_gap)
        } else {
            (StyleId::ColumnGap, own.column_gap)
        };
        Some(ButtonFaceStyle {
            title,
            subtitle: texts.get(1).map(|&c| text_style(Some(c))),
            symbol,
            button,
            image_gap: own.mask.has(gap_row).then_some(gap),
        })
    }
}
