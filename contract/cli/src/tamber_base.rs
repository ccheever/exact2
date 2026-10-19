//! The base scale on the wire (LLP 1121 stage 11 D, `tamber/1.14`; theme2's
//! base-row shape): `describe`'s `base` and `targets`, `resolve` and `explain`
//! of a base name, and `explain`'s `bases` for a token whose values reach one.
//! Every cell is read from the kernel's scale table (`style::scale`), never a
//! copy; an app's own line for a base name is its `override`.

use super::*;
use exact_kernel::style::scale::{self, ScaleSource, ScaleValue};
use serde_json::{json, Value};

/// A cell's value as the shape writes it: a CSS length (`16px`), `full`
/// (radius.full, the pill), or the terminal's `<n>cell` (`0` for none).
fn value(v: ScaleValue) -> String {
    match v {
        ScaleValue::Px(_) => scale::css(v),
        ScaleValue::Cells(0.0) => "0".into(),
        ScaleValue::Cells(n) => format!("{n}cell"),
        ScaleValue::Full => "full".into(),
    }
}

/// A cell's source as the shape writes it.
fn source(s: ScaleSource) -> Value {
    match s {
        ScaleSource::Metric(m) => json!({"kind": "metric", "ref": m}),
        ScaleSource::Measured(id) => json!({"kind": "measured", "id": id}),
        ScaleSource::Guideline(id) => json!({"kind": "guideline", "cite": id}),
        ScaleSource::Full => json!({"kind": "full"}),
        ScaleSource::Terminal => json!({"kind": "terminal"}),
    }
}

/// The design file a request names (`file`, else the app's).
fn file(s: &Server, request: &Value) -> Result<String, Value> {
    Ok(optional(request, "file")?.unwrap_or_else(|| s.design.clone()))
}

/// One base row: its kind, every column's cell with its source, and the
/// line of the design asked about that sets the name (`override`), or null.
fn row(name: &str, file: &str, design: &exact_tamber::Design) -> Option<Value> {
    let r = scale::row(name)?;
    let columns: serde_json::Map<String, Value> = scale::COLUMNS
        .iter()
        .map(|(key, column)| {
            let cell = scale::cell(r, *column);
            (
                key.to_string(),
                json!({"value": value(cell.value), "source": source(cell.source)}),
            )
        })
        .collect();
    let set = design.tokens.iter().find(|t| t.name.value == name);
    let over = set.map_or(
        Value::Null,
        |t| json!({"file": file, "line": t.name.line, "col": t.name.col, "text": t.value.value}),
    );
    let kind = if name.starts_with("radius.") {
        "radius"
    } else {
        "length"
    };
    Some(json!({"name": name, "kind": kind, "columns": columns, "override": over}))
}

/// The base names a token reaches, as strings, each once (no order promised):
/// the unset ones its own values read bare (every value the checker folds,
/// a `when` no tuple selects and a type's size included), the set ones it
/// names, and those of the tokens it names, through any chain.
fn bases_of(folding: &exact_tamber::Folding, name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    let mut stack = vec![name];
    while let Some(n) = stack.pop() {
        if seen.contains(&n) {
            continue;
        }
        seen.push(n);
        let Some(t) = folding.tokens.iter().find(|t| t.name == n) else {
            continue;
        };
        for b in t
            .bases
            .iter()
            .chain(t.names.iter().filter(|m| scale::is_base(m)))
        {
            if !out.contains(b) {
                out.push(b.clone());
            }
        }
        // (named tokens next, in the order written)
        stack.extend(t.names.iter().rev().map(String::as_str));
    }
    out
}

/// `describe`'s base rows and targets, from the design it describes.
fn base_rows(
    s: &Server,
    request: &Value,
    design: &exact_tamber::Design,
) -> Result<(Value, Value), Value> {
    let file = file(s, request)?;
    let rows = json!(scale::names()
        .filter_map(|n| row(n, &file, design))
        .collect::<Vec<_>>());
    let targets = json!(contract_lower::icons::shipped_targets(&s.dir)
        .iter()
        .map(|t| json!({"target": t, "column": scale::column_for(t)}))
        .collect::<Vec<_>>());
    Ok((rows, targets))
}

