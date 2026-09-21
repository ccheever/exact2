//! The tag and attribute table: Contract's view vocabulary onto the kernel's.
//!
//! @ref LLP 1004 D2 (kernel ordinals from `exact-kernel`, never redeclared)
//! @ref `rules/RULES.md` §Scope (the web is the standard: every row here is a
//! CSS property or an HTML attribute by its CSS/HTML name)
//! @ref LLP 1017 §8.1 (the literal CSS names, hyphens as grammar, no aliases;
//! `testId` and explicitly declared host-policy props aside)
//!
//! A tag names a kernel node type plus fixed rows (`column` is a `View` with
//! `flex-direction: column`); an attribute names one or more kernel style rows
//! or one prop, or is a handler. Anything not in the table is a rejection
//! with a stable id — there is no fallback attribute, and an old spelling
//! (`size`, `fontSize`, `radius`, `label`) is refused with the CSS name it
//! became.

use exact_kernel::{NodeType, PropId, StyleId};

/// What an attribute lowers to.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrTarget {
    /// One or more style rows that all take the attribute's value.
    Styles(Vec<StyleId>),
    /// One prop.
    Prop(PropId),
    /// A boolean prop with the inverse of the authored value.
    InvertedBoolProp(PropId),
    /// A handler for the named event.
    Handler(&'static str),
    /// CSS `flex: <n>` — grow, shrink, and basis together.
    Flex,
    /// A canvas's surface binding: `surface=name(args)` (LLP 1009 D3).
    Surface,
}

