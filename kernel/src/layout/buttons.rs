//! Native button measurement before publication. LLP 1069.011.001 D11.
use super::*;
use crate::{ButtonMeasure, ButtonMeasureRequest, PropId};

pub(super) struct ButtonRecord {
    inputs: crate::arena::button::ButtonInputs,
    request: ButtonMeasureRequest,
    // The face/style is held once. Offers retain only resolved geometry and size.
    answers: Vec<(ButtonOffer, ButtonMeasure)>,
    revision: u64,
    pub(super) parent_width: Option<f32>,
    provisional: bool,
    supported: bool,
}

#[derive(Clone, Copy, PartialEq)]
struct ButtonOffer {
    width: AxisOffer,
    geometry: [f32; 8],
}

impl ButtonOffer {
    fn of(request: &ButtonMeasureRequest) -> Self {
        let s = &request.style.button;
        Self {
            width: request.width,
            geometry: [
                s.padding_top,
                s.padding_right,
                s.padding_bottom,
                s.padding_left,
                s.border_radius_top_left,
                s.border_radius_top_right,
                s.border_radius_bottom_right,
                s.border_radius_bottom_left,
            ]
            .map(|d| match d {
                crate::Dimension::Points(n) => n,
                _ => unreachable!("button geometry is resolved before measurement"),
            }),
        }
    }
}

impl ButtonRecord {
    fn answer(
        &mut self,
        request: ButtonMeasureRequest,
        measurer: &mut dyn TextMeasurer,
    ) -> Option<ButtonMeasure> {
        let offer = ButtonOffer::of(&request);
        if let Some((_, answer)) = self.answers.iter().find(|(held, _)| *held == offer) {
            return Some(*answer);
        }
        let answer = measurer.button_measure(&request)?;
        self.provisional |= answer.provisional;
        if answer.is_valid() && !answer.provisional {
            if self.answers.len() == LEAF_OFFERS {
                self.answers.remove(0);
            }
            self.answers.push((offer, answer));
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
        let revision = measurer.measure_revision();
        for (node, slot) in nodes {
            let changed = self
                .button_records
                .get(&node)
                .is_none_or(|r| r.revision != revision || !r.inputs.matches(arena, slot));
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
                        answers: Vec::with_capacity(LEAF_OFFERS),
                        revision,
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
                request.style.resolve_geometry(
                    &arena.env_for(slot),
                    record.parent_width,
                    arena.frame(slot),
                );
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
    slot: u32,
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
        .resolve_geometry(&arena.env_for(slot), record.parent_width, frame);
    record.answer(request, measurer)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Host;
    impl TextMeasurer for Host {
        fn measure(&mut self, _: &crate::TextMeasureRequest<'_>) -> TextMetrics {
            TextMetrics::default()
        }
        fn button_measure(&mut self, r: &ButtonMeasureRequest) -> Option<ButtonMeasure> {
            Some(ButtonMeasure {
                width: match r.width {
                    AxisOffer::Definite(w) => w,
                    _ => 100.0,
                },
                height: 30.0,
                provisional: false,
            })
        }
    }
    #[test]
    fn cached_offers_keep_constant_allocation_and_compact_storage() {
        let mut kernel = crate::Kernel::with_monospace();
        kernel
            .apply(
                0,
                1,
                &[
                    crate::Op::CreateView {
                        id: 1,
                        node_type: NodeType::Control,
                    },
                    crate::Op::SetProp {
                        id: 1,
                        prop: PropId::Type,
                        value: crate::PropValue::Str("button".into()),
                    },
                ],
            )
            .unwrap();
        let arena = kernel.node(1).unwrap().arena;
        let slot = arena.slot_of(1).unwrap();
        let mut record = ButtonRecord {
            inputs: arena.button_inputs(slot),
            request: ButtonMeasureRequest {
                face: arena.press_face(slot).unwrap(),
                style: arena.button_face_style_unresolved(slot).unwrap(),
                button_style: "bordered".into(),
                width: AxisOffer::MaxContent,
            },
            answers: Vec::with_capacity(LEAF_OFFERS),
            revision: 0,
            parent_width: None,
            provisional: false,
            supported: true,
        };
        let mut host = Host;
        let mut request = record.request.clone();
        request
            .style
            .resolve_geometry(arena.env(), None, Frame::default());
        record.answer(request, &mut host);
        let capacity = record.answers.capacity();
        for width in 1..=LEAF_OFFERS * 2 {
            let mut request = record.request.clone();
            request.width = AxisOffer::Definite(width as f32);
            request
                .style
                .resolve_geometry(arena.env(), None, Frame::default());
            record.answer(request, &mut host);
        }
        assert!(
            std::mem::size_of_val(record.answers.as_slice()) <= LEAF_OFFERS * 64,
            "offers must not retain a full face/style record: {} bytes",
            std::mem::size_of_val(record.answers.as_slice())
        );
        assert_eq!(
            record.answers.capacity(),
            capacity,
            "offers must not grow the allocation"
        );
    }
}
