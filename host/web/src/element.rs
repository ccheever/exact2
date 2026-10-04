//! The element a node is on the web: its tag, its attributes and the CSS
//! the host adds to the kernel's — one projection, which the live host
//! creates imperatively and [`super::document`] writes as HTML.
//!
//! @ref LLP 1007 §1 (a bare node is a bare `<div>`) / LLP 1048 D1

use exact_kernel::svg::Paint;
use exact_kernel::SortedMap;
use exact_kernel::{Kernel, NodeFacts, NodeRef, NodeType, ObjectFit, PropId, PropValue, StyleId};

/// How a projection finds the element an SVG reference names: the kernel's
/// `resolve_id`, or a tree's own (LLP 1055.000 D3).
pub type Resolve<'r> = &'r dyn Fn(exact_kernel::ViewId, &str) -> Option<exact_kernel::ViewId>;

/// A canvas's element hosts its surface element under its children
/// (`glue.js`, LLP 1014 D2): a containing block for it, unless the author
/// positioned the canvas, and a stacking context of its own — the
/// `isolation: isolate` the web's `drawable` implies — so the surface paints
/// above the canvas's background and below its children. A container a
/// button holds is a `<span>` ([`tag_for`]) whose box is still a block unless
/// a row says otherwise, as a `<div>`'s is.
pub(super) fn host_css(node: &NodeRef<'_>, css: String, tag: &str) -> String {
    host_css_of(&node.facts(), css, tag)
}

/// Whether a text is its box's text content on the web, not an element box of
/// its own (LLP 1007.001): the only child of a box or a button, with no style
/// row, no prop but its text and no handler — nothing a box of its own could
/// show or hear. Its element stays (the runtime writes its text, the agent
/// reads its node) as `display: contents`, so it makes no box (an inline
/// box under a box that restricts touch, [`contents`]), and a flex
/// box holding it is a block ([`blocks`]): its text is the box's own line
/// boxes, laid out and painted as a stretched style-less block child's are.
/// Only where the two lay out alike — a block, or a flex column that
/// stretches and starts its items — and not where the box's own rows would
/// reach its inline content (`text-overflow`, `line-clamp`).
pub fn folds(
    text: &NodeFacts<'_>,
    parent: &NodeFacts<'_>,
    outer: Option<&NodeFacts<'_>>,
    only_child: bool,
    handled: bool,
) -> bool {
    only_child
        && !handled
        && text.node_type == NodeType::Text
        && !text.is_inline_run()
        && text.style.mask == exact_kernel::StyleMask::EMPTY
        && text.props.iter().all(|(p, _)| p == PropId::Text)
        && matches!(parent.node_type, NodeType::View | NodeType::Pressable)
        && !parent.style.mask.has(StyleId::TextOverflow)
        && !parent.style.mask.has(StyleId::LineClamp)
        && lays_out_as_block(parent)
        && (parent.node_type != NodeType::Pressable
            || parent.style.display == exact_kernel::Display::Block
            || content_tall(parent, outer))
}

/// A `<button>` that is a block centers its line boxes in its height, so a
/// flex button becomes one only where its height is its content's: none of
/// its own, nothing to grow into, and not stretched across a row by `outer`.
fn content_tall(b: &NodeFacts<'_>, outer: Option<&NodeFacts<'_>>) -> bool {
    use exact_kernel::{AlignItems, AlignSelf, Dimension, Display, FlexDirection, PositionType};
    let s = b.style;
    let Some(o) = outer.map(|o| o.style) else {
        return false;
    };
    let sized = s.height != Dimension::Auto
        || s.min_height != Dimension::Auto
        || s.aspect_ratio != Default::default()
        || s.position_type == PositionType::Absolute;
    !sized
        && match (o.display, o.flex_direction) {
            (Display::Block, _) => true,
            (Display::Flex, FlexDirection::Column | FlexDirection::ColumnReverse) => {
                s.flex_grow == 0.0 && s.flex_basis == Dimension::Auto
            }
            (Display::Flex, _) => match s.align_self {
                AlignSelf::Auto => {
                    !matches!(o.align_items, AlignItems::Normal | AlignItems::Stretch)
                }
                a => a != AlignSelf::Stretch,
            },
            _ => false,
        }
}

/// A box whose one anonymous child lays out as a block's would.
fn lays_out_as_block(b: &NodeFacts<'_>) -> bool {
    use exact_kernel::{AlignItems, Display, FlexDirection, JustifyContent};
    let s = b.style;
    match s.display {
        Display::Block => true,
        Display::Flex => {
            s.flex_direction == FlexDirection::Column
                && matches!(
                    s.justify_content,
                    JustifyContent::Normal | JustifyContent::FlexStart | JustifyContent::Start
                )
                && matches!(s.align_items, AlignItems::Normal | AlignItems::Stretch)
        }
        _ => false,
    }
}

/// [`folds`] for a node of `kernel`, as its tree stands.
pub(super) fn folded(kernel: &Kernel, node: &NodeRef<'_>, handled: bool) -> bool {
    node.node_type == NodeType::Text
        && node.parent.and_then(|p| kernel.node(p)).is_some_and(|p| {
            let outer = p.parent.and_then(|o| kernel.node(o)).map(|o| o.facts());
            folds(
                &node.facts(),
                &p.facts(),
                outer.as_ref(),
                p.children().len() == 1,
                handled,
            )
        })
}

/// Whether a node of `kernel` holds a folded text: its only child folds.
pub(super) fn holds_folded(
    kernel: &Kernel,
    node: &NodeRef<'_>,
    handled: &dyn Fn(exact_kernel::ViewId) -> bool,
) -> bool {
    let children = node.children();
    let outer = node.parent.and_then(|o| kernel.node(o)).map(|o| o.facts());
    children.len() == 1
        && kernel.node(children[0]).is_some_and(|c| {
            c.node_type == NodeType::Text
                && folds(
                    &c.facts(),
                    &node.facts(),
                    outer.as_ref(),
                    true,
                    handled(c.id),
                )
        })
}

