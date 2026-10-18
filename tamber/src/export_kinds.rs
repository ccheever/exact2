//! The DTCG export's kinds besides colour (LLP 1121 D14, stage 6 G): each
//! token's value per context as DTCG 2025.10 writes its type, what the
//! export loses kept in `$extensions.exact`, and the way back from an
//! incoming value to the text a design writes.
//!
//! - a length: `dimension`, px or `rem`; an `em`, a text style or the
//!   hairline as px at the default root size (16), marked `lossy` with its
//!   source;
//! - a number: `number`;
//! - a family: `fontFamily`, its members without their quotes;
//! - a type: `typography`, every property written: what the token leaves
//!   unsaid (the style's weight, its leading, the family) as the default the
//!   web column draws, named in `unsaid`;
//! - a shadow: `shadow`, a text shadow marked so;
//! - a duration: `duration` in ms;
//! - an easing: `cubicBezier`; a keyword as its curve, a `steps()`, a
//!   `linear()` or a spring as `linear`'s, marked `lossy`.

use crate::{Length, Value, Vocab};
use serde_json::{json, Map, Value as Json};

/// A token's incoming name, value and its source tuple.
type Incoming = (String, Json, Json);

/// The root size the export writes a relative length at, CSS's `medium`.
pub(crate) const ROOT_PX: f64 = 16.0;

/// A value as the export writes it in a context: its `$type`, its `$value`,
/// and what `$extensions.exact` keeps of it.
pub(crate) struct Written {
    pub ty: &'static str,
    pub value: Json,
    pub exact: Map<String, Json>,
}

fn dimension(px: f64, unit: &str) -> Json {
    json!({ "value": px, "unit": unit })
}

/// A named length's px at the default root size, by the vocabulary.
fn named_px(name: &str, vocab: &Vocab) -> Option<f64> {
    vocab
        .sizes
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .and_then(|(_, px)| px.parse().ok())
}

/// A length as a DTCG dimension, and the source it loses, if any.
fn length(l: &Length, vocab: &Vocab) -> (Json, Option<String>) {
    match l {
        Length::Px(n) => (dimension(*n, "px"), None),
        Length::Rem(n) => (dimension(*n, "rem"), None),
        Length::Em(n) => (dimension(n * ROOT_PX, "px"), Some(l.css())),
        Length::Named(name) => (
            dimension(named_px(name, vocab).unwrap_or(1.0), "px"),
            Some(name.clone()),
        ),
    }
}

/// A shadow list as DTCG's `shadow` (one object, or several).
fn shadows(s: &crate::Shadows, dark: bool, vocab: &Vocab) -> Option<Json> {
    let mut list = Vec::new();
    for one in &s.list {
        let color = match &one.color {
            Some(c) => crate::export::colour_side(c, dark, vocab)?,
            // `currentcolor`: black, marked by the token's `currentcolor`.
            None => json!({ "colorSpace": "srgb", "components": [0, 0, 0], "hex": "#000000" }),
        };
        let at = |i: usize| one.lengths.get(i).copied().unwrap_or(0.0);
        let mut obj = Map::new();
        obj.insert("color".into(), color);
        obj.insert("offsetX".into(), dimension(at(0), "px"));
        obj.insert("offsetY".into(), dimension(at(1), "px"));
        obj.insert("blur".into(), dimension(at(2), "px"));
        obj.insert("spread".into(), dimension(at(3), "px"));
        if one.inset {
            obj.insert("inset".into(), json!(true));
        }
        list.push(Json::Object(obj));
    }
    Some(match list.len() {
        1 => list.remove(0),
        _ => Json::Array(list),
    })
}

/// The curve an easing keyword is, CSS's.
fn keyword_curve(e: &str) -> Option<[f64; 4]> {
    Some(match e {
        "linear" => [0.0, 0.0, 1.0, 1.0],
        "ease" => [0.25, 0.1, 0.25, 1.0],
        "ease-in" => [0.42, 0.0, 1.0, 1.0],
        "ease-out" => [0.0, 0.0, 0.58, 1.0],
        "ease-in-out" => [0.42, 0.0, 0.58, 1.0],
        _ => return None,
    })
}

