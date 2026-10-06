//! The agent API's read operations, once, for every host.
//!
//! @ref LLP 1012 (agent API v1); `rules/DEFERRED.md` §Agent API
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
use exact_kernel::{NodeRef, PropValue, RowValue, StyleId};
use exact_plan::{BindingKind, Plan, TypeKind, TypesId, Value};
use std::fmt::Write as _;

mod row;
mod schedule;
mod sound;

/// Answer one request: `{"op":"tree"}`, `{"op":"state"}`,
/// `{"op":"logs","since":N}`, `{"op":"node","id":V}` — the runner's half
/// of `layout <node>` (LLP 1035.002 D1) — or `{"op":"tags"}`, the identity
/// a host stamps on its own replies (D3). Anything else is an
/// `{"error":…}`.
pub fn handle<D: DataSource>(runner: &Runner<D>, request: &str) -> String {
    match field_str(request, "op").as_deref() {
        Some("tree") => tree_request(runner, request),
        Some("state") => state_with(runner, request),
        Some("tags") => tags(runner),
        Some("frames") => frames(runner, request, &|_| false),
        Some("holds") => holds(runner),
        // @ref LLP 1103 D3 — the driver's fetch faults, a form of `prefer`:
        // the network is the environment, as `online` is (LLP 1012 §1).
        Some("prefer") if after_key(request, "faults").is_some() => faults(runner, request),
        // The module's storage still to land (LLP 1097 D10): what a host's
        // quit or suspension waits for, cheaper than `state`.
        Some("background") => format!("{{\"operations\":{}}}", runner.background_operations()),
        Some("perf") => crate::perf::reply(runner, request),
        // What `showPicker(id)` names (LLP 1069.002 D2): the file input's
        // view, `accept` and `multiple`, for the host that presents it.
        Some("picker") => match field_str(request, "id") {
            Some(id) => match runner
                .device_links()
                .picker
                .and_then(|picker| (picker.picker)(runner, &id))
            {
                Some(p) => p.json(&id, true),
                None => error(&format!("no file input with id \"{id}\"")),
            },
            None => error("picker needs an id"),
        },
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

/// The kernel's half of `layout agree` (LLP 1080.001 D2), a private
/// message: `{"op":"frames","limit":N}` answers every live node in preorder
/// as `[id, parent|null, x, y, w, h, bits]` — the parent-relative frame a
/// host is sent — with bit 1 for the node's own `display: none`, 2 for an
/// own transform row (`translate`, `translate_z`, `rotate`, `rotate_axis`,
/// `scale`, `transform`) whose value is not the initial one (`scale: 1`,
/// `translate: 0`, `transform: none` move nothing), 4 for a frame the host owns rather than the
/// kernel (`host_owned`: a native content region's), 8 for an inline run,
/// which has no box of its own. Past `limit` (default and most 20,000)
/// nodes the list stops with `complete: false`.
pub fn frames<D: DataSource>(
    runner: &Runner<D>,
    request: &str,
    host_owned: &dyn Fn(u32) -> bool,
) -> String {
    const CAP: usize = 20_000;
    let limit = field_num(request, "limit").map_or(CAP, |n| (n.max(1.0) as usize).min(CAP));
    let transform_rows: Vec<StyleId> = StyleId::ALL
        .into_iter()
        .filter(|r| {
            matches!(
                r.name(),
                "translate" | "translate_z" | "rotate" | "rotate_axis" | "scale" | "transform"
            )
        })
        .collect();
    let initial = exact_kernel::StyleProps::default();
    let kernel = runner.kernel();
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"epoch\":{},\"incarnation\":{},\"clock\":{},\"nodes\":[",
        kernel.epoch(),
        kernel.incarnation(),
        num(runner.now_ms())
    );
    let mut count = 0usize;
    let mut complete = true;
    let mut stack: Vec<u32> = kernel.roots().into_iter().rev().collect();
    while let Some(id) = stack.pop() {
        let Some(node) = kernel.node(id) else {
            continue;
        };
        if count == limit {
            complete = false;
            break;
        }
        if count > 0 {
            s.push(',');
        }
        count += 1;
        let f = node.frame;
        let (px, py) = node
            .parent
            .and_then(|p| kernel.node(p))
            .map_or((0.0, 0.0), |p| (p.frame.x, p.frame.y));
        let mut bits = 0u32;
        if matches!(node.style.get(StyleId::Display), RowValue::Enum("none")) {
            bits |= 1;
        }
        if transform_rows
            .iter()
            .any(|r| node.style.mask.has(*r) && node.style.get(*r) != initial.get(*r))
        {
            bits |= 2;
        }
        if host_owned(id) {
            bits |= 4;
        }
        if node.is_inline_run() {
            bits |= 8;
        }
        let _ = write!(s, "[{id},");
        match node.parent {
            Some(p) => {
                let _ = write!(s, "{p}");
            }
            None => s.push_str("null"),
        }
        let _ = write!(
            s,
            ",{},{},{},{},{bits}]",
            num((f.x - px) as f64),
            num((f.y - py) as f64),
            num(f.width as f64),
            num(f.height as f64)
        );
        stack.extend(node.children().into_iter().rev());
    }
    let _ = write!(s, "],\"complete\":{complete}}}");
    s
}

/// `{"holds":[…],"tickets":[…]}`: the held device requests' tickets, and
/// every ticket still pending, network and device — what a host's `clock
/// settle` reports when a hold remains (LLP 1069.007 D3).
pub fn holds<D: DataSource>(runner: &Runner<D>) -> String {
    let join = |tickets: &mut dyn Iterator<Item = u64>| {
        tickets.map(|t| t.to_string()).collect::<Vec<_>>().join(",")
    };
    let mut all: Vec<u64> = runner.pending().iter().map(|(_, t)| *t).collect();
    all.extend(runner.device_holds().iter().map(|h| h.ticket));
    all.sort_unstable();
    format!(
        "{{\"holds\":[{}],\"tickets\":[{}]}}",
        join(&mut runner.device_holds().iter().map(|h| h.ticket)),
        join(&mut all.into_iter())
    )
}

/// `tap @t <choice>` and `type @t <value>` (LLP 1069.007 D4): the agent
/// answers held device request `t` — `{"op":"tap","ticket":7,"choice":
/// "cancel"}`, `{"op":"type","ticket":7,"text":"…"}`. `None` when the
/// request names no ticket (an ordinary `tap` or `type`). The hold is
/// consumed before the reply, which says `delivery: "substituted"` and never
/// echoes a typed value; the consumed hold goes back to the host, whose
/// capability arm delivers the answer.
pub fn answer<D: DataSource>(
    runner: &mut Runner<D>,
    request: &str,
) -> Option<(String, Option<crate::Hold>)> {
    let op = field_str(request, "op")?;
    // The picker's two writes (LLP 1069.002): under the agent a
    // `showPicker` is held for the agent (D9), and each picked file is named
    // under `app:/tmp/picked/` before a host copies it in (D3).
    if op == "showPicker" {
        let id = field_str(request, "id").unwrap_or_default();
        let Some(picker) = runner.device_links().picker else {
            return Some((
                error("the file picker is not linked into this artifact"),
                None,
            ));
        };
        return Some(match (picker.hold)(runner, &id) {
            Ok(ticket) => (format!("{{\"ticket\":{ticket}}}"), None),
            Err(e) => (error(&e), None),
        });
    }
    if op == "pickedPath" {
        let name = field_str(request, "name").unwrap_or_default();
        let mut s = String::from("{\"path\":");
        let Some(picker) = runner.device_links().picker else {
            return Some((
                error("the file picker is not linked into this artifact"),
                None,
            ));
        };
        quote(&(picker.picked_path)(runner, &name), &mut s);
        s.push('}');
        return Some((s, None));
    }
    if !matches!(op.as_str(), "tap" | "type") || after_key(request, "ticket").is_none() {
        return None;
    }
    let Some(ticket) = field_num(request, "ticket")
        .filter(|n| *n >= 1.0 && *n <= u32::MAX as f64 && *n == n.trunc())
        .map(|n| n as u64)
    else {
        return Some((error("ticket must be a positive whole number"), None));
    };
    let reply = if op == "tap" {
        match field_str(request, "choice") {
            Some(choice) => crate::HoldAnswer::Choice(choice),
            None => return Some((error(&format!("tap @{ticket} needs a choice")), None)),
        }
    } else {
        match field_str(request, "text") {
            Some(text) => crate::HoldAnswer::Value(text),
            None => return Some((error(&format!("type @{ticket} needs a value")), None)),
        }
    };
    if runner.holds(ticket) && !runner.device_holds().iter().any(|h| h.ticket == ticket) {
        let e = format!("@{ticket} is a network request; only a held device request is answered");
        return Some((error(&e), None));
    }
    match runner.answer_hold(ticket, &reply) {
        Ok(hold) => {
            // A share has nothing to deliver but its journal line (LLP
            // 1069.003 D2), so every host's answer is this one.
            if let ("share", crate::HoldAnswer::Choice(c), Some((_, answered))) = (
                hold.capability.as_str(),
                &reply,
                runner.device_links().share,
            ) {
                answered(runner, c);
            }
            let mut s = format!("{{\"ticket\":{ticket},\"capability\":");
            quote(&hold.capability, &mut s);
            // The requesting node, for the capability arm that delivers
            // the answer there (a picker's `change` or `cancel`).
            if let Some(node) = hold.node {
                s.push_str(&format!(",\"node\":{node}"));
            }
            // What the app asked for, for an arm that acts on it (an
            // export's `from`, LLP 1069.010 D3): the hold's summary.
            if hold.capability == "export" {
                s.push_str(",\"request\":");
                s.push_str(&hold.args);
            }
            s.push_str(",\"answered\":");
            match &reply {
                crate::HoldAnswer::Choice(c) => quote(c, &mut s),
                crate::HoldAnswer::Value(_) => s.push_str("\"value\""),
            }
            s.push_str(",\"delivery\":\"substituted\"}");
            Some((s, Some(hold)))
        }
        Err(e) => Some((error(&e), None)),
    }
}

/// `{"error":"…"}`.
pub fn error(message: &str) -> String {
    let mut s = String::from("{\"error\":");
    quote(message, &mut s);
    s.push('}');
    s
}

/// The tree: every live node in preorder — id, parent, depth, type, props by
/// their schema names, the events it handles, `inactive` when it is under a
/// route its navigation root has not selected, its children — plus the
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
    let Some((root, depth)) = (match target(runner, request) {
        Ok(found) => found,
        Err(e) => return error(&e),
    }) else {
        if shallow {
            return error("shallow tree needs a target");
        }
        return tree(runner);
    };
    let kernel = runner.kernel();
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

/// The view a request's `target` names — a view id, or a testId's first
/// match in preorder on a selected route — and its depth; `None` when the
/// request names none. Shared by `tree` and `perf` (LLP 1079 D2).
pub(crate) fn target<D: DataSource>(
    runner: &Runner<D>,
    request: &str,
) -> Result<Option<(u32, u16)>, String> {
    if after_key(request, "target").is_none() {
        return Ok(None);
    }
    let name = field_str(request, "target");
    let id = field_num(request, "target")
        .filter(|n| *n >= 0.0 && *n <= u32::MAX as f64 && *n == n.trunc());
    if name.is_none() && id.is_none() {
        let op = field_str(request, "op").unwrap_or_default();
        return Err(format!("{op} target must be a view id or testId"));
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
        // The first match in preorder on a selected route; a covered
        // screen's copy only when no active one carries the testId.
        let located: Vec<_> = kernel
            .find_by_test_id(name.as_deref().unwrap())
            .into_iter()
            .filter_map(|key| locate(kernel.node_by_key(key)?.id))
            .collect();
        located
            .iter()
            .copied()
            .find(|(id, _)| !runner.inactive(*id))
            .or_else(|| located.first().copied())
    };
    match found {
        Some(found) => Ok(Some(found)),
        None => Err(format!(
            "no view matches {}",
            name.unwrap_or_else(|| num(id.unwrap()).to_string())
        )),
    }
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
        s.push(']');
        if runner.inactive(node.id) {
            s.push_str(",\"inactive\":true");
        }
        s.push_str(",\"children\":");
        ids(&node.children(), &mut s);
        s.push('}');
    }
    s.push_str("]}");
    s
}

