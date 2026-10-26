//! The generated title item of a host-rendered Neutral alert.

use super::*;

#[derive(Clone, Copy, Default)]
pub(super) enum Kind {
    #[default]
    Node,
    AlertPrefix,
}

pub(super) fn measure(
    arena: &NodeArena,
    context: &mut MeasureContext,
    measurer: &mut dyn TextMeasurer,
    known: Size<Option<f32>>,
    space: Size<AvailableSpace>,
    height_free: bool,
    invalid: &mut Option<crate::ViewId>,
) -> Option<(Size<f32>, TextMetrics)> {
    if !matches!(context.kind, Kind::AlertPrefix) {
        return None;
    }
    let label = arena
        .props(context.slot)
        .str(crate::PropId::AccessibilityLabel)
        .unwrap_or_default();
    if label.is_empty() {
        return Some((Size::ZERO, TextMetrics::default()));
    }
    let width = from_available(space.width);
    let height = from_available(space.height);
    let metrics = if let Some(cached) = context
        .measurements
        .iter()
        .find(|m| m.width == width && (height_free || m.height == height))
    {
        cached.metrics
    } else {
        let title = arena.alert_title_style(context.slot);
        let shown = title.hyphens.shown(title.text_transform.apply(label, ""));
        let run = TextRun {
            text: shown,
            style: crate::TextStyle::from_style(&title),
        };
        let request = TextMeasureRequest {
            atoms: Default::default(),
            runs: std::slice::from_ref(&run),
            paragraph: crate::text::Paragraph::from_style(&title),
            width,
            height,
            exclusions: &[],
        };
        let metrics = measurer.measure(&request);
        if !metrics.is_valid() {
            invalid.get_or_insert_with(|| arena.local_id(context.slot));
            return Some((Size::ZERO, TextMetrics::default()));
        }
        if context.measurements.len() == LEAF_OFFERS {
            context.measurements.remove(0);
        }
        context.measurements.push(Measurement {
            width,
            height,
            metrics,
            scale: 1.0,
        });
        metrics
    };
    Some((
        Size {
            width: known.width.unwrap_or(metrics.width),
            height: known.height.unwrap_or(metrics.height),
        },
        metrics,
    ))
}

/// The title's box: a line of its own in a flat alert's wrapping row, and in
/// a grouped alert's column one that keeps its size (host/web/index.html).
pub(super) fn prefix_style(grouped: bool) -> taffy::Style {
    let mut style = taffy::Style::default();
    if grouped {
        style.flex_shrink = 0.0;
    } else {
        style.flex_basis = taffy::style_helpers::percent(1.0);
    }
    style
}

impl LayoutTree {
    pub(super) fn new_alert_prefix(&mut self, slot: u32, grouped: bool) -> NodeId {
        let style = self.share(prefix_style(grouped));
        match self.taffy.new_leaf_with_context(
            style,
            MeasureContext {
                slot,
                kind: Kind::AlertPrefix,
                ..Default::default()
            },
        ) {
            Ok(node) => {
                self.alert_prefixes.insert(slot, node);
                node
            }
            Err(e) => {
                self.fault
                    .get_or_insert_with(|| format!("new_alert_prefix: {e:?}"));
                NodeId::from(usize::MAX)
            }
        }
    }

    pub(crate) fn sync_alert_prefix(&mut self, arena: &NodeArena, slot: u32, owner: NodeId) {
        let current = self.alert_prefixes.get(&slot).copied();
        let prefix = match (arena.has_alert_prefix(slot), current) {
            (true, Some(node)) => {
                self.mark_dirty(node);
                Some(node)
            }
            (true, None) => Some(self.new_alert_prefix(slot, arena.grouped_alert(slot))),
            (false, Some(node)) => {
                self.alert_prefixes.remove(&slot);
                self.remove(node);
                None
            }
            (false, None) => None,
        };
        if arena.node_type(slot).lays_out_children() {
            let children = order::laid_out(arena, slot, &self.taffy, owner, prefix, |child| {
                arena.taffy(child)
            });
            self.set_children(owner, &children);
        }
        self.mark_dirty(owner);
    }

    pub(crate) fn alert_prefix_frame(&self, arena: &NodeArena, slot: u32) -> Option<Frame> {
        let layout = self.taffy.layout(*self.alert_prefixes.get(&slot)?).ok()?;
        let owner = arena.frame(slot);
        Some(Frame {
            x: owner.x + layout.location.x,
            y: owner.y + layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        })
    }
}
