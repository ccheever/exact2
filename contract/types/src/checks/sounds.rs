//! `playSound(src, at=, gain=, group=)`, `playSounds(hits)` and
//! `stopSounds(group=)` (LLP 1096 D2). Each is checked here, before the
//! generic loop, whose `infer` would refuse a named argument as a canvas
//! binding (`type-named-argument`).

use super::{err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span, UnOp};
use std::collections::BTreeSet;

const PLAY: &str = "`playSound(\"assets/kit/hat.wav\", at=performanceNow() + 100, gain=0.8, group=\"hat\")`: a declared sound, then `at=` (runner milliseconds, now if left out), `gain=` (0–1, 1 if left out) and `group=` by name";
const HITS: &str = "`playSounds(hits)` takes a list of a shape whose fields are, in order, `src: string`, `at: number`, `gain: number` and `group: string` (`shape Hit` with those four)";
const STOP: &str = "`stopSounds()` ends every voice still sounding or waiting; `stopSounds(group=\"hat\")` ends that group's";

/// The three commands' arguments.
pub(super) fn args(
    name: &str,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    match name {
        "playSound" => play(args, scope, shapes, span),
        "playSounds" => hits(args, scope, shapes, span),
        _ => stop(args, scope, shapes),
    }
}

fn play(args: &[Expr], scope: &Scope, shapes: &Shapes, span: Span) -> Result<(), TypeError> {
    let (src, named) = match args {
        [src, named @ ..] if !matches!(src, Expr::NamedArg(..)) => (src, named),
        _ => return err("type-play-sound", PLAY, span),
    };
    if !matches!(infer(src, scope, shapes)?, Ty::String) {
        return err(
            "type-play-sound",
            format!("the sound is a string, its path: {PLAY}"),
            src.span(),
        );
    }
    // A literal names a declared sound here; a computed one is the runner's
    // to drop (D5), since drums chooses a track's sound from data.
    if let Expr::Str(path, at) = src {
        if !shapes.sounds.contains(path) {
            return err(
                "type-sound-undeclared",
                undeclared(path, &shapes.sounds),
                *at,
            );
        }
    }
    let mut seen = BTreeSet::new();
    for arg in named {
        let Expr::NamedArg(option, value, at) = arg else {
            return err(
                "type-play-sound",
                format!("after the sound, each argument is named: {PLAY}"),
                arg.span(),
            );
        };
        let want = match option.as_str() {
            "at" | "gain" => Ty::Number,
            "group" => Ty::String,
            other => {
                return err(
                    "type-play-sound",
                    format!("`playSound` has no option `{other}`: {PLAY}"),
                    *at,
                )
            }
        };
        if !seen.insert(option.as_str()) {
            return err(
                "type-play-sound",
                format!("`{option}=` is given twice"),
                *at,
            );
        }
        let t = infer(value, scope, shapes)?;
        if want.unify(&t).is_none() {
            return err(
                "type-play-sound",
                format!("`{option}=` is a {want}, not `{t}`: {PLAY}"),
                value.span(),
            );
        }
        // A literal gain is refused outside 0–1, as a literal `volume` is; a
        // computed one is clamped by the runner (D2).
        if option == "gain" {
            if let Some(g) = literal(value) {
                if !(0.0..=1.0).contains(&g) {
                    return err(
                        "type-play-sound",
                        format!("`gain={g}` is outside 0–1: a gain is a linear multiplier, 1 the file's own level"),
                        value.span(),
                    );
                }
            }
        }
    }
    Ok(())
}

fn hits(args: &[Expr], scope: &Scope, shapes: &Shapes, span: Span) -> Result<(), TypeError> {
    let [list] = args else {
        return err("type-play-sound", HITS, span);
    };
    if matches!(list, Expr::NamedArg(..)) {
        return err("type-play-sound", HITS, list.span());
    }
    let shape = match infer(list, scope, shapes)? {
        Ty::List(item) => match *item {
            Ty::Unknown => return Ok(()),
            Ty::Record(name) => name,
            other => {
                return err(
                    "type-play-sound",
                    format!("the list holds `{other}`: {HITS}"),
                    list.span(),
                )
            }
        },
        other => {
            return err(
                "type-play-sound",
                format!("`playSounds` takes a list, not `{other}`: {HITS}"),
                list.span(),
            )
        }
    };
    // The runner reads a hit's fields by position, so the shape is checked
    // whole, in order: the record carries no names at runtime.
    let want = [
        ("src", Ty::String),
        ("at", Ty::Number),
        ("gain", Ty::Number),
        ("group", Ty::String),
    ];
    let fields = shapes
        .map
        .get(&shape)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let fits = fields.len() == want.len()
        && fields
            .iter()
            .zip(&want)
            .all(|((name, ty), (want_name, want_ty))| name == want_name && ty == want_ty);
    if !fits {
        let has = fields
            .iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect::<Vec<_>>()
            .join(", ");
        return err(
            "type-play-sound",
            format!("`{shape}` has the fields ({has}): {HITS}"),
            list.span(),
        );
    }
    Ok(())
}

fn stop(args: &[Expr], scope: &Scope, shapes: &Shapes) -> Result<(), TypeError> {
    let mut seen = false;
    for arg in args {
        let Expr::NamedArg(option, value, at) = arg else {
            return err("type-play-sound", STOP, arg.span());
        };
        if option != "group" {
            return err(
                "type-play-sound",
                format!("`stopSounds` has no option `{option}`: {STOP}"),
                *at,
            );
        }
        if std::mem::replace(&mut seen, true) {
            return err("type-play-sound", "`group=` is given twice", *at);
        }
        let t = infer(value, scope, shapes)?;
        if Ty::String.unify(&t).is_none() {
            return err(
                "type-play-sound",
                format!("`group=` is a string, not `{t}`: {STOP}"),
                value.span(),
            );
        }
    }
    Ok(())
}

/// A number written in the source, negated or not.
fn literal(e: &Expr) -> Option<f64> {
    match e {
        Expr::Number(n, _) => Some(*n),
        Expr::Unary(UnOp::Neg, inner, _) => literal(inner).map(|n| -n),
        _ => None,
    }
}

/// The refusal for an undeclared literal, naming the declared sounds
/// nearest it, by edit distance.
fn undeclared(path: &str, declared: &BTreeSet<String>) -> String {
    if declared.is_empty() {
        return format!(
            "`{path}` is not a declared sound; declare it at the top level: `sound \"{path}\"`"
        );
    }
    let mut near: Vec<&String> = declared.iter().collect();
    near.sort_by_key(|d| distance(path, d));
    let near = near
        .iter()
        .take(3)
        .map(|d| format!("`{d}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("`{path}` is not a declared sound (declared, nearest first: {near}); declare it at the top level: `sound \"{path}\"`")
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (diagonal + usize::from(ca != *cb))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}
