//! The markdown row's `md` parsed into blocks of styled runs (SPEC 8), which app.contract lays
//! out as nested `text` (the Markdown app's pattern): `## ` heading, paragraph, `- ` list,
//! `> ` quote; inline `**bold**`, `*italic*`, `` `code` ``, `[link](url)`.

use exact_plan::Value;

/// `shape MdRun`: id, text, style (`""`, `bold`, `italic`, `code`, `link`).
fn runs(src: &str) -> Value {
    let mut out: Vec<(String, &'static str)> = Vec::new();
    let mut plain = String::new();
    let b = src.as_bytes();
    let mut i = 0;
    let flush = |plain: &mut String, out: &mut Vec<(String, &'static str)>| {
        if !plain.is_empty() {
            out.push((std::mem::take(plain), ""));
        }
    };
    while i < b.len() {
        let rest = &src[i..];
        let styled = if let Some(r) = rest.strip_prefix("**") {
            r.find("**").map(|e| (&r[..e], "bold", e + 4))
        } else if let Some(r) = rest.strip_prefix('*') {
            r.find('*').map(|e| (&r[..e], "italic", e + 2))
        } else if let Some(r) = rest.strip_prefix('`') {
            r.find('`').map(|e| (&r[..e], "code", e + 2))
        } else if let Some(r) = rest.strip_prefix('[') {
            r.find("](").and_then(|e| r[e..].find(')').map(|c| (&r[..e], "link", e + c + 2)))
        } else {
            None
        };
        match styled {
            Some((t, style, len)) => {
                flush(&mut plain, &mut out);
                out.push((t.to_string(), style));
                i += len;
            }
            None => {
                let ch = rest.chars().next().unwrap();
                plain.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    flush(&mut plain, &mut out);
    Value::list(
        out.into_iter()
            .enumerate()
            .map(|(k, (t, st))| Value::record(vec![Value::str(&k.to_string()), Value::str(&t), Value::str(st)]))
            .collect(),
    )
}

/// `shape MdBlock`: id, kind (`h2`, `p`, `ul`, `quote`), runs, items (`shape MdItem`: id, runs).
pub fn blocks(md: &str) -> Value {
    let blocks = md
        .split("\n\n")
        .enumerate()
        .map(|(i, b)| {
            let id = Value::str(&i.to_string());
            let empty = Value::list(vec![]);
            if let Some(h) = b.strip_prefix("## ") {
                Value::record(vec![id, Value::str("h2"), runs(h), empty])
            } else if b.starts_with("- ") {
                let items = b
                    .lines()
                    .enumerate()
                    .map(|(j, l)| {
                        Value::record(vec![Value::str(&j.to_string()), runs(l.trim_start_matches("- "))])
                    })
                    .collect();
                Value::record(vec![id, Value::str("ul"), empty, Value::list(items)])
            } else if let Some(q) = b.strip_prefix("> ") {
                Value::record(vec![id, Value::str("quote"), runs(q), empty])
            } else {
                Value::record(vec![id, Value::str("p"), runs(b), empty])
            }
        })
        .collect();
    Value::list(blocks)
}