/// What agent output shows for a non-empty password field's value (#134):
/// a fixed mark, whatever its length, as no reply, log or transcript should
/// carry a secret. The app's own state still holds the value.
pub const MASKED: &str = "•••";

/// A field's value as agent output shows it: an `input type="password"`'s,
/// when not empty, is [`MASKED`]; any other is itself (a `textarea` has no
/// password type in HTML, and shows its text). Every host's tree and `type`
/// reply passes its value through this.
pub fn shown_value<'a>(props: &exact_kernel::PropList, value: &'a str) -> &'a str {
    use exact_kernel::PropId;
    if !value.is_empty()
        && props.str(PropId::Type) == Some("password")
        && props.str(PropId::SemanticTag) != Some("textarea")
    {
        MASKED
    } else {
        value
    }
}

/// A node's own props by their schema names, as JSON object members; a
/// password field's value is [`shown_value`]'s.
fn props_json(node: &NodeRef<'_>, s: &mut String) {
    for (i, (id, value)) in node.props.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(id.name(), s);
        s.push(':');
        match value {
            PropValue::Str(t) if id == exact_kernel::PropId::Value => {
                quote(shown_value(node.props, t), s)
            }
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
        row::row_json(row, node.computed(row), &mut s);
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
    // @ref LLP 1043.000 §3 D4, §8 — leaf-local geometry, or why auto height refused it.
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
    if let Some(refusal) = node.flow_refusal() {
        s.push_str(",\"flow_skipped\":");
        quote(refusal.message(), &mut s);
    }
    // @ref LLP 1093 D12 — a box's fragments and a container's columns, in
    // the root's space as `absolute` is.
    let rect = |s: &mut String, x: f32, y: f32, w: f32, h: f32| {
        let _ = write!(
            s,
            "\"x\":{},\"y\":{},\"w\":{},\"h\":{}",
            num(x as f64),
            num(y as f64),
            num(w as f64),
            num(h as f64)
        );
    };
    if let Some(frags) = kernel.fragments(node.key) {
        s.push_str(",\"column_fragments\":[");
        for (i, g) in frags.iter().enumerate() {
            s.push_str(if i > 0 { ",{" } else { "{" });
            rect(&mut s, f.x + g.x, f.y + g.y, g.width, g.height);
            if g.lines.1 > g.lines.0 {
                let _ = write!(s, ",\"lines\":[{},{}]", g.lines.0, g.lines.1);
            }
            s.push('}');
        }
        s.push(']');
    }
    if let Some(columns) = kernel.columns(node.key) {
        s.push_str(",\"columns\":[");
        for (i, c) in columns.columns.iter().enumerate() {
            s.push_str(if i > 0 { ",{" } else { "{" });
            rect(&mut s, f.x + c.x, f.y + c.y, c.width, c.height);
            let _ = write!(s, ",\"holds\":{}}}", c.holds);
        }
        s.push(']');
    }
    if let Some(refusal) = kernel.fragment_refusal(node.key) {
        s.push_str(",\"fragment_skipped\":");
        quote(refusal.message(), &mut s);
    }
    if node.content != (0.0, 0.0) {
        let _ = write!(
            s,
            ",\"content\":{{\"w\":{},\"h\":{}}}",
            num(node.content.0 as f64),
            num(node.content.1 as f64)
        );
    }
    // LLP 1056 §5: a 2D canvas's last list, readable (bounded: 200 lines,
    // 16 KiB), wherever inspection is linked.
    {
        if let Some(lines) = runner.canvas_describe(id) {
            s.push_str(",\"canvasList\":[");
            for (i, line) in lines.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                quote(line, &mut s);
            }
            s.push(']');
        }
    }
    s.push('}');
    s
}

