//! Paint facts carried by templates and bindings. Isolation is always decided
//! on the instance tree by paint.js, never on the flattened template tree.
use exact_kernel::{paint_order, NodeFacts, NodeType, PropId, SortedMap, StyleId, StyleProps};
use exact_plan::{BindingKind, BindingsRow, Plan};

/// The authored integer, shared by the CSS write and the paint fact.
pub const Z_INDEX: &str =
    "v=>v!=null&&v!==\"auto\"?Math.max(-2147483645,Math.min(2147483645,Math.trunc(Number(v)))):null";

const MOTION: [StyleId; 4] = [
    StyleId::Transition,
    StyleId::Animation,
    StyleId::LayoutTransition,
    StyleId::ExitAnimation,
];

fn own(style: &StyleProps) -> paint_order::Own {
    paint_order::own_from(paint_order::Facts {
        style,
        props: &Default::default(),
        kind: NodeType::View,
        root: false,
        parent_display: None,
        beside_exclusion: false,
        holds_layout_transition: false,
    })
}

pub fn attributes(node: &NodeFacts<'_>, attrs: &mut SortedMap<String, String>) {
    let s = node.style;
    let mut authored = s.clone();
    for row in MOTION.into_iter().chain([StyleId::ZIndex]) {
        authored.mask.clear(row);
    }
    authored.position_type = exact_kernel::PositionType::Static;
    let mut motion = s.clone();
    motion.mask = exact_kernel::StyleMask::EMPTY;
    for row in MOTION {
        if s.mask.has(row) {
            motion.mask.set(row);
        }
    }
    motion.position_type = exact_kernel::PositionType::Static;
    motion.mix_blend_mode = exact_kernel::MixBlendMode::Normal;
    motion.isolation = exact_kernel::Isolation::Auto;
    for (name, yes) in [
        ("stack", own(&authored).stacks),
        ("motion", own(&motion).policy),
        (
            "own-isolation",
            s.isolation == exact_kernel::Isolation::Isolate,
        ),
        ("root", node.is_root),
        (
            "outside",
            node.node_type.is_svg_element() || node.node_type.is_metadata(),
        ),
        (
            "flex",
            matches!(
                s.display,
                exact_kernel::Display::Flex | exact_kernel::Display::Grid
            ),
        ),
        ("layout", s.mask.has(StyleId::LayoutTransition)),
        ("wrap", s.wrap_flow == exact_kernel::WrapFlow::Both),
        ("tint", s.mask.has(StyleId::TintColor)),
    ] {
        if yes {
            attrs.insert(format!("data-exact-{name}"), String::new());
        }
    }
    let position = match s.position_type {
        exact_kernel::PositionType::Static => None,
        exact_kernel::PositionType::Relative => Some("relative"),
        exact_kernel::PositionType::Absolute => Some("absolute"),
        exact_kernel::PositionType::Sticky => Some("sticky"),
    };
    if let Some(position) = position {
        attrs.insert("data-exact-position".into(), position.into());
    }
    if s.mask.has(StyleId::ZIndex) {
        attrs.insert(
            "data-exact-zi".into(),
            s.z_index
                .clamp(-paint_order::Z_MAX, paint_order::Z_MAX)
                .to_string(),
        );
    }
    let kind = match node.node_type {
        NodeType::Canvas => "canvas",
        NodeType::Image => "image",
        NodeType::Control => "control",
        NodeType::Text => "text",
        _ => "box",
    };
    attrs.insert("data-exact-kind".into(), kind.into());
    for (prop, name) in PROPS {
        if let Some(v) = node.props.str(prop) {
            attrs.insert(format!("data-exact-{name}"), v.into());
        }
    }
    if node.props.bool(PropId::Disabled) == Some(true) {
        attrs.insert("data-exact-disabled".into(), "true".into());
    }
}

const PROPS: [(PropId, &str); 8] = [
    (PropId::Type, "type"),
    (PropId::ButtonStyle, "button-style"),
    (PropId::ImageSource, "source"),
    (PropId::BackgroundMaterial, "material"),
    (PropId::NavigationPresentation, "navigation"),
    (PropId::SemanticTag, "semantic"),
    (PropId::Popover, "popover"),
    (PropId::Disabled, "disabled"),
];

