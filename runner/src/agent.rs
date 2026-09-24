//! The agent API's read operations, once, for every host.
//!
//! @ref LLP 1012 (agent API v1); `rules/NOT-DOING.md` §Agent API
//!
//! `tree`, `state`, and `logs` are answered here from the runner and its
//! kernel — never from a host's mirror of them (a projection that is a
//! parallel reconstruction is the defect exact1's 0495 §4.1 names). The
//! other five operations are the host's: `layout` and `screenshot` read what
//! it renders, `tap` and `type` go through its real input path, `clock`
//! moves its clocks (the runner's through `Runner::advance`; motion's through
//! the host's engine, whose `settle` this module also answers for it).
//!
//! Requests and replies are JSON built and read by hand (no serde anywhere
//! in the runtime): a request is `{"op":"…"}` with at most a couple of flat
//! fields, so [`field_str`] and [`field_num`] are the whole parser.

use crate::instance::InstanceStep;
use crate::runner::{DataSource, Runner};
use exact_kernel::{Color, ColorValue, Dimension, Edge, NodeRef, PropValue, RowValue, StyleId};
use exact_plan::{BindingKind, Plan, TypeKind, TypesId, Value};
use std::fmt::Write as _;

/// Answer one request: `{"op":"tree"}`, `{"op":"state"}`,
/// `{"op":"logs","since":N}`, `{"op":"node","id":V}` — the runner's half
/// of `layout <node>` (LLP 1035.002 D1) — or `{"op":"tags"}`, the identity
/// a host stamps on its own replies (D3). Anything else is an
/// `{"error":…}`.
pub fn handle<D: DataSource>(runner: &Runner<D>, request: &str) -> String {
    match field_str(request, "op").as_deref() {
        Some("tree") => tree_request(runner, request),
        Some("state") => state(runner),
        Some("tags") => tags(runner),
        Some("node") => match field_num(request, "id") {
            Some(n) if n >= 0.0 && n == n.trunc() => {
                let mut reply = node(runner, n as u32);
                if field_bool(request, "plan") && !reply.starts_with("{\"error\"") {
                    // The digest and site belong to this exact synchronous read.
                    // Kernel incarnations can repeat across host replacements.
                    // Only an explicit development inspection computes identity.
                    reply.pop();
                    reply.push_str(",\"planDigest\":");
                    quote(runner.inspection_digest(), &mut reply);
                    reply.push('}');
                }
                reply
            }
            _ => error("node needs an id"),
        },
        Some("logs") => match (after_key(request, "since"), field_num(request, "since")) {
            (None, _) => logs(runner, 0),
            (Some(_), Some(n)) if n >= 0.0 => logs(runner, n as usize),
            (Some(_), _) => error("since must be a non-negative number"),
        },
        Some(other) => error(&format!("unknown op: {other}")),
        None => error("no op"),
    }
}

/// `{"epoch":E,"incarnation":I,"clock":C}`: what every reply carries (LLP
/// 1035.002 D3). The runner's own replies are tagged where they are built;
/// a host asks for this after an operation it answered itself.
pub fn tags<D: DataSource>(runner: &Runner<D>) -> String {
    let kernel = runner.kernel();
    format!(
        "{{\"epoch\":{},\"incarnation\":{},\"clock\":{}}}",
        kernel.epoch(),
        kernel.incarnation(),
        num(runner.now_ms())
    )
}

/// `{"error":"…"}`.
pub fn error(message: &str) -> String {
    let mut s = String::from("{\"error\":");
    quote(message, &mut s);
    s.push('}');
    s
}

