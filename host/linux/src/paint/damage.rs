//! @ref LLP 1043.000 §3 D4/D7 — flow damage has no frame-history index.
//! One viewport surface and a deduplicated set of changed live nodes. Structural,
//! non-exclusion, transformed, scrolled and region updates use the full painter.
use super::*;
use exact_kernel::{CommitReceipt, LayoutReceipt, NodeKey, PositionType, WrapFlow};
use std::collections::BTreeSet;
#[derive(Default)]
pub(crate) struct Changes {
    keys: BTreeSet<NodeKey>,
    flow: bool,
    full: bool,
}
impl Changes {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn repaint(&mut self) {
        self.full = true;
        self.keys.clear();
    }
    pub fn commit(&mut self, kernel: &Kernel, r: &CommitReceipt) {
        self.full |= !r.created.is_empty() || !r.destroyed.is_empty();
        if self.full {
            self.keys.clear();
            return;
        }
        for key in &r.touched {
            self.full |= !kernel.node_by_key(*key).is_some_and(|n| {
                n.style.position_type == PositionType::Absolute
                    && n.style.wrap_flow == WrapFlow::Both
            });
            self.keys.insert(*key);
        }
        if self.full {
            self.keys.clear();
        }
    }
    pub fn layout(&mut self, r: &LayoutReceipt) {
        if self.full {
            return;
        }
        self.flow |= !r.flow_changed.is_empty();
        self.keys.extend(r.changed.iter().copied());
        self.keys.extend(r.flow_changed.iter().copied());
    }
}
#[derive(Default)]
pub(super) struct Retained {
    pub pixels: Option<Arc<Pixmap>>,
    pub next: Vec<Rect4>,
    pub last: Vec<Rect4>,
    pub dark: bool,
    pub unsupported: bool,
    pub caret: bool,
}
impl Painter {
    /// Current accepted paragraph, shared with pixels, hit metadata and the agent.
    pub fn paragraph(&self, key: NodeKey) -> Option<&Rc<Paragraph>> {
        self.accepted_text.get(&key)
    }
    /// Regions actually repainted on the last CPU flow frame; empty means full.
    pub fn damage_rects(&self) -> &[Rect4] {
        &self.damage.last
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn flow_damage<D: exact_runner::DataSource>(
        &mut self,
        host: &crate::Host<D>,
        boxes: &[PaintedBox],
        viewport: (f32, f32),
        page: (f32, f32),
        scroll: &BTreeMap<ViewId, (f32, f32)>,
        pointer: Option<(f32, f32)>,
        focus: Option<ViewId>,
    ) {
        self.damage.next.clear();
        let changes = &host.flow_damage;
        let Some(pixels) = &self.damage.pixels else {
            return;
        };
        if changes.full
            || self.damage.unsupported
            || !changes.flow
            || self.damage.dark != self.dark
            || page != (0., 0.)
            || !scroll.is_empty()
            || pointer.is_some()
            // Button focus has no ink; only an input caret invalidates this
            // optimization. Also retire a caret painted in the previous frame.
            || self.damage.caret
            || focus.is_some_and(|id| host.kernel().node(id).is_some_and(|n| n.node_type == NodeType::TextInput))
            || host.content_region().is_some()
            || pixels.width() != (viewport.0 * self.scale).round() as u32
            || pixels.height() != (viewport.1 * self.scale).round() as u32
        {
            return;
        }
        let kernel = host.kernel();
        for key in &changes.keys {
            let Some(n) = kernel.node_by_key(*key) else {
                self.damage.next.clear();
                return;
            };
            let p = host.presented(n.id);
            if p.moves() || p.opacity != 1.0 || n.node_type == NodeType::Image {
                self.damage.next.clear();
                return;
            }
            // Unclipped visible text may acquire new overflow beyond either old
            // or definite geometry. Use full repaint unless a page clips it.
            if n.node_type == NodeType::Text && !n.flow_shapes().is_empty() {
                let clipped = n
                    .parent
                    .and_then(|id| kernel.node(id))
                    .is_some_and(|parent| {
                        let (x, y) = effective_overflow(&parent);
                        x != Overflow::Visible && y != Overflow::Visible
                    });
                if !clipped {
                    self.damage.next.clear();
                    return;
                }
            }
            let f = n.frame;
            let mut rects = vec![(f.x, f.y, f.width, f.height)];
            if let Some(old) = boxes.iter().find(|b| b.id == n.id) {
                rects.push(old.rect);
            }
            // Font overhang and overflowing ordinary text are inside the damage.
            if let Some(p) = self.accepted_text.get(key) {
                rects.push((f.x, f.y, f.width.max(p.width), f.height.max(p.height)));
            }
            for (x, y, w, h) in rects {
                let pad = if n.node_type == NodeType::Text {
                    n.computed_style(StyleMask::INHERITED).font_size
                } else {
                    2.0
                };
                let x0 = (x - pad).floor().max(0.0);
                let y0 = (y - pad).floor().max(0.0);
                let x1 = (x + w + pad).ceil().min(viewport.0);
                let y1 = (y + h + pad).ceil().min(viewport.1);
                if x1 > x0 && y1 > y0 {
                    self.damage.next.push((x0, y0, x1 - x0, y1 - y0));
                }
            }
        }
    }
}