impl Server {
    pub(super) fn describe(&self, request: &Value) -> Result<Value, Value> {
        // The app's components' params (LLP 1121 D15) fold with the design,
        // so a dial only a param reads is read; listed by full name.
        let app = self.app_params();
        let all = app.tokens.as_slice();
        let (snap, folding, design) = self.design_with(request, &app)?;
        let mut ir: Value =
            serde_json::from_str(&folding.ir()).map_err(|e| json!({"message": e.to_string()}))?;
        if !all.is_empty() {
            contract::tamber_params::hide(&mut ir);
            ir["components"] =
                contract::tamber_params::components(&app, Some(&design), &self.vocab);
        }
        let meta =
            |m: &[(String, String)]| m.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>();
        let dials: serde_json::Map<String, Value> = folding
            .design
            .dials
            .iter()
            .map(|d| (d.name.value.clone(), json!(meta(&d.meta))))
            .collect();
        // By the IR's names: a role's row spelling, not the key written.
        let tokens: serde_json::Map<String, Value> = folding
            .tokens
            .iter()
            .zip(&folding.design.tokens)
            .filter(|(t, _)| !t.name.starts_with(contract_lower::params::PREFIX))
            .map(|(t, declared)| (t.name.clone(), json!(meta(&declared.meta))))
            .collect();
        // What each token is drawn on (`pair=`): a token by its name, a role
        // no token overrides with its fallback; and the contrast warnings.
        let pairs: serde_json::Map<String, Value> = folding
            .tokens
            .iter()
            .filter(|t| !t.pairs.is_empty() && !t.name.starts_with(contract_lower::params::PREFIX))
            .map(|t| {
                let named = t.pairs.iter().map(|p| {
                    let token = folding.tokens.iter().any(|u| &u.name == p);
                    let fallback = (!token)
                        .then(|| exact_tamber::fallback(&exact_tamber::Folded::Host(exact_tamber::Host::Role(p.clone())), &self.vocab))
                        .flatten();
                    json!({"name": p, "token": token, "fallback": fallback.map(|[l, d]| json!({"light": l, "dark": d}))})
                });
                (t.name.clone(), json!(named.collect::<Vec<_>>()))
            })
            .collect();
        let file = optional(request, "file")?.unwrap_or_else(|| self.design.clone());
        let warnings: Vec<Value> = folding
            .warnings
            .iter()
            .map(|d| located(&file, d, &app))
            .collect();
        // Each icon's entry says which source each shipped target draws
        // (LLP 1121 stage 9 C, *1.12*).
        if let Some(icons) = ir["icons"].as_array_mut() {
            for entry in icons {
                if let Some(icon) = folding.icons.iter().find(|i| entry["name"] == i.name) {
                    entry["draws"] = crate::tamber_explain::draws(&self.dir, icon);
                }
            }
        }
        // The base scale (*1.14*): every base name's row, this design's
        // overrides, and the targets the app ships with their columns.
        let (base, targets) = base_rows(self, request, &design)?;
        Ok(
            json!({"ir": ir, "files": files_json(&snap), "meta": {"dials": dials, "tokens": tokens}, "pairs": pairs, "warnings": warnings, "base": base, "targets": targets}),
        )
    }

