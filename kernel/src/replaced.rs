//! A replaced element's measure: an image, video, `svg`, canvas or iframe is
//! sized from its natural size, else CSS Images 3 §5's default object size,
//! never from children.
//!
//! Taffy applies the ratio (natural, or authored `aspect-ratio`) to a known
//! dimension and clamps (patches 5 and 12); this answers only what the
//! element itself contributes. Literal Chrome cases: `tests/it/browser_ratio.rs`
//! (images) and `tests/it/browser_replaced.rs` (the default object size).

use crate::arena::NodeArena;
use crate::generated::NodeType;
use taffy::geometry::{Rect, Size};
use taffy::style::{AvailableSpace, BoxSizing};
use taffy::tree::SizingMode;

/// The content size of `slot` given what is `known` and the `space` offered,
/// or `None` when it is not a replaced element. `inset` is its padding plus
/// border.
pub(crate) fn measure(
    arena: &NodeArena,
    slot: u32,
    style: &taffy::Style,
    inset: Rect<f32>,
    known: Size<Option<f32>>,
    space: Size<AvailableSpace>,
    sizing_mode: SizingMode,
) -> Option<Size<f32>> {
    let node_type = arena.node_type(slot);
    if !node_type.is_replaced() {
        return None;
    }
    let natural = arena.intrinsic(slot);
    let default = node_type.default_object_size();
    // What the ratio relates: the content box, or the border box
    // (box-sizing, unless `auto <ratio>`).
    let (pw, ph) = if style.aspect_ratio_content_box || style.box_sizing == BoxSizing::ContentBox {
        (0.0, 0.0)
    } else {
        (inset.horizontal_axis_sum(), inset.vertical_axis_sum())
    };
    // A ratio without a natural size: an `svg`'s view box, or
    // `aspect-ratio` on an iframe, a video before its metadata or an `svg`
    // without a view box.
    let unsized_ratio = match (natural, default) {
        (Some(_), _) | (None, Some((_, true))) => None,
        // The style's ratio: the view box's under `aspect-ratio: auto`, else
        // the authored one.
        _ => style
            .aspect_ratio
            .filter(|_| default.is_some() || crate::svg::natural_ratio(arena, slot).is_some()),
    };
    if let Some(ratio) = unsized_ratio {
        // @ref LLP 1055.000 D4 — a replaced element with a ratio and no
        // natural size fills the offered width (CSS Sizing's stretch fit, as
        // Chrome sizes one). Its intrinsic width starts at zero, or the
        // 300 px default object width at max-content when a percentage
        // width cannot resolve.
        let percent = style.size.width.into_raw().uses_percentage();
        let width = known.width.unwrap_or_else(|| {
            known
                .height
                .map(|h| (h + ph) * ratio - pw)
                .unwrap_or(match space.width {
                    AvailableSpace::Definite(w) => w,
                    AvailableSpace::MaxContent if percent => 300.0,
                    _ => 0.0,
                })
                // A border-box ratio must fit the vertical inset even when
                // no content contributes width. A given width still wins.
                .max((ph * ratio - pw).max(0.0))
        });
        return Some(Size {
            width,
            height: known.height.unwrap_or(((width + pw) / ratio - ph).max(0.0)),
        });
    }
    // The natural size, else the default object size; nothing at all
    // before an image loads, as a broken `<img>` is 0×0.
    let Some((iw, ih)) = natural.or(default.map(|(size, _)| size)) else {
        return Some(Size::ZERO);
    };
    // An unsized SVG's own content minimum is zero, but its contribution
    // to an ancestor includes the default object width. Taffy's ContentSize
    // probe asks for the former (a flex item's automatic minimum);
    // InherentSize asks for the latter (a block child's contribution).
    if node_type == NodeType::Svg
        && natural.is_none()
        && known.width.is_none()
        && space.width == AvailableSpace::MinContent
        && sizing_mode == SizingMode::ContentSize
    {
        return Some(Size {
            width: 0.0,
            height: known.height.unwrap_or(ih),
        });
    }
    // With neither dimension known, an authored ratio takes the natural
    // width's height (CSS Sizing 4 §5.1), as Chrome sizes a canvas.
    let ih = match (known.width, known.height, style.aspect_ratio) {
        (None, None, Some(r)) => ((iw + pw) / r - ph).max(0.0),
        _ => ih,
    };
    Some(Size {
        width: known.width.unwrap_or(iw),
        height: known.height.unwrap_or(ih),
    })
}

