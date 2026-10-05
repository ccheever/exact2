//! Source eligibility for the centered-fill Translate/Scale photo trial.
use crate::{
    BoxSizing, Dimension, Display, Kernel, NodeKey, NodeType, Overflow, PropId, StyleProps,
};

/// One coherent, generation-checked authored transform binding.
///
/// This proves source eligibility only. Hosts separately validate current
/// centered-fill geometry, coordinate mapping, presentation and both hold tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransformDragBinding {
    /// The authored input surface.
    pub handle: NodeKey,
    /// Its unique strict ancestor named by `transformDragFor`.
    pub target: NodeKey,
    /// The target's immediate clipping parent, never an inferred view ID.
    pub clip: NodeKey,
}

impl Kernel {
    /// Resolve a fixed centered-fill photo binding in O(depth), without a registry.
    ///
    /// The full path must be attached, displayed, enabled and non-inert. Every
    /// matching ancestor counts toward ambiguity, including ineligible ones.
    /// Target authoring is a full-size border-box View with positive uniform
    /// scale, pixel translation and no rotation. Its direct View parent clips
    /// both axes. Both boxes have zero effective border/padding; the target also
    /// has zero margins and no authored positional displacement. All other path
    /// nodes have identity transforms. No `testId`/`nativeId` lookup participates.
    ///
    /// Host geometry must still prove coincident centers and fill after layout,
    /// and reject active/held ancestor presentation transforms. A source proof
    /// alone does not admit a physical hold or publish measurement feedback.
    pub fn transform_drag_binding(&self, handle: NodeKey) -> Option<TransformDragBinding> {
        let node = self.node_by_key(handle)?;
        let name = node.props.str(PropId::TransformDragFor)?;
        if name.is_empty() {
            return None;
        }
        let arena = self.arena();
        let mut slot = handle.index;
        let mut target = None;
        loop {
            let props = arena.props(slot);
            let s = arena.style(slot);
            if s.display == Display::None
                || props.bool(PropId::Inert) == Some(true)
                || props.bool(PropId::Disabled) == Some(true)
            {
                return None;
            }
            let matching = slot != handle.index && props.str(PropId::Id) == Some(name);
            if matching {
                if target.is_some() {
                    return None;
                }
                target = Some(arena.key(slot));
            }
            if s.rotate != 0.0
                // Pixel translation only: a percentage moves with the box.
                || s.translate_percent.x != 0.0
                || s.translate_percent.y != 0.0
                || !s.translate.x.is_finite()
                || !s.translate.y.is_finite()
                || !s.scale.is_finite()
                || s.scale <= 0.0
                || (!matching && (s.scale != 1.0 || s.translate.x != 0.0 || s.translate.y != 0.0))
            {
                return None;
            }
            if arena.is_root(slot) {
                break;
            }
            slot = arena.parent(slot)?;
        }
        let target = target?;
        let clip = arena.key(arena.parent(target.index)?);
        let s = arena.style(target.index);
        let p = arena.style(clip.index);
        if arena.node_type(target.index) != NodeType::View
            || arena.is_inline_run(target.index)
            || arena.node_type(clip.index) != NodeType::View
            || arena.is_inline_run(clip.index)
            || s.width != Dimension::Percent(100.0)
            || s.height != Dimension::Percent(100.0)
            || s.box_sizing != BoxSizing::BorderBox
            || !s.transform_origin.centred()
            || !zero_insets(s)
            || !zero_insets(p)
            || ![s.margin_top, s.margin_right, s.margin_bottom, s.margin_left]
                .into_iter()
                .all(zero)
            || ![s.top, s.right, s.bottom, s.left]
                .into_iter()
                .all(|v| v == Dimension::Auto || zero(v))
            || p.overflow_x != Overflow::Hidden
            || p.overflow_y != Overflow::Hidden
        {
            return None;
        }
        Some(TransformDragBinding {
            handle,
            target,
            clip,
        })
    }
}

fn zero(v: Dimension) -> bool {
    matches!(v, Dimension::Points(0.0) | Dimension::Percent(0.0))
}

fn zero_insets(s: &StyleProps) -> bool {
    [
        s.padding_top,
        s.padding_right,
        s.padding_bottom,
        s.padding_left,
    ]
    .into_iter()
    .all(zero)
        && s.border_widths().into_iter().all(|v| v == 0.0)
}
