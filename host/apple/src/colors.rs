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
    /// App modes (LLP 1121 D5, the agent's `prefer mode`): the window's
    /// slice set again in one commit, which paints; the batch says the
    /// window's modes after it.
    pub fn set_modes(&mut self, pins: &[(&str, &str)]) -> String {
        let batch = match self.runner.set_modes(pins) {
            Ok(Some(receipt)) => self.commit(
                &[exact_runner::Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.commit(&[], None),
            Err(e) => return self.refused(Some(format!("mode: {e:?}"))),
        };
        // An object, as every batch is: the modes go first.
        format!("{{\"modes\":{},{}", self.runner.modes_json(), &batch[1..])
    }

    /// Whether [`Host::set_modes`] would take `pins`, changing nothing: the
    /// window's modes, or the refusal.
    pub fn check_modes(&mut self, pins: &[(&str, &str)]) -> String {
        match self.runner.check_modes(pins) {
            Ok(()) => format!("{{\"modes\":{}}}", self.runner.modes_json()),
            Err(e) => exact_runner::agent::error(&format!("mode: {e:?}")),
        }
    }

    /// Whether the root font size or the window's modes moved since a batch
    /// last said them, whatever moved them: the app's `setMode` (which
    /// reaches no host as a command) and `setRootFontSize` as well as a
    /// host's. The build channel reports each move (LLP 1122 D4).
    pub(super) fn facts_moved(&mut self) -> bool {
        let facts = (self.runner.root_font_size(), self.runner.modes());
        if self.facts_told.as_ref() == Some(&facts) {
            return false;
        }
        self.facts_told = Some(facts);
        true
    }

    /// Whether a displayed element holds the wake lock, when that changed
    /// since a batch last said it (LLP 1125 D11): the presenter takes the
    /// platform's lock while any session's batch last said `true`. A new host
    /// (a boot, a committed plan) says it at its first batch.
    pub(super) fn wakelock_moved(&mut self) -> Option<bool> {
        let held = !exact_runner::wakelock::holders(self.runner.kernel()).is_empty();
        if self.wakelock_told == Some(held) {
            return None;
        }
        self.wakelock_told = Some(held);
        Some(held)
    }

    /// Take the presenter's resolutions; when anything resolves differently,
    /// re-present what the kernel resolved from the old ones.
    pub fn set_colors(&mut self, report: Vec<(ColorValue, bool, Color)>) -> String {
        let mut batch = Batch::new();
        roles::set_reported(report);
        // The reported table's and this session's roles' (LLP 1121 stage 5).
        let mut generation = roles::generation();
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
            generation = roles::generation();
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

/// The roles the plan's theme replaces (LLP 1095 D4), for the presenter's
/// own surfaces: one message at boot.
pub(crate) fn emit_roles(batch: &mut Batch) {
    use exact_kernel::style::roles;
    let entries: Vec<(&str, String)> = (0..=u8::MAX)
        .map_while(|id| roles::role_of(id).map(|r| (id, r)))
        .filter_map(|(id, r)| {
            let mut colour = String::new();
            crate::style::push_color_value(&mut colour, roles::overridden(id)?);
            Some((r.name, colour))
        })
        .collect();
    batch.roles(&entries);
}

#[cfg(test)]
#[path = "colors_tests.rs"]
mod tests;
