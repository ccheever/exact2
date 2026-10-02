//! Refusals that need no types: an element's tag, its attributes' names,
//! and its literal style values against their rows.

use crate::{dataset, native, svg, tags, values, LowerError, MAX_REFUSALS};
use contract_syntax::{Attr, File, Node, Span};
use contract_types::Ty;
use exact_kernel::StyleId;

pub(crate) fn unknown_tag(tag: &str, span: Span) -> LowerError {
    let hint = tags::html_tag(tag)
        .or_else(|| svg::refused_tag(tag))
        .map(|spelled| format!("; {spelled}"))
        .or_else(|| tags::similar_tag(tag).map(|n| format!("; did you mean `{n}`?")))
        .unwrap_or_default();
    LowerError {
        id: "lower-unknown-tag",
        message: format!("unknown tag `{tag}`{hint}"),
        span,
    }
}

pub(crate) fn unknown_attr(tag: &str, a: &Attr) -> LowerError {
    let hint = match tags::renamed(&a.name) {
        Some(new @ ("press" | "change" | "input")) => format!(
            "; `{}` is `{new}` here: a handler is named for its event (LLP 1005 §3)",
            a.name
        ),
        Some(new) => format!(
            "; `{}` is spelled `{new}` here, the web's name (LLP 1017 §8.1)",
            a.name
        ),
        None if a.name == "className" => {
            "; `class` names a `style` declared in this file, as in `class=Card`".into()
        }
        None => tags::similar_attr(&a.name, false)
            .map(|n| format!("; did you mean `{n}`?"))
            .unwrap_or_default(),
    };
    LowerError {
        id: "lower-unknown-attr",
        message: format!("`{tag}` has no attribute `{}`{hint}", a.name),
        span: a.span,
    }
}

/// What an authored element can be refused for without any types: its
/// tag, its attributes' names, and its literal style values against their
/// rows. The driver runs this when an earlier pass refused, so a misspelled
/// tag or a bad colour is reported in the same run as a type error.
pub fn lint(file: &File) -> Vec<LowerError> {
    fn walk(nodes: &[Node], errors: &mut Vec<LowerError>) {
        for n in nodes {
            match n {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    span,
                    ..
                } => {
                    if tags::tag(tag).is_none() && !native::is_module_tag(tag) {
                        errors.push(unknown_tag(tag, *span));
                    } else {
                        let coerced = svg::coerce_lengths(tag, false, attrs);
                        let attrs = coerced.as_deref().unwrap_or(attrs);
                        for a in attrs
                            .iter()
                            .filter(|a| a.name != "class" && !native::leftover(tag, a))
                        {
                            if let Some(e) = native::refused(tag, a) {
                                errors.push(e);
                                continue;
                            }
                            if dataset::word(&a.name).is_some() {
                                errors.extend(dataset::refused(a));
                                continue;
                            }
                            let checked = match tags::attr(&a.name) {
                                None => Err(unknown_attr(tag, a)),
                                // A family is resolved against declared fonts.
                                Some(tags::AttrTarget::Styles(rows))
                                    if rows != [StyleId::FontFamily] =>
                                {
                                    values::check_style_value(a, rows, &Ty::Unknown, &[])
                                }
                                Some(tags::AttrTarget::Flex) => values::check_style_value(
                                    a,
                                    &[StyleId::FlexGrow],
                                    &Ty::Unknown,
                                    &[],
                                ),
                                Some(_) => Ok(()),
                            };
                            errors.extend(checked.err());
                        }
                    }
                    walk(children, errors);
                }
                Node::Use { children, .. } => walk(children, errors),
                Node::Provide { body, .. } => walk(body, errors),
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
    let mut errors = Vec::new();
    for c in &file.components {
        walk(&c.view, &mut errors);
    }
    errors.truncate(MAX_REFUSALS);
    errors
}
