//! `-exact-fit: shrink` (LLP 1116.001 D1.4): a text whose longest unbreakable
//! run (its whole line under `white-space: nowrap`) is wider than its content
//! box is measured at the scale that fits, down to a floor, as UIKit's
//! `adjustsFontSizeToFitWidth` with a `minimumScaleFactor` draws it. The
//! kernel picks the scale with the host's own measurer, so every host lays it
//! out alike, and keeps it on the arena (`NodeArena::fit_scale`): the runs a
//! painter reads are at it, and a host that draws from style rows reads it.
use super::*;

/// The least scale a text is drawn at (UIKit's 0.5 `minimumScaleFactor`).
const HALF: f32 = 0.5;

/// A fitted text's floor: half its size, or, for display text bigger than
/// twice the reader's root size, that root size. Legible either way, and
/// Dynamic Type's growth stays absorbable: a stopwatch's `4.5rem` readout
/// is 238 points at AX5, and half of it is still wider than a phone.
pub(super) fn floor(arena: &NodeArena, slot: u32) -> f32 {
    let size = arena.text_style(slot).font_size;
    let root = arena.root_font_size();
    if size > 0.0 && root > 0.0 {
        (root / size).min(HALF)
    } else {
        HALF
    }
}
/// How much wider than its box a line may come out and still fit.
const SLACK: f32 = 0.01;

/// Whether `slot` is a fitted paragraph.
pub(super) fn fits(arena: &NodeArena, slot: u32) -> bool {
    arena.node_type(slot) == NodeType::Text
        && arena.style(slot).rare.fit == crate::TextFit::Shrink
        && !arena.is_inline_run(slot)
}

/// The scale a fitted text takes under a width offer, from its unscaled
/// min-content width: what fits a definite width, down to the floor; the
/// floor for its min-content contribution (it can shrink that far); its own
/// size for its max-content.
pub(super) fn scale(natural: f32, width: AxisOffer, floor: f32) -> f32 {
    match width {
        AxisOffer::Definite(w) if natural > w + SLACK => (w.max(0.0) / natural).max(floor),
        AxisOffer::MinContent if natural > 0.0 => floor,
        _ => 1.0,
    }
}

/// A fitted paragraph's metrics under an offer, and the scale they are at:
/// its unscaled min-content width first (once, kept in `natural`), then its
/// runs at the scale that fits, measured again once when the shaped line
/// comes out wider than the scale promised. Never by paragraph stamp: a
/// stamp names the paragraph's runs at one size. Exclusions and columns are
/// not fitted around. `None` for a paragraph with no runs.
pub(super) fn measure<'a>(
    arena: &'a NodeArena,
    slot: u32,
    measurer: &mut dyn TextMeasurer,
    runs: &mut Vec<TextRun<'a>>,
    natural: &mut Option<f32>,
    width: AxisOffer,
    height: AxisOffer,
) -> Option<(TextMetrics, f32)> {
    let mut at = |scale: f32, width: AxisOffer| {
        runs.clear();
        arena.text_runs_at(slot, scale, runs);
        (!runs.is_empty()).then(|| {
            measurer.measure(&TextMeasureRequest {
                atoms: arena.markup_atoms(slot),
                exclusions: &[],
                runs,
                paragraph: arena.paragraph_at(slot, scale),
                width,
                height,
            })
        })
    };
    let base = match *natural {
        Some(n) => n,
        None => {
            let n = at(1.0, AxisOffer::MinContent)?.width;
            *natural = Some(n);
            n
        }
    };
    let least = floor(arena, slot);
    let mut s = scale(base, width, least);
    let mut metrics = at(s, width)?;
    if let AxisOffer::Definite(w) = width {
        if s > least && metrics.width > w + SLACK && metrics.width > 0.0 {
            s = (s * w / metrics.width).max(least);
            metrics = at(s, width)?;
        }
    }
    Some((metrics, s))
}

/// After a layout: each fitted paragraph's scale at its final content width,
/// kept on the arena. A changed scale is new paragraph input (its metric
/// revision moves, so a painter's paragraph is rebuilt) and a publication
/// candidate even where its box held (`Kernel::fit_scale`). A paragraph no
/// longer fitted goes back to its size.
pub(super) fn settle(arena: &mut NodeArena, tree: &mut LayoutTree) -> Vec<NodeKey> {
    let mut moved = Vec::new();
    let slots: Vec<u32> = tree.fitting.iter().copied().collect();
    for slot in slots {
        let node = arena
            .taffy(slot)
            .filter(|_| arena.is_live(slot) && fits(arena, slot));
        let scale = match node {
            None => {
                tree.fitting.remove(&slot);
                1.0
            }
            Some(node) => {
                let l = tree.layout(node);
                let width = l.size.width
                    - l.padding.left
                    - l.padding.right
                    - l.border.left
                    - l.border.right;
                // Hidden, or not laid out yet: the last scale stands.
                if width <= 0.0 {
                    continue;
                }
                let Some(context) = tree.taffy.get_node_context(node) else {
                    continue;
                };
                context
                    .measurements
                    .iter()
                    .find(
                        |m| matches!(m.width, AxisOffer::Definite(w) if (w - width).abs() < SLACK),
                    )
                    .map(|m| m.scale)
                    .or_else(|| {
                        let least = floor(arena, slot);
                        context
                            .fit_natural
                            .map(|n| scale(n, AxisOffer::Definite(width), least))
                    })
                    .unwrap_or(1.0)
            }
        };
        if arena.fit_scale(slot).to_bits() == scale.to_bits() || !arena.is_live(slot) {
            if !arena.is_live(slot) {
                arena.fit_scales.remove(&slot);
            }
            continue;
        }
        if scale < 1.0 {
            arena.fit_scales.insert(slot, scale);
        } else {
            arena.fit_scales.remove(&slot);
        }
        arena.revise_text(slot, true);
        moved.push(arena.key(slot));
    }
    moved
}
