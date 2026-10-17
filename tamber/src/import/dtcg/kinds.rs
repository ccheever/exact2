//! DTCG's `$type`s besides colour as Tamber's kinds (LLP 1121 D14, stage 6
//! H): a token's value in a tuple, its aliases followed (inside a composite
//! value too), as a design writes a token of its kind. A value of these
//! kinds is written out, not as a reference to the token it aliases.
//!
//! - `dimension` (2025.10's `{value, unit}`), and Tokens Studio's
//!   `borderRadius`, `spacing`, `sizing`, `borderWidth`, `fontSizes`,
//!   `letterSpacing` and `paragraphSpacing`: a length, a bare number in px;
//! - `fontFamily`, `fontFamilies`: a family, a list its members;
//! - `fontWeight` (a number, or a weight's name), `number`, `lineHeights` (a
//!   percentage a ratio), `opacity`: a number;
//! - `duration`: a duration; `cubicBezier`: an easing;
//! - `shadow`, `boxShadow` (an `innerShadow` inset): a box shadow;
//! - `typography`: a type token, its weight, size, leading and family; a
//!   letter spacing, paragraph spacing, case or decoration that says
//!   anything is refused, since a type token holds none.

use super::{alias, colour, Resolve};
use crate::measure::num;
use serde_json::Value;
use std::collections::BTreeSet;

impl Resolve<'_> {
    /// A path's value in this tuple: its aliases followed, and each alias
    /// inside it replaced by what it names.
    pub(super) fn raw(&self, path: &str, visiting: &mut BTreeSet<String>) -> Result<Value, String> {
        let Some(&i) = self.w.get(path) else {
            return Err(format!("`{{{path}}}` is no token in this mode"));
        };
        let d = &self.defs[i];
        if let Some(why) = &d.refused {
            return Err(why.clone());
        }
        if visiting.len() > 256 {
            return Err("an alias chain longer than 256".into());
        }
        match alias(&d.value) {
            Some(Ok(target)) if !visiting.insert(target.clone()) => {
                Err(format!("a cycle of aliases through `{{{target}}}`"))
            }
            Some(Ok(target)) => {
                let found = self.raw(&target, visiting);
                visiting.remove(&target);
                found
            }
            Some(Err(why)) => Err(why),
            None => self.inner(&d.value, visiting),
        }
    }

    /// `v` with each alias inside it replaced by what it names.
    fn inner(&self, v: &Value, visiting: &mut BTreeSet<String>) -> Result<Value, String> {
        match v {
            Value::Object(o) => o
                .iter()
                .map(|(k, x)| Ok((k.clone(), self.leaf(x, visiting)?)))
                .collect::<Result<_, String>>()
                .map(Value::Object),
            Value::Array(a) => a
                .iter()
                .map(|x| self.leaf(x, visiting))
                .collect::<Result<_, _>>()
                .map(Value::Array),
            v => Ok(v.clone()),
        }
    }

    fn leaf(&self, x: &Value, visiting: &mut BTreeSet<String>) -> Result<Value, String> {
        match alias(x) {
            Some(Ok(target)) if !visiting.insert(target.clone()) => {
                Err(format!("a cycle of aliases through `{{{target}}}`"))
            }
            Some(Ok(target)) => {
                let found = self.raw(&target, visiting);
                visiting.remove(&target);
                found
            }
            Some(Err(why)) => Err(why),
            None => self.inner(x, visiting),
        }
    }
}

/// A length: 2025.10's `{value, unit}`, or a number (px) or text.
fn length(v: &Value) -> Result<String, String> {
    match v {
        Value::Object(o) => {
            let n = o.get("value").and_then(Value::as_f64);
            let unit = o.get("unit").and_then(Value::as_str);
            match (n, unit) {
                (Some(n), Some(u)) => Ok(format!("{}{u}", num(n))),
                _ => Err(format!("`{v}` is no dimension")),
            }
        }
        Value::Number(n) => Ok(format!("{}px", num(n.as_f64().unwrap_or_default()))),
        Value::String(s) => match s.trim().parse::<f64>() {
            Ok(n) => Ok(format!("{}px", num(n))),
            Err(_) => Ok(s.trim().to_string()),
        },
        _ => Err(format!("`{v}` is no dimension")),
    }
}