/// A tag's node type, its fixed rows, and how positional arguments land.
#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    /// The kernel node type.
    pub node_type: NodeType,
    /// Rows every instance of this tag sets, as (style row, enum value name).
    pub fixed_styles: Vec<(StyleId, &'static str)>,
    /// Props every instance sets, as (prop, text).
    pub fixed_props: Vec<(PropId, &'static str)>,
    /// The prop the first positional argument fills, if any.
    pub positional: Option<PropId>,
}

fn s(name: &str) -> StyleId {
    StyleId::from_name(name).unwrap_or_else(|| panic!("kernel schema has no style row `{name}`"))
}

fn p(name: &str) -> PropId {
    PropId::from_name(name).unwrap_or_else(|| panic!("kernel schema has no prop `{name}`"))
}

/// Look up a tag.
pub fn tag(name: &str) -> Option<Tag> {
    let view = |fixed_styles: Vec<(StyleId, &'static str)>,
                fixed_props: Vec<(PropId, &'static str)>| Tag {
        node_type: NodeType::View,
        fixed_styles,
        fixed_props,
        positional: None,
    };
    let semantic = |name| view(vec![], vec![(p("semanticTag"), name)]);
    Some(match name {
        "view" | "box" => view(vec![], vec![]),
        "column" => view(
            vec![(s("display"), "flex"), (s("flex_direction"), "column")],
            vec![],
        ),
        "row" => view(
            vec![(s("display"), "flex"), (s("flex_direction"), "row")],
            vec![],
        ),
        "dialog" => view(
            vec![(s("position_type"), "absolute")],
            vec![(p("semanticTag"), "dialog")],
        ),
        "main" => semantic("main"),
        "header" => semantic("header"),
        "nav" => semantic("nav"),
        "section" => semantic("section"),
        "footer" => semantic("footer"),
        "article" => semantic("article"),
        "aside" => semantic("aside"),
        "list" => Tag {
            node_type: NodeType::List,
            fixed_styles: vec![],
            fixed_props: vec![(p("accessibilityRole"), "list")],
            positional: None,
        },
        "scroll" => Tag {
            node_type: NodeType::ScrollView,
            fixed_styles: vec![],
            fixed_props: vec![],
            positional: None,
        },
        "text" => Tag {
            node_type: NodeType::Text,
            fixed_styles: vec![],
            fixed_props: vec![],
            positional: Some(p("text")),
        },
        "button" => Tag {
            node_type: NodeType::Pressable,
            fixed_styles: vec![],
            fixed_props: vec![(p("accessibilityRole"), "button")],
            positional: None,
        },
        "link" => Tag {
            node_type: NodeType::Pressable,
            fixed_styles: vec![],
            fixed_props: vec![(p("accessibilityRole"), "link")],
            positional: None,
        },
        "textarea" => Tag {
            node_type: NodeType::TextInput,
            fixed_styles: vec![
                (s("white_space"), "pre-wrap"),
                (s("overflow_wrap"), "break-word"),
            ],
            fixed_props: vec![(p("semanticTag"), "textarea")],
            positional: None,
        },
        "input" => Tag {
            node_type: NodeType::TextInput,
            fixed_styles: vec![],
            fixed_props: vec![],
            positional: None,
        },
        // A bare <canvas> is 300×150 on the web; so is a bare `canvas` here.
        "canvas" => Tag {
            node_type: NodeType::Canvas,
            fixed_styles: vec![(s("width"), "300"), (s("height"), "150")],
            fixed_props: vec![],
            positional: None,
        },
        // @ref LLP 1020 D1 — a bare <iframe> is a 300×150 replaced element.
        "iframe" => Tag {
            node_type: NodeType::WebView,
            fixed_styles: vec![(s("width"), "300"), (s("height"), "150")],
            fixed_props: vec![],
            positional: Some(p("src")),
        },
        "video" => Tag {
            node_type: NodeType::Video,
            fixed_styles: vec![(s("object_fit"), "contain")],
            fixed_props: vec![],
            positional: Some(p("src")),
        },
        "image" => Tag {
            node_type: NodeType::Image,
            fixed_styles: vec![],
            fixed_props: vec![],
            positional: Some(p("imageSource")),
        },
        _ => return None,
    })
}

/// What a prop attribute's value must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropTy {
    /// Text.
    Str,
    /// `true`/`false`.
    Bool,
    /// A whole number.
    Int,
    /// A finite number, including a fractional pixel.
    Float,
}

/// The type a prop attribute takes, by the kernel prop's name.
pub fn prop_ty(prop: PropId) -> PropTy {
    match prop.kind() {
        exact_kernel::PropKind::Bool => PropTy::Bool,
        exact_kernel::PropKind::Int => PropTy::Int,
        exact_kernel::PropKind::Float => PropTy::Float,
        exact_kernel::PropKind::Str => PropTy::Str,
    }
}

/// Look up an attribute.
pub fn attr(name: &str) -> Option<AttrTarget> {
    let styles = |names: &[&str]| AttrTarget::Styles(names.iter().map(|n| s(n)).collect());
    Some(match name {
        // handlers (the web's events, LLP 1005 §3)
        "loadedmetadata" => AttrTarget::Handler("loadedmetadata"),
        "durationchange" => AttrTarget::Handler("durationchange"),
        "timeupdate" => AttrTarget::Handler("timeupdate"),
        "play" => AttrTarget::Handler("play"),
        "playing" => AttrTarget::Handler("playing"),
        "pause" => AttrTarget::Handler("pause"),
        "ended" => AttrTarget::Handler("ended"),
        "waiting" => AttrTarget::Handler("waiting"),
        "seeking" => AttrTarget::Handler("seeking"),
        "seeked" => AttrTarget::Handler("seeked"),
        "ratechange" => AttrTarget::Handler("ratechange"),
        "volumechange" => AttrTarget::Handler("volumechange"),
        "error" => AttrTarget::Handler("error"),
        "canplay" => AttrTarget::Handler("canplay"),
        "press" => AttrTarget::Handler("press"),
        "change" => AttrTarget::Handler("change"),
        "hover" => AttrTarget::Handler("hover"),
        "focus" => AttrTarget::Handler("focus"),
        "blur" => AttrTarget::Handler("blur"),
        "key" => AttrTarget::Handler("key"),
        "submit" => AttrTarget::Handler("submit"),
        "load" => AttrTarget::Handler("load"),
        "message" => AttrTarget::Handler("message"),
        "contextmenu" => AttrTarget::Handler("contextmenu"),
        "dblclick" => AttrTarget::Handler("dblclick"),
        "reachstart" => AttrTarget::Handler("reachstart"),
        "reachend" => AttrTarget::Handler("reachend"),
        "swiperight" => AttrTarget::Handler("swiperight"),
        "scroll" => AttrTarget::Handler("scroll"),
        "pan" => AttrTarget::Handler("pan"),
        "navigate" => AttrTarget::Handler("navigate"),
        "heightrelease" => AttrTarget::Handler("heightrelease"),
        "transformgeometry" => AttrTarget::Handler("transformgeometry"),
        "transformrelease" => AttrTarget::Handler("transformrelease"),
        "reorderdrop" => AttrTarget::Handler("reorderdrop"),
        "reorderFor" => AttrTarget::Prop(p("reorderFor")),
        "transformDragFor" => AttrTarget::Prop(p("transformDragFor")),
        "heightDragFor" => AttrTarget::Prop(p("heightDragFor")),
        // the canvas's surface (LLP 1009 D3)
        "surface" => AttrTarget::Surface,
        // props (HTML and ARIA attribute names; `testId` is Exact's)
        "poster" => AttrTarget::Prop(p("poster")),
        "autoplay" => AttrTarget::Prop(p("autoplay")),
        "controls" => AttrTarget::Prop(p("controls")),
        "loop" => AttrTarget::Prop(p("loop")),
        "muted" => AttrTarget::Prop(p("muted")),
        "preload" => AttrTarget::Prop(p("preload")),
        "playsinline" => AttrTarget::Prop(p("playsinline")),
        "crossorigin" => AttrTarget::Prop(p("crossorigin")),
        "controlslist" => AttrTarget::Prop(p("controlslist")),
        "disablepictureinpicture" => AttrTarget::Prop(p("disablepictureinpicture")),
        "disableremoteplayback" => AttrTarget::Prop(p("disableremoteplayback")),
        "volume" => AttrTarget::Prop(p("volume")),
        "playbackRate" => AttrTarget::Prop(p("playbackRate")),
        "currentTime" => AttrTarget::Prop(p("currentTime")),
        "paused" => AttrTarget::Prop(p("paused")),
        "playbackVisibilityThreshold" => AttrTarget::Prop(p("playbackVisibilityThreshold")),
        "preservesPitch" => AttrTarget::Prop(p("preservesPitch")),
        "allowsPictureInPicturePlayback" => AttrTarget::Prop(p("allowsPictureInPicturePlayback")),
        "canStartPictureInPictureAutomaticallyFromInline" => {
            AttrTarget::Prop(p("canStartPictureInPictureAutomaticallyFromInline"))
        }
        "entersFullScreenWhenPlaybackBegins" => {
            AttrTarget::Prop(p("entersFullScreenWhenPlaybackBegins"))
        }
        "exitsFullScreenWhenPlaybackEnds" => AttrTarget::Prop(p("exitsFullScreenWhenPlaybackEnds")),
        "showsTimecodes" => AttrTarget::Prop(p("showsTimecodes")),
        "allowsVideoFrameAnalysis" => AttrTarget::Prop(p("allowsVideoFrameAnalysis")),
        "requiresLinearPlayback" => AttrTarget::Prop(p("requiresLinearPlayback")),
        "preferredPeakBitRate" => AttrTarget::Prop(p("preferredPeakBitRate")),
        "preferredForwardBufferDuration" => AttrTarget::Prop(p("preferredForwardBufferDuration")),
        "automaticallyWaitsToMinimizeStalling" => {
            AttrTarget::Prop(p("automaticallyWaitsToMinimizeStalling"))
        }
        "preventsDisplaySleepDuringVideoPlayback" => {
            AttrTarget::Prop(p("preventsDisplaySleepDuringVideoPlayback"))
        }
        "virtualized" => AttrTarget::Prop(p("virtualized")),
        "testId" => AttrTarget::Prop(p("testId")),
        "navigationKey" => AttrTarget::Prop(p("navigationKey")),
        "navigationBack" => AttrTarget::Prop(p("navigationBack")),
        "navigationPresentation" => AttrTarget::Prop(p("navigationPresentation")),
        "navigationDetent" => AttrTarget::Prop(p("navigationDetent")),
        "navigationSource" => AttrTarget::Prop(p("navigationSource")),
        "closedby" => AttrTarget::Prop(p("closedby")),
        "contextTarget" => AttrTarget::Prop(p("contextTarget")),
        "contextMagnify" => AttrTarget::Prop(p("contextMagnify")),
        "emojiPicker" => AttrTarget::Prop(p("emojiPicker")),
        "backgroundMaterial" => AttrTarget::Prop(p("backgroundMaterial")),
        "toolbarPlacement" => AttrTarget::Prop(p("toolbarPlacement")),
        "retainFocus" => AttrTarget::Prop(p("retainFocus")),
        "swipeIndicator" => AttrTarget::Prop(p("swipeIndicator")),
        "aria-label" => AttrTarget::Prop(p("accessibilityLabel")),
        "aria-keyshortcuts" => AttrTarget::Prop(p("accessibilityKeyShortcuts")),
        "aria-description" => AttrTarget::Prop(p("accessibilityHint")),
        "aria-level" => AttrTarget::Prop(p("accessibilityHeadingLevel")),
        "role" => AttrTarget::Prop(p("accessibilityRole")),
        "placeholder" => AttrTarget::Prop(p("placeholder")),
        "type" => AttrTarget::Prop(p("type")),
        // HTML's attribute is `inputmode`; the kernel's prop keeps the DOM
        // property's spelling, as the schema does for every prop.
        "inputmode" => AttrTarget::Prop(p("inputMode")),
        "autocapitalize" => AttrTarget::Prop(p("autocapitalize")),
        "autocorrect" => AttrTarget::Prop(p("autocorrect")),
        "spellcheck" => AttrTarget::Prop(p("spellcheck")),
        // The viewport meta's `viewport-fit=cover`, read from the first root
        // (LLP 1008 §9): the layout viewport becomes the whole screen and
        // `env(safe-area-inset-*)` lengths carry the insets.
        "viewport-fit" => AttrTarget::Prop(p("viewportFit")),
        // The viewport meta's `interactive-widget`, read from the first root
        // (LLP 1008 §9): `resizes-content` shrinks the layout viewport to a
        // software keyboard's top, so what is pinned to the bottom rises with
        // it; the default, `resizes-visual`, insets the viewport instead.
        "interactive-widget" => AttrTarget::Prop(p("interactiveWidget")),
        "value" => AttrTarget::Prop(p("value")),
        "item-height" => AttrTarget::Prop(p("itemHeight")),
        "estimated-item-height" => AttrTarget::Prop(p("estimatedItemHeight")),
        "scrollTop" => AttrTarget::Prop(p("scrollTop")),
        "scrollLeft" => AttrTarget::Prop(p("scrollLeft")),
        "swipeContent" => AttrTarget::Prop(p("swipeContent")),
        "swipeLeading" => AttrTarget::Prop(p("swipeLeading")),
        "swipeTrailing" => AttrTarget::Prop(p("swipeTrailing")),
        "destructive" => AttrTarget::Prop(p("destructive")),
        "scrollFollowEnd" => AttrTarget::Prop(p("scrollFollowEnd")),
        "keyboardDismissMode" => AttrTarget::Prop(p("keyboardDismissMode")),
        "href" => AttrTarget::Prop(p("href")),
        "disabled" => AttrTarget::Prop(p("disabled")),
        "inert" => AttrTarget::Prop(p("inert")),
        "readonly" => AttrTarget::InvertedBoolProp(p("editable")),
        "lang" => AttrTarget::Prop(p("lang")),
        "src" => AttrTarget::Prop(p("src")),
        "sandbox" => AttrTarget::Prop(p("sandbox")),
        // The Popover API, by its own names (LLP 1021 D1): a container with
        // `popover` is hidden until its invoker — a `button` whose
        // `popovertarget` names the container's `id` — toggles it; open
        // state is the host's, never the plan's (D2). `aria-checked` is the
        // ARIA state a menu row's dot would hand-draw; a native menu renders
        // it as the platform's checkmark (D3).
        "id" => AttrTarget::Prop(p("id")),
        "popover" => AttrTarget::Prop(p("popover")),
        "popovertarget" => AttrTarget::Prop(p("popovertarget")),
        "popovertargetaction" => AttrTarget::Prop(p("popovertargetaction")),
        "commandfor" => AttrTarget::Prop(p("commandfor")),
        "command" => AttrTarget::Prop(p("command")),
        "aria-checked" => AttrTarget::Prop(p("accessibilityChecked")),
        // @ref LLP 1039 D6 — vertical tablists retain authored layout.
        "aria-orientation" => AttrTarget::Prop(p("accessibilityOrientation")),
        "aria-selected" => AttrTarget::Prop(p("accessibilitySelected")),
        // style rows, by their CSS property names
        "white-space" => styles(&["white_space"]),
        "overflow-wrap" => styles(&["overflow_wrap"]),
        "field-sizing" => styles(&["field_sizing"]),
        "scroll-snap-type" => styles(&["scroll_snap_type"]),
        "scrollbar-width" => styles(&["scrollbar_width"]),
        "touch-action" => styles(&["touch_action"]),
        "clip-path" => styles(&["clip_path"]),
        // @ref LLP 1043.000 §3 D1
        "wrap-flow" => styles(&["wrap_flow"]),
        "shape-outside" => styles(&["shape_outside"]),
        "shape-margin" => styles(&["shape_margin"]),
        "scroll-snap-align" => styles(&["scroll_snap_align"]),
        "line-clamp" => styles(&["line_clamp"]),
        "text-overflow" => styles(&["text_overflow"]),
        "text-decoration-line" => styles(&["text_decoration_line"]),
        "font-size" => styles(&["font_size"]),
        "font-weight" => styles(&["font_weight"]),
        "font-style" => styles(&["font_style"]),
        "font-family" => styles(&["font_family"]),
        "color" => styles(&["text_color"]),
        "background-color" => styles(&["background_color"]),
        "caret-color" => styles(&["caret_color"]),
        "tint-color" => styles(&["tint_color"]),
        "opacity" => styles(&["opacity"]),
        "letter-spacing" => styles(&["letter_spacing"]),
        "line-height" => styles(&["line_height"]),
        "text-align" => styles(&["text_align"]),
        "gap" => styles(&["row_gap", "column_gap"]),
        "row-gap" => styles(&["row_gap"]),
        "column-gap" => styles(&["column_gap"]),
        "padding" => styles(&[
            "padding_top",
            "padding_right",
            "padding_bottom",
            "padding_left",
        ]),
        "padding-top" => styles(&["padding_top"]),
        "padding-right" => styles(&["padding_right"]),
        "padding-bottom" => styles(&["padding_bottom"]),
        "padding-left" => styles(&["padding_left"]),
        "margin" => styles(&["margin_top", "margin_right", "margin_bottom", "margin_left"]),
        "margin-top" => styles(&["margin_top"]),
        "margin-right" => styles(&["margin_right"]),
        "margin-bottom" => styles(&["margin_bottom"]),
        "margin-left" => styles(&["margin_left"]),
        "border-radius" => styles(&[
            "border_radius_top_left",
            "border_radius_top_right",
            "border_radius_bottom_right",
            "border_radius_bottom_left",
        ]),
        "border-top-left-radius" => styles(&["border_radius_top_left"]),
        "border-top-right-radius" => styles(&["border_radius_top_right"]),
        "border-bottom-left-radius" => styles(&["border_radius_bottom_left"]),
        "border-bottom-right-radius" => styles(&["border_radius_bottom_right"]),
        "border-width" => styles(&[
            "border_width_top",
            "border_width_right",
            "border_width_bottom",
            "border_width_left",
        ]),
        "border-top-width" => styles(&["border_width_top"]),
        "border-right-width" => styles(&["border_width_right"]),
        "border-bottom-width" => styles(&["border_width_bottom"]),
        "border-left-width" => styles(&["border_width_left"]),
        "border-style" => styles(&[
            "border_style_top",
            "border_style_right",
            "border_style_bottom",
            "border_style_left",
        ]),
        "border-top-style" => styles(&["border_style_top"]),
        "border-right-style" => styles(&["border_style_right"]),
        "border-bottom-style" => styles(&["border_style_bottom"]),
        "border-left-style" => styles(&["border_style_left"]),
        "border-color" => styles(&[
            "border_color_top",
            "border_color_right",
            "border_color_bottom",
            "border_color_left",
        ]),
        "width" => styles(&["width"]),
        "height" => styles(&["height"]),
        "min-width" => styles(&["min_width"]),
        "min-height" => styles(&["min_height"]),
        "max-width" => styles(&["max_width"]),
        "max-height" => styles(&["max_height"]),
        "flex" => AttrTarget::Flex,
        "flex-shrink" => styles(&["flex_shrink"]),
        "flex-basis" => styles(&["flex_basis"]),
        "flex-wrap" => styles(&["flex_wrap"]),
        "flex-direction" => styles(&["flex_direction"]),
        "display" => styles(&["display"]),
        "align-items" => styles(&["align_items"]),
        "align-self" => styles(&["align_self"]),
        "box-sizing" => styles(&["box_sizing"]),
        "object-fit" => styles(&["object_fit"]),
        "justify-content" => styles(&["justify_content"]),
        "position" => styles(&["position_type"]),
        "top" => styles(&["top"]),
        "left" => styles(&["left"]),
        "right" => styles(&["right"]),
        "bottom" => styles(&["bottom"]),
        "overflow" => styles(&["overflow_x", "overflow_y"]),
        "overflow-x" => styles(&["overflow_x"]),
        "overflow-y" => styles(&["overflow_y"]),
        "overscroll-behavior" => styles(&["overscroll_behavior_x", "overscroll_behavior_y"]),
        "overscroll-behavior-x" => styles(&["overscroll_behavior_x"]),
        "overscroll-behavior-y" => styles(&["overscroll_behavior_y"]),
        "z-index" => styles(&["z_index"]),
        "transition" => styles(&["transition"]),
        "interpolate-size" => styles(&["interpolate_size"]),
        "translate" => styles(&["translate"]),
        "scale" => styles(&["scale"]),
        "rotate" => styles(&["rotate"]),
        _ => return None,
    })
}

/// The name an old spelling became — the short nicknames and the DOM's
/// camelCase that the table accepted before LLP 1017 §8.1 — so the refusal of
/// `size=13` says `font-size`. Nothing here is accepted; it is only named.
pub fn renamed(old: &str) -> Option<&'static str> {
    Some(match old {
        "size" | "fontSize" => "font-size",
        "weight" | "fontWeight" => "font-weight",
        "fontStyle" => "font-style",
        "fontFamily" => "font-family",
        "background" | "backgroundColor" => "background-color",
        "letterSpacing" => "letter-spacing",
        "lineHeight" => "line-height",
        "textAlign" => "text-align",
        "rowGap" => "row-gap",
        "columnGap" => "column-gap",
        "paddingTop" => "padding-top",
        "paddingRight" => "padding-right",
        "paddingBottom" => "padding-bottom",
        "paddingLeft" => "padding-left",
        "marginTop" => "margin-top",
        "marginRight" => "margin-right",
        "marginBottom" => "margin-bottom",
        "marginLeft" => "margin-left",
        "radius" | "borderRadius" => "border-radius",
        "borderWidth" => "border-width",
        "borderTopWidth" => "border-top-width",
        "borderRightWidth" => "border-right-width",
        "borderBottomWidth" => "border-bottom-width",
        "borderLeftWidth" => "border-left-width",
        "borderColor" => "border-color",
        "minWidth" => "min-width",
        "minHeight" => "min-height",
        "maxWidth" => "max-width",
        "maxHeight" => "max-height",
        "flexShrink" => "flex-shrink",
        "flexBasis" => "flex-basis",
        "wrap" | "flexWrap" => "flex-wrap",
        "direction" | "flexDirection" => "flex-direction",
        "align" | "alignItems" => "align-items",
        "alignSelf" => "align-self",
        "boxSizing" => "box-sizing",
        "fit" | "objectFit" => "object-fit",
        "justify" | "justifyContent" => "justify-content",
        "overflowX" => "overflow-x",
        "overflowY" => "overflow-y",
        "zIndex" => "z-index",
        "accessibilityOrientation" => "aria-orientation",
        "label" | "accessibilityLabel" => "aria-label",
        "hint" | "accessibilityHint" => "aria-description",
        "headingLevel" => "aria-level",
        "inputMode" | "keyboardType" => "inputmode",
        "viewportFit" | "safeArea" | "safeAreaView" => "viewport-fit",
        "interactiveWidget" | "keyboardAvoidingView" | "keyboardAvoiding" => "interactive-widget",
        "secureTextEntry" => "type",
        "onClick" | "onPress" => "press",
        "onChange" | "onChangeText" => "change",
        "className" | "class" | "style" => return None,
        _ => return None,
    })
}

