//! `add-param` and `remove-param` (LLP 1121 D15, stage 7 C's, landed with
//! stage 9 D3): a line of a component's `params` section in the app's own
//! Contract, the section made before the component's `view` where it has
//! none and taken out with its last line. A param no row reads is refused
//! by the compile (D15), so each goes in one transaction with the
//! `set-row` that starts or stops reading it (LLP 1122 D6's promote and
//! detach).

use contract_syntax::params::ATTRIBUTES;

/// A param's line as the server writes it.
#[derive(Debug, Clone)]
pub(crate) struct ParamLine {
    pub component: String,
    pub name: String,
    pub kind: String,
    /// A token value, written as a Contract string.
    pub default: String,
    /// `range`, `step`, … in D15's order, each written as Contract writes it.
    pub attrs: Vec<(String, String)>,
}

/// A word: a param's name or kind.
fn word(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `{component, name, kind, default, attrs?}` read from a request.
pub(crate) fn line_of(v: &serde_json::Value) -> Result<ParamLine, String> {
    let s = |k: &str| {
        v[k].as_str()
            .map(str::to_string)
            .ok_or(format!("`{k}` is a string"))
    };
    let (component, name, kind, default) = (s("component")?, s("name")?, s("kind")?, s("default")?);
    if !word(&name) || !word(&kind) {
        return Err("a param's `name` and `kind` are words (`radius`, `length`)".into());
    }
    let mut attrs = Vec::new();
    if let Some(given) = v.get("attrs") {
        let given = given.as_object().ok_or("`attrs` is an object")?;
        if let Some(k) = given.keys().find(|k| !ATTRIBUTES.contains(&k.as_str())) {
            return Err(format!(
                "`{k}` is no param attribute; they are {}",
                ATTRIBUTES.join(", ")
            ));
        }
        for k in ATTRIBUTES {
            match given.get(k) {
                None => {}
                Some(serde_json::Value::String(t)) => {
                    attrs.push((k.to_string(), crate::tamber_contract::string_literal(t)))
                }
                Some(serde_json::Value::Number(n)) => attrs.push((k.to_string(), n.to_string())),
                Some(_) => return Err(format!("`{k}` is a string or a number")),
            }
        }
    }
    Ok(ParamLine {
        component,
        name,
        kind,
        default: crate::tamber_contract::string_literal(&default),
        attrs,
    })
}

fn split(text: &str) -> (Vec<String>, bool) {
    let newline = text.ends_with('\n');
    let lines = text
        .strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .map(str::to_string)
        .collect();
    (lines, newline)
}

fn joined(all: &[String], newline: bool) -> String {
    let mut out = all.join("\n");
    if newline {
        out.push('\n');
    }
    out
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// A line's code: its text before a `//` comment, trimmed (a section's
/// header may carry one: `view // body`).
fn code(line: &str) -> &str {
    line.split("//").next().unwrap_or("").trim()
}

/// The lines of component `c` past its header: up to the next line at its
/// header's indent or less (the next declaration), or the file's end.
fn body(all: &[String], c: &contract_syntax::Component) -> std::ops::Range<usize> {
    let header = c.span.line as usize - 1;
    let top = indent(&all[header]);
    let end = (header + 1..all.len())
        .find(|&i| !code(&all[i]).is_empty() && indent(&all[i]) <= top)
        .unwrap_or(all.len());
    header + 1..end
}

/// The component named, and its member indent.
fn component<'a>(
    file: &'a contract_syntax::File,
    all: &[String],
    name: &str,
) -> Result<(&'a contract_syntax::Component, usize), String> {
    let c = file
        .components
        .iter()
        .find(|c| c.name == name)
        .ok_or_else(|| format!("no `component {name}` in this file"))?;
    let header = c.span.line as usize - 1;
    let members = all[header + 1..]
        .iter()
        .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with("//"))
        .map_or(indent(&all[header]) + 2, |l| indent(l));
    Ok((c, members))
}

/// `p` added to its component's `params` section, made where there is none.
pub(crate) fn add(text: &str, p: &ParamLine) -> Result<String, String> {
    let file = contract_syntax::parse(text).map_err(|e| e.message)?;
    let (mut all, newline) = split(text);
    let (c, members) = component(&file, &all, &p.component)?;
    if c.params.list.iter().any(|q| q.name == p.name) {
        return Err(format!(
            "`{}` has a param `{}` already",
            p.component, p.name
        ));
    }
    let mut line = format!("{}: {} = {}", p.name, p.kind, p.default);
    for (k, v) in &p.attrs {
        line.push_str(&format!("  {k}={v}"));
    }
    match c.params.list.last() {
        Some(last) => {
            let at = last.span.line as usize - 1;
            let pad = " ".repeat(indent(&all[at]));
            all.insert(at + 1, format!("{pad}{line}"));
        }
        None => {
            // Before the component's own `view`, at its members' indent.
            let view = body(&all, c)
                .find(|&i| indent(&all[i]) == members && code(&all[i]) == "view")
                .ok_or_else(|| {
                    format!(
                        "`component {}` has no `view` to put `params` before",
                        p.component
                    )
                })?;
            all.insert(view, format!("{}{line}", " ".repeat(members + 2)));
            all.insert(view, format!("{}params", " ".repeat(members)));
        }
    }
    Ok(joined(&all, newline))
}

/// The param `name` of `component` taken out, and its section with its last.
pub(crate) fn remove(text: &str, component_name: &str, name: &str) -> Result<String, String> {
    let file = contract_syntax::parse(text).map_err(|e| e.message)?;
    let (mut all, newline) = split(text);
    let (c, members) = component(&file, &all, component_name)?;
    let q = c
        .params
        .list
        .iter()
        .find(|q| q.name == name)
        .ok_or_else(|| format!("`{component_name}` has no param `{name}`"))?;
    let at = q.span.line as usize - 1;
    let header = (c.span.line as usize..at)
        .rev()
        .find(|&i| indent(&all[i]) == members && code(&all[i]) == "params");
    match c.params.list.len() {
        // The last: the section goes whole, its header and the comments
        // and blank lines inside it.
        1 => {
            let header = header.ok_or("the `params` section has no header")?;
            // And the comments under it, inside the section.
            // (through blank lines between them).
            let mut end = at;
            let mut next = at + 1;
            while let Some(l) = all.get(next) {
                if l.trim().is_empty() {
                    next += 1;
                    continue;
                }
                if !(code(l).is_empty() && indent(l) > members) {
                    break;
                }
                end = next;
                next += 1;
            }
            all.drain(header..=end);
        }
        _ => {
            all.remove(at);
        }
    }
    Ok(joined(&all, newline))
}
