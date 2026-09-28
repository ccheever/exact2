//! The plan's tables → one ES module.
//!
//! Module shape (every name local, the bundler minifies):
//! slots `s_i = sig(init)`, derives `d_i = memo(body)`, resources
//! `r_i = res(source, args, compiled value, compiled args)`, actions
//! `a_i = act(body)` (writes land at commit, as the VM's `StoreSlot`
//! collects them), then the view as nested statements: a node is `h(…)` with
//! its static class, a dynamic binding is an effect, a region is
//! `when`/`match`/`each` over closures that build an arm or a row. Timers
//! last. Everything reactive is lazy, so declaration order is free.

use crate::code::{self, Frame, Scope, Uses};
use crate::style;
use exact_kernel::{NodeType, PropId};
use exact_plan::{BindingKind, EventKind, Plan, RegionKind, Value};
use exact_web::host::template::Parts;
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug)]
pub enum Site {
    Node(u32),
    Region(u32),
}

/// The plan's child sites, grouped as the runner groups them.
pub struct Sites {
    pub root: u32,
    by_node: Vec<Vec<Site>>,
    by_arm: Vec<Vec<Site>>,
}

impl Sites {
    pub fn new(plan: &Plan) -> Result<Sites, String> {
        let mut by_node: Vec<Vec<(u32, u8, u32, Site)>> = vec![Vec::new(); plan.nodes.len()];
        let mut by_arm: Vec<Vec<(u32, u8, u32, Site)>> = vec![Vec::new(); plan.arms.len()];
        let mut root = None;
        for (i, n) in plan.nodes.iter().enumerate() {
            let e = (n.order, 0, i as u32, Site::Node(i as u32));
            // A site's group is (parent, arm): an arm's roots have no parent;
            // a deeper site names its parent node and its enclosing arm.
            match (n.parent, n.arm) {
                (Some(p), _) => by_node[p.0 as usize].push(e),
                (None, Some(a)) => by_arm[a.0 as usize].push(e),
                (None, None) => {
                    if root.replace(i as u32).is_some() {
                        return Err("more than one root".into());
                    }
                }
            }
        }
        for (i, r) in plan.regions.iter().enumerate() {
            let e = (r.order, 1, i as u32, Site::Region(i as u32));
            match (r.parent, r.arm) {
                (Some(p), _) => by_node[p.0 as usize].push(e),
                (None, Some(a)) => by_arm[a.0 as usize].push(e),
                (None, None) => return Err("a region at the root".into()),
            }
        }
        let sort = |v: Vec<Vec<(u32, u8, u32, Site)>>| {
            v.into_iter()
                .map(|mut l| {
                    l.sort_by_key(|e| (e.0, e.1, e.2));
                    l.into_iter().map(|e| e.3).collect()
                })
                .collect()
        };
        Ok(Sites {
            root: root.ok_or("no root")?,
            by_node: sort(by_node),
            by_arm: sort(by_arm),
        })
    }
    pub fn of_node(&self, n: u32) -> &[Site] {
        &self.by_node[n as usize]
    }
    pub fn of_arm(&self, a: u32) -> &[Site] {
        &self.by_arm[a as usize]
    }
}

pub struct Output {
    pub js: String,
    pub css: String,
    pub viewport: Option<String>,
    pub warnings: Vec<String>,
}

/// A plan value as a JavaScript literal (records and lists are arrays,
/// `none` is `null`, `some(x)` is `x`).
pub fn value_js(v: &Value) -> String {
    match v {
        Value::Number(n) => code::number(*n),
        Value::Bool(b) => if *b { "!0" } else { "!1" }.into(),
        Value::Str(s) => serde_json::to_string(&**s).unwrap(),
        Value::Unit => "null".into(),
        Value::Option(None) => "null".into(),
        Value::Option(Some(x)) => value_js(x),
        Value::List(items) | Value::Record(items) => {
            format!("[{}]", items.iter().map(value_js).collect::<Vec<_>>().join(","))
        }
    }
}

