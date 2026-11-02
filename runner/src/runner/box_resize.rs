//! A box the person resizes by its CSS `resize` grip (CSS UI 4 §5.1). The
//! browser draws the grip and, as the pointer drags it, writes the box's
//! `width` and `height` as inline px (measured in Chrome: a content-box
//! textarea dragged to a 120 px border box gets `height: 110px` beside its
//! 4 px padding and 1 px border). A native host draws the grip and hands the
//! border box the pointer reached to [`Runner::resize_box`], which writes
//! the same rows. Like the browser's inline style, the rows replace the
//! authored size (and a `field-sizing: content` height) until the plan
//! rewrites them; `min-*` and `max-*` still bound the box, and nothing else
//! does: Chrome lets the person shrink it below its first size.

use super::{DataSource, Runner, RunnerError};
use crate::instance::InstanceError;
use exact_kernel::{BoxSizing, CommitReceipt, Op, RowValue, StyleId, StyleProps, ViewId};
use exact_plan::Value;

/// The axes a computed `resize` value lets the person drag: (width, height).
/// `block` and `inline` are the horizontal writing mode's, the only one the
/// kernel lays out.
fn resize_axes(value: &str) -> (bool, bool) {
    match value {
        "both" => (true, true),
        "horizontal" | "inline" => (true, false),
        "vertical" | "block" => (false, true),
        _ => (false, false),
    }
}

impl<D: DataSource> Runner<D> {
    /// The person dragged `view`'s resize grip until its border box was
    /// `width` by `height` px; `None` leaves an axis as it is. Each axis its
    /// computed `resize` allows is written as the browser writes it: in the
    /// box's `box-sizing`, so a content box's padding and border (a native
    /// field's chrome among them) come off, never below zero. `None` when
    /// `view` is gone, its `resize` allows neither axis given, a content box
    /// has not been laid out, or the rows already hold the size.
    pub fn resize_box(
        &mut self,
        view: ViewId,
        width: Option<f32>,
        height: Option<f32>,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(node) = self.kernel.node(view) else {
            return Ok(None);
        };
        let RowValue::Enum(resize) = node.computed(StyleId::Resize) else {
            return Ok(None);
        };
        let (wide, tall) = resize_axes(resize);
        let content = node.style.box_sizing == BoxSizing::ContentBox;
        let key = self.kernel.arena().key_of(view);
        let insets = key.and_then(|key| {
            let padding = self.kernel.resolved_padding(key)?;
            let border = self.kernel.resolved_border(key)?;
            Some((padding, border))
        });
        let mut rows = Vec::new();
        for (allowed, target, id) in [
            (wide, width, StyleId::Width),
            (tall, height, StyleId::Height),
        ] {
            let Some(target) = target.filter(|t| allowed && t.is_finite()) else {
                continue;
            };
            // (left, top, right, bottom) each. A content box not yet laid
            // out has no insets to take off: the drag waits for a layout.
            let inset = match (content, insets) {
                (false, _) => 0.0,
                (true, None) => return Ok(None),
                (true, Some(((pl, pt, pr, pb), (bl, bt, br, bb)))) => {
                    if id == StyleId::Width {
                        pl + pr + bl + br
                    } else {
                        pt + pb + bt + bb
                    }
                }
            };
            rows.push((id, Value::Number(f64::from((target - inset).max(0.0)))));
        }
        if rows.is_empty() {
            return Ok(None);
        }
        let mut patch = StyleProps::default();
        for (id, value) in &rows {
            crate::bridge::set_plan_style(&mut patch, *id as u16, value, &self.plan)
                .map_err(|e| RunnerError::from(InstanceError::Bridge(e)))?;
        }
        let mut after = node.style.clone();
        after.apply_patch(&patch);
        if after == *node.style {
            return Ok(None);
        }
        let op = Op::SetStyle {
            id: view,
            patch: Box::new(patch),
        };
        self.apply(vec![op]).map(Some)
    }
}
