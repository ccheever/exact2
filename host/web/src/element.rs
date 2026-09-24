//! The element a node is on the web: its tag, its attributes and the CSS
//! the host adds to the kernel's — one projection, which the live host
//! creates imperatively and [`super::document`] writes as HTML.
//!
//! @ref LLP 1007 §1 (a bare node is a bare `<div>`) / LLP 1048 D1

use exact_kernel::{Kernel, NodeRef, NodeType, PropId, PropValue};
use std::collections::BTreeMap;

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
pub(super) fn tag_for(node: &NodeRef<'_>, in_button: bool) -> &'static str {
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
        NodeType::View | NodeType::List | NodeType::NativeView | NodeType::Svg => "div",
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

/// Props as DOM attributes/properties. Names are the DOM's.
pub(super) fn props_for(node: &NodeRef<'_>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
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
            other => {
                // Every other prop rides as `data-<name>` so nothing is lost.
                out.insert(format!("data-{}", other.name().to_lowercase()), text);
                continue;
            }
        };
        out.insert(name.to_string(), text);
    }
    if heading_level(node).is_some_and(|level| level > 6) {
        out.entry("role".into()).or_insert_with(|| "heading".into());
    }
    if node.node_type.scrolls_by_default() {
        out.insert("data-scroll".into(), "true".into());
    }
    if node.node_type == NodeType::Toggle {
        out.entry("type".into())
            .or_insert_with(|| "checkbox".into());
    }
    // A `<button>` submits a form unless it says otherwise; a `button` never does.
    if element(node) == "button" {
        out.entry("type".into()).or_insert_with(|| "button".into());
    }
    if node.node_type == NodeType::Image {
        if let Some(role) = node
            .props
            .str(PropId::ImageSource)
            .and_then(|s| s.strip_prefix("symbol:"))
        {
            out.insert(
                "data-symbol-path".into(),
                exact_kernel::generated::symbol(role)
                    .map(|s| s.1)
                    .unwrap_or("")
                    .into(),
            );
            out.insert("alt".into(), String::new());
        }
    }
    out
}
