//! The standard functions and intrinsics as `contract vocab <name>` prints
//! them (LLP 1116.000 D5): a function's signature is the roster's
//! (`plan/tables/format.json`'s `stdlib`, as `exact_plan::Stdlib` holds it),
//! and its rules and example are the grammar's tables (docs/contract-grammar.md,
//! "Standard functions and intrinsics"), the one place that prose is
//! written. A row names every call its first cell writes; a function may have
//! several rows (`slice` of text and of a list).

use exact_plan::Stdlib;

/// The grammar, read at build time: what `vocab` says is what the docs say.
const GRAMMAR: &str = include_str!("../../../docs/contract-grammar.md");
const SECTION: &str = "## Standard functions and intrinsics";

/// One row of the grammar's tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The first cell: the call forms, as written.
    pub call: String,
    /// The result and its restrictions.
    pub rule: String,
    /// One line of use: `` `call` is `value` `` where it can be evaluated
    /// in a view, else a phrase.
    pub example: String,
}

/// A standard function, or a compiler intrinsic (`pending`, `path`, `some`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The name a call writes.
    pub name: String,
    /// The roster's signature; `None` for an intrinsic.
    pub signature: Option<String>,
    /// Its rows in the grammar's tables.
    pub rows: Vec<Row>,
}

/// The roster's signature: its parameter types, an optional trailing one in
/// brackets, and its return type (`slice(any, number[, number]) -> any`), the
/// first parameter before the name for a function written on its value
/// (`any.slice(number[, number]) -> any`, `any.length -> number`; LLP 1116.015 D2).
pub fn signature(f: Stdlib) -> String {
    let name = f.name();
    let params = f.params();
    let required = params.len() - f.defaults().len();
    let params: Vec<String> = params.iter().map(|p| written(p)).collect();
    let returns = written(f.returns());
    if name == "length" {
        return format!("{}.length -> {returns}", params[0]);
    }
    // The roster row is the lowered call (the list, its keys, descending);
    // an author writes the key callback and a direction literal.
    if f == Stdlib::SortBy {
        return "sortBy(list, (item, index) => key[, \"asc\" | \"desc\"]) -> list".into();
    }
    let skip = usize::from(contract_syntax::idioms::is_method(name));
    let mut out = match skip {
        1 => format!("{}.{name}(", params[0]),
        _ => format!("{name}("),
    };
    for (i, p) in params.iter().enumerate().skip(skip) {
        match (i - skip, i >= required) {
            (0, true) => out.push('['),
            (_, true) => out.push_str("[, "),
            (0, false) => {}
            _ => out.push_str(", "),
        }
        out.push_str(p);
    }
    out.push_str(&"]".repeat(params.len() - required.max(skip)));
    out.push_str(") -> ");
    out.push_str(&returns);
    out
}

/// A roster type as an author writes it: the table's `list<T>` is `T[]`.
fn written(ty: &str) -> String {
    match ty.strip_prefix("list<").and_then(|t| t.strip_suffix('>')) {
        Some(inner) => format!("{}[]", written(inner)),
        None => ty.to_owned(),
    }
}

/// The cells of a table row, trimmed.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    inner.split('|').map(|c| c.trim().to_owned()).collect()
}

/// The names a call cell writes: in each code span, the name a `(` follows
/// (`slice` of `xs.slice(start, end?)`), or `length` of `value.length`.
fn names(call: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for span in call.split('`').skip(1).step_by(2) {
        let head = span.split('(').next().unwrap_or_default();
        let name = match span.contains('(') {
            true => head.rsplit('.').next().unwrap_or_default(),
            false if span.ends_with(".length") => "length",
            false => "",
        };
        let ident = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if ident && !out.iter().any(|n| n == name) {
            out.push(name.to_owned());
        }
    }
    out
}

/// Every row of the section's call and form tables, with the names it covers.
fn rows() -> Vec<(Vec<String>, Row)> {
    let start = GRAMMAR
        .find(&format!("\n{SECTION}\n"))
        .expect("docs/contract-grammar.md has its standard-function section");
    let section = &GRAMMAR[start + 1..];
    let end = section[SECTION.len()..]
        .find("\n## ")
        .map_or(section.len(), |i| i + SECTION.len());
    let mut out = Vec::new();
    let mut in_table = false;
    for line in section[..end].lines() {
        if line.starts_with("| Call |") || line.starts_with("| Form |") {
            in_table = true;
        } else if !line.starts_with('|') {
            in_table = false;
        } else if in_table && line.starts_with("| `") {
            let c = cells(line);
            let row = Row {
                call: c[0].clone(),
                rule: c.get(1).cloned().unwrap_or_default(),
                example: c.get(2).cloned().unwrap_or_default(),
            };
            out.push((names(&row.call), row));
        }
    }
    out
}

/// Every standard function, in the roster's order, then the intrinsics the
/// tables name, in theirs.
pub fn functions() -> Vec<Function> {
    let rows = rows();
    let of = |name: &str| -> Vec<Row> {
        rows.iter()
            .filter(|(names, _)| names.iter().any(|n| n == name))
            .map(|(_, row)| row.clone())
            .collect()
    };
    let mut out: Vec<Function> = Stdlib::ALL
        .iter()
        .map(|f| Function {
            name: f.name().to_owned(),
            signature: Some(signature(*f)),
            rows: of(f.name()),
        })
        .collect();
    for (names, _) in &rows {
        for name in names {
            if !out.iter().any(|f| &f.name == name) {
                out.push(Function {
                    name: name.clone(),
                    signature: None,
                    rows: of(name),
                });
            }
        }
    }
    out
}

/// One function or intrinsic by name.
pub fn function(name: &str) -> Option<Function> {
    functions().into_iter().find(|f| f.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signature_brackets_its_optional_tail() {
        let slice = Stdlib::from_name("slice").unwrap();
        assert_eq!(signature(slice), "any.slice(number[, number]) -> any");
        let trim = Stdlib::from_name("trim").unwrap();
        assert_eq!(signature(trim), "string.trim() -> string");
        let now = Stdlib::from_name("performanceNow").unwrap();
        assert_eq!(signature(now), "performanceNow() -> number");
    }

    #[test]
    fn a_call_cell_names_each_call_once() {
        assert_eq!(
            names("`formatNumber(n, \"decimal\")`, `formatNumber(n, \"percent\")`"),
            ["formatNumber"]
        );
        assert_eq!(names("`t(\"key\", name=value, …)`"), ["t"]);
        assert!(names("`offline`").is_empty());
    }
}
