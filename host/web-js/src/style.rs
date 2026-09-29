//! Static rows → the live host's element parts, and the names a dynamic
//! binding writes at run time.
//!
//! Every node site of the plan becomes a view of one kernel tree (regions
//! flattened: every arm's children under the region's parent node), with
//! its literal bindings applied through the runner's own bridge
//! (`exact_runner::bridge`). `exact_web::host::template::parts` then gives
//! the tag, props and CSS the live host would compute. A binding that is an
//! expression is left out, and emitted as an effect instead.

use exact_kernel::{
    Kernel, NodeType, Op, PropId, PropKind, PropValue, StyleCodec, StyleId, ViewId,
};
use exact_plan::{BindingKind, Opcode, Plan};
use exact_runner::bridge;
use exact_runner::vm::instructions;
use exact_web::host::template::{self, Parts};

/// A literal binding's value, if the code is one push and `Return`.
pub fn literal(plan: &Plan, code: &[u8]) -> Option<exact_plan::Value> {
    let ins: Vec<_> = instructions(code).collect::<Result<_, _>>().ok()?;
    match ins.as_slice() {
        [a, r] if r.op == Opcode::Return => match a.op {
            Opcode::Number => Some(exact_plan::Value::Number(a.number)),
            Opcode::Bool => Some(exact_plan::Value::Bool(a.args[0] != 0)),
            Opcode::Str => Some(exact_plan::Value::str(
                plan.str(exact_plan::StrId(a.args[0] as u32)),
            )),
            _ => None,
        },
        _ => None,
    }
}

/// Props whose presence decides the element's tag: a dynamic one is given a
/// sample value when the tag is computed (the live host fixes the tag from
/// the value at creation; the JS target from its presence).
fn decides_tag(p: PropId) -> bool {
    matches!(p, PropId::Href | PropId::SemanticTag)
}

fn sample(kind: PropKind) -> PropValue {
    match kind {
        PropKind::Str => PropValue::Str("x".into()),
        PropKind::Bool => PropValue::Bool(true),
        PropKind::Int => PropValue::Int(1),
        PropKind::Float => PropValue::Float(1.0),
    }
}

/// The plan's element children of each node: node sites, with region arms
/// flattened into their parent.
pub fn element_children(plan: &Plan, sites: &crate::emit::Sites) -> Vec<Vec<u32>> {
    let mut out = vec![Vec::new(); plan.nodes.len()];
    fn flatten(
        plan: &Plan,
        sites: &crate::emit::Sites,
        list: &[crate::emit::Site],
        into: &mut Vec<u32>,
    ) {
        for s in list {
            match s {
                crate::emit::Site::Node(n) => into.push(*n),
                crate::emit::Site::Region(r) => {
                    for arm in plan.regions[*r as usize].arms.iter() {
                        flatten(plan, sites, sites.of_arm(arm.0), into);
                    }
                }
            }
        }
    }
    for (i, list) in out.iter_mut().enumerate() {
        flatten(plan, sites, sites.of_node(i as u32), list);
    }
    out
}

/// Every node's parts, by node index (`None` for a head, which is no element).
pub fn project(
    plan: &Plan,
    sites: &crate::emit::Sites,
    warnings: &mut Vec<String>,
) -> Result<Vec<Option<Parts>>, String> {
    let mut kernel: Kernel = template::kernel();
    let mut ops = Vec::new();
    let view = |i: usize| -> ViewId { i as u32 + 1 };
    let stacks = plan.stacks.len();
    for (i, node) in plan.nodes.iter().enumerate() {
        let node_type = NodeType::from_wire(node.node_type).ok_or("unknown node type")?;
        ops.push(Op::CreateView {
            id: view(i),
            node_type,
        });
        let mut patch = exact_kernel::StyleProps::default();
        let mut styled = false;
        for b in node.bindings.iter() {
            let row = plan.binding(b);
            let code = plan.code(row.expr);
            match (row.kind, literal(plan, code)) {
                (BindingKind::Prop, Some(v)) => {
                    let (prop, value) =
                        bridge::prop_value(row.id, &v).map_err(|e| format!("node {i}: {e:?}"))?;
                    ops.push(Op::SetProp {
                        id: view(i),
                        prop,
                        value,
                    });
                }
                (BindingKind::Prop, None) => {
                    let prop = PropId::from_wire(row.id).ok_or("unknown prop")?;
                    if decides_tag(prop) {
                        ops.push(Op::SetProp {
                            id: view(i),
                            prop,
                            value: sample(prop.kind()),
                        });
                    }
                }
                (BindingKind::Style, Some(v)) => {
                    bridge::set_style(&mut patch, row.id, &v, stacks)
                        .map_err(|e| format!("node {i}: {e:?}"))?;
                    styled = true;
                }
                (BindingKind::Style, None) => {}
            }
        }
        if styled {
            ops.push(Op::SetStyle {
                id: view(i),
                patch: Box::new(patch),
            });
        }
    }
    for (i, children) in element_children(plan, sites).into_iter().enumerate() {
        if !children.is_empty() {
            ops.push(Op::SetChildren {
                id: view(i),
                children: children.into_iter().map(|c| view(c as usize)).collect(),
            });
        }
    }
    ops.push(Op::AttachRoot {
        id: view(sites.root as usize),
    });
    kernel
        .apply(0, 1, &ops)
        .map_err(|e| format!("kernel refused the static tree: {e:?}"))?;
    let mut out = Vec::with_capacity(plan.nodes.len());
    for (i, node) in plan.nodes.iter().enumerate() {
        if NodeType::from_wire(node.node_type) == Some(NodeType::Head) {
            out.push(None);
            continue;
        }
        let mut parts = template::parts(&kernel, plan, view(i)).ok_or("a view the kernel lost")?;
        // A sampled prop decided the tag; its value is the effect's.
        for b in node.bindings.iter() {
            let row = plan.binding(b);
            if row.kind == BindingKind::Prop && literal(plan, plan.code(row.expr)).is_none() {
                if let Some(prop) = PropId::from_wire(row.id).filter(|p| decides_tag(*p)) {
                    if let Ok(name) = prop_name(NodeType::from_wire(node.node_type).unwrap(), prop)
                    {
                        parts.props.remove(&name);
                    }
                }
            }
        }
        for (row, reason) in &parts.skipped {
            warnings.push(format!(
                "node {i}: style row {row} skipped by the web host: {reason}"
            ));
        }
        out.push(Some(parts));
    }
    Ok(out)
}

