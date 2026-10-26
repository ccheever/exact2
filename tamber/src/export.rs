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

mod refs;
use crate::export_batch::{Planned, Taken};
use crate::export_dials::{alike, last_by_name, read_css, Lasts};
use crate::import::{counted, dtcg::colour, Import, Unmapped};
use crate::syntax::Token;
use crate::{fallback, Diagnostic, Folded, Folding, Half, Host, Op, Vocab};
use exact_color::{Parsed, Space, Wide};
pub(crate) use refs::ByContext;
use refs::{context_map, header, tokens_by_context};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

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
    "a length: `$type: dimension`, px or `rem`; an `em`, a text style, the hairline or a base name the design leaves unset (its neutral column, the web's look) as px at the default root size (16), its source in `$extensions.exact.lossy`",
    "a number: `$type: number`; a family: `$type: fontFamily`, its members unquoted",
    "a type: `$type: typography`, every property written; what the token leaves unsaid (the text style's weight and leading, the family) as the web's default, named in `$extensions.exact.unsaid`; a style in `fontStyle`, a line height that is no number in `leading`",
    "a shadow: `$type: shadow`, a text shadow marked `kind: text-shadow`, `currentcolor` as black marked `currentcolor`, a length over a base name the design leaves unset as a length's, the shadow's source in `lossy`",
    "a duration: `$type: duration` in ms; an easing: `$type: cubicBezier`, a keyword as its curve, `steps()`, `linear()` and a spring as `linear`'s, the source in `lossy`",
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