/// A box holding a folded text ([`folds`]) is a block, which wraps no
/// anonymous item around its text.
pub fn blocks(mut css: String, holds_folded: bool) -> String {
    if holds_folded {
        css.push_str("display:block;");
    }
    css
}

/// A folded text's CSS ([`folds`]): no box (`display: contents`), or an
/// inline box under a box that restricts touch ([`touch_scoped`]). Chrome
/// 154 lays out the text of a `display: contents` element with a style that
/// keeps no effective `touch-action`, so a touch that starts on its glyphs
/// under a `touch-action: none` pan is the browser's, cancelled after its
/// first move (smoke step 11b); an inline box keeps its ancestors' touch
/// action, at one more layout object (LLP 1007.001 §5).
pub fn contents(css: String, folded: bool, touch_scoped: bool) -> String {
    if !folded {
        return css;
    }
    let mut css = css.replace("display:block;", "");
    css.push_str(if touch_scoped {
        "display:inline;"
    } else {
        "display:contents;"
    });
    css
}

/// Whether a box restricts touch on the web: an authored `touch-action`
/// other than `auto`, or what the page gives `touch-action: none` — a drag
/// timeline's source, a game action, a reorder handle.
pub fn restricts_touch(n: &NodeFacts<'_>) -> bool {
    use exact_kernel::{StyleId, TouchAction};
    (n.style.mask.has(StyleId::TouchAction) && n.style.touch_action != TouchAction::Auto)
        || n.style.mask.has(StyleId::DragTimeline)
        || n.props.str(PropId::Action).is_some()
        || n.props.str(PropId::ReorderFor).is_some()
}

/// A node of `kernel`'s CSS with its fold's ([`contents`]), as its tree stands.
pub(super) fn folded_css(
    kernel: &Kernel,
    node: &NodeRef<'_>,
    css: String,
    handled: bool,
) -> String {
    contents(
        css,
        folded(kernel, node, handled),
        touch_scoped(kernel, node),
    )
}

/// Whether a node of `kernel` sits under a box that [`restricts_touch`].
pub(super) fn touch_scoped(kernel: &Kernel, node: &NodeRef<'_>) -> bool {
    let mut up = node.parent;
    while let Some(p) = up.and_then(|p| kernel.node(p)) {
        if restricts_touch(&p.facts()) {
            return true;
        }
        up = p.parent;
    }
    false
}

/// [`host_css`], from a node's facts.
pub fn host_css_of(node: &NodeFacts<'_>, mut css: String, tag: &str) -> String {
    if tag == "span" && !node.is_inline_run() && !css.split(';').any(|d| d.starts_with("display:"))
    {
        css.push_str("display:block;");
    }
    if node.node_type == NodeType::Canvas {
        if !(css.starts_with("position:") || css.contains(";position:")) {
            css.push_str("position:relative;");
        }
        css.push_str("isolation:isolate;");
        canvas_css(node, &mut css);
    }
    // A root is a block formatting context in the kernel, as CSS's root
    // element is: its first child's top margin stays inside it. On the web a
    // root is an element inside `#exact-root`, and the margin would collapse
    // through it to the page, so a block root establishes its own context
    // (LLP 1001 §1). The last `display` wins, as it does in `cssText`.
    if node.is_root
        && css
            .split(';')
            .filter_map(|d| d.strip_prefix("display:"))
            .next_back()
            .is_none_or(|display| display == "block")
    {
        css.push_str("display:flow-root;");
    }
    // @ref LLP 1074 T1 — a root is the containing block of every absolutely
    // positioned box no positioned ancestor holds, as the kernel's is.
    if node.is_root && node.style.position_type == exact_kernel::PositionType::Static {
        css.push_str("position:relative;");
    }
    // A raster image with a `tint-color` is a template (LLP 1011 §3): its
    // alpha masks the tint, fitted and centered in the content box as
    // `object-fit` fits the picture, which moves out of the box, where the
    // replaced element's own clip hides it. `scale-down` needs the natural
    // size, which only the page knows: the glue sets `--exact-tint-fit`.
    // This masks the box paint too (declared in LLP 1001 §1). An img cannot
    // paint a ::before/::after layer; keep its replaced-element sizing.
    if node.node_type == NodeType::Image && node.style.mask.has(StyleId::TintColor) {
        if let Some(source) = node
            .props
            .str(PropId::ImageSource)
            .filter(|s| !s.starts_with("symbol:"))
        {
            let size = match node.style.object_fit {
                ObjectFit::Fill => "100% 100%",
                ObjectFit::Contain => "contain",
                ObjectFit::Cover => "cover",
                ObjectFit::None => "auto",
                ObjectFit::ScaleDown => "var(--exact-tint-fit,contain)",
            };
            css.push_str("background-color:var(--exact-tint);mask-image:url(");
            css.push_str(&crate::css::css_string(source));
            css.push_str(");mask-size:");
            css.push_str(size);
            css.push_str(";mask-repeat:no-repeat;mask-position:center;mask-origin:content-box;mask-clip:content-box;object-position:-100000px 0;");
        }
    }
    // @ref LLP 1053.000 D4 — linked when the plan names a material.
    if let (Some(name), Some(material)) = (
        node.props.str(PropId::BackgroundMaterial),
        crate::link::linked().materials,
    ) {
        (material.0)(&mut css, name);
    }
    css
}