/// The state: the clock and every slot, derive, and resource by the name the
/// plan declares, as typed JSON (records carry their field names); the
/// requests in flight; the names the store holds (LLP 1018); the active
/// head's fields (LLP 1048.003 D1).
pub fn state<D: DataSource>(runner: &Runner<D>) -> String {
    state_with(runner, "{}")
}

/// [`state`] for a request, which may ask `"sounds":"all"` (LLP 1096 D10).
fn state_with<D: DataSource>(runner: &Runner<D>, request: &str) -> String {
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
    s.push_str("},\"language\":{\"lang\":");
    quote(runner.resolved_locale(), &mut s);
    s.push_str(",\"dir\":");
    quote(runner.direction(), &mut s);
    // The host's date and place facts (LLP 1027.000.000 D3), so two agent
    // runs compare without an app that declares `exactTime`.
    let (time, place) = (runner.wall_time(), runner.place());
    let _ = write!(
        s,
        "}},\"time\":{{\"epochAtZero\":{},\"utcOffset\":{},\"locale\":",
        num(time.epoch_at_zero),
        num(time.utc_offset)
    );
    quote(&place.locale, &mut s);
    s.push_str(",\"timeZone\":");
    quote(&place.time_zone, &mut s);
    let _ = write!(s, ",\"seed\":{}", num(place.seed));
    // The device facts (LLP 1069.000; LLP 1069.007 D2), by their web names,
    // whether or not the app declares a source that reads them.
    let (media, page, fold) = (
        runner.viewport().preferences,
        runner.page(),
        runner.viewport().fold,
    );
    let _ = write!(
        s,
        "}},\"device\":{{\"prefersReducedMotion\":{},\"prefersReducedTransparency\":{},\"prefersContrast\":\"{}\",\"prefersColorScheme\":\"{}\",\"visibilityState\":\"{}\",\"onLine\":{},\"canShare\":{},\"canOpenFiles\":{},\"rootFontSize\":{},\"devicePosture\":\"{}\",\"horizontalViewportSegments\":{},\"verticalViewportSegments\":{},\"colorGamut\":\"{}\",\"dynamicRange\":\"{}\"",
        media.reduced_motion,
        media.reduced_transparency,
        media.contrast.keyword(),
        media.color_scheme(),
        page.visibility_state(),
        page.on_line,
        page.can_share,
        page.can_open_files,
        num(runner.root_font_size()),
        fold.posture.keyword(),
        fold.cols,
        fold.rows,
        media.gamut.keyword(),
        if media.high_dynamic_range { "high" } else { "standard" }
    );
    s.push('}');
    // The driver's fetch faults (LLP 1103 D3), when any are armed or spent.
    if !runner.faults().is_empty() {
        s.push_str(",\"faults\":");
        s.push_str(&runner.faults().json());
    }
    s.push_str(",\"derives\":{");
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
    let in_flight = runner.in_flight();
    for (i, (name, ticket)) in in_flight.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(name, &mut s);
        let _ = write!(s, ",\"ticket\":{ticket}}}");
    }
    // Held device requests, after the network's (LLP 1069.007 D3): each
    // with its capability and that capability's inspection summary.
    let network = in_flight.len();
    for (i, hold) in runner.device_holds().iter().enumerate() {
        if i > 0 || network > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(&hold.name, &mut s);
        let _ = write!(
            s,
            ",\"ticket\":{},\"device\":{{\"capability\":",
            hold.ticket
        );
        quote(&hold.capability, &mut s);
        s.push_str(",\"args\":");
        if hold.args.starts_with('{') && hold.args.ends_with('}') {
            s.push_str(&hold.args);
        } else {
            s.push_str("{}");
        }
        s.push_str("}}");
    }
    // Open streams (LLP 1016.000 D5): pending until their first message,
    // then listed here with what they delivered and what coalesced.
    s.push_str("],\"streams\":[");
    for (i, (name, ticket, count)) in runner.streams().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(name, &mut s);
        let _ = write!(
            s,
            ",\"ticket\":{ticket},\"messages\":{},\"coalesced\":{}}}",
            count.messages, count.coalesced
        );
    }
    s.push(']');
    // The module's background storage (LLP 1097 D8), where it has any.
    if let Some(b) = runner.background_state() {
        let _ = write!(
            s,
            ",\"background\":{{\"queued\":{},\"inFlight\":{},\"done\":{},\"failed\":{},\"last\":",
            b.queued, b.in_flight, b.done, b.failed
        );
        match &b.last {
            Some(line) => quote(line, &mut s),
            None => s.push_str("null"),
        }
        s.push('}');
    }
    schedule::tasks(runner, &mut s);
    schedule::queued(runner, &mut s);
    // Notifications posted under the agent, where none reaches the system.
    s.push_str(",\"notifications\":[");
    for (i, n) in runner.notifications().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        n.summary(&mut s);
    }
    s.push(']');
    sound::state(runner, request, &mut s);
    // The store's names, never its values (LLP 1018 D5).
    s.push_str(",\"store\":[");
    for (i, name) in runner.store_names().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
    }
    s.push_str("],\"head\":{");
    let head = runner.head();
    for (i, (name, value)) in head.fields().into_iter().enumerate() {
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
    match head.status {
        Some(code) => {
            let _ = write!(s, ",\"status\":{code}");
        }
        None => s.push_str(",\"status\":null"),
    }
    let _ = write!(s, ",\"edited\":{}", head.edited);
    s.push_str("},\"delivery\":");
    delivery(runner, &mut s);
    s.push_str(",\"logic\":");
    logic(runner, &mut s);
    // Each virtualized list's snapshot, nested ones included: its axis, its
    // count, its offset's extent and its mounted rows (LLP 1070 G3).
    s.push_str(",\"collections\":");
    s.push_str(&runner.collections_json());
    // The reorder session, if one is under way (LLP 1094 D12).
    s.push_str(",\"reorder\":");
    s.push_str(&runner.reorder_json());
    // Each list's latest scrollIntoView and how it stands (LLP 1070.000).
    s.push_str(",\"scrollIntoView\":");
    s.push_str(&runner.into_view_json());
    // Where each inner list's reader was when its outer row left (Q1).
    s.push_str(",\"kept\":");
    s.push_str(&runner.kept_positions_json());
    s.push_str(",\"canvas\":");
    runner.canvas_state(&mut s);
    // Each surface whose latest record was refused, and why: its readers
    // keep the last record accepted (`set_surface_record`).
    s.push_str(",\"surfaceRefusals\":{");
    for (i, (name, why)) in runner.surface_refusals().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
        s.push(':');
        quote(why, &mut s);
    }
    s.push_str("}}");
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
        (_, v @ exact_plan::str_value!()) => quote(v.text(), out),
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
        v @ exact_plan::str_value!() => quote(v.text(), out),
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
pub fn num(n: f64) -> Num {
    Num(n)
}

