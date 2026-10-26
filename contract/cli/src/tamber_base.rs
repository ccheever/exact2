//! The base scale on the wire (LLP 1121 stage 11 D, `tamber/1.14`; theme2's
//! base-row shape): `describe`'s `base` and `targets`, `resolve` and `explain`
//! of a base name, and `explain`'s `bases` for a token whose values reach one.
//! Every cell is read from the kernel's scale table (`style::scale`), never a
//! copy; an app's own line for a base name is its `override`.

use super::*;
use exact_kernel::style::scale::{self, ScaleSource, ScaleValue};
use serde_json::{json, Value};

/// The most names one `resolve` answers (`tokens`, *1.17*): the clone's
/// design has 267, the importer's stress design 3,000.
const RESOLVE_BATCH: usize = 4096;
/// And the most bytes its answers take (the clone's 267 take 98 KB).
const RESOLVE_BYTES: usize = 16 << 20;

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

/// A length over unset base names (an alias of one, or an expression,
/// stage 12 B2) in each scale column, as the base rows write a column's
/// value (*1.15*, stage 12 D1): each folded by the kernel's own
/// per-column evaluator, never a copy; a column it doesn't fold in is left
/// out. None when the text reads no base name.
fn columns_of(text: &str) -> Option<Value> {
    if scale::named_in(text).is_empty() {
        return None;
    }
    // (a bare alias, `space.md`, folded as the one-term expression it is)
    let text = &match text.contains('(') {
        true => text.to_string(),
        false => format!("calc({text})"),
    };
    let columns: serde_json::Map<String, Value> = scale::COLUMNS
        .iter()
        .filter_map(|(key, column)| {
            let n = scale::evaluate(text, *column)?;
            // The evaluator's own f32, shortest (Rust's Display), a zero
            // never negative (after the terminal's division too).
            let zero = |n: f32| if n == 0.0 { 0.0 } else { n };
            // (the terminal's in cells before its layout rounds them: the
            // evaluator reads a cell, `1ch`, as its column's px, so a
            // column-width cell here, as `Env::cell_spaces` then rounds on
            // the axis it spaces)
            let v = match *key {
                "terminal" => match zero(n / exact_kernel::style::cells::COLUMN) {
                    0.0 => "0".to_string(),
                    c => format!("{c}cell"),
                },
                _ => format!("{}px", zero(n)),
            };
            Some((key.to_string(), json!({ "value": v })))
        })
        .collect();
    (!columns.is_empty()).then_some(Value::Object(columns))
}

/// The scale columns a shadow is answered in: every one but the
/// terminal's, which draws no shadow (theme2's 20:50 Q1, the one exception
/// to D1's "left out only where it doesn't fold").
fn shadow_columns() -> impl Iterator<Item = (&'static str, exact_kernel::style::metrics::Column)> {
    scale::COLUMNS
        .into_iter()
        .filter(|(key, _)| *key != "terminal")
}

/// A shadow's text in `column` as that column's host reads it: the
/// kernel's own fold (`scale::fold_lengths`), each base name read from its
/// table. (No zero it writes is negative: the kernel's `calc()` folds a
/// zero to +0, and a px length's text, `0px`, carries no sign.)
fn shadow_text(text: &str, column: exact_kernel::style::metrics::Column) -> Option<String> {
    scale::fold_lengths(text, |b| scale::of(b, column).map(scale::css))
}

/// A shadow token's value over unset base names in each column but the
/// terminal's, D1's form (*1.16*): `{value}`, the whole shadow's
/// text as that column's host reads it ([`shadow_text`]); a column it
/// doesn't fold in is left out. None for a shadow with no length over an
/// unset base (px only, or over a base the app sets, folded to px).
fn shadow_columns_of(s: &exact_tamber::Shadows) -> Option<Value> {
    use exact_tamber::ShadowLength;
    let over = |one: &exact_tamber::Shadow| {
        one.lengths
            .iter()
            .any(|l| matches!(l, ShadowLength::Base(_)))
    };
    if !s.list.iter().any(over) {
        return None;
    }
    let text = s.css();
    let columns: serde_json::Map<String, Value> = shadow_columns()
        .filter_map(|(key, column)| {
            let v = shadow_text(&text, column)?;
            Some((key.to_string(), json!({ "value": v })))
        })
        .collect();
    (!columns.is_empty()).then_some(Value::Object(columns))
}

