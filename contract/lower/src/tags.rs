//! The tag and attribute table: Contract's view vocabulary onto the kernel's.
//!
//! @ref LLP 1004 D2 (kernel ordinals from `exact-kernel`, never redeclared)
//! @ref `rules/RULES.md` §Scope (the web is the standard: every row here is a
//! CSS property or an HTML attribute by its CSS/HTML name)
//!
//! A tag names a kernel node type plus fixed rows (`column` is a `View` with
//! `flex-direction: column`); an attribute names one or more kernel style rows
//! or one prop, or is a handler. Anything not in the table is a rejection
//! with a stable id — there is no fallback attribute.

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

/// Look up an attribute.
pub fn attr(name: &str) -> Option<AttrTarget> {
    let styles = |names: &[&str]| AttrTarget::Styles(names.iter().map(|n| s(n)).collect());
    Some(match name {
        // handlers
        "press" => AttrTarget::Handler("press"),
        "change" => AttrTarget::Handler("change"),
        // props (HTML attribute names)
        "testId" => AttrTarget::Prop(p("testId")),
        "label" => AttrTarget::Prop(p("accessibilityLabel")),
        "hint" => AttrTarget::Prop(p("accessibilityHint")),
        "role" => AttrTarget::Prop(p("accessibilityRole")),
        "placeholder" => AttrTarget::Prop(p("placeholder")),
        "value" => AttrTarget::Prop(p("value")),
        "href" => AttrTarget::Prop(p("href")),
        "disabled" => AttrTarget::Prop(p("disabled")),
        "lang" => AttrTarget::Prop(p("lang")),
        "headingLevel" => AttrTarget::Prop(p("accessibilityHeadingLevel")),
        // style rows (CSS property names; the camelCase spelling is the DOM's)
        "size" | "fontSize" => styles(&["font_size"]),
        "weight" | "fontWeight" => styles(&["font_weight"]),
        "color" => styles(&["text_color"]),
        "background" | "backgroundColor" => styles(&["background_color"]),
        "opacity" => styles(&["opacity"]),
        "letterSpacing" => styles(&["letter_spacing"]),
        "lineHeight" => styles(&["line_height"]),
        "textAlign" => styles(&["text_align"]),
        "gap" => styles(&["row_gap", "column_gap"]),
        "rowGap" => styles(&["row_gap"]),
        "columnGap" => styles(&["column_gap"]),
        "padding" => styles(&[
            "padding_top",
            "padding_right",
            "padding_bottom",
            "padding_left",
        ]),
        "paddingTop" => styles(&["padding_top"]),
        "paddingRight" => styles(&["padding_right"]),
        "paddingBottom" => styles(&["padding_bottom"]),
        "paddingLeft" => styles(&["padding_left"]),
        "margin" => styles(&["margin_top", "margin_right", "margin_bottom", "margin_left"]),
        "marginTop" => styles(&["margin_top"]),
        "marginRight" => styles(&["margin_right"]),
        "marginBottom" => styles(&["margin_bottom"]),
        "marginLeft" => styles(&["margin_left"]),
        "radius" | "borderRadius" => styles(&[
            "border_radius_top_left",
            "border_radius_top_right",
            "border_radius_bottom_right",
            "border_radius_bottom_left",
        ]),
        "borderWidth" => styles(&[
            "border_width_top",
            "border_width_right",
            "border_width_bottom",
            "border_width_left",
        ]),
        "borderBottomWidth" => styles(&["border_width_bottom"]),
        "borderTopWidth" => styles(&["border_width_top"]),
        "borderColor" => styles(&[
            "border_color_top",
            "border_color_right",
            "border_color_bottom",
            "border_color_left",
        ]),
        "width" => styles(&["width"]),
        "height" => styles(&["height"]),
        "minWidth" => styles(&["min_width"]),
        "minHeight" => styles(&["min_height"]),
        "maxWidth" => styles(&["max_width"]),
        "maxHeight" => styles(&["max_height"]),
        "flex" => AttrTarget::Flex,
        "flexShrink" => styles(&["flex_shrink"]),
        "flexBasis" => styles(&["flex_basis"]),
        "wrap" | "flexWrap" => styles(&["flex_wrap"]),
        "direction" | "flexDirection" => styles(&["flex_direction"]),
        "display" => styles(&["display"]),
        "align" | "alignItems" => styles(&["align_items"]),
        "alignSelf" => styles(&["align_self"]),
        "boxSizing" => styles(&["box_sizing"]),
        "justify" | "justifyContent" => styles(&["justify_content"]),
        "position" => styles(&["position_type"]),
        "top" => styles(&["top"]),
        "left" => styles(&["left"]),
        "right" => styles(&["right"]),
        "bottom" => styles(&["bottom"]),
        "overflow" => styles(&["overflow_x", "overflow_y"]),
        "zIndex" => styles(&["z_index"]),
        "transition" => styles(&["transition"]),
        "scale" => styles(&["scale"]),
        "rotate" => styles(&["rotate"]),
        _ => return None,
    })
}
