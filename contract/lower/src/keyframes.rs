//! `keyframes` declarations and the `animation` attribute that names them.
//!
//! @ref LLP 1057 (keyframe animation) D1–D3
//!
//! A declaration resolves once, here, to the motion crate's [`Keyframes`];
//! its values go through the kernel's own row parser, so `translate="4px
//! 8px"` in a keyframe means what it means on a node. An `animation` value's
//! every literal is then rewritten to the row's self-contained text — the
//! shorthand followed by each `@keyframes` rule it names — so the plan, the
//! runner and every host carry keyframes, never a name to look up. A name
//! nobody declared is refused here, at compile time.

use crate::{err, tags, values, LowerError};
use contract_syntax::{Attr, Expr, File};
use exact_kernel::motion::{stroke_targets, targets};
use exact_kernel::{StyleId, StyleProps, StyleValue};
use exact_motion::{Animations, Easing, KeyframeBlock, Keyframes, ParseError};
use std::collections::BTreeMap;

/// The file's keyframes: by name, and every rule as CSS text, which an
/// `animation` literal is parsed against.
#[derive(Debug, Default)]
pub(crate) struct Table {
    by_name: BTreeMap<String, Keyframes>,
    rules: String,
}

/// Resolve every `keyframes` declaration. A refused one is left out of the
/// table and reported; the rest still resolve.
pub(crate) fn resolve(file: &File) -> (Table, Vec<LowerError>) {
    let mut table = Table::default();
    let mut errors = Vec::new();
    for decl in &file.keyframes {
        let mut blocks = Vec::new();
        let mut refused = false;
        for block in &decl.blocks {
            let mut out = KeyframeBlock {
                offset: 0.0,
                easing: None,
                values: Vec::new(),
            };
            for a in &block.attrs {
                match value(a) {
                    Ok(Setting::Easing(easing)) => out.easing = Some(easing),
                    Ok(Setting::Value(property, value)) => out.values.push((property, value)),
                    Err(e) => {
                        errors.push(e);
                        refused = true;
                    }
                }
            }
            for &percent in &block.offsets {
                if !(0.0..=100.0).contains(&percent) {
                    errors.push(LowerError {
                        id: "lower-keyframes",
                        message: format!(
                            "keyframe `{percent}%` is outside 0%–100% in `keyframes {}`",
                            decl.name
                        ),
                        span: block.span,
                    });
                    refused = true;
                }
                blocks.push(KeyframeBlock {
                    offset: percent / 100.0,
                    ..out.clone()
                });
            }
        }
        if refused {
            continue;
        }
        match Keyframes::new(&decl.name, blocks) {
            Ok(keyframes) => {
                table.rules.push(' ');
                table.rules.push_str(&keyframes.rule(&keyframes.name));
                table.by_name.insert(decl.name.clone(), keyframes);
            }
            Err(e) => errors.push(LowerError {
                id: "lower-keyframes",
                message: format!("`keyframes {}` is refused: {e:?}", decl.name),
                span: decl.span,
            }),
        }
    }
    (table, errors)
}

enum Setting {
    Easing(Easing),
    Value(exact_motion::Property, exact_motion::Value),
}