/// A canvas's `div` sized as a `<canvas>` is: a replaced element whose
/// natural size is 300×150, ratio 2:1, and whose children never size it.
/// `contain: size` keeps the children out and `contain-intrinsic-size`
/// stands in for the natural size (none when a height is given, so the
/// width comes through the ratio); `justify-self: start` keeps an auto width
/// at that size in block flow, where a `div` would stretch; the natural
/// ratio holds under `aspect-ratio: auto`; an automatic minimum height is
/// its content's, as a replaced box's is, unless a height row bounds it; an
/// absolutely positioned one is its natural size whatever its insets. Chrome
/// sizes it as a `<canvas>` everywhere but a flex row's automatic minimum
/// width, which would need the parent's direction (LLP 1001 §1).
fn canvas_css(node: &NodeFacts<'_>, css: &mut String) {
    use exact_kernel::{Dimension, PositionType};
    let style = &node.style;
    let given = |id: StyleId, value: Dimension| style.mask.has(id) && value != Dimension::Auto;
    let (width, height) = (
        given(StyleId::Width, style.width),
        given(StyleId::Height, style.height),
    );
    css.push_str("contain:size;justify-self:start;");
    if !height {
        css.push_str("contain-intrinsic-size:300px 150px;");
    }
    if style.aspect_ratio.defers_to_natural() {
        css.push_str("aspect-ratio:auto 2/1;");
    }
    if !given(StyleId::MinHeight, style.min_height) {
        css.push_str(if height || given(StyleId::MaxHeight, style.max_height) {
            "min-height:0;"
        } else {
            "min-height:min-content;"
        });
    }
    if style.position_type == PositionType::Absolute {
        if !width {
            css.push_str("width:fit-content;");
        }
        if !height {
            css.push_str("height:fit-content;");
        }
    }
}

/// The element for a node: its type, refined by `semanticTag`. A `<button>`
/// holds only phrasing content, so there a container — a box, a paragraph,
/// a heading, a landmark — is a `<span>` with the same style (LLP 1007 §1).
pub(super) fn tag_for<'a>(node: &NodeRef<'a>, in_button: bool) -> &'a str {
    tag_of(&node.facts(), in_button)
}

/// [`tag_for`], from a node's facts.
pub fn tag_of<'a>(node: &NodeFacts<'a>, in_button: bool) -> &'a str {
    // @ref LLP 1024 D2 — a module node is its custom element, by the name
    // the plan carries, checked again: plan bytes are network bytes.
    if let Some(name) = node
        .props
        .str(PropId::NativeViewModuleName)
        .filter(|n| node.node_type == NodeType::NativeView && module_name(n))
    {
        return name;
    }
    match element(node) {
        "div" | "main" | "header" | "nav" | "section" | "footer" | "article" | "aside" | "h1"
        | "h2" | "h3" | "h4" | "h5" | "h6"
            if in_button =>
        {
            "span"
        }
        tag => tag,
    }
}

/// HTML's potential custom element name, lowercase (LLP 1024 D1): the
/// compiler's admission, repeated where plan bytes become a DOM tag.
pub(super) fn module_name(name: &str) -> bool {
    const RESERVED: [&str; 8] = [
        "annotation-xml",
        "color-profile",
        "font-face",
        "font-face-src",
        "font-face-uri",
        "font-face-format",
        "font-face-name",
        "missing-glyph",
    ];
    let word = |w: &str| {
        let mut chars = w.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    name.contains('-') && name.split('-').all(word) && !RESERVED.contains(&name)
}

/// Whether a `<button>` holds the node.
pub(super) fn in_button(kernel: &Kernel, node: &NodeRef<'_>) -> bool {
    let mut parent = node.parent;
    while let Some(p) = parent.and_then(|id| kernel.node(id)) {
        if element(&p.facts()) == "button" {
            return true;
        }
        parent = p.parent;
    }
    false
}

/// The element for a node wherever it is: its type, refined by `semanticTag`.
fn element(node: &NodeFacts<'_>) -> &'static str {
    if node.node_type == NodeType::TextInput
        && node.props.str(PropId::SemanticTag) == Some("textarea")
    {
        return "textarea";
    }
    if node.props.str(PropId::Href).is_some()
        && (node.is_inline_run() || node.node_type == NodeType::Pressable)
    {
        return "a";
    }
    if let Some(t) = node.props.str(PropId::SemanticTag) {
        match t {
            "main" => return "main",
            "header" => return "header",
            "nav" => return "nav",
            "section" => return "section",
            "footer" => return "footer",
            "article" => return "article",
            "aside" => return "aside",
            "dialog" => return "dialog",
            _ => {}
        }
    }
    match node.node_type {
        NodeType::View | NodeType::List | NodeType::NativeView => "div",
        // @ref LLP 1055 D4 — real inline SVG, created in the SVG namespace.
        NodeType::Svg => "svg",
        NodeType::SvgGroup => "g",
        NodeType::SvgPath => "path",
        NodeType::SvgPolyline => "polyline",
        NodeType::SvgPolygon => "polygon",
        NodeType::SvgCircle => "circle",
        NodeType::SvgLine => "line",
        NodeType::SvgRect => "rect",
        NodeType::SvgEllipse => "ellipse",
        NodeType::SvgViewport => "svg",
        NodeType::SvgDefs => "defs",
        NodeType::SvgLinearGradient => "linearGradient",
        NodeType::SvgRadialGradient => "radialGradient",
        NodeType::SvgStop => "stop",
        NodeType::SvgUse => "use",
        NodeType::SvgSymbol => "symbol",
        NodeType::SvgClipPath => "clipPath",
        NodeType::SvgMarker => "marker",
        NodeType::SvgMask => "mask",
        NodeType::SvgPattern => "pattern",
        NodeType::SvgForeignObject => "foreignObject",
        NodeType::SvgFilter => "filter",
        // @ref LLP 1055.000 D14 — a primitive's tag is its `fe` prop.
        NodeType::SvgFe => {
            let fe = node.props.str(PropId::Fe).unwrap_or("");
            FE_TAGS
                .iter()
                .find(|t| **t == fe)
                .copied()
                .unwrap_or("feFlood")
        }
        NodeType::SvgText => "text",
        NodeType::SvgTSpan => "tspan",
        NodeType::ScrollView => "div",
        NodeType::Text => {
            if node.is_inline_run() {
                "span"
            } else if exact_kernel::control::is_option_node(node.node_type, node.props) {
                // @ref LLP 1069.001 D2 — the select's own options.
                "option"
            } else {
                match heading_level(node) {
                    Some(1) => "h1",
                    Some(2) => "h2",
                    Some(3) => "h3",
                    Some(4) => "h4",
                    Some(5) => "h5",
                    Some(6) => "h6",
                    _ => "div",
                }
            }
        }
        NodeType::Image => "img",
        NodeType::TextInput => "input",
        NodeType::Pressable => "button",
        NodeType::Control if node.props.str(PropId::Type) == Some("select") => "select",
        // @ref LLP 1069.011 D8 — a native button is the browser's own.
        NodeType::Control if node.props.str(PropId::Type) == Some("button") => "button",
        NodeType::Control => "input",
        NodeType::Canvas => "canvas",
        NodeType::WebView => "iframe",
        NodeType::Video => "video",
        // Never created: a head is the page's `<head>` (LLP 1048.003 D1).
        NodeType::Head => "template",
    }
}

/// A text block's heading level, when it is a heading: `aria-level` with no
/// other role. HTML's `h1`–`h6` carry levels 1–6 into the accessibility tree
/// (an `aria-level` on a role-less `div` is ignored); a deeper level is a
/// `div` with `role="heading"`. `index.html` resets the UA heading styles, so
/// the box stays a bare div's. The tag is fixed at creation; a level bound
/// to data that changes later still reaches `aria-level`.
fn heading_level(node: &NodeFacts<'_>) -> Option<i64> {
    if node.node_type != NodeType::Text || node.is_inline_run() {
        return None;
    }
    if node
        .props
        .str(PropId::AccessibilityRole)
        .is_some_and(|role| role != "heading")
    {
        return None;
    }
    match node.props.get(PropId::AccessibilityHeadingLevel) {
        Some(PropValue::Int(level)) if *level >= 1 => Some(*level),
        _ => None,
    }
}

/// The filter primitives' tags (LLP 1055.000 D14), as `fe` holds them.
const FE_TAGS: [&str; 24] = [
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feConvolveMatrix",
    "feDiffuseLighting",
    "feDisplacementMap",
    "feDropShadow",
    "feFlood",
    "feFuncR",
    "feFuncG",
    "feFuncB",
    "feFuncA",
    "feGaussianBlur",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
    "feSpecularLighting",
    "feTile",
    "feTurbulence",
    "feDistantLight",
    "fePointLight",
    "feSpotLight",
];

/// Props as DOM attributes/properties. Names are the DOM's.
/// The rows a node's `cssText` carries. A nested `svg` takes `x`, `y`,
/// `width` and `height` as attributes instead ([`props_for`]): Chrome 154
/// lays one out from its attributes and ignores those CSS properties
/// (LLP 1055.000 D4).
pub(super) fn css_style<'a>(
    kernel: &Kernel,
    node: &NodeRef<'a>,
) -> std::borrow::Cow<'a, exact_kernel::StyleProps> {
    css_style_of(&|from, id| kernel.resolve_id(from, id), &node.facts())
}