    /// A token's stored value per tuple and scheme; a host value (a role, a
    /// platform colour) is a residual, labelled so, with its fallback.
    pub(super) fn resolve(&self, request: &Value) -> Result<Value, Value> {
        let (snap, folding, name) = self.lookup(request)?;
        // A base name (*1.14*), not a param named like one: unset, its row
        // and no fold (each column's value); set, its token's tuples and row.
        let base = (required(request, "token")? == name)
            .then(|| {
                row(
                    &name,
                    &file(self, request).unwrap_or_default(),
                    &folding.design,
                )
            })
            .flatten();
        if let Some(base) = &base {
            if base["override"].is_null() {
                return Ok(json!({"name": name, "base": base, "files": files_json(&snap)}));
            }
        }
        // An icon (D10): its sources, the same in every tuple.
        if let Some(icon) = folding.icons.iter().find(|i| i.name == name) {
            let tuples: Vec<Value> = (0..folding.tuples)
                .map(|t| json!({"modes": modes_of(&folding.design.axes, t), "kind": "icon", "sf": icon.sf, "material": icon.material, "svg": icon.svg}))
                .collect();
            return Ok(json!({"token": name, "tuples": tuples, "files": files_json(&snap)}));
        }
        let token = folding
            .tokens
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| json!({"message": format!("no token `{name}`")}))?;
        let axes = &folding.design.axes;
        let tuples: Vec<Value> = token
            .values
            .iter()
            .enumerate()
            .map(|(tuple, v)| {
                let modes = modes_of(axes, tuple);
                let v = match v {
                    // A length's stored form: px, or the unit or named
                    // length it keeps, whose px is the build's (its text
                    // size, `resolve-on`).
                    exact_tamber::Value::Length(l) => {
                        let (unit, amount) = match l {
                            exact_tamber::Length::Px(n) => ("px", json!(n)),
                            exact_tamber::Length::Rem(n) => ("rem", json!(n)),
                            exact_tamber::Length::Em(n) => ("em", json!(n)),
                            exact_tamber::Length::Named(_) => ("named", Value::Null),
                        };
                        return json!({"modes": modes, "value": v.css(), "kind": "length", "unit": unit, "amount": amount});
                    }
                    exact_tamber::Value::Number(n) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "number", "amount": n});
                    }
                    // Each shadow: its lengths in px and its colour's CSS
                    // (null for a text shadow's currentcolor).
                    exact_tamber::Value::Shadow(s) => {
                        let shadows: Vec<Value> = s
                            .list
                            .iter()
                            .map(|one| json!({"inset": one.inset, "lengths": one.lengths, "color": one.color.as_ref().map(|c| c.css())}))
                            .collect();
                        return json!({"modes": modes, "value": v.css(), "kind": if s.text { "text-shadow" } else { "box-shadow" }, "shadows": shadows});
                    }
                    // A motion token: a time in ms, or an easing's text.
                    exact_tamber::Value::Duration(ms) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "duration", "ms": ms});
                    }
                    exact_tamber::Value::Easing(_) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "easing"});
                    }
                    // A type token's longhands as written (null: unsaid), a
                    // family's stack (stage 6 C, optional fields under 1.9).
                    exact_tamber::Value::Type(t) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "type", "style": t.style, "weight": t.weight, "size": t.size.css(), "leading": t.leading, "family": t.family});
                    }
                    exact_tamber::Value::Family(f) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "family", "family": f});
                    }
                    exact_tamber::Value::Color(c) => c,
                };
                match v {
                    // `srgb`: each side's 8-bit sRGB clip, what a contrast
                    // ratio is taken on (`light` and `dark` keep the wide value).
                    exact_tamber::Folded::One(h) => {
                        json!({"modes": modes, "value": v.css(), "light": h.css(), "dark": h.css(), "srgb": {"light": h.clip(), "dark": h.clip()}})
                    }
                    exact_tamber::Folded::Pair(l, d) => {
                        json!({"modes": modes, "value": v.css(), "light": l.css(), "dark": d.css(), "srgb": {"light": l.clip(), "dark": d.clip()}})
                    }
                    exact_tamber::Folded::Host(_) => {
                        let fallback = exact_tamber::fallback(v, &self.vocab)
                            .map(|[l, d]| json!({"light": l, "dark": d}));
                        json!({"modes": modes, "value": v.css(), "residual": true, "fallback": fallback})
                    }
                }
            })
            .collect();
        // The digests it read, so a client reading several tokens can tell
        // one revision from two.
        let mut out = json!({"token": name, "tuples": tuples, "files": files_json(&snap)});
        if let Some(base) = base {
            out["base"] = base;
        }
        Ok(out)
    }

    /// A token's chain: its default and `when` values, the dials it depends
    /// on, and the tokens those values name, as the checker resolved them.
    pub(super) fn explain(&self, request: &Value) -> Result<Value, Value> {
        let (snap, folding, name) = self.lookup(request)?;
        // An icon, an icon param's too (D10, *1.12*).
        let app = self.app_params();
        // The design asked about (`file`), whose lines set the app's values.
        let design = optional(request, "file")?.unwrap_or_else(|| self.design.clone());
        if let Some(icon) =
            crate::tamber_explain::icon(&self.dir, &design, &app, &snap, &folding, &name)
        {
            return Ok(icon);
        }
        // A base name (*1.14*), not a param named like one: its row, with
        // the app line's explain fields when the design sets it.
        let base = (required(request, "token")? == name)
            .then(|| row(&name, &design, &folding.design))
            .flatten();
        let bases = bases_of(&folding, &name);
        let with = |mut out: Value| {
            if !bases.is_empty() {
                out["bases"] = json!(bases);
            }
            if let Some(base) = &base {
                out["kind"] = json!("base");
                out["name"] = json!(name);
                out["base"] = base.clone();
            }
            out
        };
        if base.as_ref().is_some_and(|b| b["override"].is_null()) {
            return Ok(with(json!({"files": files_json(&snap)})));
        }
        let (token, declared) = folding
            .tokens
            .iter()
            .zip(&folding.design.tokens)
            .find(|(t, _)| t.name == name)
            .ok_or_else(|| no_token(&folding, &name))?;
        let whens: Vec<Value> = declared
            .whens
            .iter()
            .map(|w| {
                json!({"when": w.conds.iter().map(|(a, v)| json!({"axis": a, "value": v})).collect::<Vec<_>>(), "value": w.value.value})
            })
            .collect();
        let mut out = json!({"token": name, "value": declared.value.value, "whens": whens, "dials": token.dials, "names": token.names, "files": files_json(&snap)});
        // A param (D15): where it is declared, its default as written, and
        // the design's line that sets the app's value, *1.12*.
        if let Some(p) = app.tokens.iter().find(|p| p.token == name) {
            out["param"] = crate::tamber_explain::param(&self.dir, &design, p, &app, &snap);
        }
        Ok(with(out))
    }
}
