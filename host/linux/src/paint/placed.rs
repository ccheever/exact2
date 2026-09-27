//! A canvas child placed by its surface (LLP 1014 D5): painted apart and
//! warped into the frame.
use super::*;

impl Painter {
    pub(super) fn placed(
        &mut self,
        walk: &mut Walk<'_, '_>,
        id: ViewId,
        ts: Transform,
        offset: (f32, f32),
        clip: Option<Rect4>,
    ) -> bool {
        use crate::placement::{self, Placement};
        let Some(p) = self.placements.get(&id).copied() else {
            return false;
        };
        let Placement::Visible {
            h,
            canvas,
            clip_depth,
            ..
        } = p
        else {
            return true;
        };
        let Some(node) = walk.scene.kernel.node(id) else {
            return true;
        };
        let Some(parent) = walk.scene.kernel.node(canvas) else {
            return true;
        };
        let h = placement::compose(h, ts, parent.frame.x - offset.0, parent.frame.y - offset.1);
        let Some(inv) = placement::inverse(h) else {
            return true;
        };
        let f = node.frame;
        if f.width <= 0. || f.height <= 0. {
            return true;
        }
        let mut painter = Painter::new(
            self.text.clone(),
            self.scale,
            Box::new(crate::raster::Raster::transparent()),
        );
        painter.dark = self.dark;
        painter.placements = self.placements.clone();
        painter.placements.remove(&id);
        painter.viewport = (f.width, f.height);
        painter.backend.begin(f.width, f.height, self.scale);
        let mut child_walk = Walk {
            scene: walk.scene,
            boxes: Vec::new(),
            text: BTreeMap::new(),
            skip: None,
            replay: None,
        };
        painter.node(&mut child_walk, id, Transform::identity(), (f.x, f.y), None);
        walk.text.extend(child_walk.text);
        if let Ok(source) = painter.backend.finish() {
            if let Some((pixels, rect)) =
                placement::warp_clipped(&source, h, clip_depth, self.scale, self.viewport)
            {
                self.backend.surface_image(Arc::new(pixels), rect);
            }
        }
        for mut b in child_walk.boxes {
            b.projective = Some((inv, b.rect, b.clip, clip_depth));
            b.rect = placement::clipped_bounds(&h, clip_depth, b.rect);
            b.clip = clip;
            walk.boxes.push(b);
        }
        true
    }
}
