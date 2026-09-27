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
    Styles(&'static [StyleId]),
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
    pub fixed_styles: &'static [(StyleId, &'static str)],
    /// Props every instance sets, as (prop, text).
    pub fixed_props: &'static [(PropId, &'static str)],
    /// The prop the first positional argument fills, if any.
    pub positional: Option<PropId>,
}

fn p(name: &str) -> PropId {
    PropId::from_name(name).unwrap_or_else(|| panic!("kernel schema has no prop `{name}`"))
}

/// Look up a tag.
pub fn tag(name: &str) -> Option<Tag> {
    let view = |fixed_styles: &'static [(StyleId, &'static str)],
                fixed_props: &'static [(PropId, &'static str)]| Tag {
        node_type: NodeType::View,
        fixed_styles,
        fixed_props,
        positional: None,
    };
    Some(match name {
        "view" | "box" => view(&[], &[]),
        "column" => view(
            &[
                (StyleId::Display, "flex"),
                (StyleId::FlexDirection, "column"),
            ],
            &[],
        ),
        "row" => view(
            &[(StyleId::Display, "flex"), (StyleId::FlexDirection, "row")],
            &[],
        ),
        "dialog" => view(
            &[(StyleId::PositionType, "absolute")],
            &[(PropId::SemanticTag, "dialog")],
        ),
        "main" => view(&[], &[(PropId::SemanticTag, "main")]),
        "header" => view(&[], &[(PropId::SemanticTag, "header")]),
        "nav" => view(&[], &[(PropId::SemanticTag, "nav")]),
        "section" => view(&[], &[(PropId::SemanticTag, "section")]),
        "footer" => view(&[], &[(PropId::SemanticTag, "footer")]),
        "article" => view(&[], &[(PropId::SemanticTag, "article")]),
        "aside" => view(&[], &[(PropId::SemanticTag, "aside")]),
        "list" => Tag {
            node_type: NodeType::List,
            fixed_styles: &[],
            fixed_props: &[(PropId::AccessibilityRole, "list")],
            positional: None,
        },
        "scroll" => Tag {
            node_type: NodeType::ScrollView,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        "text" => Tag {
            node_type: NodeType::Text,
            fixed_styles: &[],
            fixed_props: &[],
            positional: Some(PropId::Text),
        },
        // A pressable `column` (Charlie, 2026-09-23: "One native button, flex
        // column"): a block <button> would centre its content in an anonymous
        // box, which a flex one does not, so the web lays it out as the
        // kernel does (LLP 1006 §3, LLP 1007 §1).
        "button" => Tag {
            node_type: NodeType::Pressable,
            fixed_styles: &[
                (StyleId::Display, "flex"),
                (StyleId::FlexDirection, "column"),
            ],
            fixed_props: &[(PropId::AccessibilityRole, "button")],
            positional: None,
        },
        "link" => Tag {
            node_type: NodeType::Pressable,
            fixed_styles: &[],
            fixed_props: &[(PropId::AccessibilityRole, "link")],
            positional: None,
        },
        "textarea" => Tag {
            node_type: NodeType::TextInput,
            fixed_styles: &[
                (StyleId::WhiteSpace, "pre-wrap"),
                (StyleId::OverflowWrap, "break-word"),
            ],
            fixed_props: &[(PropId::SemanticTag, "textarea")],
            positional: None,
        },
        "input" => Tag {
            node_type: NodeType::TextInput,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        // A bare <canvas> is 300×150 on the web; so is a bare `canvas` here.
        "canvas" => Tag {
            node_type: NodeType::Canvas,
            fixed_styles: &[(StyleId::Width, "300"), (StyleId::Height, "150")],
            fixed_props: &[],
            positional: None,
        },
        // @ref LLP 1020 D1 — a bare <iframe> is a 300×150 replaced element.
        "iframe" => Tag {
            node_type: NodeType::WebView,
            fixed_styles: &[(StyleId::Width, "300"), (StyleId::Height, "150")],
            fixed_props: &[],
            positional: Some(PropId::Src),
        },
        "video" => Tag {
            node_type: NodeType::Video,
            fixed_styles: &[(StyleId::ObjectFit, "contain")],
            fixed_props: &[],
            positional: Some(PropId::Src),
        },
        "image" => Tag {
            node_type: NodeType::Image,
            fixed_styles: &[],
            fixed_props: &[],
            positional: Some(PropId::ImageSource),
        },
        // @ref LLP 1065 D1 — one vector path in a box: `<svg viewBox><path d>`.
        "path" => Tag {
            node_type: NodeType::Path,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        // @ref LLP 1048.003 D1 — the document's metadata: no space, no children.
        "head" => Tag {
            node_type: NodeType::Head,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
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

/// Whether an attribute sets style rows.
pub fn style(name: &str) -> bool {
    matches!(attr(name), Some(AttrTarget::Styles(_)))
}

/// Look up an attribute.
pub fn attr(name: &str) -> Option<AttrTarget> {
    let styles = |rows: &'static [StyleId]| AttrTarget::Styles(rows);
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
        "select" => AttrTarget::Handler("select"),
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
        "refresh" => AttrTarget::Handler("refresh"),
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
        // `head`'s fields (LLP 1048.003 D1); they belong to `head` alone.
        "title" => AttrTarget::Prop(p("headTitle")),
        "description" => AttrTarget::Prop(p("headDescription")),
        "image" => AttrTarget::Prop(p("headImage")),
        "canonical" => AttrTarget::Prop(p("headCanonical")),
        "robots" => AttrTarget::Prop(p("headRobots")),
        "status" => AttrTarget::Prop(p("headStatus")),
        // `scroll document=(expr)`: the page's scroller when the expression
        // holds (LLP 1048.003 D4); bare `scroll document` is `document=true`.
        "document" => AttrTarget::Prop(p("scrollDocument")),
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
        "aria-live" => AttrTarget::Prop(p("accessibilityLive")),
        "autofocus" => AttrTarget::Prop(p("autofocus")),
        "action" => AttrTarget::Prop(p("action")),
        "aria-label" => AttrTarget::Prop(p("accessibilityLabel")),
        "aria-keyshortcuts" => AttrTarget::Prop(p("accessibilityKeyShortcuts")),
        "aria-description" => AttrTarget::Prop(p("accessibilityHint")),
        "aria-level" => AttrTarget::Prop(p("accessibilityHeadingLevel")),
        "role" => AttrTarget::Prop(p("accessibilityRole")),
        // `markup="markdown"` on a `text` or `textarea`: the host styles the
        // node's own string (LLP 1045 D3). Not CSS; there is none for this.
        "markup" => AttrTarget::Prop(p("markup")),
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
        "refreshing" => AttrTarget::Prop(p("refreshing")),
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
        "aria-expanded" => AttrTarget::Prop(p("accessibilityExpanded")),
        "aria-hidden" => AttrTarget::Prop(p("accessibilityElementsHidden")),
        // @ref LLP 1065 — a `path`'s data and coordinate system, SVG's names.
        "d" => AttrTarget::Prop(p("pathData")),
        "viewBox" => AttrTarget::Prop(p("viewBox")),
        "preserveAspectRatio" => AttrTarget::Prop(p("preserveAspectRatio")),
        // style rows, by their CSS property names
        // @ref LLP 1065 D3/D4 — SVG's painting properties, and the stroke's
        // visible fraction of the path's length.
        "fill" => styles(&[StyleId::Fill]),
        "stroke" => styles(&[StyleId::Stroke]),
        "stroke-width" => styles(&[StyleId::StrokeWidth]),
        "stroke-linecap" => styles(&[StyleId::StrokeLinecap]),
        "stroke-linejoin" => styles(&[StyleId::StrokeLinejoin]),
        "stroke-miterlimit" => styles(&[StyleId::StrokeMiterlimit]),
        "stroke-dasharray" => styles(&[StyleId::StrokeDasharray]),
        "stroke-dashoffset" => styles(&[StyleId::StrokeDashoffset]),
        "fill-rule" => styles(&[StyleId::FillRule]),
        "vector-effect" => styles(&[StyleId::VectorEffect]),
        "stroke-start" => styles(&[StyleId::StrokeStart]),
        "stroke-end" => styles(&[StyleId::StrokeEnd]),
        "white-space" => styles(&[StyleId::WhiteSpace]),
        "overflow-wrap" => styles(&[StyleId::OverflowWrap]),
        "field-sizing" => styles(&[StyleId::FieldSizing]),
        "scroll-snap-type" => styles(&[StyleId::ScrollSnapType]),
        "scrollbar-width" => styles(&[StyleId::ScrollbarWidth]),
        "touch-action" => styles(&[StyleId::TouchAction]),
        "clip-path" => styles(&[StyleId::ClipPath]),
        // @ref LLP 1043.000 §3 D1
        "wrap-flow" => styles(&[StyleId::WrapFlow]),
        "shape-outside" => styles(&[StyleId::ShapeOutside]),
        "shape-margin" => styles(&[StyleId::ShapeMargin]),
        "scroll-snap-align" => styles(&[StyleId::ScrollSnapAlign]),
        "line-clamp" => styles(&[StyleId::LineClamp]),
        "text-overflow" => styles(&[StyleId::TextOverflow]),
        // @ref LLP 1053 §0 G4 — `normal` and `tabular-nums`; others refused by name.
        "font-variant-numeric" => styles(&[StyleId::FontVariantNumeric]),
        "text-decoration-line" => styles(&[StyleId::TextDecorationLine]),
        // @ref LLP 1055 D5
        "text-transform" => styles(&[StyleId::TextTransform]),
        "font-size" => styles(&[StyleId::FontSize]),
        "font-weight" => styles(&[StyleId::FontWeight]),
        "font-style" => styles(&[StyleId::FontStyle]),
        "font-family" => styles(&[StyleId::FontFamily]),
        "color" => styles(&[StyleId::TextColor]),
        "background-color" => styles(&[StyleId::BackgroundColor]),
        // @ref LLP 1056 — `none` or one linear/radial gradient.
        "background-image" => styles(&[StyleId::BackgroundImage]),
        "caret-color" => styles(&[StyleId::CaretColor]),
        "tint-color" => styles(&[StyleId::TintColor]),
        "opacity" => styles(&[StyleId::Opacity]),
        // @ref LLP 1055 D1 — one value, each row takes its part of the parse.
        "box-shadow" => styles(&[
            StyleId::ShadowColor,
            StyleId::ShadowOffset,
            StyleId::ShadowRadius,
            StyleId::ShadowOpacity,
        ]),
        "letter-spacing" => styles(&[StyleId::LetterSpacing]),
        "line-height" => styles(&[StyleId::LineHeight]),
        "text-align" => styles(&[StyleId::TextAlign]),
        "gap" => styles(&[StyleId::RowGap, StyleId::ColumnGap]),
        "row-gap" => styles(&[StyleId::RowGap]),
        "column-gap" => styles(&[StyleId::ColumnGap]),
        "padding" => styles(&[
            StyleId::PaddingTop,
            StyleId::PaddingRight,
            StyleId::PaddingBottom,
            StyleId::PaddingLeft,
        ]),
        "padding-top" => styles(&[StyleId::PaddingTop]),
        "padding-right" => styles(&[StyleId::PaddingRight]),
        "padding-bottom" => styles(&[StyleId::PaddingBottom]),
        "padding-left" => styles(&[StyleId::PaddingLeft]),
        "margin" => styles(&[
            StyleId::MarginTop,
            StyleId::MarginRight,
            StyleId::MarginBottom,
            StyleId::MarginLeft,
        ]),
        "margin-top" => styles(&[StyleId::MarginTop]),
        "margin-right" => styles(&[StyleId::MarginRight]),
        "margin-bottom" => styles(&[StyleId::MarginBottom]),
        "margin-left" => styles(&[StyleId::MarginLeft]),
        "border-radius" => styles(&[
            StyleId::BorderRadiusTopLeft,
            StyleId::BorderRadiusTopRight,
            StyleId::BorderRadiusBottomRight,
            StyleId::BorderRadiusBottomLeft,
        ]),
        "border-top-left-radius" => styles(&[StyleId::BorderRadiusTopLeft]),
        "border-top-right-radius" => styles(&[StyleId::BorderRadiusTopRight]),
        "border-bottom-left-radius" => styles(&[StyleId::BorderRadiusBottomLeft]),
        "border-bottom-right-radius" => styles(&[StyleId::BorderRadiusBottomRight]),
        "border-width" => styles(&[
            StyleId::BorderWidthTop,
            StyleId::BorderWidthRight,
            StyleId::BorderWidthBottom,
            StyleId::BorderWidthLeft,
        ]),
        "border-top-width" => styles(&[StyleId::BorderWidthTop]),
        "border-right-width" => styles(&[StyleId::BorderWidthRight]),
        "border-bottom-width" => styles(&[StyleId::BorderWidthBottom]),
        "border-left-width" => styles(&[StyleId::BorderWidthLeft]),
        "border-style" => styles(&[
            StyleId::BorderStyleTop,
            StyleId::BorderStyleRight,
            StyleId::BorderStyleBottom,
            StyleId::BorderStyleLeft,
        ]),
        "border-top-style" => styles(&[StyleId::BorderStyleTop]),
        "border-right-style" => styles(&[StyleId::BorderStyleRight]),
        "border-bottom-style" => styles(&[StyleId::BorderStyleBottom]),
        "border-left-style" => styles(&[StyleId::BorderStyleLeft]),
        "border-color" => styles(&[
            StyleId::BorderColorTop,
            StyleId::BorderColorRight,
            StyleId::BorderColorBottom,
            StyleId::BorderColorLeft,
        ]),
        "border-top-color" => styles(&[StyleId::BorderColorTop]),
        "border-right-color" => styles(&[StyleId::BorderColorRight]),
        "border-bottom-color" => styles(&[StyleId::BorderColorBottom]),
        "border-left-color" => styles(&[StyleId::BorderColorLeft]),
        "width" => styles(&[StyleId::Width]),
        "height" => styles(&[StyleId::Height]),
        "min-width" => styles(&[StyleId::MinWidth]),
        "min-height" => styles(&[StyleId::MinHeight]),
        "max-width" => styles(&[StyleId::MaxWidth]),
        "max-height" => styles(&[StyleId::MaxHeight]),
        "flex" => AttrTarget::Flex,
        // @ref LLP 1053 G3 — the longhand: `flex-basis` stays `auto`, unlike `flex`.
        "flex-grow" => styles(&[StyleId::FlexGrow]),
        "flex-shrink" => styles(&[StyleId::FlexShrink]),
        "flex-basis" => styles(&[StyleId::FlexBasis]),
        "flex-wrap" => styles(&[StyleId::FlexWrap]),
        "flex-direction" => styles(&[StyleId::FlexDirection]),
        // @ref LLP 1053 — CSS `direction` (inherited), not a flex direction.
        "direction" => styles(&[StyleId::Direction]),
        // @ref LLP 1053 G1 — `auto || <ratio>`.
        "aspect-ratio" => styles(&[StyleId::AspectRatio]),
        "display" => styles(&[StyleId::Display]),
        "align-items" => styles(&[StyleId::AlignItems]),
        "align-self" => styles(&[StyleId::AlignSelf]),
        "box-sizing" => styles(&[StyleId::BoxSizing]),
        "object-fit" => styles(&[StyleId::ObjectFit]),
        "justify-content" => styles(&[StyleId::JustifyContent]),
        "position" => styles(&[StyleId::PositionType]),
        "inset" => styles(&[StyleId::Top, StyleId::Right, StyleId::Bottom, StyleId::Left]),
        "top" => styles(&[StyleId::Top]),
        "left" => styles(&[StyleId::Left]),
        "right" => styles(&[StyleId::Right]),
        "bottom" => styles(&[StyleId::Bottom]),
        "overflow" => styles(&[StyleId::OverflowX, StyleId::OverflowY]),
        "overflow-x" => styles(&[StyleId::OverflowX]),
        "overflow-y" => styles(&[StyleId::OverflowY]),
        "overscroll-behavior" => {
            styles(&[StyleId::OverscrollBehaviorX, StyleId::OverscrollBehaviorY])
        }
        "overscroll-behavior-x" => styles(&[StyleId::OverscrollBehaviorX]),
        "overscroll-behavior-y" => styles(&[StyleId::OverscrollBehaviorY]),
        "scroll-behavior" => styles(&[StyleId::ScrollBehavior]),
        "z-index" => styles(&[StyleId::ZIndex]),
        "transition" => styles(&[StyleId::Transition]),
        // @ref LLP 1057 — its keyframes resolve at compile time (`keyframes.rs`).
        "animation" => styles(&[StyleId::Animation]),
        // @ref LLP 1063 — played as the node leaves; resolved like `animation`.
        "exit-animation" => styles(&[StyleId::ExitAnimation]),
        // @ref LLP 1063 — how the laid-out box moves when layout moves it.
        "layout-transition" => styles(&[StyleId::LayoutTransition]),
        "interpolate-size" => styles(&[StyleId::InterpolateSize]),
        "translate" => styles(&[StyleId::Translate]),
        "scale" => styles(&[StyleId::Scale]),
        "rotate" => styles(&[StyleId::Rotate]),
        // @ref LLP 1061 D1 — host-owned press feedback; not a motion target.
        "press-scale" => styles(&[StyleId::PressScale]),
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
        "borderTopColor" => "border-top-color",
        "borderRightColor" => "border-right-color",
        "borderBottomColor" => "border-bottom-color",
        "borderLeftColor" => "border-left-color",
        "minWidth" => "min-width",
        "minHeight" => "min-height",
        "maxWidth" => "max-width",
        "maxHeight" => "max-height",
        "flexShrink" => "flex-shrink",
        "flexBasis" => "flex-basis",
        "wrap" | "flexWrap" => "flex-wrap",
        "flexDirection" => "flex-direction",
        "flexGrow" => "flex-grow",
        "aspectRatio" => "aspect-ratio",
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
        if children
            .iter()
            .any(|c| matches!(c, Node::Each { index: Some(_), .. }))
        {
            // A windowed row outlives its position (LLP 1062 D8).
            return super::err(
                "lower-list-rows",
                "a windowed list's rows name no position: `each item in list`, with the position in the item if a row needs it",
                span,
            );
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

/// The attributes only `path` takes (LLP 1065 D1): its data, its coordinate
/// system, and how much of its stroke shows. The painting properties
/// (`fill`, `stroke`, …) inherit, as SVG's do, so any box may set them.
pub const PATH_FIELDS: &[&str] = &[
    "d",
    "viewBox",
    "preserveAspectRatio",
    "stroke-start",
    "stroke-end",
];

/// Refuse a path field off a `path`, and literal path data, a literal
/// `viewBox` or `preserveAspectRatio` a browser would not draw in full, or a
/// literal miter limit SVG calls invalid.
pub(crate) fn check_path_attr(
    tag: &str,
    a: &contract_syntax::Attr,
) -> Result<(), super::LowerError> {
    use contract_syntax::Expr;
    if a.name == "stroke-miterlimit" {
        return match crate::values::numeric_literal(&a.value) {
            Some(n) if n < 1.0 => super::err(
                "lower-attr-value",
                format!("`stroke-miterlimit={n}` is below 1, which SVG refuses"),
                a.span,
            ),
            _ => Ok(()),
        };
    }
    if !PATH_FIELDS.contains(&a.name.as_str()) {
        return Ok(());
    }
    if tag != "path" {
        return super::err(
            "lower-attr-tag",
            format!("`{}` belongs to `path`, not `{tag}`", a.name),
            a.span,
        );
    }
    let Expr::Str(text, _) = &a.value else {
        return Ok(());
    };
    match a.name.as_str() {
        "d" => match exact_kernel::vector::PathData::parse(text).error() {
            Some(e) => super::err(
                "lower-attr-value",
                format!(
                    "`d` is not SVG path data from byte {}: {:?}; a browser draws only what comes before it",
                    e.at,
                    text.get(e.at..).unwrap_or("").chars().take(24).collect::<String>()
                ),
                a.span,
            ),
            None => Ok(()),
        },
        "preserveAspectRatio"
            if exact_kernel::vector::PreserveAspectRatio::parse(text).is_none() =>
        {
            super::err(
                "lower-attr-value",
                format!("`preserveAspectRatio=\"{text}\"` is not `none` or an alignment like `xMidYMid`, then `meet` or `slice`"),
                a.span,
            )
        }
        "viewBox" if exact_kernel::vector::parse_view_box(text).is_none() => super::err(
            "lower-attr-value",
            format!("`viewBox=\"{text}\"` is not `min-x min-y width height` with a positive width and height"),
            a.span,
        ),
        _ => Ok(()),
    }
}

/// The attributes `head` takes, and only `head` (LLP 1048.003 D1).
pub const HEAD_FIELDS: &[&str] = &[
    "title",
    "description",
    "image",
    "canonical",
    "robots",
    "status",
];

/// Suggest one unambiguous single-edit spelling from the existing attribute
/// lookup. No second vocabulary is maintained, and this never admits an alias.
pub(crate) fn similar_attr(name: &str, style_only: bool) -> Option<String> {
    similar(name, |candidate| match attr(candidate) {
        Some(AttrTarget::Styles(_) | AttrTarget::Flex) => true,
        Some(_) => !style_only,
        None => false,
    })
}

/// The same for a tag, from the tag lookup.
pub(crate) fn similar_tag(name: &str) -> Option<String> {
    similar(name, |candidate| tag(candidate).is_some())
}

/// What Contract calls an HTML element it spells differently.
pub(crate) fn html_tag(name: &str) -> Option<&'static str> {
    Some(match name {
        "div" => "a flex container is `column` or `row`, and a plain box `view`",
        "span" | "p" | "label" | "strong" | "em" | "b" | "i" | "h1" | "h2" | "h3" | "h4" | "h5"
        | "h6" => "text is `text`",
        "img" => "an image is `image`",
        "a" => "a link is `link`",
        "title" | "meta" => "a page's title and description are `head title=… description=…`",
        "ul" | "ol" | "li" => "a list is `list` (or a `column` of rows)",
        _ => return None,
    })
}

/// One unambiguous single-edit spelling of `name` that `admitted` accepts,
/// found by trying every edit against the lookup itself.
fn similar(name: &str, admitted: impl Fn(&str) -> bool) -> Option<String> {
    if !name.is_ascii() || !(3..=64).contains(&name.len()) {
        return None;
    }
    let mut found: Option<String> = None;
    let mut consider = |bytes: &[u8]| {
        let candidate = std::str::from_utf8(bytes).expect("ASCII spelling edits");
        if candidate == name {
            return true;
        }
        if admitted(candidate) {
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
