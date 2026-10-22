//! `explain` of an icon and a param, and `describe`'s icon `draws` (LLP
//! 1121 stage 9 C1, *1.12*): one computation, two readers (theme2 19:25).

use crate::tamber_ops::files_json;
use contract::tamber_params::AppParams;
use contract_lower::params::ParamToken;
use serde_json::{json, Value};
use std::path::Path;

/// `{target: source}`: which of an icon's sources each target the app at
/// `dir` ships draws (`sf`, `svg`, or null for none), by D10's rule; its
/// SVG read under the assets of the root whose file writes it (`app`'s
/// package's, stage 13), as the build reads it.
pub(crate) fn draws(dir: &Path, app: &AppParams, icon: &exact_tamber::Icon) -> Value {
    let home = contract_lower::icons::Home::of(icon, Some(dir), &app.packages, &app.tokens);
    let by: serde_json::Map<String, Value> = contract_lower::icons::draws(icon, dir, home)
        .into_iter()
        .map(|(target, source)| (target, json!(source)))
        .collect();
    Value::Object(by)
}

/// `explain` of an icon, an icon param's too: its `describe` entry, its
/// value as written, and its chain (the icon or param its value names).
/// `None` for any other name.
pub(crate) fn icon(
    dir: &Path,
    design: &str,
    app: &AppParams,
    snap: &[(String, String, String)],
    folding: &exact_tamber::Folding,
    name: &str,
) -> Option<Value> {
    let icon = folding.icons.iter().find(|i| i.name == name)?;
    let param = app.tokens.iter().find(|p| p.token == name);
    let declared = param.map(|p| self::param(dir, design, p, app, snap));
    // An icon's value as its design writes it (the folded design has an
    // icon naming another replaced by that one's `icon(…)`); a package's
    // icon the app doesn't set, as its package's design does (stage 13).
    let line = |text: &str| {
        exact_tamber::parse(text).ok().and_then(|d| {
            d.icons
                .iter()
                .find(|t| t.name.value == name)
                .map(|t| t.value.value.clone())
        })
    };
    let written = snap
        .iter()
        .find(|(f, ..)| f == design)
        .and_then(|(_, text, _)| line(text))
        .or_else(|| app.packages.iter().find_map(|p| line(&p.text)));
    let value = match &declared {
        Some(d) => d["default"].as_str().map(str::to_string),
        None => written.clone().or_else(|| {
            folding
                .design
                .icons
                .iter()
                .find(|t| t.name.value == name)
                .map(|t| t.value.value.clone())
        }),
    };
    // An icon param's chain: the param or the icon token its default names,
    // as the compiler decoded it (its default rewritten: a param by its IR
    // name), shown by name; an icon its line names (below).
    let decoded = param
        .map(|p| p.param.default.trim().to_string())
        .unwrap_or_default();
    let names: Vec<String> = match param {
        Some(_) => app
            .tokens
            .iter()
            .find(|q| q.token == decoded)
            .map(|q| q.full.clone())
            .or_else(|| {
                folding
                    .design
                    .icons
                    .iter()
                    .find(|t| t.name.value == decoded)
                    .map(|t| t.name.value.clone())
            })
            .into_iter()
            .collect(),
        // An icon naming another (`icon.alias = icon.home`): that one.
        None => written
            .map(|w| w.trim().to_string())
            .filter(|w| folding.icons.iter().any(|i| i.name == *w))
            .into_iter()
            .collect(),
    };
    let entry = json!({"name": param.map_or(icon.name.as_str(), |p| p.full.as_str()), "sf": icon.sf, "material": icon.material, "svg": icon.svg, "draws": draws(dir, app, icon)});
    let mut out = json!({"token": param.map_or(name, |p| p.full.as_str()), "kind": "icon", "value": value, "whens": [], "dials": [], "names": names, "icon": entry, "files": files_json(snap)});
    if let Some(d) = declared {
        out["param"] = d;
    }
    Some(out)
}

/// A param's declaration (`explain`'s `param`): its component, kind,
/// place, default as written, and `set`, the design's line that sets the
/// app's value, where there is one.
pub(crate) fn param(
    dir: &Path,
    design: &str,
    p: &ParamToken,
    app: &AppParams,
    snap: &[(String, String, String)],
) -> Value {
    let span = p.param.span;
    let file = app
        .files
        .get(span.source_id as usize)
        .cloned()
        .unwrap_or_default();
    let d = p.param.default_span;
    // The default as written: its line's text at its span, from the
    // snapshot the answer reads (an owned file), else the file (a package's).
    let default = snap
        .iter()
        .find(|(f, ..)| *f == file)
        .map(|(_, t, _)| t.clone())
        // A package's file read where it is (round 2, Astra 3): its name is no path.
        .or_else(|| {
            let path = app.paths.get(span.source_id as usize).cloned();
            std::fs::read_to_string(path.unwrap_or_else(|| dir.join(&file))).ok()
        })
        .and_then(|t| {
            let line = t
                .lines()
                .nth((d.line as usize).checked_sub(1)?)?
                .to_string();
            line.get((d.col as usize).checked_sub(1)?..(d.end_col as usize).checked_sub(1)?)
                .map(str::to_string)
        })
        .unwrap_or_else(|| p.param.default.clone());
    let component = p.identity.rsplit('#').next().unwrap_or(&p.identity);
    let short = format!("{component}.{}", p.param.name);
    let set = snap
        .iter()
        .find(|(f, ..)| f == design)
        .and_then(|(_, text, _)| exact_tamber::parse(text).ok())
        .and_then(|parsed| {
            parsed
                .params
                .iter()
                .find(|e| e.name.value == p.full || e.name.value == short)
                .map(|e| json!({"file": design, "line": e.name.line, "value": e.value.value}))
        });
    json!({"component": component, "identity": p.identity, "kind": p.param.kind, "file": file, "line": span.line, "col": span.col, "default": default, "set": set})
}

