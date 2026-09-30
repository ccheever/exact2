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
use exact_web::host::layers;
use exact_web::host::template::{self, Parts};

/// A canvas's explicit bitmap size (LLP 1056 D6 r3) as the attributes
/// canvas2d.js reads, as the runner reads the node's props; a dynamic one is
/// refused.
pub fn canvas_bitmap(
    plan: &Plan,
    row: &exact_plan::NodesRow,
) -> Result<Vec<(String, String)>, String> {
    let mut attrs = Vec::new();
    for b in row.bindings.iter().map(|b| plan.binding(b)) {
        let name = match b.id {
            _ if b.kind != BindingKind::Prop => continue,
            id if id == PropId::BitmapWidth as u16 => "data-bitmap-width",
            id if id == PropId::BitmapHeight as u16 => "data-bitmap-height",
            _ => continue,
        };
        match literal(plan, plan.code(b.expr)) {
            Some(exact_plan::Value::Number(n)) => {
                attrs.push((name.into(), (n.max(0.0) as u32).to_string()))
            }
            _ => return Err("a dynamic canvas bitmap size is not in the JS target".into()),
        }
    }
    Ok(attrs)
}

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

/// Whether a binding's code pushes a string `used` accepts: what it can be,
/// as runner/src/uses.rs asks it.
pub fn can_be(plan: &Plan, code: &[u8], used: &dyn Fn(&str) -> bool) -> bool {
    instructions(code).any(|i| {
        i.is_ok_and(|i| i.op == Opcode::Str && used(plan.str(exact_plan::StrId(i.args[0] as u32))))
    })
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
    // `position: relative` where the live host would make it (layers.rs), for
    // any tree the template builds.
    let mut rows = Vec::new();
    for region in plan
        .regions
        .iter()
        .filter(|r| r.kind == exact_plan::RegionKind::Each)
    {
        for arm in region.arms.iter() {
            roots_of(plan, sites, sites.of_arm(arm.0), &mut rows);
        }
    }
    let relative = layers::relatives(&kernel, &[view(sites.root as usize)], &|id| {
        let i = id as usize - 1;
        layers::Dynamic {
            repeated: rows.contains(&(i as u32)),
            ..dynamic(plan, i)
        }
    });
    for id in relative {
        if let Some(parts) = out[id as usize - 1].as_mut() {
            parts.css.push_str("position:relative;");
        }
    }
    Ok(out)
}

/// What node `i`'s dynamic rows and props may bring to the page's painting
/// order, which the template kernel doesn't hold.
fn dynamic(plan: &Plan, i: usize) -> layers::Dynamic {
    let mut d = layers::Dynamic::default();
    for row in plan.nodes[i].bindings.iter().map(|b| plan.binding(b)) {
        if literal(plan, plan.code(row.expr)).is_some() {
            continue;
        }
        let has = |ids: &[StyleId]| ids.iter().any(|s| row.id == *s as u16);
        match row.kind {
            BindingKind::Style if has(&[StyleId::PositionType]) => {
                d.paint.positioned = true;
                d.absolute = true;
            }
            BindingKind::Style if has(&layers::STACKS) => d.paint.stacks = true,
            BindingKind::Style if has(&layers::INSETS) => d.paint.insets = true,
            BindingKind::Prop => match PropId::from_wire(row.id) {
                Some(PropId::BackgroundMaterial | PropId::NavigationKey)
                | Some(PropId::NavigationPresentation) => d.paint.stacks = true,
                Some(PropId::Markup) => d.paint.positioned = true,
                _ => {}
            },
            _ => {}
        }
    }
    d
}

/// The nodes a site list puts at its level: nodes, and a region's arms' own.
fn roots_of(
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
                    roots_of(plan, sites, sites.of_arm(arm.0), into);
                }
            }
        }
    }
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

/// What a dynamic style row writes: one declaration or more, each a CSS
/// property, the unit a number takes, and, for a few, a JavaScript function
/// of the value that gives the declaration's (`null` writes none).
pub struct Write {
    pub name: String,
    pub unit: String,
    pub map: Option<&'static str>,
}

/// A row's value `none` (or the keyword `auto`/`normal`) writes nothing, as
/// css.rs writes no declaration for the row's empty value.
const NONE: &str = "v=>v==null||/^\\s*none\\s*$/i.test(v)?null:v";

fn one(name: impl Into<String>, unit: impl Into<String>) -> Vec<Write> {
    vec![Write {
        name: name.into(),
        unit: unit.into(),
        map: None,
    }]
}

