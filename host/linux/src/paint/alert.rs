//! A Neutral alert is a retained subtree above the page, not a second row layout.
use super::*;
use exact_kernel::NodeKey;
#[derive(Clone)]
pub(crate) struct Picture {
    pub key: NodeKey,
    pub offset: (f32, f32),
    pub title: Option<(Rc<Paragraph>, Rect4, crate::text::RunPaint)>,
}
impl Painter {
    pub(super) fn paint_alert(&mut self, walk: &mut Walk<'_, '_>) {
        let Some(alert) = self.alert.clone() else {
            return;
        };
        let Some(node) = walk.scene.kernel.node_by_key(alert.key) else {
            return;
        };
        self.damage.unsupported = true;
        self.backend.fill(
            &Shape::rect((0., 0., self.viewport.0, self.viewport.1)),
            [
                0,
                0,
                0,
                (255. * look::number(exact_kernel::generated::metric::SCRIM_ALPHA)).round() as u8,
            ],
            Transform::identity(),
        );
        walk.reveal = Some(node.id);
        self.node(
            walk,
            node.id,
            Transform::identity(),
            alert.offset,
            Some((0., 0., self.viewport.0, self.viewport.1)),
        );
        walk.reveal = None;
    }
    pub(super) fn alert_title(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &NodeRef<'_>,
        ts: Transform,
    ) {
        let Some(alert) = self.alert.as_ref().filter(|a| a.key == node.key).cloned() else {
            return;
        };
        if let Some((paragraph, rect, ink)) = alert.title {
            self.backend.text(
                &mut self.text.borrow_mut(),
                &paragraph,
                &[ink],
                (rect.0, rect.1),
                ts,
            );
            walk.text.insert(node.key, paragraph);
        }
    }
}
