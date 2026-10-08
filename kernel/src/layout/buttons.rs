//! Native button measurement before publication. LLP 1069.011.001 D11.
use super::*;
use crate::{ButtonMeasureRequest, PropId};

impl LayoutTree {
    pub(super) fn prepare_buttons(
        &mut self,
        root: NodeId,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
    ) -> Result<IdMap<u32, ButtonMeasureRequest>, LayoutError> {
        let mut requests = IdMap::default();
        if !arena.has_type(NodeType::Control) {
            return Ok(requests);
        }
        let mut nodes = Vec::new();
        for (&node, &slot) in &self.slots {
            if !arena.is_native_button(slot) {
                continue;
            }
            let mut at = Some(node);
            while let Some(n) = at {
                if self
                    .taffy
                    .style(n)
                    .is_ok_and(|s| s.display == taffy::Display::None)
                {
                    break;
                }
                if n == root {
                    nodes.push((node, slot));
                    break;
                }
                at = self.taffy.parent(n);
            }
        }
        for (node, slot) in nodes {
            let request = ButtonMeasureRequest {
                face: arena.press_face(slot).expect("button"),
                style: arena.button_face_style(slot).expect("button"),
                button_style: arena
                    .props(slot)
                    .str(PropId::ButtonStyle)
                    .unwrap_or("bordered")
                    .to_owned(),
                width: AxisOffer::MaxContent,
            };
            if let Some(answer) = measurer.button_measure(&request) {
                if !answer.is_valid() {
                    return Err(LayoutError::InvalidButtonMeasure(arena.local_id(slot)));
                }
                self.provisional_chrome |= answer.provisional;
                // Query the host again at this pass's actual widths, even when
                // Taffy could reuse a prior provisional geometry or offer.
                // None above preserves the existing intrinsic/default path.
                self.mark_dirty(node);
                requests.insert(slot, request);
            }
        }
        Ok(requests)
    }
}

/// The host sizes a border box; Taffy's leaf callback sizes its content box.
/// Add the engine's insets to the offer and remove them from the answer so
/// authored padding reaches the platform exactly once.
pub(super) fn measure(
    request: &ButtonMeasureRequest,
    measurer: &mut dyn TextMeasurer,
    known: Size<Option<f32>>,
    space: Size<AvailableSpace>,
    inset: taffy::Rect<f32>,
) -> Option<crate::ButtonMeasure> {
    let horizontal = inset.left + inset.right;
    let mut request = request.clone();
    request.width = known.width.map_or_else(
        || match space.width {
            AvailableSpace::Definite(w) => AxisOffer::Definite(w + horizontal),
            AvailableSpace::MinContent => AxisOffer::MinContent,
            AvailableSpace::MaxContent => AxisOffer::MaxContent,
        },
        |w| AxisOffer::Definite(w + horizontal),
    );
    measurer.button_measure(&request)
}