/// A dynamic style row's writes (see [`style_row`]), with what css.rs
/// writes beside a row that is not one declaration of its own: the
/// timeline rows' custom properties (LLP 1057.003 D2, D4), which the drag
/// code reads, and a paused play state under a bound `animation-timeline`.
/// `timeline` says the node has an `animation-timeline` row, so a dynamic
/// `animation` (whose shorthand resets the play state) is paused again;
pub fn style_writes(id: u16, timeline: bool) -> Result<Vec<Write>, String> {
    let row = StyleId::from_bit(id as u32).ok_or("unknown style row")?;
    let with = |name: &str, map: &'static str| Write {
        name: name.into(),
        unit: String::new(),
        map: Some(map),
    };
    Ok(match row {
        // @ref LLP 1055 D5/D7 — the browser runs it; its `@keyframes` are in
        // the stylesheet (emit.rs), under the author's names.
        StyleId::Animation if timeline => vec![
            with("animation", NONE),
            with(
                "animation-play-state",
                "v=>v==null||/^\\s*none\\s*$/i.test(v)?null:\"paused\"",
            ),
        ],
        StyleId::Animation => vec![with("animation", NONE)],
        StyleId::DragTimeline => vec![with("--exact-drag-timeline", NONE)],
        StyleId::AnimationTimeline => vec![
            with(
                "--exact-animation-timeline",
                "v=>v==null||/^\\s*auto\\s*$/i.test(v)?null:v",
            ),
            with(
                "animation-play-state",
                "v=>v==null||/^\\s*auto\\s*$/i.test(v)?null:\"paused\"",
            ),
        ],
        StyleId::AnimationRange => vec![with(
            "--exact-animation-range",
            "v=>v==null||/^\\s*normal\\s*$/i.test(v)?null:v",
        )],
        StyleId::TimelineScope => vec![
            with("timeline-scope", NONE),
            with("--exact-timeline-scope", NONE),
        ],
        // A spring is lowered by the engine (motion.js): the declaration
        // is the rest, as css.rs `transition_css` leaves springs out.
        StyleId::Transition => vec![with(
            "transition",
            "v=>v==null?v:v.split(/,(?![^(]*\\))/).filter(t=>!/spring\\(/.test(t)).join(\",\")||\"none\"",
        )],
        // The kernel's `clip-path` is `none`, `url(#id)` or `path()` (clip.rs);
        // any other shape, which the browser would take, is refused: unset.
        StyleId::ClipPath => vec![with(
            "clip-path",
            "v=>v==null||/^\\s*(none|path\\(|url\\()/i.test(v)?v:null",
        )],
        _ => {
            let (name, unit) = style_row(id)?;
            one(name, unit)
        }
    })
}

/// A dynamic style row's CSS property and the unit a number takes, read
/// from the web host's own `css_text` for a sample value (`7` → `7px`,
/// `7deg` or `7`), so the unit rule is css.rs's, not a copy. Text values
/// (enums, `auto`, `N%`, colours as `#rrggbb[aa]`, and the rows whose
/// grammar is CSS's own: `clip-path`, `shape-outside`, SVG paint, dashes,
/// filters, transform origins, gradients) are written as the author wrote
/// them; the browser parses them as the kernel does. Rows that are not one
/// declaration each are refused, never guessed.
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
    // css.rs `declared`: each of these is its value's own CSS text, in the
    // author's grammar. Not an SVG `transform` (SVG's syntax, which the
    // kernel restates as CSS's) or a marker (a reference, see emit.rs).
    if matches!(
        row.codec(),
        StyleCodec::ClipPath
            | StyleCodec::ShapeOutside
            | StyleCodec::Paint
            | StyleCodec::DashArray
            | StyleCodec::TransformOrigin
            | StyleCodec::PaintOrder
            | StyleCodec::Filter
            | StyleCodec::BackgroundImage
    ) {
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

/// A dynamic `font-family`'s declaration for each of the plan's stacks, by
/// index (the value a binding gives), as css.rs writes it with the host's
/// family names (`host/web/src/host/fonts.rs` `font_names`).
pub fn font_family_table(plan: &Plan) -> Vec<String> {
    use exact_plan::{StackMemberKind, StacksId};
    let names: Vec<String> = (0..plan.stacks.len())
        .map(|i| {
            let stack = plan.stack(StacksId(i as u32));
            let member = plan.stack_member(stack.members.iter().next().expect("validated stack"));
            match member.kind {
                StackMemberKind::Family => format!("ExactPlanStack{i}"),
                generic => generic.name().to_string(),
            }
        })
        .collect();
    (0..names.len())
        .map(|i| {
            let mut p = exact_kernel::StyleProps::default();
            let _ = p.set_dynamic(
                StyleId::FontFamily,
                &exact_kernel::StyleValue::Number(i as f64),
            );
            let (text, _) = exact_web::css::css_text(&p, &names);
            text.trim_end_matches(';')
                .split_once(':')
                .map_or(String::new(), |(_, v)| v.to_string())
        })
        .collect()
}

/// A marker row's CSS property (css.rs `property`).
pub fn style_marker(id: StyleId) -> (String, String) {
    (css_property(id), String::new())
}
