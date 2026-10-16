//! DTCG export (LLP 1121 D14, stage 5): a design as DTCG Format 2025.10
//! with the Resolver Module 2025.10, and the round trip of that export.
//!
//! The document has one modifier, `mode`, whose contexts are the design's
//! full tuples (every app-axis tuple, then `light` and `dark`), each a
//! complete set of the colour tokens, checked against the folded table
//! before it is written. The resolver's root takes no `$extensions`, so the
//! export's header (what it is, its conversions, the axes, each context's
//! tuple, the dials) is the root `$extensions["exact"]` of the one token
//! document of a set `exact`, first in the resolution order.
//!
//! Re-importing the export ([`reimport`]) compares each context's incoming
//! `$value` with the baseline, what the last export wrote (kept in the app's
//! `.exact/`, [`BASELINE`]): an unchanged value keeps its source, an
//! expression or a role included, and a changed one is proposed as a
//! `set-token` for its tuple.

use crate::import::{counted, dtcg::colour, Import, Unmapped};
use crate::syntax::Token;
use crate::{fallback, Diagnostic, Folded, Folding, Half, Host, Op, Vocab};
use exact_color::{Parsed, Space, Wide};
use serde_json::{json, Map, Value};

/// The header's `export`: what the document is, for the importer.
pub const MARK: &str = "tamber-dtcg/1";

/// The baseline's file in the app's `.exact/`.
pub const BASELINE: &str = "tamber-dtcg.json";

/// What the export says it converted, in its header and its description.
const CONVERSIONS: &[&str] = &[
    "a colour token: `$type: color`, its folded value per context as a 2025.10 colour object, `hex` its 8-bit sRGB fallback",
    "an expression (a reference, a function, `light-dark()`): its folded value, its source in `$extensions.exact.source`",
    "a value that is a role: the role's fallback for the context, with `$extensions.exact.role`",
    "a platform colour: its fallback for the context, with `$extensions.exact.platform`",
    "a role override (`AccentColor = …`): a token named for the role's row, with `$extensions.exact.overrides`",
    "a colour outside DTCG's spaces or ranges: oklab; one past oklab's lightness (HDR) clipped, with its CSS in `$extensions.exact.lossy`",
    "dials: this header's `dials`, each its source and folded value",
    "icons: not tokens, not exported",
    "lengths, numbers, `rem`, `em` and text styles: not yet (LLP 1121 stage 6)",
];

/// One context: a full tuple and a side of the scheme.
pub(crate) struct Context {
    /// Its name in the modifier: `light`, or `axis=value, …, dark`.
    pub name: String,
    pub tuple: usize,
    pub dark: bool,
    /// Every axis and its value in the tuple.
    pub modes: Vec<(String, String)>,
}

/// Each context, a tuple's light side then its dark side, in tuple order.
pub(crate) fn contexts(f: &Folding) -> Vec<Context> {
    let mut out = Vec::new();
    for tuple in 0..f.tuples {
        let modes = modes(f, tuple);
        for dark in [false, true] {
            let side = if dark { "dark" } else { "light" };
            let name = modes
                .iter()
                .map(|(a, v)| format!("{a}={v}"))
                .chain([side.to_string()])
                .collect::<Vec<_>>()
                .join(", ");
            out.push(Context {
                name,
                tuple,
                dark,
                modes: modes.clone(),
            });
        }
    }
    out
}

/// Tuple `tuple`'s value of every axis, the first axis slowest.
fn modes(f: &Folding, mut tuple: usize) -> Vec<(String, String)> {
    let axes = &f.design.axes;
    let mut out = vec![(String::new(), String::new()); axes.len()];
    for (i, a) in axes.iter().enumerate().rev() {
        let n = a.values.len().max(1);
        out[i] = (a.name.value.clone(), a.values[tuple % n].clone());
        tuple /= n;
    }
    out
}