/// A colour's side as 2025.10's object, for a shadow's colour (stage 6 G).
pub(crate) fn colour_side(value: &Folded, dark: bool, vocab: &Vocab) -> Option<Value> {
    side(value, dark, vocab).map(|(h, _)| colour_json(h).0)
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
    // A name another one continues (`a` beside `a.b`): each name's dotted
    // prefixes, read once.
    let groups: std::collections::HashSet<&str> = f
        .tokens
        .iter()
        .flat_map(|t| t.name.match_indices('.').map(|(i, _)| &t.name[..i]))
        .collect();
    let group = |name: &str| groups.contains(name);
    let mut errors = Vec::new();
    let mut map = Map::new();
    for c in &contexts {
        let mut tree = Map::new();
        let mut count = 0;
        for (checked, t) in f.tokens.iter().zip(&f.design.tokens) {
            // Every other kind as its DTCG type (stage 6 G).
            if checked.kind != crate::Kind::Color {
                let Some(w) = checked
                    .values
                    .get(c.tuple)
                    .and_then(|v| crate::export_kinds::written(v, c.dark, vocab))
                else {
                    errors.push(Diagnostic::new(
                        "tamber-export",
                        format!(
                            "`{}` has no value DTCG writes in `{}`",
                            checked.name, c.name
                        ),
                        t.name.line,
                        t.name.col,
                    ));
                    continue;
                };
                let mut exact = w.exact;
                let text = source(f, t, &c.modes, c.dark).trim();
                if text != checked.values[c.tuple].css() {
                    exact.insert("source".into(), json!(text));
                }
                let mut token = Map::new();
                token.insert("$type".into(), json!(w.ty));
                token.insert("$value".into(), w.value);
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
                continue;
            }
            let Some(folded) = checked.values.get(c.tuple).and_then(crate::Value::color) else {
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
        map.insert(c.name.clone(), json!([crate::budget::tree(tree)?]));
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
    colour(value).and_then(|c| read_css(&c))
}

/// Two colour values the same: as written, or as the colours they read as
/// (a tool that drops `hex`, or prints the components again).
fn same(a: &Value, b: &Value) -> bool {
    a == b
        || matches!((read(a), read(b)), (Some(x), Some(y)) if alike(&x, &y))
        // Another kind's numbers by their value: a tool's `27` is `27.0`.
        || crate::export_kinds::alike(a, b)
}

/// A tuple: every axis and its value.
type Modes = Vec<(String, String)>;

/// A line's contexts: each tuple, side and context name.
type Members<'a> = Vec<(&'a Modes, bool, &'a String)>;

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
/// nothing, its source kept. A changed one edits the line that wins for the
/// changed tuples (D6's target branch), or, where that line's other tuples
/// did not change, adds the narrowest `when` that tells the changed ones
/// apart; a side the file did not change keeps the design's. The operations
/// are applied and folded as the design would take them, and a tuple they
/// reach that the file left alone is pinned to the design's value. A token
/// the design would refuse, alone or with the others, one the baseline lacks
/// or the file removes, one missing from a context, a context the export did
/// not have, or a value no colour is read from, is reported.
pub fn reimport(
    incoming: &str,
    kept: &[String],
    current: &str,
    vocab: &Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    reimport_against(incoming, None, kept, current, vocab)
}

/// [`reimport`] against `explicit`, the export a client says it pulled
/// (stage 10b's `baseline`), when given: that one only, never another kept
/// one or the design's own, and only while its axes and contexts are the
/// design's (an export from before an axis moved is pulled again).
pub fn reimport_against(
    incoming: &str,
    explicit: Option<&str>,
    kept: &[String],
    current: &str,
    vocab: &Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    let _work = crate::budget::Work::begin(crate::budget::REIMPORT);
    let planned = reimport_within(incoming, explicit, kept, current, vocab)?;
    // A search that stopped at the budget (the narrowest `when`'s) widened
    // nothing in silence: the re-import is refused in the budget's words.
    match crate::budget::spent() {
        Some(why) => Err(vec![Diagnostic::new("tamber-export-import", why, 1, 1)]),
        None => Ok(planned),
    }
}

/// [`reimport_against`] within its budget.
fn reimport_within(
    incoming: &str,
    explicit: Option<&str>,
    kept: &[String],
    current: &str,
    vocab: &Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    let refuse = |m: String| vec![Diagnostic::new("tamber-export-import", m, 1, 1)];
    let doc: Value = crate::import::dtcg_doc(incoming, &refuse)?;
    let cur = crate::check(current, vocab)?;
    let fresh = dtcg(&cur, vocab)?;
    let baseline = match explicit {
        Some(b) => {
            let (theirs, ours): (Value, Value) = (
                crate::import::dtcg_doc(b, &|m| refuse(format!("the baseline: {m}")))?,
                serde_json::from_str(&fresh)
                    .map_err(|e| refuse(format!("the design's export: {e}")))?,
            );
            // Each context's name, tuple and scheme, and the axes, the design's
            // own: a recognised digest over a forged mapping plans nothing.
            let axes = |d: &Value| header(d).map(|h| h["axes"].clone());
            if context_map(&theirs).is_none()
                || context_map(&theirs) != context_map(&ours)
                || axes(&theirs) != axes(&ours)
            {
                return Err(refuse(
                    "pull first: the baseline's axes or contexts are not the design's".into(),
                ));
            }
            b.to_string()
        }
        None => baseline_for(&doc, kept, &fresh).map_err(refuse)?,
    };
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
    // Each context's tokens by name, read once: the last source that
    // declares one wins, as the resolver says.
    let (their, our) = (last_by_name(&theirs), last_by_name(&ours));
    fn value<'a>(tokens: &Lasts<'a>, context: &str, name: &str) -> Option<&'a Value> {
        tokens.get(context)?.get(name).map(|t| &t["$value"])
    }
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
    let mut seen = std::collections::HashSet::new();
    for all in theirs.values().chain(ours.values()) {
        for (n, _) in all {
            if seen.insert(n.as_str()) {
                names.push(n.clone());
            }
        }
    }
    let first = contexts
        .first()
        .map(|(n, ..)| n.clone())
        .unwrap_or_default();
    let in_base = |n: &str| {
        our.get(first.as_str())
            .is_some_and(|all| all.contains_key(n))
    };
    // The first context's first token of each name, and the design's own
    // tokens by name and by a role's spelling in any case, each the first.
    let mut first_of: HashMap<&str, &Value> = HashMap::new();
    for (n, t) in ours.get(&first).into_iter().flatten() {
        first_of.entry(n.as_str()).or_insert(t);
    }
    let (mut declared_as, mut declared_any) = (HashMap::new(), HashMap::new());
    for (i, t) in cur.design.tokens.iter().enumerate().take(cur.tokens.len()) {
        declared_as.entry(t.name.value.as_str()).or_insert(i);
        declared_any
            .entry(t.name.value.to_ascii_lowercase())
            .or_insert(i);
    }
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
    // Tokens whose every changed context is on a line that is one colour dial
    // alone, moved to one literal: the dial's change (D14; stage 10b).
    let mut dialed: Vec<crate::export_dials::Candidate> = Vec::new();
    // Each token's plan, or what the whole proposal's folding (`taken`)
    // makes of its lines (option B, `export_batch`).
    let plan = |name: &String, taken: Option<Taken>| -> Planned {
        let refuse = |block: &str, why: &str| Planned::Report(block.to_string(), why.to_string());
        if !in_base(name) {
            return refuse("", "not in the export it came from: a new token is added to the design, not by re-import");
        }
        let missing: Vec<&str> = contexts
            .iter()
            .filter(|(c, ..)| value(&their, c, name).is_none())
            .map(|(c, ..)| c.as_str())
            .collect();
        if missing.len() == contexts.len() {
            return refuse(
                "",
                "not in the incoming file: a token is removed in the design, not by re-import",
            );
        }
        if let Some(c) = missing.first() {
            return refuse(
                c,
                "missing from this context: each context of the export is a complete set",
            );
        }
        // The contexts the file changes, each read once (a context is asked
        // many times a token).
        let moved: std::collections::HashSet<&str> = contexts
            .iter()
            .map(|(c, ..)| c.as_str())
            .filter(|c| !matches!((value(&their, c, name), value(&our, c, name)), (Some(a), Some(b)) if same(a, b)))
            .collect();
        let changed = |context: &str| moved.contains(context);
        if !contexts.iter().any(|(c, ..)| changed(c)) {
            return Planned::Unchanged;
        }
        // The design's own spelling: exactly, or a role's row in any case.
        let overrides = first_of
            .get(name.as_str())
            .is_some_and(|t| t["$extensions"]["exact"]["overrides"].is_string());
        let declared = declared_as
            .get(name.as_str())
            .or_else(|| {
                overrides
                    .then(|| declared_any.get(&name.to_ascii_lowercase()))
                    .flatten()
            })
            .copied();
        let Some(index) = declared else {
            return refuse(
                "",
                "not in the design any more: a token is added to the design, not by re-import",
            );
        };
        let (now, decl) = (&cur.tokens[index], &cur.design.tokens[index]);
        let written = decl.name.value.clone();
        // Every other kind: one value a tuple, whatever the scheme (stage 6 G).
        if now.kind != crate::Kind::Color {
            let baseline = |context: &str| {
                our.get(context)
                    .and_then(|all| all.get(name.as_str()))
                    .map_or((Value::Null, Value::Null), |t| {
                        (t["$value"].clone(), t["$extensions"]["exact"].clone())
                    })
            };
            let incoming = |context: &str| {
                let t = (*their.get(context)?.get(name.as_str())?).clone();
                Some((
                    t["$type"].as_str()?.to_string(),
                    t["$value"].clone(),
                    t["$extensions"]["exact"].clone(),
                ))
            };
            return match crate::export_kinds::reimport_ops(
                decl,
                &written,
                &tuples,
                &under,
                &changed,
                &incoming,
                &baseline,
                &|modes| branch(decl, &under, modes, false),
                taken
                    .as_ref()
                    .map(|t| (t.folding, t.unchanged, &cur, index)),
                vocab,
            ) {
                Ok(ops) => Planned::Kind(index, ops),
                Err((block, why)) => Planned::Report(block, why),
            };
        }
        // What a context should be: the file's where it changed, the design's
        // as it is where it did not, as text and as the colour it renders.
        let now_side = |context: &str| -> Option<(String, Option<Wide>)> {
            let modes = modes_of(context)?;
            let dark = contexts.iter().any(|(n, _, d)| n == context && *d);
            let pins: Vec<(&str, &str)> = modes
                .iter()
                .map(|(a, v)| (a.as_str(), v.as_str()))
                .collect();
            let folded = now.values.get(cur.tuple(&pins)?)?.color()?;
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
                let v = value(&their, context, name)?.clone();
                Some((written_colour(&v)?, read(&v)))
            } else {
                now_side(context)
            }
        };
        // A line's source, and one side of it as written: a top-level
        // `light-dark(a, b)`'s, else the whole source.
        let text_of = |b: Option<usize>| {
            b.map_or(&decl.value.value, |w| &decl.whens[w].value.value)
                .trim()
                .to_string()
        };
        let side_text = |b: Option<usize>, dark: bool| {
            let source = text_of(b);
            sides_of(&source).map_or(source, |p| p[usize::from(dark)].clone())
        };
        // A side as a new line writes it: the file's where it changed, else
        // the side of the line that wins there, as written (an expression,
        // a reference), not its folded colour.
        let side_of = |modes: &Modes, context: &String, dark: bool| -> Option<String> {
            match changed(context) {
                true => want(context).map(|(t, _)| t),
                false => Some(side_text(branch(decl, &under, modes, dark), dark)),
            }
        };
        let line =
            |modes: &Modes, sides: &[String; 2], every: bool| -> Result<Op, (String, String)> {
                let light = tuple_of(&base_of(modes)).map_or(&sides[0], |(_, s)| &s[0]);
                let (Some(l), Some(d)) = (
                    side_of(modes, light, false),
                    side_of(modes, &sides[1], true),
                ) else {
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
        // A dial's change, if it is one; its literals are planned the same,
        // for when the dial cannot take it.
        let candidate = crate::export_dials::candidate(
            &cur,
            decl,
            &written,
            &tuples,
            &changed,
            &|c| want(c).map(|(t, _)| t),
            &|modes, dark| branch(decl, &under, modes, dark),
        );
        let mut token_ops: Vec<Op> = Vec::new();
        let mut why = None;
        // A light side under an axis under scheme=dark shows its base's: an
        // edit there alone is no value of the design.
        for (modes, sides) in &tuples {
            let light = tuple_of(&base_of(modes)).map_or(&sides[0], |(_, s)| &s[0]);
            if light != &sides[0] && changed(&sides[0]) && !changed(light) {
                why = Some((sides[0].clone(), format!("the light side here is `{light}`'s: an axis under scheme=dark varies the dark side only")));
                break;
            }
        }
        // D6's target branch: each context's side comes from the line that
        // wins for its tuple (`explain`), the default or a `when`. A branch
        // whose contexts on a side all changed, to one value, takes it there,
        // its other side kept. One some of whose contexts did not change
        // cannot move without moving them: its changed tuples get the
        // narrowest `when` that holds for them and for none of its others.
        let mut branches: Vec<(Option<usize>, Members)> = Vec::new();
        for (modes, sides) in &tuples {
            for dark in [false, true] {
                let b = branch(decl, &under, modes, dark);
                let member = (modes, dark, &sides[usize::from(dark)]);
                match branches.iter_mut().find(|(x, _)| *x == b) {
                    Some((_, all)) => all.push(member),
                    None => branches.push((b, vec![member])),
                }
            }
        }
        let mut narrow: Vec<&Modes> = Vec::new();
        for (b, members) in &branches {
            if why.is_some() {
                break;
            }
            if !members.iter().any(|(_, _, c)| changed(c)) {
                continue;
            }
            // Per side: none of its contexts changed (kept), or all to one value.
            let side_new = |dark: bool| -> Result<Option<String>, ()> {
                let on: Vec<&String> = members
                    .iter()
                    .filter(|(_, d, _)| *d == dark)
                    .map(|(_, _, c)| *c)
                    .collect();
                let moved: Vec<&String> = on.iter().copied().filter(|c| changed(c)).collect();
                if moved.is_empty() {
                    return Ok(None);
                }
                if moved.len() != on.len() {
                    return Err(());
                }
                let texts: Vec<Option<String>> =
                    moved.iter().map(|c| want(c).map(|(t, _)| t)).collect();
                match texts.first() {
                    Some(Some(t)) if texts.iter().all(|x| x.as_ref() == Some(t)) => {
                        Ok(Some(t.clone()))
                    }
                    _ => Err(()),
                }
            };
            match (side_new(false), side_new(true)) {
                (Ok(l), Ok(d)) => {
                    let source = text_of(*b);
                    let kept = |dark: bool| {
                        sides_of(&source).map_or(source.clone(), |p| p[usize::from(dark)].clone())
                    };
                    let l = l.unwrap_or_else(|| kept(false));
                    let d = d.unwrap_or_else(|| kept(true));
                    token_ops.push(Op::SetToken {
                        name: written.clone(),
                        when: b.map_or(Vec::new(), |w| decl.whens[w].conds.clone()),
                        value: if l == d {
                            l
                        } else {
                            format!("light-dark({l}, {d})")
                        },
                        at: None,
                        comment: None,
                    });
                }
                _ => {
                    for (modes, _, c) in members {
                        if changed(c) && !narrow.contains(modes) {
                            narrow.push(modes);
                        }
                    }
                }
            }
        }
        // A `when` naming an axis under scheme=dark keeps the light side of
        // the line its tuples show light from: when that line's light moved,
        // its own light follows.
        let moved: Vec<(Option<usize>, String)> = token_ops
            .iter()
            .filter_map(|op| match op {
                Op::SetToken { when, value, .. } => {
                    let b = if when.is_empty() {
                        None
                    } else {
                        decl.whens.iter().position(|w| &w.conds == when)
                    };
                    let light = sides_of(value).map_or(value.clone(), |p| p[0].clone());
                    Some((b, light))
                }
                _ => None,
            })
            .collect();
        for (w, line_w) in decl.whens.iter().enumerate() {
            if !line_w
                .conds
                .iter()
                .any(|(a, _)| under.contains(&a.as_str()))
            {
                continue;
            }
            // Its tuples, whether it wins there or another as narrow does
            // (the checker reads a shadowed one too).
            let Some((modes, _)) = tuples
                .iter()
                .find(|(m, _)| line_w.conds.iter().all(|c| m.contains(c)))
            else {
                continue;
            };
            let base_branch = branch(decl, &under, modes, false);
            let own = text_of(Some(w));
            let Some((_, light)) = moved.iter().find(|(b, l)| {
                *b == base_branch && sides_of(&own).map_or(own.clone(), |p| p[0].clone()) != *l
            }) else {
                continue;
            };
            let dark = sides_of(&own).map_or(own.clone(), |p| p[1].clone());
            match token_ops
                .iter_mut()
                .find(|op| matches!(op, Op::SetToken { when, .. } if *when == line_w.conds))
            {
                Some(Op::SetToken { value, .. }) => {
                    let d = sides_of(value).map_or(value.clone(), |p| p[1].clone());
                    *value = format!("light-dark({light}, {d})");
                }
                _ => token_ops.push(Op::SetToken {
                    name: written.clone(),
                    when: line_w.conds.clone(),
                    value: format!("light-dark({light}, {dark})"),
                    at: None,
                    comment: None,
                }),
            }
        }
        for modes in narrow {
            if why.is_some() {
                break;
            }
            let Some((_, sides)) = tuple_of(modes) else {
                continue;
            };
            // Only the dark side moved: a `when` may name an axis under
            // scheme=dark; a light side can be told apart by the others only.
            let light_moved = changed(&sides[0]);
            let others: Vec<&Modes> = tuples
                .iter()
                .map(|(m, _)| m)
                .filter(|m| *m != modes)
                .filter(|m| {
                    // A tuple it must leave alone: one whose wanted value on a
                    // side differs from this tuple's.
                    let (_, s) = tuple_of(m).expect("a tuple");
                    [0, 1].iter().any(|&i| !matches!((want(&s[i]), want(&sides[i])), (Some((a, _)), Some((b, _))) if a == b))
                })
                .collect();
            let axes: Vec<&(String, String)> = modes
                .iter()
                .filter(|(a, _)| !(light_moved && under.contains(&a.as_str())))
                .collect();
            let mut found: Option<Modes> = None;
            let mut tried = 0; // (past SELECTORS, the line naming every axis)
            'size: for size in 0..=axes.len() {
                for pick in crate::selectors::subsets(axes.len(), size) {
                    tried += 1;
                    // Each candidate charged to the re-import's budget for
                    // the tuples and `when` lines it is held against (the
                    // checker's work in token counts). Past the 4096
                    // candidates, the line naming every axis (the
                    // selector's rule); past the budget, the same, and the
                    // re-import is then refused in the budget's words.
                    let cost = tuples.len() * (1 + decl.whens.len() + token_ops.len());
                    if tried > crate::selectors::SELECTORS
                        || crate::budget::spend(cost as u64).is_err()
                    {
                        break 'size;
                    }
                    let conds: Modes = pick.iter().map(|&k| axes[k].clone()).collect();
                    if conds.is_empty() {
                        continue;
                    }
                    let holds = |m: &Modes| conds.iter().all(|c| m.contains(c));
                    // No tie with a `when` as narrow wherever it holds: the
                    // checker refuses two that hold and name as many axes.
                    // ... or with a `when` this import adds.
                    let queued: Vec<&Modes> = token_ops
                        .iter()
                        .filter_map(|op| match op {
                            Op::SetToken { when, .. } if !when.is_empty() => Some(when),
                            _ => None,
                        })
                        .collect();
                    let ties = tuples.iter().any(|(m, _)| {
                        holds(m)
                            && decl
                                .whens
                                .iter()
                                .map(|w| &w.conds)
                                .chain(queued.iter().copied())
                                .any(|w| {
                                    w.len() == conds.len()
                                        && *w != conds
                                        && w.iter().all(|c| m.contains(c))
                                })
                    });
                    if !ties && others.iter().all(|m| !holds(m)) {
                        found = Some(conds);
                        break 'size;
                    }
                }
            }
            match line(modes, sides, false) {
                Ok(Op::SetToken { name, value, .. }) => {
                    let when = found.unwrap_or_else(|| modes.clone());
                    // Another changed tuple's narrowest line may be this one.
                    if !token_ops
                        .iter()
                        .any(|op| matches!(op, Op::SetToken { when: w, .. } if *w == when))
                    {
                        token_ops.push(Op::SetToken {
                            name,
                            when,
                            value,
                            at: None,
                            comment: None,
                        });
                    }
                }
                Ok(_) => {}
                Err(w) => why = Some(w),
            }
        }
        // Applied and folded as the design would take them, with the rest of
        // the proposal: a tuple a changed line reaches that the file left
        // alone is pinned, with a line naming every axis, until every context
        // renders as wanted.
        let Some(taken) = taken else {
            return Planned::Colour {
                index,
                ops: token_ops,
                why,
                dial: candidate,
            };
        };
        // The design has it already (this file accepted before).
        if taken.unchanged {
            return Planned::Cleared;
        }
        let folding = taken.folding;
        let Some(checked) = folding.tokens.get(index) else {
            return Planned::Done;
        };
        let mut off: Vec<(&Modes, &[String; 2])> = Vec::new();
        for (modes, sides) in &tuples {
            let pins: Vec<(&str, &str)> = modes
                .iter()
                .map(|(a, v)| (a.as_str(), v.as_str()))
                .collect();
            let Some(folded) = folding
                .tuple(&pins)
                .and_then(|t| checked.values.get(t))
                .and_then(crate::Value::color)
            else {
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
            return Planned::Done;
        }
        if taken.rounds >= 3 {
            return refuse("", "no lines this design takes give every context its value (a light side that differs between tuples an axis under scheme=dark tells apart)");
        }
        let mut pins = Vec::new();
        for (modes, sides) in off {
            match line(modes, sides, true) {
                Ok(op) => pins.push(op),
                Err((block, w)) => return Planned::Report(block, w),
            }
        }
        Planned::Pins(pins)
    };
    // Each token planned, then the proposal judged whole in rounds: its lines
    // applied together and checked once, each token's values read from that
    // folding (its pins added for the next), a refused set halved to the
    // tokens the design refuses (`export_batch`).
    let mut planned: Vec<Planned> = names.iter().map(|n| plan(n, None)).collect();
    let alone =
        crate::export_batch::rounds(&names, &mut planned, &cur, current, vocab, |name, taken| {
            plan(name, Some(taken))
        });
    let alone: std::collections::HashSet<usize> = alone.into_iter().collect();
    // Each candidate's place: its index, and where its own report would go
    // among the others (a report that leaves it out of its dial's change, as
    // it was said before the dial was decided first).
    // (by the candidate's token, as the design spells it, which `dials` asks
    // by: an export's name can differ, a role override's, Astra's check)
    let mut place: HashMap<String, (usize, usize, String)> = HashMap::new();
    for (i, (name, p)) in names.iter().zip(&planned).enumerate() {
        match p {
            Planned::Colour { dial: Some(c), .. } if alone.contains(&i) => {
                place.insert(c.token().to_string(), (i, unmapped.len(), name.clone()));
                dialed.push(c.clone());
            }
            Planned::Report(block, why) => report(&mut unmapped, name, block, why.clone()),
            Planned::Unchanged => accepted.push((name.clone(), Vec::new())),
            Planned::Kind(_, ops) => accepted.push((name.clone(), ops.clone())),
            Planned::Colour { ops, why, dial, .. } => match (dial, why) {
                (Some(c), why) => {
                    // (a candidate a round settled: its literal known)
                    place.insert(c.token().to_string(), (i, unmapped.len(), name.clone()));
                    let _ = (ops, why);
                    dialed.push(c.clone());
                }
                (None, Some((block, w))) => report(&mut unmapped, name, block, w.clone()),
                (None, None) => accepted.push((name.clone(), ops.clone())),
            },
            Planned::Cleared | Planned::Done | Planned::Pins(_) => {
                unreachable!("a round's answer is folded into its plan")
            }
        }
    }
    // Past the budget, nothing more is planned: refused in its words.
    if let Some(why) = crate::budget::spent() {
        return Err(refuse(why));
    }
    // A changed value whose line is one dial alone is that dial's change
    // (D14; stage 10b), first in the transaction: decided on the candidates'
    // values alone, each candidate's literal settled (one check, charged)
    // only if its dial falls back (theme2's ruling on the counts' round 2).
    let mut literal = |token: &str| {
        use crate::export_dials::Fallback;
        let i = place[token].0;
        if alone.contains(&i) && crate::budget::spent().is_none() {
            crate::export_batch::settle_alone(
                i,
                &names,
                &mut planned,
                &cur,
                current,
                vocab,
                |name, taken| plan(name, Some(taken)),
            );
        }
        match &planned[i] {
            Planned::Colour { ops, why: None, .. } | Planned::Kind(_, ops) => {
                Fallback::Literal(Ok(ops.clone()))
            }
            Planned::Colour {
                why: Some(bw),
                dial: Some(_),
                ..
            } => Fallback::Literal(Err(bw.clone())),
            Planned::Colour {
                why: Some((b, w)),
                dial: None,
                ..
            }
            | Planned::Report(b, w) => Fallback::Report(b.clone(), w.clone()),
            Planned::Unchanged => Fallback::Literal(Ok(Vec::new())),
            Planned::Cleared | Planned::Done | Planned::Pins(_) => {
                unreachable!("a round's answer is folded into its plan")
            }
        }
    };
    let dialed = crate::export_dials::dials(&cur, current, dialed, &mut literal, vocab);
    if let Some(why) = crate::budget::spent() {
        return Err(refuse(why));
    }
    // A candidate whose dial fell back and whose literal lines can't be made:
    // reported as its own, where it was said before (in the tokens' order).
    let mut own: Vec<(usize, Unmapped)> = dialed
        .reported
        .iter()
        .map(|(token, block, why)| {
            let (_, at, name) = &place[token.as_str()];
            let mut one = Vec::new();
            report(&mut one, name, block, why.clone());
            (*at, one.pop().expect("one"))
        })
        .collect();
    own.sort_by_key(|(at, _)| *at);
    for (k, (at, u)) in own.into_iter().enumerate() {
        unmapped.insert(at + k, u);
    }
    for (name, block, why) in &dialed.refused {
        report(&mut unmapped, name, block, why.clone());
    }
    // A dial that cannot take its tokens' change: their literals, pinned.
    accepted.extend(dialed.fallback);
    let (mut ops, mut dials) = (dialed.ops, dialed.changes);
    let mut mapped = dialed.kept + dials.iter().map(|d| d.from.len()).sum::<usize>();
    // Together, as one transaction: a token whose operations the design
    // refuses beside the others' (a dial nothing uses any more) is reported,
    // the transaction judged whole and halved to them when refused
    // (`export_batch`), on what the dials' changes made.
    let members: Vec<&[Op]> = accepted
        .iter()
        .map(|(_, o)| o.as_slice())
        .filter(|o| !o.is_empty())
        .collect();
    let judged = crate::edit::stepped_all(current, &ops, vocab)
        .and_then(|base| crate::export_batch::judged(&base, &members, vocab));
    // Past the budget, nothing more is planned (the marks' checks).
    if let Some(why) = crate::budget::spent() {
        return Err(refuse(why));
    }
    let mut refused = match &judged {
        Ok(j) => j
            .refused
            .iter()
            .map(|(k, all)| (*k, all.clone()))
            .collect::<Vec<_>>(),
        Err(all) => (0..members.len()).map(|k| (k, all.clone())).collect(),
    }
    .into_iter()
    .peekable();
    let mut k = 0;
    for (name, token_ops) in accepted {
        if !token_ops.is_empty() {
            k += 1;
            if let Some((_, all)) = refused.next_if(|(r, _)| *r == k - 1) {
                let first = all
                    .first()
                    .map_or(String::new(), |d| format!("{}: {}", d.id, d.message));
                report(
                    &mut unmapped,
                    &name,
                    "",
                    format!("the design would refuse it with the rest of the proposal ({first})"),
                );
                continue;
            }
            ops.extend(token_ops);
        }
        mapped += 1;
    }
    let counted = counted(names.len() + extra.len(), mapped, unmapped.len())?;
    let pins = crate::export_dials::marks(&cur, current, &ops, &mut dials, vocab, &under, &branch);
    Ok(Import {
        ops,
        unmapped,
        counted,
        dials,
        pins,
        ..Import::default()
    })
}

/// The line of token `t` a tuple's side takes (`None` the default), as the
/// checker selects it: the `when` naming the most axes among those that
/// hold; for the light side of one naming an axis `under scheme=dark`, the
/// line without such axes.
pub(crate) fn branch(t: &Token, under: &[&str], modes: &Modes, dark: bool) -> Option<usize> {
    let select = |base: bool| {
        let holding = (0..t.whens.len()).filter(|&i| {
            let w = &t.whens[i];
            !(base && w.conds.iter().any(|(a, _)| under.contains(&a.as_str())))
                && w.conds.iter().all(|c| modes.contains(c))
        });
        let most = holding.clone().map(|i| t.whens[i].conds.len()).max()?;
        holding
            .into_iter()
            .find(|&i| t.whens[i].conds.len() == most)
    };
    let chosen = select(false);
    if !dark
        && chosen.is_some_and(|w| {
            t.whens[w]
                .conds
                .iter()
                .any(|(a, _)| under.contains(&a.as_str()))
        })
    {
        return select(true);
    }
    chosen
}

/// The two sides of a top-level `light-dark(a, b)`, as written.
fn sides_of(text: &str) -> Option<[String; 2]> {
    let inner = text.strip_prefix("light-dark(")?.strip_suffix(')')?;
    let mut depth = 0;
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                return Some([
                    inner[..i].trim().to_string(),
                    inner[i + 1..].trim().to_string(),
                ])
            }
            _ => {}
        }
    }
    None
}
