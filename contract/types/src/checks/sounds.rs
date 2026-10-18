//! `playSound(src, at=, gain=, group=, playbackRate=, pan=)`,
//! `playSounds(hits)` and `stopSounds(group=)` (LLP 1096 D2; the rate and
//! pan, LLP 1125 D10). Each is checked here, before the
//! generic loop, whose `infer` would refuse a named argument as a canvas
//! binding (`type-named-argument`).

use super::{err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span, UnOp};
use std::collections::BTreeSet;

const PLAY: &str = "`playSound(\"assets/kit/hat.wav\", at=performanceNow() + 100, gain=0.8, group=\"hat\", playbackRate=1.5, pan=-0.5)`: a declared sound, then `at=` (runner milliseconds, now if left out), `gain=` (0–1, 1 if left out), `group=`, `playbackRate=` (0.25–4, 1 if left out) and `pan=` (−1 left to 1 right, 0 if left out) by name";
const HITS: &str = "`playSounds(hits)` takes a list of a shape whose fields are, in order, `src: string`, `at: number`, `gain: number` and `group: string` (`shape Hit` with those four), then optionally `playbackRate: number` and then `pan: number`";
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
    if !infer(src, scope, shapes)?.is_string() {
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
            "at" | "gain" | "playbackRate" | "pan" => Ty::Number,
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
        // A literal gain, playback rate or pan is refused outside its range,
        // as a literal `volume` is; a computed one is clamped by the runner
        // (D2; LLP 1125 D10).
        let range = match option.as_str() {
            "gain" => Some((
                0.0,
                1.0,
                "0–1: a gain is a linear multiplier, 1 the file's own level",
            )),
            "playbackRate" => Some((
                0.25,
                4.0,
                "0.25–4: 1 is the file's own speed, 2 an octave up and half as long",
            )),
            "pan" => Some((
                -1.0,
                1.0,
                "−1–1: −1 is the left speaker, 0 the centre, 1 the right",
            )),
            _ => None,
        };
        if let (Some((low, high, why)), Some(n)) = (range, literal(value)) {
            if !(low..=high).contains(&n) {
                return err(
                    "type-play-sound",
                    format!("`{option}={n}` is outside {why}"),
                    value.span(),
                );
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
    // whole, in order: the record carries no names at runtime. The first
    // four are required; the rate and then the pan may follow (LLP 1125 D10).
    let want = [
        ("src", Ty::String),
        ("at", Ty::Number),
        ("gain", Ty::Number),
        ("group", Ty::String),
        ("playbackRate", Ty::Number),
        ("pan", Ty::Number),
    ];
    let fields = shapes
        .map
        .get(&shape)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let fits = (4..=want.len()).contains(&fields.len())
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