/// The source text a side of a tuple takes from token `t`: the `when` the
/// checker selects (the one naming the most axes among those that hold),
/// and for the light side of one naming an axis `under scheme=dark`, the
/// line that tuple's light side comes from.
fn source<'a>(f: &Folding, t: &'a Token, modes: &[(String, String)], dark: bool) -> &'a str {
    let under = |axis: &str| {
        f.design
            .axes
            .iter()
            .any(|a| a.name.value == axis && a.under_dark)
    };
    let select = |base: bool| {
        let holding = (0..t.whens.len()).filter(|&i| {
            let w = &t.whens[i];
            !(base && w.conds.iter().any(|(a, _)| under(a)))
                && w.conds.iter().all(|c| modes.contains(c))
        });
        let most = holding.clone().map(|i| t.whens[i].conds.len()).max()?;
        holding
            .into_iter()
            .find(|&i| t.whens[i].conds.len() == most)
    };
    let mut chosen = select(false);
    if !dark && chosen.is_some_and(|w| t.whens[w].conds.iter().any(|(a, _)| under(a))) {
        chosen = select(true);
    }
    chosen.map_or(&t.value.value, |w| &t.whens[w].value.value)
}

/// A side of a folded value as a colour, and what the export notes of it:
/// a role or platform colour at its fallback.
fn side(
    value: &Folded,
    dark: bool,
    vocab: &Vocab,
) -> Option<(Half, Option<(&'static str, String)>)> {
    let pick = |pair: [String; 2]| match exact_color::parse(&pair[usize::from(dark)])? {
        Parsed::Legacy(c) => Some(Half::legacy(c)),
        Parsed::Wide(w) => Some(Half::Wide(w)),
        _ => None,
    };
    match value {
        Folded::One(_) | Folded::Pair(..) => value.side(dark).ok().map(|h| (h, None)),
        Folded::Host(Host::Role(row)) => {
            Some((pick(fallback(value, vocab)?)?, Some(("role", row.clone()))))
        }
        Folded::Host(Host::Platform(name, pair)) => {
            Some((pick(pair.clone())?, Some(("platform", name.clone()))))
        }
    }
}

/// Within a space's 2025.10 ranges: RGB and XYZ components 0–1, Lab's and
/// LCh's lightness 0–100, Oklab's 0–1, a chroma at least 0.
fn in_range(w: &Wide) -> bool {
    let unit = |c: f64| (0.0..=1.0).contains(&c);
    match w.space {
        Space::Srgb
        | Space::SrgbLinear
        | Space::DisplayP3
        | Space::A98Rgb
        | Space::ProphotoRgb
        | Space::Rec2020
        | Space::XyzD50
        | Space::XyzD65 => w.c.iter().all(|&c| unit(c)),
        Space::Lab => (0.0..=100.0).contains(&w.c[0]),
        Space::Lch => (0.0..=100.0).contains(&w.c[0]) && w.c[1] >= 0.0,
        Space::Oklab => unit(w.c[0]),
        Space::Oklch => unit(w.c[0]) && w.c[1] >= 0.0,
        Space::DisplayP3Linear | Space::Rec2100Pq | Space::Rec2100Hlg | Space::Rec2100Linear => {
            false
        }
    }
}

/// A colour as 2025.10's object, and whether it lost anything: its own
/// space where DTCG has it and its components fit, else Oklab, its
/// lightness clipped past 1 (an HDR colour).
fn colour_json(h: Half) -> (Value, bool) {
    let hex = h.clip()[..7].to_string();
    let mut w = h.wide();
    let mut lossy = false;
    if !in_range(&w) {
        let [l, a, b] = exact_color::linear_to_oklab(w.linear_srgb());
        lossy = l > 1.0;
        w = Wide {
            space: Space::Oklab,
            c: [l.clamp(0.0, 1.0), a, b],
            alpha: w.alpha,
        };
    }
    if matches!(w.space, Space::Lch | Space::Oklch) {
        w.c[2] = w.c[2].rem_euclid(360.0);
    }
    let mut obj = Map::new();
    obj.insert("colorSpace".into(), json!(w.space.name()));
    obj.insert("components".into(), json!(w.c));
    if w.alpha < 1.0 {
        obj.insert("alpha".into(), json!(w.alpha));
    }
    obj.insert("hex".into(), json!(hex));
    (Value::Object(obj), lossy)
}

/// Whether a colour object reads back as `h` (DTCG's numbers are f64, so to
/// a rounding).
fn reads_as(value: &Value, h: Half) -> bool {
    let Some(read) = colour(value).and_then(|c| exact_color::parse(&c)) else {
        return false;
    };
    let read = match read {
        Parsed::Legacy(c) => Half::legacy(c).wide(),
        Parsed::Wide(w) => w,
        _ => return false,
    };
    let (a, b) = (read.linear_srgb(), h.wide().linear_srgb());
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-6)
        && (read.alpha - h.wide().alpha).abs() < 1e-3
}