/// [`css_style`], from a node's facts and its tree's references.
pub fn css_style_of<'a>(
    resolve: Resolve<'_>,
    node: &NodeFacts<'a>,
) -> std::borrow::Cow<'a, exact_kernel::StyleProps> {
    let as_attributes = attribute_rows(node.node_type);
    let url = |p: &Paint| matches!(p, Paint::Url(..));
    // @ref LLP 1069.001 D2 — an option is `display: none` to layout, never
    // to the browser's menu, which a hidden `<option>` leaves out.
    if exact_kernel::control::is_option_node(node.node_type, node.props) {
        let mut style = node.style.clone();
        let mut mask = exact_kernel::StyleMask::EMPTY;
        mask.set(exact_kernel::StyleId::Display);
        style.clear(mask);
        return std::borrow::Cow::Owned(style);
    }
    if as_attributes.is_empty()
        && !url(&node.style.fill)
        && !url(&node.style.stroke)
        && node.style.clip_path.url().is_none()
        && node.style.svg_mask.url().is_none()
        && node.style.filter.is_none()
        && [
            &node.style.marker_start,
            &node.style.marker_mid,
            &node.style.marker_end,
        ]
        .iter()
        .all(|m| m.url().is_none())
    {
        return std::borrow::Cow::Borrowed(node.style);
    }
    let mut style = node.style.clone();
    let mut mask = exact_kernel::StyleMask::EMPTY;
    for row in as_attributes {
        mask.set(row.0);
    }
    style.clear(mask);
    // @ref LLP 1055.000 D10 — a clipPath by the id the page gives it.
    if let Some(target) = style.clip_path.url().and_then(|id| resolve(node.id, id)) {
        if let Some(c) = exact_kernel::clip::ClipPath::parse(&format!("url(#{})", dom_id(target))) {
            style.clip_path = c;
        }
    }
    // @ref LLP 1055.000 D14 — a filter by the id the page gives it.
    for f in style.filter.0.iter_mut() {
        if let exact_kernel::svg::filter::FilterFn::Url(id) = f {
            if let Some(target) = resolve(node.id, id) {
                *id = dom_id(target).into();
            }
        }
    }
    // @ref LLP 1055.000 D10 — a mask by the id the page gives it.
    if let Some(target) = style.svg_mask.url().and_then(|id| resolve(node.id, id)) {
        style.svg_mask = exact_kernel::svg::MarkerRef(Some(dom_id(target).into()));
    }
    // @ref LLP 1055.000 D9 — a marker by the id the page gives it.
    for marker in [
        &mut style.marker_start,
        &mut style.marker_mid,
        &mut style.marker_end,
    ] {
        if let Some(target) = marker.url().and_then(|id| resolve(node.id, id)) {
            *marker = exact_kernel::svg::MarkerRef(Some(dom_id(target).into()));
        }
    }
    // @ref LLP 1055.000 D3 — a paint server by the id the page gives it.
    for paint in [&mut style.fill, &mut style.stroke] {
        if let Paint::Url(id, fallback) = paint {
            if let Some(target) = resolve(node.id, id) {
                *paint = Paint::Url(dom_id(target).into(), *fallback);
            }
        }
    }
    std::borrow::Cow::Owned(style)
}

