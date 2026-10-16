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
        // Its 8-bit bytes and alpha byte, as the plan carries it (`#rrggbbaa`),
        // not the unquantised value.
        Half::Legacy(c, _) => {
            let mut w = Half::legacy(c).wide();
            w.alpha = (c.a * 255.0).round() / 255.0;
            w
        }
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
    let contexts = doc
        .get("modifiers")?
        .get("mode")?
        .get("contexts")?
        .as_object()?;
    if let Some(h) = header(doc) {
        let all: Vec<(String, Modes, bool)> = h["contexts"]
            .as_object()?
            .iter()
            .map(|(name, c)| {
                let modes = c["modes"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(a, v)| (a.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect();
                (name.clone(), modes, c["scheme"] == "dark")
            })
            .collect();
        return all
            .iter()
            .all(|(n, ..)| contexts.contains_key(n))
            .then_some(all);
    }
    // Without the header, the export's shape: a set `exact` and the one
    // modifier `mode`, in that order, its contexts named for their tuples.
    let refs: Vec<&str> = doc
        .get("resolutionOrder")?
        .as_array()?
        .iter()
        .filter_map(|r| r["$ref"].as_str())
        .collect();
    if refs != ["#/sets/exact", "#/modifiers/mode"] || doc["modifiers"].as_object()?.len() != 1 {
        return None;
    }
    contexts
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

/// A token tree's tokens: each name (a group's `$root` the group's) and its
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

/// A context source's tokens, in order: a token tree, or a reference to a
/// set or a tree in the same document (`#/sets/…`), resolved as the
/// Resolver Module says. A reference elsewhere, or a cycle, is refused.
fn source_tokens(
    doc: &Value,
    src: &Value,
    depth: usize,
    out: &mut Vec<(String, Value)>,
) -> Result<(), String> {
    if depth > 16 {
        return Err("references nest too deep".into());
    }
    let Some(r) = src.get("$ref").and_then(Value::as_str) else {
        flatten(src, "", out);
        return Ok(());
    };
    let target = r
        .strip_prefix('#')
        .and_then(|p| doc.pointer(p))
        .ok_or_else(|| format!("`{r}` names nothing in this document"))?;
    match target.get("sources").and_then(Value::as_array) {
        Some(srcs) => {
            for s in srcs {
                source_tokens(doc, s, depth + 1, out)?;
            }
        }
        None => flatten(target, "", out),
    }
    Ok(())
}

/// Each context's tokens by name, from a document's `mode` modifier, a
/// later source after an earlier.
type ByContext = std::collections::BTreeMap<String, Vec<(String, Value)>>;
fn tokens_by_context(doc: &Value) -> Result<ByContext, String> {
    let mut out = ByContext::new();
    for (context, srcs) in doc["modifiers"]["mode"]["contexts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let mut all = Vec::new();
        for src in srcs.as_array().into_iter().flatten() {
            source_tokens(doc, src, 0, &mut all).map_err(|e| format!("`{context}`: {e}"))?;
        }
        out.insert(context.clone(), all);
    }
    Ok(out)
}

/// A colour object as Tamber writes it: its `hex` where that, and its alpha
/// in steps of a byte, are exactly what its sRGB components say (an 8-bit
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
            Some(Some(a)) if a >= 0.0 && (a * 255.0 - (a * 255.0).round()).abs() < 1e-6 => {
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

/// A colour value as the colour it reads as.
fn read(value: &Value) -> Option<Wide> {
    match colour(value).and_then(|c| exact_color::parse(&c))? {
        Parsed::Legacy(c) => Some(Half::legacy(c).wide()),
        Parsed::Wide(w) => Some(w),
        _ => None,
    }
}

/// Two colours the same, to a reprint's rounding: a millionth in linear
/// sRGB, well inside an 8-bit step.
fn alike(x: &Wide, y: &Wide) -> bool {
    let (p, q) = (x.linear_srgb(), y.linear_srgb());
    p.iter().zip(&q).all(|(m, n)| (m - n).abs() < 1e-6) && (x.alpha - y.alpha).abs() < 1e-6
}

/// Two colour values the same: as written, or as the colours they read as
/// (a tool that drops `hex`, or prints the components again).
fn same(a: &Value, b: &Value) -> bool {
    a == b || matches!((read(a), read(b)), (Some(x), Some(y)) if alike(&x, &y))
}

/// A tuple: every axis and its value.
type Modes = Vec<(String, String)>;

/// The baseline an incoming export is compared with: the kept one its
/// header's digest names, or the design's own export when that is the
/// digest. With no header, the one candidate (a kept export, or the design's)
/// with the file's contexts; more than one is refused, since the file's
/// values cannot tell an edit from another export's.
fn baseline_for(incoming: &Value, kept: &[String], fresh: &str) -> Result<String, String> {
    if let Some(d) = header(incoming).and_then(|h| h["digest"].as_str()) {
        if let Some(b) = kept.iter().find(|b| digest(b).as_deref() == Some(d)) {
            return Ok(b.clone());
        }
        if digest(fresh).as_deref() == Some(d) {
            return Ok(fresh.to_string());
        }
        return Err(format!("no baseline for this export: it came from the design at digest {d}, which no kept export here and not the design as it is has; export again, and make the edits in that file"));
    }
    let keys = |doc: &Value| {
        doc["modifiers"]["mode"]["contexts"]
            .as_object()
            .map(|c| c.keys().cloned().collect::<Vec<_>>())
    };
    let theirs = keys(incoming);
    let mut found: Vec<&str> = Vec::new();
    let mut digests: Vec<Option<String>> = Vec::new();
    for b in kept.iter().map(String::as_str).chain([fresh]) {
        let doc: Value = serde_json::from_str(b).map_err(|e| format!("a baseline: {e}"))?;
        let d = digest(b);
        if keys(&doc) == theirs && !digests.contains(&d) {
            digests.push(d);
            found.push(b);
        }
    }
    match found.as_slice() {
        [one] => Ok(one.to_string()),
        [] => Err("no baseline for this export: its header is gone and no kept export, nor the design, has its contexts; export again".into()),
        _ => Err("no baseline for this export: its header is gone, and more than one export it could have come from is kept (the design changed since it was written); export again, and make the edits in that file".into()),
    }
}

/// Re-import a Tamber export (`incoming`) against the baseline it came from
/// (one of `kept`, newest first; or a fresh export of `current`, the design
/// as it is). A token whose `$value` is unchanged in every context proposes
/// nothing, its source kept. A changed one proposes a `set-token` per tuple
/// whose value changed, its `when` naming every axis of that tuple (an axis
/// `under scheme=dark` only on a tuple off its default, whose light side is
/// the line without it), each side the file's where it changed and the
/// design's as it is where it did not. The operations are applied and folded
/// as the design would take them: a tuple they reach that the file left
/// alone is pinned to the design's value, until every context renders as
/// wanted. A token the design would refuse, alone or with the others, one the
/// baseline lacks or the file removes, one missing from a context, a context
/// the export did not have, or a value no colour is read from, is reported.
pub fn reimport(
    incoming: &str,
    kept: &[String],
    current: &str,
    vocab: &Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    let refuse = |m: String| vec![Diagnostic::new("tamber-export-import", m, 1, 1)];
    let doc: Value =
        serde_json::from_str(incoming).map_err(|e| refuse(format!("not JSON: {e}")))?;
    let cur = crate::check(current, vocab)?;
    let fresh = dtcg(&cur, vocab)?;
    let baseline = baseline_for(&doc, kept, &fresh).map_err(refuse)?;
    let base: Value = serde_json::from_str(&baseline)
        .map_err(|e| refuse(format!("the baseline: not JSON: {e}")))?;
    let contexts =
        context_map(&base).ok_or_else(|| refuse("the baseline is no Tamber export".into()))?;
    let axes = header(&base)
        .and_then(|h| h["axes"].as_array())
        .cloned()
        .unwrap_or_default();
    let under: Vec<&str> = axes
        .iter()
        .filter(|a| a["under"] == "dark")
        .filter_map(|a| a["name"].as_str())
        .collect();
    let defaults: Vec<(&str, &str)> = axes
        .iter()
        .filter_map(|a| Some((a["name"].as_str()?, a["values"][0].as_str()?)))
        .collect();
    let theirs = tokens_by_context(&doc).map_err(refuse)?;
    let ours = tokens_by_context(&base).map_err(refuse)?;
    let value = |tokens: &ByContext, context: &str, name: &str| {
        tokens
            .get(context)
            // The last source that declares it wins, as the resolver says.
            .and_then(|all| all.iter().rev().find(|(n, _)| n == name))
            .map(|(_, t)| t["$value"].clone())
    };
    // The tuples, each its light and dark context; and a tuple's base, with
    // every axis under scheme=dark at its default, whose light side it shows.
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
            .map(|(a, v)| match under.contains(&a.as_str()) {
                true => (
                    a.clone(),
                    defaults
                        .iter()
                        .find(|(d, _)| *d == a)
                        .map_or(v.clone(), |(_, dv)| dv.to_string()),
                ),
                false => (a.clone(), v.clone()),
            })
            .collect()
    };
    let tuple_of = |modes: &Modes| tuples.iter().find(|(m, _)| m == modes);
    let modes_of = |context: &str| {
        tuples
            .iter()
            .find(|(_, s)| s.iter().any(|c| c == context))
            .map(|(m, _)| m.clone())
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
    let report = |unmapped: &mut Vec<Unmapped>, name: &str, block: &str, why: String| {
        unmapped.push(Unmapped {
            block: block.to_string(),
            name: name.to_string(),
            value: String::new(),
            why,
            token: Some(name.to_string()),
        });
    };
    let mut unmapped = Vec::new();
    // A context the export did not have: no tuple of the design.
    let extra: Vec<&String> = theirs
        .keys()
        .filter(|c| !contexts.iter().any(|(n, ..)| n == *c))
        .collect();
    for c in &extra {
        unmapped.push(Unmapped {
            block: (*c).clone(),
            name: (*c).clone(),
            value: String::new(),
            why: "a context the export did not have: its contexts are the design's tuples, and an axis is added to the design, not by re-import".into(),
            token: None,
        });
    }
    let mut accepted: Vec<(String, Vec<Op>)> = Vec::new();
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
        let changed = |context: &str| !matches!((value(&theirs, context, name), value(&ours, context, name)), (Some(a), Some(b)) if same(&a, &b));
        if !contexts.iter().any(|(c, ..)| changed(c)) {
            mapped_unchanged(&mut accepted, name);
            continue;
        }
        // The design's own spelling: exactly, or a role's row in any case.
        let overrides = ours[&first]
            .iter()
            .find(|(n, _)| n == name)
            .is_some_and(|(_, t)| t["$extensions"]["exact"]["overrides"].is_string());
        let declared = cur
            .tokens
            .iter()
            .zip(&cur.design.tokens)
            .find(|(_, t)| t.name.value == *name)
            .or_else(|| {
                overrides
                    .then(|| {
                        cur.tokens
                            .iter()
                            .zip(&cur.design.tokens)
                            .find(|(_, t)| t.name.value.eq_ignore_ascii_case(name))
                    })
                    .flatten()
            });
        let Some((now, decl)) = declared else {
            report(
                &mut unmapped,
                name,
                "",
                "not in the design any more: a token is added to the design, not by re-import"
                    .into(),
            );
            continue;
        };
        let written = decl.name.value.clone();
        // What a context should be: the file's where it changed, the design's
        // as it is where it did not, as text and as the colour it renders.
        let now_side = |context: &str| -> Option<(String, Option<Wide>)> {
            let modes = modes_of(context)?;
            let dark = contexts.iter().any(|(n, _, d)| n == context && *d);
            let pins: Vec<(&str, &str)> = modes
                .iter()
                .map(|(a, v)| (a.as_str(), v.as_str()))
                .collect();
            let folded = now.values.get(cur.tuple(&pins)?)?;
            let text = match folded {
                Folded::One(_) | Folded::Pair(..) => {
                    // An opaque 8-bit colour as a design writes it, `#rrggbb`.
                    let css = folded.side(dark).ok()?.css();
                    match css.strip_suffix("ff") {
                        Some(short) if css.len() == 9 && css.starts_with('#') => short.to_string(),
                        _ => css,
                    }
                }
                Folded::Host(h) => Folded::Host(h.clone()).css(),
            };
            Some((text, side(folded, dark, vocab).map(|(h, _)| rendered(h))))
        };
        let want = |context: &str| -> Option<(String, Option<Wide>)> {
            if changed(context) {
                let v = value(&theirs, context, name)?;
                Some((written_colour(&v)?, read(&v)))
            } else {
                now_side(context)
            }
        };
        let line =
            |modes: &Modes, sides: &[String; 2], every: bool| -> Result<Op, (String, String)> {
                let light = tuple_of(&base_of(modes)).map_or(&sides[0], |(_, s)| &s[0]);
                let (Some((l, _)), Some((d, _))) = (want(light), want(&sides[1])) else {
                    return Err((
                        sides.join(" / "),
                        "a value this importer reads no colour from".into(),
                    ));
                };
                let off = modes.iter().any(|(a, v)| {
                    under.contains(&a.as_str())
                        && base_of(modes).iter().any(|(b, w)| b == a && w != v)
                });
                let when: Modes = modes
                    .iter()
                    .filter(|(a, _)| every || off || !under.contains(&a.as_str()))
                    .cloned()
                    .collect();
                Ok(Op::SetToken {
                    name: written.clone(),
                    when,
                    value: if l == d {
                        l
                    } else {
                        format!("light-dark({l}, {d})")
                    },
                    at: None,
                    comment: None,
                })
            };
        let mut token_ops = Vec::new();
        let mut why = None;
        for (modes, sides) in &tuples {
            let light = tuple_of(&base_of(modes)).map_or(&sides[0], |(_, s)| &s[0]);
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
            // its base's, before and after, takes the base's new line (the
            // fold below pins it if not).
            if let Some((_, b)) = tuple_of(&base_of(modes)).filter(|(m, _)| m != modes) {
                let follows = |tokens: &ByContext| matches!((value(tokens, &sides[1], name), value(tokens, &b[1], name)), (Some(x), Some(y)) if same(&x, &y));
                if follows(&theirs) && follows(&ours) {
                    continue;
                }
            }
            match line(modes, sides, false) {
                Ok(op) => token_ops.push(op),
                Err(w) => {
                    why = Some(w);
                    break;
                }
            }
        }
        // Applied and folded as the design would take them: a tuple a changed
        // line reaches that the file left alone is pinned, with a line naming
        // every axis, until every context renders as wanted.
        let mut rounds = 0;
        while why.is_none() && !token_ops.is_empty() {
            let refused = |all: Vec<Diagnostic>| {
                let first = all
                    .first()
                    .map_or(String::new(), |d| format!("{}: {}", d.id, d.message));
                Some((
                    String::new(),
                    format!("the design would refuse it ({first})"),
                ))
            };
            let edited = match crate::apply_all(current, &token_ops, vocab) {
                // The design has it already (this file accepted before).
                Ok(edited) if edited.text == current => {
                    token_ops.clear();
                    break;
                }
                Ok(edited) => edited.text,
                Err(all) => {
                    why = refused(all);
                    break;
                }
            };
            let folding = match crate::check(&edited, vocab) {
                Ok(f) => f,
                Err(all) => {
                    why = refused(all);
                    break;
                }
            };
            let Some(checked) = folding
                .tokens
                .iter()
                .zip(&folding.design.tokens)
                .find(|(_, t)| t.name.value == written)
                .map(|(c, _)| c)
            else {
                break;
            };
            let mut off: Vec<(&Modes, &[String; 2])> = Vec::new();
            for (modes, sides) in &tuples {
                let pins: Vec<(&str, &str)> = modes
                    .iter()
                    .map(|(a, v)| (a.as_str(), v.as_str()))
                    .collect();
                let Some(folded) = folding.tuple(&pins).and_then(|t| checked.values.get(t)) else {
                    continue;
                };
                let wrong = [false, true].iter().any(|&dark| {
                    let have = side(folded, dark, vocab).map(|(h, _)| rendered(h));
                    let target = want(&sides[usize::from(dark)]).and_then(|(_, w)| w);
                    !matches!((have, target), (Some(h), Some(t)) if alike(&h, &t))
                });
                if wrong {
                    off.push((modes, sides));
                }
            }
            if off.is_empty() {
                break;
            }
            rounds += 1;
            if rounds > 3 {
                why = Some((String::new(), "no lines this design takes give every context its value (a light side that differs between tuples an axis under scheme=dark tells apart)".into()));
                break;
            }
            for (modes, sides) in off {
                match line(modes, sides, true) {
                    Ok(op) => token_ops.push(op),
                    Err(w) => {
                        why = Some(w);
                        break;
                    }
                }
            }
        }
        match why {
            Some((block, w)) => report(&mut unmapped, name, &block, w),
            None => accepted.push((name.clone(), token_ops)),
        }
    }
    // Together, as one transaction: a token whose operations the design
    // refuses beside the others' (a dial nothing uses any more) is reported.
    let mut ops: Vec<Op> = Vec::new();
    let mut mapped = 0;
    for (name, token_ops) in accepted {
        if !token_ops.is_empty() {
            let mut trial = ops.clone();
            trial.extend(token_ops.iter().cloned());
            if let Err(all) = crate::apply_all(current, &trial, vocab) {
                let first = all
                    .first()
                    .map_or(String::new(), |d| format!("{}: {}", d.id, d.message));
                report(
                    &mut unmapped,
                    &name,
                    "",
                    format!("the design would refuse it with the tokens before it ({first})"),
                );
                continue;
            }
            ops = trial;
        }
        mapped += 1;
    }
    let counted = counted(names.len() + extra.len(), mapped, unmapped.len())?;
    Ok(Import {
        ops,
        offers: Vec::new(),
        unmapped,
        counted,
    })
}

/// An unchanged token: mapped, no operations.
fn mapped_unchanged(accepted: &mut Vec<(String, Vec<Op>)>, name: &str) {
    accepted.push((name.to_string(), Vec::new()));
}
