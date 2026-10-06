//! Masks and patterns (LLP 1055.000 D7, D10): the two references whose
//! content native hosts render to pixels, an island, rather than to vector
//! layers.
//!
//! @ref LLP 1055.000 D10 (`mask`: `maskUnits` `objectBoundingBox` with the
//! region −10%/−10%/120%/120%, `maskContentUnits` `userSpaceOnUse`,
//! `mask-type` luminance or alpha), D7 (`pattern`: `patternUnits`
//! `objectBoundingBox`, `patternContentUnits` `userSpaceOnUse`, `viewBox`,
//! `patternTransform`, `href` templates); CSS Masking 1 §7, SVG 2 §14.3
//!
//! The kernel resolves both into items, as it does a `use`: the content
//! inherits from the referenced element's own ancestors, and is placed in
//! the referencing element's user space (a mask) or in pattern space (a
//! pattern's tile). How the items become pixels is each host's.

use super::{cascade, Item, Kind, Resolver, Transform, Viewport};
use crate::generated::{MaskType, NodeType, PropId, StyleId, StyleMask};
use crate::kernel::NodeRef;
use crate::style::Dimension;
use crate::svg::transform::{self as tf, Affine};
use crate::svg::{view_box, view_box_transform, TransformList};

/// A resolved `mask`: its content is rendered, turned into coverage by
/// luminance or alpha, and limited to the region.
#[derive(Debug, Clone, PartialEq)]
pub struct Mask {
    /// The mask region, x, y, width, height, in the masked element's user
    /// space. Nothing of the element shows outside it.
    pub region: (f32, f32, f32, f32),
    /// `mask-type: luminance` (else `alpha`).
    pub luminance: bool,
    /// The content, in the masked element's user space.
    pub items: Vec<Item>,
}

/// A resolved `pattern` paint: the tile, repeated over pattern space.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    /// The tile, x, y, width, height, in pattern space; never empty.
    pub tile: (f32, f32, f32, f32),
    /// Pattern space to the painted shape's user space
    /// (`patternTransform`).
    pub transform: Affine,
    /// The tile's content, in pattern space (the tile's origin, the view
    /// box or `patternContentUnits` applied). Hosts clip it to the tile.
    pub items: Vec<Item>,
}

/// A region coordinate: a fraction of the box under `objectBoundingBox`
/// (a number or a percentage), else a length against the viewport axis.
fn coord(d: Option<Dimension>, default: f32, obb: bool, basis: f32) -> f32 {
    match d {
        Some(Dimension::Points(v)) => v,
        Some(Dimension::Percent(p)) if obb => p / 100.0,
        Some(Dimension::Percent(p)) => p / 100.0 * basis,
        _ if obb => default,
        _ => default * basis,
    }
}

/// `(x, y, w, h)` mapped by the box under `objectBoundingBox`.
fn in_box(r: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    (b.0 + r.0 * b.2, b.1 + r.1 * b.3, r.2 * b.2, r.3 * b.3)
}

impl Resolver<'_, '_> {
    /// The mask `node`'s `mask` names, for an element whose object bounding
    /// box is `bbox` and whose user space maps to the content box by `ctm`.
    /// `None` when it names no `mask` (the element is not masked); a mask
    /// with no area masks everything away.
    pub(super) fn mask(
        &mut self,
        node: &NodeRef<'_>,
        bbox: Option<(f32, f32, f32, f32)>,
        vp: Viewport,
        ctm: Affine,
    ) -> Option<Mask> {
        let id = node.style.rare.svg_mask.url()?;
        let target = self.kernel.resolve_id(node.id, id)?;
        let mask = self.kernel.node(target)?;
        if mask.node_type != NodeType::SvgMask {
            return None;
        }
        let empty = Mask {
            region: (0.0, 0.0, 0.0, 0.0),
            luminance: true,
            items: Vec::new(),
        };
        // A mask inside its own content masks nothing more.
        if self.uses.contains(&(mask.id as u64)) || self.uses.len() > 16 {
            return Some(empty);
        }
        let obb = mask.props.str(PropId::MaskUnits) != Some("userSpaceOnUse");
        let content_obb = mask.props.str(PropId::MaskContentUnits) == Some("objectBoundingBox");
        let rows = |s: StyleId| mask.style.mask.has(s);
        let at = |s: StyleId, d: Dimension| rows(s).then_some(d);
        let raw = (
            coord(at(StyleId::X, mask.style.x), -0.1, obb, vp.width),
            coord(at(StyleId::Y, mask.style.y), -0.1, obb, vp.height),
            coord(at(StyleId::Width, mask.style.width), 1.2, obb, vp.width),
            coord(at(StyleId::Height, mask.style.height), 1.2, obb, vp.height),
        );
        let bx = bbox.filter(|b| b.2 > 0.0 && b.3 > 0.0);
        let region = match (obb, bx) {
            (true, Some(b)) => in_box(raw, b),
            (false, _) => raw,
            // No box to measure the region by: nothing shows.
            (true, None) => return Some(empty),
        };
        if !(region.2 > 0.0 && region.3 > 0.0) {
            return Some(empty);
        }
        let units = match (content_obb, bx) {
            (false, _) => tf::IDENTITY,
            (true, Some(b)) => [b.2, 0.0, 0.0, b.3, b.0, b.1],
            (true, None) => return Some(empty),
        };
        let inherited = mask.computed_style(StyleMask::INHERITED);
        let mut mstyle = mask.style.clone();
        mstyle.copy_rows(&inherited, StyleMask::INHERITED.minus(mask.style.mask));
        // Two elements naming one mask are two instances: salt the keys.
        self.uses.push((1 << 44) | node.id as u64);
        self.uses.push(mask.id as u64);
        let children = self.children(&mask, &mstyle, vp, tf::mul(ctm, units));
        let uid = self.uid(mask.id);
        self.uses.pop();
        self.uses.pop();
        let items = vec![wrapper(&mask, uid, units, tf::mul(ctm, units), children)];
        Some(Mask {
            region,
            luminance: mstyle.mask_type == MaskType::Luminance,
            items,
        })
    }

