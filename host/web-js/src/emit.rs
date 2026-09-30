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
use exact_kernel::{NodeType, PropId, StyleId};
use exact_plan::{BindingKind, EventKind, Plan, RegionKind, Value};
use exact_web::host::template::Parts;
use std::fmt::Write as _;

#[path = "motion.rs"]
mod motion;
#[path = "rows.rs"]
mod rows;

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
    pub names: String,
    /// The locations rendered at build (`render=build` routes without
    /// parameters, and the not-found page), as JSON.
    pub pages: String,
    /// Whether a node renders Markdown (the page fetches `markdown.wasm`).
    pub markdown: bool,
    /// Whether a node is a Canvas 2D surface (the page loads its engine,
    /// `canvas2d.js`, and the data module draws).
    pub canvas2d: bool,
    /// Whether the plan uses motion (the page fetches `motion.wasm`).
    pub motion: bool,
    /// Whether a text field is a Markdown editor (`markup-editor.wasm`).
    pub editor: bool,
    /// Whether a node has a `wrap-flow` row (`textflow.wasm`).
    pub flow: bool,
    /// The declared faces' preloads, for the page's head.
    pub preloads: String,
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
        text @ exact_plan::str_value!() => serde_json::to_string(text.text()).unwrap(),
        Value::Unit => "null".into(),
        Value::Option(None) => "null".into(),
        Value::Option(Some(x)) => value_js(x),
        Value::List(items) | Value::Record(items) => {
            format!(
                "[{}]",
                items.iter().map(value_js).collect::<Vec<_>>().join(",")
            )
        }
    }
}