/// The tree: every live node in preorder — id, parent, depth, type, props by
/// their schema names, the events it handles, its children — plus the
/// kernel's epoch and incarnation (the consistency token: nothing moves
/// between two calls unless the agent moved it).
fn tree_request<D: DataSource>(runner: &Runner<D>, request: &str) -> String {
    let shallow = match after_key(request, "shallow") {
        None => false,
        Some(value) => match value.split([',', '}']).next().unwrap().trim() {
            "true" => true,
            "false" => false,
            _ => return error("tree shallow must be a boolean"),
        },
    };
    if after_key(request, "target").is_none() {
        if shallow {
            return error("shallow tree needs a target");
        }
        return tree(runner);
    }
    let name = field_str(request, "target");
    let id = field_num(request, "target")
        .filter(|n| *n >= 0.0 && *n <= u32::MAX as f64 && *n == n.trunc());
    if name.is_none() && id.is_none() {
        return error("tree target must be a view id or testId");
    }
    let kernel = runner.kernel();
    let locate = |id| {
        let mut node = kernel.node(id)?;
        let mut depth = 0u16;
        while let Some(parent) = node.parent {
            node = kernel.node(parent)?;
            depth = depth.saturating_add(1);
        }
        // The selector index also contains detached nodes; tree reads do not.
        kernel
            .arena()
            .is_root(node.key.index)
            .then_some((id, depth))
    };
    let found = if let Some(id) = id {
        locate(id as u32)
    } else {
        kernel
            .find_first_by_test_id(name.as_deref().unwrap())
            .and_then(|key| locate(kernel.node_by_key(key)?.id))
    };
    let Some((root, depth)) = found else {
        return error(&format!(
            "no view matches {}",
            name.unwrap_or_else(|| num(id.unwrap()).to_string())
        ));
    };
    if shallow {
        let mut row = kernel.row(root).expect("located live node");
        row.depth = depth;
        return tree_rows(runner, &[row], &[root]);
    }
    let mut subtree = kernel.rows(Some(root)).unwrap_or_default();
    for row in &mut subtree {
        row.depth = row.depth.saturating_add(depth);
    }
    tree_rows(runner, &subtree, &[root])
}

/// Every live root and node, in structural preorder.
pub fn tree<D: DataSource>(runner: &Runner<D>) -> String {
    let rows = runner.kernel().rows(None).unwrap_or_default();
    tree_rows(runner, &rows, &runner.roots())
}

fn tree_rows<D: DataSource>(
    runner: &Runner<D>,
    rows: &[exact_kernel::export::NodeRow],
    roots: &[u32],
) -> String {
    let kernel = runner.kernel();
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"epoch\":{},\"incarnation\":{},\"clock\":{},\"roots\":",
        kernel.epoch(),
        kernel.incarnation(),
        num(runner.now_ms())
    );
    ids(roots, &mut s);
    s.push_str(",\"nodes\":[");
    let handlers = (rows.len() != 1).then(|| runner.handlers());
    let mut first = true;
    for row in rows {
        let Some(node) = kernel.node(row.id) else {
            continue;
        };
        if !first {
            s.push(',');
        }
        first = false;
        let _ = write!(s, "{{\"id\":{},\"parent\":", node.id);
        match node.parent {
            Some(p) => {
                let _ = write!(s, "{p}");
            }
            None => s.push_str("null"),
        }
        let _ = write!(s, ",\"depth\":{},\"type\":", row.depth);
        quote(node.node_type.name(), &mut s);
        s.push_str(",\"props\":{");
        props_json(&node, &mut s);
        s.push_str("},\"handlers\":[");
        let single;
        let events = if let Some(all) = &handlers {
            all.get(&node.id).map_or(&[][..], Vec::as_slice)
        } else {
            single = runner.handlers_of(node.id);
            &single
        };
        for (i, e) in events.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            quote(e.name(), &mut s);
        }
        s.push_str("],\"children\":");
        ids(&node.children(), &mut s);
        s.push('}');
    }
    s.push_str("]}");
    s
}

/// A node's own props by their schema names, as JSON object members.
fn props_json(node: &NodeRef<'_>, s: &mut String) {
    for (i, (id, value)) in node.props.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(id.name(), s);
        s.push(':');
        match value {
            PropValue::Str(t) => quote(t, s),
            PropValue::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
            PropValue::Int(i) => {
                let _ = write!(s, "{i}");
            }
            PropValue::Float(f) => {
                let _ = write!(s, "{}", num(*f));
            }
        }
    }
}

