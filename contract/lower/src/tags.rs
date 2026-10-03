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

/// The rows that make a box the containing block of its absolutely
/// positioned descendants on some host, whatever its `position` (LLP 1074
/// T1): a box with one is lowered `position: relative` unless it names a
/// position.
///
/// - `overflow`: a native host clips and scrolls a box's view subtree, so a
///   descendant placed against an ancestor outside it would still be clipped
///   and scrolled by it. CSS does not make a clipping box a containing block;
///   this is the declared deviation.
/// - The transforms, `filter` and `backdrop-filter`: CSS's own rule.
/// - Motion (an animation, a transition, a press scale, a timeline): the
///   browser makes the box a containing block while a transform runs, so it
///   is one at rest too.
pub const CONTAINS_ABSOLUTE: [StyleId; 16] = [
    StyleId::OverflowX,
    StyleId::OverflowY,
    StyleId::Translate,
    StyleId::Scale,
    StyleId::Rotate,
    StyleId::Transform,
    StyleId::Filter,
    StyleId::BackdropBlur,
    StyleId::Animation,
    StyleId::Transition,
    StyleId::LayoutTransition,
    StyleId::ExitAnimation,
    StyleId::PressScale,
    StyleId::DragTimeline,
    StyleId::AnimationTimeline,
    // CSS: `perspective` makes a containing block too (LLP 1077 D8).
    StyleId::Perspective,
];

/// Whether an attribute makes its box a containing block (see
/// [`CONTAINS_ABSOLUTE`]): one of those rows, a material (a backdrop filter),
/// a navigation screen or modal (which the host moves), or a context
/// preview (which the host transforms).
pub fn contains_absolute(name: &str, value: &contract_syntax::Expr) -> bool {
    // A literal that clips nothing and transforms nothing makes no containing block.
    if matches!(value, contract_syntax::Expr::Str(v, _) if v == "visible" || v == "none") {
        return false;
    }
    match attr(name) {
        Some(AttrTarget::Styles(rows)) => rows.iter().any(|row| CONTAINS_ABSOLUTE.contains(row)),
        _ => matches!(
            name,
            "backgroundMaterial" | "navigationKey" | "navigationPresentation" | "contextTarget"
        ),
    }
}

/// A Contract button has one cross-host inner layout: the flex column fixed
/// by [`tag`]. Chrome gives a block/inline `<button>` an anonymous box that
/// centres its contents, which the kernel cannot represent as that display.
pub(crate) fn validate_button_display(
    name: &str,
    attrs: &[contract_syntax::Attr],
) -> Result<(), crate::LowerError> {
    use contract_syntax::Expr;
    if name != "button" {
        return Ok(());
    }
    let Some(display) = attrs.iter().rev().find(|a| a.name == "display") else {
        return Ok(());
    };
    if matches!(&display.value, Expr::Str(v, _) if v == "block" || v.starts_with("inline")) {
        return crate::err(
            "lower-attr-value",
            "a `button` is a flex column on every host; remove `display`, or use `display=\"flex\"`",
            display.span,
        );
    }
    Ok(())
}

/// An element's attributes with `position: relative` added, when it is the
/// containing block of its absolutely positioned descendants on every host
/// and names no position: it has a [`contains_absolute`] attribute, scrolls
/// by its tag, is a canvas (whose surface the page positions) or a Markdown
/// editor (whose line markers it holds). `None` otherwise. An authored
/// `position: static` there is refused.
pub(crate) fn positioned(
    tag: &Tag,
    attrs: &[contract_syntax::Attr],
    in_svg: bool,
    span: contract_syntax::Span,
    host_transform: bool,
    holds_nothing: bool,
) -> Result<Option<Vec<contract_syntax::Attr>>, crate::LowerError> {
    use contract_syntax::Expr;
    let literal =
        |a: &contract_syntax::Attr, v: &str| matches!(&a.value, Expr::Str(s, _) if s == v);
    let contains = !in_svg
        && !tag.node_type.is_svg_element()
        && (host_transform
            || attrs.iter().any(|a| contains_absolute(&a.name, &a.value))
            || tag.node_type.scrolls_by_default()
            || tag.node_type == NodeType::Canvas
            || attrs
                .iter()
                .any(|a| a.name == "markup" && literal(a, "markdown")));
    if !contains {
        return Ok(None);
    }
    match attrs.iter().find(|a| a.name == "position") {
        Some(a) if literal(a, "static") => crate::err(
            "lower-attr-value",
            "`position: static` on a box that clips, scrolls, transforms or animates: such a box is the containing block of its absolutely positioned descendants on every host, so it is `relative`; remove `position`",
            a.span,
        ),
        // A bound position is fine when every value it can take is positioned.
        Some(a) if !always_positioned(&a.value) => crate::err(
            "lower-attr-value",
            "a bound `position` on a box that clips, scrolls, transforms or animates: such a box is the containing block of its absolutely positioned descendants on every host, so every value its position can take must be `relative` or `absolute`",
            a.span,
        ),
        Some(_) => Ok(None),
        None if tag.fixed_styles.iter().any(|(id, _)| *id == StyleId::PositionType) => Ok(None),
        // Only its own rows (it clips, transforms or animates) and nothing
        // absolute can be under it (`Lowerer::may_hold_absolute`): it is the
        // containing block of nothing, and a positioned box costs a host a
        // layer to paint and hit-test (10,000 grid rows' clipping cells: a
        // 25 ms hit test a pointer event).
        None if holds_nothing
            && !host_transform
            && !tag.node_type.scrolls_by_default()
            && tag.node_type != NodeType::Canvas
            && !attrs.iter().any(|a| {
                (a.name == "markup" && literal(a, "markdown"))
                    || matches!(
                        a.name.as_str(),
                        "backgroundMaterial"
                            | "navigationKey"
                            | "navigationPresentation"
                            | "contextTarget"
                            | "z-index"
                            | "top"
                            | "right"
                            | "bottom"
                            | "left"
                    )
            }) =>
        {
            Ok(None)
        }
        None => {
            let mut attrs = attrs.to_vec();
            attrs.push(contract_syntax::Attr {
                name: "position".into(),
                value: Expr::Str("relative".into(), span),
                span,
            });
            Ok(Some(attrs))
        }
    }
}

