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
//! `$value` with the baseline, what the export it came from wrote (kept in
//! the app's `.exact/`, [`BASELINES`]): an unchanged value keeps its source, an
//! expression or a role included, and a changed one is proposed as a
//! `set-token` for its tuple.

use crate::import::{counted, dtcg::colour, Import, Unmapped};
use crate::syntax::Token;
use crate::{fallback, Diagnostic, Folded, Folding, Half, Host, Op, Vocab};
use exact_color::{Parsed, Space, Wide};
use serde_json::{json, Map, Value};

/// The header's `export`: what the document is, for the importer.
pub const MARK: &str = "tamber-dtcg/1";

/// The directory in the app's `.exact/` that keeps each export's baseline,
/// as `<digest>.json` by the design's digest.
pub const BASELINES: &str = "tamber-dtcg";

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
    let mut w = rendered(h);
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

/// The colour a host renders for `h`: a wide one as the kernel reads its
/// CSS back, which clamps what an unclamped relative colour carries (a
/// negative chroma), as `Half::clip` does.
fn rendered(h: Half) -> Wide {
    match h {
        Half::Wide(w) => match exact_color::parse(&w.css()) {
            Some(Parsed::Wide(read)) => read,
            _ => w,
        },
        Half::Legacy(..) => h.wide(),
    }
}

/// Whether a colour object reads back as the colour a host renders for `h`
/// (DTCG's numbers are f64, so to a rounding).
fn reads_as(value: &Value, h: Half) -> bool {
    let Some(read) = colour(value).and_then(|c| exact_color::parse(&c)) else {
        return false;
    };
    let read = match read {
        Parsed::Legacy(c) => Half::legacy(c).wide(),
        Parsed::Wide(w) => w,
        _ => return false,
    };
    let want = rendered(h);
    let (a, b) = (read.linear_srgb(), want.linear_srgb());
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-6) && (read.alpha - want.alpha).abs() < 1e-3
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

/// The export's header, when `doc` carries it.
fn header(doc: &Value) -> Option<&Value> {
    let h = &doc["sets"]["exact"]["sources"][0]["$extensions"]["exact"];
    (h["export"] == MARK).then_some(h)
}

/// A context's tuple and side, read from its name (`axis=value, …, dark`),
/// for an export a tool stripped of its `$extensions`.
fn named(context: &str) -> Option<(Modes, bool)> {
    let mut parts: Vec<&str> = context.split(", ").collect();
    let dark = match parts.pop()? {
        "light" => false,
        "dark" => true,
        _ => return None,
    };
    let modes = parts
        .iter()
        .map(|p| {
            p.split_once('=')
                .map(|(a, v)| (a.to_string(), v.to_string()))
        })
        .collect::<Option<Modes>>()?;
    Some((modes, dark))
}

/// Each context of an export, its tuple and side: from the header, or from
/// the names of the `mode` modifier's contexts when a tool dropped it.
fn context_map(doc: &Value) -> Option<Vec<(String, Modes, bool)>> {
    if let Some(h) = header(doc) {
        return h["contexts"].as_object().map(|all| {
            all.iter()
                .map(|(name, c)| {
                    let modes = c["modes"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(a, v)| (a.clone(), v.as_str().unwrap_or_default().to_string()))
                        .collect();
                    (name.clone(), modes, c["scheme"] == "dark")
                })
                .collect()
        });
    }
    // Without the header, the export's shape: a set `exact` and the one
    // modifier `mode`, in that order, its contexts named for their tuples.
    let order = doc["resolutionOrder"].as_array()?;
    let refs: Vec<&str> = order.iter().filter_map(|r| r["$ref"].as_str()).collect();
    let modifiers = doc["modifiers"].as_object()?;
    if refs != ["#/sets/exact", "#/modifiers/mode"] || modifiers.len() != 1 {
        return None;
    }
    modifiers["mode"]["contexts"]
        .as_object()?
        .keys()
        .map(|name| named(name).map(|(m, d)| (name.clone(), m, d)))
        .collect()
}

/// Whether `text` is a Tamber DTCG export, which [`reimport`] reads: one
/// with its header, or a tool's copy with the export's shape.
pub fn is_own(text: &str) -> bool {
    serde_json::from_str::<Value>(text).is_ok_and(|doc| context_map(&doc).is_some())
}

