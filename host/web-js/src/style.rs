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
/// the value at creation; the JS target from its presence). A `progress`'s
/// `value` is one: with it HTML's `<progress>` bar, without it the drawn
/// ring (LLP 1116 D8).
fn decides_tag(plan: &Plan, node: &exact_plan::NodesRow, p: PropId) -> bool {
    matches!(p, PropId::Href | PropId::SemanticTag)
        || p == PropId::Value
            && node.bindings.iter().any(|b| {
                let row = plan.binding(b);
                row.kind == BindingKind::Prop
                    && row.id == PropId::Type as u16
                    && literal(plan, plan.code(row.expr))
                        .is_some_and(|v| v.as_str() == Some("progress"))
            })
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
                (BindingKind::Prop, Some(v)) => match bridge::prop_value(row.id, &v) {
                    Ok((prop, value)) => ops.push(Op::SetProp {
                        id: view(i),
                        prop,
                        value,
                    }),
                    Err(bridge::BridgeError::Unset(_)) => {}
                    Err(e) => return Err(format!("node {i}: {e:?}")),
                },
                (BindingKind::Prop, None) => {
                    let prop = PropId::from_wire(row.id).ok_or("unknown prop")?;
                    if decides_tag(plan, node, prop) {
                        ops.push(Op::SetProp {
                            id: view(i),
                            prop,
                            value: sample(prop.kind()),
                        });
                    }
                }
                // An explicit clear has no own value but still beats host defaults.
                (BindingKind::Style, Some(v)) if bridge::unsets(row.id, &v) => {
                    if row.written {
                        patch
                            .written
                            .set(StyleId::from_bit(row.id as u32).ok_or("unknown style")?);
                        styled = true;
                    }
                }
                // With what the binding says of the row (the author wrote it, or the compiler
                // implied it: LLP 1115.000 D2), which the page's marks read (`data-exact-said`).
                (BindingKind::Style, Some(v)) => {
                    bridge::set_binding_style(&mut patch, row, &v, plan)
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
        if let Some(writes) = exact_runner::press_writes_router(plan, exact_plan::NodesId(i as u32))
        {
            parts
                .props
                .insert("data-exact-router-press".into(), writes.to_string());
        }
        // A sampled prop decided the tag; its value is the effect's.
        for b in node.bindings.iter() {
            let row = plan.binding(b);
            if row.kind == BindingKind::Prop && literal(plan, plan.code(row.expr)).is_none() {
                if let Some(prop) =
                    PropId::from_wire(row.id).filter(|p| decides_tag(plan, node, *p))
                {
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

/// Every `for` row's roots, through any region at its top.
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
/// (an invalid computed declaration: CSS unset, with a diagnostic).
pub fn color_map(plan: &Plan) -> String {
    let map = platform_map(plan);
    match theme_map(plan) {
        Some(theme) => format!("((T,M)=>v=>M(T(v)))({theme},{map})"),
        None => map,
    }
}

/// @ref LLP 1095 D4 — a bound colour that names a theme entry, or a role the
/// theme overrides, writes what the literal path writes for the entry: the
/// kernel's CSS for the colour it stands for. `None` without a theme.
fn theme_map(plan: &Plan) -> Option<String> {
    if plan.theme.is_empty() {
        return None;
    }
    let _theme = exact_kernel::style::theme::bind(&exact_runner::bridge::theme_table(plan));
    let _profiles = exact_kernel::style::profiled::declarations(
        plan.profiles
            .iter()
            .map(|row| (plan.str(row.name), plan.str(row.src), plan.str(row.intent))),
    );
    let (mut names, mut roles) = (Vec::new(), Vec::new());
    let varying = crate::modes::properties(plan);
    for row in &plan.theme {
        let value = plan.str(row.value);
        let Some(colour) = exact_kernel::ColorValue::parse_light_dark(value)
            .or_else(|| exact_kernel::Color::parse(value).map(exact_kernel::ColorValue::Fixed))
        else {
            continue;
        };
        let role = plan.str(row.role);
        // The tint's roles stay themselves (`ColorValue::themed`): the page's
        // `--exact-accent` carries the theme's colour under an authored one.
        if exact_kernel::style::roles::role(role).is_some_and(exact_kernel::style::roles::tint) {
            continue;
        }
        let mut css = String::new();
        exact_kernel::gradient::color_css(&mut css, colour);
        // A token that varies by an app axis is its property (LLP 1121 D7).
        if let Some((_, prop)) = varying.iter().find(|(n, _)| n == plan.str(row.name)) {
            css = format!("var({prop})");
        }
        names.push(format!(
            "{}:{}",
            serde_json::to_string(plan.str(row.name)).unwrap(),
            serde_json::to_string(&css).unwrap()
        ));
        if !role.is_empty() {
            roles.push(format!(
                "{}:{}",
                serde_json::to_string(&role.to_ascii_lowercase()).unwrap(),
                serde_json::to_string(&css).unwrap()
            ));
        }
    }
    // A colour function's own text is left whole, as the kernel reads no
    // name inside one (a `light-dark()` half, LLP 1095 D1), unless it is a
    // synthetic token's, which is named by that text (LLP 1121 D3); a
    // gradient's stops and a shadow's colour are components, read as names.
    // A comment, a `url()` or a quoted string is copied through whole.
    Some(format!(
        r#"((N,R,H)=>{{const F=/\/\*[\s\S]*?\*\/|url\(\s*(?:"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|[^)]*)\)|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|(?<![-\w])(?:light-dark|color-mix|rgba?|hsla?|hwb|lab|lch|oklab|oklch|color|-exact-platform-color)\(/gi,T=/url\(\s*(?:"[^"]*"|'[^']*'|[^)]*)\)|"[^"]*"|'[^']*'|\/\*[\s\S]*?\*\/|#[-\w]*|[-\w.\\\u0080-￿]+\(?/gi,M=s=>s.replace(T,t=>Object.hasOwn(N,t)?H(N[t]):Object.hasOwn(R,t.toLowerCase())?H(R[t.toLowerCase()]):t);return v=>{{if(typeof v!=="string")return v;let o="",at=0,m;F.lastIndex=0;while((m=F.exec(v))){{if(m[0].slice(-1)!=="("){{o+=M(v.slice(at,m.index))+m[0];at=F.lastIndex;continue}}let d=1,j=F.lastIndex;while(j<v.length&&d){{const c=v[j++];if(c==="(")d++;else if(c===")")d--}}const f=v.slice(m.index,j);o+=M(v.slice(at,m.index))+(Object.hasOwn(N,f)?H(N[f]):f);at=j;F.lastIndex=j}}return o+M(v.slice(at))}}}})({{{}}},{{{}}},{})"#,
        names.join(","),
        roles.join(","),
        // Whether a row read a token, which a mode boundary asks (modes.js).
        match crate::modes::active(plan) {
            true => "v=>((globalThis.exact??={}).$th=1,v)",
            false => "v=>v",
        }
    ))
}

/// [`SYSTEM_COLOR_MAP`] behind the platform-colour gate.
fn platform_map(plan: &Plan) -> String {
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
        "((P,M)=>v=>typeof v===\"string\"&&v.includes(\"{PLATFORM}\")?Object.hasOwn(P,v)?M(P[v]):{refused}:M(v))({{{}}},{m})",
        admitted.join(","),
        m = SYSTEM_COLOR_MAP.as_str(),
        refused = invalid_unset("color")
    )
}

const PLATFORM: &str = "-exact-platform-color(";

/// A bound row whose code can push a metric's name (LLP 1115.002 D6: a
/// sheet the compiler wrote, choosing between two by a flag) reads each as
/// its value in the neutral column this build draws, which is what
/// the kernel reads a literal one as: a number, the row's own CSS text, or
/// nothing where the look has no row (`null`: the row unset). `None` when
/// the code names none.
pub fn metric_map(plan: &Plan, code: &[u8]) -> Option<String> {
    use exact_kernel::style::metrics::{column, named, value};
    let mut pairs: Vec<String> = Vec::new();
    for i in instructions(code).flatten() {
        if i.op != Opcode::Str {
            continue;
        }
        let name = plan.str(exact_plan::StrId(i.args[0] as u32));
        let Some(metric) = named(name) else { continue };
        let web = match value(metric, column()) {
            exact_kernel::MetricValue::Number(n) => serde_json::json!(n),
            exact_kernel::MetricValue::Text("unset") => serde_json::Value::Null,
            exact_kernel::MetricValue::Text(t) => serde_json::json!(t),
            // A `length` row's, which no sheet names.
            exact_kernel::MetricValue::Em(_) | exact_kernel::MetricValue::Rem(_) => continue,
        };
        let pair = format!("{}:{web}", serde_json::json!(name));
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }
    (!pairs.is_empty()).then(|| {
        format!(
            "(M=>v=>typeof v===\"string\"&&Object.hasOwn(M,v)?M[v]:v)({{{}}})",
            pairs.join(",")
        )
    })
}

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
        let rem = Unit::TextStyle(i as u8).basis(MEDIUM, MEDIUM, 1.0) / MEDIUM;
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

/// A bound corner's map: a list (white space outside parentheses, or a
/// `/`) is the elliptical radius no row holds, refused whole with the
/// journal line rt.js `css` writes for a value a row does not take, as the
/// kernel refuses it (LLP 1001 §1); one value is the row's. `say` is rt.js's.
pub fn corner_list(id: StyleId) -> String {
    format!(
        r#"v=>{{if(typeof v!=="string")return v;let t=v;while(t!==(t=t.replace(/\([^()]*\)/g,"()")));return /\S[ \t\n\r\f]+\S|\//.test(t.trim())?(say(`unset {}: ${{JSON.stringify(v)}} is not a value it takes`),"unset"):v}}"#,
        css_property(id)
    )
}

// A rejected computed declaration uses CSS unset, while null removes it.
fn invalid_unset(row: &str) -> String {
    format!("(()=>{{const e=globalThis.exact;e?.journal?.push(`t=${{e.now?.()??0}} invalid {row} value ${{JSON.stringify(v)}}; unset`);return \"unset\"}})()")
}

// The bounded cursor vocabulary is the schema's, including dynamic bindings.
static CURSOR_MAP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        "v=>{{if(v==null)return null;if(typeof v===\"string\"){{v=v.trim().toLowerCase();if({:?}.includes(v)||v===\"inherit\"||v===\"unset\")return v}}return {}}}",
        StyleId::Cursor.enum_names(), invalid_unset("cursor")
    )
});
static CLIP_MAP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        r#"v=>v==null||/^\s*(unset\s*$|none\s*$|path\(|url\()/i.test(v)?v:{}"#,
        invalid_unset("clip-path")
    )
});
static SCHEME_MAP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        r#"v=>v==null?v:v==="light"||v==="dark"?v:/^\s*(normal|inherit|unset)\s*$/i.test(v)?"unset":{}"#,
        invalid_unset("color-scheme")
    )
});
static CORNER_MAP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        r#"v=>v==null?v:typeof v!=="string"||/-apple-continuous/i.test(v)?{}:v.replace(/-exact-continuous/gi,"superellipse(1.6)")"#,
        invalid_unset("corner-shape")
    )
});

