//! The wire's operations (`tamber/1`, LLP 1121 D13): each `ops` entry read
//! into the crate's [`Op`], with the file it edits.

use crate::tamber_serve::{optional, required};
use exact_tamber::Op;
use serde_json::{json, Value};

/// The wire's operations (D13's names, kebab-case; named fields), each with
/// the file it edits (the design's own when unsaid). A `restore` is no
/// client's to send.
pub(crate) fn parse_ops(ops: &Value, design: &str) -> Result<Vec<(String, Op)>, Value> {
    let bad = |m: String| json!({"message": m});
    let s = required;
    let opt = optional;
    let list = |v: &Value, k: &str| -> Result<Vec<Value>, Value> {
        match &v[k] {
            Value::Null => Ok(Vec::new()),
            Value::Array(a) => Ok(a.clone()),
            _ => Err(bad(format!("`{k}` is a list"))),
        }
    };
    let conds = |v: &Value| -> Result<Vec<(String, String)>, Value> {
        list(v, "when")?
            .iter()
            .map(|c| Ok((s(c, "axis")?, s(c, "value")?)))
            .collect()
    };
    let mut out = Vec::new();
    for v in ops
        .as_array()
        .ok_or_else(|| bad("`ops` is a list".into()))?
    {
        if !v.is_object() {
            return Err(bad("each of `ops` is an object".into()));
        }
        let file = opt(v, "file")?.unwrap_or_else(|| design.to_string());
        let op = match v["op"].as_str() {
            Some("set-dial") => Op::SetDial {
                name: s(v, "name")?,
                value: s(v, "value")?,
            },
            Some("set-token") => Op::SetToken {
                name: s(v, "name")?,
                when: conds(v)?,
                value: s(v, "value")?,
                at: None,
                comment: opt(v, "comment")?,
            },
            Some("unset-token-when") => Op::UnsetTokenWhen {
                name: s(v, "name")?,
                when: conds(v)?,
            },
            Some("add-token") => Op::AddToken {
                name: s(v, "name")?,
                kind: opt(v, "kind")?,
                value: s(v, "value")?,
                // `key=value` words, as the source writes them.
                meta: list(v, "meta")?
                    .iter()
                    .map(|m| {
                        m.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| bad("each of `meta` is a `key=value` string".into()))
                    })
                    .collect::<Result<_, _>>()?,
                whens: list(v, "whens")?
                    .iter()
                    .map(|w| Ok((conds(w)?, s(w, "value")?, opt(w, "comment")?)))
                    .collect::<Result<Vec<_>, Value>>()?,
                comment: opt(v, "comment")?,
                before: opt(v, "before")?,
            },
            Some("rename-token") => Op::RenameToken {
                from: s(v, "from")?,
                to: s(v, "to")?,
            },
            Some("remove-token") => Op::RemoveToken {
                name: s(v, "name")?,
            },
            Some("add-axis-value") => Op::AddAxisValue {
                axis: s(v, "axis")?,
                value: s(v, "value")?,
                before: opt(v, "before")?,
            },
            Some("add-axis") => Op::AddAxis {
                name: s(v, "name")?,
                values: list(v, "values")?
                    .iter()
                    .map(|x| {
                        x.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| bad("each of `values` is a string".into()))
                    })
                    .collect::<Result<_, _>>()?,
                under_dark: match opt(v, "under")?.as_deref() {
                    None => false,
                    Some("scheme=dark") => true,
                    Some(other) => {
                        return Err(bad(format!("`under` is `scheme=dark`, not `{other}`")))
                    }
                },
            },
            Some("remove-axis-value") => Op::RemoveAxisValue {
                axis: s(v, "axis")?,
                value: s(v, "value")?,
            },
            other => return Err(bad(format!("no operation {}", other.unwrap_or("(none)")))),
        };
        out.push((file, op));
    }
    Ok(out)
}
