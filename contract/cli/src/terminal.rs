//! The terminal profile (LLP 1101 D2): a terminal entry is checked against
//! the schema's `terminal` column before it compiles, so a row a terminal
//! cannot draw is an error the author sees while writing, never a silent
//! degradation at run time.
//!
//! Each style row the schema marks `cell` takes lengths only in cells (`ch`
//! a column, `lh` a row), a percentage, `auto` or 0 (D3); `admit` and
//! `quantize` rows take what they take everywhere; a row with no mark is
//! refused. The tags a terminal draws nothing for are refused by name.

use super::CompileError;
use contract_lower::tags::{attr_valued, AttrTarget};
use contract_syntax::{Expr, File, Node, Span};
use exact_kernel::StyleId;
use std::collections::HashMap;
use std::sync::OnceLock;

/// How a terminal entry admits a style row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Lengths in cells only.
    Cell,
    /// As everywhere.
    Admit,
    /// Approximated by the host (colour depth, weight, border widths).
    Quantize,
}

/// The schema's `terminal` column, by row name.
fn column() -> &'static HashMap<String, Admission> {
    static COLUMN: OnceLock<HashMap<String, Admission>> = OnceLock::new();
    COLUMN.get_or_init(|| {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../kernel/tables/schema.json"))
                .expect("the schema is JSON");
        schema["styles"]
            .as_array()
            .expect("styles")
            .iter()
            .filter_map(|row| {
                let admission = match row["terminal"].as_str()? {
                    "cell" => Admission::Cell,
                    "admit" => Admission::Admit,
                    "quantize" => Admission::Quantize,
                    other => panic!("schema: unknown terminal admission {other:?}"),
                };
                Some((row["field"].as_str()?.to_string(), admission))
            })
            .collect()
    })
}

/// How a terminal entry admits a row; `None` is refused.
pub fn admission(row: StyleId) -> Option<Admission> {
    column().get(row.name()).copied()
}

/// Tags with no terminal form, and why.
fn refused_tag(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "canvas" => "a terminal has no canvas",
        "video" | "audio" => "a terminal plays no media",
        "iframe" => "a terminal embeds no page",
        "svg" => "a terminal draws no vector graphics",
        "img" | "picture" => "images in a terminal are not built yet (LLP 1101 §4: kitty, iTerm2, sixel, half-blocks)",
        _ => return None,
    })
}

fn refusal(id: &str, message: String, span: Span) -> CompileError {
    CompileError {
        pass: "terminal",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    }
}

/// Whether one written length is a terminal length.
fn cell_length(part: &str) -> bool {
    let part = part.trim();
    matches!(
        part,
        "0" | "auto" | "none" | "min-content" | "max-content" | "fit-content"
    ) || part.ends_with('%')
        || exact_kernel::style::cells::parse(part).is_some()
}

/// The literal lengths in `value` that are not in cells.
fn non_cells(value: &Expr, out: &mut Vec<String>) {
    match value {
        Expr::Number(n, _) if *n != 0.0 => out.push(format!("{n}")),
        Expr::Str(text, _) => out.extend(
            text.split_whitespace()
                .filter(|p| !cell_length(p))
                .map(str::to_string),
        ),
        Expr::Ternary(_, yes, no, _) => {
            non_cells(yes, out);
            non_cells(no, out);
        }
        _ => {}
    }
}

fn check_attrs(tag: &str, attrs: &[contract_syntax::Attr], errors: &mut Vec<CompileError>) {
    for a in attrs {
        let Some(AttrTarget::Styles(rows)) = attr_valued(&a.name, &a.value) else {
            continue;
        };
        for row in rows.iter() {
            match admission(*row) {
                None => {
                    errors.push(refusal(
                        "terminal-refused",
                        format!(
                            "`{}` on `{tag}`: a terminal entry cannot draw `{}` (LLP 1101 §4)",
                            a.name,
                            row.name().replace('_', "-")
                        ),
                        a.span,
                    ));
                    break;
                }
                Some(Admission::Cell) => {
                    let mut bad = Vec::new();
                    non_cells(&a.value, &mut bad);
                    if let Some(first) = bad.first() {
                        errors.push(refusal(
                            "terminal-length",
                            format!(
                                "`{}={first}` on `{tag}`: a terminal length is in cells — `Nch` (columns), `Nlh` (rows), a percentage, `auto` or 0; a number is pixels (LLP 1101 D3)",
                                a.name
                            ),
                            a.span,
                        ));
                        break;
                    }
                }
                Some(Admission::Admit | Admission::Quantize) => {}
            }
        }
    }
}

fn walk(nodes: &[Node], errors: &mut Vec<CompileError>) {
    for n in nodes {
        match n {
            Node::Element {
                tag,
                attrs,
                children,
                span,
                ..
            } => {
                if let Some(why) = refused_tag(tag) {
                    errors.push(refusal(
                        "terminal-tag",
                        format!("`{tag}` in a terminal entry: {why}"),
                        *span,
                    ));
                }
                check_attrs(tag, attrs, errors);
                walk(children, errors);
            }
            Node::Use { children, .. } => walk(children, errors),
            Node::When {
                then, otherwise, ..
            } => {
                walk(then, errors);
                walk(otherwise, errors);
            }
            Node::Each { body, .. } => walk(body, errors),
            Node::Match { some, none, .. } => {
                walk(&some.1, errors);
                walk(none, errors);
            }
            Node::Children { .. } => {}
        }
    }
}

/// Every refusal in a terminal entry, in source order.
pub(super) fn check(file: &File) -> Result<(), Vec<CompileError>> {
    let mut errors = Vec::new();
    for c in &file.components {
        walk(&c.view, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(src: &str) -> Vec<String> {
        let file = contract_syntax::parse(src).expect("parses");
        check(&file)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect()
    }

    #[test]
    fn cells_pass_and_pixels_shadows_and_canvases_are_refused() {
        let ok = "component A\n  view\n    column padding=\"0 1ch\" gap=\"1lh\" width=\"50%\" min-height=0 color=\"#fff\" font-weight=700\n      text \"hi\" margin-left=\"auto\"\n";
        assert_eq!(ids(ok), Vec::<String>::new());
        let bad = "component A\n  view\n    column padding=16 box-shadow=\"0 1px #000\"\n      text \"hi\" font-size=20 margin=\"1lh 2px\"\n      canvas width=\"1ch\"\n";
        assert_eq!(
            ids(bad),
            [
                "terminal-length",
                "terminal-refused",
                "terminal-refused",
                "terminal-length",
                "terminal-tag"
            ]
        );
    }

    #[test]
    fn every_admitted_row_names_a_schema_row() {
        assert!(column().len() >= 70, "the schema's terminal column is read");
        for name in column().keys() {
            assert!(StyleId::from_name(name).is_some(), "{name}");
        }
        assert_eq!(admission(StyleId::PaddingTop), Some(Admission::Cell));
        assert_eq!(admission(StyleId::BoxShadow), None);
    }
}