pub fn dump(plan: &Plan) {
    let mut uses = Uses::default();
    let s = Scope::default();
    let f = |code: exact_plan::Code, uses: &mut Uses, scope: &Scope, params: usize| {
        code::function(plan, plan.code(code), scope, params, uses).unwrap_or_else(|e| format!("<{e}>"))
    };
    eprintln!("router: {:?}", plan.router);
    for (i, r) in plan.slots.iter().enumerate() {
        eprintln!("slot {i} {} = {}", plan.str(r.name), f(r.init, &mut uses, &s, 0));
    }
    for (i, r) in plan.derives.iter().enumerate() {
        eprintln!("derive {i} {} = {}", plan.str(r.name), f(r.body, &mut uses, &s, 0));
    }
    for (i, r) in plan.resources.iter().enumerate() {
        eprintln!("resource {i} {} = {}(..{})", plan.str(r.name), plan.str(r.source), r.args.len);
    }
    let a = Scope { action: true, ..Scope::default() };
    for (i, r) in plan.actions.iter().enumerate() {
        eprintln!("action {i} {} = {}", plan.str(r.name), f(r.body, &mut uses, &a, r.params.len as usize));
    }
    for (i, r) in plan.regions.iter().enumerate() {
        eprintln!("region {i} {:?} parent {:?} arm {:?} order {} arms {:?}", r.kind, r.parent, r.arm, r.order, r.arms);
    }
    for (i, n) in plan.nodes.iter().enumerate() {
        eprintln!(
            "node {i} type {:?} parent {:?} arm {:?} order {} bindings {} handlers {}",
            NodeType::from_wire(n.node_type),
            n.parent,
            n.arm,
            n.order,
            n.bindings.len,
            n.handlers.len
        );
    }
}

struct Em<'a> {
    plan: &'a Plan,
    sites: &'a Sites,
    parts: Vec<Option<Parts>>,
    uses: Uses,
    out: String,
    classes: Vec<String>,
    warnings: Vec<String>,
}

pub fn emit(plan: &Plan) -> Result<Output, String> {
    if !plan.mutations.is_empty() {
        return Err("mutations (`send`) are not in the spike".into());
    }
    let sites = Sites::new(plan)?;
    let mut warnings = Vec::new();
    let parts = style::project(plan, &sites, &mut warnings)?;
    let mut em = Em {
        plan,
        sites: &sites,
        parts,
        uses: Uses::default(),
        out: String::new(),
        classes: Vec::new(),
        warnings,
    };
    let top = Scope::default();
    let action = Scope { action: true, ..Scope::default() };
    let mut body = String::new();
    // Slots, in order: an initializer reads only earlier slots.
    for (i, r) in plan.slots.iter().enumerate() {
        let init = code::function(plan, plan.code(r.init), &top, 0, &mut em.uses)
            .map_err(|e| format!("slot {}: {e}", plan.str(r.name)))?;
        let sig = em.uses.rt("sig");
        let _ = write!(body, "const s_{i}={sig}(({init})());");
        if plan.router.map(|s| s.0 as usize) == Some(i) {
            em.warnings.push(format!(
                "slot {} is the router: its launch value is the initializer's, not `Router::launch` (no router in the spike)",
                plan.str(r.name)
            ));
        }
    }
    for (i, r) in plan.derives.iter().enumerate() {
        let f = code::function(plan, plan.code(r.body), &top, 0, &mut em.uses)
            .map_err(|e| format!("derive {}: {e}", plan.str(r.name)))?;
        let memo = em.uses.rt("memo");
        let _ = write!(body, "const d_{i}={memo}({f});");
    }
    for (i, r) in plan.resources.iter().enumerate() {
        let mut args = Vec::new();
        for a in r.args.iter() {
            let f = code::function(plan, plan.code(plan.arg(a).expr), &top, 0, &mut em.uses)
                .map_err(|e| format!("resource {}: {e}", plan.str(r.name)))?;
            args.push(format!("({f})()"));
        }
        let initial = plan.bytes(r.initial);
        let initial = if initial.is_empty() {
            "void 0".to_string()
        } else {
            value_js(&Value::from_bytes(initial).map_err(|e| e.to_string())?)
        };
        let initial_args = plan.bytes(r.initial_args);
        let initial_args = if initial_args.is_empty() {
            "void 0".to_string()
        } else {
            value_js(&Value::from_bytes(initial_args).map_err(|e| e.to_string())?)
        };
        let res = em.uses.rt("res");
        let _ = write!(
            body,
            "const r_{i}={res}({},()=>[{}],{initial},{initial_args});",
            serde_json::to_string(plan.str(r.source)).unwrap(),
            args.join(",")
        );
    }
    for (i, r) in plan.actions.iter().enumerate() {
        let f = code::function(plan, plan.code(r.body), &action, r.params.len as usize, &mut em.uses)
            .map_err(|e| format!("action {}: {e}", plan.str(r.name)))?;
        let act = em.uses.rt("act");
        let _ = write!(body, "const a_{i}={act}({f});");
    }
    em.out.clear();
    em.node(sites.root, "R", &top)?;
    let view = std::mem::take(&mut em.out);
    let mount = em.uses.rt("mount");
    let _ = write!(body, "{mount}(R=>{{{view}}});");
    for t in plan.timers.iter() {
        let every = em.uses.rt("every");
        let _ = write!(body, "{every}({},a_{},{});", t.interval_ms, t.action.0, t.once as u8);
    }
    let viewport = em.parts[sites.root as usize].as_ref().and_then(|p| {
        let fit = p.props.get("viewportFit");
        let widget = p.props.get("interactiveWidget");
        (fit.is_some() || widget.is_some()).then(|| {
            let mut m = String::from("width=device-width, initial-scale=1");
            if let Some(f) = fit {
                let _ = write!(m, ", viewport-fit={f}");
            }
            if let Some(w) = widget {
                let _ = write!(m, ", interactive-widget={w}");
            }
            m
        })
    });
    // The sources' parameter types, for a data module that needs values
    // encoded by type (records and lists are both arrays here).
    let mut sources = Vec::new();
    for r in plan.sources.iter() {
        let params: Vec<String> = r.params.iter().map(|p| type_code(plan, plan.source_params[p.0 as usize].ty)).collect();
        sources.push(format!("{}:{}", serde_json::to_string(plan.str(r.name)).unwrap(), serde_json::to_string(&params.concat()).unwrap()));
    }
    let imports: Vec<String> = em.uses.names.iter().cloned().collect();
    let js = format!(
        "// Generated by exact-web3 from the app's plan. Do not edit.\nimport{{{}}}from\"./rt.js\";\nexport const sources={{{}}};export default function(){{{body}}}\n",
        imports.join(","),
        sources.join(",")
    );
    let mut css = String::from("#exact-root#exact-root{");
    for (i, c) in em.classes.iter().enumerate() {
        let _ = write!(css, ".c{}{{{c}}}", i + 1);
    }
    css.push('}');
    Ok(Output { js, css, viewport, warnings: em.warnings })
}

