//! The tag and attribute table: Contract's view vocabulary onto the kernel's.
//!
//! @ref LLP 1004 D2 (kernel ordinals from `exact-kernel`, never redeclared)
//! @ref `rules/RULES.md` §Scope (the web is the standard: every row here is a
//! CSS property or an HTML attribute by its CSS/HTML name)
//! @ref LLP 1017 §8.1 (the literal CSS names, hyphens as grammar, no aliases;
//! `testId` the one Exact-named attribute)
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
        "main" | "header" | "nav" | "section" | "footer" | "article" | "aside" => {
            view(vec![], vec![(p("semanticTag"), leak(name))])
        }
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
        "image" => Tag {
            node_type: NodeType::Image,
            fixed_styles: vec![],
            fixed_props: vec![],
            positional: Some(p("imageSource")),
        },
        _ => return None,
    })
}

fn leak(name: &str) -> &'static str {
    // The semantic tag set is closed above; leaking a handful of short
    // strings once per process is the simplest way to hand out `&'static`.
    Box::leak(name.to_string().into_boxed_str())
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
}

/// The type a prop attribute takes, by the kernel prop's name.
pub fn prop_ty(prop: PropId) -> PropTy {
    match prop.name() {
        "disabled" => PropTy::Bool,
        "accessibilityHeadingLevel" => PropTy::Int,
        _ => PropTy::Str,
    }
}

/// Look up an attribute.
pub fn attr(name: &str) -> Option<AttrTarget> {
    let styles = |names: &[&str]| AttrTarget::Styles(names.iter().map(|n| s(n)).collect());
    Some(match name {
        // handlers (the web's events, LLP 1005 §3)
        "press" => AttrTarget::Handler("press"),
        "change" => AttrTarget::Handler("change"),
        "hover" => AttrTarget::Handler("hover"),
        "focus" => AttrTarget::Handler("focus"),
        "blur" => AttrTarget::Handler("blur"),
        "key" => AttrTarget::Handler("key"),
        "submit" => AttrTarget::Handler("submit"),
        // the canvas's surface (LLP 1009 D3)
        "surface" => AttrTarget::Surface,
        // props (HTML and ARIA attribute names; `testId` is Exact's)
        "testId" => AttrTarget::Prop(p("testId")),
        "aria-label" => AttrTarget::Prop(p("accessibilityLabel")),
        "aria-description" => AttrTarget::Prop(p("accessibilityHint")),
        "aria-level" => AttrTarget::Prop(p("accessibilityHeadingLevel")),
        "role" => AttrTarget::Prop(p("accessibilityRole")),
        "placeholder" => AttrTarget::Prop(p("placeholder")),
        "type" => AttrTarget::Prop(p("type")),
        // HTML's attribute is `inputmode`; the kernel's prop keeps the DOM
        // property's spelling, as the schema does for every prop.
        "inputmode" => AttrTarget::Prop(p("inputMode")),
        // The viewport meta's `viewport-fit=cover`, read from the first root
        // (LLP 1008 §9): the layout viewport becomes the whole screen and
        // `env(safe-area-inset-*)` lengths carry the insets.
        "viewport-fit" => AttrTarget::Prop(p("viewportFit")),
        "value" => AttrTarget::Prop(p("value")),
        "href" => AttrTarget::Prop(p("href")),
        "disabled" => AttrTarget::Prop(p("disabled")),
        "lang" => AttrTarget::Prop(p("lang")),
        // style rows, by their CSS property names
        "font-size" => styles(&["font_size"]),
        "font-weight" => styles(&["font_weight"]),
        "color" => styles(&["text_color"]),
        "background-color" => styles(&["background_color"]),
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
        "z-index" => styles(&["z_index"]),
        "transition" => styles(&["transition"]),
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
        "label" | "accessibilityLabel" => "aria-label",
        "hint" | "accessibilityHint" => "aria-description",
        "headingLevel" => "aria-level",
        "inputMode" | "keyboardType" => "inputmode",
        "viewportFit" | "safeArea" | "safeAreaView" => "viewport-fit",
        "secureTextEntry" => "type",
        "onClick" | "onPress" => "press",
        "onChange" | "onChangeText" => "change",
        "className" | "class" | "style" => return None,
        _ => return None,
    })
}
