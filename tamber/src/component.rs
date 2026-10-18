//! A component that isn't Contract (LLP 1121 D15): a native module's view
//! or a Rust canvas component declares its params in the design,
//! `component MapCard` over one `params` block, each param a dial's line of
//! any token kind (`pin: color = accent.fill  control=color  priority=1`).
//! The compiler names each one by the declaring file and the component, as
//! a Contract param, and its declaration makes it a root: never unused.

use crate::lex::{self, Line};
use crate::syntax::{entries, ranged, syntax, At};
use crate::Diagnostic;

/// `component MapCard` and its params.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// Its name.
    pub name: At<String>,
    /// Its params, in order.
    pub params: Vec<Param>,
}

/// `inset: length = 8px  in 0..24  control=slider`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Its name, an identifier.
    pub name: At<String>,
    /// Its kind as written; what kinds a param may be is the compiler's.
    pub kind: String,
    /// Its default, as CSS text.
    pub value: At<String>,
    /// Its closed range, as written.
    pub range: Option<(String, String)>,
    /// Editor metadata (`control=`, `priority=` …), in order.
    pub meta: Vec<(String, String)>,
}

fn refused(l: &Line, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("tamber-component", message, l.line, l.col)
}

/// A `component` block: `head` its line, `body` the lines under it; `known`
/// the components above it in the file.
pub(crate) fn block(
    head: &Line,
    body: &[Line],
    known: &[Component],
    errors: &mut Vec<Diagnostic>,
) -> Option<Component> {
    let name = head.text["component".len()..].trim();
    if !head.text.starts_with("component ") || !lex::is_ident(name) {
        errors.push(syntax(
            head,
            "a component is `component Name`, its `params` under it",
        ));
        return None;
    }
    if known.iter().any(|c| c.name.value == name) {
        errors.push(refused(
            head,
            format!("a second `component {name}`: a design declares a component once"),
        ));
        return None;
    }
    let Some((params, under)) = body.split_first().filter(|(l, _)| l.text == "params") else {
        let at = body.first().unwrap_or(head);
        errors.push(refused(
            at,
            format!("`component {name}` holds one `params` block"),
        ));
        return None;
    };
    // The params' lines, deeper than `params`; anything at its indent after
    // it is a second block, which a component has not.
    let end = under
        .iter()
        .position(|l| l.indent <= params.indent)
        .unwrap_or(under.len());
    if let Some(l) = under.get(end) {
        errors.push(refused(
            l,
            format!("`{}`: `component {name}` holds one `params` block", l.text),
        ));
    }
    // A param has one value in its component: the app's, with `when` lines,
    // is its own design's (D15).
    let lines = &under[..end];
    let first = lines.first().map_or(0, |l| l.indent);
    let (whens, lines): (Vec<Line>, Vec<Line>) = lines
        .iter()
        .cloned()
        .partition(|l| l.indent > first && l.text.starts_with("when "));
    for l in &whens {
        errors.push(refused(
            l,
            "a param's default has no `when`: the app's design sets it per mode, by name",
        ));
    }
    let shape = "a param is `name: kind = value [in low..high] [meta=…]`";
    let mut out: Vec<Param> = Vec::new();
    entries(&lines, errors, |l, e| {
        let p = ranged(l, e, shape, "param")?;
        if out.iter().any(|o| o.name.value == p.name.value) {
            e.push(refused(
                l,
                format!("a second param `{}` in `component {name}`", p.name.value),
            ));
            return None;
        }
        out.push(p);
        Some(())
    });
    if out.is_empty() && end == 0 {
        errors.push(refused(
            params,
            format!("`component {name}` declares no params"),
        ));
    }
    Some(Component {
        name: At {
            value: name.to_string(),
            line: head.line,
            col: head.col,
        },
        params: out,
    })
}