/// One shadow of a token's, over unset base names, in each column but the
/// terminal's (*1.16*): `{value, lengths}`, its own text as that
/// column's host reads it and each of its lengths' px there, a number in
/// `lengths`' order (the kernel's fold of each, as `fold_lengths` folds
/// it; a zero never negative). None for one with no such length.
fn one_shadow_columns(text: bool, one: &exact_tamber::Shadow) -> Option<Value> {
    use exact_tamber::ShadowLength;
    if !one
        .lengths
        .iter()
        .any(|l| matches!(l, ShadowLength::Base(_)))
    {
        return None;
    }
    let css = exact_tamber::Shadows {
        text,
        list: vec![one.clone()],
    }
    .css();
    let zero = |n: f64| if n == 0.0 { 0.0 } else { n };
    let columns: serde_json::Map<String, Value> = shadow_columns()
        .filter_map(|(key, column)| {
            let v = shadow_text(&css, column)?;
            let lengths = one
                .lengths
                .iter()
                .map(|l| match l {
                    ShadowLength::Px(n) => Some(zero(*n)),
                    // As the fold writes it: the kernel's f32, shortest.
                    ShadowLength::Base(r) => {
                        let px = scale::fold(&format!(
                            "calc({})",
                            scale::substitute(&r.css(), column)?
                        ))?;
                        format!("{px}").parse::<f64>().ok().map(zero)
                    }
                })
                .collect::<Option<Vec<f64>>>()?;
            Some((key.to_string(), json!({ "value": v, "lengths": lengths })))
        })
        .collect();
    (!columns.is_empty()).then_some(Value::Object(columns))
}

/// The `when` line of `t` a tuple selects, the one naming the most axes
/// among those that hold (the checker's rule for a length, whose light
/// and dark sides are one); None for its default.
fn branch_of(t: &exact_tamber::Token, axes: &[exact_tamber::Axis], tuple: usize) -> Option<usize> {
    let mut rest = tuple;
    let mut chosen = vec![""; axes.len()];
    for (i, a) in axes.iter().enumerate().rev() {
        chosen[i] = &a.values[rest % a.values.len()];
        rest /= a.values.len();
    }
    let holds = |(axis, v): &(String, String)| {
        axes.iter()
            .position(|a| &a.name.value == axis)
            .is_some_and(|i| chosen[i] == v)
    };
    let holding = (0..t.whens.len()).filter(|&i| t.whens[i].conds.iter().all(holds));
    let most = holding.clone().map(|i| t.whens[i].conds.len()).max()?;
    holding
        .into_iter()
        .find(|&i| t.whens[i].conds.len() == most)
}