/// The DOM prop name the live host gives `prop` on a node of `node_type`,
/// found by projecting a lone node with and without a sample value.
pub fn prop_name(node_type: NodeType, prop: PropId) -> Result<String, String> {
    let keys = |with: bool| -> Result<Vec<String>, String> {
        let mut k = template::kernel();
        let id: ViewId = 1;
        let mut ops = vec![Op::CreateView { id, node_type }, Op::AttachRoot { id }];
        if with {
            ops.push(Op::SetProp {
                id,
                prop,
                value: sample(prop.kind()),
            });
        }
        k.apply(0, 1, &ops).map_err(|e| format!("{e:?}"))?;
        // The plan only matters for fonts, which a lone node has none of.
        let plan = exact_plan::builder::PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
            .finish()
            .map_err(|e| e.to_string())?;
        Ok(template::parts(&k, &plan, id)
            .ok_or("lost view")?
            .props
            .keys()
            .cloned()
            .collect())
    };
    let before = keys(false)?;
    let after = keys(true)?;
    after
        .into_iter()
        .find(|k| !before.contains(k))
        .ok_or_else(|| {
            format!(
                "prop {} has no DOM name on {}",
                prop.name(),
                node_type.name()
            )
        })
}

/// A dynamic style row's CSS property and the unit a number takes, read
/// from the web host's own `css_text` for a sample value (`7` → `7px`,
/// `7deg` or `7`), so the unit rule is css.rs's, not a copy. Text values
/// (enums, `auto`, `N%`, colours as `#rrggbb[aa]`) are written as the
/// author wrote them; the browser parses them as the kernel does. Rows that
/// are not one declaration each are refused, never guessed.
pub fn style_row(id: u16) -> Result<(String, String), String> {
    let row = StyleId::from_bit(id as u32).ok_or("unknown style row")?;
    // `translate` is one declaration of the author's two lengths (css.rs
    // `lowered`): written as the author wrote them.
    if row == StyleId::Translate {
        return Ok(("translate".into(), String::new()));
    }
    // `line-height` is CSS's own (kernel `LineHeight::css`): a number is a
    // multiple of the font size, unitless; a length is written with its unit;
    // `aspect-ratio` too (kernel `Ratio::css`): a number is `n / 1`.
    if row == StyleId::LineHeight || row == StyleId::AspectRatio {
        return Ok((css_property(row), String::new()));
    }
    if !matches!(
        row.codec(),
        StyleCodec::Dimension
            | StyleCodec::Enum
            | StyleCodec::ColorValue
            | StyleCodec::Rgba8
            | StyleCodec::KeywordColor
            | StyleCodec::F32
            | StyleCodec::U8
            | StyleCodec::U16
            | StyleCodec::U32
            | StyleCodec::I32
    ) || matches!(
        row,
        StyleId::ShadowOffset
            | StyleId::ShadowRadius
            | StyleId::ShadowColor
            | StyleId::ShadowOpacity
            | StyleId::FontFamily
            | StyleId::LineClamp
            | StyleId::PressScale
            | StyleId::FontVariantNumeric
            | StyleId::BackdropBlur
    ) {
        return Err(format!(
            "a dynamic `{}` ({:?}) is not one declaration; not in the JS target",
            row.name(),
            row.codec()
        ));
    }
    let sample = |v: exact_kernel::StyleValue| {
        let mut p = exact_kernel::StyleProps::default();
        p.set_dynamic(row, &v).ok()?;
        let (text, _) = exact_web::css::css_text(&p, &[]);
        let (name, value) = text.trim_end_matches(';').split_once(':')?;
        Some((name.to_string(), value.to_string()))
    };
    if let Some((name, value)) = sample(exact_kernel::StyleValue::Number(7.0)) {
        if let Some(unit) = value.strip_prefix('7') {
            return Ok((name, unit.to_string()));
        }
    }
    let name = sample(exact_kernel::StyleValue::Text("#000000".into()))
        .map(|(n, _)| n)
        .unwrap_or_else(|| css_property(row));
    Ok((name, String::new()))
}

/// `host/web/src/css.rs` `property`: the row's name with `-` for `_`, but
/// for the few spelled there.
fn css_property(id: StyleId) -> String {
    let name = match id {
        StyleId::TextColor => return "color".into(),
        StyleId::TintColor => return "--exact-tint".into(),
        StyleId::PositionType => return "position".into(),
        StyleId::BackdropBlur => return "backdrop-filter".into(),
        StyleId::SvgMask => return "mask".into(),
        id => id.name(),
    };
    for (prefix, suffix) in [
        ("border_radius_", "-radius"),
        ("border_width_", "-width"),
        ("border_style_", "-style"),
        ("border_color_", "-color"),
    ] {
        if let Some(side) = name.strip_prefix(prefix) {
            return format!("border-{}{suffix}", side.replace('_', "-"));
        }
    }
    name.replace('_', "-")
}
