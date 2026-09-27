//! The element a node is on the web: its tag, its attributes and the CSS
//! the host adds to the kernel's — one projection, which the live host
//! creates imperatively and [`super::document`] writes as HTML.
//!
//! @ref LLP 1007 §1 (a bare node is a bare `<div>`) / LLP 1048 D1

use exact_kernel::svg::Paint;
use exact_kernel::SortedMap;
use exact_kernel::{Kernel, NodeRef, NodeType, PropId, PropValue};

/// A canvas's element hosts its surface element under its children
/// (`glue.js`, LLP 1014 D2): a containing block for it, unless the author
/// positioned the canvas, and a stacking context of its own — the
/// `isolation: isolate` the web's `drawable` implies — so the surface paints
/// above the canvas's background and below its children. A container a
/// button holds is a `<span>` ([`tag_for`]) whose box is still a block unless
/// a row says otherwise, as a `<div>`'s is.
pub(super) fn host_css(node: &NodeRef<'_>, mut css: String, tag: &str) -> String {
    if tag == "span" && !node.is_inline_run() && !css.split(';').any(|d| d.starts_with("display:"))
    {
        css.push_str("display:block;");
    }
    if node.node_type == NodeType::Canvas {
        if !(css.starts_with("position:") || css.contains(";position:")) {
            css.push_str("position:relative;");
        }
        css.push_str("isolation:isolate;");
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
    css
}

/// The element for a node: its type, refined by `semanticTag`. A `<button>`
/// holds only phrasing content, so there a container — a box, a paragraph,
/// a heading, a landmark — is a `<span>` with the same style (LLP 1007 §1).
pub(super) fn tag_for<'a>(node: &NodeRef<'a>, in_button: bool) -> &'a str {
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
        if element(&p) == "button" {
            return true;
        }
        parent = p.parent;
    }
    false
}

/// The element for a node wherever it is: its type, refined by `semanticTag`.
fn element(node: &NodeRef<'_>) -> &'static str {
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
        NodeType::Toggle => "input",
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
fn heading_level(node: &NodeRef<'_>) -> Option<i64> {
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
    let as_attributes = attribute_rows(node.node_type);
    let url = |p: &Paint| matches!(p, Paint::Url(..));
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
    if let Some(target) = style
        .clip_path
        .url()
        .and_then(|id| kernel.resolve_id(node.id, id))
    {
        if let Some(c) = exact_kernel::clip::ClipPath::parse(&format!("url(#{})", dom_id(target))) {
            style.clip_path = c;
        }
    }
    // @ref LLP 1055.000 D14 — a filter by the id the page gives it.
    for f in style.filter.0.iter_mut() {
        if let exact_kernel::svg::filter::FilterFn::Url(id) = f {
            if let Some(target) = kernel.resolve_id(node.id, id) {
                *id = dom_id(target).into();
            }
        }
    }
    // @ref LLP 1055.000 D10 — a mask by the id the page gives it.
    if let Some(target) = style
        .svg_mask
        .url()
        .and_then(|id| kernel.resolve_id(node.id, id))
    {
        style.svg_mask = exact_kernel::svg::MarkerRef(Some(dom_id(target).into()));
    }
    // @ref LLP 1055.000 D9 — a marker by the id the page gives it.
    for marker in [
        &mut style.marker_start,
        &mut style.marker_mid,
        &mut style.marker_end,
    ] {
        if let Some(target) = marker.url().and_then(|id| kernel.resolve_id(node.id, id)) {
            *marker = exact_kernel::svg::MarkerRef(Some(dom_id(target).into()));
        }
    }
    // @ref LLP 1055.000 D3 — a paint server by the id the page gives it.
    for paint in [&mut style.fill, &mut style.stroke] {
        if let Paint::Url(id, fallback) = paint {
            if let Some(target) = kernel.resolve_id(node.id, id) {
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
        .and_then(|h| kernel.resolve_id(node.id, h))
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
    let mut out = SortedMap::new();
    if node.style.wrap_flow == exact_kernel::WrapFlow::Both {
        out.insert("data-wrap-flow".into(), "both".into());
    }
    if node.node_type == NodeType::Text && !node.is_inline_run() {
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
        let text = match value {
            PropValue::Str(s) => s.clone(),
            PropValue::Bool(b) => b.to_string(),
            PropValue::Int(i) => i.to_string(),
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
            PropId::RetainFocus => "retainFocus",
            PropId::SwipeIndicator => "swipeIndicator",
            PropId::Href if text.is_empty() => continue,
            PropId::Href => "href",
            PropId::Disabled => "disabled",
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
            PropId::ToggleValue => "checked",
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
    if node.node_type == NodeType::Toggle {
        out.get_or_insert_with("type".into(), || "checkbox".into());
    }
    // A `<button>` submits a form unless it says otherwise; a `button` never does.
    if element(node) == "button" {
        out.get_or_insert_with("type".into(), || "button".into());
    }
    if node.node_type == NodeType::Image {
        if let Some(role) = node
            .props
            .str(PropId::ImageSource)
            .and_then(|s| s.strip_prefix("symbol:"))
        {
            let symbol = exact_kernel::generated::symbol(role);
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
    out
}

#[cfg(test)]
mod name_tests {
    #[test]
    fn prop_names_are_ascii() {
        for prop in exact_kernel::PropId::ALL {
            assert!(prop.name().is_ascii(), "{}", prop.name());
        }
    }
}