/// [`num`]'s number: a piece of `exact_num::text!` for the journal's lines,
/// which skip `core::fmt`, and `Display` (the same text) for the agent's
/// replies.
pub struct Num(f64);

impl std::fmt::Display for Num {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use exact_num::Piece;
        let mut text = String::new();
        self.push_to(&mut text);
        f.write_str(&text)
    }
}

impl exact_num::Piece for Num {
    fn push_to(&self, out: &mut String) {
        let n = self.0;
        if !n.is_finite() {
            out.push_str("null");
        } else if n == n.trunc() && n.abs() < 1e15 {
            (n as i64).push_to(out);
        } else {
            exact_num::Shortest(n).push_to(out);
        }
    }
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

/// `prefer` with `faults` (LLP 1103 D3): `{"fail": prefix, "times"?: n}`
/// arms a prefix, `{"pass": prefix}` stops it failing; either way, and with
/// neither, the reply is the session's table as `state.faults` shows it.
fn faults<D: DataSource>(runner: &Runner<D>, request: &str) -> String {
    let table = runner.faults();
    let request = after_key(request, "faults").unwrap_or("{}");
    if let Some(prefix) = field_str(request, "fail") {
        let times = match after_key(request, "times") {
            None => None,
            Some(_) => match field_num(request, "times") {
                Some(n) if n >= 1.0 && n == n.trunc() && n <= u32::MAX as f64 => Some(n as u32),
                _ => return error("fail fetch: `times` is a positive integer"),
            },
        };
        if let Err(e) = table.arm(&prefix, times) {
            return error(&e);
        }
    } else if let Some(prefix) = field_str(request, "pass") {
        if !table.pass(&prefix) {
            return error(&format!(
                "pass fetch \"{prefix}\": no fault was armed for it"
            ));
        }
    }
    format!("{{\"faults\":{}}}", table.json())
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