/// A cell's source as the shape writes it.
fn source(s: ScaleSource) -> Value {
    match s {
        ScaleSource::Metric(m) => json!({"kind": "metric", "ref": m}),
        ScaleSource::Stretch { metric, why } => {
            json!({"kind": "stretch", "ref": metric, "why": why})
        }
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
/// names, and those of the tokens it names, through any chain: every
/// branch, not per tuple (as wide as `names`).
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

/// The base names `name` reaches in each tuple (stage 13 D4, *1.16*), as
/// the checker folded that tuple (`Checked::by_tuple`): the unset ones its
/// value there reads bare, the set ones it names, and those of the tokens
/// it names, in the same tuple, through any chain; a line no tuple selects
/// in none. One `{modes, light, dark}` a tuple, in `resolve`'s order: a
/// base name is a length's, whose light and dark sides are one (a `when`
/// under `scheme=dark` is a colour's alone, `tamber-under-kind`), so the
/// sides are alike.
fn bases_by_tuple(folding: &exact_tamber::Folding, name: &str) -> Vec<Value> {
    let tokens = &folding.tokens;
    let start = tokens.iter().position(|t| t.name == name);
    let reach = |tuple: usize| {
        let mut out: Vec<String> = Vec::new();
        let mut seen = vec![false; tokens.len()];
        let mut stack: Vec<usize> = start.into_iter().collect();
        while let Some(i) = stack.pop() {
            if std::mem::replace(&mut seen[i], true) {
                continue;
            }
            let by = &tokens[i].by_tuple;
            let Some((named, bases)) = by.at.get(tuple).and_then(|&r| by.reads.get(r as usize))
            else {
                continue;
            };
            let set = named
                .iter()
                .map(|&j| &tokens[j as usize].name)
                .filter(|m| scale::is_base(m));
            for b in bases.iter().chain(set) {
                if !out.contains(b) {
                    out.push(b.clone());
                }
            }
            // (named tokens next, in the order written)
            stack.extend(named.iter().rev().map(|&j| j as usize));
        }
        out
    };
    (0..folding.tuples)
        .map(|tuple| {
            let bases = reach(tuple);
            json!({"modes": modes_of(&folding.design.axes, tuple), "light": bases, "dark": bases})
        })
        .collect()
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
        if !all.is_empty() || app.library.iter().any(|(_, _, d)| d.library) {
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
                    entry["draws"] = crate::tamber_explain::draws(&self.dir, &app, icon);
                }
            }
        }
        // The base scale (*1.14*): every base name's row, this design's
        // overrides, and the targets the app ships with their columns.
        let (base, targets) = base_rows(self, request, &design)?;
        // A package token's rows name its package and file; the package designs listed (*1.14*, D11).
        let text = snap
            .iter()
            .find(|(f, _, _)| *f == file)
            .map_or("", |(_, t, _)| t.as_str());
        // Its icons' too (stage 13, *1.16*).
        for rows in ["tokens", "icons"] {
            crate::tamber_packages::mark(
                ir[rows].as_array_mut().map_or(&mut [][..], |r| &mut r[..]),
                &app,
                text,
                &file,
            );
        }
        let mut out = json!({"ir": ir, "files": files_json(&snap), "meta": {"dials": dials, "tokens": tokens}, "pairs": pairs, "warnings": warnings, "base": base, "targets": targets});
        crate::tamber_packages::packaged(&app, &mut out, false);
        Ok(out)
    }

    /// A param's additions (stage 12 B2-levels, *1.15*) to `out`, from the
    /// one walker over `file`'s `text`; nothing for a token that is no
    /// param's.
    fn levelled(
        &self,
        app: &contract::tamber_params::AppParams,
        file: &str,
        text: Option<&str>,
        name: &str,
        out: &mut Value,
    ) {
        if let Some(p) = app.tokens.iter().find(|p| p.token == name) {
            let designs = contract::tamber_levels::Designs::read(file, text, app, &self.vocab);
            designs.annotate(p, out);
        }
    }

    /// A token's stored value per tuple and scheme; a host value (a role, a
    /// platform colour) is a residual, labelled so, with its fallback.
    /// `tokens` (*1.17*) asks for several from one snapshot and one fold:
    /// `{files, answers}`, each answer the one `token` would give without
    /// its `files`, a name one would refuse its refusal (`{error}`), in
    /// place.
    pub(super) fn resolve(&self, request: &Value) -> Result<Value, Value> {
        let Some(asked) = request.get("tokens") else {
            let (snap, folding, name) = self.lookup(request)?;
            return self.resolve_in(request, &required(request, "token")?, &snap, &folding, name);
        };
        // Refused whole, before any fold.
        if request.get("token").is_some() {
            return Err(json!({"message": "a resolve names `token` or `tokens`, not both"}));
        }
        let names: Vec<&str> = asked
            .as_array()
            .and_then(|all| all.iter().map(Value::as_str).collect())
            .ok_or_else(|| json!({"message": "`tokens` is a list of names"}))?;
        if names.len() > RESOLVE_BATCH {
            return Err(
                json!({"message": format!("tamber-resolve-batch: one resolve names at most {RESOLVE_BATCH} tokens, not {}", names.len()), "id": "tamber-resolve-batch"}),
            );
        }
        let app = self.app_params();
        let (snap, folding, _) = self.design_with(request, &app)?;
        // Each answer named as one alone is (`present`, per answer, so one
        // that can't be shown leaves the others shown); their bytes bounded
        // as they are made, whatever the names.
        let mut answers = Vec::with_capacity(names.len());
        let mut bytes = 0;
        for given in names {
            let answer = match Self::token_of(&app, given)
                .and_then(|name| self.resolve_in(request, given, &snap, &folding, name))
            {
                Ok(mut answer) => {
                    answer.as_object_mut().map(|o| o.remove("files"));
                    self.present(answer)
                }
                Err(error) => json!({ "error": self.present(error) }),
            };
            bytes += answer.to_string().len();
            if bytes > RESOLVE_BYTES {
                return Err(
                    json!({"message": format!("tamber-resolve-batch: one resolve answers at most {} MiB; ask for fewer tokens at once", RESOLVE_BYTES >> 20), "id": "tamber-resolve-batch"}),
                );
            }
            answers.push(answer);
        }
        // (Its `files` named as one alone's are, once.)
        Ok(json!({"files": self.present(files_json(&snap)), "answers": answers}))
    }

    /// [`Self::resolve`] of the name `given`, looked up as `name`, in
    /// `snap` folded as `folding`.
    fn resolve_in(
        &self,
        request: &Value,
        given: &str,
        snap: &Snapshot,
        folding: &exact_tamber::Folding,
        name: String,
    ) -> Result<Value, Value> {
        // A base name (*1.14*), not a param named like one: unset, its row
        // and no fold (each column's value); set, its token's tuples and row.
        let base = (given == name)
            .then(|| Ok::<_, Value>(row(&name, &file(self, request)?, &folding.design)))
            .transpose()?
            .flatten();
        if let Some(base) = &base {
            if base["override"].is_null() {
                return Ok(json!({"name": name, "base": base, "files": files_json(snap)}));
            }
        }
        let file = optional(request, "file")?.unwrap_or_else(|| self.design.clone());
        let text = snap
            .iter()
            .find(|(f, ..)| *f == file)
            .map(|(_, t, _)| t.to_string());
        // A param's (stage 12 B2-levels, *1.15*): each tuple's winning level
        // and the name there, and its levels, from the one walker.
        let app = self.app_params();
        let levelled = |out: &mut Value| self.levelled(&app, &file, text.as_deref(), &name, out);
        // An icon (D10): its sources, the same in every tuple.
        if let Some(icon) = folding.icons.iter().find(|i| i.name == name) {
            let tuples: Vec<Value> = (0..folding.tuples)
                .map(|t| json!({"modes": modes_of(&folding.design.axes, t), "kind": "icon", "sf": icon.sf, "material": icon.material, "svg": icon.svg}))
                .collect();
            let mut out = json!({"token": name, "tuples": tuples, "files": files_json(snap)});
            levelled(&mut out);
            // A package's icon names its package and file, as a token does (stage 13, *1.16*).
            crate::tamber_packages::fields(&app, snap, &file, &name, &mut out, false);
            return Ok(out);
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
                            // An expression over base names is each
                            // host's, as a bare one is (stage 12 B2).
                            exact_tamber::Length::Named(_) | exact_tamber::Length::Base(_) => {
                                ("named", Value::Null)
                            }
                        };
                        let mut out = json!({"modes": modes, "value": v.css(), "kind": "length", "unit": unit, "amount": amount});
                        // Each host's, by column, for one over base names (*1.15*).
                        if let Some(columns) = columns_of(&v.css()) {
                            out["columns"] = columns;
                        }
                        return out;
                    }
                    exact_tamber::Value::Number(n) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "number", "amount": n});
                    }
                    // Each shadow: its lengths in px (null for one over base
                    // names, whose text `length_texts` gives with every
                    // other's; stage 12 B3, *1.15*) and its colour's CSS
                    // (null for a text shadow's currentcolor).
                    exact_tamber::Value::Shadow(s) => {
                        use exact_tamber::ShadowLength;
                        let shadows: Vec<Value> = s
                            .list
                            .iter()
                            .map(|one| {
                                let lengths: Vec<Value> = one
                                    .lengths
                                    .iter()
                                    .map(|l| match l {
                                        // A zero never negative (`calc(space.md * -0)`), *1.16*.
                                        ShadowLength::Px(n) => json!(if *n == 0.0 { 0.0 } else { *n }),
                                        ShadowLength::Base(_) => Value::Null,
                                    })
                                    .collect();
                                let mut out = json!({"inset": one.inset, "lengths": lengths, "color": one.color.as_ref().map(|c| c.css())});
                                if one.lengths.iter().any(|l| matches!(l, ShadowLength::Base(_))) {
                                    let texts: Vec<String> = one.lengths.iter().map(ShadowLength::css).collect();
                                    out["length_texts"] = json!(texts);
                                }
                                // Its own text and numbers by column (*1.16*).
                                if let Some(columns) = one_shadow_columns(s.text, one) {
                                    out["columns"] = columns;
                                }
                                out
                            })
                            .collect();
                        let mut out = json!({"modes": modes, "value": v.css(), "kind": if s.text { "text-shadow" } else { "box-shadow" }, "shadows": shadows});
                        // Each host's text, by column but the terminal's (*1.16*).
                        if let Some(columns) = shadow_columns_of(s) {
                            out["columns"] = columns;
                        }
                        return out;
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
        let mut out = json!({"token": name, "tuples": tuples, "files": files_json(snap)});
        levelled(&mut out);
        if let Some(base) = base {
            out["base"] = base;
        }
        crate::tamber_packages::fields(&app, snap, &file, &name, &mut out, false);
        Ok(out)
    }

    /// A token's chain: its default and `when` values, the dials it depends
    /// on, and the tokens those values name, as the checker resolved them.
    pub(super) fn explain(&self, request: &Value) -> Result<Value, Value> {
        // Each token's per-tuple names recorded, for `bases_by_tuple`.
        let (snap, folding, name) = exact_tamber::traced(|| self.lookup(request))?;
        // An icon, an icon param's too (D10, *1.12*).
        let app = self.app_params();
        // The design asked about (`file`), whose lines set the app's values.
        let design = optional(request, "file")?.unwrap_or_else(|| self.design.clone());
        let text = snap
            .iter()
            .find(|(f, ..)| *f == design)
            .map(|(_, t, _)| t.to_string());
        if let Some(mut icon) =
            crate::tamber_explain::icon(&self.dir, &design, &app, &snap, &folding, &name)
        {
            self.levelled(&app, &design, text.as_deref(), &name, &mut icon);
            // A package's icon names its package and file, as a token does (stage 13, *1.16*).
            crate::tamber_packages::fields(&app, &snap, &design, &name, &mut icon, true);
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
                out["bases_by_tuple"] = json!(bases_by_tuple(&folding, &name));
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
        // Each line's value over base names in each column (*1.15*; a shadow's *1.16*), as the
        // first tuple that line chooses folds it (a line no tuple chooses,
        // none): a length's, and a shadow's but for the terminal.
        let axes = &folding.design.axes;
        let columns = |line: Option<usize>| {
            (0..folding.tuples)
                .find(|&t| branch_of(declared, axes, t) == line)
                .and_then(|t| token.values.get(t))
                .and_then(|v| match v {
                    exact_tamber::Value::Length(_) => columns_of(&v.css()),
                    exact_tamber::Value::Shadow(s) => shadow_columns_of(s),
                    _ => None,
                })
        };
        let whens: Vec<Value> = declared
            .whens
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let mut out = json!({"when": w.conds.iter().map(|(a, v)| json!({"axis": a, "value": v})).collect::<Vec<_>>(), "value": w.value.value});
                if let Some(c) = columns(Some(i)) {
                    out["columns"] = c;
                }
                out
            })
            .collect();
        let mut out = json!({"token": name, "value": declared.value.value, "whens": whens, "dials": token.dials, "names": token.names, "files": files_json(&snap)});
        if let Some(c) = columns(None) {
            out["columns"] = c;
        }
        // A param (D15): where it is declared, its default as written, and
        // the design's line that sets the app's value, *1.12*.
        if let Some(p) = app.tokens.iter().find(|p| p.token == name) {
            out["param"] = crate::tamber_explain::param(&self.dir, &design, p, &app, &snap);
        }
        // Its levels (stage 12 B2-levels, *1.15*).
        self.levelled(&app, &design, text.as_deref(), &name, &mut out);
        crate::tamber_packages::fields(&app, &snap, &design, &name, &mut out, true);
        Ok(with(out))
    }
}