    /// The pattern `url(#id)` names for a shape `from` whose object bounding
    /// box is `bbox`: `Ok(None)` when the target is not a `pattern` (the
    /// paint falls back), `Err(())` when it is one that paints nothing (a
    /// tile with no area, or a cycle).
    pub(super) fn pattern(
        &mut self,
        from: &NodeRef<'_>,
        id: &str,
        bbox: Option<(f32, f32, f32, f32)>,
        vp: Viewport,
        ctm: Affine,
    ) -> Result<Option<Pattern>, ()> {
        let Some(target) = self
            .kernel
            .resolve_id(from.id, id)
            .and_then(|t| self.kernel.node(t))
        else {
            return Ok(None);
        };
        if target.node_type != NodeType::SvgPattern {
            return Ok(None);
        }
        // The `href` chain, this pattern first; a cycle ends it.
        let mut chain: Vec<NodeRef<'_>> = vec![target];
        loop {
            let last = chain.last().expect("the target");
            let next = last
                .props
                .str(PropId::Href)
                .and_then(|h| h.strip_prefix('#'))
                .and_then(|h| self.kernel.resolve_id(last.id, h))
                .and_then(|t| self.kernel.node(t))
                .filter(|t| t.node_type == NodeType::SvgPattern);
            match next {
                Some(n) if !chain.iter().any(|c| c.id == n.id) && chain.len() < 16 => chain.push(n),
                _ => break,
            }
        }
        let prop = |p: PropId| chain.iter().find_map(|n| n.props.str(p));
        let row = |s: StyleId| chain.iter().find(|n| n.style.mask.has(s));
        let obb = prop(PropId::PatternUnits) != Some("userSpaceOnUse");
        let content_obb = prop(PropId::PatternContentUnits) == Some("objectBoundingBox");
        let raw = (
            coord(row(StyleId::X).map(|n| n.style.x), 0.0, obb, vp.width),
            coord(row(StyleId::Y).map(|n| n.style.y), 0.0, obb, vp.height),
            coord(
                row(StyleId::Width).map(|n| n.style.width),
                0.0,
                obb,
                vp.width,
            ),
            coord(
                row(StyleId::Height).map(|n| n.style.height),
                0.0,
                obb,
                vp.height,
            ),
        );
        let bx = bbox.filter(|b| b.2 > 0.0 && b.3 > 0.0);
        let tile = match (obb, bx) {
            (true, Some(b)) => in_box(raw, b),
            (false, _) => raw,
            (true, None) => return Err(()),
        };
        // SVG 2: a tile with no area disables the paint.
        if !(tile.2 > 0.0 && tile.3 > 0.0) {
            return Err(());
        }
        let holder = chain
            .iter()
            .find(|n| {
                n.children()
                    .into_iter()
                    .filter_map(|c| self.kernel.node(c))
                    .any(|c| c.node_type.is_svg_element())
            })
            .unwrap_or(&chain[0]);
        if self.uses.contains(&(holder.id as u64)) || self.uses.len() > 16 {
            return Err(());
        }
        let vb = chain.iter().find_map(|n| view_box(n.props));
        let content = match vb {
            Some(_) => {
                let preserve = prop(PropId::PreserveAspectRatio);
                view_box_transform(vb, preserve, tile.2, tile.3).ok_or(())?
            }
            None => match (content_obb, bx) {
                (false, _) => tf::IDENTITY,
                (true, Some(b)) => tf::scale(b.2, b.3),
                (true, None) => return Err(()),
            },
        };
        let place = tf::mul(tf::translate(tile.0, tile.1), content);
        let transform = prop(PropId::PatternTransform)
            .and_then(TransformList::parse)
            .map_or(tf::IDENTITY, |t| t.matrix());
        let inherited = holder.computed_style(StyleMask::INHERITED);
        let style = cascade(holder, &inherited);
        let inner = match vb {
            Some(b) => Viewport {
                width: b.width,
                height: b.height,
            },
            None => vp,
        };
        self.uses.push((2 << 44) | from.id as u64);
        self.uses.push(holder.id as u64);
        let at = tf::mul(ctm, tf::mul(transform, place));
        let children = self.children(holder, &style, inner, at);
        let uid = self.uid(holder.id);
        self.uses.pop();
        self.uses.pop();
        Ok(Some(Pattern {
            tile,
            transform,
            items: vec![wrapper(holder, uid, place, at, children)],
        }))
    }
}

/// One group holding a reference's content, placed by `matrix`.
fn wrapper(node: &NodeRef<'_>, uid: u64, matrix: Affine, ctm: Affine, children: Vec<Item>) -> Item {
    Item {
        id: node.id,
        uid,
        key: node.key,
        opacity: 1.0,
        transform: (!tf::is_identity(matrix)).then_some(Transform {
            origin: (0.0, 0.0),
            translate: (0.0, 0.0),
            rotate: 0.0,
            scale: 1.0,
            matrix,
        }),
        ctm,
        clip: None,
        mask: None,
        filter: None,
        blend: 0,
        isolate: false,
        instance: true,
        kind: Kind::Group(children),
    }
}