static TINT_MAP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let mut initial = String::new();
    exact_kernel::gradient::color_css(
        &mut initial,
        exact_kernel::StyleProps::default()
            .tint_color
            .expect("schema initial tint color"),
    );
    format!(r#"v=>v==null?v:/^\s*(unset)?\s*$/i.test(v)?{initial:?}:v"#)
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
        // A mode boundary's tuple: its rows are set in it (LLP 1121 D7).
        StyleId::Mode => Vec::new(),
        StyleId::FontSize => vec![Write {
            name: "font-size".into(),
            unit: "px".into(),
            map: Some(TEXT_STYLE_MAP.as_str()),
        }],
        StyleId::ControlSize => vec![with("--exact-control-font-size", "v=>v==null?v:({mini:'x-small',small:'small',medium:'medium',large:'large',unset:'medium'})[v]??'medium'")],
        StyleId::TintColor => vec![with("--exact-tint", &TINT_MAP)],
        StyleId::PlaceholderColor => vec![with("--exact-placeholder-color", "v=>v==null?v:/^\\s*(unset)?\\s*$/i.test(v)?'initial':v")],
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
            with("--exact-button-columns", "v=>v==null?v:v.startsWith('column')?'minmax(var(--exact-button-title-min,min-content),auto)':'auto auto minmax(var(--exact-button-title-min,min-content),auto)'"),
            with("--exact-button-space", "v=>v==null?v:v.startsWith('column')?'var(--exact-button-row-space)':'var(--exact-button-column-space)'"),
        ],
        // @ref LLP 1116 D1 — a native button's title truncates only when the
        // author keeps it to one line (exact_web's button_css.rs `rows`).
        StyleId::WhiteSpace => {
            let (name, unit) = style_row(id)?;
            let mut writes = one(name, unit);
            writes.push(with("--exact-button-title-min", "v=>v==null?v:/^\\s*(nowrap|pre)\\s*$/i.test(v)?'0px':null"));
            writes
        }
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
        // The accent role must inherit, never write a self-reference.
        StyleId::AccentColor => vec![
            with("accent-color", "v=>v"),
            with("--exact-accent", "v=>v==null||typeof v===\"string\"&&v.trim()===\"var(--exact-accent, AccentColor)\"?null:/^\\s*auto\\s*$/i.test(v)?\"AccentColor\":v"),
        ],
        StyleId::PositionTryFallbacks => vec![with(
            "position-try-fallbacks",
            r#"v=>v==null||/^\s*(?:none|(?:flip-block(?:\s+flip-inline)?|flip-inline(?:\s+flip-block)?)(?:\s*,\s*(?:flip-block(?:\s+flip-inline)?|flip-inline(?:\s+flip-block)?))*)\s*$/i.test(v)?v:"invalid""#,
        )],
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
            &CLIP_MAP,
        )],
        // @ref LLP 1077 D1 — Apple's curve as the web's stand-in. A bound
        // radius is not rescaled here, as css.rs scales a static one: a
        // dynamic `-exact-continuous` reaches less far on the web.
        // @ref LLP 1034 §8 — `light` and `dark`; a bound `normal` follows
        // the surrounding scheme (an explicit clear), as the kernel unsets
        // it; anything else is no value it takes.
        StyleId::ColorScheme => vec![with(
            "color-scheme",
            &SCHEME_MAP,
        )],
        StyleId::TextWrapStyle => one("text-wrap-style", ""),
        // A row's word to CSS's two (host/web/src/css.rs `text_box_edge`).
        StyleId::TextBoxEdge => vec![with(
            "text-box-edge",
            "v=>v==null?v:({cap:\"cap text\",ex:\"ex text\"})[v]??v.replace(\"-alphabetic\",\" alphabetic\")",
        )],
        StyleId::CornerShape => vec![with(
            "corner-shape",
            &CORNER_MAP,
        )],
        // A bound colour unsets the theme's scheme text for its subtree, as
        // a literal one does (roles.rs `scheme_color`); a value that unsets
        // the row (StyleValue::text_unsets) authors none.
        StyleId::TextColor if exact_kernel::style::roles::overridden_canvas_text() => {
            let (name, unit) = style_row(id)?;
            let mut writes = one(name, unit);
            writes.push(with("--exact-scheme-color", r"v=>v==null||/^\s*(unset|inherit|currentcolor)?\s*$/i.test(v)?null:'initial'"));
            writes
        }
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
        StyleCodec::PositionTryFallbacks
            | StyleCodec::ClipPath
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
        // Not the theme's reset an authored colour writes beside itself.
        let declaration = text
            .split(';')
            .find(|d| !d.starts_with("--exact-scheme-color:"))?;
        let (name, value) = declaration.split_once(':')?;
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
        // @ref LLP 1104 D3 — css.rs's `spell_property`.
        StyleId::PlaceholderColor => return "--exact-placeholder-color".into(),
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

    /// @ref LLP 1095 D4 — a bound colour naming a theme entry or an
    /// overridden role writes the entry's CSS, as the literal path does.
    #[test]
    fn a_bound_theme_name_writes_its_entry() {
        let plan = contract::compile("theme\n  brand = \"#c8102e\"\n  ink = \"brand\"\n  -exact-secondary-label = \"light-dark(#111, #eee)\"\n  AccentColor = \"#00ff00\"\n\ncomponent App\n  state c: string = \"brand\"\n  view\n    column\n      text \"a\" color=c\n      text \"b\" color=\"light-dark(brand, #000)\"\n").unwrap();
        let values = [
            "brand",
            "ink",
            "-exact-secondary-label",
            "-Exact-Secondary-Label",
            "0 1px 2px brand",
            "BRAND",
            "CanvasText",
            "#123456",
            "AccentColor",
            "light-dark(brand, #000)",
            "light-dark(rgb(0 0 0 / calc((1))), brand)",
            "linear-gradient(brand, #000)",
        ];
        assert_eq!(
            run(&color_map(&plan), &values),
            serde_json::json!([
                "#c8102eff",
                "#c8102eff",
                "light-dark(#111111ff, #eeeeeeff)",
                "light-dark(#111111ff, #eeeeeeff)",
                "0 1px 2px #c8102eff",
                "BRAND",
                run(&SYSTEM_COLOR_MAP, &["CanvasText"])[0].clone(),
                "#123456",
                // The tint's role stays itself, under an authored accent too;
                // no name is read inside a colour function but a synthetic
                // token's, which the view wrote (LLP 1121 D3).
                run(&SYSTEM_COLOR_MAP, &["AccentColor"])[0].clone(),
                "light-dark(#c8102eff, #000000ff)",
                "light-dark(rgb(0 0 0 / calc((1))), brand)",
                "linear-gradient(#c8102eff, #000)"
            ])
        );
    }

    /// LLP 1121 D3 — a design's dotted name and a synthetic token's
    /// expression, bound, read their folded colours as the kernel does.
    #[test]
    fn a_bound_design_name_and_synthetic_token_write_their_values() {
        let dir = std::env::temp_dir().join(format!("exact-web-js-tamber-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("app.tamber"),
            "tamber t\n\ntheme\n  brand = #c8102e\n  surface.card = #000000\n",
        )
        .unwrap();
        std::fs::write(dir.join("app.contract"), "component App\n  state c: string = \"brand\"\n  view\n    column\n      text \"a\" color=c\n      text \"b\" color=\"light-dark(brand, surface.card)\"\n").unwrap();
        let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
        let values = [
            "surface.card",
            "0 1px 2px surface.card",
            "light-dark(brand, surface.card)",
            "linear-gradient(light-dark(brand, surface.card), #fff)",
            "1.5px",
            "url(\"a/light-dark(brand, surface.card).png\") brand",
            "url(\"a\\\")light-dark(brand, surface.card).png\")",
            "/* \" */ brand /* \" */",
        ];
        assert_eq!(
            run(&color_map(&plan), &values),
            serde_json::json!([
                "#000000ff",
                "0 1px 2px #000000ff",
                "light-dark(#c8102eff, #000000ff)",
                "linear-gradient(light-dark(#c8102eff, #000000ff), #fff)",
                "1.5px",
                "url(\"a/light-dark(brand, surface.card).png\") #c8102eff",
                "url(\"a\\\")light-dark(brand, surface.card).png\")",
                "/* \" */ #c8102eff /* \" */"
            ])
        );
    }

    #[test]
    fn a_bound_accent_role_leaves_the_inherited_accent_unset() {
        let writes = style_writes(StyleId::AccentColor as u16, false).unwrap();
        let values = ["AccentColor", " accentcolor ", "#5856d6", "auto", "inherit"];
        let maps: Vec<_> = writes
            .iter()
            .map(|write| {
                let map = format!(
                    "v=>({})(({})(v))",
                    write.map.unwrap(),
                    SYSTEM_COLOR_MAP.as_str()
                );
                (write.name.as_str(), run(&map, &values))
            })
            .collect();
        assert_eq!(
            maps,
            vec![
                (
                    "accent-color",
                    serde_json::json!([
                        "var(--exact-accent, AccentColor)",
                        " var(--exact-accent, AccentColor) ",
                        "#5856d6",
                        "auto",
                        "inherit"
                    ])
                ),
                (
                    "--exact-accent",
                    serde_json::json!([null, null, "#5856d6", "AccentColor", "inherit"])
                )
            ]
        );
    }

    #[test]
    fn mapped_clears_and_invalid_values_keep_declarations_but_none_removes_them() {
        for (id, invalid) in [
            (StyleId::Cursor, "not-a-cursor"),
            (StyleId::ClipPath, "circle(20%)"),
            (StyleId::ColorScheme, "orange"),
            (StyleId::CornerShape, "-apple-continuous"),
        ] {
            let writes = style_writes(id as u16, false).unwrap();
            let map = writes[0].map.unwrap();
            let script = format!("globalThis.exact={{journal:[],now:()=>0}};const m={map};console.log(JSON.stringify({{values:[null,'unset',{invalid:?}].map(m),journal:exact.journal}}))");
            let out =
                std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
                    .args(["-e", &script])
                    .output()
                    .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            let observed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(
                observed["values"],
                serde_json::json!([null, "unset", "unset"]),
                "{id:?}"
            );
            assert_eq!(
                observed["journal"].as_array().unwrap().len(),
                1,
                "{id:?}: {observed}"
            );
        }
    }

    #[test]
    fn grid_refusals_keep_declarations_but_absent_values_remove_them() {
        let cases: Vec<_> = [
            (StyleId::GridTemplateColumns, "40px 1fr", "subgrid [rails]"),
            (StyleId::GridTemplateRows, "40px 1fr", "repeat(10001, 1px)"),
            (StyleId::GridColumn, "1 / span 2", "10001"),
            (StyleId::GridRow, "1 / span 2", "span 10001"),
            (StyleId::GridAutoFlow, "row dense", "inherit"),
            (StyleId::JustifyItems, "center", "left legacy"),
        ]
        .into_iter()
        .map(|(id, valid, refused)| {
            let writes = style_writes(id as u16, false).unwrap();
            (writes[0].map.unwrap(), valid, refused)
        })
        .collect();
        let script = format!(
            r#"import {{ mock }} from 'bun:test';
import {{ strict as assert }} from 'node:assert';
const journal = [];
mock.module('./rt.js', () => ({{ normal: (_, v) => v, say: line => journal.push(line) }}));
const {{ gridValue }} = await import('./grid.js');
for (const [source, valid, refused] of {}) {{
  const map = eval(`(${{source}})`);
  assert.equal(map(null), null);
  assert.equal(map(undefined), undefined);
  assert.equal(map(valid), valid);
  for (const value of [refused, '']) {{
    const before = journal.length;
    assert.equal(map(value), 'unset', source);
    assert.equal(journal.length, before + 1);
    assert.ok(journal.at(-1).startsWith('unset '));
    assert.ok(journal.at(-1).includes(JSON.stringify(value)));
  }}
}}"#,
            serde_json::to_string(&cases).unwrap()
        );
        let out = std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
            .args(["-e", &script])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn balanced_wrap_writes_the_same_property_for_bound_values() {
        let writes = style_writes(StyleId::TextWrapStyle as u16, false).unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].name, "text-wrap-style");
        assert_eq!(writes[0].unit, "");
        assert_eq!(writes[0].map, None);
    }

    /// LLP 1104 D3: a bound `::placeholder` colour writes css.rs's
    /// private property, which index.html's `::placeholder` rule reads.
    #[test]
    fn a_bound_placeholder_colour_writes_the_private_property() {
        let writes = style_writes(StyleId::PlaceholderColor as u16, false).unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].name, "--exact-placeholder-color");
        let map = writes[0].map.unwrap();
        assert_eq!(
            run(
                &format!("v=>({map})(JSON.parse(v))"),
                &["null", "\"unset\"", "\"#123456\""]
            ),
            serde_json::json!([null, "initial", "#123456"])
        );
    }

    #[test]
    fn a_bound_tint_clear_restores_the_symbol_role() {
        let writes = style_writes(StyleId::TintColor as u16, false).unwrap();
        assert_eq!(writes[0].name, "--exact-tint");
        let map = writes[0].map.unwrap();
        assert_eq!(
            run(
                &format!("v=>({map})(JSON.parse(v))"),
                &["null", "\"unset\"", "\"#123456\""]
            ),
            serde_json::json!([
                null,
                "var(--exact-symbol, var(--exact-accent, AccentColor))",
                "#123456"
            ])
        );
    }

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
    /// bound `normal` (or an invalid value) retains an unset declaration.
    #[test]
    fn a_bound_color_scheme_is_light_dark_or_unset() {
        let writes = style_writes(StyleId::ColorScheme as u16, false).unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].name, "color-scheme");
        assert_eq!(
            run(
                writes[0].map.unwrap(),
                &["light", "dark", "normal", "light dark"]
            ),
            serde_json::json!(["light", "dark", "unset", "unset"])
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
                "unset",
                "unset",
                "unset",
                role("secondary-label"),
            ])
        );
        // A plan without one writes the plain map.
        assert_eq!(color_map(&exact_plan::Plan::default()), *SYSTEM_COLOR_MAP);
    }
}
