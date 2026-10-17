//! The JS target's inspect table (LLP 1122 T2b, the JS target): what a
//! development page needs to report a selected node row for row as the
//! runner's `node` op does (runner/src/agent.rs), with no kernel in the
//! page. A separate generated file beside app.js, development builds only
//! (theme2's ruling): release bytes are unchanged.
//!
//! From the static tree the parts are made from (`style::static_tree`, every
//! site a view), for each site: its type; the chain of regions above it
//! (an `each` region's row, keyed at run time, or a conditional's arm); and
//! each row it sets, as `node` gives it: the value in each app tuple (the
//! same words, `row_json`), or none for a row bound by an expression; whether
//! a heading implies it; the tokens its text reads, with the text; and for a
//! row it does not set, the site whose platform text style gives it. A row
//! it inherits is its nearest ancestor's own, found in the page; one nothing
//! sets is the initial value, given once.

use crate::emit::Sites;
use crate::style;
use exact_kernel::{StyleId, StyleProps};
use exact_plan::{BindingKind, Plan, RegionKind};
use exact_runner::{agent, bridge};
use serde_json::{Map, Value as Json, json};

fn row_value(row: StyleId, props: &StyleProps) -> Json {
    let mut s = String::new();
    agent::row_json(row, props.get(row), &mut s);
    serde_json::from_str(&s).unwrap_or(Json::Null)
}