pub fn dump(plan: &Plan) {
    let mut uses = Uses::default();
    let s = Scope::default();
    let f = |code: exact_plan::Code, uses: &mut Uses, scope: &Scope, params: usize| {
        code::function(plan, plan.code(code), scope, params, uses)
            .unwrap_or_else(|e| format!("<{e}>"))
    };
    eprintln!("router: {:?}", plan.router);
    for (i, r) in plan.slots.iter().enumerate() {
        eprintln!(
            "slot {i} {} = {}",
            plan.str(r.name),
            f(r.init, &mut uses, &s, 0)
        );
    }
    for (i, r) in plan.derives.iter().enumerate() {
        eprintln!(
            "derive {i} {} = {}",
            plan.str(r.name),
            f(r.body, &mut uses, &s, 0)
        );
    }
    for (i, r) in plan.resources.iter().enumerate() {
        eprintln!(
            "resource {i} {} = {}(..{})",
            plan.str(r.name),
            plan.str(r.source),
            r.args.len
        );
    }
    let a = Scope {
        action: true,
        ..Scope::default()
    };
    for (i, r) in plan.actions.iter().enumerate() {
        eprintln!(
            "action {i} {} = {}",
            plan.str(r.name),
            f(r.body, &mut uses, &a, r.params.len as usize)
        );
    }
    for (i, r) in plan.regions.iter().enumerate() {
        eprintln!(
            "region {i} {:?} parent {:?} arm {:?} order {} arms {:?}",
            r.kind, r.parent, r.arm, r.order, r.arms
        );
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
    row_actions: std::collections::BTreeSet<usize>,
    markdown: bool,
    canvas2d: bool,
    motion: bool,
    editor: bool,
    flow: bool,
    /// Whether a virtualized list is in the plan (`list.js` is imported).
    list: bool,
    /// Whether an image draws a symbol (`symbols.js`), and whether a
    /// binding names one (its roles are then the plan's strings).
    symbols: (bool, bool),
    /// Each height drag handle's owner (`heightDragFor`, resolved as the
    /// kernel resolves it), by node.
    heights: std::collections::BTreeMap<u32, u32>,
    /// Each transform drag handle's target and clip, by node.
    transforms: std::collections::BTreeMap<u32, (u32, u32)>,
    /// Each Arrange grip's list (`reorderFor`), by node.
    reorders: std::collections::BTreeMap<u32, u32>,
    /// The `@keyframes` that animate `scale` (a pressed node plays their
    /// `-exact-press` copies).
    press_keyframes: Vec<String>,
    /// Whether a dynamic row can reference an element (`url(#…)`): nodes
    /// with an `id` then name it (`data-exact-id`, rt.js `Sr`).
    refs: bool,
}

/// The runner's reserved sources the JS runtime answers itself: the page's
/// facts, never the build's (`exactTime` in the entry, the rest in
/// facts.js). `exactDelivery` answers what the build baked (facts.js).
const HOST_FACTS: &[&str] = &["exactViewport", "exactTime", "exactPage", "exactSurface"];

pub fn emit(plan: &Plan) -> Result<Output, String> {
    let fonts = crate::faces::fonts(plan)?;
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
        row_actions: Default::default(),
        markdown: false,
        canvas2d: false,
        motion: false,
        editor: false,
        flow: false,
        list: false,
        symbols: (false, false),
        heights: Default::default(),
        transforms: Default::default(),
        reorders: Default::default(),
        press_keyframes: rows::press_keyframes(plan),
        refs: rows::can_refer(plan),
    };
    em.heights = em.height_targets();
    em.transforms = em.transform_targets();
    em.reorders = em.reorder_lists();
    let top = Scope::default();
    let action = Scope {
        action: true,
        ..Scope::default()
    };
    let mut body = String::new();
    // The route table, and the router slot filled from the location before
    // any initializer (LLP 1038 D5).
    if let Some(slot) = plan.router {
        let rows: Vec<String> = plan
            .routes
            .iter()
            .map(|r| {
                format!(
                    "[{},{},{},{},{}]",
                    serde_json::to_string(plan.str(r.name)).unwrap(),
                    serde_json::to_string(plan.str(r.pattern)).unwrap(),
                    r.parent.map_or(-1, |p| p.0 as i64),
                    r.tab as u8,
                    r.notfound as u8
                )
            })
            .collect();
        let (routes, launch, sig) = (
            em.uses.rt("routes"),
            em.uses.rt("launch"),
            em.uses.rt("sig"),
        );
        let _ = write!(
            body,
            "{routes}([{}]);const s_{}={sig}({launch}(location.pathname+location.search));",
            rows.join(","),
            slot.0
        );
    }
    // Localized strings (LLP 1060): the tables, and the locale slot (the
    // base) before any initializer, which may call `t`; after boot the page's
    // locale picks the table (rt.js `language`, the runner's `set_place`).
    if let Some(slot) = plan.locale {
        let tables: Vec<String> = plan
            .locales
            .iter()
            .map(|row| {
                let texts: Vec<String> = plan.texts[row.texts.start as usize..]
                    [..row.texts.len as usize]
                    .iter()
                    .map(|t| {
                        format!(
                            "{}:{}",
                            serde_json::to_string(plan.str(t.key)).unwrap(),
                            serde_json::to_string(plan.str(t.text)).unwrap()
                        )
                    })
                    .collect();
                format!(
                    "[{},{},{{{}}}]",
                    serde_json::to_string(plan.str(row.name)).unwrap(),
                    row.rtl as u8,
                    texts.join(",")
                )
            })
            .collect();
        let init = code::expression(plan, plan.code(plan.slot(slot).init), &top, &mut em.uses)?;
        let (strings, sig) = (em.uses.rt("strings"), em.uses.rt("sig"));
        let _ = write!(
            body,
            "{strings}([{}]);const s_{}={sig}({init},\"s\");",
            tables.join(","),
            slot.0
        );
    }
    // Slots, in order: an initializer reads only earlier slots.
    for (i, r) in plan.slots.iter().enumerate() {
        if r.owner.is_some()
            || plan.router == Some(exact_plan::SlotsId(i as u32))
            || plan.locale == Some(exact_plan::SlotsId(i as u32))
        {
            continue; // a row slot lives on its row; the router and locale are above
        }
        let init = code::expression(plan, plan.code(r.init), &top, &mut em.uses)
            .map_err(|e| format!("slot {}: {e}", plan.str(r.name)))?;
        let sig = em.uses.rt("sig");
        let _ = write!(
            body,
            "const s_{i}={sig}({init},{});",
            serde_json::to_string(&type_code(plan, r.ty)).unwrap()
        );
    }
    for (i, r) in plan.derives.iter().enumerate() {
        let f = code::function(plan, plan.code(r.body), &top, 0, &mut em.uses)
            .map_err(|e| format!("derive {}: {e}", plan.str(r.name)))?;
        let memo = em.uses.rt("memo");
        let _ = write!(
            body,
            "const d_{i}={memo}({f},{});",
            serde_json::to_string(&type_code(plan, r.ty)).unwrap()
        );
    }
    // The reserved sources facts.js answers, their declared fields filled by name.
    let facts = crate::facts::reserved(plan, &mut body)?;
    for (i, r) in plan.resources.iter().enumerate() {
        let mut args = Vec::new();
        for a in r.args.iter() {
            let f = code::expression(plan, plan.code(plan.arg(a).expr), &top, &mut em.uses)
                .map_err(|e| format!("resource {}: {e}", plan.str(r.name)))?;
            args.push(f);
        }
        // What it shows while it waits with nothing kept (LLP 1048.003 D6,
        // 1054.000.002): an `else source()` row's value, a declared
        // `else empty(…)`, or the type's zero; a placeholder row has none.
        let is_placeholder = plan
            .resources
            .iter()
            .any(|o| o.placeholder.map(|p| p.0 as usize) == Some(i));
        let placeholder = match (r.placeholder, plan.bytes(r.placeholder_value)) {
            (Some(p), _) => format!("()=>r_{}()", p.0),
            (None, b) if !b.is_empty() => {
                value_js(&Value::from_bytes(b).map_err(|e| e.to_string())?)
            }
            _ if is_placeholder => "void 0".into(),
            _ => zero(plan, r.ty),
        };
        // A host fact is the page's; a store reader's value a placeholder (LLP 1027 D4).
        let fact = HOST_FACTS.contains(&plan.str(r.source));
        let initial = plan.bytes(r.initial);
        let initial = if initial.is_empty() || fact {
            "void 0".to_string()
        } else {
            value_js(&Value::from_bytes(initial).map_err(|e| e.to_string())?)
        };
        let initial_args = plan.bytes(r.initial_args);
        let initial_args = if initial_args.is_empty() || fact || r.reader {
            "void 0".to_string()
        } else {
            value_js(&Value::from_bytes(initial_args).map_err(|e| e.to_string())?)
        };
        let res = em.uses.rt("res");
        let _ = write!(
            body,
            "const r_{i}={res}({},{},()=>[{}],{initial},{initial_args},{},{placeholder});",
            serde_json::to_string(plan.str(r.name)).unwrap(),
            serde_json::to_string(plan.str(r.source)).unwrap(),
            args.join(","),
            serde_json::to_string(&type_code(plan, r.ty)).unwrap()
        );
    }
    for (i, m) in plan.mutations.iter().enumerate() {
        let refreshes: Vec<String> = m
            .refreshes
            .iter()
            .map(|x| format!("r_{}", plan.mutation_refreshes[x.0 as usize].resource.0))
            .collect();
        let mt = em.uses.rt("mut");
        let _ = write!(
            body,
            "const m_{i}={mt}({},s_{},[{}],{});",
            serde_json::to_string(plan.str(m.name)).unwrap(),
            m.slot.0,
            refreshes.join(","),
            serde_json::to_string(&type_code(plan, m.ty)).unwrap()
        );
    }
    for (i, r) in plan.actions.iter().enumerate() {
        // An action that touches row slots takes the row in force first.
        let rows = code::touches_rows(plan, plan.code(r.body));
        let scope = Scope {
            rows: rows.then(|| "$r".to_string()),
            ..action.clone()
        };
        let mut f = code::function(
            plan,
            plan.code(r.body),
            &scope,
            r.params.len as usize,
            &mut em.uses,
        )
        .map_err(|e| format!("action {}: {e}", plan.str(r.name)))?;
        if rows {
            em.row_actions.insert(i);
            f = f.replacen('(', "($r,", 1).replace("($r,)", "($r)");
        }
        let act = em.uses.rt("act");
        let _ = write!(body, "const a_{i}={act}({f});");
    }
    for (i, m) in plan.mutations.iter().enumerate() {
        if let Some(a) = m.then {
            let _ = write!(body, "m_{i}.then=a_{};", a.0);
        }
    }
    em.out.clear();
    em.node(sites.root, "$R", &top)?;
    let view = std::mem::take(&mut em.out);
    // Symbols (symbols.js): a bound source picks its role from the plan's
    // strings that name one, `symbol:`-prefixed or not.
    if em.symbols.0 {
        let mut roles = Vec::new();
        if em.symbols.1 {
            for s in &plan.strings {
                let role = s.strip_prefix("symbol:").unwrap_or(s);
                if let Some((_, path, filled)) = exact_kernel::generated::symbol(role) {
                    let entry = format!(
                        "{}:[{},{}]",
                        serde_json::to_string(role).unwrap(),
                        serde_json::to_string(path).unwrap(),
                        filled as u8
                    );
                    if !roles.contains(&entry) {
                        roles.push(entry);
                    }
                }
            }
        }
        let _ = write!(body, "$symbols({{{}}});", roles.join(","));
    }
    let mount = em.uses.rt("mount");
    let _ = write!(body, "{mount}($R=>{{{view}}});");
    // A plan whose actions read geometry fetches the page's reader after
    // first paint, as the wasm host does for an artifact that imports it.
    if em.uses.names.contains("x_frame") || em.uses.names.contains("x_measure") {
        let geo = em.uses.rt("geo");
        let _ = write!(body, "{geo}();");
    }
    if let Some(slot) = plan.locale {
        let language = em.uses.rt("language");
        let _ = write!(body, "{language}(s_{});", slot.0);
    }
    if let Some(slot) = plan.router {
        let router = em.uses.rt("router");
        let _ = write!(body, "{router}(s_{},$navigation);", slot.0);
    }
    // The state's getters, for the agent (names live in `names.js`).
    let list = |p: &str, n: usize| {
        (0..n)
            .map(|i| format!("{p}_{i}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let _ = write!(
        body,
        "const $state=[[{}],[{}],[{}]];",
        plan.slots
            .iter()
            .enumerate()
            .filter(|(_, r)| r.owner.is_none())
            .map(|(i, _)| format!("s_{i}"))
            .collect::<Vec<_>>()
            .join(","),
        list("d", plan.derives.len()),
        list("r", plan.resources.len())
    );
    for t in plan.timers.iter() {
        if t.frame {
            // LLP 1073: once per presented frame, virtual frames on a seek.
            let frames = em.uses.rt("frames");
            let _ = write!(body, "{frames}(a_{});", t.action.0);
            continue;
        }
        let every = em.uses.rt("every");
        let _ = write!(
            body,
            "{every}({},a_{},{});",
            t.interval_ms, t.action.0, t.once as u8
        );
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
        let params: Vec<String> = r
            .params
            .iter()
            .map(|p| type_code(plan, plan.source_params[p.0 as usize].ty))
            .collect();
        sources.push(format!(
            "{}:{}",
            serde_json::to_string(plan.str(r.name)).unwrap(),
            serde_json::to_string(&params.concat()).unwrap()
        ));
    }
    let names = |v: Vec<&str>| serde_json::to_string(&v).unwrap();
    let types = |v: Vec<exact_plan::TypesId>| {
        format!(
            "[{}]",
            v.into_iter()
                .map(|t| type_json(plan, t))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let names_js = format!(
        "export default[{},{},{}];export const types=[{},{},{}];\n",
        names(
            plan.slots
                .iter()
                .filter(|r| r.owner.is_none())
                .map(|r| plan.str(r.name))
                .collect()
        ),
        names(plan.derives.iter().map(|r| plan.str(r.name)).collect()),
        names(plan.resources.iter().map(|r| plan.str(r.name)).collect()),
        types(
            plan.slots
                .iter()
                .filter(|r| r.owner.is_none())
                .map(|r| r.ty)
                .collect()
        ),
        types(plan.derives.iter().map(|r| r.ty).collect()),
        types(plan.resources.iter().map(|r| r.ty).collect())
    );
    // Each source's parameter and result types, by field name, for a
    // TypeScript module (records are objects there).
    let source_types: Vec<String> = plan
        .sources
        .iter()
        .map(|r| {
            let params: Vec<String> = r
                .params
                .iter()
                .map(|p| type_json(plan, plan.source_params[p.0 as usize].ty))
                .collect();
            format!(
                "{}:[[{}],{}]",
                serde_json::to_string(plan.str(r.name)).unwrap(),
                params.join(","),
                type_json(plan, r.ty)
            )
        })
        .collect();
    // Each route's render and activation policy, for the renderers (LLP 1048.003 D5).
    let pages: Vec<String> = plan
        .routes
        .iter()
        .map(|r| {
            format!(
                "[\"{}\",\"{}\",{}]",
                r.render.name(),
                r.activate.name(),
                r.notfound as u8
            )
        })
        .collect();
    let names_js = format!(
        "{names_js}export const sourceTypes={{{}}};export const pages=[{}];\n",
        source_types.join(","),
        pages.join(",")
    );
    let imports: Vec<String> = em.uses.names.iter().cloned().collect();
    let js = format!(
        "// Generated by exact-web-js from the app's plan. Do not edit.\nimport{{{}}}from\"./rt.js\";{}{}\nexport const sources={{{}}};export const wait={};export default function(){{{body}return $state}}\n",
        imports.join(","),
        if plan.router.is_some() { "import{navigation as $navigation}from\"./navigation.js\";" } else { "" },
        // Loaded pieces, imported only where the plan uses them.
        [
            (em.list, "import{vl as $vl}from\"./list.js\";"),
            (!facts.is_empty(), facts.as_str()),
            (em.symbols.0, "import{symbols as $symbols}from\"./symbols.js\";"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, i)| *i)
        .collect::<String>(),
        sources.join(","),
        // A resource with no compiled value: the page waits for its source.
        false
    );
    // Canvas text names a declared family by its declared name (LLP 1056 D8).
    let js = if em.canvas2d && !fonts.aliases.is_empty() {
        format!(
            "{js}(globalThis.exact??={{}}).fontAliases={};\n",
            fonts.aliases
        )
    } else {
        js
    };
    let mut css = fonts.css.clone();
    css.push_str("#exact-root#exact-root{");
    for (i, c) in em.classes.iter().enumerate() {
        let _ = write!(css, ".c{}{{{c}}}", i + 1);
    }
    css.push('}');
    // The plan's `@keyframes` (LLP 1055 D5, D7), as the document's head
    // carries them: the browser runs `animation` rows.
    for row in &plan.keyframes {
        if let Ok(frames) = exact_motion::Keyframes::parse(plan.str(row.css)) {
            let text = frames.css();
            let _ = write!(css, "@keyframes {}{{{text}}}", plan.str(row.name));
            // A pressable node plays the copy that also animates
            // `--exact-scale` (exact_web::css::keyframes_name).
            if text.contains("scale:") {
                let mut press = String::new();
                let mut rest = text.as_str();
                while let Some(at) = rest.find("scale:") {
                    let Some(end) = rest[at..].find(';').map(|e| at + e) else {
                        break;
                    };
                    press.push_str(&rest[..=end]);
                    press.push_str("--exact-");
                    press.push_str(&rest[at..=end]);
                    rest = &rest[end + 1..];
                }
                press.push_str(rest);
                let _ = write!(
                    css,
                    "@keyframes {}-exact-press{{{press}}}",
                    plan.str(row.name)
                );
            }
        }
    }
    Ok(Output {
        js,
        css,
        names: names_js,
        pages: build_pages(plan),
        markdown: em.markdown,
        canvas2d: em.canvas2d,
        motion: em.motion,
        editor: em.editor,
        flow: em.flow,
        preloads: fonts.preloads,
        viewport,
        warnings: em.warnings,
    })
}

fn build_pages(plan: &Plan) -> String {
    let rows: Vec<String> = plan
        .routes
        .iter()
        .filter(|r| {
            r.render == exact_plan::RenderPolicy::Build && !plan.str(r.pattern).contains(':')
        })
        .map(|r| {
            let location = if r.notfound {
                "/404"
            } else {
                plan.str(r.pattern)
            };
            format!(
                "{{\"location\":{},\"notfound\":{}}}",
                serde_json::to_string(location).unwrap(),
                r.notfound
            )
        })
        .collect();
    format!("[{}]", rows.join(","))
}

/// The type's zero, as the runner's `zero` makes it.
fn zero(plan: &Plan, ty: exact_plan::TypesId) -> String {
    let t = &plan.types[ty.0 as usize];
    match t.kind {
        exact_plan::TypeKind::Number => "0".into(),
        exact_plan::TypeKind::Bool => "!1".into(),
        exact_plan::TypeKind::String => "\"\"".into(),
        exact_plan::TypeKind::Unit | exact_plan::TypeKind::Option => "null".into(),
        exact_plan::TypeKind::List => "[]".into(),
        exact_plan::TypeKind::Record => format!(
            "[{}]",
            t.fields
                .iter()
                .map(|f| zero(plan, plan.fields[f.0 as usize].ty))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// A type for the agent's typed JSON: `"n"`, `"b"`, `"s"`, `"u"`,
/// `["?",T]`, `["[",T]`, `{"field":T,…}` in field order.

pub(crate) fn type_json(plan: &Plan, ty: exact_plan::TypesId) -> String {
    let t = &plan.types[ty.0 as usize];
    match t.kind {
        exact_plan::TypeKind::Number => "\"n\"".into(),
        exact_plan::TypeKind::Bool => "\"b\"".into(),
        exact_plan::TypeKind::String => "\"s\"".into(),
        exact_plan::TypeKind::Unit => "\"u\"".into(),
        exact_plan::TypeKind::Option => format!(
            "[\"?\",{}]",
            type_json(plan, t.elem.expect("option element"))
        ),
        exact_plan::TypeKind::List => {
            format!("[\"[\",{}]", type_json(plan, t.elem.expect("list element")))
        }
        exact_plan::TypeKind::Record => format!(
            "{{{}}}",
            t.fields
                .iter()
                .map(|f| {
                    let f = &plan.fields[f.0 as usize];
                    format!(
                        "{}:{}",
                        serde_json::to_string(plan.str(f.name)).unwrap(),
                        type_json(plan, f.ty)
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
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
        exact_plan::TypeKind::Option => {
            format!("?{}", type_code(plan, t.elem.expect("option element")))
        }
        exact_plan::TypeKind::List => {
            format!("[{}", type_code(plan, t.elem.expect("list element")))
        }
        exact_plan::TypeKind::Record => {
            let fields: String = t
                .fields
                .iter()
                .map(|f| type_code(plan, plan.fields[f.0 as usize].ty))
                .collect();
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
            // An authored `autofocus=false` is kept for the agent's tree
            // (the runner reports it); the attribute would mean true.
            "autofocus" => {
                if value == "true" {
                    attrs.push((name.clone(), String::new()));
                } else {
                    attrs.push(("data-autofocus".into(), "false".into()));
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
            "autoplay"
            | "controls"
            | "loop"
            | "muted"
            | "playsinline"
            | "disablepictureinpicture"
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
        for s in list.iter().copied() {
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

        if node_type == NodeType::Head {
            let mut fields = Vec::new();
            for b in row.bindings.iter() {
                let b = plan.binding(b);
                let prop = PropId::from_wire(b.id).ok_or("unknown prop")?;
                match style::literal(plan, plan.code(b.expr)) {
                    Some(text @ exact_plan::str_value!()) => fields.push(format!(
                        "{}:{}",
                        serde_json::to_string(prop.name()).unwrap(),
                        serde_json::to_string(text.text()).unwrap()
                    )),
                    _ => {
                        let f = self.f(b.expr, scope)?;
                        fields.push(format!(
                            "{}:{f}",
                            serde_json::to_string(prop.name()).unwrap()
                        ));
                    }
                }
            }
            let hd = self.uses.rt("hd");
            let _ = write!(self.out, "{hd}({parent},{{{}}});", fields.join(","));
            return Ok(());
        }
        let parts = self.parts[i as usize].clone().ok_or("no parts")?;
        let mut virtualized = false;
        for b in row.bindings.iter().map(|b| plan.binding(b)) {
            if b.kind == BindingKind::Prop && b.id == PropId::Virtualized as u16 {
                match style::literal(plan, plan.code(b.expr)) {
                    Some(Value::Bool(v)) => virtualized = v,
                    _ => {
                        return Err(format!(
                            "node {i}: a dynamic `virtualized` is not in the JS target"
                        ))
                    }
                }
            }
        }
        let element = if parts.tag == "canvas" {
            "div"
        } else {
            parts.tag.as_str()
        };
        if element == "img" {
            let bound = row.bindings.iter().map(|b| plan.binding(b)).any(|b| {
                b.kind == BindingKind::Prop
                    && b.id == PropId::ImageSource as u16
                    && style::literal(plan, plan.code(b.expr)).is_none()
            });
            if bound || parts.props.contains_key("data-symbol-path") {
                self.symbols.0 = true;
                self.symbols.1 |= bound;
            }
        }
        let (mut attrs, content, extra) = attributes(element, &parts.props);
        let mut css = parts.css.clone();
        css.push_str(&extra);
        // @ref LLP 1063 — `layout-transition` and `exit-animation` are custom
        // properties the web host's presence-glue.js reads from the element's
        // own declaration: inline, as the live host writes every row, not
        // the class (a class's custom property would be inherited).
        let presence = rows::presence_decls(&mut css);
        // A node with press feedback: the web host's input piece shows it.
        if presence.contains("--exact-press:") {
            let press = self.uses.rt("pressFeedback");
            let _ = write!(self.out, "{press}();");
        }
        if element == "a" {
            attrs.push(("data-view".into(), String::new()));
        }
        if let Some(id) = self.exact_id(i) {
            attrs.push(("data-exact-id".into(), id));
        }
        let kinds: Vec<EventKind> = row.handlers.iter().map(|h| plan.handler(h).event).collect();
        if !kinds.is_empty() {
            attrs.push((
                "data-exact-on".into(),
                kinds.iter().map(|k| k.name()).collect::<Vec<_>>().join(" "),
            ));
        }
        if kinds
            .iter()
            .any(|k| matches!(k, EventKind::Focus | EventKind::Blur | EventKind::Key))
            && !matches!(element, "input" | "button")
        {
            attrs.push(("tabindex".into(), "0".into()));
        }
        // A `symbol`'s content is drawn as `use`'s clones, which Chrome
        // styles from their own attributes, not the page's class rules: its
        // static rows go inline too (the live host writes every row inline).
        if !css.is_empty() && self.in_symbol(i) {
            attrs.push(("style".into(), format!("{css}{presence}")));
        } else if !presence.is_empty() {
            attrs.push(("style".into(), presence.clone()));
        }
        if parts.tag == "canvas" {
            attrs.extend(style::canvas_bitmap(plan, row)?);
        }
        let class = if css.is_empty() {
            "0".to_string()
        } else {
            format!("{}", self.class(&css) + 1)
        };
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
        let text = content
            .map(|t| serde_json::to_string(&t).unwrap())
            .unwrap_or("0".into());
        // An SVG node's element is in SVG's namespace (element.rs's tag).
        let h = self
            .uses
            .rt(if format!("{node_type:?}").starts_with("Svg") {
                "hs"
            } else {
                "h"
            });
        let e = format!("e{i}");
        let _ = write!(
            self.out,
            "const {e}={h}({parent},\"{element}\",{class},{attrs_js},{text});"
        );
        if !presence.is_empty() {
            let pr = self.uses.rt("pr");
            let _ = write!(self.out, "{pr}({e});");
        }
        if parts.tag == "canvas" {
            let cv = self.uses.rt("cv");
            let _ = write!(self.out, "{cv}({e});");
        }
        if parts.tag.contains('-') {
            let _ = write!(self.out, "{}({e});", self.uses.rt("nm")); // a native module, LLP 1024 D3
        }
        // Its surface's inputs, named or positional (LLP 1009 D2).
        if let Some(sf) = row.surface {
            let sf = &plan.surfaces[sf.0 as usize];
            // A surface the app's GPU module draws (the build names them)
            // goes to it; any other is a Canvas 2D surface, drawn by the
            // data module and replayed by the web host's own glue.
            let name = plan.str(sf.name);
            let gpu = gpu_surfaces().iter().any(|s| s == name);
            let named = sf.mode == exact_plan::SurfaceArgsMode::Named;
            if !gpu {
                // Positional values, their declared types where known, and
                // the authored names (LLP 1056 D1: a draw's `args`).
                self.canvas2d = true;
                let values = sf
                    .args
                    .iter()
                    .map(|a| {
                        let a = &plan.surface_args[a.0 as usize];
                        code::expression(plan, plan.code(a.expr), scope, &mut self.uses)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let types: Vec<String> = sf
                    .args
                    .iter()
                    .map(|a| {
                        let a = &plan.surface_args[a.0 as usize];
                        crate::faces::arg_type(plan, plan.code(a.expr)).map_or("0".into(), |t| {
                            serde_json::to_string(&type_code(plan, t)).unwrap()
                        })
                    })
                    .collect();
                let names: Vec<String> = if named {
                    sf.args
                        .iter()
                        .map(|a| {
                            serde_json::to_string(plan.str(plan.surface_args[a.0 as usize].name))
                                .unwrap()
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                let c2 = self.uses.rt("c2");
                let _ = write!(
                    self.out,
                    "{c2}({e},{},()=>[{}],[{}],[{}]);",
                    serde_json::to_string(name).unwrap(),
                    values.join(","),
                    types.join(","),
                    names.join(",")
                );
            } else {
                let mut values = Vec::new();
                for a in sf.args.iter() {
                    let a = &plan.surface_args[a.0 as usize];
                    let v = code::expression(plan, plan.code(a.expr), scope, &mut self.uses)?;
                    values.push(if named {
                        format!("{}:{v}", serde_json::to_string(plan.str(a.name)).unwrap())
                    } else {
                        v
                    });
                }
                let gs = self.uses.rt("gs");
                let body = if named {
                    format!("({{{}}})", values.join(","))
                } else {
                    format!("[{}]", values.join(","))
                };
                let _ = write!(
                    self.out,
                    "{gs}({e},{},()=>{body});",
                    serde_json::to_string(name).unwrap()
                );
            }
        }
        if element == "video" && parts.props.get("muted").map(String::as_str) == Some("true") {
            let _ = write!(self.out, "{e}.muted=!0;");
        }
        // A `markup="markdown"` text builds its pieces (LLP 1045 D3).
        let markdown = node_type == NodeType::Text
            && row.bindings.iter().any(|b| {
                let b = plan.binding(b);
                b.kind == BindingKind::Prop
                    && b.id == PropId::Markup as u16
                    && matches!(style::literal(plan, plan.code(b.expr)), Some(v) if v.as_str() == Some("markdown"))
            });
        if markdown {
            self.markdown = true;
        }
        self.editor(i, element, &e)?;
        for b in row.bindings.iter() {
            let b = plan.binding(b);
            if style::literal(plan, plan.code(b.expr)).is_some() {
                continue;
            }
            let f = self
                .f(b.expr, scope)
                .map_err(|x| format!("node {i}: {x}"))?;
            let at = self.out.len();
            match b.kind {
                BindingKind::Prop if markdown && b.id == PropId::Text as u16 => {
                    let md = self.uses.rt("md");
                    let _ = write!(self.out, "{md}({e},{f});");
                }
                BindingKind::Prop => {
                    let prop = PropId::from_wire(b.id).ok_or("unknown prop")?;
                    let name = style::prop_name(node_type, prop)?;
                    let p = self.uses.rt("P");
                    let _ = write!(
                        self.out,
                        "{p}({e},{},{f});",
                        serde_json::to_string(&name).unwrap()
                    );
                }
                BindingKind::Style => self.style_row(i, b, &parts, &e, &f)?,
            }
            // The row item's fields it reads (LLP 1071.000 D3).
            if let Some(m) = crate::reads::field_mask(&f).filter(|_| self.out.len() > at) {
                let (fm, stmt) = (self.uses.rt("fm"), self.out.split_off(at));
                let _ = write!(self.out, "{fm}({m},()=>{{{stmt}}});");
            }
        }
        self.motion_node(i, &e, scope)?;
        self.height_owner(i, &e, scope)?;
        if let Some(l) = self.reorders.get(&i) {
            self.motion = true;
            let on = self.uses.rt("onReorder");
            let _ = write!(self.out, "{on}({e},e{l});");
        }
        self.wrap_flow(i, &e, scope)?;
        let mut edges = ["0".to_string(), "0".to_string()];
        for h in row.handlers.iter() {
            let h = plan.handler(h);
            let mut args = Vec::new();
            for a in h.args.iter() {
                let f = code::expression(plan, plan.code(plan.arg(a).expr), scope, &mut self.uses)?;
                args.push(f);
            }
            match h.event {
                EventKind::Navigate
                | EventKind::Press
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
                | EventKind::Volumechange
                | EventKind::Scroll
                | EventKind::Refresh
                | EventKind::Pan
                | EventKind::Cancel
                | EventKind::Select => {}
                // The motion piece's: the swipe's holds, a pan's velocity.
                EventKind::Swiperight
                | EventKind::Panrelease
                | EventKind::Heightrelease
                | EventKind::Transformgeometry
                | EventKind::Transformrelease => self.motion = true,
                EventKind::Reachstart | EventKind::Reachend if virtualized => {}
                EventKind::Reorderdrop if virtualized => self.motion = true,
                k => {
                    return Err(format!(
                        "node {i}: the `{}` event is not in the JS target",
                        k.name()
                    ))
                }
            }
            let on = self.uses.rt("on");
            if self.row_actions.contains(&(h.action.0 as usize)) {
                args.insert(0, scope.rows.clone().unwrap_or_else(|| "{}".into()));
            }
            let handler = if args.is_empty() {
                format!("a_{}", h.action.0)
            } else {
                args.push("...v".into());
                format!("(...v)=>a_{}({})", h.action.0, args.join(","))
            };
            // The motion and input pieces' events (rt.js), only where used.
            let piece = match h.event {
                EventKind::Swiperight => Some("onSwipe"),
                EventKind::Pan => Some("onPan"),
                EventKind::Panrelease => Some("onPanRelease"),
                EventKind::Select => Some("onSelect"),
                EventKind::Heightrelease => Some("onHeight"),
                EventKind::Transformgeometry => Some("onTGeom"),
                EventKind::Transformrelease => Some("onTRelease"),
                EventKind::Reorderdrop => Some("onDrop"),
                _ => None,
            };
            if let Some(piece) = piece {
                let f = self.uses.rt(piece);
                let owner = match (h.event, self.heights.get(&i), self.transforms.get(&i)) {
                    (EventKind::Heightrelease, Some(t), _) => format!(",e{t}"),
                    (EventKind::Transformrelease, _, Some((t, c))) => format!(",e{t},e{c}"),
                    _ => String::new(),
                };
                let _ = write!(self.out, "{f}({e},{handler}{owner});");
                continue;
            }
            if h.event == EventKind::Navigate {
                let nav = self.uses.rt("navigateTo");
                let _ = write!(self.out, "{nav}({handler});");
                continue;
            }
            // A list's edges are the runner's, from its window (list.js).
            if matches!(h.event, EventKind::Reachstart | EventKind::Reachend) {
                edges[(h.event == EventKind::Reachend) as usize] = handler;
                continue;
            }
            let _ = write!(self.out, "{on}({e},\"{}\",{handler});", h.event.name());
        }
        if virtualized {
            let opts = self.list_options(i, scope, &edges)?;
            let [Site::Region(r)] = self.sites.of_node(i) else {
                return Err(format!(
                    "node {i}: a virtualized list needs one direct `each`"
                ));
            };
            let r = *r;
            if plan.regions[r as usize].kind != RegionKind::Each {
                return Err(format!(
                    "node {i}: a virtualized list needs one direct `each`"
                ));
            }
            self.list = true;
            return self.each(r, &e, scope, Some(opts));
        }
        self.children(self.sites.of_node(i), &e, scope)?;
        Ok(())
    }

    /// What the runner reads when a virtualized list is created
    /// (`Collection::create`), evaluated there: its axis, its literal sizes,
    /// its row estimate, `scrollFollowEnd` (read again at each update),
    /// `scroll-restoration`, its edges' handlers, and its plan site.
    fn list_options(
        &mut self,
        i: u32,
        scope: &Scope,
        edges: &[String; 2],
    ) -> Result<String, String> {
        let plan = self.plan;
        let (mut ph, mut pw) = (Vec::new(), Vec::new());
        let (mut x, mut est, mut follow, mut manual) = (
            "!1".to_string(),
            "void 0".to_string(),
            "0".to_string(),
            "!1".to_string(),
        );
        let mut init = "void 0".to_string();
        for b in plan.nodes[i as usize]
            .bindings
            .iter()
            .map(|b| plan.binding(b))
        {
            let value = match style::literal(plan, plan.code(b.expr)) {
                Some(v) => value_js(&v),
                None => code::expression(plan, plan.code(b.expr), scope, &mut self.uses)
                    .map_err(|x| format!("node {i}: {x}"))?,
            };
            match b.kind {
                BindingKind::Style
                    if b.id == StyleId::Height as u16 || b.id == StyleId::MaxHeight as u16 =>
                {
                    ph.push(value)
                }
                BindingKind::Style
                    if b.id == StyleId::Width as u16 || b.id == StyleId::MaxWidth as u16 =>
                {
                    pw.push(value)
                }
                BindingKind::Style if b.id == StyleId::Display as u16 => {
                    x = format!("({value})===\"flex\"")
                }
                BindingKind::Prop
                    if b.id == PropId::EstimatedItemHeight as u16
                        || b.id == PropId::EstimatedItemWidth as u16 =>
                {
                    est = value
                }
                BindingKind::Prop if b.id == PropId::ScrollFollowEnd as u16 => {
                    follow = format!("()=>{value}")
                }
                BindingKind::Prop if b.id == PropId::InitialItemCount as u16 => init = value,
                BindingKind::Prop if b.id == PropId::ScrollRestoration as u16 => {
                    manual = format!("({value})===\"manual\"")
                }
                _ => {}
            }
        }
        Ok(format!(
            "{{x:{x},ph:[{}],pw:[{}],est:{est},init:{init},follow:{follow},manual:{manual},start:{},end:{},site:\"{i}\"}}",
            ph.join(","),
            pw.join(","),
            edges[0],
            edges[1]
        ))
    }

    fn region(&mut self, r: u32, parent: &str, scope: &Scope) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.regions[r as usize];
        let subject = self
            .f(row.subject, scope)
            .map_err(|x| format!("region {r}: {x}"))?;
        let arms: Vec<u32> = row.arms.iter().map(|a| a.0).collect();
        match row.kind {
            RegionKind::When | RegionKind::Match => {
                let bound = (row.kind == RegionKind::Match).then(|| format!("b{r}"));
                let mut inner = scope.clone();
                inner.frames.push(Frame {
                    bound: bound.clone(),
                    ..Frame::default()
                });
                let mut bodies = Vec::new();
                for (k, arm) in arms.iter().enumerate() {
                    let saved = std::mem::take(&mut self.out);
                    // `match`'s none arm holds no binding.
                    let sc = if k == 0 {
                        inner.clone()
                    } else {
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
                let f = self.uses.rt(if row.kind == RegionKind::When {
                    "when"
                } else {
                    "match"
                });
                let _ = write!(
                    self.out,
                    "{f}({parent},{subject},{},{});",
                    bodies[0], bodies[1]
                );
            }
            RegionKind::Each => self.each(r, parent, scope, None)?,
        }
        Ok(())
    }

    /// An `each`, or a virtualized list's rows when `list` carries the
    /// list's options (list.js `vl`).
    fn each(
        &mut self,
        r: u32,
        parent: &str,
        scope: &Scope,
        list: Option<String>,
    ) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.regions[r as usize];
        let subject = self
            .f(row.subject, scope)
            .map_err(|x| format!("region {r}: {x}"))?;
        let arms: Vec<u32> = row.arms.iter().map(|a| a.0).collect();
        let (item, index) = (format!("i{r}"), format!("x{r}"));
        let mut inner = scope.clone();
        inner.frames.push(Frame {
            item: Some(item.clone()),
            index: Some(index.clone()),
            bound: None,
        });
        let key = code::expression(plan, plan.code(row.key), &inner, &mut self.uses)
            .map_err(|x| format!("region {r} key: {x}"))?;
        // The row's own slots, started from their initializers when
        // the row is created and kept with its key (LLP 1017 P4c).
        let mut own = Vec::new();
        for (k, slot) in plan.slots.iter().enumerate() {
            if slot.owner.map(|o| o.0) == Some(r) {
                let init = code::expression(plan, plan.code(slot.init), &inner, &mut self.uses)
                    .map_err(|x| format!("row slot {}: {x}", plan.str(slot.name)))?;
                let sig = self.uses.rt("sig");
                own.push(format!("{k}:{sig}({init})"));
            }
        }
        let mut rows_decl = String::new();
        if !own.is_empty() {
            let name = format!("$r{r}");
            rows_decl = match &scope.rows {
                Some(outer) => format!("const {name}={{...{outer},{}}};", own.join(",")),
                None => format!("const {name}={{{}}};", own.join(",")),
            };
            inner.rows = Some(name);
        }
        let saved = std::mem::take(&mut self.out);
        self.out.push_str(&rows_decl);
        self.children(self.sites.of_arm(arms[0]), "p", &inner)?;
        let built = std::mem::replace(&mut self.out, saved);
        match list {
            Some(opts) => {
                let _ = write!(
                    self.out,
                    "$vl({parent},{subject},({item},{index})=>{key},(p,{item},{index})=>{{{built}}},{opts});"
                );
            }
            None => {
                let each = self.uses.rt("each");
                let pure = if crate::reads::pure_key(&key, &item, &index) {
                    ",1"
                } else {
                    ""
                };
                let _ = write!(
                    self.out,
                    "{each}({parent},{subject},({item},{index})=>{key},(p,{item},{index})=>{{{built}}}{pure});"
                );
            }
        }
        Ok(())
    }
}

/// The surfaces the app's GPU module draws, as the build names them
/// (`EXACT_JS_GPU_SURFACES`, comma-separated; host/web-js/build.mjs).
fn gpu_surfaces() -> Vec<String> {
    std::env::var("EXACT_JS_GPU_SURFACES")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}
