//! The element resize event, ResizeObserver's: `resize=action` on an
//! element hears its content box after layout, at first and whenever its
//! size changes (x2apps backlog: decided, routed by value; `resize="none"`
//! stays CSS's property). The web's hosts observe with the browser's
//! `ResizeObserver`; a native host lays out, asks [`Runner::resize_due`]
//! what changed, notes and dispatches each, lays out again, and repeats as
//! the browser's loop does (Resize Observer 1 §3.4.1): each round only nodes
//! deeper than the shallowest one the last round delivered to, so a handler
//! that keeps resizing its own node or an ancestor cannot spin. What that
//! rule leaves is delivered after the next layout, and said in the log as
//! the browser says it ([`Runner::resize_settled`]).

use super::Runner;
use crate::DataSource;
use exact_kernel::{NodeKey, ViewId};
use exact_plan::{EventKind, Value};

/// ResizeObserverEntry's `contentRect`, DOM's `DOMRectReadOnly`: the content
/// box, its place the padding's left and top (its offset in the padding
/// box), in CSS px.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ResizeRect {
    /// The padding's left (`contentRect.x`).
    pub x: f64,
    /// The padding's top (`contentRect.y`).
    pub y: f64,
    /// The content box's width.
    pub width: f64,
    /// The content box's height.
    pub height: f64,
}

/// The browser's words for what the depth rule left (`ResizeObserver loop
/// completed with undelivered notifications`).
pub const UNDELIVERED: &str =
    "resize: ResizeObserver loop completed with undelivered notifications";

impl ResizeRect {
    /// The `DOMRectReadOnly` record, its fields in the compiler's order:
    /// x, y, width, height, top, right, bottom, left.
    pub fn value(self) -> Value {
        let ResizeRect {
            x,
            y,
            width,
            height,
        } = self;
        Value::record(
            [x, y, width, height, y, x + width, y + height, x]
                .map(Value::Number)
                .to_vec(),
        )
    }

    /// Host kind 39's payload: `x,y,width,height`, finite, the size not
    /// negative.
    pub fn parse(payload: &str) -> Option<ResizeRect> {
        let mut n = payload
            .split(',')
            .map(|p| exact_num::parse_f64(p.trim()).ok());
        let rect = ResizeRect {
            x: n.next()??,
            y: n.next()??,
            width: n.next()??,
            height: n.next()??,
        };
        let finite = [rect.x, rect.y, rect.width, rect.height]
            .iter()
            .all(|v| v.is_finite());
        (n.next().is_none() && finite && rect.width >= 0.0 && rect.height >= 0.0).then_some(rect)
    }
}

impl<D: DataSource> Runner<D> {
    /// The nodes with a `resize` handler whose content box, as last laid
    /// out, is not the one last delivered (none yet counts: a node is
    /// observed from its first layout), at `depth` or deeper in the tree
    /// (Resize Observer 1 §3.4.1): `(view, rect, depth)`, shallowest first.
    /// A node not laid out, or under `display: none`, has an empty box, as
    /// the browser reports one.
    pub fn resize_due(&self, depth: usize) -> Vec<(ViewId, ResizeRect, usize)> {
        let handled = |node| {
            self.plan
                .node(node)
                .handlers
                .iter()
                .any(|h| self.plan.handler(h).event == EventKind::Resize)
        };
        if !self
            .plan
            .handlers
            .iter()
            .any(|h| h.event == EventKind::Resize)
        {
            return Vec::new();
        }
        let arena = self.kernel.arena();
        let mut due: Vec<_> = self
            .ids
            .sites()
            .filter(|(_, node)| handled(*node))
            .filter_map(|(view, _)| {
                let key = arena.key_of(view)?;
                let mut d = 0;
                let mut at = key.index;
                while let Some(up) = (!arena.is_root(at)).then(|| arena.parent(at)).flatten() {
                    d += 1;
                    at = up;
                }
                let rect = self
                    .kernel
                    .laid_out_frame(key)
                    .and_then(|_| self.kernel.node(view))
                    .map_or(ResizeRect::default(), |node| {
                        // A percentage padding is of the containing block's
                        // width, its parent's content box.
                        let block = (!arena.is_root(key.index))
                            .then(|| arena.parent(key.index))
                            .flatten()
                            .and_then(|up| self.kernel.node(arena.local_id(up)))
                            .map_or(node.frame.width, |up| {
                                exact_kernel::svg::scene::content_box(&up).2
                            });
                        content_rect(&node, block)
                    });
                let last = self
                    .resized
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, r)| *r);
                (d >= depth && last != Some(rect)).then_some((view, rect, d))
            })
            .collect();
        due.sort_by_key(|(_, _, d)| *d);
        due
    }

    /// Note `rect` as delivered to `view`, before the host dispatches
    /// `Event::Resize(rect)` to it: the next [`Runner::resize_due`] leaves
    /// it out until its box changes again.
    pub fn resize_delivered(&mut self, view: ViewId, rect: ResizeRect) {
        let arena = self.kernel.arena();
        self.resized
            .retain(|(key, _)| arena.resolve(*key).is_some());
        let Some(key) = arena.key_of(view) else {
            return;
        };
        match self.resized.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = rect,
            None => self.resized.push((key, rect)),
        }
    }

    /// After a host's last round: whether the depth rule left a node
    /// undelivered, which the log says as the browser does; it is
    /// delivered after the next layout.
    pub fn resize_settled(&mut self) {
        if !self.resize_due(0).is_empty() {
            self.log(UNDELIVERED);
        }
    }
}

/// What the runner keeps: each observed node's last delivered rect.
pub type Resized = Vec<(NodeKey, ResizeRect)>;

/// ResizeObserver's `contentRect` of `node` (b6 review B2): `x` and `y` the
/// used padding's left and top — the border is outside the padding box the
/// rect is placed in — and the size the border box less padding and border.
fn content_rect(node: &exact_kernel::NodeRef<'_>, block: f32) -> ResizeRect {
    use exact_kernel::{BorderStyle, Dimension};
    let s = node.style;
    let pad = |d: Dimension| match d {
        Dimension::Points(p) => p,
        Dimension::Percent(p) => block * p / 100.0,
        _ => 0.0,
    };
    let border = |w: f32, style: BorderStyle| match style {
        BorderStyle::None | BorderStyle::Hidden => 0.0,
        _ => w,
    };
    let (left, top) = (pad(s.padding_left), pad(s.padding_top));
    let width = node.frame.width
        - left
        - pad(s.padding_right)
        - border(s.border_width_left, s.border_style_left)
        - border(s.border_width_right, s.border_style_right);
    let height = node.frame.height
        - top
        - pad(s.padding_bottom)
        - border(s.border_width_top, s.border_style_top)
        - border(s.border_width_bottom, s.border_style_bottom);
    ResizeRect {
        x: left.into(),
        y: top.into(),
        width: width.max(0.0).into(),
        height: height.max(0.0).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::ResizeRect;

    #[test]
    fn a_payload_is_four_finite_numbers_and_a_size_not_negative() {
        assert_eq!(
            ResizeRect::parse("4,2,100,50.5"),
            Some(ResizeRect {
                x: 4.0,
                y: 2.0,
                width: 100.0,
                height: 50.5
            })
        );
        for refused in ["", "1,2,3", "1,2,3,4,5", "0,0,-1,4", "0,0,NaN,1", "a,0,1,1"] {
            assert_eq!(ResizeRect::parse(refused), None, "{refused}");
        }
    }
}
