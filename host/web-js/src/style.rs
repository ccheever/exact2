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

/// A canvas's explicit bitmap size (LLP 1056 D6 r3) and getContext settings
/// (LLP 1100 D12a) as the attributes canvas2d.js reads, as the runner reads
/// the node's props; a dynamic one is refused.
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
            id if id == PropId::ColorSpace as u16 => "data-color-space",
            id if id == PropId::ColorType as u16 => "data-color-type",
            _ => continue,
        };
        let value = match literal(plan, plan.code(b.expr)) {
            Some(exact_plan::Value::Number(n)) => Some((n.max(0.0) as u32).to_string()),
            Some(v) if name.starts_with("data-color") => v.as_str().map(str::to_string),
            _ => None,
        };
        let Some(value) = value else {
            return Err("a dynamic canvas bitmap size or setting is not in the JS target".into());
        };
        attrs.push((name.into(), value));
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
                // `unset`, or `inherit` on an inherited row: no row (feed F1).
                (BindingKind::Style, Some(v)) if bridge::unsets(row.id, &v) => {}
                (BindingKind::Style, Some(v)) => {
                    bridge::set_plan_style(&mut patch, row.id, &v, plan)
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
    let tree = element_children(plan, sites);
    for (i, children) in tree.iter().cloned().enumerate() {
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
    // A text folds into its box's content (LLP 1007.001) only when the
    // template holds all of it: no bound row or prop but its text, no
    // handler, and not a repeated row (copies would join one block). A box
    // with a bound row keeps its text too: the row may make it lay out
    // other than a block, and so does a button whose parent has one (its
    // height may come to be the parent's).
    let repeated = each_rows(plan, sites);
    let bound = |i: usize, only_text: bool| {
        plan.nodes[i]
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .any(|b| {
                literal(plan, plan.code(b.expr)).is_none()
                    && !(only_text && b.kind == BindingKind::Prop && b.id == PropId::Text as u16)
                    && (only_text || b.kind == BindingKind::Style)
            })
    };
    let parents = {
        let mut p = vec![None; plan.nodes.len()];
        for (i, c) in tree.iter().enumerate() {
            for c in c {
                p[*c as usize] = Some(i);
            }
        }
        p
    };
    let may_fold: Vec<bool> = (0..plan.nodes.len())
        .map(|i| {
            plan.nodes[i].handlers.len == 0
                && !repeated.contains(&i)
                && !bound(i, true)
                && parents[i].is_some_and(|p| {
                    let button =
                        NodeType::from_wire(plan.nodes[p].node_type) == Some(NodeType::Pressable);
                    !bound(p, false) && !(button && parents[p].is_none_or(|g| bound(g, false)))
                })
        })
        .collect();
    let mut out = Vec::with_capacity(plan.nodes.len());
    for (i, node) in plan.nodes.iter().enumerate() {
        if NodeType::from_wire(node.node_type) == Some(NodeType::Head) {
            out.push(None);
            continue;
        }
        let child_may_fold = matches!(tree[i].as_slice(), [c] if may_fold[*c as usize]);
        let mut parts = template::parts_with(&kernel, plan, view(i), may_fold[i], child_may_fold)
            .ok_or("a view the kernel lost")?;
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
    // Arms and each copies share templates. Only candidacy is static;
    // paint.js decides isolation from the actual instance's descendants.
    for (i, parts) in out.iter_mut().enumerate() {
        let Some(parts) = parts else { continue };
        let node = kernel.node(view(i)).expect("template node");
        crate::paint::attributes(&node.facts(), &mut parts.props);
    }
    Ok(out)
}

/// Every `each` row's roots, through any region at its top.
fn each_rows(plan: &Plan, sites: &crate::emit::Sites) -> Vec<usize> {
    fn roots(
        plan: &Plan,
        sites: &crate::emit::Sites,
        list: &[crate::emit::Site],
        into: &mut Vec<usize>,
    ) {
        for s in list {
            match s {
                crate::emit::Site::Node(i) => into.push(*i as usize),
                crate::emit::Site::Region(r) => {
                    for arm in plan.regions[*r as usize].arms.iter() {
                        roots(plan, sites, sites.of_arm(arm.0), into);
                    }
                }
            }
        }
    }
    let mut rows = Vec::new();
    for region in plan
        .regions
        .iter()
        .filter(|r| r.kind == exact_plan::RegionKind::Each)
    {
        for arm in region.arms.iter() {
            roots(plan, sites, sites.of_arm(arm.0), &mut rows);
        }
    }
    rows
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

/// A bound value naming a colour role (LLP 1095 D2) as the role's CSS: each
/// whole ident token that is a role's name or its WebKit `-apple-system-*`
/// alias, so a role inside a gradient, a shadow or a shorthand is one too;
/// the kernel's table, so literal and bound values agree. A `url()`, a
/// string, a comment, a hash and a function's name are tokens of their own,
/// and an ident is matched whole, so `context-fill`, `--exact-label` and
/// `url(label.png)` stay as written.
pub static SYSTEM_COLOR_MAP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    use exact_kernel::{ColorValue, COLOR_ROLES};
    let mut pairs = Vec::new();
    for (i, r) in COLOR_ROLES.iter().enumerate() {
        let mut css = String::new();
        exact_kernel::gradient::color_css(&mut css, ColorValue::Role(i as u8));
        // A CSS colour by its name; an Exact role as `-exact-<role>` (LLP
        // 1081 D2), so a bare role name stays as written and is no colour.
        let name = if exact_kernel::style::roles::is_css_system(r) {
            r.name.to_ascii_lowercase()
        } else {
            format!("-exact-{}", r.name)
        };
        pairs.push(format!("{name:?}:{css:?}"));
        if !r.alias.is_empty() {
            pairs.push(format!("{:?}:{css:?}", r.alias));
        }
    }
    format!(
        r#"(M=>v=>typeof v==="string"?v.replace(/url\(\s*(?:"[^"]*"|'[^']*'|[^)]*)\)|"[^"]*"|'[^']*'|\/\*[\s\S]*?\*\/|#[-\w]*|[-\w\\\u0080-\uffff]+\(?/gi,t=>M[t.toLowerCase()]??t):v)({{{}}})"#,
        pairs.join(",")
    )
});

/// [`SYSTEM_COLOR_MAP`] for a binding of `plan`, gated as the runner's
/// `set_plan_style` is (LLP 1095 D3): a value with a `-exact-platform-color()` is
/// admitted only as one of the plan's own string literals, and writes what
/// the literal path writes for each of its functions; any other is refused
/// (`null`: the row unset).
pub fn color_map(plan: &Plan) -> String {
    let admitted: Vec<String> = plan
        .strings
        .iter()
        .filter(|s| s.contains(PLATFORM))
        .map(|s| {
            format!(
                "{}:{}",
                serde_json::to_string(s).unwrap(),
                serde_json::to_string(&platform_css(s)).unwrap()
            )
        })
        .collect();
    if admitted.is_empty() {
        return SYSTEM_COLOR_MAP.clone();
    }
    format!(
        "(P=>v=>typeof v===\"string\"&&v.includes(\"{PLATFORM}\")?Object.hasOwn(P,v)?({m})(P[v]):null:({m})(v))({{{}}})",
        admitted.join(","),
        m = SYSTEM_COLOR_MAP.as_str()
    )
}

const PLATFORM: &str = "-exact-platform-color(";

/// A computed `transition`'s entries as an author's (LLP 1081 D5), shared by
/// the ordinary and the pressed mappers: comments go (CSS removes them, so a
/// check must too); springs are the engine's; an entry naming the host's
/// `--exact-*` or an old spelling is dropped, as the native parser refuses
/// it; `-exact-tint-color` becomes the property the browser animates,
/// `--exact-tint`. `p` is the list of kept entries.
pub const TRANSITION_ENTRIES: &str = "const p=v.replace(/\\/\\*[\\s\\S]*?\\*\\//g,\" \").split(/,(?![^(]*\\))/).map(t=>t.trim()).filter(t=>t&&!/spring\\(/i.test(t)&&!/(^|\\s)(--exact-\\S*|tint-color|exit-animation|layout-transition|press-scale|drag-timeline|symbol-rendering|symbol-palette|symbol-value|symbol-effect|press-haptic|content-transition|scroll-edge-effect|hover-effect|smart-invert|animation-trigger)(?=\\s|$)/i.test(t)).map(t=>t.replace(/(^|\\s)-exact-tint-color(?=\\s|$)/i,\"$1--exact-tint\"));";

/// A computed `transition`, as the browser takes it: [`TRANSITION_ENTRIES`].
pub static TRANSITION_MAP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!("v=>{{if(v==null)return v;{TRANSITION_ENTRIES}return p.join(\",\")||\"none\"}}")
});

