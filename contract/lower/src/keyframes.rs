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
use contract_syntax::{Attr, Expr, File, FnDecl, TemplatePart};
use exact_kernel::motion::{stroke_targets, targets};
use exact_kernel::style::ColorValue;
use exact_kernel::{RowValue, StyleId, StyleProps, StyleValue};
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
                dark: Vec::new(),
            };
            for a in &block.attrs {
                match value(a, &file.fns) {
                    Ok(Setting::Easing(easing)) => out.easing = Some(easing),
                    Ok(Setting::Value(values, dark)) => {
                        out.values.extend(values);
                        out.dark.extend(dark);
                    }
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

type Values = Vec<(exact_motion::Property, exact_motion::Value)>;

enum Setting {
    Easing(Easing),
    /// The values, and a `light-dark()` colour's dark ones.
    Value(Values, Values),
}

/// A keyframe's value as written, through calls to argument-free functions
/// (`color=accent()` where `fn accent(): string = "light-dark(…)"`): a
/// palette is written once, and a keyframe is still known when the app
/// compiles.
fn constant<'a>(e: &'a Expr, fns: &'a [FnDecl]) -> &'a Expr {
    let mut e = e;
    for _ in 0..16 {
        let Expr::Call(name, args, _) = e else { break };
        match fns.iter().find(|f| &f.name == name && f.params.is_empty()) {
            Some(f) if args.is_empty() => e = &f.body,
            _ => break,
        }
    }
    e
}

/// The motion property a keyframe row animates: the four compositor rows,
/// and the colours (LLP 1062).
fn animated(row: StyleId) -> Option<exact_motion::Property> {
    use exact_motion::Property as P;
    Some(match row {
        StyleId::Translate => P::Translate,
        StyleId::Scale => P::Scale,
        StyleId::Rotate => P::Rotate,
        StyleId::Opacity => P::Opacity,
        StyleId::BackgroundColor => P::BackgroundColor,
        StyleId::TextColor => P::Color,
        StyleId::BorderColorTop => P::BorderTopColor,
        StyleId::BorderColorRight => P::BorderRightColor,
        StyleId::BorderColorBottom => P::BorderBottomColor,
        StyleId::BorderColorLeft => P::BorderLeftColor,
        StyleId::TintColor => P::TintColor,
        // @ref LLP 1065 D4 — a path's stroke fractions.
        StyleId::StrokeStart => P::StrokeStart,
        StyleId::StrokeEnd => P::StrokeEnd,
        StyleId::Fill => P::Fill,
        StyleId::Stroke => P::Stroke,
        _ => return None,
    })
}

/// One keyframe attribute: `animation-timing-function`, or a row motion
/// animates — the four compositor rows or a colour — read by the kernel's
/// parser for that row.
fn value(a: &Attr, fns: &[FnDecl]) -> Result<Setting, LowerError> {
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
    let rows = match tags::attr(&a.name) {
        Some(tags::AttrTarget::Styles(rows)) if rows.iter().all(|r| animated(*r).is_some()) => {
            rows
        }
        Some(_) => {
            return err(
                "lower-keyframes",
                format!("`{}` cannot be in a keyframe: keyframes animate `translate`, `scale`, `rotate`, `opacity` and the colours, which motion runs without layout (LLP 1002, LLP 1062), and a path's `stroke-start` and `stroke-end` (LLP 1065)", a.name),
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
    let literal = match constant(&a.value, fns) {
        Expr::Str(s, _) => StyleValue::Text(s.clone()),
        other => match values::numeric_literal(other) {
            Some(n) => StyleValue::Number(n),
            None => {
                return err(
                    "lower-keyframes",
                    format!("`{}` in a keyframe is a number or a string, written or returned by a function without parameters", a.name),
                    a.span,
                )
            }
        },
    };
    let (mut out, mut dark) = (Vec::new(), Vec::new());
    for row in rows {
        let mut style = StyleProps::default();
        if let Err(e) = style.set_dynamic(*row, &literal) {
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
        let property = animated(*row).expect("an animated row");
        let value = match style.get(*row) {
            RowValue::ColorValue(ColorValue::Fixed(c)) => {
                exact_motion::Value::rgba8([c.r(), c.g(), c.b(), c.a()])
            }
            // The host's appearance picks one when it paints (LLP 1062 D9).
            RowValue::ColorValue(ColorValue::LightDark(l, d)) => {
                dark.push((
                    property,
                    exact_motion::Value::rgba8([d.r(), d.g(), d.b(), d.a()]),
                ));
                exact_motion::Value::rgba8([l.r(), l.g(), l.b(), l.a()])
            }
            // `none` and `currentcolor` do not interpolate (SVG's `<paint>`).
            RowValue::Enum(keyword) => {
                return err(
                    "lower-keyframes",
                    format!(
                    "`{}=\"{keyword}\"` cannot be in a keyframe: a keyframe's paint is a colour",
                    a.name
                ),
                    a.span,
                )
            }
            _ => {
                targets(&style)
                    .into_iter()
                    .chain(stroke_targets(&style))
                    .find(|(p, _)| *p == property)
                    .expect("targets name every compositor row")
                    .1
            }
        };
        out.push((property, value));
    }
    Ok(Setting::Value(out, dark))
}

/// An `animation` value with every literal resolved to the row's text. A
/// computed value is refused: which keyframes play is known when the app
/// compiles, and a condition may still choose between literals.
/// `exit-animation` (LLP 1063) resolves the same way, and must end: its node
/// is removed when it does.
pub(crate) fn animation_value(
    value: &Expr,
    table: &Table,
    row: StyleId,
) -> Result<Expr, LowerError> {
    let attr = if row == StyleId::ExitAnimation {
        "exit-animation"
    } else {
        "animation"
    };
    Ok(match value {
        Expr::Str(text, span) => {
            let parsed = Animations::parse(&format!("{text}{}", table.rules))
                .or_else(|e| err(e_id(&e), message(attr, text, &e, table), *span))?;
            if row == StyleId::ExitAnimation && parsed.validate_ending().is_err() {
                return err(
                    "lower-exit-endless",
                    format!("`exit-animation=\"{text}\"` never ends (`infinite` or `paused`): a leaving node stays until its exit ends"),
                    *span,
                );
            }
            Expr::Str(parsed.text(), *span)
        }
        // A class choice's side that leaves the row unset.
        Expr::None(_) => value.clone(),
        Expr::Template(parts, span) => animation_template(parts, *span, table, attr)?,
        Expr::Ternary(c, yes, no, span) => Expr::Ternary(
            c.clone(),
            Box::new(animation_value(yes, table, row)?),
            Box::new(animation_value(no, table, row)?),
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
            some: Box::new(animation_value(some, table, row)?),
            none: Box::new(animation_value(none, table, row)?),
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
            body: Box::new(animation_value(body, table, row)?),
            span: *span,
        },
        other => {
            return err(
                "lower-animation-literal",
                format!("`{attr}` is literal text, or a condition choosing between literals: its keyframes are resolved when the app compiles"),
                other.span(),
            )
        }
    })
}

/// A template whose only interpolations are times (LLP 1062 D7): each
/// `${…}` is followed by `ms` or `s`, so it can only be a duration or a
/// delay, and the names stay literal. The shorthand is checked here with
/// every time at zero, and the rules it names are appended to it, so the row
/// the app computes at run time carries its keyframes like a literal's.
fn animation_template(
    parts: &[TemplatePart],
    span: contract_syntax::Span,
    table: &Table,
    attr: &str,
) -> Result<Expr, LowerError> {
    let mut probe = String::new();
    for (i, part) in parts.iter().enumerate() {
        match part {
            TemplatePart::Text(text) => probe.push_str(text),
            TemplatePart::Expr(e) => {
                let unit = match parts.get(i + 1) {
                    Some(TemplatePart::Text(next)) => ["ms", "s"].into_iter().find(|u| {
                        next.strip_prefix(u)
                            .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', ',']))
                    }),
                    _ => None,
                };
                if unit.is_none() {
                    return err(
                        "lower-animation-literal",
                        "`animation` interpolates only times: write `${…}ms` or `${…}s` for a duration or delay; its keyframes are resolved when the app compiles",
                        e.span(),
                    );
                }
                probe.push('0');
            }
        }
    }
    let parsed = Animations::parse(&format!("{probe}{}", table.rules))
        .or_else(|e| err(e_id(&e), message(attr, &probe, &e, table), span))?;
    if attr == "exit-animation" && parsed.validate_ending().is_err() {
        return err(
            "lower-exit-endless",
            format!("`exit-animation=\"{probe}\"` never ends (`infinite` or `paused`): a leaving node stays until its exit ends"),
            span,
        );
    }
    let mut rules = String::new();
    for (i, a) in parsed.0.iter().enumerate() {
        if parsed.0[..i].iter().all(|b| b.keyframes != a.keyframes) {
            rules.push(' ');
            rules.push_str(&a.keyframes.rule(&a.keyframes.name));
        }
    }
    let mut parts = parts.to_vec();
    parts.push(TemplatePart::Text(rules));
    Ok(Expr::Template(parts, span))
}

fn e_id(e: &ParseError) -> &'static str {
    match e {
        ParseError::UnknownKeyframes(_) => "lower-unknown-keyframes",
        _ => "lower-attr-value",
    }
}

fn message(attr: &str, text: &str, e: &ParseError, table: &Table) -> String {
    match e {
        ParseError::UnknownKeyframes(name) => {
            let declared: Vec<String> = table.by_name.keys().map(|n| format!("`{n}`")).collect();
            format!(
                "`{attr}=\"{text}\"` names `{name}`, but no `keyframes {name}` is declared; {}",
                if declared.is_empty() {
                    "no keyframes are declared".to_string()
                } else {
                    format!("declared: {}", declared.join(", "))
                }
            )
        }
        ParseError::SpringInAnimation => format!(
            "`{attr}=\"{text}\"`: `spring()` is a `transition` extension; an animation takes a CSS easing"
        ),
        other => format!(
            "`{attr}=\"{text}\"` is not a CSS `animation` shorthand (name, duration, easing, delay, iteration count, direction, fill mode, play state): {other:?}"
        ),
    }
}