/// Put `token` at `name` in a token tree: dotted segments as groups, and a
/// name that is also a group (`surface` beside `surface.card`) at `$root`.
fn insert(tree: &mut Map<String, Value>, name: &str, group: bool, token: Value) {
    let segs: Vec<&str> = name.split('.').collect();
    let mut at = tree;
    for s in &segs[..segs.len() - 1] {
        at = at
            .entry((*s).to_string())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("a group");
    }
    let last = segs[segs.len() - 1].to_string();
    if group {
        at.entry(last)
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("a group")
            .insert("$root".into(), token);
    } else {
        at.insert(last, token);
    }
}

/// The export of a checked design: DTCG 2025.10 with the Resolver Module,
/// as pretty JSON. Refused where a context would be incomplete or a value
/// would not read back as the folded table has it.
pub fn dtcg(f: &Folding, vocab: &Vocab) -> Result<String, Vec<Diagnostic>> {
    let contexts = contexts(f);
    let names: Vec<&str> = f.tokens.iter().map(|t| t.name.as_str()).collect();
    let group = |name: &str| names.iter().any(|n| n.starts_with(&format!("{name}.")));
    let mut errors = Vec::new();
    let mut map = Map::new();
    for c in &contexts {
        let mut tree = Map::new();
        let mut count = 0;
        for (checked, t) in f.tokens.iter().zip(&f.design.tokens) {
            let Some(folded) = checked.values.get(c.tuple) else {
                continue;
            };
            let Some((half, note)) = side(folded, c.dark, vocab) else {
                errors.push(Diagnostic::new(
                    "tamber-export",
                    format!("`{}` has no colour in `{}`", checked.name, c.name),
                    t.name.line,
                    t.name.col,
                ));
                continue;
            };
            let (value, lossy) = colour_json(half);
            if !lossy && !reads_as(&value, half) {
                errors.push(Diagnostic::new(
                    "tamber-export",
                    format!(
                        "`{}` in `{}` would not read back as `{}`",
                        checked.name,
                        c.name,
                        half.css()
                    ),
                    t.name.line,
                    t.name.col,
                ));
            }
            let mut exact = Map::new();
            let text = source(f, t, &c.modes, c.dark).trim();
            if exact_color::parse(text).is_none() {
                exact.insert("source".into(), json!(text));
            }
            if let Some((key, what)) = note {
                exact.insert(key.into(), json!(what));
            }
            if let Some(role) = &checked.role {
                exact.insert("overrides".into(), json!(role));
            }
            if lossy {
                exact.insert("lossy".into(), json!(half.css()));
            }
            let mut token = Map::new();
            token.insert("$type".into(), json!("color"));
            token.insert("$value".into(), value);
            if !exact.is_empty() {
                token.insert("$extensions".into(), json!({ "exact": exact }));
            }
            insert(
                &mut tree,
                &checked.name,
                group(&checked.name),
                Value::Object(token),
            );
            count += 1;
        }
        // Complete: every token in every context.
        if count != f.tokens.len() {
            errors.push(Diagnostic::new(
                "tamber-export",
                format!(
                    "`{}` has {count} of the design's {} tokens",
                    c.name,
                    f.tokens.len()
                ),
                1,
                1,
            ));
        }
        map.insert(c.name.clone(), json!([Value::Object(tree)]));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let digest = serde_json::from_str::<Value>(&f.ir())
        .ok()
        .and_then(|ir| ir["digest"].as_str().map(str::to_string))
        .unwrap_or_default();
    let header = json!({
        "export": MARK,
        "design": f.design.name,
        "digest": digest,
        "conversions": CONVERSIONS,
        "axes": f.design.axes.iter().map(|a| json!({
            "name": a.name.value,
            "values": a.values,
            "under": a.under_dark.then_some("dark"),
        })).collect::<Vec<_>>(),
        "contexts": contexts.iter().map(|c| (c.name.clone(), json!({
            "modes": c.modes.iter().map(|(a, v)| (a.clone(), json!(v))).collect::<Map<_, _>>(),
            "scheme": if c.dark { "dark" } else { "light" },
        }))).collect::<Map<_, _>>(),
        "dials": f.design.dials.iter().zip(&f.dials).map(|(d, folded)| json!({
            "name": d.name.value,
            "kind": d.kind.name(),
            "source": d.value.value,
            "value": folded,
            "range": d.range.as_ref().map(|(lo, hi)| [lo, hi]),
        })).collect::<Vec<_>>(),
    });
    let description = format!(
        "The Tamber design `{}` (LLP 1121 D14): one modifier, `mode`, whose contexts are the design's full tuples, each a complete set. Conversions: {}.",
        f.design.name,
        CONVERSIONS.join("; ")
    );
    let doc = json!({
        "name": f.design.name,
        "version": "2025.10",
        "description": description,
        "sets": { "exact": { "sources": [ { "$extensions": { "exact": header } } ] } },
        "modifiers": { "mode": { "contexts": map, "default": contexts[0].name } },
        "resolutionOrder": [ { "$ref": "#/sets/exact" }, { "$ref": "#/modifiers/mode" } ],
    });
    Ok(serde_json::to_string_pretty(&doc).expect("JSON") + "\n")
}

/// The export's header, when `doc` is a Tamber export.
fn header(doc: &Value) -> Option<&Value> {
    let h = &doc["sets"]["exact"]["sources"][0]["$extensions"]["exact"];
    (h["export"] == MARK).then_some(h)
}

/// Whether `text` is a Tamber DTCG export, which [`reimport`] reads.
pub fn is_own(text: &str) -> bool {
    serde_json::from_str::<Value>(text).is_ok_and(|doc| header(&doc).is_some())
}

/// A context's tokens: each name (a group's `$root` the group's) and its
/// token object.
fn flatten(tree: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
    let Some(obj) = tree.as_object() else { return };
    if obj.contains_key("$value") {
        out.push((prefix.to_string(), tree.clone()));
        return;
    }
    for (k, v) in obj {
        let name = match (k.as_str(), prefix) {
            ("$root", _) => prefix.to_string(),
            (k, _) if k.starts_with('$') => continue,
            (k, "") => k.to_string(),
            (k, p) => format!("{p}.{k}"),
        };
        flatten(v, &name, out);
    }
}

/// A context's tokens by name, from a document's `mode` modifier.
fn context_tokens(doc: &Value, context: &str) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for src in doc["modifiers"]["mode"]["contexts"][context]
        .as_array()
        .into_iter()
        .flatten()
    {
        flatten(src, "", &mut out);
    }
    out
}