/// The table, as JSON text.
pub fn table(plan: &Plan, sites: &Sites) -> Result<String, String> {
    let tuples = bridge::tuples(plan).max(1);
    // The static tree in each app tuple: a row's value as the kernel
    // resolves it there (a relative length against the font size around it).
    let mut kernels = Vec::new();
    for t in 0..tuples {
        let before = bridge::set_tuple(t);
        let tree = style::static_tree(plan, sites);
        bridge::set_tuple(before);
        kernels.push(tree?.0);
    }
    let kernel = &kernels[0];
    let axes: Vec<Json> = plan
        .axes
        .iter()
        .map(|a| json!([plan.str(a.name), plan.str(a.values).split('|').collect::<Vec<_>>()]))
        .collect();
    // The initial value of every inherited row (an empty style's).
    let empty = StyleProps::default();
    let mut initial = Map::new();
    for row in StyleId::ALL {
        if row.inherited() {
            initial.insert(row.name().into(), row_value(row, &empty));
        }
    }
    let order: Vec<&str> = StyleId::ALL.iter().map(|r| r.name()).collect();
    // Each row's CSS property, as the page writes it (style.rs).
    let css: Map<String, Json> = StyleId::ALL.iter().map(|r| (r.name().to_string(), json!(style::css_property(*r)))).collect();
    let mut out = Map::new();
    for (i, node) in plan.nodes.iter().enumerate() {
        let Some(view) = kernel.node(i as u32 + 1) else {
            continue;
        };
        // The regions above it, outermost first: `[region, arm]`, the arm
        // null for an `each` region's row (its key is the page's).
        let mut chain = Vec::new();
        let mut arm = node.arm;
        while let Some(a) = arm {
            let r = plan.arms[a.0 as usize].region;
            let region = &plan.regions[r.0 as usize];
            let index = a.0 - region.arms.start;
            chain.push(if region.kind == RegionKind::Each {
                json!([r.0, null])
            } else {
                json!([r.0, index])
            });
            arm = region.arm;
        }
        chain.reverse();
        let mut rows = Vec::new();
        let mut given = Map::new();
        let mut start = Map::new();
        for row in StyleId::ALL {
            // The binding that sets it, if a binding does (a class expansion
            // or the compiler may set a row no binding names). An expression's
            // row is in no static tree: it is its own all the same.
            let binding = node.bindings.iter().map(|b| plan.binding(b)).find(|b| {
                b.kind == BindingKind::Style && StyleId::from_bit(b.id as u32) == Some(row)
            });
            // Literal as the runner's `node` says (`vm::is_literal`); the
            // value, as the emitter reads it.
            let literal = binding.and_then(|b| style::literal(plan, plan.code(b.expr)));
            let dynamic = binding.is_some_and(|b| !exact_runner::vm::is_literal(plan.code(b.expr)));
            // A platform text style that gives the row where no own row
            // does (an expression's row cleared falls back to it too).
            if !view.style.mask.has(row) {
                if let Some(from) = view.style_source_of(row) {
                    // Its site, and the value it gives in each tuple.
                    let vs: Vec<Json> = kernels.iter().map(|k| {
                        k.node(i as u32 + 1).map_or(Json::Null, |n| {
                            let mut s = String::new();
                            agent::row_json(row, n.computed(row), &mut s);
                            serde_json::from_str(&s).unwrap_or(Json::Null)
                        })
                    }).collect();
                    let v = if vs.iter().all(|v| *v == vs[0]) { vs[0].clone() } else { json!({ "tuples": vs }) };
                    given.insert(row.name().into(), json!([from - 1, v]));
                }
            }
            // (An expression's row cleared falls back to these too.)
            if !view.style.mask.has(row) {
                if view.style_source_of(row).is_none() && row.inherited() && view.source_of(row).is_none() {
                    // Nothing above sets it: its initial value, which a node's
                    // type can make its own (a button's weight); and kept even
                    // when it is the initial one where a node above sets the
                    // row (a control's start stops it: the page must not
                    // inherit it).
                    let mut s = String::new();
                    agent::row_json(row, view.computed(row), &mut s);
                    let v: Json = serde_json::from_str(&s).unwrap_or(Json::Null);
                    let mut up = view.parent;
                    let mut stopped = false;
                    while let Some(p) = up.and_then(|p| kernel.node(p)) {
                        if p.style.mask.has(row) {
                            stopped = true;
                            break;
                        }
                        up = p.parent;
                    }
                    if stopped || initial.get(row.name()) != Some(&v) {
                        start.insert(row.name().into(), v);
                    }
                }
                if !dynamic {
                    continue;
                }
            }
            let mut values = Vec::new();
            let mut names: Vec<String> = Vec::new();
            match (binding, &literal) {
                (Some(b), Some(v)) if !dynamic => {
                    for (t, k) in kernels.iter().enumerate() {
                        // The value the kernel resolved in that tuple; the
                        // tokens the text reads there.
                        values.push(k.node(i as u32 + 1).map_or(Json::Null, |n| row_value(row, &n.style)));
                        let before = bridge::set_tuple(t as u32);
                        let mut patch = StyleProps::default();
                        let (_, read) = exact_kernel::style::theme::recorded(|| {
                            bridge::set_binding_style(&mut patch, b, v, plan)
                        });
                        bridge::set_tuple(before);
                        for entry in read {
                            let token = plan.theme.iter().find(|tk| {
                                *plan.str(tk.name) == *entry || plan.str(tk.role).eq_ignore_ascii_case(&entry)
                            });
                            let name = token.map_or(entry.to_string(), |tk| plan.str(tk.name).to_string());
                            if !names.contains(&name) {
                                names.push(name);
                            }
                        }
                    }
                }
                // Set by no binding the emitter reads (a class expansion, the
                // compiler): the static tree's own value in each tuple.
                _ if !dynamic => {
                    for k in &kernels {
                        values.push(k.node(i as u32 + 1).map_or(Json::Null, |n| row_value(row, &n.style)));
                    }
                }
                _ => {}
            }
            let text = match (&literal, names.is_empty()) {
                (Some(v @ exact_plan::str_value!()), false) => json!(v.text()),
                (Some(v), false) => {
                    let mut s = String::new();
                    agent::untyped_json(v, &mut s);
                    json!(s)
                }
                _ => Json::Null,
            };
            // One value when every tuple's is the same.
            let value = if values.is_empty() {
                Json::Null
            } else if values.iter().all(|v| *v == values[0]) {
                values[0].clone()
            } else {
                json!({ "tuples": values })
            };
            // A length relative to the element's or the root's font (`em`,
            // `rem`) or to the display (a hairline), as the kernel parses it:
            // the page reads it from the element, whose font and display are
            // the instance's. (A text style's size is its ramp at the 16 px
            // root: a limit.)
            let relative = literal.as_ref().and_then(|v| v.as_str()).is_some_and(|t| {
                matches!(exact_kernel::style::relative::parse(t), Some((exact_kernel::style::relative::Unit::Em | exact_kernel::style::relative::Unit::Rem | exact_kernel::style::relative::Unit::Hairline, _)))
            });
            rows.push(json!({
                "n": row.name(),
                "u": relative,
                "v": value,
                "d": dynamic,
                "h": view.heading_implies(row),
                "k": if names.is_empty() { Json::Null } else { json!(names) },
                "x": text,
            }));
        }
        let mut site = Map::new();
        site.insert("t".into(), json!(view.node_type.name()));
        // A mode boundary (its `mode` or `color-scheme` row, as modes.rs
        // finds it): the inherited colour rows it does not set are its own
        // implied rows (LLP 1121 D7), the page's to find.
        if node.bindings.iter().map(|b| plan.binding(b)).any(|b| b.kind == BindingKind::Style && matches!(StyleId::from_bit(b.id as u32), Some(StyleId::Mode | StyleId::ColorScheme))) {
            site.insert("b".into(), json!(true));
        }
        if !chain.is_empty() {
            site.insert("c".into(), Json::Array(chain));
        }
        if !rows.is_empty() {
            site.insert("r".into(), Json::Array(rows));
        }
        if !given.is_empty() {
            site.insert("s".into(), Json::Object(given));
        }
        if !start.is_empty() {
            site.insert("i".into(), Json::Object(start));
        }
        out.insert(i.to_string(), Json::Object(site));
    }
    // Each token's custom property (a development build's every token): a
    // row an expression binds that names one reads it, which the page maps
    // back to the token.
    let props: Map<String, Json> = crate::modes::properties(plan).into_iter().map(|(name, prop)| (prop, json!(name))).collect();
    // The inherited colour rows a mode boundary writes again (LLP 1121 D7;
    // runner/src/instance/theme.rs `INHERITED`).
    let implied = [StyleId::TextColor, StyleId::CaretColor, StyleId::Fill, StyleId::Stroke, StyleId::AccentColor, StyleId::TextShadow, StyleId::TextStrokeColor, StyleId::SymbolPalette].map(|r| r.name());
    // The design the values were set in (dev-design.json's digest): a page
    // running another (a live edit since) takes a token's value from it.
    let built = crate::modes::design(plan);
    let design = built["digest"].clone();
    // Each token's value in each tuple in that design: a live edit since
    // that left a token as it was leaves its rows' values as the table says.
    let tokens: Map<String, Json> = built["tokens"].as_array().map_or_else(Map::new, |ts| ts.iter().filter_map(|t| Some((t["name"].as_str()?.to_string(), json!([t["value"].clone(), t["slices"].clone()])))).collect());
    // The colour channels a boundary's modes.js keeps (modes.rs CHANNELS), in its order.
    let channels: Vec<&str> = crate::modes::CHANNELS.iter().map(|(r, _)| r.name()).collect();
    Ok(json!({ "axes": axes, "tuples": tuples, "order": order, "css": css, "channels": channels, "initial": initial, "implied": implied, "props": props, "design": design, "tokens": tokens, "plan": contract::plan_digest(&plan.encode()), "sites": out }).to_string())
}
