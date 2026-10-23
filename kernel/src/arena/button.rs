//! Semantic button faces, shared by host queries and layout. LLP 1069.011.001 D1–D3.
use super::*;
use crate::{ButtonFaceStyle, ButtonImagePlacement, ControlKind, PressFace};

pub(crate) struct ButtonInputs {
    env: crate::Env,
    neutral_tab_root: Option<f32>,
    neutral_role_floor: bool,
    /// A list row's inset (LLP 1138), which the face's padding takes.
    row_inset: Option<[f32; 2]>,
    nodes: Vec<ButtonNode>,
}

struct ButtonNode {
    key: NodeKey,
    kind: NodeType,
    style: Rc<StyleProps>,
    props: PropList,
    inherited: InheritedStyle,
    paragraph: Option<ParagraphStamp>,
    /// Raster metadata changes the face without a prop or style write.
    intrinsic: Option<(f32, f32)>,
}

impl ButtonInputs {
    pub(crate) fn matches(&self, arena: &NodeArena, slot: u32) -> bool {
        if self.env != *arena.env()
            || self.neutral_tab_root != arena.neutral_tab_root(slot)
            || self.neutral_role_floor != arena.neutral_role_floor(slot)
            || self.row_inset != crate::rows::row_inset(arena, slot)
        {
            return false;
        }
        let slots = arena.subtree(slot);
        self.nodes.len() == slots.len()
            && self.nodes.iter().zip(slots).all(|(held, s)| {
                held.key == arena.key(s)
                    && held.kind == arena.node_type(s)
                    && (Rc::ptr_eq(&held.style, &arena.styles[s as usize])
                        || *held.style == *arena.style(s))
                    && held.props == *arena.props(s)
                    && held.paragraph == arena.paragraph_stamp(s)
                    && held.intrinsic
                        == arena
                            .intrinsic(s)
                            .filter(|_| arena.node_type(s) == NodeType::Image)
                    && held
                        .inherited
                        .changed_mask(&arena.computed_inherited(s))
                        .is_empty()
            })
    }
}

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

    pub(crate) fn neutral_tab_root(&self, slot: u32) -> Option<f32> {
        (self.is_neutral_tab(slot)
            && self.is_native_button(slot)
            && matches!(
                self.props(slot)
                    .str(PropId::ButtonStyle)
                    .unwrap_or("bordered"),
                "bordered" | "gray"
            ))
        .then(|| self.root_font_size())
    }

    pub(crate) fn neutral_tab_min_height(&self, slot: u32) -> Option<f32> {
        let root = self.neutral_tab_root(slot)?;
        let own = self.style(slot);
        if own.mask.union(own.written).has(StyleId::MinHeight) {
            return None;
        }
        // A known authored height bypasses the intrinsic measurement;
        // CSS's minimum must also constrain that engine size.
        crate::style::metrics::length_in(
            crate::generated::metric::TABBAR_HEIGHT,
            crate::style::metrics::Column::Neutral,
            self.computed_source(slot, StyleId::FontSize).font_size,
            root,
        )
    }

    fn neutral_role_floor(&self, slot: u32) -> bool {
        if matches!(
            self.props(slot).str(PropId::AccessibilityRole),
            Some("tab" | "radio")
        ) {
            return true;
        }
        let mut parent = self.parent(slot);
        while let Some(at) = parent {
            if self.props(at).str(PropId::AccessibilityRole) == Some("menu") {
                return true;
            }
            parent = self.parent(at);
        }
        false
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
                            None => {
                                face.raster = source.is_some_and(|s| !s.is_empty());
                                face.image = source.filter(|s| !s.is_empty()).map(|s| {
                                    crate::control::ButtonRasterImage {
                                        view: self.local_id(child),
                                        source: s.to_owned(),
                                        size: self.intrinsic(child),
                                    }
                                });
                            }
                        }
                        face.leading = face.title.is_none() && face.subtitle.is_none();
                    }
                }
                _ => face.fits = false,
            }
        }
        face.fits &= texts <= 1 && images <= 1;
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

    pub(crate) fn button_inputs(&self, slot: u32) -> ButtonInputs {
        ButtonInputs {
            env: self.env.clone(),
            neutral_tab_root: self.neutral_tab_root(slot),
            neutral_role_floor: self.neutral_role_floor(slot),
            row_inset: crate::rows::row_inset(self, slot),
            nodes: self
                .subtree(slot)
                .into_iter()
                .map(|s| ButtonNode {
                    key: self.key(s),
                    kind: self.node_type(s),
                    style: self.styles[s as usize].clone(),
                    props: self.props(s).clone(),
                    inherited: self.computed_inherited(s),
                    paragraph: self.paragraph_stamp(s),
                    intrinsic: self
                        .intrinsic(s)
                        .filter(|_| self.node_type(s) == NodeType::Image),
                })
                .collect(),
        }
    }

    pub(crate) fn button_face_style(&self, slot: u32) -> Option<ButtonFaceStyle> {
        let mut face = self.button_face_style_unresolved(slot)?;
        face.resolve_geometry(
            self.env(),
            self.button_bases.get(&slot).copied().flatten(),
            self.frame(slot),
        );
        Some(face)
    }

    pub(crate) fn button_face_style_unresolved(&self, slot: u32) -> Option<ButtonFaceStyle> {
        if !self.is_native_button(slot) {
            return None;
        }
        let own = self.style(slot);
        let neutral_tab_root = self.neutral_tab_root(slot);
        let title_mask = mask(TITLE_ROWS);
        let text_style = |child: Option<u32>| {
            let at = child.unwrap_or(slot);
            let mut style = self.computed_rows(at, title_mask);
            // Non-inheriting rows (line-clamp) also map from button to title.
            for &row in TITLE_ROWS {
                // A text style the title names gives its own (LLP 1115.000
                // D12.2).
                if !self.style(at).mask.has(row)
                    && own.mask.has(row)
                    && self.text_style_source(at, row).is_none()
                {
                    let mut one = StyleMask::EMPTY;
                    one.set(row);
                    style.copy_rows(own, one);
                }
            }
            style.mask = own.mask.union(self.style(at).mask).intersect(title_mask);
            // A text style's weight and leading (LLP 1115.000 D12.2) are the
            // title's as much as its own rows are.
            for row in [StyleId::FontWeight, StyleId::LineHeight] {
                if title_mask.has(row) && self.text_style_source(at, row).is_some() {
                    style.mask.set(row);
                }
            }
            // Bold Text's step (LLP 1115.000 D12.3) on the weight the title
            // has, the button's included, said so the host paints it.
            if self.bold_text {
                let family = self.computed_source(at, StyleId::FontFamily).font_family;
                let weight = self.emboldened(at, style.font_weight, family);
                if weight != style.font_weight {
                    style.font_weight = weight;
                    style.mask.set(StyleId::FontWeight);
                }
            }
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
        if !symbol.mask.has(StyleId::FontSize)
            && !(neutral_tab_root.is_some() && symbol.written.has(StyleId::FontSize))
        {
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
        // A list's row inset (LLP 1138) is the padding the host measures and
        // places the title by, at an end its author wrote none of.
        if let Some([left, right]) = crate::rows::row_inset(self, slot) {
            for (id, length) in [(StyleId::PaddingLeft, left), (StyleId::PaddingRight, right)] {
                if !own.written.has(id) {
                    let side = match id {
                        StyleId::PaddingLeft => &mut button.padding_left,
                        _ => &mut button.padding_right,
                    };
                    *side = crate::style::Dimension::Points(length);
                    button.mask.set(id);
                }
            }
        }
        // The Neutral tab's floor is below an own minimum, including a clear.
        // Keep this provenance when narrowing the host's configuration rows.
        if neutral_tab_root.is_some() && own.mask.has(StyleId::MinHeight) {
            button.mask.set(StyleId::MinHeight);
        }
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
            neutral_tab_root,
            neutral_role_floor: self.neutral_role_floor(slot),
            title,
            subtitle: texts.get(1).map(|&c| text_style(Some(c))),
            symbol,
            button,
            image_gap: own.mask.has(gap_row).then_some(gap),
        })
    }
}

impl crate::style::wide::Colors for ButtonInputs {
    fn colors(&self, m: &mut crate::style::wide::WideMark) {
        for node in &self.nodes {
            node.style.colors(m);
            node.inherited.colors(m);
        }
    }
}