/// A colour object as Tamber writes it: its `hex` where that is exactly
/// what its sRGB components say (an 8-bit colour, as a design writes one),
/// else what the DTCG importer reads.
fn written_colour(value: &Value) -> Option<String> {
    let hex = value["hex"].as_str().filter(|h| h.len() == 7)?;
    let parts = value["components"].as_array()?;
    let exact = value["colorSpace"] == "srgb"
        && parts.len() == 3
        && parts.iter().enumerate().all(|(i, c)| {
            let byte = u8::from_str_radix(&hex[1 + 2 * i..3 + 2 * i], 16).ok();
            matches!((c.as_f64(), byte), (Some(c), Some(b)) if (c * 255.0 - f64::from(b)).abs() < 1e-6)
        });
    if !exact {
        return colour(value);
    }
    match value["alpha"].as_f64() {
        Some(a) if a < 1.0 => Some(format!("{hex}{:02x}", (a * 255.0).round() as u8)),
        _ => Some(hex.to_ascii_lowercase()),
    }
    .or_else(|| colour(value))
}

/// Two JSON values the same, numbers to a rounding (a tool may reprint
/// them).
fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0),
            _ => x == y,
        },
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| close(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| close(v, w)))
        }
        _ => a == b,
    }
}

/// A tuple: every axis and its value.
type Modes = Vec<(String, String)>;

