//! @ref LLP 1115.004 D16 — Windows list/details.
//! @ref LLP 1115.002 D4 — pane placement, list selection and the empty detail.
use super::*;
use crate::presenter::split::{Split, DIVIDER};

impl Painter {
    pub(crate) fn split_changed(&mut self, next: Option<Split>) {
        if self.split != next {
            // Selection and chevrons are host paint, so no authored row changed.
            self.colors_changed();
            self.split = next;
        }
    }

    fn split_clip(&mut self, rect: Rect4, ts: Transform, clip: Option<Rect4>) -> Option<Rect4> {
        self.backend.push_clip(&Shape::rect(rect), ts);
        let rect = bbox(ts, rect);
        Some(clip.map_or(rect, |c| intersect(c, rect)))
    }

    pub(super) fn node(
        &mut self,
        walk: &mut Walk<'_, '_>,
        id: ViewId,
        ts: Transform,
        offset: (f32, f32),
        clip: Option<Rect4>,
    ) {
        let Some(s) = self.split.as_ref() else {
            return self.node_at(walk, id, ts, offset, clip);
        };
        if s.chevrons.contains(&id) || s.placeholder == Some(id) {
            return;
        }
        if id != s.root && !s.routes.contains(&id) {
            return self.node_at(walk, id, ts, offset, clip);
        }
        if (walk.scene.hidden)(id) {
            return;
        }
        let s = s.clone();
        self.row_refuse();
        let pane = s.pane();
        let pane = (pane.0 - offset.0, pane.1 - offset.1, pane.2, pane.3);
        if id == s.root {
            let list = (
                s.rect.0 - offset.0,
                s.rect.1 - offset.1,
                s.list_width,
                s.rect.3,
            );
            let clipped = self.split_clip(list, ts, clip);
            self.node_at(walk, id, ts, offset, clipped);
            self.backend.pop_clip();
            self.backend.fill(
                &Shape::rect((pane.0 - DIVIDER, pane.1, DIVIDER, pane.3)),
                look::role("-exact-separator", self.dark),
                ts,
            );
            let clipped = self.split_clip(pane, ts, clip);
            self.backend
                .fill(&Shape::rect(pane), page_color(self.dark), ts);
            if let Some(placeholder) = s.placeholder {
                self.node_at(walk, placeholder, ts, (-pane.0, -pane.1), clipped);
            } else if s.detail.is_none() {
                self.split_empty(walk, &s, pane, ts);
            }
            self.backend.pop_clip();
        } else {
            let clipped = self.split_clip(pane, ts, clip);
            self.backend
                .fill(&Shape::rect(pane), page_color(self.dark), ts);
            self.node_at(walk, id, ts, (-pane.0, -pane.1), clipped);
            self.backend.pop_clip();
        }
    }

    fn split_empty(&mut self, walk: &Walk<'_, '_>, s: &Split, pane: Rect4, ts: Transform) {
        if cfg!(windows) {
            return;
        }
        let Some(root) = walk.scene.kernel.node(s.root) else {
            return;
        };
        let mut style = root.computed_style(StyleMask::INHERITED);
        style.font_size = walk.scene.kernel.root_font_size() * 1.25;
        style.font_weight = 400;
        let mut spec = text_spec(&style, "No Selection");
        spec.align = exact_kernel::TextAlign::Center;
        let p = self.text.borrow_mut().paragraph(&spec, Some(pane.2));
        self.backend.text(
            &mut self.text.borrow_mut(),
            &p,
            &[RunPaint {
                source: s.root,
                color: look::role("-exact-secondary-label", self.dark),
            }],
            (pane.0, pane.1 + (pane.3 - p.height) / 2.),
            ts,
        );
    }

    pub(super) fn split_row(&self, node: &NodeRef<'_>, paint: &mut BoxPaint) {
        if self.split.as_ref().and_then(|s| s.row) == Some(node.id)
            && !node
                .style
                .mask
                .union(node.style.written)
                .has(StyleId::BackgroundColor)
        {
            paint.background = look::role("-exact-tertiary-fill", self.dark);
        }
    }

    pub(super) fn split_hides_symbol(&self, node: &NodeRef<'_>) -> bool {
        self.split
            .as_ref()
            .is_some_and(|s| node.children().iter().any(|id| s.chevrons.contains(id)))
    }

    pub(super) fn split_parent<'a>(
        &self,
        kernel: &'a Kernel,
        node: &NodeRef<'_>,
    ) -> Option<(NodeRef<'a>, exact_kernel::Frame)> {
        let split = self.split.as_ref().filter(|s| s.block(node.id));
        let parent = kernel.node(split.map(|s| s.within).or(node.parent)?)?;
        let mut frame = parent.frame;
        if let Some(s) = split {
            // Perspective and its origin must use the child's block space.
            let pane = s.pane();
            frame.x -= pane.0;
            frame.y -= pane.1;
        }
        Some((parent, frame))
    }

    pub(crate) fn split_port(&self, id: ViewId, mut rect: Rect4) -> Rect4 {
        if let Some(s) = self.split.as_ref().filter(|s| s.root == id) {
            rect.2 = rect.2.min(s.list_width);
        }
        rect
    }
}