/// `text` with each `-exact-platform-color()` in it as its CSS (its `web` colour or
/// its fallback), as the kernel writes a literal's.
fn platform_css(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(PLATFORM) {
        out.push_str(&rest[..at]);
        let call = &rest[at..];
        let mut depth = 0usize;
        let end = call
            .char_indices()
            .find_map(|(i, c)| {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(i + 1);
                        }
                    }
                    _ => {}
                }
                None
            })
            .unwrap_or(call.len());
        match exact_kernel::ColorValue::parse_light_dark(&call[..end]) {
            Some(c) => exact_kernel::gradient::color_css(&mut out, c),
            None => out.push_str(&call[..end]),
        }
        rest = &call[end..];
    }
    out.push_str(rest);
    out
}

/// A bound `font-size` naming a platform text style (LLP 1115 D3) as css.rs
/// writes a literal one: the ramp's size at CSS's `medium`, in `rem`. Any
/// other value is written as it is (a number takes the row's `px`).
pub static TEXT_STYLE_MAP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    use exact_kernel::style::relative::{Unit, MEDIUM};
    let mut pairs = Vec::new();
    for (i, s) in exact_kernel::TEXT_STYLES.iter().enumerate() {
        let rem = Unit::TextStyle(i as u8).basis(MEDIUM, MEDIUM) / MEDIUM;
        let css = format!("{rem}rem");
        pairs.push(format!("{:?}:{css:?}", format!("-exact-{}", s.name)));
        if !s.alias.is_empty() {
            pairs.push(format!("{:?}:{css:?}", s.alias));
        }
    }
    format!(
        r#"(M=>v=>typeof v==="string"?M[v.trim().toLowerCase()]??v:v)({{{}}})"#,
        pairs.join(",")
    )
});

