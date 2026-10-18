//! `explain` of an icon and a param, and `describe`'s icon `draws` (LLP
//! 1121 stage 9 C1, *1.12*): one computation, two readers (theme2 19:25).

use crate::tamber_ops::files_json;
use contract::tamber_params::AppParams;
use contract_lower::params::ParamToken;
use serde_json::{json, Value};
use std::path::Path;

/// `{target: source}`: which of an icon's sources each target the app at
/// `dir` ships draws (`sf`, `svg`, or null for none), by D10's rule.
pub(crate) fn draws(dir: &Path, icon: &exact_tamber::Icon) -> Value {
    let by: serde_json::Map<String, Value> = contract_lower::icons::draws(icon, dir)
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
    // icon naming another replaced by that one's `icon(…)`).
    let written = snap
        .iter()
        .find(|(f, ..)| f == design)
        .and_then(|(_, text, _)| exact_tamber::parse(text).ok())
        .and_then(|d| {
            d.icons
                .iter()
                .find(|t| t.name.value == name)
                .map(|t| t.value.value.clone())
        });
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
    let entry = json!({"name": param.map_or(icon.name.as_str(), |p| p.full.as_str()), "sf": icon.sf, "material": icon.material, "svg": icon.svg, "draws": draws(dir, icon)});
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
        .or_else(|| std::fs::read_to_string(dir.join(&file)).ok())
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