/// One node, explained (LLP 1035.002 D1): every row it sets or inherits with
/// where the value came from — `authored` (the own row), `inherited` from
/// the ancestor whose own row won (CSS inheritance, LLP 1035.000 D1), or
/// `initial` — its props, the plan site it was instantiated from with the
/// instance path to it (D6), and the kernel's frames: `frame` relative to
/// the parent, `absolute` in the root's space, `content` when it scrolls.
/// Observations of the runner's memory, tagged with the epoch and
/// incarnation; a host adds the spaces and what it mounted. An id that is
/// not a live node in this incarnation is refused by name.
pub fn node<D: DataSource>(runner: &Runner<D>, id: u32) -> String {
    let kernel = runner.kernel();
    let Some(node) = kernel.node(id) else {
        return error(&format!(
            "stale node #{id} (incarnation {})",
            kernel.incarnation()
        ));
    };
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"epoch\":{},\"incarnation\":{},\"clock\":{},\"id\":{id},\"type\":",
        kernel.epoch(),
        kernel.incarnation(),
        num(runner.now_ms())
    );
    quote(node.node_type.name(), &mut s);
    s.push_str(",\"parent\":");
    match node.parent {
        Some(p) => {
            let _ = write!(s, "{p}");
        }
        None => s.push_str("null"),
    }
    let site = runner.site_of(id);
    if let Some((site, path)) = &site {
        let _ = write!(s, ",\"site\":{},\"instance\":[", site.0);
        for (i, step) in path.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            match step {
                InstanceStep::Row { region, key } => {
                    let _ = write!(s, "{{\"region\":{},\"key\":", region.0);
                    untyped_json(key, &mut s);
                    s.push('}');
                }
                InstanceStep::Arm { region, arm } => {
                    let _ = write!(s, "{{\"region\":{},\"arm\":", region.0);
                    match arm {
                        Some(a) => {
                            let _ = write!(s, "{a}");
                        }
                        None => s.push_str("null"),
                    }
                    s.push('}');
                }
            }
        }
        s.push(']');
    }
    // An own row the plan binds by an expression is `dynamic` (D5); one it
    // binds by a literal — or that a class expansion or the runner set — is
    // `authored`. The plan's bindings for this node's site say which.
    let plan = runner.plan();
    let dynamic = |row: StyleId| -> bool {
        site.as_ref().is_some_and(|(site, _)| {
            plan.node(*site).bindings.iter().any(|b| {
                let b = plan.binding(b);
                b.kind == BindingKind::Style
                    && StyleId::from_bit(b.id as u32) == Some(row)
                    && !crate::vm::is_literal(plan.code(b.expr))
            })
        })
    };
    s.push_str(",\"style\":{");
    let mut first = true;
    for row in StyleId::ALL {
        let own = node.style.mask.has(row);
        if !own && !row.inherited() {
            continue;
        }
        if !first {
            s.push(',');
        }
        first = false;
        quote(row.name(), &mut s);
        s.push_str(":{\"value\":");
        row_json(node.computed(row), &mut s);
        if own && dynamic(row) {
            s.push_str(",\"source\":\"dynamic\"}");
        } else if own {
            s.push_str(",\"source\":\"authored\"}");
        } else {
            match node.source_of(row) {
                Some(from) => {
                    let _ = write!(s, ",\"source\":\"inherited\",\"from\":{from}}}");
                }
                None => s.push_str(",\"source\":\"initial\"}"),
            }
        }
    }
    s.push_str("},\"props\":{");
    props_json(&node, &mut s);
    let f = node.frame;
    let (px, py) = node
        .parent
        .and_then(|p| kernel.node(p))
        .map_or((0.0, 0.0), |p| (p.frame.x, p.frame.y));
    let _ = write!(
        s,
        "}},\"frame\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}},\"absolute\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
        num((f.x - px) as f64),
        num((f.y - py) as f64),
        num(f.width as f64),
        num(f.height as f64),
        num(f.x as f64),
        num(f.y as f64),
        num(f.width as f64),
        num(f.height as f64)
    );
    // @ref LLP 1043.000 §3 D4 — leaf-local geometry and the M8 deferral reason.
    if !node.flow_shapes().is_empty() {
        s.push_str(",\"flow_shapes\":[");
        for (i, shape) in node.flow_shapes().iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            shape.write_json(&mut s);
        }
        s.push(']');
    }
    if node.flow_skipped() {
        s.push_str(",\"flow_skipped\":\"Taffy measured this paragraph's height; auto-height flow requires M8\"");
    }
    if node.content != (0.0, 0.0) {
        let _ = write!(
            s,
            ",\"content\":{{\"w\":{},\"h\":{}}}",
            num(node.content.0 as f64),
            num(node.content.1 as f64)
        );
    }
    s.push('}');
    s
}