// A list's fixed or measured row template and scrollport are explicit host policy.
pub(crate) fn validate_list(
    tag: &str,
    expanded: &[contract_syntax::Attr],
    children: &[contract_syntax::Node],
    span: contract_syntax::Span,
) -> Result<(), super::LowerError> {
    use contract_syntax::{Expr, Node};
    if tag == "list" {
        let heights: Vec<_> = expanded
            .iter()
            .filter(|a| matches!(a.name.as_str(), "item-height" | "estimated-item-height"))
            .collect();
        if heights.is_empty() {
            return Ok(());
        }
        if expanded
            .iter()
            .any(|a| a.name == "virtualized" && matches!(a.value, Expr::Bool(true, _)))
        {
            if heights.len() == 1
                && heights[0].name == "estimated-item-height"
                && matches!(heights[0].value, Expr::Number(n, _) if n.is_finite() && n > 0.0)
            {
                // Shared collection template, viewport and flow checks follow
                // in check_collection, including their specific diagnostics.
                return Ok(());
            }
            return super::err(
                "lower-list-height",
                "virtualized lists accept one positive literal `estimated-item-height`, not a fixed row height",
                span,
            );
        }
        let height = (heights.len() == 1).then(|| heights[0]);
        if !height
            .is_some_and(|a| matches!(a.value, Expr::Number(n, _) if n.is_finite() && n > 0.0))
        {
            return super::err(
                "lower-list-height",
                "`list` needs exactly one positive literal `item-height` or `estimated-item-height` in CSS pixels",
                span,
            );
        }
        if children.len() != 1 || !matches!(&children[0], Node::Each { .. }) {
            return super::err(
                "lower-list-rows",
                "`list` contains exactly one direct keyed `each`",
                span,
            );
        }
    } else {
        let heights: Vec<_> = expanded
            .iter()
            .filter(|a| matches!(a.name.as_str(), "item-height" | "estimated-item-height"))
            .collect();
        if !heights.is_empty() {
            return super::err(
                "lower-list-height",
                "row height hints belong on `list`",
                span,
            );
        }
    }
    if tag == "list"
        && !expanded
            .iter()
            .any(|a| matches!(a.name.as_str(), "height" | "max-height" | "flex"))
    {
        return super::err(
            "lower-list-viewport",
            "`list` needs a constrained scrollport: height, max-height, or flex",
            span,
        );
    }
    Ok(())
}