/// One keyframe attribute: `animation-timing-function`, or one of the four
/// rows motion animates, read by the kernel's parser for that row.
fn value(a: &Attr) -> Result<Setting, LowerError> {
    if a.name == "animation-timing-function" {
        let Expr::Str(text, _) = &a.value else {
            return err(
                "lower-keyframes",
                "`animation-timing-function` is a CSS easing in quotes, like \"ease-in\"",
                a.span,
            );
        };
        return Easing::parse(text).map(Setting::Easing).or_else(|_| {
            err(
                "lower-keyframes",
                format!("`animation-timing-function=\"{text}\"` is not a CSS easing (`spring()` is for `transition` only)"),
                a.span,
            )
        });
    }
    let row = match tags::attr(&a.name) {
        Some(tags::AttrTarget::Styles(
            [row @ (StyleId::Translate
            | StyleId::Scale
            | StyleId::Rotate
            | StyleId::Opacity
            | StyleId::StrokeStart
            | StyleId::StrokeEnd)],
        )) => *row,
        Some(_) => {
            return err(
                "lower-keyframes",
                format!("`{}` cannot be in a keyframe: keyframes animate `translate`, `scale`, `rotate` and `opacity`, the properties motion runs without layout (LLP 1002), and a path's `stroke-start` and `stroke-end` (LLP 1065)", a.name),
                a.span,
            )
        }
        None => {
            let hint = tags::similar_attr(&a.name, true)
                .map(|n| format!("; did you mean `{n}`?"))
                .unwrap_or_default();
            return err(
                "lower-unknown-attr",
                format!("a keyframe has no attribute `{}`{hint}", a.name),
                a.span,
            );
        }
    };
    let literal = match &a.value {
        Expr::Str(s, _) => StyleValue::Text(s.clone()),
        other => match values::numeric_literal(other) {
            Some(n) => StyleValue::Number(n),
            None => {
                return err(
                    "lower-keyframes",
                    format!("`{}` in a keyframe is a number or a string", a.name),
                    a.span,
                )
            }
        },
    };
    let mut style = StyleProps::default();
    if let Err(e) = style.set_dynamic(row, &literal) {
        return err(
            "lower-attr-value",
            format!(
                "`{}` in a keyframe is not a valid `{}`: {}",
                a.name,
                a.name,
                values::describe(&e)
            ),
            a.span,
        );
    }
    let property = exact_motion::Property::from_name(&a.name).expect("a motion row");
    let (_, value) = targets(&style)
        .into_iter()
        .chain(stroke_targets(&style))
        .find(|(p, _)| *p == property)
        .expect("targets name every compositor row");
    Ok(Setting::Value(property, value))
}

/// An `animation` value with every literal resolved to the row's text. A
/// computed value is refused: which keyframes play is known when the app
/// compiles, and a condition may still choose between literals.
pub(crate) fn animation_value(value: &Expr, table: &Table) -> Result<Expr, LowerError> {
    Ok(match value {
        Expr::Str(text, span) => {
            let parsed = Animations::parse(&format!("{text}{}", table.rules))
                .or_else(|e| err(e_id(&e), message(text, &e, table), *span))?;
            Expr::Str(parsed.text(), *span)
        }
        // A class choice's side that leaves the row unset.
        Expr::None(_) => value.clone(),
        Expr::Ternary(c, yes, no, span) => Expr::Ternary(
            c.clone(),
            Box::new(animation_value(yes, table)?),
            Box::new(animation_value(no, table)?),
            *span,
        ),
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => Expr::Match {
            subject: subject.clone(),
            var: var.clone(),
            some: Box::new(animation_value(some, table)?),
            none: Box::new(animation_value(none, table)?),
            span: *span,
        },
        Expr::Let {
            name,
            value: bound,
            body,
            span,
        } => Expr::Let {
            name: name.clone(),
            value: bound.clone(),
            body: Box::new(animation_value(body, table)?),
            span: *span,
        },
        other => {
            return err(
                "lower-animation-literal",
                "`animation` is literal text, or a condition choosing between literals: its keyframes are resolved when the app compiles",
                other.span(),
            )
        }
    })
}

fn e_id(e: &ParseError) -> &'static str {
    match e {
        ParseError::UnknownKeyframes(_) => "lower-unknown-keyframes",
        _ => "lower-attr-value",
    }
}

fn message(text: &str, e: &ParseError, table: &Table) -> String {
    match e {
        ParseError::UnknownKeyframes(name) => {
            let declared: Vec<String> = table.by_name.keys().map(|n| format!("`{n}`")).collect();
            format!(
                "`animation=\"{text}\"` names `{name}`, but no `keyframes {name}` is declared; {}",
                if declared.is_empty() {
                    "no keyframes are declared".to_string()
                } else {
                    format!("declared: {}", declared.join(", "))
                }
            )
        }
        ParseError::SpringInAnimation => format!(
            "`animation=\"{text}\"`: `spring()` is a `transition` extension; an animation takes a CSS easing"
        ),
        other => format!(
            "`animation=\"{text}\"` is not a CSS `animation` shorthand (name, duration, easing, delay, iteration count, direction, fill mode, play state): {other:?}"
        ),
    }
}