/// A row's value as JSON, in CSS's spellings where CSS has one: a length as
/// a number of CSS pixels, `"auto"`, `"50%"`, `"env(safe-area-inset-top)"`;
/// a colour as `"#rrggbb"` (`"#rrggbbaa"` when translucent) or
/// `"light-dark(#…, #…)"`; an enum by its CSS name; a vector as `[x, y]`; a
/// clip path as its canonical text. The engine's and the grid's rows are
/// named, not spelled — nothing reads them here.
fn row_json(v: RowValue<'_>, out: &mut String) {
    let hex = |c: Color| -> String {
        if c.a() == 255 {
            format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
        }
    };
    match v {
        RowValue::Dimension(Dimension::Auto) => out.push_str("\"auto\""),
        RowValue::Dimension(Dimension::Points(p)) => {
            let _ = write!(out, "{}", num(p as f64));
        }
        RowValue::Dimension(Dimension::Percent(p)) => quote(&format!("{}%", num(p as f64)), out),
        RowValue::Dimension(Dimension::Env(edge, offset)) => {
            let edge = match edge {
                Edge::Top => "top",
                Edge::Right => "right",
                Edge::Bottom => "bottom",
                Edge::Left => "left",
            };
            let text = if offset == 0.0 {
                format!("env(safe-area-inset-{edge})")
            } else {
                format!(
                    "calc(env(safe-area-inset-{edge}) + {}px)",
                    num(offset as f64)
                )
            };
            quote(&text, out)
        }
        RowValue::LineHeight(v) => match v {
            exact_kernel::LineHeight::Number(n) => {
                let _ = write!(out, "{}", num(n as f64));
            }
            _ => quote(&v.css(), out),
        },
        RowValue::Number(n) => {
            let _ = write!(out, "{}", num(n));
        }
        RowValue::Color(c) | RowValue::ColorValue(ColorValue::Fixed(c)) => quote(&hex(c), out),
        RowValue::ColorValue(ColorValue::LightDark(l, d)) => {
            quote(&format!("light-dark({}, {})", hex(l), hex(d)), out)
        }
        RowValue::Enum(name) => quote(name, out),
        RowValue::Vec2(v) => {
            let _ = write!(out, "[{},{}]", num(v.x as f64), num(v.y as f64));
        }
        RowValue::ClipPath(p) => quote(&p.css(), out),
        RowValue::ShapeOutside(p) => quote(&p.css(), out),
        RowValue::Transitions(_) => quote("(transition)", out),
        RowValue::Color2(_) | RowValue::Tracks(_) | RowValue::Placement(_) => quote("(grid)", out),
    }
}

/// The state: the clock and every slot, derive, and resource by the name the
/// plan declares, as typed JSON (records carry their field names); the
/// requests in flight; the names the store holds (LLP 1018); the active
/// head's fields (LLP 1048.003 D1).
pub fn state<D: DataSource>(runner: &Runner<D>) -> String {
    let plan = runner.plan();
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"epoch\":{},\"incarnation\":{},\"clock\":{},\"slots\":{{",
        runner.kernel().epoch(),
        runner.kernel().incarnation(),
        num(runner.now_ms())
    );
    let mut first = true;
    for row in plan.slots.iter() {
        if row.owner.is_some() {
            // A row slot has a value per row, not one here (LLP 1017 P4c).
            continue;
        }
        let name = plan.str(row.name);
        if !first {
            s.push(',');
        }
        first = false;
        quote(name, &mut s);
        s.push(':');
        match runner.slot(name) {
            Some(v) => typed_json(plan, row.ty, v, &mut s),
            None => s.push_str("null"),
        }
    }
    s.push_str("},\"derives\":{");
    for (i, row) in plan.derives.iter().enumerate() {
        let name = plan.str(row.name);
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
        s.push(':');
        match runner.derive(name) {
            Some(v) => typed_json(plan, row.ty, v, &mut s),
            None => s.push_str("null"),
        }
    }
    s.push_str("},\"resources\":{");
    for (i, row) in plan.resources.iter().enumerate() {
        let name = plan.str(row.name);
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
        s.push(':');
        match runner.resource(name) {
            Some(v) => typed_json(plan, row.ty, v, &mut s),
            None => s.push_str("null"),
        }
    }
    s.push_str("},\"pending\":[");
    for (i, (name, ticket)) in runner.pending().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(name, &mut s);
        let _ = write!(s, ",\"ticket\":{ticket}}}");
    }
    // The store's names, never its values (LLP 1018 D5).
    s.push_str("],\"store\":[");
    for (i, name) in runner.store_names().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
    }
    s.push_str("],\"head\":{");
    for (i, (name, value)) in runner.head().fields().into_iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
        s.push(':');
        match value {
            Some(v) => quote(v, &mut s),
            None => s.push_str("null"),
        }
    }
    match runner.head().status {
        Some(code) => {
            let _ = write!(s, ",\"status\":{code}");
        }
        None => s.push_str(",\"status\":null"),
    }
    s.push_str("},\"delivery\":");
    delivery(runner, &mut s);
    s.push_str(",\"logic\":");
    logic(runner, &mut s);
    s.push('}');
    s
}