/// Whether every value a `position` expression can take is `relative` or
/// `absolute`: a literal, or a choice between such expressions. Anything a
/// value could come from at run time (a state, a field, a call) is not.
fn always_positioned(value: &contract_syntax::Expr) -> bool {
    use contract_syntax::Expr;
    match value {
        Expr::Str(v, _) => v == "relative" || v == "absolute",
        Expr::Ternary(_, a, b, _) => always_positioned(a) && always_positioned(b),
        _ => false,
    }
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
    let fe = |fixed_props: &'static [(PropId, &'static str)]| Tag {
        node_type: NodeType::SvgFe,
        fixed_styles: &[],
        fixed_props,
        positional: None,
    };
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
        // @ref LLP 1069.011 D1 — Exact's UA sheet: a button is the author's
        // box (`appearance: none`); `appearance="auto"` asks for the platform's.
        "button" => Tag {
            node_type: NodeType::Pressable,
            fixed_styles: &[
                (StyleId::Display, "flex"),
                (StyleId::FlexDirection, "column"),
                (StyleId::Appearance, "none"),
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
        // A bare <canvas> is 300×150 on the web: its natural size, which the
        // kernel gives it (a replaced element, not authored rows).
        "canvas" => Tag {
            node_type: NodeType::Canvas,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        // @ref LLP 1020 D1 — a bare <iframe> is a 300×150 replaced element:
        // the default object size, which the kernel gives it.
        "iframe" => Tag {
            node_type: NodeType::WebView,
            fixed_styles: &[],
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
        // @ref LLP 1055 D1/D3 — inline SVG: a 300×150 replaced box that
        // clips, as the UA's `svg:not(:root) { overflow: hidden }` does.
        "svg" => Tag {
            node_type: NodeType::Svg,
            fixed_styles: &[
                (StyleId::OverflowX, "hidden"),
                (StyleId::OverflowY, "hidden"),
            ],
            fixed_props: &[],
            positional: None,
        },
        // @ref LLP 1055.000 D7/D8 — definitions and references; a `symbol`
        // clips, as the UA's `symbol { overflow: hidden }` does.
        "symbol" => Tag {
            node_type: NodeType::SvgSymbol,
            fixed_styles: &[
                (StyleId::OverflowX, "hidden"),
                (StyleId::OverflowY, "hidden"),
            ],
            fixed_props: &[],
            positional: None,
        },
        // @ref LLP 1055.000 D9 — a marker clips to its viewport, as the UA's
        // `marker { overflow: hidden }` does.
        "marker" => Tag {
            node_type: NodeType::SvgMarker,
            fixed_styles: &[
                (StyleId::OverflowX, "hidden"),
                (StyleId::OverflowY, "hidden"),
            ],
            fixed_props: &[],
            positional: None,
        },
        // @ref LLP 1055.000 D7/D10 — a mask and a pattern render only where
        // they are referenced; a pattern's tile clips its content, which
        // the hosts do (no row says so).
        "mask" => Tag {
            node_type: NodeType::SvgMask,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        "pattern" => Tag {
            node_type: NodeType::SvgPattern,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        // @ref LLP 1055.000 D13, §8 ruling 5 — HTML inside an `svg`: its
        // children are boxes. The web draws it; native hosts refuse it by
        // name at run time (one plan serves every host).
        "foreignObject" => Tag {
            node_type: NodeType::SvgForeignObject,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        // @ref LLP 1055.000 D14 — a filter and its primitives; a primitive's
        // tag is its `fe` prop.
        "filter" => Tag {
            node_type: NodeType::SvgFilter,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        "feBlend" => fe(&[(PropId::Fe, "feBlend")]),
        "feColorMatrix" => fe(&[(PropId::Fe, "feColorMatrix")]),
        "feComponentTransfer" => fe(&[(PropId::Fe, "feComponentTransfer")]),
        "feComposite" => fe(&[(PropId::Fe, "feComposite")]),
        "feConvolveMatrix" => fe(&[(PropId::Fe, "feConvolveMatrix")]),
        "feDiffuseLighting" => fe(&[(PropId::Fe, "feDiffuseLighting")]),
        "feDisplacementMap" => fe(&[(PropId::Fe, "feDisplacementMap")]),
        "feDropShadow" => fe(&[(PropId::Fe, "feDropShadow")]),
        "feFlood" => fe(&[(PropId::Fe, "feFlood")]),
        "feFuncR" => fe(&[(PropId::Fe, "feFuncR")]),
        "feFuncG" => fe(&[(PropId::Fe, "feFuncG")]),
        "feFuncB" => fe(&[(PropId::Fe, "feFuncB")]),
        "feFuncA" => fe(&[(PropId::Fe, "feFuncA")]),
        "feGaussianBlur" => fe(&[(PropId::Fe, "feGaussianBlur")]),
        "feMerge" => fe(&[(PropId::Fe, "feMerge")]),
        "feMergeNode" => fe(&[(PropId::Fe, "feMergeNode")]),
        "feMorphology" => fe(&[(PropId::Fe, "feMorphology")]),
        "feOffset" => fe(&[(PropId::Fe, "feOffset")]),
        "feSpecularLighting" => fe(&[(PropId::Fe, "feSpecularLighting")]),
        "feTile" => fe(&[(PropId::Fe, "feTile")]),
        "feTurbulence" => fe(&[(PropId::Fe, "feTurbulence")]),
        "feDistantLight" => fe(&[(PropId::Fe, "feDistantLight")]),
        "fePointLight" => fe(&[(PropId::Fe, "fePointLight")]),
        "feSpotLight" => fe(&[(PropId::Fe, "feSpotLight")]),
        // @ref LLP 1055.000 D11 — a run of SVG text, its string positional.
        "tspan" => Tag {
            node_type: NodeType::SvgTSpan,
            fixed_styles: &[],
            fixed_props: &[],
            positional: Some(PropId::Text),
        },
        "defs" | "linearGradient" | "radialGradient" | "stop" | "use" | "clipPath" => Tag {
            node_type: match name {
                "defs" => NodeType::SvgDefs,
                "clipPath" => NodeType::SvgClipPath,
                "linearGradient" => NodeType::SvgLinearGradient,
                "radialGradient" => NodeType::SvgRadialGradient,
                "stop" => NodeType::SvgStop,
                _ => NodeType::SvgUse,
            },
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        "g" | "path" | "polyline" | "polygon" | "circle" | "ellipse" | "line" | "rect" => Tag {
            node_type: match name {
                "g" => NodeType::SvgGroup,
                "path" => NodeType::SvgPath,
                "polyline" => NodeType::SvgPolyline,
                "polygon" => NodeType::SvgPolygon,
                "circle" => NodeType::SvgCircle,
                "ellipse" => NodeType::SvgEllipse,
                "line" => NodeType::SvgLine,
                _ => NodeType::SvgRect,
            },
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        // @ref LLP 1069.001 D1, D2 — HTML's `select`: the platform's pop-up
        // control, its options its children. ARIA's role for a one-line
        // select is `combobox`.
        "select" => Tag {
            node_type: NodeType::Control,
            fixed_styles: &[],
            fixed_props: &[
                (PropId::Type, "select"),
                (PropId::AccessibilityRole, "combobox"),
            ],
            positional: None,
        },
        // An option is a paragraph a closed select never lays out: HTML's
        // `option` shows only in the menu the host builds from it.
        "option" => Tag {
            node_type: NodeType::Text,
            fixed_styles: &[(StyleId::Display, "none")],
            fixed_props: &[(PropId::SemanticTag, "option")],
            positional: Some(PropId::Text),
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
        // @ref LLP 1069.001 D4 — HTML's two: `input` as the value moves (a
        // text field's every keystroke), `change` when it is committed.
        "change" => AttrTarget::Handler("change"),
        "input" => AttrTarget::Handler("input"),
        "checked" => AttrTarget::Prop(p("checked")),
        // @ref LLP 1069.002 D1, D2 — a file input's types and count, and
        // HTML's `cancel` when its picker is dismissed.
        "accept" => AttrTarget::Prop(p("accept")),
        "multiple" => AttrTarget::Prop(p("multiple")),
        // @ref LLP 1069.001 D1 — a range's and a date's bounds, HTML's.
        "min" => AttrTarget::Prop(p("min")),
        "max" => AttrTarget::Prop(p("max")),
        "step" => AttrTarget::Prop(p("step")),
        "cancel" => AttrTarget::Handler("cancel"),
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
        // @ref LLP 1005 §3 — DOM's pointer down and up (a cancel is an up).
        "pointerdown" => AttrTarget::Handler("pointerdown"),
        "pointerup" => AttrTarget::Handler("pointerup"),
        "reachstart" => AttrTarget::Handler("reachstart"),
        "reachend" => AttrTarget::Handler("reachend"),
        "swiperight" => AttrTarget::Handler("swiperight"),
        "refresh" => AttrTarget::Handler("refresh"),
        "scroll" => AttrTarget::Handler("scroll"),
        "pan" => AttrTarget::Handler("pan"),
        "panrelease" => AttrTarget::Handler("panrelease"),
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
        // @ref LLP 1075.003 §3.5 — the route's content scroll view, by HTML id.
        "navigationScroll" => AttrTarget::Prop(p("navigationScroll")),
        // @ref LLP 1075.003.000 — the node the app's native code receives.
        "hook" => AttrTarget::Prop(p("hook")),
        "closedby" => AttrTarget::Prop(p("closedby")),
        "contextTarget" => AttrTarget::Prop(p("contextTarget")),
        "contextMagnify" => AttrTarget::Prop(p("contextMagnify")),
        "emojiPicker" => AttrTarget::Prop(p("emojiPicker")),
        "backgroundMaterial" => AttrTarget::Prop(p("backgroundMaterial")),
        "glassGroup" => AttrTarget::Prop(p("glassGroup")),
        "buttonStyle" => AttrTarget::Prop(p("buttonStyle")),
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
        "estimated-item-height" => AttrTarget::Prop(p("estimatedItemHeight")),
        // The row-axis twin (LLP 1070 H2): a virtualized row list's estimate.
        "estimated-item-width" => AttrTarget::Prop(p("estimatedItemWidth")),
        // How many rows a virtualized list builds before its first layout
        // report — a served page's and the first window's (LLP 1010 §6.5).
        "initial-item-count" => AttrTarget::Prop(p("initialItemCount")),
        // History's `scrollRestoration` values (LLP 1070 §4.2): whether a
        // nested list keeps its position across its row's retirement.
        "scroll-restoration" => AttrTarget::Prop(p("scrollRestoration")),
        // Where a virtualized list opens: at its start, or at its end (a
        // transcript), CSS Scroll Snap 2's container-level `scroll-start`.
        "scroll-start" => AttrTarget::Prop(p("scrollStart")),
        // @ref LLP 1056 D6 (r3): a canvas's explicit bitmap size, HTML's
        // `width`/`height` content attributes (Contract's are the CSS box).
        "bitmap-width" => AttrTarget::Prop(p("bitmapWidth")),
        "bitmap-height" => AttrTarget::Prop(p("bitmapHeight")),
        "scrollTop" => AttrTarget::Prop(p("scrollTop")),
        "scrollLeft" => AttrTarget::Prop(p("scrollLeft")),
        "swipeContent" => AttrTarget::Prop(p("swipeContent")),
        "swipeLeading" => AttrTarget::Prop(p("swipeLeading")),
        "swipeTrailing" => AttrTarget::Prop(p("swipeTrailing")),
        "destructive" => AttrTarget::Prop(p("destructive")),
        "scrollFollowEnd" => AttrTarget::Prop(p("scrollFollowEnd")),
        "refreshing" => AttrTarget::Prop(p("refreshing")),
        "keyboardDismissMode" => AttrTarget::Prop(p("keyboardDismissMode")),
        // @ref LLP 1077 D12 — what a discrete symbol effect plays on.
        "symbolEffectValue" => AttrTarget::Prop(p("symbolEffectValue")),
        "href" => AttrTarget::Prop(p("href")),
        "disabled" => AttrTarget::Prop(p("disabled")),
        "inert" => AttrTarget::Prop(p("inert")),
        "readonly" => AttrTarget::InvertedBoolProp(p("editable")),
        "lang" => AttrTarget::Prop(p("lang")),
        "src" => AttrTarget::Prop(p("src")),
        "sandbox" => AttrTarget::Prop(p("sandbox")),
        // The Popover API, by its own names (LLP 1021 D1): a container with `popover` is
        // hidden until its invoker — a `button` whose `popovertarget` names the container's
        // `id` — toggles it; open state is the host's, never the plan's (D2). `aria-checked`
        // is the ARIA state a menu row's dot would hand-draw; a native menu renders it as the
        // platform's checkmark (D3).
        "id" => AttrTarget::Prop(p("id")),
        "popover" => AttrTarget::Prop(p("popover")),
        "popovertarget" => AttrTarget::Prop(p("popovertarget")),
        "popovertargetaction" => AttrTarget::Prop(p("popovertargetaction")),
        "commandfor" => AttrTarget::Prop(p("commandfor")),
        "command" => AttrTarget::Prop(p("command")),
        "aria-checked" => AttrTarget::Prop(p("accessibilityChecked")),
        // @ref LLP 1039 D6 — vertical tablists retain authored layout.
        "aria-orientation" => AttrTarget::Prop(p("accessibilityOrientation")),
        // @ref LLP 1075.003 §3.7 — a tab names its tabpanel.
        "aria-controls" => AttrTarget::Prop(p("accessibilityControls")),
        "aria-selected" => AttrTarget::Prop(p("accessibilitySelected")),
        "aria-expanded" => AttrTarget::Prop(p("accessibilityExpanded")),
        "aria-pressed" => AttrTarget::Prop(p("accessibilityPressed")),
        "aria-hidden" => AttrTarget::Prop(p("accessibilityElementsHidden")),
        // SVG 2 attributes CSS cannot set (LLP 1055 D1/D2), by their SVG names.
        "viewBox" => AttrTarget::Prop(p("viewBox")),
        "preserveAspectRatio" => AttrTarget::Prop(p("preserveAspectRatio")),
        "points" => AttrTarget::Prop(p("points")),
        "d" => AttrTarget::Prop(p("d")),
        "pathLength" => AttrTarget::Prop(p("pathLength")),
        "fx" => AttrTarget::Prop(p("fx")),
        "fy" => AttrTarget::Prop(p("fy")),
        "fr" => AttrTarget::Prop(p("fr")),
        "gradientUnits" => AttrTarget::Prop(p("gradientUnits")),
        "gradientTransform" => AttrTarget::Prop(p("gradientTransform")),
        "spreadMethod" => AttrTarget::Prop(p("spreadMethod")),
        "offset" => AttrTarget::Prop(p("offset")),
        "clipPathUnits" => AttrTarget::Prop(p("clipPathUnits")),
        "maskUnits" => AttrTarget::Prop(p("maskUnits")),
        "maskContentUnits" => AttrTarget::Prop(p("maskContentUnits")),
        "patternUnits" => AttrTarget::Prop(p("patternUnits")),
        "patternContentUnits" => AttrTarget::Prop(p("patternContentUnits")),
        "patternTransform" => AttrTarget::Prop(p("patternTransform")),
        "filterUnits" => AttrTarget::Prop(p("filterUnits")),
        "primitiveUnits" => AttrTarget::Prop(p("primitiveUnits")),
        "in" => AttrTarget::Prop(p("in")),
        "in2" => AttrTarget::Prop(p("in2")),
        "result" => AttrTarget::Prop(p("result")),
        "stdDeviation" => AttrTarget::Prop(p("stdDeviation")),
        "feDx" => AttrTarget::Prop(p("feDx")),
        "feDy" => AttrTarget::Prop(p("feDy")),
        "operator" => AttrTarget::Prop(p("operator")),
        "k1" => AttrTarget::Prop(p("k1")),
        "k2" => AttrTarget::Prop(p("k2")),
        "k3" => AttrTarget::Prop(p("k3")),
        "k4" => AttrTarget::Prop(p("k4")),
        "mode" => AttrTarget::Prop(p("mode")),
        "values" => AttrTarget::Prop(p("values")),
        "edgeMode" => AttrTarget::Prop(p("edgeMode")),
        "feRadius" => AttrTarget::Prop(p("feRadius")),
        "tableValues" => AttrTarget::Prop(p("tableValues")),
        "slope" => AttrTarget::Prop(p("slope")),
        "intercept" => AttrTarget::Prop(p("intercept")),
        "amplitude" => AttrTarget::Prop(p("amplitude")),
        "exponent" => AttrTarget::Prop(p("exponent")),
        "baseFrequency" => AttrTarget::Prop(p("baseFrequency")),
        "numOctaves" => AttrTarget::Prop(p("numOctaves")),
        "seed" => AttrTarget::Prop(p("seed")),
        "stitchTiles" => AttrTarget::Prop(p("stitchTiles")),
        "feScale" => AttrTarget::Prop(p("feScale")),
        "xChannelSelector" => AttrTarget::Prop(p("xChannelSelector")),
        "yChannelSelector" => AttrTarget::Prop(p("yChannelSelector")),
        "order" => AttrTarget::Prop(p("order")),
        "kernelMatrix" => AttrTarget::Prop(p("kernelMatrix")),
        "divisor" => AttrTarget::Prop(p("divisor")),
        "bias" => AttrTarget::Prop(p("bias")),
        "targetX" => AttrTarget::Prop(p("targetX")),
        "targetY" => AttrTarget::Prop(p("targetY")),
        "preserveAlpha" => AttrTarget::Prop(p("preserveAlpha")),
        "surfaceScale" => AttrTarget::Prop(p("surfaceScale")),
        "diffuseConstant" => AttrTarget::Prop(p("diffuseConstant")),
        "specularConstant" => AttrTarget::Prop(p("specularConstant")),
        "specularExponent" => AttrTarget::Prop(p("specularExponent")),
        "azimuth" => AttrTarget::Prop(p("azimuth")),
        "elevation" => AttrTarget::Prop(p("elevation")),
        "lightX" => AttrTarget::Prop(p("lightX")),
        "lightY" => AttrTarget::Prop(p("lightY")),
        "lightZ" => AttrTarget::Prop(p("lightZ")),
        "pointsAtX" => AttrTarget::Prop(p("pointsAtX")),
        "pointsAtY" => AttrTarget::Prop(p("pointsAtY")),
        "pointsAtZ" => AttrTarget::Prop(p("pointsAtZ")),
        "limitingConeAngle" => AttrTarget::Prop(p("limitingConeAngle")),
        "filter" => styles(&[StyleId::Filter]),
        "mix-blend-mode" => styles(&[StyleId::MixBlendMode]),
        "isolation" => styles(&[StyleId::Isolation]),
        "flood-color" => styles(&[StyleId::FloodColor]),
        "flood-opacity" => styles(&[StyleId::FloodOpacity]),
        "lighting-color" => styles(&[StyleId::LightingColor]),
        "color-interpolation-filters" => styles(&[StyleId::ColorInterpolationFilters]),
        "mask" => styles(&[StyleId::SvgMask]),
        "mask-type" => styles(&[StyleId::MaskType]),
        "markerWidth" => AttrTarget::Prop(p("markerWidth")),
        "markerHeight" => AttrTarget::Prop(p("markerHeight")),
        "refX" => AttrTarget::Prop(p("refX")),
        "refY" => AttrTarget::Prop(p("refY")),
        "orient" => AttrTarget::Prop(p("orient")),
        "markerUnits" => AttrTarget::Prop(p("markerUnits")),
        "marker-start" => styles(&[StyleId::MarkerStart]),
        "marker-mid" => styles(&[StyleId::MarkerMid]),
        "marker-end" => styles(&[StyleId::MarkerEnd]),
        "marker" => styles(&[StyleId::MarkerStart, StyleId::MarkerMid, StyleId::MarkerEnd]),
        // @ref LLP 1055.000 D11 — SVG text's positions (`x`, `y`, `dx`,
        // `dy` on `text`/`tspan` lower to these) and its alignment.
        "textX" => AttrTarget::Prop(p("textX")),
        "textY" => AttrTarget::Prop(p("textY")),
        "textDx" => AttrTarget::Prop(p("textDx")),
        "textDy" => AttrTarget::Prop(p("textDy")),
        "text-anchor" => styles(&[StyleId::TextAnchor]),
        "dominant-baseline" => styles(&[StyleId::DominantBaseline]),
        "pointer-events" => styles(&[StyleId::PointerEvents]),
        "clip-rule" => styles(&[StyleId::ClipRule]),
        "stop-color" => styles(&[StyleId::StopColor]),
        "stop-opacity" => styles(&[StyleId::StopOpacity]),
        "paint-order" => styles(&[StyleId::PaintOrder]),
        "x1" => AttrTarget::Prop(p("x1")),
        "y1" => AttrTarget::Prop(p("y1")),
        "x2" => AttrTarget::Prop(p("x2")),
        "y2" => AttrTarget::Prop(p("y2")),
        // SVG 2 presentation and geometry properties: CSS rows (LLP 1055 D2).
        "fill" => styles(&[StyleId::Fill]),
        "stroke" => styles(&[StyleId::Stroke]),
        "stroke-width" => styles(&[StyleId::StrokeWidth]),
        "stroke-linecap" => styles(&[StyleId::StrokeLinecap]),
        "stroke-linejoin" => styles(&[StyleId::StrokeLinejoin]),
        "stroke-miterlimit" => styles(&[StyleId::StrokeMiterlimit]),
        "stroke-dasharray" => styles(&[StyleId::StrokeDasharray]),
        "stroke-dashoffset" => styles(&[StyleId::StrokeDashoffset]),
        "fill-opacity" => styles(&[StyleId::FillOpacity]),
        "stroke-opacity" => styles(&[StyleId::StrokeOpacity]),
        "fill-rule" => styles(&[StyleId::FillRule]),
        "x" => styles(&[StyleId::X]),
        "y" => styles(&[StyleId::Y]),
        "rx" => styles(&[StyleId::Rx]),
        "ry" => styles(&[StyleId::Ry]),
        // @ref LLP 1055.000 D5 — transforms on SVG elements.
        "transform" => styles(&[StyleId::Transform]),
        "transform-origin" => styles(&[StyleId::TransformOrigin]),
        "transform-box" => styles(&[StyleId::TransformBox]),
        "vector-effect" => styles(&[StyleId::VectorEffect]),
        "visibility" => styles(&[StyleId::Visibility]),
        "cx" => styles(&[StyleId::Cx]),
        "cy" => styles(&[StyleId::Cy]),
        "r" => styles(&[StyleId::R]),
        // CSS Animations (LLP 1055 D5): the shorthand is the row; the
        // longhands compose into it before lowering (`svg::compose_animation`).
        "animation"
        | "animation-name"
        | "animation-duration"
        | "animation-timing-function"
        | "animation-delay"
        | "animation-iteration-count"
        | "animation-direction"
        | "animation-fill-mode"
        | "animation-play-state" => styles(&[StyleId::Animation]),
        // style rows, by their CSS property names
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
        // @ref LLP 1064 D5
        "text-transform" => styles(&[StyleId::TextTransform]),
        "font-size" => styles(&[StyleId::FontSize]),
        "font-weight" => styles(&[StyleId::FontWeight]),
        "font-style" => styles(&[StyleId::FontStyle]),
        "font-family" => styles(&[StyleId::FontFamily]),
        "color" => styles(&[StyleId::TextColor]),
        "background-color" => styles(&[StyleId::BackgroundColor]),
        // @ref LLP 1066 — `none` or one linear/radial gradient.
        "background-image" => styles(&[StyleId::BackgroundImage]),
        // @ref LLP 1066 D7 — `fixed`: the gradient box is the viewport.
        "background-attachment" => styles(&[StyleId::BackgroundAttachment]),
        // @ref LLP 1077 D1–D3
        "mask-image" => styles(&[StyleId::MaskImage]),
        "text-shadow" => styles(&[StyleId::TextShadow]),
        "corner-shape" => styles(&[StyleId::CornerShape]),
        "background-clip" => styles(&[StyleId::BackgroundClip]),
        // @ref LLP 1077 D7 — the Compat Standard's names; the shorthand
        // binds both rows, each taking its part.
        "-webkit-text-stroke" => styles(&[StyleId::TextStrokeWidth, StyleId::TextStrokeColor]),
        "-webkit-text-stroke-width" => styles(&[StyleId::TextStrokeWidth]),
        "-webkit-text-stroke-color" => styles(&[StyleId::TextStrokeColor]),
        "caret-color" => styles(&[StyleId::CaretColor]),
        // @ref LLP 1069.001 D6 — a form control's tint and whether the
        // platform draws it.
        "accent-color" => styles(&[StyleId::AccentColor]),
        "appearance" => styles(&[StyleId::Appearance]),
        "tint-color" => styles(&[StyleId::TintColor]),
        "opacity" => styles(&[StyleId::Opacity]),
        // @ref LLP 1064 D1 — one value, each row takes its part of the parse.
        "box-shadow" => styles(&[StyleId::BoxShadow]),
        // @ref LLP 1053.000 D1 — `none` or one `blur(<length>)`; the rest of
        // CSS's filter functions are refused by name.
        "backdrop-filter" => styles(&[StyleId::BackdropBlur]),
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
        // @ref LLP 1057.003 D1 — drag timelines, CSS scroll-driven animations' shape.
        "drag-timeline" => styles(&[StyleId::DragTimeline]),
        "animation-timeline" => styles(&[StyleId::AnimationTimeline]),
        "animation-range" => styles(&[StyleId::AnimationRange]),
        // @ref LLP 1057.003 D4 — CSS `timeline-scope`.
        "timeline-scope" => styles(&[StyleId::TimelineScope]),
        "display" => styles(&[StyleId::Display]),
        "grid-auto-flow" => styles(&[StyleId::GridAutoFlow]),
        "grid-template-columns" => styles(&[StyleId::GridTemplateColumns]),
        "grid-template-rows" => styles(&[StyleId::GridTemplateRows]),
        "grid-column" => styles(&[StyleId::GridColumn]),
        "grid-row" => styles(&[StyleId::GridRow]),
        "align-items" => styles(&[StyleId::AlignItems]),
        "align-content" => styles(&[StyleId::AlignContent]),
        "align-self" => styles(&[StyleId::AlignSelf]),
        "box-sizing" => styles(&[StyleId::BoxSizing]),
        "object-fit" => styles(&[StyleId::ObjectFit]),
        "justify-content" => styles(&[StyleId::JustifyContent]),
        "justify-items" => styles(&[StyleId::JustifyItems]),
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
        // @ref LLP 1063 — played as the node leaves; its names resolve against
        // the plan's keyframes as `animation`'s do (LLP 1055 D5).
        "exit-animation" => styles(&[StyleId::ExitAnimation]),
        // @ref LLP 1063 — how the laid-out box moves when layout moves it.
        "layout-transition" => styles(&[StyleId::LayoutTransition]),
        "interpolate-size" => styles(&[StyleId::InterpolateSize]),
        // @ref LLP 1077 D8 — one value to two rows: x and y, and z; the
        // angle, and its axis.
        "translate" => styles(&[StyleId::Translate, StyleId::TranslateZ]),
        "scale" => styles(&[StyleId::Scale]),
        "rotate" => styles(&[StyleId::Rotate, StyleId::RotateAxis]),
        "perspective" => styles(&[StyleId::Perspective]),
        "perspective-origin" => styles(&[StyleId::PerspectiveOrigin]),
        "backface-visibility" => styles(&[StyleId::BackfaceVisibility]),
        // @ref LLP 1077 §5 — declared rows CSS has no name for.
        "symbol-rendering" => styles(&[StyleId::SymbolRendering]),
        "symbol-palette" => styles(&[StyleId::SymbolPalette]),
        "symbol-value" => styles(&[StyleId::SymbolValue]),
        "symbol-effect" => styles(&[StyleId::SymbolEffect]),
        "press-haptic" => styles(&[StyleId::PressHaptic]),
        "content-transition" => styles(&[StyleId::ContentTransition]),
        "scroll-edge-effect" => styles(&[StyleId::ScrollEdgeEffect]),
        "hover-effect" => styles(&[StyleId::HoverEffect]),
        "smart-invert" => styles(&[StyleId::SmartInvert]),
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
        "dragTimeline" => "drag-timeline",
        "animationTimeline" => "animation-timeline",
        "animationRange" => "animation-range",
        "timelineScope" => "timeline-scope",
        "transformOrigin" => "transform-origin",
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
        "onChange" | "onChangeText" | "onInput" => "input",
        "className" | "class" | "style" => return None,
        _ => return None,
    })
}

// A list's row-size estimate is the virtualized list's host policy (LLP 1010
// §6.5); the fixed-height windowed list it once also named is deleted (LLP
// 1070 stage 1), so a hint without `virtualized=true` says how to migrate.
pub(crate) fn validate_list(
    tag: &str,
    expanded: &[contract_syntax::Attr],
    span: contract_syntax::Span,
) -> Result<(), super::LowerError> {
    use contract_syntax::Expr;
    let virtualized = tag == "list"
        && expanded
            .iter()
            .any(|a| a.name == "virtualized" && matches!(a.value, Expr::Bool(true, _)));
    // A virtualized list's first window, and where it opens (its value is
    // checked with the list's, `collection.rs`).
    for name in ["initial-item-count", "scroll-start"] {
        if let Some(a) = expanded.iter().find(|a| a.name == name && !virtualized) {
            let why =
                format!("`{name}` is a virtualized list's; it goes on `list virtualized=true`");
            return super::err("lower-list-virtualized", why, a.span);
        }
    }
    if let Some(count) = expanded.iter().find(|a| a.name == "initial-item-count") {
        if !matches!(count.value, Expr::Number(n, _) if n.fract() == 0.0 && (1.0..=64.0).contains(&n))
        {
            return super::err(
                "lower-list-height",
                "`initial-item-count` is one literal whole number of rows, 1 to 64",
                count.span,
            );
        }
    }
    if let Some(fixed) = expanded.iter().find(|a| a.name == "item-height") {
        return super::err(
            "lower-list-height",
            "`item-height` was the deleted windowed list's; write `virtualized=true estimated-item-height=N`, which measures every row",
            fixed.span,
        );
    }
    let Some(estimate) = expanded.iter().find(|a| {
        matches!(
            a.name.as_str(),
            "estimated-item-height" | "estimated-item-width"
        )
    }) else {
        return Ok(());
    };
    if tag != "list" {
        return super::err(
            "lower-list-height",
            "row height hints belong on `list`",
            span,
        );
    }
    if !expanded
        .iter()
        .any(|a| a.name == "virtualized" && matches!(a.value, Expr::Bool(true, _)))
    {
        return super::err(
            "lower-list-virtualized",
            format!("`{}` is a virtualized list's estimate; add `virtualized=true` (the windowed list without it is deleted, LLP 1070)", estimate.name),
            estimate.span,
        );
    }
    if !matches!(estimate.value, Expr::Number(n, _) if n.is_finite() && n > 0.0) {
        return super::err(
            "lower-list-height",
            format!(
                "virtualized lists accept one positive literal `{}`",
                estimate.name
            ),
            estimate.span,
        );
    }
    Ok(())
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

/// Exclusions wrap their parent's text and must be positioned by that parent.
pub(crate) fn check_exclusion(
    tag: &Tag,
    attrs: &[contract_syntax::Attr],
    parent_positioned: bool,
) -> Result<(), crate::LowerError> {
    use contract_syntax::Expr;
    // @ref LLP 1043.000 §3 D1 — dynamic positioning is checked by layout.
    if let Some(wrap) = attrs.iter().find(|a| a.name == "wrap-flow") {
        if matches!(&wrap.value, Expr::Str(v, _) if v == "both") {
            let position = attrs.iter().find(|a| a.name == "position");
            let absolute = tag
                .fixed_styles
                .iter()
                .any(|(id, v)| *id == StyleId::PositionType && *v == "absolute");
            if position.map_or(
                !absolute,
                |a| matches!(&a.value, Expr::Str(v, _) if v != "absolute"),
            ) {
                return crate::err(
                    "lower-attr-value",
                    "`wrap-flow: both` requires `position: absolute` in exact2 v1",
                    wrap.span,
                );
            }
            if !parent_positioned {
                return crate::err(
                    "lower-attr-value",
                    "`wrap-flow: both` wraps its parent's text; give the parent `position: relative`",
                    wrap.span,
                );
            }
        }
    }
    Ok(())
}

/// Host transforms can address a box by its place in the expanded view, not
/// just its attributes (LLP 1074 D1). Regions have no box. Geometry decides
/// which siblings move at runtime, so every possible recipient contains at rest.
pub(crate) fn host_transform_recipients(
    lower: &crate::Lowerer<'_>,
    nodes: &[contract_syntax::Node],
) -> std::collections::BTreeSet<(contract_syntax::Span, u32)> {
    use contract_syntax::{Attr, Expr, Node, Span};
    struct BoxSite {
        key: (Span, u32),
        parent: Option<usize>,
        attrs: Vec<Attr>,
        absolute: bool,
        maybe_absolute: bool,
        scrolls: bool,
        repeated: bool,
    }
    fn collect(
        lower: &crate::Lowerer<'_>,
        nodes: &[Node],
        parent: Option<usize>,
        repeated: bool,
        boxes: &mut Vec<BoxSite>,
    ) {
        for node in nodes {
            match node {
                Node::Element {
                    tag: name,
                    attrs,
                    children,
                    span,
                    instance,
                    ..
                } => {
                    // Normal lowering reports class errors with its other refusals.
                    let mut rows = lower
                        .class_rows(attrs)
                        .ok()
                        .flatten()
                        .map_or_else(Vec::new, |(_, r)| r);
                    rows.extend_from_slice(attrs);
                    let position = rows.iter().rev().find(|a| a.name == "position");
                    let fixed = tag(name).is_some_and(|t| {
                        t.fixed_styles
                            .iter()
                            .any(|(id, v)| *id == StyleId::PositionType && *v == "absolute")
                    });
                    let absolute = position.map_or(
                        fixed,
                        |a| matches!(&a.value, Expr::Str(v, _) if v == "absolute"),
                    );
                    let maybe_absolute =
                        absolute || position.is_some_and(|a| !matches!(&a.value, Expr::Str(_, _)));
                    let scrolls = tag(name).is_some_and(|t| t.node_type.scrolls_by_default())
                        || rows.iter().any(|a| matches!(a.name.as_str(), "overflow" | "overflow-y")
                            && !matches!(&a.value, Expr::Str(v, _) if v != "auto" && v != "scroll"));
                    let index = boxes.len();
                    boxes.push(BoxSite {
                        key: (*span, *instance),
                        parent,
                        attrs: rows,
                        absolute,
                        maybe_absolute,
                        scrolls,
                        repeated,
                    });
                    collect(lower, children, Some(index), false, boxes);
                }
                Node::When {
                    then, otherwise, ..
                } => {
                    collect(lower, then, parent, repeated, boxes);
                    collect(lower, otherwise, parent, repeated, boxes);
                }
                Node::Each { body, .. } => collect(lower, body, parent, true, boxes),
                Node::Match { some, none, .. } => {
                    collect(lower, &some.1, parent, repeated, boxes);
                    collect(lower, none, parent, repeated, boxes);
                }
                _ => {}
            }
        }
    }
    let mut boxes = Vec::new();
    collect(lower, nodes, None, false, &mut boxes);
    let mut recipients = std::collections::BTreeSet::new();
    for (preview, site) in boxes.iter().enumerate() {
        let Some(target) = site.attrs.iter().find(|a| a.name == "contextTarget") else {
            continue;
        };
        let mut path = Vec::new();
        let mut child = preview;
        let mut panel = false;
        // The authored root is below the host carrier: inspect it too.
        while let Some(parent) = boxes[child].parent {
            path.push((child, parent));
            panel |= boxes[parent].maybe_absolute;
            if boxes[parent].absolute {
                break;
            }
            child = parent;
        }
        if !panel {
            continue;
        }
        for (child, parent) in path {
            // Other instances of this same template are runtime siblings.
            // In a virtual list the flow root also shields its descendants
            // from the generated wrapper that the host transforms.
            if boxes[child].repeated {
                recipients.insert(boxes[child].key);
            }
            for (index, sibling) in boxes.iter().enumerate() {
                if sibling.parent == Some(parent) && index != child {
                    recipients.insert(sibling.key);
                }
            }
        }
        // contextContent(target): the source's child of its nearest scroll
        // ancestor moves too. Literal IDs narrow this exactly; bound IDs may
        // name any authored ID, but never an anonymous, unrelated subtree.
        for (source, candidate) in boxes.iter().enumerate() {
            let Some(id) = candidate.attrs.iter().find(|a| a.name == "id") else {
                continue;
            };
            if matches!((&target.value, &id.value), (Expr::Str(a, _), Expr::Str(b, _)) if a != b) {
                continue;
            }
            let mut child = source;
            while let Some(parent) = boxes[child].parent {
                if boxes[parent].scrolls {
                    recipients.insert(boxes[child].key);
                    break;
                }
                child = parent;
            }
        }
    }
    recipients
}