/// The DOM id of an SVG element: unique by construction, so forty
/// instances of one component's `id="fade"` are forty ids (LLP 1055.000 D3).
fn dom_id(view: exact_kernel::ViewId) -> String {
    format!("x{view}")
}

/// Rows an element takes as attributes: Chrome 154 lays out a nested `svg`
/// and places a `use` from their attributes and ignores those CSS
/// properties; a `mask`'s and a `pattern`'s region is attributes only, as
/// is a radial gradient's `cx`, `cy` and `r` are attributes
/// only (LLP 1055.000 D4, D7).
fn attribute_rows(t: NodeType) -> &'static [(exact_kernel::StyleId, &'static str)] {
    use exact_kernel::StyleId::*;
    match t {
        NodeType::SvgViewport
        | NodeType::SvgUse
        | NodeType::SvgMask
        | NodeType::SvgPattern
        | NodeType::SvgForeignObject
        | NodeType::SvgFilter
        | NodeType::SvgFe => &[(X, "x"), (Y, "y"), (Width, "width"), (Height, "height")],
        NodeType::SvgRadialGradient => &[(Cx, "cx"), (Cy, "cy"), (R, "r")],
        _ => &[],
    }
}

/// An SVG element's references and attribute rows as the page takes them:
/// its `id` rewritten to its DOM id, `href` to its target's, and the rows
/// of [`attribute_rows`] as attributes.
pub(super) fn svg_props(kernel: &Kernel, node: &NodeRef<'_>, out: &mut SortedMap<String, String>) {
    svg_props_of(&|from, id| kernel.resolve_id(from, id), &node.facts(), out)
}

/// [`svg_props`], from a node's facts and its tree's references.
pub fn svg_props_of(
    resolve: Resolve<'_>,
    node: &NodeFacts<'_>,
    out: &mut SortedMap<String, String>,
) {
    if !node.node_type.is_svg_element() {
        return;
    }
    if node.props.str(PropId::Id).is_some() {
        out.insert("id".into(), dom_id(node.id));
    }
    if let Some(target) = node
        .props
        .str(PropId::Href)
        .and_then(|h| h.strip_prefix('#'))
        .and_then(|h| resolve(node.id, h))
    {
        out.insert("href".into(), format!("#{}", dom_id(target)));
    }
    for (row, name) in attribute_rows(node.node_type) {
        if !node.style.mask.has(*row) {
            continue;
        }
        let text = match node.style.get(*row) {
            exact_kernel::RowValue::Dimension(exact_kernel::Dimension::Points(v)) => {
                exact_num::Shortest(v as f64).to_string()
            }
            exact_kernel::RowValue::Dimension(exact_kernel::Dimension::Percent(p)) => {
                format!("{}%", exact_num::Shortest(p as f64))
            }
            _ => continue,
        };
        out.insert((*name).into(), text);
    }
}

pub(super) fn props_for(node: &NodeRef<'_>) -> SortedMap<String, String> {
    props_of(&node.facts())
}