// The revision comes from the admitted source, not a host's copy of metadata.
// `ready` distinguishes a deferred, paired module from an activated executor.
fn logic<D: DataSource>(runner: &Runner<D>, s: &mut String) {
    let source = runner.data_ref();
    let revision = source.revision();
    let rust = revision
        .and_then(|r| r.strip_prefix("rust:"))
        .and_then(|r| r.split_once(':'))
        .map(|(executor, _)| executor);
    let executor = match rust {
        Some("wasm") if cfg!(target_arch = "wasm32") => Some("browser"),
        Some("wasm") => Some("wasm"),
        Some("native") => Some("native"),
        _ => None,
    };
    s.push_str("{\"revision\":");
    if let Some(revision) = revision {
        quote(revision, s);
    } else {
        s.push_str("null");
    }
    s.push_str(",\"rustExecutor\":");
    if let Some(executor) = executor {
        quote(executor, s);
    } else {
        s.push_str("null");
    }
    let _ = write!(s, ",\"ready\":{}}}", source.ready());
}

/// `state.delivery` (LLP 1030 D7): the `delivery` resource's own fields,
/// mirrored for a smoke test, `metrics.mjs`, and a developer's eyes —
/// whether the app declares that resource or not — plus what only the agent
/// needs: the compatibility id, `L` (is there an update store), and `E`
/// (which executors are linked). An `L = 0` client reads `"embedded"` here,
/// which is the honest statement that nothing can be delivered to it.
fn delivery<D: DataSource>(runner: &Runner<D>, s: &mut String) {
    let d = runner.delivery();
    s.push_str("{\"stream\":");
    quote(&d.stream, s);
    let _ = write!(
        s,
        ",\"seq\":{},\"embeddedSeq\":{},\"staged\":{},\"sunset\":",
        d.seq, d.embedded_seq, d.staged
    );
    // A resource has no absence (LLP 1004 D3), and this mirrors the
    // resource: no sunset is "".
    quote(d.sunset.as_deref().unwrap_or(""), s);
    s.push_str(",\"interpreted\":[");
    for (i, name) in d.interpreted.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(name, s);
    }
    s.push_str("],\"compatibilityId\":");
    quote(&d.compatibility_id, s);
    s.push_str(",\"L\":");
    quote(&d.store.to_string(), s);
    s.push_str(",\"E\":[");
    for (i, name) in d.executors.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(name, s);
    }
    s.push_str("]}");
}

/// Standard base64 (with padding) — a request or reply body in a batch.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The journal from `since`: `{"next":N,"from":M,"lines":[…]}`. `next` is
/// what to pass to read only what is new; `from` is the index of the first
/// line returned — above `since` when the ring has dropped older lines (the
/// oldest go first, silently: `from - since` of them are gone), and never
/// beyond `next`.
pub fn logs<D: DataSource>(runner: &Runner<D>, since: usize) -> String {
    let start = runner.journal_start();
    let lines: Vec<&str> = runner.journal().collect();
    let from = since.clamp(start, start + lines.len());
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"next\":{},\"from\":{from},\"lines\":[",
        start + lines.len()
    );
    for (i, line) in lines.iter().skip(from - start).enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(line, &mut s);
    }
    s.push_str("]}");
    s
}