/// A type as the data module client reads it: `n` number, `b` bool, `s`
/// string, `u` unit, `?T` option, `[T` list, `{T…}` record.
fn type_code(plan: &Plan, ty: exact_plan::TypesId) -> String {
    let t = &plan.types[ty.0 as usize];
    match t.kind {
        exact_plan::TypeKind::Number => "n".into(),
        exact_plan::TypeKind::Bool => "b".into(),
        exact_plan::TypeKind::String => "s".into(),
        exact_plan::TypeKind::Unit => "u".into(),
        exact_plan::TypeKind::Option => format!("?{}", type_code(plan, t.elem.expect("option element"))),
        exact_plan::TypeKind::List => format!("[{}", type_code(plan, t.elem.expect("list element"))),
        exact_plan::TypeKind::Record => {
            let fields: String = t.fields.iter().map(|f| type_code(plan, plan.fields[f.0 as usize].ty)).collect();
            format!("{{{fields}}}")
        }
    }
}

/// The document's attribute rules (`host/web/src/document.rs`, `Walk::element`),
/// for a static prop: `(attributes, text content, extra CSS)`.
fn attributes(
    element: &str,
    props: &exact_kernel::SortedMap<String, String>,
) -> (Vec<(String, String)>, Option<String>, String) {
    let mut attrs = Vec::new();
    let mut content = None;
    let mut css = String::new();
    for (name, value) in props {
        match name.as_str() {
            "scrollFollowEnd" | "scrollTop" | "scrollLeft" => {}
            "autofocus" => {
                if value == "true" {
                    attrs.push((name.clone(), String::new()));
                }
            }
            "src" if element == "img" && value.starts_with("symbol:") => {}
            "text" => {
                if element != "canvas" && !value.is_empty() {
                    content = Some(value.clone());
                }
            }
            "data-action" => {
                attrs.push((name.clone(), value.clone()));
                css.push_str("touch-action:none;");
            }
            "value" => match element {
                "input" | "button" => attrs.push((name.clone(), value.clone())),
                "textarea" => content = Some(value.clone()),
                _ => {}
            },
            "checked" | "inert" | "disabled" | "readonly" => {
                if value == "true" {
                    attrs.push((name.clone(), String::new()));
                }
            }
            "autoplay" | "controls" | "loop" | "muted" | "playsinline" | "disablepictureinpicture"
            | "disableremoteplayback"
                if element == "video" =>
            {
                if value == "true" {
                    attrs.push((name.clone(), String::new()));
                }
            }
            "href" if !exact_web::host::document::navigable(value) => {}
            "src" if element == "iframe" && !exact_web::host::document::navigable(value) => {
                attrs.push((name.clone(), "about:blank".into()));
            }
            _ => attrs.push((name.clone(), value.clone())),
        }
    }
    (attrs, content, css)
}