/// `uses` of an icon (stage 9 C2, *1.12*), or of an icon param (`name`
/// its IR token): the plan nodes that draw it, directly, through a design
/// icon naming it (`icon.alias = icon.base`), or through an icon param
/// whose value (the design's line by its full name, or its short name
/// where only one component has it; else its default as decoded) names it
/// or a param that does, to a fixed point; a param's identity kept.
pub(crate) fn icon_uses(
    dir: &Path,
    design: &str,
    app: &AppParams,
    snap: &[(String, String, String)],
    name: &str,
) -> Result<Value, Value> {
    let parsed = snap
        .iter()
        .find(|(f, ..)| f == design)
        .and_then(|(_, text, _)| exact_tamber::parse(text).ok());
    let set: Vec<(String, String)> = parsed
        .as_ref()
        .map(|d| {
            d.params
                .iter()
                .map(|e| (e.name.value.clone(), e.value.value.clone()))
                .collect()
        })
        .unwrap_or_default();
    let queried = app.tokens.iter().find(|p| p.token == name);
    // The icon and the design's icons naming it, to a fixed point.
    let mut icons: Vec<String> = queried
        .is_none()
        .then(|| name.to_string())
        .into_iter()
        .collect();
    if let Some(d) = &parsed {
        loop {
            let more: Vec<String> = d
                .icons
                .iter()
                .filter(|t| {
                    !icons.contains(&t.name.value)
                        && icons.contains(&t.value.value.trim().to_string())
                })
                .map(|t| t.name.value.clone())
                .collect();
            if more.is_empty() {
                break;
            }
            icons.extend(more);
        }
    }
    let short = |p: &ParamToken| {
        format!(
            "{}.{}",
            p.identity.rsplit('#').next().unwrap_or(&p.identity),
            p.param.name
        )
    };
    // A short name the build reads only where one component has it.
    let mut counts: std::collections::HashMap<String, usize> = Default::default();
    for p in &app.tokens {
        *counts.entry(short(p)).or_default() += 1;
    }
    let unique = |p: &ParamToken| counts.get(&short(p)) == Some(&1);
    // Each icon param's value, once.
    let values: Vec<(&ParamToken, String)> = app
        .tokens
        .iter()
        .filter(|p| p.param.kind == "icon")
        .map(|p| {
            let line = set
                .iter()
                .find(|(n, _)| *n == p.full || (unique(p) && *n == short(p)))
                .map(|(_, v)| v.clone());
            let v = line.unwrap_or_else(|| p.param.default.clone());
            (p, v.trim().trim_matches('"').trim().to_string())
        })
        .collect();
    // The params that reach it: the one asked about, then any whose value
    // names it (or one of its icons) or one already reached.
    let mut reach: Vec<&ParamToken> = queried.into_iter().collect();
    loop {
        let more: Vec<&ParamToken> = values
            .iter()
            .filter(|(p, _)| !reach.iter().any(|q| q.full == p.full))
            .filter(|(p, v)| {
                icons.contains(v)
                    || reach.iter().any(|q| {
                        q.token == *v
                            || q.identity == p.identity
                                && v.strip_prefix("param.") == Some(q.param.name.as_str())
                    })
            })
            .map(|(p, _)| *p)
            .collect();
        if more.is_empty() {
            break;
        }
        reach.extend(more);
    }
    // Each read with the reference it goes through; the thing asked about
    // is read directly.
    let reads: Vec<(String, Option<String>)> = icons
        .iter()
        .map(|i| (format!("symbol:{i}"), (i != name).then(|| i.clone())))
        .collect();
    let params: Vec<(String, String, String, Option<String>)> = reach
        .iter()
        .map(|p| {
            let via = queried
                .is_none_or(|q| q.full != p.full)
                .then(|| p.full.clone());
            (
                p.identity.clone(),
                p.param.name.clone(),
                p.token.clone(),
                via,
            )
        })
        .collect();
    let (plan, map) = contract::compile_path_mapped(&dir.join("app.contract"))
        .map_err(|e| json!({"message": format!("the app does not build: {}", e.message)}))?;
    let map: Value = serde_json::from_str(&map.json(&plan.encode()))
        .map_err(|e| json!({"message": format!("source map: {e}")}))?;
    let mut sites =
        contract::tamber_uses::icon_sites(&dir.join("app.contract"), &map, &reads, &params)
            .map_err(|e| json!({"message": format!("the app does not build: {e}")}))?;
    contract::tamber_uses::relative_files(&mut sites, dir);
    let aliases: Vec<&str> = icons
        .iter()
        .filter(|i| *i != name)
        .map(String::as_str)
        .chain(params.iter().filter_map(|(.., via)| via.as_deref()))
        .collect();
    let token = queried.map_or(name, |q| q.full.as_str());
    Ok(
        json!({"token": token, "plan": map["digest"], "sites": sites, "aliases": aliases, "files": files_json(snap)}),
    )
}
