//! A `sequence` (LLP 1125 D2): its gate and key as a task's, and its
//! `play(tracks, bpm=, from=, countIn=, clock=, clockOut=)`. The runner reads
//! a track's and a step's fields by position, so their shapes are checked
//! whole, in order, as `playSounds`'s hit is (a record carries no names at
//! runtime). Only the root declares one, as only the root has a clock.

use super::super::{infer, Scope, Shapes, Sink, Ty, TypeError};
use contract_syntax::{Component, Expr, Span, UnOp};

const TRACK: &str = "a track is a shape whose fields are, in order, `beats: number` (its length, which it loops at), `loops: number`, `offset: number`, `output: string` and `steps: Step[]`";
const STEP: &str = "a step is a shape whose fields are, in order, `beat: number`, `src: string`, `gain: number`, `group: string`, `note: number`, `velocity: number`, `channel: number`, `duration: number` (beats until the note-off) and `haptic: string`, then optionally `playbackRate: number` and then `pan: number`";
const PLAY: &str = "`play(tracks, bpm=120, from=-4, countIn=clicks, ended=finished)`: a list of tracks, then `bpm=` (beats a minute), `from=` (the beat at the start; below 0 a count-in), `countIn=` (steps played once over `[from, 0)`), `clock=` (a MIDI input to follow instead: `bpm=` is then its tempo until enough ticks came), `clockOut=` and `ended=` by name";

/// Every sequence of `c`; `root` is whether `c` is the root component.
pub(crate) fn check(c: &Component, root: bool, scope: &Scope, shapes: &Shapes, sink: &mut Sink) {
    if !root {
        if let Some(q) = c.sequences.first() {
            sink.push(TypeError {
                id: "type-child-resource",
                message: format!(
                    "only the root, the root file's first component, may declare a `sequence`; `{}` is a child. Move `sequence {}` into the root and pass what `{}` needs as props",
                    c.name, q.name, c.name
                ),
                span: q.span,
            });
        }
        return;
    }
    for q in &c.sequences {
        super::check_gate(
            q.gate.as_ref(),
            q.key.as_ref(),
            "sequence",
            scope,
            shapes,
            sink,
        );
        if let Err(e) = play(q, scope, shapes) {
            sink.push(e);
        }
    }
}

fn err<T>(message: impl Into<String>, span: Span) -> Result<T, TypeError> {
    Err(TypeError {
        id: "type-sequence-play",
        message: message.into(),
        span,
    })
}

fn play(q: &contract_syntax::Sequence, scope: &Scope, shapes: &Shapes) -> Result<(), TypeError> {
    let tracks = &q.tracks;
    match infer(tracks, scope, shapes)? {
        Ty::List(item) => match *item {
            Ty::Unknown => {}
            Ty::Record(name) => track(&name, shapes, tracks.span())?,
            other => {
                return err(
                    format!("the tracks are a list of `{other}`: {TRACK}"),
                    tracks.span(),
                )
            }
        },
        other => {
            return err(
                format!("`play` takes a list of tracks, not `{other}`: {PLAY}"),
                tracks.span(),
            )
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for (option, value, at) in &q.options {
        if seen.contains(&option.as_str()) {
            return err(format!("`{option}=` is given twice"), *at);
        }
        seen.push(option);
        let t = infer(value, scope, shapes)?;
        match option.as_str() {
            "bpm" | "from" => {
                if Ty::Number.unify(&t).is_none() {
                    return err(
                        format!("`{option}=` is a number, not `{t}`: {PLAY}"),
                        value.span(),
                    );
                }
                let n = literal(value);
                if option == "bpm" && n.is_some_and(|n| n <= 0.0) {
                    return err(
                        format!(
                            "`bpm={}` is not a tempo: beats a minute are above 0",
                            n.unwrap_or(0.0)
                        ),
                        value.span(),
                    );
                }
            }
            "countIn" => match t {
                Ty::List(item) => match *item {
                    Ty::Unknown => {}
                    Ty::Record(name) => step(&name, shapes, value.span())?,
                    other => {
                        return err(
                            format!("`countIn=` is a list of `{other}`: {STEP}"),
                            value.span(),
                        )
                    }
                },
                other => {
                    return err(
                        format!("`countIn=` is a list of steps, not `{other}`: {STEP}"),
                        value.span(),
                    )
                }
            },
            "clock" | "clockOut" => {
                if Ty::String.unify(&t).is_none() {
                    return err(
                        format!("`{option}=` is a MIDI port's id, a string, not `{t}`"),
                        value.span(),
                    );
                }
            }
            other => return err(format!("`play` has no option `{other}`: {PLAY}"), *at),
        }
    }
    if !seen.contains(&"bpm") && !seen.contains(&"clock") {
        return err(
            format!("`play` needs `bpm=` or `clock=`: {PLAY}"),
            q.tracks.span(),
        );
    }
    Ok(())
}

/// Whether shape `name`'s fields are `want`'s, by name and type, in order:
/// all of them, or only the first `required`.
fn fits(shapes: &Shapes, name: &str, want: &[(&str, Ty)], required: usize) -> Result<(), String> {
    let fields = shapes.map.get(name).map(Vec::as_slice).unwrap_or_default();
    let ok = (required..=want.len()).contains(&fields.len())
        && fields
            .iter()
            .zip(want)
            .all(|((n, ty), (wn, wt))| n == wn && ty == wt);
    if ok {
        return Ok(());
    }
    let has = fields
        .iter()
        .map(|(n, ty)| format!("{n}: {ty}"))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!("`{name}` has the fields ({has})"))
}

fn track(name: &str, shapes: &Shapes, span: Span) -> Result<(), TypeError> {
    let steps = match shapes.map.get(name).and_then(|f| f.get(4)) {
        Some((field, Ty::List(item))) if field == "steps" => match &**item {
            Ty::Record(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    };
    let list = Ty::List(Box::new(Ty::Record(steps.clone().unwrap_or_default())));
    let want = [
        ("beats", Ty::Number),
        ("loops", Ty::Number),
        ("offset", Ty::Number),
        ("output", Ty::String),
        ("steps", list),
    ];
    if let Err(has) = fits(shapes, name, &want, want.len()) {
        return err(format!("{has}: {TRACK}"), span);
    }
    step(&steps.unwrap_or_default(), shapes, span)
}

fn step(name: &str, shapes: &Shapes, span: Span) -> Result<(), TypeError> {
    let want = [
        ("beat", Ty::Number),
        ("src", Ty::String),
        ("gain", Ty::Number),
        ("group", Ty::String),
        ("note", Ty::Number),
        ("velocity", Ty::Number),
        ("channel", Ty::Number),
        ("duration", Ty::Number),
        ("haptic", Ty::String),
        ("playbackRate", Ty::Number),
        ("pan", Ty::Number),
    ];
    match fits(shapes, name, &want, 9) {
        // A step carries both of D10's fields or neither.
        Ok(()) if shapes.map.get(name).map_or(0, Vec::len) != 10 => Ok(()),
        Ok(()) => err(
            format!("`{name}` has `playbackRate` without `pan`: {STEP}"),
            span,
        ),
        Err(has) => err(format!("{has}: {STEP}"), span),
    }
}

/// A number written in the source, negated or not.
fn literal(e: &Expr) -> Option<f64> {
    match e {
        Expr::Number(n, _) => Some(*n),
        Expr::Unary(UnOp::Neg, inner, _) => literal(inner).map(|n| -n),
        _ => None,
    }
}