/// A number, or a number's text; a weight by its name.
fn number(ty: &str, v: &Value) -> Result<String, String> {
    if let Some(n) = v.as_f64() {
        return Ok(num(n));
    }
    let s = v
        .as_str()
        .map(str::trim)
        .ok_or(format!("`{v}` is no number"))?;
    if let Ok(n) = s.parse::<f64>() {
        return Ok(num(n));
    }
    if let Some(p) = s
        .strip_suffix('%')
        .and_then(|p| p.trim().parse::<f64>().ok())
    {
        if ty == "lineHeights" || ty == "lineHeight" {
            return Ok(num(p / 100.0));
        }
    }
    let squashed: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    let named = match squashed.as_str() {
        "thin" | "hairline" => 100,
        "extralight" | "ultralight" => 200,
        "light" => 300,
        "regular" | "normal" => 400,
        "medium" => 500,
        "semibold" | "demibold" => 600,
        "bold" => 700,
        "extrabold" | "ultrabold" => 800,
        "black" | "heavy" => 900,
        _ => return Err(format!("`{s}` is no number")),
    };
    match ty.starts_with("fontWeight") {
        true => Ok(named.to_string()),
        false => Err(format!("`{s}` is no number")),
    }
}

/// A family: a name, a stack's text, or a list of names.
fn family(v: &Value) -> Result<String, String> {
    let member = |m: &str| {
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
    };
    match v {
        // A stack written as text stays as written.
        Value::String(s) if s.contains(',') => Ok(s.trim().to_string()),
        Value::String(s) => Ok(member(s.trim())),
        Value::Array(a) => a
            .iter()
            .map(|m| m.as_str().map(|m| member(m.trim())))
            .collect::<Option<Vec<_>>>()
            .map(|m| m.join(", "))
            .ok_or(format!("`{v}` is no list of family names")),
        _ => Err(format!("`{v}` is no family")),
    }
}

/// One shadow: its offsets, blur, spread and colour.
fn shadow(s: &Value) -> Result<String, String> {
    let o = s.as_object().ok_or(format!("`{s}` is no shadow"))?;
    let mut words = Vec::new();
    if o.get("inset") == Some(&Value::Bool(true))
        || o.get("type").and_then(Value::as_str) == Some("innerShadow")
    {
        words.push("inset".to_string());
    }
    for keys in [
        ["offsetX", "x"],
        ["offsetY", "y"],
        ["blur", "blur"],
        ["spread", "spread"],
    ] {
        match keys.iter().find_map(|k| o.get(*k)) {
            Some(v) => words.push(length(v)?),
            None if keys[0] == "spread" => {}
            None => return Err(format!("a shadow with no `{}`", keys[0])),
        }
    }
    let c = o.get("color").ok_or("a shadow with no `color`")?;
    words.push(colour(c).ok_or(format!("`{c}` is no colour Tamber reads"))?);
    Ok(words.join(" "))
}

/// A `typography` value as a type token.
fn typography(v: &Value) -> Result<String, String> {
    let o = v.as_object().ok_or(format!("`{v}` is no typography"))?;
    // What a type token holds none of, where it says anything.
    for (k, quiet) in [
        ("letterSpacing", &["0", "0%", "0px", "0em", "normal"][..]),
        ("paragraphSpacing", &["0", "0px"][..]),
        ("paragraphIndent", &["0", "0px"][..]),
        ("textCase", &["none"][..]),
        ("textDecoration", &["none"][..]),
    ] {
        let Some(x) = o.get(k) else { continue };
        let said = match x {
            Value::Number(n) => n.as_f64() != Some(0.0),
            Value::String(s) => !quiet.contains(&s.trim()),
            Value::Object(d) => d.get("value").and_then(Value::as_f64) != Some(0.0),
            _ => true,
        };
        if said {
            return Err(format!("its `{k}` is `{x}`, and a type token holds none"));
        }
    }
    let mut out = Vec::new();
    match o.get("fontStyle").and_then(Value::as_str).map(str::trim) {
        Some("italic") => out.push("italic".to_string()),
        Some("normal") | None => {}
        Some(s) => {
            return Err(format!(
                "its `fontStyle` is `{s}`: a type token is upright or italic"
            ))
        }
    }
    if let Some(w) = o.get("fontWeight") {
        out.push(number("fontWeight", w)?);
    }
    let size = length(o.get("fontSize").ok_or("a typography with no `fontSize`")?)?;
    out.push(match o.get("lineHeight") {
        None => size,
        Some(l @ Value::Object(_)) => format!("{size}/{}", length(l)?),
        Some(l) => format!("{size}/{}", number("lineHeight", l)?),
    });
    if let Some(f) = o.get("fontFamily") {
        out.push(family(f)?);
    }
    Ok(out.join(" "))
}