/// A `cubic-bezier(…)`'s four numbers.
fn bezier(e: &str) -> Option<[f64; 4]> {
    let inner = e.trim().strip_prefix("cubic-bezier(")?.strip_suffix(')')?;
    let n: Vec<f64> = inner
        .split(',')
        .map(|p| exact_num::parse_f64(p.trim()).ok())
        .collect::<Option<_>>()?;
    <[f64; 4]>::try_from(n).ok()
}

/// A family stack's members without their quotes.
fn members(stack: &str) -> Vec<String> {
    let (mut out, mut start, mut quote) = (Vec::new(), 0, None);
    for (i, c) in stack.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, ',') => {
                out.push(&stack[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&stack[start..]);
    out.into_iter()
        .map(|m| m.trim().trim_matches(['"', '\'']).to_string())
        .collect()
}

/// `members` as one string or a list, as DTCG's `fontFamily` takes them.
fn family_json(stack: &str) -> Json {
    let m = members(stack);
    match m.len() {
        1 => json!(m[0]),
        _ => json!(m),
    }
}

/// A value of a kind besides colour as the export writes it in a context
/// on side `dark`; `None` for a colour, or one with no side the export
/// reads (a shadow's role with no fallback).
pub(crate) fn written(v: &Value, dark: bool, vocab: &Vocab) -> Option<Written> {
    let mut exact = Map::new();
    let (ty, value) = match v {
        Value::Color(_) => return None,
        Value::Length(l) => {
            let (value, lost) = length(l, vocab);
            if let Some(source) = lost {
                exact.insert("lossy".into(), json!(source));
            }
            ("dimension", value)
        }
        Value::Number(n) => ("number", json!(n)),
        Value::Family(f) => ("fontFamily", family_json(f)),
        Value::Type(t) => {
            let mut unsaid = Vec::new();
            let (size, lost) = length(&t.size, vocab);
            if let Some(source) = lost {
                exact.insert("lossy".into(), json!(source));
            }
            let weight = t.weight.unwrap_or_else(|| {
                unsaid.push("fontWeight");
                400
            });
            let line = match t.leading.as_deref() {
                Some(l) => match exact_num::parse_f64(l) {
                    Ok(n) => json!(n),
                    // A length or `normal`: no number DTCG's `lineHeight`
                    // takes; its source kept.
                    Err(_) => {
                        exact.insert("leading".into(), json!(l));
                        json!(1.2)
                    }
                },
                None => {
                    unsaid.push("lineHeight");
                    json!(1.2)
                }
            };
            let family = match &t.family {
                Some(f) => family_json(f),
                None => {
                    unsaid.push("fontFamily");
                    json!("system-ui")
                }
            };
            if t.style.is_some() {
                exact.insert("fontStyle".into(), json!(t.style));
            }
            if !unsaid.is_empty() {
                exact.insert("unsaid".into(), json!(unsaid));
            }
            (
                "typography",
                json!({
                    "fontFamily": family,
                    "fontSize": size,
                    "fontWeight": weight,
                    "letterSpacing": dimension(0.0, "px"),
                    "lineHeight": line,
                }),
            )
        }
        Value::Shadow(s) => {
            if s.text {
                exact.insert("kind".into(), json!("text-shadow"));
            }
            if s.list.iter().any(|one| one.color.is_none()) {
                exact.insert("currentcolor".into(), json!(true));
            }
            if s.list.is_empty() {
                exact.insert("lossy".into(), json!("none"));
            }
            ("shadow", shadows(s, dark, vocab)?)
        }
        Value::Duration(ms) => ("duration", json!({ "value": ms, "unit": "ms" })),
        Value::Easing(e) => {
            let curve = keyword_curve(e).or_else(|| bezier(e)).unwrap_or_else(|| {
                exact.insert("lossy".into(), json!(e));
                [0.0, 0.0, 1.0, 1.0]
            });
            ("cubicBezier", json!(curve))
        }
    };
    Some(Written { ty, value, exact })
}

/// A number as a design writes it.
fn num(n: f64) -> String {
    crate::measure::num(n)
}

/// A DTCG dimension as a design writes a length.
fn read_dimension(v: &Json) -> Option<String> {
    let value = v["value"].as_f64()?;
    match v["unit"].as_str()? {
        "px" => Some(format!("{}px", num(value))),
        "rem" => Some(format!("{}rem", num(value))),
        _ => None,
    }
}

/// A family member as a stack writes it: identifiers bare, else quoted.
fn member_text(m: &str) -> String {
    let bare = !m.is_empty()
        && m.split_whitespace().all(|w| {
            w.chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                && !w.starts_with(|c: char| c.is_ascii_digit())
        });
    match bare {
        true => m.to_string(),
        false => format!("\"{m}\""),
    }
}

fn read_family(v: &Json) -> Option<String> {
    let list: Vec<&str> = match v {
        Json::String(s) => vec![s.as_str()],
        Json::Array(a) => a.iter().map(Json::as_str).collect::<Option<_>>()?,
        _ => return None,
    };
    Some(
        list.into_iter()
            .map(member_text)
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// An incoming DTCG value of `ty` as the text a design writes for a token
/// of that kind; the baseline's value and `exact` extensions say what the
/// export left unsaid or lost (a type's size kept as its text style where
/// it did not change). `None` when the value is no such type's.
pub(crate) fn read(ty: &str, v: &Json, base: &Json, exact: &Json) -> Option<String> {
    match ty {
        "dimension" => read_dimension(v),
        "number" => Some(num(v.as_f64()?)),
        "fontFamily" => read_family(v),
        "duration" => {
            let value = v["value"].as_f64()?;
            match v["unit"].as_str()? {
                "ms" => Some(format!("{}ms", num(value))),
                "s" => Some(format!("{}ms", num(value * 1000.0))),
                _ => None,
            }
        }
        "cubicBezier" => {
            let n: Vec<f64> = v
                .as_array()?
                .iter()
                .map(Json::as_f64)
                .collect::<Option<_>>()?;
            let [a, b, c, d] = <[f64; 4]>::try_from(n).ok()?;
            Some(format!(
                "cubic-bezier({}, {}, {}, {})",
                num(a),
                num(b),
                num(c),
                num(d)
            ))
        }
        "typography" => {
            let unsaid: Vec<&str> = exact["unsaid"]
                .as_array()
                .map(|a| a.iter().filter_map(Json::as_str).collect())
                .unwrap_or_default();
            // Unsaid in the export, unless the file moved it: then it is said.
            let said = |p: &str| !unsaid.contains(&p) || !alike(&v[p], &base[p]);
            let mut out = Vec::new();
            if let Some(style) = exact["fontStyle"].as_str() {
                out.push(style.to_string());
            }
            if said("fontWeight") {
                out.push(num(v["fontWeight"].as_f64()?));
            }
            // A source the export lost (a text style, a leading in a unit)
            // where the file did not move its placeholder; its own value
            // where it did.
            let size = match exact["lossy"].as_str() {
                Some(source) if alike(&v["fontSize"], &base["fontSize"]) => source.to_string(),
                _ => read_dimension(&v["fontSize"])?,
            };
            let kept = alike(&v["lineHeight"], &base["lineHeight"]);
            out.push(match (said("lineHeight"), exact["leading"].as_str()) {
                (false, _) => size,
                (true, Some(source)) if kept => format!("{size}/{source}"),
                (true, _) => format!("{size}/{}", num(v["lineHeight"].as_f64()?)),
            });
            if said("fontFamily") {
                out.push(read_family(&v["fontFamily"])?);
            }
            Some(out.join(" "))
        }
        "shadow" => {
            let list = match v {
                Json::Array(a) => a.clone(),
                one => vec![one.clone()],
            };
            let text = exact["kind"] == "text-shadow";
            let bases = match base {
                Json::Array(a) => a.clone(),
                one => vec![one.clone()],
            };
            let mut parts = Vec::new();
            for (i, s) in list.iter().enumerate() {
                let px = |k: &str| s[k]["value"].as_f64().filter(|_| s[k]["unit"] == "px");
                let mut words = Vec::new();
                if s["inset"] == true {
                    words.push("inset".to_string());
                }
                let lengths = [px("offsetX")?, px("offsetY")?, px("blur")?, px("spread")?];
                let keep = if text { 3 } else { 4 };
                words.extend(lengths[..keep].iter().map(|n| format!("{}px", num(*n))));
                // `currentcolor` stays, unless the file gave the shadow a colour.
                let current = exact["currentcolor"] == true
                    && bases
                        .get(i)
                        .is_some_and(|b| alike(&b["color"], &s["color"]));
                if !current {
                    words.push(crate::import::dtcg::colour(&s["color"])?);
                }
                parts.push(words.join(" "));
            }
            Some(parts.join(", "))
        }
        _ => None,
    }
}

/// A tuple: every axis and its value.
type Modes = Vec<(String, String)>;

/// What re-importing a token of a kind besides colour proposes: each
/// changed tuple's incoming value as the design writes it (one value in
/// light and dark: a length has no scheme), on the line that wins there
/// when every tuple it wins in moved to that value, else on a `when`
/// naming the tuple's axes (none under scheme=dark, which only a colour
/// varies by). Checked by applying and folding: every tuple must export as
/// the file has it. A refusal names the context and why.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reimport_ops(
    decl: &crate::Token,
    written: &str,
    tuples: &[(Modes, [String; 2])],
    under: &[&str],
    changed: &dyn Fn(&str) -> bool,
    incoming: &dyn Fn(&str) -> Option<Incoming>,
    baseline: &dyn Fn(&str) -> (Json, Json),
    branch: &dyn Fn(&Modes) -> Option<usize>,
    current: &str,
    vocab: &Vocab,
) -> Result<Vec<crate::Op>, (String, String)> {
    let kind = |s: &str| {
        (
            s.to_string(),
            format!(
                "a value this importer reads no {} from",
                decl.kind.as_deref().unwrap_or("token")
            ),
        )
    };
    // Each tuple's wanted text, or none where it did not change.
    let mut wants: Vec<(usize, String)> = Vec::new();
    for (i, (_, sides)) in tuples.iter().enumerate() {
        if !changed(&sides[0]) && !changed(&sides[1]) {
            continue;
        }
        let (Some((ty, light, theirs)), Some((_, dark, _))) =
            (incoming(&sides[0]), incoming(&sides[1]))
        else {
            return Err(kind(&sides[0]));
        };
        // What the export kept beside the value: the file's where a tool
        // kept `$extensions` (a style deleted there is gone), else the
        // baseline's.
        let (base, kept) = baseline(&sides[0]);
        let exact = if theirs.is_object() { theirs } else { kept };
        let text = if alike(&light, &dark) {
            read(&ty, &light, &base, &exact)
        } else if ty == "shadow" {
            // A shadow's colour may differ by scheme: one value, its colours
            // `light-dark()` pairs.
            shadow_sides(&light, &dark, &exact)
        } else {
            return Err((
                sides[1].clone(),
                "a token of this kind has one value in light and dark".into(),
            ));
        };
        let text = text.ok_or_else(|| kind(&sides[0]))?;
        wants.push((i, text));
    }
    let want = |i: usize| wants.iter().find(|(j, _)| *j == i).map(|(_, t)| t);
    let mut ops: Vec<crate::Op> = Vec::new();
    let mut set = |when: Modes, value: String| {
        if !ops
            .iter()
            .any(|op| matches!(op, crate::Op::SetToken { when: w, .. } if *w == when))
        {
            ops.push(crate::Op::SetToken {
                name: written.to_string(),
                when,
                value,
                at: None,
                comment: None,
            });
        }
    };
    let mut lines: Vec<(Option<usize>, Vec<usize>)> = Vec::new();
    for (i, (modes, _)) in tuples.iter().enumerate() {
        let b = branch(modes);
        match lines.iter_mut().find(|(x, _)| *x == b) {
            Some((_, all)) => all.push(i),
            None => lines.push((b, vec![i])),
        }
    }
    for (b, members) in &lines {
        let moved: Vec<&String> = members.iter().filter_map(|&i| want(i)).collect();
        if moved.is_empty() {
            continue;
        }
        if moved.len() == members.len() && moved.iter().all(|t| *t == moved[0]) {
            set(
                b.map_or(Vec::new(), |w| decl.whens[w].conds.clone()),
                moved[0].clone(),
            );
            continue;
        }
        for &i in members {
            if let Some(text) = want(i) {
                let when: Modes = tuples[i]
                    .0
                    .iter()
                    .filter(|(a, _)| !under.contains(&a.as_str()))
                    .cloned()
                    .collect();
                set(when, text.clone());
            }
        }
    }
    // As the design takes them: every changed tuple reads back as the file
    // has it (by its text, so `0.2s` is `200ms`); one the file left alone
    // keeps what the design has now. Already the design's: nothing to do.
    let refused = |m: String| (String::new(), m);
    let edited = crate::apply_all(current, &ops, vocab).map_err(|all| {
        refused(format!(
            "the design would refuse it ({})",
            all.first()
                .map_or(String::new(), |d| format!("{}: {}", d.id, d.message))
        ))
    })?;
    let folding = crate::check(&edited.text, vocab).map_err(|all| {
        refused(format!(
            "the design would refuse it ({})",
            all.first()
                .map_or(String::new(), |d| format!("{}: {}", d.id, d.message))
        ))
    })?;
    let checked = folding
        .tokens
        .iter()
        .zip(&folding.design.tokens)
        .find(|(_, t)| t.name.value == written)
        .map(|(c, _)| c)
        .ok_or_else(|| refused("the token is not in the design any more".into()))?;
    if edited.text == current {
        return Ok(Vec::new());
    }
    // A tuple the file left alone keeps the design's value: a line the
    // edit needs that reaches it too (an axis under scheme=dark, which a
    // `when` of this kind cannot name) is no edit this design takes.
    let before = crate::check(current, vocab).ok();
    let was = |pins: &[(&str, &str)]| {
        let f = before.as_ref()?;
        let t = f
            .tokens
            .iter()
            .zip(&f.design.tokens)
            .find(|(_, t)| t.name.value == written)?
            .0;
        t.values.get(f.tuple(pins)?).map(Value::css)
    };
    for (i, (modes, sides)) in tuples.iter().enumerate() {
        let pins: Vec<(&str, &str)> = modes
            .iter()
            .map(|(a, v)| (a.as_str(), v.as_str()))
            .collect();
        let Some(value) = folding.tuple(&pins).and_then(|t| checked.values.get(t)) else {
            continue;
        };
        let target = match want(i) {
            Some(want) => canonical(want, value, vocab),
            None => match was(&pins) {
                Some(css) => css,
                None => continue,
            },
        };
        if value.css() != target {
            return Err((
                sides[0].clone(),
                "no lines this design takes give every context its value (a token of this kind varies by no axis under scheme=dark)".into(),
            ));
        }
    }
    Ok(ops)
}

/// Two DTCG values alike: numbers by their value (`6` is `6.0`), the rest
/// exactly.
pub(crate) fn alike(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(x), Json::Number(y)) => x.as_f64() == y.as_f64(),
        (Json::Array(x), Json::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| alike(p, q))
        }
        (Json::Object(x), Json::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| alike(v, w)))
        }
        _ => a == b,
    }
}