/// A plan value as JSON under its declared type: numbers, strings, booleans;
/// `null` for unit and `none`; lists; records as objects keyed by field
/// name. A value that does not match its type (never, past the runner's
/// conformance checks) falls back to the positional form.
pub fn typed_json(plan: &Plan, ty: TypesId, v: &Value, out: &mut String) {
    let row = plan.type_(ty);
    match (row.kind, v) {
        (_, Value::Number(n)) => {
            let _ = write!(out, "{}", num(*n));
        }
        (_, Value::Bool(b)) => out.push_str(if *b { "true" } else { "false" }),
        (_, Value::Str(s)) => quote(s, out),
        (_, Value::Unit) | (_, Value::Option(None)) => out.push_str("null"),
        (TypeKind::Option, Value::Option(Some(inner))) => match row.elem {
            Some(elem) => typed_json(plan, elem, inner, out),
            None => untyped_json(inner, out),
        },
        (_, Value::Option(Some(inner))) => untyped_json(inner, out),
        (TypeKind::List, Value::List(items)) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                match row.elem {
                    Some(elem) => typed_json(plan, elem, item, out),
                    None => untyped_json(item, out),
                }
            }
            out.push(']');
        }
        (TypeKind::Record, Value::Record(fields)) if row.fields.len as usize == fields.len() => {
            out.push('{');
            for (i, (f, value)) in row.fields.iter().zip(fields.iter()).enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let field = plan.field(f);
                quote(plan.str(field.name), out);
                out.push(':');
                typed_json(plan, field.ty, value, out);
            }
            out.push('}');
        }
        (_, Value::List(_)) | (_, Value::Record(_)) => untyped_json(v, out),
    }
}

/// A plan value as JSON with no type to hand: records positional.
pub fn untyped_json(v: &Value, out: &mut String) {
    match v {
        Value::Number(n) => {
            let _ = write!(out, "{}", num(*n));
        }
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Str(s) => quote(s, out),
        Value::Unit | Value::Option(None) => out.push_str("null"),
        Value::Option(Some(inner)) => untyped_json(inner, out),
        Value::List(items) | Value::Record(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                untyped_json(item, out);
            }
            out.push(']');
        }
    }
}

/// Format a finite number as JSON, or `null`, without a temporary string.
pub fn num(n: f64) -> impl std::fmt::Display {
    struct Number(f64);
    impl std::fmt::Display for Number {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let n = self.0;
            if n.is_finite() {
                if n == n.trunc() && n.abs() < 1e15 {
                    write!(f, "{}", n as i64)
                } else {
                    write!(f, "{}", exact_num::Shortest(n))
                }
            } else {
                f.write_str("null")
            }
        }
    }
    Number(n)
}

/// A JSON string.
pub fn quote(s: &str, out: &mut String) {
    out.push('"');
    let mut start = 0;
    let bytes = s.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let i = cursor;
        let byte = bytes[i];
        cursor += 1;
        if !matches!(byte, b'"' | b'\\' | 0..=0x1f) {
            continue;
        }
        // Every escape is ASCII, so both slice boundaries are UTF-8 boundaries.
        // Copy ordinary text together instead of decoding and pushing each char.
        out.push_str(&s[start..i]);
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            _ => {
                let _ = write!(out, "\\u{byte:04x}");
            }
        }
        start = i + 1;
    }
    out.push_str(&s[start..]);
    out.push('"');
}

fn ids(ids: &[u32], out: &mut String) {
    out.push('[');
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{id}");
    }
    out.push(']');
}