/// A value of `$type` `ty`, its aliases replaced, as a design writes a
/// token of its kind; or why not.
pub(super) fn text(ty: &str, v: &Value) -> Result<String, String> {
    match ty {
        "dimension" | "borderRadius" | "spacing" | "sizing" | "borderWidth" | "fontSizes"
        | "letterSpacing" | "paragraphSpacing" => length(v),
        "fontFamily" | "fontFamilies" => family(v),
        "fontWeight" | "fontWeights" | "number" | "lineHeights" | "opacity" => number(ty, v),
        "duration" => match v {
            Value::Object(o) => {
                let n = o.get("value").and_then(Value::as_f64);
                match (n, o.get("unit").and_then(Value::as_str)) {
                    (Some(n), Some("ms")) => Ok(format!("{}ms", num(n))),
                    (Some(n), Some("s")) => Ok(format!("{}ms", num(n * 1000.0))),
                    _ => Err(format!("`{v}` is no duration")),
                }
            }
            Value::String(s) => Ok(s.trim().to_string()),
            _ => Err(format!("`{v}` is no duration")),
        },
        "cubicBezier" => match v {
            Value::Array(a) if a.len() == 4 => a
                .iter()
                .map(|n| n.as_f64().map(num))
                .collect::<Option<Vec<_>>>()
                .map(|n| format!("cubic-bezier({})", n.join(", ")))
                .ok_or(format!("`{v}` is no curve")),
            Value::String(s) => Ok(s.trim().to_string()),
            _ => Err(format!("`{v}` is no curve")),
        },
        "shadow" | "boxShadow" => match v {
            Value::Array(a) => a
                .iter()
                .map(shadow)
                .collect::<Result<Vec<_>, _>>()
                .map(|s| s.join(", ")),
            one => shadow(one),
        },
        "typography" => typography(v),
        _ => Err(format!("a `{ty}` Tamber's DTCG mapping does not name")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_type_as_a_design_writes_it() {
        assert_eq!(
            text("dimension", &json!({"value": 4, "unit": "px"})),
            Ok("4px".into())
        );
        assert_eq!(text("borderRadius", &json!("8")), Ok("8px".into()));
        assert_eq!(text("fontWeights", &json!("Semi Bold")), Ok("600".into()));
        assert_eq!(text("lineHeights", &json!("150%")), Ok("1.5".into()));
        assert_eq!(
            text("fontFamily", &json!(["Inter", "system-ui"])),
            Ok("Inter, system-ui".into())
        );
        assert_eq!(
            text("cubicBezier", &json!([0.4, 0, 0.2, 1])),
            Ok("cubic-bezier(0.4, 0, 0.2, 1)".into())
        );
        assert_eq!(
            text(
                "shadow",
                &json!({"color": "#000", "offsetX": {"value": 0, "unit": "px"}, "offsetY": {"value": 1, "unit": "px"}, "blur": {"value": 2, "unit": "px"}, "spread": {"value": 0, "unit": "px"}, "inset": true})
            ),
            Ok("inset 0px 1px 2px 0px #000".into())
        );
        assert_eq!(
            text(
                "typography",
                &json!({"fontFamily": "Inter", "fontSize": {"value": 1, "unit": "rem"}, "fontWeight": 600, "lineHeight": 1.5, "letterSpacing": {"value": 0, "unit": "px"}})
            ),
            Ok("600 1rem/1.5 Inter".into())
        );
        assert!(text(
            "typography",
            &json!({"fontSize": "16px", "letterSpacing": "2%"})
        )
        .is_err());
    }
}
