//! Style types for Block layout
use crate::style::AlignContent;
use crate::{CoreStyle, Style};

/// The set of styles required for a Block layout container
pub trait BlockContainerStyle: CoreStyle {
    /// Defines which row in the grid the item should start and end at
    #[inline(always)]
    fn text_align(&self) -> TextAlign {
        Style::<Self::CustomIdent>::DEFAULT.text_align
    }

    /// How children of this block container are aligned in the block (cross) axis
    #[inline(always)]
    fn align_content(&self) -> Option<AlignContent> {
        Style::<Self::CustomIdent>::DEFAULT.align_content
    }

    /// EXACT PATCH 27: the container's multi-column layout, if it is one
    #[inline(always)]
    fn multicol(&self) -> Option<Multicol> {
        None
    }
}

/// EXACT PATCH 27: a multi-column container (CSS Multi-column Layout 1). Its
/// in-flow children are laid out once, as one column of the used column
/// width (the *flow thread*); cutting that column into columns is the
/// caller's (Exact's kernel, LLP 1093 D2–D5).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Multicol {
    /// `column-count`; `None` is `auto`.
    pub count: Option<u16>,
    /// `column-width` in points; `None` is `auto`.
    pub width: Option<f32>,
    /// The used gap between columns, in points.
    pub gap: f32,
    /// The columns' content-box height, once the caller has cut the flow
    /// thread. It sizes an automatic height only: the container's outer
    /// height is this plus its vertical padding and border, clamped by
    /// `min-height` and `max-height`. Never a percentage basis, so the
    /// children's inputs, and their cache entries, stand.
    pub used_height: Option<f32>,
    /// The fragmented scrollable overflow, in the container's border-box
    /// space, once the caller has cut the flow thread: it replaces the flow
    /// thread's, which is one tall column.
    pub overflow: Option<crate::geometry::Rect<f32>>,
}

impl Multicol {
    /// The used column count and width for a content-box width `available`:
    /// CSS Multi-column Layout 1 §3.4's pseudo-algorithm.
    pub fn columns(&self, available: f32) -> (u32, f32) {
        let gap = self.gap;
        let fit = |w: f32| (((available + gap) / (w + gap)).floor() as u32).max(1);
        let n = match (self.count, self.width) {
            (Some(n), None) => return (n.max(1) as u32, ((available - (n.max(1) - 1) as f32 * gap) / n.max(1) as f32).max(0.0)),
            (None, Some(w)) => fit(w),
            (Some(n), Some(w)) => fit(w).min(n.max(1) as u32),
            (None, None) => 1,
        };
        (n, ((available + gap) / n as f32 - gap).max(0.0))
    }
}

/// The set of styles required for a Block layout item (child of a Block container)
pub trait BlockItemStyle: CoreStyle {
    /// Whether the item is a table. Table children are handled specially in block layout.
    #[inline(always)]
    fn is_table(&self) -> bool {
        false
    }

    /// Whether the item is a floated
    #[cfg(feature = "float_layout")]
    #[inline(always)]
    fn float(&self) -> super::Float {
        super::Float::None
    }

    /// Whether the item is a floated
    #[cfg(feature = "float_layout")]
    #[inline(always)]
    fn clear(&self) -> super::Clear {
        super::Clear::None
    }
}

/// Used by block layout to implement the legacy behaviour of `<center>` and `<div align="left | right | center">`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TextAlign {
    /// No special legacy text align behaviour.
    #[default]
    Auto,
    /// Corresponds to `-webkit-left` or `-moz-left` in browsers
    LegacyLeft,
    /// Corresponds to `-webkit-right` or `-moz-right` in browsers
    LegacyRight,
    /// Corresponds to `-webkit-center` or `-moz-center` in browsers
    LegacyCenter,
}

#[cfg(feature = "parse")]
crate::util::parse::impl_parse_for_keyword_enum!(TextAlign,
    "auto" => Auto,
    "-webkit-left" => LegacyLeft,
    "-webkit-right" => LegacyRight,
    "-webkit-center" => LegacyCenter,
);