/// [`props_for`], from a node's facts.
pub fn props_of(node: &NodeFacts<'_>) -> SortedMap<String, String> {
    let mut out = SortedMap::new();
    if node.style.wrap_flow == exact_kernel::WrapFlow::Both {
        out.insert("data-wrap-flow".into(), "both".into());
    }
    if node.node_type == NodeType::Text
        && !node.is_inline_run()
        && !exact_kernel::control::is_option_node(node.node_type, node.props)
    {
        out.insert("data-exact-text".into(), String::new());
    }
    // A `markup="markdown"` text node paints its source as pieces the page
    // builds into spans (LLP 1045 D3, D4): the same expansion the native
    // hosts measure and paint, as one JSON value, never as HTML. Markdown is
    // a linked capability (LLP 1047 D3): boot admitted this plan only if the
    // artifact links it.
    let markup = (node.node_type == NodeType::Text
        && node.props.str(PropId::Markup) == Some("markdown")
        && !node.is_inline_run())
    .then_some(crate::link::linked().markup)
    .flatten();
    if let (Some(pieces), Some(source)) = (markup, node.props.str(PropId::Text)) {
        out.insert("markupPieces".into(), pieces(source));
    }
    for (id, value) in node.props.iter() {
        if markup.is_some() && id == PropId::Text {
            continue;
        }
        if id == PropId::Editable {
            out.insert(
                "readonly".into(),
                (value == &PropValue::Bool(false)).to_string(),
            );
            continue;
        }
        // @ref LLP 1075.003 §3.3 — the app's words are real attributes.
        if let (PropId::Dataset, PropValue::Str(json)) = (id, value) {
            if let Some(dataset) = crate::link::linked().dataset {
                for (word, value) in dataset(json) {
                    out.insert("data-".to_owned() + &word, value);
                }
            }
            continue;
        }
        let text = match value {
            PropValue::Str(s) => s.clone(),
            PropValue::Bool(b) => b.to_string(),
            PropValue::Int(i) => i.to_string(),
            // LLP 1053.000.000.000 D3: the reserved `-1` is written as `auto`.
            PropValue::Float(f) if id == PropId::GlassGroup && *f == -1.0 => "auto".into(),
            PropValue::Float(f) => crate::css::num(*f as f32),
        };
        let name = match id {
            PropId::Text => "text",
            PropId::Markup => "markup",
            PropId::TestId => "data-testid",
            // An image's label is its `alt`: the replaced element's text
            // alternative, shown when it does not load.
            PropId::AccessibilityLabel if node.node_type == NodeType::Image => "alt",
            PropId::AccessibilityLive => "aria-live",
            PropId::Autofocus => "autofocus",
            PropId::AccessibilityLabel => "aria-label",
            PropId::AccessibilityKeyShortcuts => "aria-keyshortcuts",
            PropId::AccessibilityRole => "role",
            PropId::AccessibilityHint => "aria-description",
            PropId::AccessibilityOrientation => "aria-orientation",
            PropId::AccessibilityControls => "aria-controls",
            PropId::AccessibilityHeadingLevel => "aria-level",
            PropId::AccessibilityPosInSet => "aria-posinset",
            PropId::AccessibilitySetSize => "aria-setsize",
            PropId::Placeholder => "placeholder",
            PropId::Type => "type",
            PropId::InputMode => "inputmode",
            PropId::Autocapitalize => "autocapitalize",
            PropId::Autocorrect => "autocorrect",
            PropId::Spellcheck => "spellcheck",
            PropId::Value => "value",
            PropId::ScrollTop => "scrollTop",
            PropId::ScrollLeft => "scrollLeft",
            PropId::ScrollFollowEnd => "scrollFollowEnd",
            PropId::ViewportFit => "viewportFit",
            PropId::InteractiveWidget => "interactiveWidget",
            PropId::NavigationKey => "navigationKey",
            PropId::NavigationBack => "navigationBack",
            PropId::NavigationPresentation => "navigationPresentation",
            PropId::NavigationSource => "navigationSource",
            PropId::Closedby => "closedby",
            PropId::ContextTarget => "contextTarget",
            PropId::ContextMagnify => "contextMagnify",
            PropId::SwipeContent => "swipeContent",
            PropId::SwipeLeading => "swipeLeading",
            PropId::SwipeTrailing => "swipeTrailing",
            PropId::Destructive => "data-destructive",
            PropId::EmojiPicker => "emojiPicker",
            PropId::BackgroundMaterial => "backgroundMaterial",
            // LLP 1053.000.000 D1: written, read by no rule, drawn nowhere.
            PropId::GlassGroup => "glassGroup",
            PropId::RetainFocus => "retainFocus",
            PropId::SwipeIndicator => "swipeIndicator",
            PropId::Href if text.is_empty() => continue,
            PropId::Href => "href",
            PropId::Disabled => "disabled",
            PropId::Min => "min",
            PropId::Max => "max",
            PropId::Step => "step",
            PropId::Inert => "inert",
            PropId::Lang => "lang",
            PropId::ImageSource => "src",
            PropId::Src => "src",
            PropId::Poster => "poster",
            PropId::Autoplay => "autoplay",
            PropId::Controls => "controls",
            PropId::Loop => "loop",
            PropId::Muted => "muted",
            PropId::Preload => "preload",
            PropId::Playsinline => "playsinline",
            PropId::Crossorigin => "crossorigin",
            PropId::Controlslist => "controlslist",
            PropId::Disablepictureinpicture => "disablepictureinpicture",
            PropId::Disableremoteplayback => "disableremoteplayback",
            PropId::Volume => "volume",
            PropId::PlaybackRate => "playbackRate",
            PropId::CurrentTime => "currentTime",
            PropId::Paused => "paused",
            PropId::PlaybackVisibilityThreshold => "playbackVisibilityThreshold",
            PropId::PreservesPitch => "preservesPitch",
            PropId::AllowsPictureInPicturePlayback => "allowsPictureInPicturePlayback",
            PropId::CanStartPictureInPictureAutomaticallyFromInline => {
                "canStartPictureInPictureAutomaticallyFromInline"
            }
            PropId::EntersFullScreenWhenPlaybackBegins => "entersFullScreenWhenPlaybackBegins",
            PropId::ExitsFullScreenWhenPlaybackEnds => "exitsFullScreenWhenPlaybackEnds",
            PropId::ShowsTimecodes => "showsTimecodes",
            PropId::AllowsVideoFrameAnalysis => "allowsVideoFrameAnalysis",
            PropId::RequiresLinearPlayback => "requiresLinearPlayback",
            PropId::PreferredPeakBitRate => "preferredPeakBitRate",
            PropId::PreferredForwardBufferDuration => "preferredForwardBufferDuration",
            PropId::AutomaticallyWaitsToMinimizeStalling => "automaticallyWaitsToMinimizeStalling",
            PropId::PreventsDisplaySleepDuringVideoPlayback => {
                "preventsDisplaySleepDuringVideoPlayback"
            }

            PropId::Sandbox => "sandbox",
            PropId::SemanticTag => continue,
            PropId::Checked => "checked",
            // The Popover API by identity (LLP 1021 D5): the browser owns
            // the top layer, light dismiss, and Escape once these land on
            // the real elements.
            PropId::Id => "id",
            PropId::Popover => "popover",
            PropId::Popovertarget => "popovertarget",
            PropId::Popovertargetaction => "popovertargetaction",
            PropId::Commandfor => "commandfor",
            PropId::Command => "command",
            PropId::AccessibilityChecked => "aria-checked",
            PropId::AccessibilitySelected => "aria-selected",
            PropId::AccessibilityExpanded => "aria-expanded",
            PropId::AccessibilityPressed => "aria-pressed",
            PropId::AccessibilityModal => "aria-modal",
            PropId::AccessibilityElementsHidden => "aria-hidden",
            // SVG 2 attributes by their exact (case-sensitive) names (LLP 1055 D1).
            PropId::ViewBox => "viewBox",
            PropId::PreserveAspectRatio => "preserveAspectRatio",
            PropId::Points => "points",
            PropId::D => "d",
            PropId::PathLength => "pathLength",
            PropId::X1 => "x1",
            PropId::Y1 => "y1",
            PropId::X2 => "x2",
            PropId::Y2 => "y2",
            PropId::Fx => "fx",
            PropId::Fy => "fy",
            PropId::Fr => "fr",
            PropId::GradientUnits => "gradientUnits",
            PropId::GradientTransform => "gradientTransform",
            PropId::SpreadMethod => "spreadMethod",
            PropId::Offset => "offset",
            PropId::ClipPathUnits => "clipPathUnits",
            PropId::MarkerWidth => "markerWidth",
            PropId::MarkerHeight => "markerHeight",
            PropId::RefX => "refX",
            PropId::RefY => "refY",
            PropId::Orient => "orient",
            PropId::MarkerUnits => "markerUnits",
            PropId::MaskUnits => "maskUnits",
            PropId::MaskContentUnits => "maskContentUnits",
            PropId::PatternUnits => "patternUnits",
            PropId::PatternContentUnits => "patternContentUnits",
            PropId::PatternTransform => "patternTransform",
            PropId::TextX => "x",
            PropId::TextY => "y",
            PropId::TextDx => "dx",
            PropId::TextDy => "dy",
            // @ref LLP 1055.000 D14 — a primitive's attributes by their SVG
            // names; `fe` is the tag itself.
            PropId::Fe => continue,
            PropId::FeDx => "dx",
            PropId::FeDy => "dy",
            PropId::FeScale => "scale",
            PropId::FeRadius => "radius",
            PropId::LightX => "x",
            PropId::LightY => "y",
            PropId::LightZ => "z",
            PropId::ButtonStyle => "data-button-style",
            other if matches!(node.node_type, NodeType::SvgFe | NodeType::SvgFilter) => {
                other.name()
            }
            other => {
                // Every other prop rides as `data-<name>` so nothing is lost.
                // Schema names are ASCII (`prop_names_are_ascii`), so ASCII
                // lowering is the whole lowering and links no Unicode tables.
                out.insert(format!("data-{}", other.name().to_ascii_lowercase()), text);
                continue;
            }
        };
        out.insert(name.to_string(), text);
    }
    if heading_level(node).is_some_and(|level| level > 6) {
        out.get_or_insert_with("role".into(), || "heading".into());
    }
    if node.node_type.scrolls_by_default() {
        out.insert("data-scroll".into(), "true".into());
    }
    if element(node) == "select" {
        // A `<select>` is its own kind; its `type` is not an attribute.
        out.remove("type");
    } else if node.node_type == NodeType::Control {
        out.get_or_insert_with("type".into(), || "checkbox".into());
        // @ref LLP 1069.001 D1 — WebKit's `switch`; a browser without it
        // draws a checkbox that ARIA still hears as a switch.
        if node.props.str(PropId::AccessibilityRole) == Some("switch") {
            out.insert("switch".into(), String::new());
        }
    }
    // A `<button>` submits a form unless it says otherwise; a `button` never does.
    if element(node) == "button" {
        out.get_or_insert_with("type".into(), || "button".into());
    }
    // A native button's look, its default named too (LLP 1069.011 D8).
    if node.node_type == NodeType::Control && node.props.str(PropId::Type) == Some("button") {
        out.get_or_insert_with("data-button-style".into(), || "bordered".into());
    }
    if node.node_type == NodeType::Image {
        if let Some(role) = node
            .props
            .str(PropId::ImageSource)
            .and_then(|s| s.strip_prefix("symbol:"))
        {
            let symbol = exact_kernel::generated::symbol(role);
            out.insert("data-symbol-source".into(), format!("symbol:{role}"));
            out.insert(
                "data-symbol-path".into(),
                symbol.map(|s| s.1).unwrap_or("").into(),
            );
            // A filled role's path is a silhouette, drawn filled, not stroked.
            if symbol.is_some_and(|s| s.2) {
                out.insert("data-symbol-fill".into(), String::new());
            }
            out.insert("alt".into(), String::new());
        }
    }
    // A role the element already has natively is left off (ARIA in HTML:
    // authors should not restate it): a button's `button`, a checkbox's
    // `checkbox`, a link's `link`. Its accessibility is the element's own.
    let implicit = match (element(node), out.get("type").map(String::as_str)) {
        ("button", _) => Some("button"),
        ("input", Some("checkbox")) => Some("checkbox"),
        ("a", _) if out.contains_key("href") => Some("link"),
        _ => None,
    };
    if implicit.is_some() && out.get("role").map(String::as_str) == implicit {
        out.remove("role");
    }
    out
}

