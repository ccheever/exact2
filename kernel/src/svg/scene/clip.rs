//! `clip-path: url(#id)` on an SVG element: the `clipPath`'s children as
//! one union of shapes in the element's own user space, intersected with the
//! `clipPath`'s own `clip-path`.
//!
//! @ref LLP 1055.000 D10; CSS Masking 1 §6 (`clipPath`: `clipPathUnits`,
//! whose initial is `userSpaceOnUse`; children contribute their geometry,
//! each with its own `clip-rule`; a missing reference does not clip)

use super::{cascade, Resolver};
use crate::generated::{ClipRule, Display, NodeType, PropId, StyleMask, Visibility};
use crate::kernel::NodeRef;
use crate::svg::length::Viewport;
use crate::svg::transform::{self as tf, Affine};
use crate::svg::Path;

/// A resolved clip: the union of `shapes`, intersected with `then`. No
/// shapes clips everything away.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// The shapes whose union is the clip region, in the clipped element's
    /// user space.
    pub shapes: Vec<ClipShape>,
    /// The `clipPath`'s own `clip-path`, intersected.
    pub then: Option<Box<Clip>>,
}

/// One clip child's geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipShape {
    /// The path, in the clipped element's user space.
    pub path: Path,
    /// `clip-rule: evenodd`.
    pub even_odd: bool,
}

impl Clip {
    /// The same clip mapped by `m` (a host that draws an element about
    /// another origin moves its clip with it).
    pub fn transformed(&self, m: Affine) -> Clip {
        Clip {
            shapes: self
                .shapes
                .iter()
                .map(|s| ClipShape {
                    path: s.path.transformed(m),
                    even_odd: s.even_odd,
                })
                .collect(),
            then: self.then.as_ref().map(|t| Box::new(t.transformed(m))),
        }
    }
}

impl Resolver<'_, '_> {
    /// The clip `node`'s `clip-path` names, whose object bounding box is
    /// `bbox`; `None` when it names nothing that clips (a missing or
    /// wrong-type target, or a cycle): the element is not clipped.
    pub(super) fn clip(
        &mut self,
        node: &NodeRef<'_>,
        bbox: Option<(f32, f32, f32, f32)>,
        vp: Viewport,
        depth: usize,
    ) -> Option<Clip> {
        let id = node.style.rare.clip_path.url()?;
        let target = self.kernel.resolve_id(node.id, id)?;
        let clip = self.kernel.node(target)?;
        if clip.node_type != NodeType::SvgClipPath || depth > 8 {
            return None;
        }
        let units = if clip.props.str(PropId::ClipPathUnits) == Some("objectBoundingBox") {
            match bbox {
                Some((x, y, w, h)) if w > 0.0 && h > 0.0 => [w, 0.0, 0.0, h, x, y],
                // No box to clip against: nothing shows.
                _ => {
                    return Some(Clip {
                        shapes: Vec::new(),
                        then: None,
                    })
                }
            }
        } else {
            tf::IDENTITY
        };
        // The clipPath's own transform, then its units.
        let style = clip.computed_style(StyleMask::INHERITED);
        let own = self
            .transform(&clip, &clip.style.clone(), vp)
            .map_or(tf::IDENTITY, |t| t.affine());
        let base = tf::mul(own, units);
        let mut shapes = Vec::new();
        for child in clip.children() {
            let Some(c) = self.kernel.node(child) else {
                continue;
            };
            let cs = cascade(&c, &style);
            if cs.display == Display::None || cs.visibility != Visibility::Visible {
                continue;
            }
            // A `use` contributes its target's shape, placed.
            let (shape, at) = if c.node_type == NodeType::SvgUse {
                let Some(t) = c
                    .props
                    .str(PropId::Href)
                    .and_then(|h| h.strip_prefix('#'))
                    .and_then(|h| self.kernel.resolve_id(c.id, h))
                    .and_then(|t| self.kernel.node(t))
                    .filter(|t| t.node_type.is_svg_shape())
                else {
                    continue;
                };
                (t, tf::translate(vp.x(cs.x), vp.y(cs.y)))
            } else if c.node_type.is_svg_shape() {
                (c, tf::IDENTITY)
            } else {
                continue;
            };
            let ss = cascade(&shape, &cs);
            let Some((path, _)) = self.geometry(&shape, &ss, vp) else {
                continue;
            };
            let m = self
                .transform(&c, &cs, vp)
                .map_or(tf::IDENTITY, |t| t.affine());
            shapes.push(ClipShape {
                path: path.transformed(tf::mul(base, tf::mul(m, at))),
                even_odd: ss.clip_rule == ClipRule::Evenodd,
            });
        }
        let then = self.clip(&clip, bbox, vp, depth + 1).map(Box::new);
        Some(Clip { shapes, then })
    }
}
