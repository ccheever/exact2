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

/// CSS's fragmentation properties exact2 knows and does not implement: the
/// spanning element and paged media's breaks (LLP 1093 §1, §5). Multi-column
/// layout and its `break-*`, `widows` and `orphans` are rows (LLP 1093); these
/// are refused by what they would need, not as misspellings.
pub(crate) fn fragmentation(name: &str) -> Option<&'static str> {
    Some(match name {
        "column-span" => "is CSS Multi-column Layout's spanning element, which exact2 does not implement (only `column-span: none` exists, LLP 1093 §1); end the column flow and put the spanning content after it",
        "page-break-before" | "page-break-after" | "page-break-inside" => "controls where content breaks across printed pages, and exact2 does not print (LLP 1093 §5); in a multi-column flow `break-before`, `break-after` and `break-inside` take `column`, `avoid-column` and `avoid`",
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

/// The physical longhands for a CSS logical box property (r37 t1 wrote `padding-block`),
/// in a left-to-right, top-to-bottom flow.
fn logical(name: &str) -> Option<String> {
    let (base, rest) = ["padding", "margin", "inset"]
        .into_iter()
        .find_map(|b| name.strip_prefix(b).map(|r| (b, r)))?;
    let side = |s: &str| {
        if base == "inset" {
            format!("`{s}`")
        } else {
            format!("`{base}-{s}`")
        }
    };
    Some(match rest {
        "-block" => format!("{} and {}", side("top"), side("bottom")),
        "-inline" => format!(
            "{} and {} in a left-to-right flow",
            side("left"),
            side("right")
        ),
        "-block-start" => side("top"),
        "-block-end" => side("bottom"),
        "-inline-start" => format!("{} in a left-to-right flow", side("left")),
        "-inline-end" => format!("{} in a left-to-right flow", side("right")),
        _ => return None,
    })
}

/// What is written for a React Native style name that has no one CSS name.
pub(crate) fn react_native(name: &str) -> Option<&'static str> {
    Some(match name {
        "paddingHorizontal" => "; React Native's `paddingHorizontal` is `padding-left` and `padding-right` here, or `padding=\"<vertical> <horizontal>\"`",
        "paddingVertical" => "; React Native's `paddingVertical` is `padding-top` and `padding-bottom` here, or `padding=\"<vertical> <horizontal>\"`",
        "marginHorizontal" => "; React Native's `marginHorizontal` is `margin-left` and `margin-right` here, or `margin=\"<vertical> <horizontal>\"`",
        "marginVertical" => "; React Native's `marginVertical` is `margin-top` and `margin-bottom` here, or `margin=\"<vertical> <horizontal>\"`",
        "shadowColor" | "shadowOpacity" | "shadowRadius" | "shadowOffset" | "elevation" => {
            "; React Native's shadow is one `box-shadow=\"<x>px <y>px <2 × shadowRadius>px <shadowColor at shadowOpacity>\"` here"
        }
        "selectable" => "; selectable text is `user-select=\"text\"` here",
        _ => return None,
    })
}

pub(crate) fn unknown_attr(tag: &str, a: &Attr) -> LowerError {
    let hint = match tags::renamed(&a.name) {
        None if crate::fields::placeholder_hint(&a.name).is_some() => {
            crate::fields::placeholder_hint(&a.name).unwrap_or_default()
        }
        Some(new @ ("press" | "change" | "input")) => format!(
            "; `{}` is `{new}` here: a handler is named for its event (LLP 1005 §3)",
            a.name
        ),
        // CSS's own shorthand, not a nickname: Contract takes the longhand (app farm wave 1, about twelve builds).
        Some(new @ "background-color") if a.name == "background" => {
            format!("; CSS's `background` shorthand is not taken here: a colour is `{new}` (LLP 1017 §8.1)")
        }
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
        None if react_native(&a.name).is_some() => {
            react_native(&a.name).unwrap_or_default().into()
        }
        None if logical(&a.name).is_some() => format!(
            "; CSS's logical `{}` is not admitted: write {}",
            a.name,
            logical(&a.name).unwrap_or_default()
        ),
        None if tag == "dialog" && a.name == "open" => {
            ": in HTML, `open` alone shows a dialog non-modally, which Exact does not support. For a \
             modal, give the dialog an `id` and call `showModal(\"id\")` in an action, or put \
             `commandfor=\"id\" command=\"show-modal\"` on a button; close it with `close(\"id\")` \
             or `commandfor=\"id\" command=\"close\"`. A dialog that state holds open is not \
             available yet"
                .into()
        }
        // @ref LLP 1061 D7: no selectors, so no pressed block of any row;
        // what the host can change under a finger, with no layout, is named.
        None if a.name.starts_with("-exact-press-") => {
            "; a pressed look is paint only, never layout: `-exact-press-opacity`, \
             `-exact-press-background-color`, `-exact-press-color` and `-exact-press-scale` \
             (and `-exact-press-haptic`), which the host shows while a pointer holds the node"
                .into()
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
    // A theme's names are colours (LLP 1095 D4); the lint asks only whether a
    // row's value is one, so each stands for any.
    crate::theme::bind_axes(file);
    let _theme = crate::theme::lint_scope(file);
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
                        // `delete=` is lowering's, a list row's swipe (`swipe_rows.rs`).
                        for a in attrs.iter().filter(|a| {
                            a.name != "class" && a.name != "delete" && !native::leftover(tag, a)
                        }) {
                            if let Some(e) = native::refused(tag, a) {
                                errors.push(e);
                                continue;
                            }
                            if dataset::word(&a.name).is_some() {
                                errors.extend(dataset::refused(a));
                                continue;
                            }
                            let check = |a: &contract_syntax::Attr| match tags::attr_valued(
                                &a.name, &a.value,
                            ) {
                                None => Err(unknown_attr(tag, a)),
                                // A family is resolved against declared fonts.
                                Some(tags::AttrTarget::Styles(rows))
                                    if rows != [StyleId::FontFamily] =>
                                {
                                    values::check_style_value(
                                        a,
                                        rows,
                                        &Ty::Unknown,
                                        &[],
                                        &Default::default(),
                                        &Default::default(),
                                    )
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
                                                &Default::default(),
                                                &Default::default(),
                                            )
                                        })
                                }
                                // A type token's longhands are its own (stage 6 C).
                                Some(tags::AttrTarget::Shorthand)
                                    if a.name == "font"
                                        && matches!(&a.value, contract_syntax::Expr::Str(t, _)
                                            if crate::theme::measure_values(t).is_some()) =>
                                {
                                    Ok(())
                                }
                                Some(tags::AttrTarget::Shorthand) => {
                                    super::shorthands::component(&a.value, &a.name, 0).map(|_| ())
                                }
                                Some(_) => Ok(()),
                            };
                            // An expression over a token is folded by the
                            // build (a synthetic token, LLP 1121 D3), not here,
                            // in a choice's arm as in a whole value; the other
                            // arms are checked here.
                            let folded = |t: &str| {
                                t.contains('(')
                                    && crate::theme::words(t).any(exact_kernel::style::theme::names)
                            };
                            let mut leaves = Vec::new();
                            crate::theme::literal_leaves(&a.value, &mut leaves);
                            if leaves.iter().any(|(t, _)| folded(t)) {
                                for (t, span) in leaves.into_iter().filter(|(t, _)| !folded(t)) {
                                    let value = contract_syntax::Expr::Str(t, span);
                                    let leaf = contract_syntax::Attr { value, ..a.clone() };
                                    errors.extend(check(&leaf).err());
                                }
                            } else {
                                errors.extend(check(a).err());
                            }
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
    // A param by its full name, never its token's (LLP 1121 D15).
    let params = crate::params::tokens(file);
    for e in &mut errors {
        e.message = crate::params::shown(&e.message, &params);
    }
    errors
}