#[cfg(test)]
mod name_tests {
    use super::host_css_of;
    use exact_kernel::{NodeFacts, NodeType, PropId, PropList, PropValue, StyleProps};

    #[test]
    fn prop_names_are_ascii() {
        for prop in exact_kernel::PropId::ALL {
            assert!(prop.name().is_ascii(), "{}", prop.name());
        }
    }

    #[test]
    fn page_reset_and_lowered_css_leave_native_control_sizing_to_chrome() {
        let page = include_str!("../index.html");
        let stylesheet = page
            .split_once("<style>")
            .and_then(|(_, rest)| rest.split_once("</style>"))
            .map(|(css, _)| css)
            .expect("page stylesheet");
        assert!(page.contains("select { display: block; }"));
        assert!(page.contains("input[type=\"checkbox\"] { box-sizing: border-box; }"));
        let mut file_appearance_restored = false;
        for rule in stylesheet.split('}') {
            let Some((selectors, declarations)) = rule.rsplit_once('{') else {
                continue;
            };
            if selectors.contains("input[type=\"file\"]")
                && declarations
                    .split(';')
                    .any(|declaration| declaration.trim() == "appearance: auto")
            {
                file_appearance_restored = true;
            }
            if selectors.split(',').any(|selector| {
                let selector = selector.trim();
                selector.contains("input")
                    || selector.contains("textarea")
                    || selector.contains("select")
                    || selector.contains("button")
            }) {
                assert!(
                    !declarations
                        .split(';')
                        .any(|declaration| declaration.trim_start().starts_with("width:")),
                    "control reset must not author a width: {selectors} {{{declarations}}}"
                );
            }
        }
        assert!(file_appearance_restored);
        for (tag, ty) in [("input", "checkbox"), ("select", "select")] {
            let style = StyleProps::default();
            let mut props = PropList::default();
            props.set(PropId::Type, PropValue::Str(ty.into()));
            let facts = NodeFacts {
                id: 1,
                node_type: NodeType::Control,
                style: &style,
                props: &props,
                is_root: false,
                inline_run: false,
            };
            let css = host_css_of(&facts, String::new(), tag);
            assert!(!css.contains("width:"), "{tag}: {css}");
            assert!(!css.contains("box-sizing:"), "{tag}: {css}");
        }
    }
}