/// The role an image draws as a system symbol, as written after `symbol:`
/// (`sf/<name>` or a portable role), or `None` for any other image.
pub(crate) fn symbol_role(arena: &NodeArena, slot: u32) -> Option<&str> {
    if arena.node_type(slot) != NodeType::Image {
        return None;
    }
    arena
        .props(slot)
        .str(crate::PropId::ImageSource)?
        .strip_prefix("symbol:")
}

/// The Apple system symbol a `symbol:` role draws: an `sf/<name>`'s own
/// name, an icon token's SF Symbol (LLP 1121 D10), or a portable role's
/// Apple name; "" for a role with none, which draws no glyph.
pub fn apple_symbol_name(role: &str) -> &str {
    if let Some(icon) = Icon::parse(role) {
        return icon.sf;
    }
    role.strip_prefix("sf/")
        .or_else(|| crate::generated::symbol(role).map(|s| s.0))
        .unwrap_or("")
}

/// An icon token's source form, as the build writes it after `symbol:`
/// (LLP 1121 D10): `icon|<sf>|<view_box>|<path>|<svg>`, each empty for none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon<'a> {
    /// The SF Symbol Apple draws.
    pub sf: &'a str,
    /// The SVG's viewBox, `min-x min-y width height`.
    pub view_box: &'a str,
    /// Its paths' `d`, drawn as a template (filled); empty for an SVG the
    /// web draws as the file.
    pub path: &'a str,
    /// The SVG's path under `assets/`.
    pub svg: &'a str,
}

impl<'a> Icon<'a> {
    /// The icon a name after `symbol:` is, when it is one.
    pub fn parse(name: &'a str) -> Option<Icon<'a>> {
        let mut parts = name.strip_prefix("icon|")?.splitn(4, '|');
        Some(Icon {
            sf: parts.next()?,
            view_box: parts.next()?,
            path: parts.next()?,
            svg: parts.next()?,
        })
    }
}

/// A symbol's path for a host that draws paths (LLP 1035.004 D1, LLP 1121
/// D10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolPath<'a> {
    /// Its paths' `d`, joined by `;` (an icon's SVG may hold several, each
    /// filled on its own).
    pub d: &'a str,
    /// Filled, else stroked (a role's outline).
    pub filled: bool,
    /// Filled even-odd (a `-fill` role's silhouette), else nonzero (an icon).
    pub even_odd: bool,
    /// Its viewBox, `min-x min-y width height`: a role's is `0 0 24 24`.
    pub view_box: &'a str,
}

/// The path a name after `symbol:` draws: a role, an icon token's form, or
/// `sf/<name>` that is a role's Apple name; `None` for a name no path draws.
pub fn symbol_path(name: &str) -> Option<SymbolPath<'_>> {
    if let Some(icon) = Icon::parse(name) {
        return (!icon.path.is_empty()).then_some(SymbolPath {
            d: icon.path,
            filled: true,
            even_odd: false,
            view_box: icon.view_box,
        });
    }
    let role = match name.strip_prefix("sf/") {
        Some(sf) => crate::generated::SYMBOL_ROLES
            .iter()
            .copied()
            .find(|r| crate::generated::symbol(r).is_some_and(|s| s.0 == sf))?,
        None => name,
    };
    crate::generated::symbol(role).map(|(_, d, filled)| SymbolPath {
        d,
        filled,
        even_odd: filled,
        view_box: "0 0 24 24",
    })
}

#[cfg(test)]
mod tests {
    use super::{apple_symbol_name, symbol_path};

    /// LLP 1121 D10: an icon's form, and an SF Symbol that is a role's own
    /// Apple name, draw everywhere; another SF Symbol only on Apple.
    #[test]
    fn an_icon_and_a_roles_apple_name_have_paths() {
        let icon = "icon|bubble.left|0 -960 960 960|M0 0h1Z|icons/reply.svg";
        assert_eq!(apple_symbol_name(icon), "bubble.left");
        let path = symbol_path(icon).unwrap();
        assert_eq!(
            (path.d, path.filled, path.even_odd, path.view_box),
            ("M0 0h1Z", true, false, "0 -960 960 960")
        );
        assert_eq!(symbol_path("icon||0 0 24 24||icons/two.svg"), None);
        let back = crate::generated::symbol("back").unwrap();
        let role = symbol_path("back").unwrap();
        assert_eq!(
            (role.d, role.filled, role.view_box),
            (back.1, back.2, "0 0 24 24")
        );
        assert_eq!(symbol_path(&format!("sf/{}", back.0)), Some(role));
        assert_eq!(symbol_path("sf/flag.checkered"), None);
        assert_eq!(apple_symbol_name("sf/flag.checkered"), "flag.checkered");
    }
}
