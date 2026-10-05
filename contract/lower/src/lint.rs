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

/// CSS properties exact2 knows and does not implement because they need
/// fragmentation (CSS Fragmentation 3): a block's content continuing from
/// one box into the next. The kernel's layout has none yet, so these are
/// refused by what they would need, not as misspellings (the reader diary).
/// LLP 1093 (admitted, not yet built) is the planned home of multi-column and
/// the break rules inside it; each refusal stays until its stage lands, and
/// says so. `column-span` and paged media stay out (LLP 1093 §5).
pub(crate) fn fragmentation(name: &str) -> Option<&'static str> {
    Some(match name {
        "widows" | "orphans" => "applies only where a paragraph's lines are split across pages, columns or regions (CSS Fragmentation 3); exact2 has no fragmentation context yet (no paged media, no `columns`), so a paragraph's lines are never split and it would change nothing. It arrives with CSS multi-column, planned in LLP 1093 (admitted, not yet built); until then an app that pages a column by translating it keeps its own rule",
        "columns" | "column-count" | "column-width" | "column-fill" | "column-rule"
        | "column-rule-width" | "column-rule-style" | "column-rule-color" => "is CSS Multi-column Layout, which exact2 does not implement yet: the kernel's layout has no fragmentation, so one flow cannot continue from one column box into the next on a native host. LLP 1093 (admitted, not yet built) is its planned home; until it lands, page a fixed-height column by translating it, and show two such windows for a spread",
        "column-span" => "is CSS Multi-column Layout's spanning element, which exact2 does not implement and LLP 1093, the planned multi-column, leaves out (§5); end the column flow and put the spanning content after it",
        "break-before" | "break-after" | "break-inside" => "controls where content fragments across pages or columns; exact2 has no fragmentation context yet (no paged media, no `columns`), so there is no break to control. Its column values arrive with CSS multi-column, planned in LLP 1093 (admitted, not yet built); page and region values stay out",
        "page-break-before" | "page-break-after" | "page-break-inside" => "controls where content fragments across pages; exact2 has no fragmentation context (paged media stays out, LLP 1093 §5), so there is no break to control",
        _ => return None,
    })
}

/// WAI-ARIA 1.2's states and properties: an unknown `aria-*` name is told
/// whether ARIA has it, and which of these Contract carries (`tags::attr`).
const ARIA: &[&str] = &[
    "aria-activedescendant",
    "aria-atomic",
    "aria-autocomplete",
    "aria-braillelabel",
    "aria-brailleroledescription",
    "aria-busy",
    "aria-checked",
    "aria-colcount",
    "aria-colindex",
    "aria-colindextext",
    "aria-colspan",
    "aria-controls",
    "aria-current",
    "aria-describedby",
    "aria-description",
    "aria-details",
    "aria-disabled",
    "aria-dropeffect",
    "aria-errormessage",
    "aria-expanded",
    "aria-flowto",
    "aria-grabbed",
    "aria-haspopup",
    "aria-hidden",
    "aria-invalid",
    "aria-keyshortcuts",
    "aria-label",
    "aria-labelledby",
    "aria-level",
    "aria-live",
    "aria-modal",
    "aria-multiline",
    "aria-multiselectable",
    "aria-orientation",
    "aria-owns",
    "aria-placeholder",
    "aria-posinset",
    "aria-pressed",
    "aria-readonly",
    "aria-relevant",
    "aria-required",
    "aria-roledescription",
    "aria-rowcount",
    "aria-rowindex",
    "aria-rowindextext",
    "aria-rowspan",
    "aria-selected",
    "aria-setsize",
    "aria-sort",
    "aria-valuemax",
    "aria-valuemin",
    "aria-valuenow",
    "aria-valuetext",
];

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
        None if fragmentation(&a.name).is_some() => {
            format!(
                ": `{}` {}",
                a.name,
                fragmentation(&a.name).unwrap_or_default()
            )
        }
        None if a.name == "className" => {
            "; `class` names a `style` declared in this file, as in `class=Card`".into()
        }
        None if a.name.starts_with("aria-") => {
            let carried: Vec<&str> = ARIA
                .iter()
                .copied()
                .filter(|n| tags::attr(n).is_some())
                .collect();
            let what = if ARIA.contains(&a.name.as_str()) {
                "is ARIA's, and Contract does not carry it yet".to_string()
            } else {
                match tags::similar_attr(&a.name, false) {
                    Some(n) => format!("is not ARIA's (did you mean `{n}`?)"),
                    None => "is not ARIA's".to_string(),
                }
            };
            format!(
                "; `{}` {what}; Contract carries {}",
                a.name,
                carried.join(", ")
            )
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
                            let checked = match tags::attr_valued(&a.name, &a.value) {
                                None => Err(unknown_attr(tag, a)),
                                // A family is resolved against declared fonts.
                                Some(tags::AttrTarget::Styles(rows))
                                    if rows != [StyleId::FontFamily] =>
                                {
                                    values::check_style_value(a, rows, &Ty::Unknown, &[])
                                }
                                // The shorthand's parts, as lowering checks them: `flex="none"`
                                // was refused here as a grow number beside another error
                                // (authoring bench: three builders).
                                Some(tags::AttrTarget::Flex) => {
                                    [StyleId::FlexGrow, StyleId::FlexShrink, StyleId::FlexBasis]
                                        .into_iter()
                                        .enumerate()
                                        .try_for_each(|(index, row)| {
                                            let value = values::flex_component(&a.value, index)?;
                                            let part = contract_syntax::Attr { value, ..a.clone() };
                                            values::check_style_value(
                                                &part,
                                                &[row],
                                                &Ty::Unknown,
                                                &[],
                                            )
                                        })
                                }
                                Some(tags::AttrTarget::Shorthand) => {
                                    super::shorthands::component(&a.value, &a.name, 0).map(|_| ())
                                }
                                Some(_) => Ok(()),
                            };
                            errors.extend(checked.err());
                        }
                    }
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
    let mut errors = Vec::new();
    for c in &file.components {
        walk(&c.view, &mut errors);
    }
    errors.truncate(MAX_REFUSALS);
    errors
}