/// Re-import a Tamber export (`incoming`) against `baseline`, what the last
/// export wrote (or a fresh export of the design as it is): each token whose
/// `$value` is unchanged in every context proposes nothing, its source kept;
/// a tuple whose light or dark value changed proposes one `set-token`, its
/// `when` naming every axis (the default line when the design has none) and
/// its value the tuple's two incoming sides. A token the baseline lacks, or
/// a value this importer does not read, is reported.
pub fn reimport(
    incoming: &str,
    baseline: &str,
    design: &crate::Design,
) -> Result<Import, Vec<Diagnostic>> {
    let refuse = |m: String| vec![Diagnostic::new("tamber-export-import", m, 1, 1)];
    let doc: Value =
        serde_json::from_str(incoming).map_err(|e| refuse(format!("not JSON: {e}")))?;
    let base: Value = serde_json::from_str(baseline)
        .map_err(|e| refuse(format!("the baseline: not JSON: {e}")))?;
    let head = header(&base).ok_or_else(|| refuse("the baseline is no Tamber export".into()))?;
    header(&doc).ok_or_else(|| refuse("not a Tamber export".into()))?;
    let contexts: Vec<(&String, &Value)> = head["contexts"]
        .as_object()
        .ok_or_else(|| refuse("the baseline's header names no contexts".into()))?
        .iter()
        .collect();
    let axes = head["axes"].as_array().map_or(0, Vec::len);
    let first = contexts.first().map(|(n, _)| n.as_str()).unwrap_or("light");
    let baseline_tokens = context_tokens(&base, first);
    let mut unmapped = Vec::new();
    let mut ops = Vec::new();
    let mut mapped = 0;
    // The tuples, each its light and dark context, in the baseline's order.
    let mut tuples: Vec<(Modes, [&str; 2])> = Vec::new();
    for (name, c) in &contexts {
        let modes: Vec<(String, String)> = c["modes"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(a, v)| (a.clone(), v.as_str().unwrap_or_default().to_string()))
            .collect();
        let dark = c["scheme"] == "dark";
        match tuples.iter_mut().find(|(m, _)| *m == modes) {
            Some((_, sides)) => sides[usize::from(dark)] = name,
            None => {
                let mut sides = [name.as_str(); 2];
                sides[usize::from(dark)] = name;
                tuples.push((modes, sides));
            }
        }
    }
    let lookup = |doc: &Value, context: &str, name: &str| {
        context_tokens(doc, context)
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t["$value"].clone())
    };
    let incoming_names: Vec<String> = context_tokens(&doc, first)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    for name in &incoming_names {
        if !baseline_tokens.iter().any(|(n, _)| n == name) {
            unmapped.push(Unmapped {
                block: first.to_string(),
                name: name.clone(),
                value: String::new(),
                why: "not in the export it came from: a new token is added to the design, not by re-import".into(),
                token: Some(name.clone()),
            });
        }
    }
    for (name, _) in &baseline_tokens {
        if !incoming_names.contains(name) {
            continue;
        }
        let written = design
            .tokens
            .iter()
            .find(|t| t.name.value.eq_ignore_ascii_case(name))
            .map_or(name.clone(), |t| t.name.value.clone());
        let mut token_ops = Vec::new();
        let mut unread = None;
        for (modes, sides) in &tuples {
            let changed = sides.iter().any(|s| {
                let (a, b) = (lookup(&doc, s, name), lookup(&base, s, name));
                !matches!((&a, &b), (Some(a), Some(b)) if close(a, b))
            });
            if !changed {
                continue;
            }
            let read = sides.map(|s| lookup(&doc, s, name).as_ref().and_then(written_colour));
            let [Some(light), Some(dark)] = read else {
                unread = Some(sides.join(" / "));
                break;
            };
            let value = if light == dark {
                light
            } else {
                format!("light-dark({light}, {dark})")
            };
            token_ops.push(Op::SetToken {
                name: written.clone(),
                when: if axes == 0 { Vec::new() } else { modes.clone() },
                value,
                at: None,
                comment: None,
            });
        }
        match unread {
            Some(at) => unmapped.push(Unmapped {
                block: at,
                name: name.clone(),
                value: String::new(),
                why: "a value this importer reads no colour from, or a context it lacks".into(),
                token: Some(written),
            }),
            None => {
                mapped += 1;
                ops.extend(token_ops);
            }
        }
    }
    let counted = counted(incoming_names.len(), mapped, unmapped.len())?;
    Ok(Import {
        ops,
        offers: Vec::new(),
        unmapped,
        counted,
    })
}