/// The attribute and its value expression, with `v` the binding's value.
/// The tests are own_from's value tests, in the row's authored vocabulary.
pub fn binding(plan: &Plan, b: &BindingsRow) -> Option<(String, String)> {
    if b.kind == BindingKind::Prop {
        let id = PropId::from_wire(b.id)?;
        return PROPS
            .iter()
            .find(|(p, _)| *p == id)
            .map(|(_, n)| (format!("data-exact-{n}"), "v".into()));
    }
    let id = StyleId::from_bit(b.id as u32)?;
    let boolean = |name: String, test: &str| Some((name, format!("({test})?\"\":null")));
    let fact = |name: &str, test: &str| boolean(format!("data-exact-{name}"), test);
    let test = match id {
        StyleId::PositionType => {
            return Some((
                "data-exact-position".into(),
                "v!=null&&v!==\"static\"?v:null".into(),
            ))
        }
        StyleId::Display => return fact("flex", "v===\"flex\"||v===\"grid\""),
        StyleId::ZIndex => return Some(("data-exact-zi".into(), format!("({Z_INDEX})(v)"))),
        StyleId::WrapFlow => return fact("wrap", "v===\"both\""),
        StyleId::TintColor => return fact("tint", "v!=null"),
        StyleId::Isolation => return fact("own-isolation", "v===\"isolate\""),
        StyleId::LayoutTransition => return fact("layout", "v!=null"),
        StyleId::ExitAnimation => return fact("exit", "v!=null"),
        StyleId::Transition => {
            let properties: Vec<_> = exact_motion::Property::ALL
                .iter()
                .map(|p| p.css_name())
                .chain(["all", "border-color"])
                .collect();
            let names = serde_json::to_string(&properties).unwrap();
            return boolean(
                format!("data-exact-motion-{}", b.id),
                &format!(
                    r#"v!=null&&String(v).split(/,(?![^(]*\))/).some(t=>{{const words=t.trim().split(/\s+(?![^(]*\))/);const p=words.find(w=>{names}.includes(w));return p?["all","opacity","translate","scale","rotate"].includes(p):t.trim()!==""&&t.trim()!=="none"}})"#
                ),
            );
        }
        StyleId::Animation => {
            let names: Vec<_> = plan
                .keyframes
                .iter()
                .filter(|k| {
                    exact_motion::Keyframes::parse(plan.str(k.css)).is_ok_and(|f| {
                        f.0.iter().any(|k| {
                            k.values.iter().any(|(p, _)| {
                                matches!(
                                    p,
                                    exact_motion::Property::Opacity
                                        | exact_motion::Property::Translate
                                        | exact_motion::Property::Scale
                                        | exact_motion::Property::Rotate
                                        | exact_motion::Property::Layout
                                )
                            })
                        })
                    })
                })
                .map(|k| plan.str(k.name))
                .collect();
            return boolean(
                format!("data-exact-motion-{}", b.id),
                &format!(
                    "v!=null&&String(v).split(/[\\s,]+/).some(n=>{}.includes(n))",
                    serde_json::to_string(&names).unwrap()
                ),
            );
        }
        StyleId::Opacity => "v!=null&&Number(v)<1",
        StyleId::PressScale => "v!=null&&Number(v)!==1",
        StyleId::BackdropBlur | StyleId::Perspective => "v!=null&&parseFloat(v)>0",
        StyleId::Translate
        | StyleId::Scale
        | StyleId::Rotate
        | StyleId::Transform
        | StyleId::Filter
        | StyleId::ClipPath
        | StyleId::MaskImage => "v!=null&&String(v).trim()!==\"none\"",
        StyleId::MixBlendMode => "v!=null&&v!==\"normal\"",
        _ => return None,
    };
    boolean(format!("data-exact-stack-{}", b.id), test)
}

pub fn may_layer<'a>(
    plan: &Plan,
    node: &NodeFacts<'_>,
    bindings: impl Iterator<Item = &'a BindingsRow>,
) -> bool {
    let p = paint_order::own_from(paint_order::Facts {
        style: node.style,
        props: node.props,
        kind: node.node_type,
        root: node.is_root,
        parent_display: None,
        beside_exclusion: false,
        holds_layout_transition: false,
    });
    p.positioned
        || p.stacks
        || node.style.mask.has(StyleId::ZIndex)
        || bindings.into_iter().any(|b| {
            crate::style::literal(plan, plan.code(b.expr)).is_none() && binding(plan, b).is_some()
        })
        || node.node_type == NodeType::Text
}