/// Suggest one unambiguous single-edit spelling from the existing attribute
/// lookup. No second vocabulary is maintained, and this never admits an alias.
pub(crate) fn similar_attr(name: &str, style_only: bool) -> Option<String> {
    if !name.is_ascii() || !(3..=64).contains(&name.len()) {
        return None;
    }
    let mut found: Option<String> = None;
    let mut consider = |bytes: &[u8]| {
        let candidate = std::str::from_utf8(bytes).expect("ASCII spelling edits");
        if candidate == name {
            return true;
        }
        let admitted = match attr(candidate) {
            Some(AttrTarget::Styles(_) | AttrTarget::Flex) => true,
            Some(_) => !style_only,
            None => false,
        };
        if admitted {
            if found.as_deref().is_some_and(|old| old != candidate) {
                return false;
            }
            found = Some(candidate.to_owned());
        }
        true
    };
    const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
    let mut candidate = name.as_bytes().to_vec();
    for index in 0..name.len() {
        let original = candidate.remove(index);
        if !consider(&candidate) {
            return None;
        }
        candidate.insert(index, original);
        for &letter in LETTERS {
            candidate[index] = letter;
            if !consider(&candidate) {
                return None;
            }
        }
        candidate[index] = original;
        if index + 1 < name.len() {
            candidate.swap(index, index + 1);
            if !consider(&candidate) {
                return None;
            }
            candidate.swap(index, index + 1);
        }
    }
    for index in 0..=name.len() {
        candidate.insert(index, b'a');
        for &letter in LETTERS {
            candidate[index] = letter;
            if !consider(&candidate) {
                return None;
            }
        }
        candidate.remove(index);
    }
    found
}
