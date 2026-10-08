//! Native button measurement before publication. LLP 1069.011.001 D11.
use super::*;
use crate::{ButtonMeasure, ButtonMeasureRequest, PropId};

pub(super) struct ButtonRecord {
    inputs: crate::arena::button::ButtonInputs,
    request: ButtonMeasureRequest,
    answers: Vec<(ButtonMeasureRequest, ButtonMeasure)>,
    pub(super) parent_width: Option<f32>,
    provisional: bool,
    supported: bool,
}

impl ButtonRecord {
    fn answer(
        &mut self,
        request: ButtonMeasureRequest,
        measurer: &mut dyn TextMeasurer,
    ) -> Option<ButtonMeasure> {
        if let Some((_, answer)) = self.answers.iter().find(|(held, _)| *held == request) {
            return Some(*answer);
        }
        let answer = measurer.button_measure(&request)?;
        self.provisional |= answer.provisional;
        if answer.is_valid() && !answer.provisional {
            if self.answers.len() == LEAF_OFFERS {
                self.answers.remove(0);
            }
            self.answers.push((request, answer));
        }
        Some(answer)
    }
}

impl LayoutTree {
    pub(super) fn prepare_buttons(
        &mut self,
        root: NodeId,
        arena: &NodeArena,
        measurer: &mut dyn TextMeasurer,
    ) -> Result<IdMap<u32, NodeId>, LayoutError> {
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
            let changed = self
                .button_records
                .get(&node)
                .is_none_or(|r| !r.inputs.matches(arena, slot));
            if changed {
                self.button_records.insert(
                    node,
                    ButtonRecord {
                        inputs: arena.button_inputs(slot),
                        request: ButtonMeasureRequest {
                            face: arena.press_face(slot).expect("button"),
                            style: arena.button_face_style_unresolved(slot).expect("button"),
                            button_style: arena
                                .props(slot)
                                .str(PropId::ButtonStyle)
                                .unwrap_or("bordered")
                                .to_owned(),
                            width: AxisOffer::MaxContent,
                        },
                        answers: Vec::new(),
                        parent_width: None,
                        provisional: false,
                        supported: false,
                    },
                );
            }
            let record = self.button_records.get_mut(&node).expect("record");
            let retry = record.provisional || !record.supported;
            if changed || retry {
                record.provisional = false;
                let mut request = record.request.clone();
                request
                    .style
                    .resolve_geometry(arena.env(), record.parent_width, arena.frame(slot));
                if let Some(answer) = record.answer(request, measurer) {
                    if !answer.is_valid() {
                        return Err(LayoutError::InvalidButtonMeasure(arena.local_id(slot)));
                    }
                    record.supported = true;
                }
                self.provisional_chrome |= record.provisional;
                // Only changed inputs and pending main-thread misses invalidate
                // Taffy. An unrelated layout retains the final offers below.
                self.mark_dirty(node);
            }
            requests.insert(slot, node);
        }
        Ok(requests)
    }
}

/// The host sizes a border box; Taffy's callback sizes its content box.
/// Geometry is resolved against the containing block, never the button offer.
pub(super) fn measure(
    record: &mut ButtonRecord,
    arena: &NodeArena,
    measurer: &mut dyn TextMeasurer,
    known: Size<Option<f32>>,
    space: Size<AvailableSpace>,
    inset: taffy::Rect<f32>,
) -> Option<ButtonMeasure> {
    if !record.supported {
        return None;
    }
    let horizontal = inset.left + inset.right;
    let mut request = record.request.clone();
    request.width = known.width.map_or_else(
        || match space.width {
            AvailableSpace::Definite(w) => AxisOffer::Definite(w + horizontal),
            AvailableSpace::MinContent => AxisOffer::MinContent,
            AvailableSpace::MaxContent => AxisOffer::MaxContent,
        },
        |w| AxisOffer::Definite(w + horizontal),
    );
    let mut frame = Frame::default();
    frame.width = known.width.map_or(0.0, |w| w + horizontal);
    frame.height = known.height.map_or(0.0, |h| h + inset.top + inset.bottom);
    request
        .style
        .resolve_geometry(arena.env(), record.parent_width, frame);
    record.answer(request, measurer)
}