/// A row's value `none` (or the keyword `auto`/`normal`) writes nothing, as
/// css.rs writes no declaration for the row's empty value.
const NONE: &str = "v=>v==null||/^\\s*none\\s*$/i.test(v)?null:v";

// The bounded cursor vocabulary is the schema's, including dynamic bindings.
static CURSOR_MAP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        "v=>{{if(typeof v!==\"string\")return null;v=v.trim().toLowerCase();return {:?}.includes(v)?v:null}}",
        StyleId::Cursor.enum_names()
    )
});

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
        StyleId::FontSize => vec![Write {
            name: "font-size".into(),
            unit: "px".into(),
            map: Some(TEXT_STYLE_MAP.as_str()),
        }],
        StyleId::ControlSize => vec![with("--exact-control-font-size", "v=>v==null?v:({mini:'x-small',small:'small',medium:'medium',large:'large'})[v]??null")],
        StyleId::ControlCornerStyle => vec![with("--exact-control-radius", "v=>v==null?v:v==='capsule'?'calc(infinity * 1px)':'revert'")],
        StyleId::ColumnGap | StyleId::RowGap => {
            let column = row == StyleId::ColumnGap;
            let mut writes = one(if column { "column-gap" } else { "row-gap" }, "px");
            writes.extend(one(if column { "--exact-button-column-gap" } else { "--exact-button-row-gap" }, "px"));
            writes.push(with(if column { "--exact-button-column-space" } else { "--exact-button-row-space" }, r#"v=>v==null?v:'""'"#));
            writes
        }
        StyleId::FlexDirection => vec![
            with("flex-direction", "v=>v"),
            with("--exact-button-leading-areas", r#"v=>v==null?v:v.startsWith('column')?'"image" "space" "title"':'"image space title"'"#),
            with("--exact-button-trailing-areas", r#"v=>v==null?v:v.startsWith('column')?'"title" "space" "image"':'"title space image"'"#),
            with("--exact-button-leading-subtitle-areas", r#"v=>v==null?v:v.startsWith('column')?'"image" "space" "title" "subtitle"':'"image space title" "image space subtitle"'"#),
            with("--exact-button-trailing-subtitle-areas", r#"v=>v==null?v:v.startsWith('column')?'"title" "subtitle" "space" "image"':'"title space image" "subtitle space image"'"#),
            with("--exact-button-space-width", "v=>v==null?v:v.startsWith('column')?'0px':'var(--exact-button-column-gap,auto)'"),
            with("--exact-button-space-height", "v=>v==null?v:v.startsWith('column')?'var(--exact-button-row-gap,1em)':'0px'"),
            with("--exact-button-columns", "v=>v==null?v:v.startsWith('column')?'minmax(0,auto)':'auto auto minmax(0,auto)'"),
            with("--exact-button-space", "v=>v==null?v:v.startsWith('column')?'var(--exact-button-row-space)':'var(--exact-button-column-space)'"),
        ],
        StyleId::TextAlign => vec![with("text-align", "v=>v"), with("--exact-button-align", "v=>v==null?v:({left:'start',start:'start',right:'end',end:'end'})[v]??'center'")],
        StyleId::Cursor => vec![with("cursor", &CURSOR_MAP)],
        StyleId::ZIndex => vec![with("z-index", crate::paint::Z_INDEX)],
        // @ref LLP 1055 D5/D7 — the browser runs it; its `@keyframes` are in
        // the stylesheet (emit.rs), under the author's names.
        StyleId::Animation if timeline => vec![
            with("animation", NONE),
            with(
                "animation-play-state",
                "v=>v==null||/^\\s*none\\s*$/i.test(v)?null:\"paused\"",
            ),
        ],
        // @ref LLP 1077 §5 — Apple's affordances: no declaration on the web
        // (its forms are declared in LLP 1001).
        StyleId::SymbolRendering
        | StyleId::SymbolPalette
        | StyleId::SymbolValue
        | StyleId::SymbolEffect
        | StyleId::ContentTransition
        | StyleId::ScrollEdgeEffect
        | StyleId::HoverEffect
        | StyleId::SmartInvert => vec![],
        // @ref LLP 1055 D13 — the browser starts an animation at insertion;
        // this target does not hold one for its row (a declared deviation).
        StyleId::AnimationTrigger => vec![],
        StyleId::Animation => vec![with("animation", NONE)],
        // @ref LLP 1069.011 D8 — and the custom property a native button reads.
        StyleId::AccentColor => vec![
            with("accent-color", "v=>v"),
            with("--exact-accent", "v=>v==null?v:/^\\s*auto\\s*$/i.test(v)?\"AccentColor\":v"),
        ],
        StyleId::DragTimeline => vec![with("--exact-drag-timeline", NONE)],
        // @ref LLP 1055.002 — `-exact-clock(Name)` is css.rs's clock property and
        // leaves the play state alone; a drag timeline pauses.
        StyleId::AnimationTimeline => vec![
            with(
                "--exact-animation-timeline",
                "v=>v==null||/^\\s*(auto|(-exact-)?clock\\(.*\\))\\s*$/i.test(v)?null:v",
            ),
            with(
                "--exact-animation-clock",
                "v=>v==null?v:/^\\s*-exact-clock\\(\\s*([^)\\s]+)\\s*\\)\\s*$/i.exec(v)?.[1]??null",
            ),
            with(
                "animation-play-state",
                "v=>v==null||/^\\s*(auto|(-exact-)?clock\\(.*\\))\\s*$/i.test(v)?null:\"paused\"",
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
        StyleId::GridTemplateColumns | StyleId::GridTemplateRows => vec![with(
            &css_property(row),
            "v=>gridValue(\"tracks\",v)",
        )],
        StyleId::GridColumn | StyleId::GridRow => vec![with(
            &css_property(row),
            "v=>gridValue(\"placement\",v)",
        )],
        StyleId::GridAutoFlow => vec![with(
            "grid-auto-flow",
            "v=>gridValue(\"flow\",v)",
        )],
        StyleId::JustifyItems => vec![with(
            "justify-items",
            "v=>gridValue(\"justify\",v)",
        )],
        // A spring is lowered by the engine (motion.js): the declaration
        // is the rest, as css.rs `transition_css` leaves springs out.
        StyleId::Transition => vec![with(
            "transition",
            TRANSITION_MAP.as_str(),
        )],
        // The kernel's `clip-path` is `none`, `url(#id)` or `path()` (clip.rs);
        // any other shape, which the browser would take, is refused: unset.
        StyleId::ClipPath => vec![with(
            "clip-path",
            "v=>v==null||/^\\s*(none|path\\(|url\\()/i.test(v)?v:null",
        )],
        // @ref LLP 1077 D1 — Apple's curve as the web's stand-in. A bound
        // radius is not rescaled here, as css.rs scales a static one: a
        // dynamic `-exact-continuous` reaches less far on the web.
        // @ref LLP 1034 §8 — `light` and `dark`; a bound `normal` follows
        // the surrounding scheme (the property removed), as the kernel unsets
        // it; anything else is no value it takes.
        StyleId::ColorScheme => vec![with(
            "color-scheme",
            "v=>v===\"light\"||v===\"dark\"?v:null",
        )],
        StyleId::CornerShape => vec![with(
            "corner-shape",
            "v=>v==null?v:/-apple-continuous/i.test(v)?null:v.replace(/-exact-continuous/gi,\"superellipse(1.6)\")",
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
    // These codecs parse CSS text in the kernel. `style_writes` lets Chrome
    // parse the same authored text after excluding Taffy's declared gaps.
    if matches!(row.codec(), StyleCodec::Tracks | StyleCodec::Placement) {
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
            | StyleCodec::MaskImage
            | StyleCodec::BackdropFilter
            | StyleCodec::BoxShadow
            | StyleCodec::TextShadow
            | StyleCodec::CornerShape
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
        StyleId::FontFamily
            | StyleId::LineClamp
            | StyleId::PressScale
            | StyleId::FontVariantNumeric
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
        StyleId::BackdropFilter => return "backdrop-filter".into(),
        StyleId::SvgMask => return "mask".into(),
        StyleId::TextStrokeWidth => return "-webkit-text-stroke-width".into(),
        StyleId::TextStrokeColor => return "-webkit-text-stroke-color".into(),
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
    let names = exact_web::css::font_family_names(plan);
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The role's CSS, as the literal path writes it.
    pub(crate) fn role(name: &str) -> String {
        let id = exact_kernel::COLOR_ROLES
            .iter()
            .position(|r| r.name == name)
            .unwrap();
        let mut css = String::new();
        exact_kernel::gradient::color_css(&mut css, exact_kernel::ColorValue::Role(id as u8));
        css
    }

    /// `SYSTEM_COLOR_MAP` run by Bun over each value.
    fn mapped(values: &[&str]) -> Vec<String> {
        serde_json::from_value(run(SYSTEM_COLOR_MAP.as_str(), values)).unwrap()
    }

    /// `map` run by Bun over each value.
    pub(crate) fn run(map: &str, values: &[&str]) -> serde_json::Value {
        let script = format!(
            "console.log(JSON.stringify({}.map({map})))",
            serde_json::to_string(values).unwrap(),
        );
        let out = std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
            .args(["-e", &script])
            .output()
            .expect("bun runs the map (the repo pins it)");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }

    /// LLP 1115 D3: a bound text style is written as css.rs writes a
    /// literal one; a number is left for the row's `px`.
    #[test]
    fn a_bound_text_style_is_its_rem() {
        let out = run(
            TEXT_STYLE_MAP.as_str(),
            &[
                "-exact-title1",
                " -Apple-System-Headline ",
                "1.5rem",
                "-exact-nope",
            ],
        );
        assert_eq!(
            out,
            serde_json::json!(["1.6875rem", "1rem", "1.5rem", "-exact-nope"])
        );
    }

    #[test]
    fn a_computed_transition_is_checked_as_an_authors() {
        // LLP 1081 D5: the new tint becomes the property the browser
        // animates; the old spelling, the host's own name (behind a comment
        // too) and any old name are dropped, as the native parser refuses
        // them; springs are the engine's.
        let values = [
            "-exact-tint-color 1s",
            "tint-color 1s, opacity 1s",
            "/**/--exact-tint 1s",
            "opacity 1s -exact-spring(1, 2, 3), scale 1s",
            "press-scale 1s",
            "animation-trigger 1s, --exact-π 1s",
        ];
        assert_eq!(
            run(TRANSITION_MAP.as_str(), &values),
            serde_json::json!([
                "--exact-tint 1s",
                "opacity 1s",
                "none",
                "scale 1s",
                "none",
                "none"
            ])
        );
    }

    /// LLP 1034 §8: a bound `color-scheme` writes `light` or `dark`, and a
    /// bound `normal` (or anything else) removes the property, so the node
    /// follows its parent's scheme as the kernel's unset row does.
    #[test]
    fn a_bound_color_scheme_is_light_dark_or_removed() {
        let writes = style_writes(StyleId::ColorScheme as u16, false).unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].name, "color-scheme");
        assert_eq!(
            run(
                writes[0].map.unwrap(),
                &["light", "dark", "normal", "light dark"]
            ),
            serde_json::json!(["light", "dark", null, null])
        );
    }

    #[test]
    fn a_bound_role_is_its_css_whole_or_inside_a_composite_value() {
        let (orange, label, fill) = (role("system-orange"), role("secondary-label"), role("fill"));
        assert_eq!(
            mapped(&[
                " -exact-secondary-label ",
                "-Exact-System-Orange",
                "linear-gradient(-exact-system-orange, #fff)",
                "radial-gradient(circle,-exact-fill 0%,-exact-system-orange 100%)",
                "0 1px 2px -exact-system-orange, inset 0 0 4px -apple-system-secondary-label",
                "1px -apple-system-orange",
            ]),
            [
                format!(" {label} "),
                orange.clone(),
                format!("linear-gradient({orange}, #fff)"),
                format!("radial-gradient(circle,{fill} 0%,{orange} 100%)"),
                format!("0 1px 2px {orange}, inset 0 0 4px {label}"),
                format!("1px {orange}"),
            ]
        );
    }

    #[test]
    fn a_role_name_inside_another_token_stays_as_written() {
        let values = [
            "context-fill",
            "var(--exact-label,#000)",
            "url(label.png)",
            "url(\"fill (1).png\"), linear-gradient(red, blue)",
            "image-set(\"background.png\" 1x)",
            "url(#fill)",
            "#label",
            "fill-rule",
            "labels",
            "background2",
            "fill(1)",
            "/* label */ red",
            // An Exact role's bare name is no colour (LLP 1081 D2).
            "system-orange",
            "linear-gradient(secondary-label, fill)",
        ];
        assert_eq!(mapped(&values), values);
    }

    #[test]
    fn a_bound_platform_color_is_admitted_only_as_a_plan_literal() {
        let literal = "-exact-platform-color(ios webJsTestColor, #010203)";
        let gradient =
            "linear-gradient(-exact-platform-color(ios webJsTestColor, #010203), -exact-system-orange)";
        let mut plan = exact_plan::Plan::default();
        plan.strings.extend([literal.into(), gradient.into()]);
        let mut css = String::new();
        exact_kernel::gradient::color_css(
            &mut css,
            exact_kernel::ColorValue::parse_light_dark(literal).unwrap(),
        );
        assert!(!css.contains("platform-color"), "{css}");
        let map = color_map(&plan);
        assert_eq!(
            run(
                &map,
                &[
                    literal,
                    gradient,
                    "-exact-platform-color(ios webJsOtherColor, #010203)",
                    "linear-gradient(-exact-platform-color(ios webJsTestColor, #010203), #fff)",
                    " -exact-platform-color(ios webJsTestColor, #010203)",
                    "-exact-secondary-label"
                ]
            ),
            serde_json::json!([
                css,
                format!("linear-gradient({css}, {})", role("system-orange")),
                null,
                null,
                null,
                role("secondary-label"),
            ])
        );
        // A plan without one writes the plain map.
        assert_eq!(color_map(&exact_plan::Plan::default()), *SYSTEM_COLOR_MAP);
    }
}
