//! Native field chrome is engine geometry, never authored rows. LLP 1104 D5.
use super::*;
use crate::{ControlFont, FieldChrome, FieldChromeRequest, FieldKind};

impl LayoutTree {
    pub(super) fn prepare_fields(
        &mut self,
        root: NodeId,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
    ) -> Result<(), LayoutError> {
        self.provisional_chrome = false;
        if !arena.has_type(NodeType::TextInput) && self.field_chrome.is_empty() {
            return Ok(());
        }
        let mut answers = Vec::new();
        for (&node, &slot) in &self.slots {
            let native = arena.is_native_text_control(slot);
            if !native && !self.field_chrome.contains_key(&node) {
                continue;
            }
            let mut at = Some(node);
            let mut visible = true;
            let mut inside = false;
            while let Some(n) = at {
                visible &= self
                    .taffy
                    .style(n)
                    .is_ok_and(|s| s.display != taffy::Display::None);
                if n == root {
                    inside = true;
                    break;
                }
                at = self.taffy.parent(n);
            }
            if !inside || !visible {
                continue;
            }
            let chrome = if native {
                let text = arena.text_style(slot);
                let kind = FieldKind::from_props(arena.props(slot));
                let platform = arena.control_font(slot);
                let family = platform
                    .filter(|f| f.family_id == text.font_family)
                    .map_or_else(String::new, |f| f.family.clone());
                let request = FieldChromeRequest {
                    kind,
                    font: ControlFont {
                        family,
                        family_id: text.font_family,
                        size: text.font_size,
                        weight: text.font_weight,
                        style: text.font_style,
                    },
                };
                let chrome = measurer.field_chrome(&request);
                if !chrome.is_valid() {
                    return Err(LayoutError::InvalidFieldChrome(arena.local_id(slot)));
                }
                Some(chrome)
            } else {
                None
            };
            answers.push((node, slot, chrome));
        }
        // Validate all answers before changing any derived engine style.
        self.provisional_chrome = answers
            .iter()
            .any(|(_, _, c)| c.is_some_and(|c| c.provisional));
        for (node, slot, chrome) in answers {
            if let Some(chrome) = chrome {
                self.field_chrome.insert(node, chrome);
                let mut style = crate::style::taffy_style(arena, slot);
                add_chrome(&mut style, chrome);
                if chrome.minimum_height == 0.0
                    || crate::FieldKind::from_props(arena.props(slot)) == crate::FieldKind::Textarea
                {
                    self.field_minima.remove(&node);
                }
                if let Some(&minimum) = self.field_minima.get(&node) {
                    style.min_size.height = taffy::LengthPercentageAuto::length(minimum);
                }
                self.set_style(node, style);
            } else {
                self.field_chrome.remove(&node);
                self.field_minima.remove(&node);
                self.set_style(node, crate::style::taffy_style(arena, slot));
            }
        }
        Ok(())
    }

    /// Whether this layout used a stand-in chrome answer, including cache hits.
    pub fn provisional_chrome(&self) -> bool {
        self.provisional_chrome
    }

    pub(super) fn settle_field_minima(&mut self, minima: IdMap<NodeId, f32>) -> bool {
        let mut changed = false;
        for (node, minimum) in minima {
            self.field_minima.insert(node, minimum);
            let mut style = self.taffy.style(node).expect("measured node").clone();
            let next = taffy::LengthPercentageAuto::length(minimum);
            if style.min_size.height != next {
                style.min_size.height = next;
                self.set_style(node, style);
                changed = true;
            }
        }
        changed
    }
}

fn add_chrome(style: &mut taffy::Style, chrome: FieldChrome) {
    // Authored borders are absolute lengths. Chrome is kept in a separate
    // record, but is represented as additional border inset inside Taffy so
    // parents, flex/grid probes and constraints all see the full outer frame.
    for (edge, extra) in [
        (&mut style.border.top, chrome.top),
        (&mut style.border.right, chrome.right),
        (&mut style.border.bottom, chrome.bottom),
        (&mut style.border.left, chrome.left),
    ] {
        *edge = taffy::LengthPercentage::length(
            edge.resolve_or_zero(None, crate::style::resolve_calc) + extra,
        );
    }
}

/// Resolve the frame floor into the node's sizing box using this pass's
/// actual percentage bases. Returning it to the engine lets flex/grid use
/// the same floor on their next size probe, before any frame is published.
pub(super) fn minimum(
    arena: &NodeArena,
    slot: u32,
    inputs: LayoutInput,
    style: &taffy::Style,
    chrome: FieldChrome,
) -> Option<f32> {
    if chrome.minimum_height == 0.0
        || FieldKind::from_props(arena.props(slot)) == FieldKind::Textarea
    {
        return None;
    }
    let inset = style
        .padding
        .resolve_or_zero(inputs.parent_size.width, crate::style::resolve_calc)
        + style
            .border
            .resolve_or_zero(inputs.parent_size.width, crate::style::resolve_calc);
    let floor = if style.box_sizing == taffy::BoxSizing::ContentBox {
        (chrome.minimum_height - inset.top - inset.bottom).max(0.0)
    } else {
        chrome.minimum_height
    };
    let authored = crate::style::taffy_style(arena, slot)
        .min_size
        .height
        .maybe_resolve(inputs.parent_size.height, crate::style::resolve_calc)
        .unwrap_or(0.0);
    Some(authored.max(floor))
}