/// The string value of a top-level `"key":"…"` field in a JSON object, with
/// JSON's escapes decoded (surrogate pairs included). `None` when the key is
/// absent at the top level, the value is not a string, or the string never
/// ends.
pub fn field_str(json: &str, key: &str) -> Option<String> {
    let rest = after_key(json, key)?;
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                'u' => {
                    let mut unit = hex4(&mut chars)?;
                    if (0xD800..0xDC00).contains(&unit) {
                        // A high surrogate: the low one must follow as `\uXXXX`.
                        if chars.next()? != '\\' || chars.next()? != 'u' {
                            return None;
                        }
                        let low = hex4(&mut chars)?;
                        if !(0xDC00..0xE000).contains(&low) {
                            return None;
                        }
                        unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                    }
                    out.push(char::from_u32(unit)?);
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

fn hex4(chars: &mut std::str::Chars<'_>) -> Option<u32> {
    let hex: String = chars.by_ref().take(4).collect();
    if hex.len() != 4 {
        return None;
    }
    u32::from_str_radix(&hex, 16).ok()
}

/// The numeric value of a top-level `"key":N` field.
pub fn field_num(json: &str, key: &str) -> Option<f64> {
    let rest = after_key(json, key)?;
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')))
        .unwrap_or(rest.len());
    exact_num::parse_f64(&rest[..end]).ok()
}

/// Whether a top-level `"key":true` field is set.
pub fn field_bool(json: &str, key: &str) -> bool {
    after_key(json, key).is_some_and(|rest| rest.starts_with("true"))
}

/// What follows `"key":` at the top level of a JSON object — a one-pass scan
/// that steps over string tokens and nested objects and arrays, so a key
/// inside a string value or a nested object never matches. The whole
/// parser: requests are flat.
fn after_key<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let b = json.as_bytes();
    let mut i = 0;
    let mut depth = 0i32;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let start = i + 1;
                let mut j = start;
                loop {
                    let c = *b.get(j)?;
                    if c == b'\\' {
                        j += 2;
                    } else if c == b'"' {
                        break;
                    } else {
                        j += 1;
                    }
                }
                let token = &json[start..j.min(b.len())];
                i = j + 1;
                if depth == 1 && token == key {
                    let rest = json.get(i..)?.trim_start();
                    if let Some(rest) = rest.strip_prefix(':') {
                        return Some(rest.trim_start());
                    }
                }
            }
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_fields_parse() {
        let r = r#"{"op":"logs","since":12,"text":"a \"b\" \\ c\n","wheel":true}"#;
        assert_eq!(field_str(r, "op").as_deref(), Some("logs"));
        assert_eq!(field_num(r, "since"), Some(12.0));
        assert_eq!(field_str(r, "text").as_deref(), Some("a \"b\" \\ c\n"));
        assert!(field_bool(r, "wheel"));
        assert!(!field_bool(r, "since"));
        assert_eq!(field_str(r, "missing"), None);
        assert_eq!(field_num(r, "op"), None);
    }

    #[test]
    fn keys_inside_values_and_nested_objects_never_match() {
        // A string value that contains `"op":`, a nested object with an `op`,
        // and an array — only the top-level `op` counts.
        let r =
            r#"{"text":"{\"op\":\"logs\"}","meta":{"op":"state"},"list":[{"op":"x"}],"op":"tree"}"#;
        assert_eq!(field_str(r, "op").as_deref(), Some("tree"));
        assert_eq!(field_str(r, "text").as_deref(), Some(r#"{"op":"logs"}"#));
        assert_eq!(field_str(r#"{"meta":{"op":"state"}}"#, "op"), None);
        // A key at the top level whose value is not a string.
        assert_eq!(field_str(r#"{"op":3}"#, "op"), None);
        assert_eq!(field_num(r#"{"since":"3"}"#, "since"), None);
        // Unterminated strings and requests are `None`, never a panic.
        assert_eq!(field_str(r#"{"op":"tre"#, "op"), None);
        assert_eq!(field_str(r#"{"op"#, "op"), None);
        assert_eq!(field_str(r#"{"op\"#, "op"), None);
        // Surrogate pairs decode to one character; a lone surrogate is refused.
        assert_eq!(field_str(r#"{"t":"😀"}"#, "t").as_deref(), Some("😀"));
        assert_eq!(field_str(r#"{"t":"\ud83d"}"#, "t"), None);
    }

    #[test]
    fn numbers_render_as_json() {
        assert_eq!(num(3.0).to_string(), "3");
        assert_eq!(num(-0.0).to_string(), "0");
        assert_eq!(num(-0.5).to_string(), "-0.5");
        assert_eq!(num(1e-8).to_string(), "0.00000001");
        assert_eq!(num(f64::NAN).to_string(), "null");
        assert_eq!(num(f64::INFINITY).to_string(), "null");
        assert_eq!(num(f64::NEG_INFINITY).to_string(), "null");
        assert_eq!(num(1e20).to_string(), "100000000000000000000");
        let mut output = String::from("[");
        write!(output, "{},{},{}]", num(-0.0), num(1e-8), num(f64::NAN)).unwrap();
        assert_eq!(output, "[0,0.00000001,null]");
        let mut s = String::new();
        quote("tab\there \"q\" \u{1}", &mut s);
        assert_eq!(s, "\"tab\\there \\\"q\\\" \\u0001\"");
    }

    #[test]
    fn quoted_strings_preserve_unicode_and_escape_boundaries() {
        for input in [
            "",
            "plain",
            "é🦀",
            "\"é\\🦀\n",
            "\u{2028}\u{2029}",
            "\t\r\n",
        ] {
            let mut json = String::from("{\"text\":");
            quote(input, &mut json);
            json.push('}');
            assert_eq!(field_str(&json, "text").as_deref(), Some(input));
        }
        let mut json = String::new();
        quote("é\u{0}🦀\u{8}\u{c}\u{1f}\u{7f}", &mut json);
        assert_eq!(json, "\"é\\u0000🦀\\u0008\\u000c\\u001f\u{7f}\"");
    }
}