/// The design digest an export names in its header.
pub fn digest(text: &str) -> Option<String> {
    let doc: Value = serde_json::from_str(text).ok()?;
    header(&doc)?["digest"].as_str().map(str::to_string)
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

/// Each context's tokens by name, from a document's `mode` modifier.
fn tokens_by_context(doc: &Value) -> std::collections::BTreeMap<String, Vec<(String, Value)>> {
    let mut out = std::collections::BTreeMap::new();
    for (context, srcs) in doc["modifiers"]["mode"]["contexts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let mut all = Vec::new();
        for src in srcs.as_array().into_iter().flatten() {
            flatten(src, "", &mut all);
        }
        out.insert(context.clone(), all);
    }
    out
}

/// A colour object as Tamber writes it: its `hex` where that, and its alpha
/// in eighths of 255, are exactly what its sRGB components say (an 8-bit
/// colour, as a design writes one), else what the DTCG importer reads.
fn written_colour(value: &Value) -> Option<String> {
    let byte = |hex: &str, i: usize| {
        hex.get(1 + 2 * i..3 + 2 * i)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
    };
    let short = (|| {
        let hex = value["hex"]
            .as_str()
            .filter(|h| h.len() == 7 && h.is_ascii() && h.starts_with('#'))?;
        let parts = value["components"].as_array().filter(|p| p.len() == 3)?;
        let exact = value["colorSpace"] == "srgb"
            && parts.iter().enumerate().all(|(i, c)| {
                matches!((c.as_f64(), byte(hex, i)), (Some(c), Some(b)) if (c * 255.0 - f64::from(b)).abs() < 1e-6)
            });
        if !exact {
            return None;
        }
        match value.get("alpha").map(Value::as_f64) {
            None => Some(hex.to_ascii_lowercase()),
            Some(Some(a)) if a >= 1.0 => Some(hex.to_ascii_lowercase()),
            Some(Some(a)) if (a * 255.0 - (a * 255.0).round()).abs() < 1e-9 && a >= 0.0 => {
                Some(format!(
                    "{}{:02x}",
                    hex.to_ascii_lowercase(),
                    (a * 255.0).round() as u8
                ))
            }
            _ => None,
        }
    })();
    short.or_else(|| colour(value))
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

/// Two colour values the same: as written, to a rounding, or as the colours
/// they read as (a tool that drops `hex`, or writes the components again).
fn same(a: &Value, b: &Value) -> bool {
    if close(a, b) {
        return true;
    }
    let read = |v: &Value| match colour(v).and_then(|c| exact_color::parse(&c))? {
        Parsed::Legacy(c) => Some(Half::legacy(c).wide()),
        Parsed::Wide(w) => Some(w),
        _ => None,
    };
    match (read(a), read(b)) {
        (Some(x), Some(y)) => {
            let (p, q) = (x.linear_srgb(), y.linear_srgb());
            p.iter().zip(&q).all(|(m, n)| (m - n).abs() < 1e-9) && (x.alpha - y.alpha).abs() < 1e-9
        }
        _ => false,
    }
}

/// A tuple: every axis and its value.
type Modes = Vec<(String, String)>;

/// The baseline an incoming export is compared with: the kept one its
/// header's digest names; else the design's own export when the incoming
/// file came from the design as it is (its digest, or with no header its
/// contexts); else, with no header, the newest kept one with its contexts.
fn baseline_for(incoming: &Value, kept: &[String], fresh: &str) -> Result<String, String> {
    let wanted = header(incoming).and_then(|h| h["digest"].as_str());
    if let Some(d) = wanted {
        if let Some(b) = kept.iter().find(|b| digest(b).as_deref() == Some(d)) {
            return Ok(b.clone());
        }
        if digest(fresh).as_deref() == Some(d) {
            return Ok(fresh.to_string());
        }
        return Err(format!("no baseline for this export: it came from the design at digest {d}, which no kept export here and not the design as it is has; export again, and make the edits in that file"));
    }
    let contexts = |text: &str| {
        serde_json::from_str::<Value>(text).ok().and_then(|d| {
            d["modifiers"]["mode"]["contexts"]
                .as_object()
                .map(|c| c.keys().cloned().collect::<Vec<_>>())
        })
    };
    let theirs = incoming["modifiers"]["mode"]["contexts"]
        .as_object()
        .map(|c| c.keys().cloned().collect::<Vec<_>>());
    if contexts(fresh) == theirs {
        return Ok(fresh.to_string());
    }
    kept.iter()
        .find(|b| contexts(b) == theirs)
        .cloned()
        .ok_or_else(|| "no baseline for this export: its header is gone and no kept export has its contexts; export again".to_string())
}

/// Re-import a Tamber export (`incoming`) against the baseline it came from
/// (one of `kept`, newest first, by its header's digest; or a fresh export of
/// `current`, the design as it is). A token whose `$value` is unchanged in
/// every context proposes nothing, its source kept. A changed one proposes
/// a `set-token` per tuple whose value changed, its `when` naming every axis
/// of that tuple; an axis `under scheme=dark` only on a tuple off its
/// default, since its light side is the line without it. Each token's
/// operations are checked as the design would take them, and a token they
/// do not apply to, one the baseline lacks or the file removes, one missing
/// from a context, or a value this importer reads no colour from, is
/// reported.
pub fn reimport(
    incoming: &str,
    kept: &[String],
    current: &str,
    vocab: &Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    let refuse = |m: String| vec![Diagnostic::new("tamber-export-import", m, 1, 1)];
    let doc: Value =
        serde_json::from_str(incoming).map_err(|e| refuse(format!("not JSON: {e}")))?;
    let design = crate::parse(current)?;
    let fresh = crate::check(current, vocab).and_then(|f| dtcg(&f, vocab))?;
    let baseline = baseline_for(&doc, kept, &fresh).map_err(refuse)?;
    let base: Value = serde_json::from_str(&baseline)
        .map_err(|e| refuse(format!("the baseline: not JSON: {e}")))?;
    let contexts =
        context_map(&base).ok_or_else(|| refuse("the baseline is no Tamber export".into()))?;
    let under: Vec<String> = header(&base)
        .and_then(|h| h["axes"].as_array())
        .into_iter()
        .flatten()
        .filter(|a| a["under"] == "dark")
        .filter_map(|a| a["name"].as_str().map(str::to_string))
        .collect();
    let defaults: Vec<(String, String)> = header(&base)
        .and_then(|h| h["axes"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|a| {
            Some((
                a["name"].as_str()?.to_string(),
                a["values"][0].as_str()?.to_string(),
            ))
        })
        .collect();
    let theirs = tokens_by_context(&doc);
    let ours = tokens_by_context(&base);
    let value = |tokens: &std::collections::BTreeMap<String, Vec<(String, Value)>>,
                 context: &str,
                 name: &str| {
        tokens
            .get(context)
            .and_then(|all| all.iter().find(|(n, _)| n == name))
            .map(|(_, t)| t["$value"].clone())
    };
    // The tuples: each one's light and dark context, and its base, the
    // tuple with every axis under scheme=dark at its default, whose light
    // side it shows.
    let mut tuples: Vec<(Modes, [String; 2])> = Vec::new();
    for (name, modes, dark) in &contexts {
        match tuples.iter_mut().find(|(m, _)| m == modes) {
            Some((_, sides)) => sides[usize::from(*dark)] = name.clone(),
            None => {
                let mut sides = [name.clone(), name.clone()];
                sides[usize::from(*dark)] = name.clone();
                tuples.push((modes.clone(), sides));
            }
        }
    }
    let base_of = |modes: &Modes| -> Modes {
        modes
            .iter()
            .map(|(a, v)| match under.contains(a) {
                true => (
                    a.clone(),
                    defaults
                        .iter()
                        .find(|(d, _)| d == a)
                        .map_or(v.clone(), |(_, dv)| dv.clone()),
                ),
                false => (a.clone(), v.clone()),
            })
            .collect()
    };
    let light_of = |modes: &Modes| -> Option<&String> {
        let b = base_of(modes);
        tuples.iter().find(|(m, _)| *m == b).map(|(_, s)| &s[0])
    };
    let mut names: Vec<String> = Vec::new();
    for all in theirs.values().chain(ours.values()) {
        for (n, _) in all {
            if !names.contains(n) {
                names.push(n.clone());
            }
        }
    }
    let first = contexts
        .first()
        .map(|(n, ..)| n.clone())
        .unwrap_or_default();
    let in_base = |n: &str| {
        ours.get(&first)
            .is_some_and(|all| all.iter().any(|(m, _)| m == n))
    };
    let (mut ops, mut unmapped, mut mapped) = (Vec::new(), Vec::new(), 0);
    let report = |unmapped: &mut Vec<Unmapped>, name: &str, block: &str, why: String| {
        unmapped.push(Unmapped {
            block: block.to_string(),
            name: name.to_string(),
            value: String::new(),
            why,
            token: Some(name.to_string()),
        });
    };
    for name in &names {
        if !in_base(name) {
            report(&mut unmapped, name, "", "not in the export it came from: a new token is added to the design, not by re-import".into());
            continue;
        }
        let missing: Vec<&str> = contexts
            .iter()
            .filter(|(c, ..)| value(&theirs, c, name).is_none())
            .map(|(c, ..)| c.as_str())
            .collect();
        if missing.len() == contexts.len() {
            report(
                &mut unmapped,
                name,
                "",
                "not in the incoming file: a token is removed in the design, not by re-import"
                    .into(),
            );
            continue;
        }
        if let Some(c) = missing.first() {
            report(
                &mut unmapped,
                name,
                c,
                "missing from this context: each context of the export is a complete set".into(),
            );
            continue;
        }
        // The design's own spelling: exactly, or a role's row in any case.
        let overrides = ours[&first]
            .iter()
            .find(|(n, _)| n == name)
            .is_some_and(|(_, t)| t["$extensions"]["exact"]["overrides"].is_string());
        let written = design
            .tokens
            .iter()
            .find(|t| t.name.value == *name)
            .or_else(|| {
                overrides
                    .then(|| {
                        design
                            .tokens
                            .iter()
                            .find(|t| t.name.value.eq_ignore_ascii_case(name))
                    })
                    .flatten()
            })
            .map_or(name.clone(), |t| t.name.value.clone());
        let changed = |context: &str| !matches!((value(&theirs, context, name), value(&ours, context, name)), (Some(a), Some(b)) if same(&a, &b));
        let mut token_ops = Vec::new();
        let mut why = None;
        for (modes, sides) in &tuples {
            let Some(light) = light_of(modes) else {
                continue;
            };
            // A light side under an axis under scheme=dark shows its base's:
            // an edit there alone is no value of the design.
            if light != &sides[0] && changed(&sides[0]) && !changed(light) {
                why = Some((sides[0].clone(), format!("the light side here is `{light}`'s: an axis under scheme=dark varies the dark side only")));
                break;
            }
            if !changed(light) && !changed(&sides[1]) {
                continue;
            }
            // A tuple off an axis under scheme=dark whose dark side follows
            // its base's, before and after, takes the base's new line.
            let base_dark = tuples
                .iter()
                .find(|(m, _)| *m == base_of(modes))
                .map(|(_, s)| &s[1]);
            if let Some(bd) = base_dark.filter(|bd| *bd != &sides[1]) {
                let follows = |tokens| matches!((value(tokens, &sides[1], name), value(tokens, bd, name)), (Some(a), Some(b)) if same(&a, &b));
                if follows(&theirs) && follows(&ours) {
                    continue;
                }
            }
            let read = [light, &sides[1]]
                .map(|c| value(&theirs, c, name).as_ref().and_then(written_colour));
            let [Some(l), Some(d)] = read else {
                why = Some((
                    sides.join(" / "),
                    "a value this importer reads no colour from".into(),
                ));
                break;
            };
            let off: bool = modes.iter().any(|(a, v)| {
                under.contains(a) && base_of(modes).iter().any(|(b, w)| b == a && w != v)
            });
            let when: Modes = modes
                .iter()
                .filter(|(a, _)| off || !under.contains(a))
                .cloned()
                .collect();
            token_ops.push(Op::SetToken {
                name: written.clone(),
                when,
                value: if l == d {
                    l
                } else {
                    format!("light-dark({l}, {d})")
                },
                at: None,
                comment: None,
            });
        }
        if why.is_none() && !token_ops.is_empty() {
            match crate::apply_all(current, &token_ops, vocab) {
                // The design has it already (this file accepted before).
                Ok(edited) if edited.text == current => token_ops.clear(),
                Ok(_) => {}
                Err(all) => {
                    let first = all
                        .first()
                        .map_or(String::new(), |d| format!("{}: {}", d.id, d.message));
                    why = Some((
                        String::new(),
                        format!("the design would refuse it ({first})"),
                    ));
                }
            }
        }
        match why {
            Some((block, w)) => report(&mut unmapped, name, &block, w),
            None => {
                mapped += 1;
                ops.extend(token_ops);
            }
        }
    }
    let counted = counted(names.len(), mapped, unmapped.len())?;
    Ok(Import {
        ops,
        offers: Vec::new(),
        unmapped,
        counted,
    })
}