impl Em<'_> {
    fn class(&mut self, css: &str) -> usize {
        match self.classes.iter().position(|c| c == css) {
            Some(i) => i,
            None => {
                self.classes.push(css.to_string());
                self.classes.len() - 1
            }
        }
    }

    fn children(&mut self, list: &[Site], parent: &str, scope: &Scope) -> Result<(), String> {
        for s in list.to_vec() {
            match s {
                Site::Node(n) => self.node(n, parent, scope)?,
                Site::Region(r) => self.region(r, parent, scope)?,
            }
        }
        Ok(())
    }

    fn f(&mut self, code: exact_plan::Code, scope: &Scope) -> Result<String, String> {
        code::function(self.plan, self.plan.code(code), scope, 0, &mut self.uses)
    }

    fn node(&mut self, i: u32, parent: &str, scope: &Scope) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.nodes[i as usize];
        let node_type = NodeType::from_wire(row.node_type).ok_or("unknown node type")?;
        if row.surface.is_some() {
            self.warnings.push(format!(
                "node {i}: a canvas surface (GPU, a loaded capability) is not drawn; its children are"
            ));
        }
        if node_type == NodeType::Head {
            let mut fields = Vec::new();
            for b in row.bindings.iter() {
                let b = plan.binding(b);
                let prop = PropId::from_wire(b.id).ok_or("unknown prop")?;
                match style::literal(plan, plan.code(b.expr)) {
                    Some(Value::Str(s)) => fields.push(format!(
                        "{}:{}",
                        serde_json::to_string(prop.name()).unwrap(),
                        serde_json::to_string(&*s).unwrap()
                    )),
                    _ => self.warnings.push(format!("head {}: a dynamic head field is not in the spike", prop.name())),
                }
            }
            let hd = self.uses.rt("hd");
            let _ = write!(self.out, "{hd}({{{}}});", fields.join(","));
            return Ok(());
        }
        let parts = self.parts[i as usize].clone().ok_or("no parts")?;
        let element = if parts.tag == "canvas" { "div" } else { parts.tag.as_str() };
        let (mut attrs, content, extra) = attributes(element, &parts.props);
        let mut css = parts.css.clone();
        css.push_str(&extra);
        if element == "a" {
            attrs.push(("data-view".into(), String::new()));
        }
        let kinds: Vec<EventKind> = row.handlers.iter().map(|h| plan.handler(h).event).collect();
        if !kinds.is_empty() {
            attrs.push((
                "data-exact-on".into(),
                kinds.iter().map(|k| k.name()).collect::<Vec<_>>().join(" "),
            ));
        }
        if kinds.iter().any(|k| matches!(k, EventKind::Focus | EventKind::Blur | EventKind::Key))
            && !matches!(element, "input" | "button")
        {
            attrs.push(("tabindex".into(), "0".into()));
        }
        let class = if css.is_empty() { "0".to_string() } else { format!("{}", self.class(&css) + 1) };
        let attrs_js = if attrs.is_empty() {
            "0".to_string()
        } else {
            format!(
                "{{{}}}",
                attrs
                    .iter()
                    .map(|(k, v)| format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap(),
                        serde_json::to_string(v).unwrap()
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let text = content.map(|t| serde_json::to_string(&t).unwrap()).unwrap_or("0".into());
        let h = self.uses.rt("h");
        let e = format!("e{i}");
        let _ = write!(self.out, "const {e}={h}({parent},\"{element}\",{class},{attrs_js},{text});");
        if parts.tag == "canvas" {
            let cv = self.uses.rt("cv");
            let _ = write!(self.out, "{cv}({e});");
        }
        if element == "video" && parts.props.get("muted").map(String::as_str) == Some("true") {
            let _ = write!(self.out, "{e}.muted=!0;");
        }
        for b in row.bindings.iter() {
            let b = plan.binding(b);
            if style::literal(plan, plan.code(b.expr)).is_some() {
                continue;
            }
            let f = self.f(b.expr, scope).map_err(|x| format!("node {i}: {x}"))?;
            match b.kind {
                BindingKind::Prop => {
                    let prop = PropId::from_wire(b.id).ok_or("unknown prop")?;
                    let name = style::prop_name(node_type, prop)?;
                    let p = self.uses.rt("P");
                    let _ = write!(self.out, "{p}({e},{},{f});", serde_json::to_string(&name).unwrap());
                }
                BindingKind::Style => {
                    let (name, kind) = style::style_row(b.id).map_err(|x| format!("node {i}: {x}"))?;
                    let s = self.uses.rt("S");
                    let _ = write!(self.out, "{s}({e},\"{name}\",\"{kind}\",{f});");
                }
            }
        }
        for h in row.handlers.iter() {
            let h = plan.handler(h);
            let mut args = Vec::new();
            for a in h.args.iter() {
                let f = self.f(plan.arg(a).expr, scope)?;
                args.push(format!("({f})()"));
            }
            match h.event {
                EventKind::Press
                | EventKind::Change
                | EventKind::Input
                | EventKind::Hover
                | EventKind::Focus
                | EventKind::Blur
                | EventKind::Key
                | EventKind::Submit
                | EventKind::Load
                | EventKind::Message
                | EventKind::Contextmenu
                | EventKind::Dblclick
                | EventKind::Play
                | EventKind::Playing
                | EventKind::Pause
                | EventKind::Ended
                | EventKind::Error
                | EventKind::Timeupdate
                | EventKind::Durationchange
                | EventKind::Loadedmetadata
                | EventKind::Canplay
                | EventKind::Waiting
                | EventKind::Seeking
                | EventKind::Seeked
                | EventKind::Ratechange
                | EventKind::Volumechange => {}
                k => return Err(format!("node {i}: the `{}` event is not in the spike", k.name())),
            }
            let on = self.uses.rt("on");
            args.push("...v".into());
            let _ = write!(
                self.out,
                "{on}({e},\"{}\",(...v)=>a_{}({}));",
                h.event.name(),
                h.action.0,
                args.join(",")
            );
        }
        self.children(self.sites.of_node(i), &e, scope)?;
        Ok(())
    }

    fn region(&mut self, r: u32, parent: &str, scope: &Scope) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.regions[r as usize];
        let subject = self.f(row.subject, scope).map_err(|x| format!("region {r}: {x}"))?;
        let arms: Vec<u32> = row.arms.iter().map(|a| a.0).collect();
        match row.kind {
            RegionKind::When | RegionKind::Match => {
                let bound = (row.kind == RegionKind::Match).then(|| format!("b{r}"));
                let mut inner = scope.clone();
                inner.frames.push(Frame { bound: bound.clone(), ..Frame::default() });
                let mut bodies = Vec::new();
                for (k, arm) in arms.iter().enumerate() {
                    let saved = std::mem::take(&mut self.out);
                    // `match`'s none arm holds no binding.
                    let sc = if k == 0 { inner.clone() } else {
                        let mut s = scope.clone();
                        s.frames.push(Frame::default());
                        s
                    };
                    self.children(self.sites.of_arm(*arm), "p", &sc)?;
                    let built = std::mem::replace(&mut self.out, saved);
                    let params = match (&bound, k) {
                        (Some(b), 0) => format!("(p,{b})"),
                        _ => "p".into(),
                    };
                    bodies.push(format!("{params}=>{{{built}}}"));
                }
                while bodies.len() < 2 {
                    bodies.push("0".into());
                }
                let f = self.uses.rt(if row.kind == RegionKind::When { "when" } else { "match" });
                let _ = write!(self.out, "{f}({parent},{subject},{},{});", bodies[0], bodies[1]);
            }
            RegionKind::Each => {
                let (item, index) = (format!("i{r}"), format!("x{r}"));
                let mut inner = scope.clone();
                inner.frames.push(Frame {
                    item: Some(item.clone()),
                    index: Some(index.clone()),
                    bound: None,
                });
                let key = code::function(plan, plan.code(row.key), &inner, 0, &mut self.uses)
                    .map_err(|x| format!("region {r} key: {x}"))?;
                let saved = std::mem::take(&mut self.out);
                self.children(self.sites.of_arm(arms[0]), "p", &inner)?;
                let built = std::mem::replace(&mut self.out, saved);
                let each = self.uses.rt("each");
                let _ = write!(
                    self.out,
                    "{each}({parent},{subject},({item},{index})=>({key})(),(p,{item},{index})=>{{{built}}});"
                );
            }
        }
        Ok(())
    }
}