/// Text `want` folded as `like`'s kind, as the design would hold it (`0.2s`
/// is `200ms`); `want` itself where it does not fold alone (it names a
/// token, which the design's fold resolves).
fn canonical(want: &str, like: &Value, vocab: &Vocab) -> String {
    let src = format!("tamber w\n\ntheme\n  w: {} = {want}\n", like.kind().name());
    crate::check(&src, vocab)
        .ok()
        .and_then(|f| {
            f.tokens
                .first()
                .and_then(|t| t.values.first())
                .map(Value::css)
        })
        .unwrap_or_else(|| want.to_string())
}

/// A shadow whose light and dark sides differ only in their colours, as
/// one value with each colour a `light-dark()` pair; `None` otherwise.
fn shadow_sides(light: &Json, dark: &Json, exact: &Json) -> Option<String> {
    let list = |v: &Json| match v {
        Json::Array(a) => a.clone(),
        one => vec![one.clone()],
    };
    let (l, d) = (list(light), list(dark));
    if l.len() != d.len() || exact["currentcolor"] == true {
        return None;
    }
    let text = exact["kind"] == "text-shadow";
    let mut parts = Vec::new();
    for (a, b) in l.iter().zip(&d) {
        let mut words = Vec::new();
        if a["inset"] != b["inset"] {
            return None;
        }
        if a["inset"] == true {
            words.push("inset".to_string());
        }
        let keys: &[&str] = if text {
            &["offsetX", "offsetY", "blur"]
        } else {
            &["offsetX", "offsetY", "blur", "spread"]
        };
        for k in keys {
            if !alike(&a[*k], &b[*k]) || a[*k]["unit"] != "px" {
                return None;
            }
            words.push(format!("{}px", num(a[*k]["value"].as_f64()?)));
        }
        let cl = crate::import::dtcg::colour(&a["color"])?;
        let cd = crate::import::dtcg::colour(&b["color"])?;
        words.push(if cl == cd {
            cl
        } else {
            format!("light-dark({cl}, {cd})")
        });
        parts.push(words.join(" "));
    }
    Some(parts.join(", "))
}
