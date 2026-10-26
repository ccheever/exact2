//! The tag and attribute table: Contract's view vocabulary onto the kernel's.
//!
//! @ref LLP 1004 D2 (kernel ordinals from `exact-kernel`, never redeclared)
//! @ref LLP 1115 §2 (names stay CSS: every row here is a
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
use exact_plan::BindingsRow;
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
    /// CSS `flex` shorthand — grow, shrink, and basis together.
    Flex,
    /// CSS border and text-decoration shorthands.
    Shorthand,
    /// A canvas's surface binding: `surface=name(args)` (LLP 1009 D3).
    Surface,
    /// `metadata=MediaMetadata(…)` on `audio` or `video`: four string props,
    /// one a field (LLP 1098 D1).
    MediaMetadata,
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
        // @ref LLP 1021 D1 — HTML's separator, one hairline (LLP 1115.000
        // D10.3); void.
        "hr" => view(crate::menus::HR, &[(PropId::SemanticTag, "hr")]),
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
        "time" => Tag {
            node_type: NodeType::Text,
            fixed_styles: &[],
            fixed_props: &[(PropId::SemanticTag, "time")],
            positional: None,
        },
        "text" => Tag {
            node_type: NodeType::Text,
            fixed_styles: &[],
            fixed_props: &[],
            positional: Some(PropId::Text),
        },
        // Chrome's `<button>` (Charlie, 2026-10-04, reversing 2026-09-23's
        // "One native button, flex column"; LLP 1001 §1): a block whose
        // content the kernel centres as HTML's anonymous button box does,
        // with the UA sheet's `text-align: center`. An authored `display`
        // makes it a flex or grid container, as in Chrome.
        // @ref LLP 1104 D1, D2 — lowering chooses native or bare from the
        // author's rows and face; the tag has no fixed appearance.
        "button" => Tag {
            node_type: NodeType::Pressable,
            fixed_styles: &[(StyleId::TextAlign, "center")],
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
        "audio" => crate::media::AUDIO,
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
        // they are referenced; a pattern's tile clips its content, which the hosts do (no row says so).
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
        // @ref LLP 1069.001 (amended 2026-10-07) — HTML's `progress` with no
        // `value`, indeterminate: the platform's activity indicator, a
        // measured leaf (20 × 20 until sized); the lowering adds `aria-busy`.
        // @ref LLP 1116 D8 — with a `value` (and `max`, strings as a
        // range's), the determinate bar, sized by HTML's UA sheet. ARIA's
        // role is `progressbar` either way.
        "progress" => Tag {
            node_type: NodeType::Control,
            fixed_styles: &[],
            fixed_props: &[
                (PropId::Type, "progress"),
                (PropId::AccessibilityRole, "progressbar"),
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
    // A state part's row (`:disabled opacity`, LLP 1121 D8) is the row it is.
    let name = contract_syntax::state::state_of(name).map_or(name, |(_, row)| row);
    matches!(
        attr(name),
        Some(AttrTarget::Styles(_) | AttrTarget::Flex | AttrTarget::Shorthand)
    )
}
/// Look up an attribute as written, routed by its value where one name is
/// two things: `resize` is CSS's property for a string, and given an action
/// (an `Ident` or a `Call`, never valid CSS there) the element resize event,
/// ResizeObserver's (x2apps backlog: decided, route by value).
pub fn attr_valued(name: &str, value: &contract_syntax::Expr) -> Option<AttrTarget> {
    use contract_syntax::Expr;
    if name == "resize" && matches!(value, Expr::Ident(..) | Expr::Call(..)) {
        return Some(AttrTarget::Handler("resize"));
    }
    attr(name)
}
/// The style rows an attribute writes: its own, a shorthand's longhands,
/// none for a prop or a handler.
pub fn rows(a: &contract_syntax::Attr) -> Vec<StyleId> {
    match attr_valued(&a.name, &a.value) {
        Some(AttrTarget::Styles(ids)) => ids.to_vec(),
        Some(AttrTarget::Shorthand) => crate::shorthands::rows(&a.name).to_vec(),
        Some(AttrTarget::Flex) => vec![StyleId::FlexGrow, StyleId::FlexShrink, StyleId::FlexBasis],
        _ => Vec::new(),
    }
}

/// Look up an attribute.
pub fn attr(name: &str) -> Option<AttrTarget> {
    if let Some(target) = crate::style_names::lookup(name) {
        return Some(target);
    }
    Some(match name {
        "datetime" => AttrTarget::Prop(PropId::Datetime),
        "format" => AttrTarget::Prop(PropId::TimeFormat),
        "start" => AttrTarget::Prop(PropId::Start),
        "end" => AttrTarget::Prop(PropId::End),
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
        // A `video` entered or left full screen (`requestFullscreen`, the
        // platform's own controls); the payload says which.
        "fullscreenchange" => AttrTarget::Handler("fullscreenchange"),
        "canplay" => AttrTarget::Handler("canplay"),
        // @ref LLP 1098 D1, D2 — the media session: `metadata=` claims it,
        // the six actions by `setActionHandler`'s names, and the seconds a
        // seek moves when the platform gives none.
        "metadata" => AttrTarget::MediaMetadata,
        "decorations" => AttrTarget::Prop(PropId::Decorations),
        // LLP 1045.000 D3: a Markdown node's atom kinds and faces (the
        // lowerer's `bind_atoms` and `bind_atom_faces` read them first).
        "atoms" => AttrTarget::Prop(PropId::Atoms),
        "atomFaces" => AttrTarget::Prop(PropId::AtomFaces),
        "alertPart" => AttrTarget::Prop(PropId::AlertPart),
        "defaultSurface" => AttrTarget::Prop(PropId::DefaultSurface),
        "seekbackward" => AttrTarget::Handler("seekbackward"),
        "seekforward" => AttrTarget::Handler("seekforward"),
        "seekto" => AttrTarget::Handler("seekto"),
        "previoustrack" => AttrTarget::Handler("previoustrack"),
        "nexttrack" => AttrTarget::Handler("nexttrack"),
        "stop" => AttrTarget::Handler("stop"),
        "seekbackwardOffset" => AttrTarget::Prop(p("seekbackwardOffset")),
        "seekforwardOffset" => AttrTarget::Prop(p("seekforwardOffset")),
        "press" => AttrTarget::Handler("press"),
        // @ref LLP 1069.001 D4 — HTML's two: `input` as the value moves (a
        // text field's every keystroke), `change` when it is committed.
        "change" => AttrTarget::Handler("change"),
        "input" => AttrTarget::Handler("input"),
        "checked" => AttrTarget::Prop(p("checked")),
        // HTML's radio button group (x2apps survey #2).
        "name" => AttrTarget::Prop(p("name")),
        // @ref LLP 1069.002 D1, D2 — a file input's types and count, and
        // HTML's `cancel` when its picker is dismissed.
        "accept" => AttrTarget::Prop(p("accept")),
        "multiple" => AttrTarget::Prop(p("multiple")),
        // @ref LLP 1069.001 D1 — a range's and a date's bounds, HTML's.
        "min" => AttrTarget::Prop(p("min")),
        "max" => AttrTarget::Prop(p("max")),
        "step" => AttrTarget::Prop(p("step")),
        "rows" => AttrTarget::Prop(p("rows")),
        "maxlength" => AttrTarget::Prop(p("maxlength")),
        "cancel" => AttrTarget::Handler("cancel"),
        "close" => AttrTarget::Handler("close"),
        "toggle" => AttrTarget::Handler("toggle"),
        "select" => AttrTarget::Handler("select"),
        "hover" => AttrTarget::Handler("hover"),
        "focus" => AttrTarget::Handler("focus"),
        "blur" => AttrTarget::Handler("blur"),
        "focusin" => AttrTarget::Handler("focusin"),
        "focusout" => AttrTarget::Handler("focusout"),
        "key" => AttrTarget::Handler("key"),
        "keycapture" => AttrTarget::Handler("keycapture"),
        // DOM's `keyup`: a key's release, bubbling as `key` does (#140).
        "keyup" => AttrTarget::Handler("keyup"),
        "submit" => AttrTarget::Handler("submit"),
        "load" => AttrTarget::Handler("load"),
        "message" => AttrTarget::Handler("message"),
        "contextmenu" => AttrTarget::Handler("contextmenu"),
        "dblclick" => AttrTarget::Handler("dblclick"),
        "pointerdown" => AttrTarget::Handler("pointerdown"), // LLP 1005 §Events, DOM's own
        "pointerup" => AttrTarget::Handler("pointerup"),
        "pointermove" => AttrTarget::Handler("pointermove"), // LLP 1056 §3 stage 3
        // DOM's `beforeunload` (studio diary R17): the window is closing or
        // the app quitting; an action that calls `preventDefault()` keeps it.
        "beforeunload" => AttrTarget::Handler("beforeunload"),
        // @ref LLP 1125 D8 — `MIDIInput`'s event: every mounted element that
        // declares it hears each message from an input its `midiinputs` lists.
        "midimessage" => AttrTarget::Handler("midimessage"),
        "midiinputs" => AttrTarget::Prop(p("midiInputs")),
        // DOM's `wheel` (studio diary R3), `drop` of files (R19), and the
        // `dragenter` and `dragleave` of a file drag over the node.
        "wheel" => AttrTarget::Handler("wheel"),
        "drop" => AttrTarget::Handler("drop"),
        "dragenter" => AttrTarget::Handler("dragenter"),
        "dragleave" => AttrTarget::Handler("dragleave"),
        // A press on an inline atom (LLP 1045.000 D7).
        "atompress" => AttrTarget::Handler("atompress"),
        "copy" => AttrTarget::Handler("copy"),
        "cut" => AttrTarget::Handler("cut"),
        "paste" => AttrTarget::Handler("paste"),
        "beforeinput" => AttrTarget::Handler("beforeinput"),
        "selectionchange" => AttrTarget::Handler("selectionchange"),
        "reachstart" => AttrTarget::Handler("reachstart"),
        "intersect" => AttrTarget::Handler("intersect"),
        "intersect-margin" => AttrTarget::Prop(p("intersectMargin")),
        "reachend" => AttrTarget::Handler("reachend"),
        "swiperight" => AttrTarget::Handler("swiperight"),
        "refresh" => AttrTarget::Handler("refresh"),
        "scroll" => AttrTarget::Handler("scroll"),
        "scrollend" => AttrTarget::Handler("scrollend"),
        "pan" => AttrTarget::Handler("pan"),
        "panrelease" => AttrTarget::Handler("panrelease"),
        "navigate" => AttrTarget::Handler("navigate"),
        // @ref LLP 1020.002 D2 — an iframe's guest, by the Navigation API's
        // event names (an iframe's `navigate=` is its guest's too).
        "currententrychange" => AttrTarget::Handler("currententrychange"),
        "navigatesuccess" => AttrTarget::Handler("navigatesuccess"),
        "navigateerror" => AttrTarget::Handler("navigateerror"),
        // @ref LLP 1020.002 D5 — a guest's window request and its downloads.
        "newwindow" => AttrTarget::Handler("newwindow"),
        "download" => AttrTarget::Handler("download"),
        // @ref LLP 1020.002 D7 — the answer to a frame's `captureScreenshot`.
        "capture" => AttrTarget::Handler("capture"),
        // @ref LLP 1020.002 D9 — an agent-controlled frame's dialog.
        "userpromptopened" => AttrTarget::Handler("userpromptopened"),
        "heightrelease" => AttrTarget::Handler("heightrelease"),
        "transformgeometry" => AttrTarget::Handler("transformgeometry"),
        "transformrelease" => AttrTarget::Handler("transformrelease"),
        "reorderdrop" => AttrTarget::Handler("reorderdrop"),
        "reorderFor" => AttrTarget::Prop(p("reorderFor")),
        "reorderGroup" => AttrTarget::Prop(p("reorderGroup")),
        "transformDragFor" => AttrTarget::Prop(p("transformDragFor")),
        "heightDragFor" => AttrTarget::Prop(p("heightDragFor")),
        "surface" => AttrTarget::Surface, // the canvas's surface (LLP 1009 D3)
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
        "edited" => AttrTarget::Prop(p("headEdited")),
        // `scroll document=(expr)`: the page's scroller when it holds (LLP 1048.003 D4); bare is `=true`.
        "document" => AttrTarget::Prop(p("scrollDocument")),
        "virtualized" => AttrTarget::Prop(p("virtualized")),
        "testId" => AttrTarget::Prop(p("testId")),
        "navigationKey" => AttrTarget::Prop(p("navigationKey")),
        "navigationBack" => AttrTarget::Prop(p("navigationBack")),
        "navigationPresentation" => AttrTarget::Prop(p("navigationPresentation")),
        "navigationDetent" => AttrTarget::Prop(p("navigationDetent")),
        "navigationInitialDetent" => AttrTarget::Prop(p("navigationInitialDetent")),
        "navigationGrabber" => AttrTarget::Prop(p("navigationGrabber")),
        // @ref LLP 1125 D11 — the Screen Wake Lock API's one type, on any element.
        "wakelock" => AttrTarget::Prop(p("wakelock")),
        // @ref LLP 1133.029 D1 — the Screen Orientation API's lock types, on any element.
        "orientationlock" => AttrTarget::Prop(p("orientationlock")),
        // @ref LLP 1115.000 D5.2 — a route's say over the platform's bar:
        // `none` for a full-bleed screen.
        "navigationBar" => AttrTarget::Prop(p("navigationBar")),
        "-exact-safe-area" => AttrTarget::Prop(p("safeArea")),
        "navigationSource" | "sharedElement" => AttrTarget::Prop(p(name)),
        // @ref LLP 1075.003 §3.5 — the route's content scroll view, by HTML id.
        "navigationScroll" => AttrTarget::Prop(p("navigationScroll")),
        // @ref LLP 1116.004 D1.2 — a wide window's split: "none", or the list column's width.
        "navigationSplit" => AttrTarget::Prop(p("navigationSplit")),
        // @ref LLP 1075.003.000 — the node the app's native code receives.
        "hatch" => AttrTarget::Prop(p("hatch")),
        "closedby" => AttrTarget::Prop(p("closedby")),
        "contextTarget" => AttrTarget::Prop(p("contextTarget")),
        "contextMagnify" => AttrTarget::Prop(p("contextMagnify")),
        // @ref LLP 1021 §5.1 — the popover a node's context menu shows, and its preview row.
        "contextPopover" | "contextPreview" => AttrTarget::Prop(p(name)),
        "emojiPicker" => AttrTarget::Prop(p("emojiPicker")),
        "backgroundMaterial" => AttrTarget::Prop(p("backgroundMaterial")),
        "glassGroup" => AttrTarget::Prop(p("glassGroup")),
        "buttonStyle" => AttrTarget::Prop(p("buttonStyle")),
        "listStyle" => AttrTarget::Prop(p("listStyle")),
        "toolbarPlacement" => AttrTarget::Prop(p("toolbarPlacement")),
        "retainFocus" => AttrTarget::Prop(p("retainFocus")),
        "focusGuide" => AttrTarget::Prop(p("focusGuide")),
        "swipeIndicator" => AttrTarget::Prop(p("swipeIndicator")),
        "aria-live" => AttrTarget::Prop(p("accessibilityLive")),
        "aria-busy" => AttrTarget::Prop(p("accessibilityBusy")),
        "aria-atomic" => AttrTarget::Prop(p("accessibilityAtomic")),
        "autofocus" => AttrTarget::Prop(p("autofocus")),
        "action" => AttrTarget::Prop(p("action")),
        "aria-label" => AttrTarget::Prop(p("accessibilityLabel")),
        "-exact-menu" => AttrTarget::Prop(p("menuPlacement")),
        "-exact-menu-remove" => AttrTarget::Prop(p("menuRemove")),
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
        "enterkeyhint" => AttrTarget::Prop(p("enterKeyHint")),
        "autocomplete" => AttrTarget::Prop(p("autocomplete")),
        // An image's accessible name by HTML's spelling (feed F1).
        "alt" => AttrTarget::Prop(p("accessibilityLabel")),
        "autocapitalize" => AttrTarget::Prop(p("autocapitalize")),
        "autocorrect" => AttrTarget::Prop(p("autocorrect")),
        "spellcheck" => AttrTarget::Prop(p("spellcheck")),
        // The viewport meta's `viewport-fit=cover`, read from the first root
        // (LLP 1008 §9): the layout viewport becomes the whole screen and
        // `env(safe-area-inset-*)` lengths carry the insets.
        "viewport-fit" => AttrTarget::Prop(p("viewportFit")),
        // The status bar's text over this node's screen, from state (LLP 1105).
        "status-bar-style" => AttrTarget::Prop(p("statusBarStyle")),
        "status-bar-animation" => AttrTarget::Prop(p("statusBarAnimation")),
        // The viewport meta's `interactive-widget`, read from the first root
        // (LLP 1008 §9): `resizes-content` shrinks the layout viewport to a
        // software keyboard's top, so what is pinned to the bottom rises with
        // it, iOS's default (LLP 1116 D2); `resizes-visual`, the web's
        // default and iOS's opt-out, insets the viewport instead.
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
        // @ref LLP 1145 D4 — how far ahead a virtualized list's edges fire.
        "reach-margin" => AttrTarget::Prop(p("reachMargin")),
        // @ref LLP 1056 D6 (r3): a canvas's explicit bitmap size, HTML's
        // `width`/`height` content attributes (Contract's are the CSS box).
        "bitmap-width" => AttrTarget::Prop(p("bitmapWidth")),
        "bitmap-height" => AttrTarget::Prop(p("bitmapHeight")),
        // LLP 1100 D12a: getContext's settings.
        "color-space" => AttrTarget::Prop(p("colorSpace")),
        "color-type" => AttrTarget::Prop(p("colorType")),
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
        "href" | "target" => AttrTarget::Prop(p(name)),
        "disabled" => AttrTarget::Prop(p("disabled")),
        "inert" => AttrTarget::Prop(p("inert")),
        "readonly" => AttrTarget::InvertedBoolProp(p("editable")),
        "lang" => AttrTarget::Prop(p("lang")),
        "src" => AttrTarget::Prop(p("src")),
        "sandbox" => AttrTarget::Prop(p("sandbox")),
        // @ref LLP 1020.002 D1, §9 — a top-level guest's own storage, and
        // `WKWebView`'s words under the platform's key (`frames.rs`).
        "partition" => AttrTarget::Prop(p("partition")),
        "apple.allowsBackForwardNavigationGestures" => {
            AttrTarget::Prop(p("appleAllowsBackForwardNavigationGestures"))
        }
        "apple.allowsMagnification" => AttrTarget::Prop(p("appleAllowsMagnification")),
        "apple.inactiveSchedulingPolicy" => AttrTarget::Prop(p("appleInactiveSchedulingPolicy")),
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
        "aria-controls" => AttrTarget::Prop(p("accessibilityControls")), // LLP 1075.003 §3.7: a tab names its tabpanel.
        "aria-selected" => AttrTarget::Prop(p("accessibilitySelected")),
        "aria-expanded" => AttrTarget::Prop(p("accessibilityExpanded")),
        "aria-pressed" => AttrTarget::Prop(p("accessibilityPressed")),
        "aria-disabled" => AttrTarget::Prop(p("accessibilityDisabled")),
        "aria-modal" => AttrTarget::Prop(p("accessibilityModal")),
        "aria-hidden" => AttrTarget::Prop(p("accessibilityElementsHidden")),
        "aria-invalid" => AttrTarget::Prop(p("accessibilityInvalid")), // onboarding F22, spreadsheet F20
        "aria-describedby" => AttrTarget::Prop(p("accessibilityDescribedBy")),
        // ARIA's name by reference (ledger2 Rough 3): the ids, space-separated,
        // of the elements whose text names this one, before `aria-label`.
        "aria-labelledby" => AttrTarget::Prop(p("accessibilityLabelledBy")),
        // HTML's `tabindex`, no alias (LLP 1088 D7.3): present makes a box
        // focusable, `>= 0` a Tab stop; absent is never `0`.
        "tabindex" => AttrTarget::Prop(p("tabIndex")),
        "aria-required" => AttrTarget::Prop(p("accessibilityRequired")),
        "aria-haspopup" => AttrTarget::Prop(p("accessibilityHasPopup")),
        "aria-current" => AttrTarget::Prop(p("accessibilityCurrent")), // Depot: a nav link's page
        // @ref LLP 1116 D8 — a range role's value (`progressbar`, `slider`,
        // `meter`, …), numbers written as HTML writes them (`aria_values`).
        "aria-valuenow" => AttrTarget::Prop(p("accessibilityValueNow")),
        "aria-valuemin" => AttrTarget::Prop(p("accessibilityValueMin")),
        "aria-valuemax" => AttrTarget::Prop(p("accessibilityValueMax")),
        "aria-valuetext" => AttrTarget::Prop(p("accessibilityValueText")),
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
        "feOrder" => AttrTarget::Prop(p("feOrder")),
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
        "markerWidth" => AttrTarget::Prop(p("markerWidth")),
        "markerHeight" => AttrTarget::Prop(p("markerHeight")),
        "refX" => AttrTarget::Prop(p("refX")),
        "refY" => AttrTarget::Prop(p("refY")),
        "orient" => AttrTarget::Prop(p("orient")),
        "markerUnits" => AttrTarget::Prop(p("markerUnits")),
        // @ref LLP 1055.000 D11 — SVG text's positions (`x`, `y`, `dx`,
        // `dy` on `text`/`tspan` lower to these) and its alignment.
        "textX" => AttrTarget::Prop(p("textX")),
        "textY" => AttrTarget::Prop(p("textY")),
        "textDx" => AttrTarget::Prop(p("textDx")),
        "textDy" => AttrTarget::Prop(p("textDy")),
        "x1" => AttrTarget::Prop(p("x1")),
        "y1" => AttrTarget::Prop(p("y1")),
        "x2" => AttrTarget::Prop(p("x2")),
        "y2" => AttrTarget::Prop(p("y2")),
        _ => return None,
    })
}

/// The name an old nickname or DOM camelCase spelling became (LLP 1017 §8.1), so the refusal of
/// `size=13` says `font-size`. Nothing here is accepted; it is only named.
pub fn renamed(old: &str) -> Option<&'static str> {
    Some(match old {
        "size" | "fontSize" => "font-size",
        "weight" | "fontWeight" => "font-weight",
        "fontStyle" => "font-style",
        "fontStretch" => "font-stretch",
        "fontVariationSettings" => "font-variation-settings",
        "fontOpticalSizing" => "font-optical-sizing",
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
        "dragTimeline" | "drag-timeline" => "-exact-drag-timeline",
        "animationTimeline" => "animation-timeline",
        "animationRange" => "animation-range",
        "animationTrigger" | "animation-trigger" => "-exact-animation-trigger",
        "timelineScope" => "timeline-scope",
        "transformOrigin" => "transform-origin",
        "align" | "alignItems" => "align-items",
        "alignSelf" => "align-self",
        "boxSizing" => "box-sizing",
        "fit" | "objectFit" | "resizeMode" => "object-fit",
        // React Native's names for CSS's text rows.
        "numberOfLines" => "line-clamp",
        "ellipsizeMode" => "text-overflow",
        "textTransform" => "text-transform",
        "fontVariant" => "font-variant-numeric",
        // UIKit's, SwiftUI's and React Native's shrink-to-fit (LLP 1116.001 D1.4).
        "adjustsFontSizeToFitWidth"
        | "adjustsFontSizeToFit"
        | "minimumScaleFactor"
        | "minimumFontScale" => "-exact-fit",
        "justify" | "justifyContent" => "justify-content",
        "overflowX" => "overflow-x",
        "overflowY" => "overflow-y",
        "zIndex" => "z-index",
        "accessibilityOrientation" => "aria-orientation",
        "label" | "accessibilityLabel" => "aria-label",
        "tabIndex" => "tabindex",
        "hint" | "accessibilityHint" => "aria-description",
        "headingLevel" => "aria-level",
        "inputMode" | "keyboardType" => "inputmode",
        "enterKeyHint" | "returnKeyType" => "enterkeyhint",
        "autoComplete" | "textContentType" | "autoCompleteType" => "autocomplete",
        "viewportFit" | "safeArea" | "safeAreaView" => "viewport-fit",
        "statusBarStyle" | "barStyle" | "StatusBar" => "status-bar-style",
        // Android's view attribute, UIKit's idle timer, Expo's hook (LLP 1125 D11).
        "keepScreenOn" | "keep-screen-on" | "idleTimerDisabled" | "keepAwake" | "wakeLock" => {
            "wakelock"
        }
        // The web's call, Android's activity attribute, UIKit's (LLP 1133.029 D1).
        "screenOrientation"
        | "screen-orientation"
        | "lockOrientation"
        | "orientation"
        | "requestedOrientation"
        | "supportedInterfaceOrientations" => "orientationlock",
        "interactiveWidget" | "keyboardAvoidingView" | "keyboardAvoiding" => "interactive-widget",
        "secureTextEntry" => "type",
        "onClick" | "onPress" => "press",
        // Web MIDI's handler properties (LLP 1125 D8).
        "onMIDIMessage" | "onmidimessage" | "midiMessage" => "midimessage",
        "midiInputs" | "midi-inputs" => "midiinputs",
        // LLP 1081 D2: the names Exact invents are spelled `-exact-`.
        "tint-color" => "-exact-tint-color",
        "exit-animation" => "-exact-exit-animation",
        "layout-transition" => "-exact-layout-transition",
        "press-scale" => "-exact-press-scale",
        "symbol-rendering" => "-exact-symbol-rendering",
        "symbol-palette" => "-exact-symbol-palette",
        "symbol-value" => "-exact-symbol-value",
        "symbol-effect" => "-exact-symbol-effect",
        "press-haptic" => "-exact-press-haptic",
        // LLP 1061 D7: the pressed paint, and React Native's
        // `TouchableOpacity`/`TouchableHighlight` props that set it.
        "press-opacity" | "activeOpacity" => "-exact-press-opacity",
        "press-background-color" | "underlayColor" => "-exact-press-background-color",
        "press-color" => "-exact-press-color",
        "content-transition" => "-exact-content-transition",
        "scroll-edge-effect" => "-exact-scroll-edge-effect",
        "hover-effect" => "-exact-hover-effect",
        "smart-invert" => "-exact-smart-invert",
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
                format!("`{name}` is a virtualized list's; it goes on `list virtualized=true` (a chat's transcript: `contract vocab chat-thread`)");
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
    // LLP 1069.010 D6: the document has unsaved changes (a declared deviation).
    "edited",
];
/// Suggest one unambiguous single-edit spelling from the existing attribute
/// lookup. No second vocabulary is maintained, and this never admits an alias.
pub(crate) fn similar_attr(name: &str, style_only: bool) -> Option<String> {
    similar(name, |candidate| match attr(candidate) {
        // Lowering's own row for `:active`'s opacity (LLP 1121 D8).
        _ if candidate == crate::states::PRESS_OPACITY_TO => false,
        Some(AttrTarget::Styles(_) | AttrTarget::Flex | AttrTarget::Shorthand) => true,
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
        "span" | "p" | "strong" | "em" | "b" | "i" => "text is `text`",
        "label" => {
            "a label is `text` beside its field, and the field is named by `aria-label` (or `aria-labelledby` with the text's `id`)"
        }
        "heading" => "a heading is `h1` (to `h6` for the level)",
        // The app farm's guesses (round 1): each names the Contract form
        // that does the job.
        "small" | "code" | "pre" | "paragraph" | "subtitle" => {
            "text is `text`, styled by its attributes (`font-size`, `font-family`, `white-space`)"
        }
        "form" => {
            "there is no `form`: a field's Enter is its `submit` (`input … submit=save`), and a `button`'s `press` acts"
        }
        "table" | "thead" | "tbody" | "tr" | "td" | "th" | "grid" => {
            "a table or grid is `view display=\"grid\"` with `grid-template-columns`, or a `column` of `row`s"
        }
        "details" | "summary" => {
            "there is no `details`: keep `open` in state, show the body `if open`, and toggle it from a `button`"
        }
        "hstack" | "vstack" | "stack" | "flex" => "a horizontal stack is `row`, a vertical one `column`",
        "container" | "card" => "a box is `view` (or `column`, `row`), styled by a `style` declared in this file",
        "spacer" => "a spacer is `view flex=1` in a `row` or `column`",
        "divider" => "a divider is `hr`",
        "br" => "a line break is a new `text`, or `\\n` in a string with `white-space=\"pre-line\"`",
        "icon" => "an icon is `image \"symbol:<role>\"`, or an `svg`",
        "img" | "picture" => "an image is `image`",
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
                        || crate::overflow::Overflow::of(&rows, "visible").may_scroll_y();
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

/// How an element is a flex or grid container, if it is: its tag, or a
/// literal `display` (LLP 1093 §1).
pub(crate) fn flex_container<'a>(tag: &'a str, attrs: &[contract_syntax::Attr]) -> Option<&'a str> {
    let display = attrs
        .iter()
        .rev()
        .find(|a| a.name == "display")
        .and_then(|a| match &a.value {
            contract_syntax::Expr::Str(v, _) => Some(v.as_str()),
            _ => None,
        });
    match display {
        Some("flex" | "grid" | "inline-flex" | "inline-grid") => {
            Some("a `display` of flex or grid")
        }
        Some(_) => None,
        None => matches!(tag, "column" | "row").then_some(tag),
    }
}

/// @ref LLP 1093 §1 — a flex or grid container is never a multi-column one:
/// CSS ignores the rows there, so they are refused rather than dropped.
pub(crate) fn multicol_on_flex(
    tag: &str,
    a: &contract_syntax::Attr,
) -> Result<(), crate::LowerError> {
    let multicol = matches!(
        a.name.as_str(),
        "columns"
            | "column-count"
            | "column-width"
            | "column-fill"
            | "column-rule"
            | "column-rule-width"
            | "column-rule-style"
            | "column-rule-color"
    );
    if multicol {
        return crate::err("lower-attr-tag", format!("`{}` makes a block a multi-column container, and `{tag}` makes a flex or grid container, where CSS ignores it; write `view` (a block) for columns", a.name), a.span);
    }
    Ok(())
}

impl crate::Lowerer<'_> {
    /// @ref LLP 1115 §3, LLP 1115.000 D3.5 — a link's unsaid colour is the
    /// platform's link role (the tint on iOS, `linkColor` on macOS, the
    /// accent on the web), as the UA's `a:link` is: a run with an `href` in
    /// its paragraph, and a block `link` of text (a `tel:` or `mailto:`
    /// value, a "Learn more"), which a hand-built screen draws in the tint.
    /// A `link` holding a row or a column, or given a `role`, is its
    /// author's row or button and keeps inheriting. Pushed before the
    /// author's rows, so a `color` on the link, the run or a class wins. A
    /// terminal entry's block link is left as it is: what a terminal shows
    /// for a link is the terminal's to decide (`QUEUE.md`).
    ///
    /// An `href` that turns out empty is no link (`Props::link` is how a
    /// host reads the value), so the row follows the value: a computed
    /// `href` chooses between the role and `inherit` as it changes, and a
    /// written `href=""` gets no row. A package that writes `href=r.href`
    /// on every run of a paragraph, empty for plain prose, then colours
    /// only its links.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn link_color(
        &mut self,
        tag: &str,
        parent_tag: Option<&str>,
        attrs: &[contract_syntax::Attr],
        children: &[contract_syntax::Node],
        scope: &crate::Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
    ) {
        use contract_syntax::Expr;
        let Some(chosen) = self.link_colour(tag, parent_tag, attrs, children) else {
            return;
        };
        let link = match &chosen {
            Expr::Str(word, _) if word == "LinkText" => self.fixed(true, "LinkText"),
            // A value the attribute's own lowering refuses is said there.
            _ => match self.expr_code(&chosen, scope, locals) {
                Ok(code) => code,
                Err(_) => return,
            },
        };
        bindings.push(crate::style_row(StyleId::TextColor, link, crate::IMPLIED));
    }

    /// The colour a link's text takes when its author writes none: `LinkText`,
    /// or `inherit` where its computed target is empty; `None` for a node that
    /// is no link's text (a state part's base, LLP 1121 D8, reads it too).
    pub(crate) fn link_colour(
        &self,
        tag: &str,
        parent_tag: Option<&str>,
        attrs: &[contract_syntax::Attr],
        children: &[contract_syntax::Node],
    ) -> Option<contract_syntax::Expr> {
        use contract_syntax::{BinOp, Expr, Node, TemplatePart};
        // Texts and images alone, an `if` or a `for` of them included.
        fn words(node: &Node) -> bool {
            match node {
                Node::Element { tag, .. } => tag == "text" || tag == "image",
                Node::When {
                    then, otherwise, ..
                } => then.iter().chain(otherwise).all(words),
                Node::Each { body, .. } => body.iter().all(words),
                Node::Match { some, none, .. } => some.1.iter().chain(none).all(words),
                Node::Use { .. } | Node::Children { .. } => false,
            }
        }
        let has = |name: &str| attrs.iter().any(|a| a.name == name);
        let href = attrs.iter().rev().find(|a| a.name == "href");
        let run = tag == "text" && parent_tag == Some("text") && href.is_some();
        let block = tag == "link"
            && self.profile != crate::Profile::Terminal
            && !has("role")
            && !children.is_empty()
            && children.iter().all(words);
        if !(run || block) {
            return None;
        }
        let span = href.map_or(contract_syntax::Span::default(), |a| a.value.span());
        let word = |w: &str| Expr::Str(w.into(), span);
        Some(match href.map(|a| &a.value) {
            Some(Expr::Str(to, _)) if to.is_empty() => return None,
            // A written target, a template with words of its own, or a
            // block `link` that is its `press`: a link whatever happens.
            None | Some(Expr::Str(..)) => word("LinkText"),
            Some(Expr::Template(parts, _))
                if parts
                    .iter()
                    .any(|p| matches!(p, TemplatePart::Text(t) if !t.is_empty())) =>
            {
                word("LinkText")
            }
            Some(to) => {
                let empty = Expr::Binary(BinOp::Eq, Box::new(to.clone()), Box::new(word("")), span);
                Expr::Ternary(
                    Box::new(empty),
                    Box::new(word("inherit")),
                    Box::new(word("LinkText")),
                    span,
                )
            }
        })
    }
}
