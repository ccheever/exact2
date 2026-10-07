//! Platform colours the kernel resolves itself (LLP 1095 D1, D6).
//!
//! A row naming a role or a `-exact-platform-color()` crosses as its name, and the
//! presenter resolves it per view (`SystemColor.swift`). What the kernel
//! resolves before it crosses — paint motion's endpoints, gradient stops,
//! an SVG scene's paint and filter colours — the presenter resolves too and
//! reports: every reference, light and dark, under the traits it shows
//! (Increased Contrast is process-wide), again on every trait change. A new
//! report re-targets paint motion (it transitions, as an appearance change
//! does), rebuilds SVG scenes and re-sends styles with a reference in a
//! gradient or a box filter's shadow, so nothing the platform adjusts is a
//! frozen fallback. The table is the process's; each session re-presents
//! when it moved since that session last looked, so every embedded session
//! refreshes, not only the first to report.

use super::Host;
use crate::batch::Batch;
use crate::style;
use exact_kernel::gradient::Gradient;
use exact_kernel::motion::motion_node;
use exact_kernel::style::{roles, Color, ColorValue, RowValue};
use exact_kernel::svg::filter::{FilterFn, FilterList};
use exact_kernel::{StyleId, ViewId};
use exact_runner::DataSource;

fn is_reference(c: ColorValue) -> bool {
    matches!(c, ColorValue::Role(_) | ColorValue::Platform(_))
}

fn refers(g: &Gradient) -> bool {
    g.stops.iter().any(|s| is_reference(s.color))
}

/// Whether a box filter's shadow resolves a reference: its own colour, or
/// `currentcolor` when the node's colour is one.
fn filter_refers(list: &FilterList, text: ColorValue) -> bool {
    list.0.iter().any(|f| match f {
        FilterFn::DropShadow(.., c) => is_reference(c.unwrap_or(text)),
        _ => false,
    })
}

impl<D: DataSource> Host<D> {
    /// Take the presenter's resolutions; when anything resolves differently,
    /// re-present what the kernel resolved from the old ones.
    pub fn set_colors(&mut self, report: Vec<(ColorValue, bool, Color)>) -> String {
        let mut batch = Batch::new();
        let mut generation = roles::set_reported(report);
        // What is recorded is the generation this session painted from: if
        // another session replaced the table while it painted, it paints
        // again (a few times at most; its next report catches up after).
        // The session's first report corrects what boot resolved, without
        // motion (LLP 1095 D6), whatever the table held then.
        for _ in 0..3 {
            if Some(generation) == self.colors_seen {
                break;
            }
            let first = self.colors_seen.is_none();
            self.re_present_colors(&mut batch, first);
            self.colors_seen = Some(generation);
            generation = roles::reported_generation();
        }
        self.finish(batch, None)
    }

    /// Re-present what the kernel resolved from the reported table.
    fn re_present_colors(&mut self, batch: &mut Batch, first: bool) {
        self.repaint_colors(batch, first);
        self.svg.all_dirty();
        let kernel = self.runner.kernel();
        let env = kernel.env();
        let views: Vec<ViewId> = self.keys.values().copied().collect();
        for view in views {
            let Some(node) = kernel.node(view) else {
                continue;
            };
            let resolved = [
                StyleId::BackgroundImage,
                StyleId::MaskImage,
                StyleId::Filter,
            ]
            .into_iter()
            .any(|id| match node.computed(id) {
                RowValue::BackgroundImage(b) => b.layers().iter().any(refers),
                RowValue::MaskImage(m) => m.gradient().is_some_and(refers),
                RowValue::Filter(f) => filter_refers(f, node.text_color()),
                _ => false,
            });
            if resolved {
                let shown = self.shown_paint(motion_node(node.key));
                batch.style(view, &style::style_json_presented(&node, &env, &shown).0);
            }
        }
        self.present(batch, false);
    }
}

#[cfg(test)]
#[path = "colors_tests.rs"]
mod tests;