impl<D: exact_runner::DataSource> super::Host<D> {
    /// A view's CSS as the page has it: the rows, the host's additions, a
    /// folded text's (LLP 1007.001) and its paint isolation.
    pub(crate) fn view_css(&self, node: &exact_kernel::NodeRef<'_>) -> String {
        let kernel = self.runner.kernel();
        let m = self.mirror.get(&node.id);
        let (css, _) = crate::css::css_text(&css_style(kernel, node), &self.font_names);
        let css = host_css(node, css, tag_for(node, m.is_some_and(|m| m.in_button)));
        let css = folded_css(kernel, node, css, m.is_some_and(|m| m.handled));
        let handled = |c| self.mirror.get(&c).is_some_and(|m| m.handled);
        let css = blocks(css, holds_folded(kernel, node, &handled));
        super::layers::with_isolation(css, self.layers.isolated(node.id))
    }

    /// A box's CSS and its text children's again when it or its children
    /// moved: whether a text folds into it (LLP 1007.001).
    pub(super) fn refold(&mut self, id: exact_kernel::ViewId, batch: &mut crate::batch::Batch) {
        let kernel = self.runner.kernel();
        let children = kernel.node(id).map(|n| n.children()).unwrap_or_default();
        // A button's fold reads its parent's rows ([`content_tall`]): a
        // button child is refolded, and so is the one text it holds.
        let held = |c: &exact_kernel::ViewId| {
            let n = kernel
                .node(*c)
                .filter(|n| n.node_type == NodeType::Pressable)?;
            let t = n.children();
            (t.len() == 1).then(|| t[0])
        };
        let texts: Vec<_> = children.iter().filter_map(held).collect();
        for child in std::iter::once(id).chain(children).chain(texts) {
            let Some(node) = kernel.node(child) else {
                continue;
            };
            if child != id && !matches!(node.node_type, NodeType::Text | NodeType::Pressable) {
                continue;
            }
            let css = self.view_css(&node);
            if let Some(m) = self.mirror.get_mut(&child).filter(|m| m.css != css) {
                batch.style(child, &css);
                m.css = css;
            }
        }
    }
}

#[cfg(test)]
mod dataset_tests {
    use exact_kernel::{NodeFacts, NodeType, PropId, PropKind, PropList, PropValue, StyleProps};

    fn written(
        node_type: NodeType,
        prop: PropId,
        value: PropValue,
    ) -> super::SortedMap<String, String> {
        let style = StyleProps::default();
        let mut props = PropList::new();
        props.set(prop, value);
        super::props_of(&NodeFacts {
            id: 1,
            node_type,
            style: &style,
            props: &props,
            is_root: false,
            inline_run: false,
        })
    }

    /// An SVG filter primitive's own prop: the compiler sets it only on an
    /// `fe*` or `filter` element, which writes it under its own name.
    fn svg_only(prop: PropId) -> bool {
        let name = prop.name();
        name.starts_with("pointsAt")
            || name.starts_with("specular")
            || name.ends_with("ChannelSelector")
            || matches!(
                name,
                "amplitude"
                    | "azimuth"
                    | "baseFrequency"
                    | "bias"
                    | "diffuseConstant"
                    | "divisor"
                    | "edgeMode"
                    | "elevation"
                    | "exponent"
                    | "filterUnits"
                    | "in"
                    | "in2"
                    | "intercept"
                    | "k1"
                    | "k2"
                    | "k3"
                    | "k4"
                    | "kernelMatrix"
                    | "limitingConeAngle"
                    | "mode"
                    | "numOctaves"
                    | "operator"
                    | "order"
                    | "preserveAlpha"
                    | "primitiveUnits"
                    | "result"
                    | "seed"
                    | "slope"
                    | "stdDeviation"
                    | "stitchTiles"
                    | "surfaceScale"
                    | "tableValues"
                    | "targetX"
                    | "targetY"
                    | "values"
            )
    }

    /// @ref LLP 1075.003 §3.3 — every `data-` name this host writes on an
    /// HTML element for a prop of its own is a word Contract refuses, so an
    /// app's `data-*` never lands on one.
    #[test]
    fn every_data_name_the_host_writes_is_a_reserved_word() {
        let kinds = [
            NodeType::View,
            NodeType::Text,
            NodeType::Pressable,
            NodeType::TextInput,
            NodeType::Image,
            NodeType::NativeView,
            NodeType::ScrollView,
            NodeType::List,
            NodeType::Video,
            NodeType::WebView,
            NodeType::Canvas,
            NodeType::Svg,
            NodeType::Control,
        ];
        for prop in PropId::ALL {
            if prop == PropId::Dataset || svg_only(prop) {
                continue;
            }
            let value = match prop.kind() {
                PropKind::Str => PropValue::Str("x".into()),
                PropKind::Bool => PropValue::Bool(true),
                PropKind::Int => PropValue::Int(1),
                PropKind::Float => PropValue::Float(1.0),
            };
            for kind in kinds {
                for name in written(kind, prop, value.clone()).keys() {
                    if let Some(word) = name.strip_prefix("data-") {
                        assert!(
                            contract_lower::dataset::reserved(word),
                            "`{name}` (from {}) is not reserved in contract/lower/src/dataset.rs",
                            prop.name()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_dataset_is_one_attribute_per_word() {
        crate::link::link(crate::Linked {
            dataset: Some(crate::document::dataset),
            ..crate::link::linked()
        });
        let out = written(
            NodeType::View,
            PropId::Dataset,
            PropValue::Str(r#"{"large-title":"Inbox","trailing":"compose"}"#.into()),
        );
        assert_eq!(
            out.get("data-large-title").map(String::as_str),
            Some("Inbox")
        );
        assert_eq!(
            out.get("data-trailing").map(String::as_str),
            Some("compose")
        );
        assert!(out.get("data-dataset").is_none());
        assert_eq!(
            crate::document::dataset(r#"{"a":"q\"\\\t\u0001","b":""}"#),
            vec![
                ("a".into(), "q\"\\\t\u{1}".into()),
                ("b".into(), String::new())
            ]
        );
    }
}
