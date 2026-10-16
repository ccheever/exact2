//! `requestMIDIAccess()` and `sendMIDI(output, data, at=)` (LLP 1125 D6,
//! D7). A literal `data` is held to MIDI 1.0 here; a computed one is the
//! runner's to drop and journal.

use super::{err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span, UnOp};

const SEND: &str = "`sendMIDI(output, [144, 60, 100], at=performanceNow() + 100)` (144 is 0x90, a note-on on channel 1): an output's `id` from `exactMIDI().outputs`, the bytes of one or more whole MIDI 1.0 messages (no running status, no SysEx), then `at=` (runner milliseconds, now if left out) by name";

/// The two commands' arguments.
pub(super) fn args(
    name: &str,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    if name == "requestMIDIAccess" {
        if !args.is_empty() {
            return err(
                "type-request-midi-access",
                "`requestMIDIAccess()` takes no arguments: it asks for the MIDI ports `exactMIDI()` answers",
                span,
            );
        }
        return Ok(());
    }
    let (output, data, named) = match args {
        [output, data, named @ ..]
            if !matches!(output, Expr::NamedArg(..)) && !matches!(data, Expr::NamedArg(..)) =>
        {
            (output, data, named)
        }
        _ => return err("type-send-midi", SEND, span),
    };
    if !infer(output, scope, shapes)?.is_string() {
        return err(
            "type-send-midi",
            format!("the output is a string, its `id`: {SEND}"),
            output.span(),
        );
    }
    let t = infer(data, scope, shapes)?;
    if Ty::List(Box::new(Ty::Number)).unify(&t).is_none() {
        return err(
            "type-send-midi",
            format!("the data is a `number[]`, not `{t}`: {SEND}"),
            data.span(),
        );
    }
    if let Expr::List(items, _) = data {
        let bytes: Option<Vec<f64>> = items.iter().map(literal).collect();
        if let Some(why) = bytes.as_deref().and_then(malformed) {
            return err("type-send-midi", format!("{why}: {SEND}"), data.span());
        }
    }
    let mut seen = false;
    for arg in named {
        let Expr::NamedArg(option, value, at) = arg else {
            return err(
                "type-send-midi",
                format!("after the data, each argument is named: {SEND}"),
                arg.span(),
            );
        };
        if option != "at" {
            return err(
                "type-send-midi",
                format!("`sendMIDI` has no option `{option}`: {SEND}"),
                *at,
            );
        }
        if std::mem::replace(&mut seen, true) {
            return err("type-send-midi", "`at=` is given twice", *at);
        }
        let t = infer(value, scope, shapes)?;
        if Ty::Number.unify(&t).is_none() {
            return err(
                "type-send-midi",
                format!("`at=` is a number, not `{t}`: {SEND}"),
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

/// Why `bytes` are not whole MIDI 1.0 messages, the runner's rule
/// (`exact_runner::midi::message`): each a whole number 0–255; a status
/// byte, then its data bytes (below 0x80); no running status; SysEx and the
/// undefined status bytes refused.
fn malformed(bytes: &[f64]) -> Option<String> {
    if bytes.is_empty() {
        return Some("the data holds no message".into());
    }
    if let Some(b) = bytes
        .iter()
        .find(|b| !(b.fract() == 0.0 && (0.0..=255.0).contains(*b)))
    {
        return Some(format!("{b} is not a byte, a whole number 0–255"));
    }
    let mut i = 0;
    while i < bytes.len() {
        let status = bytes[i] as u8;
        let length = match status {
            0x80..=0xBF | 0xE0..=0xEF | 0xF2 => 3,
            0xC0..=0xDF | 0xF1 | 0xF3 => 2,
            0xF6 | 0xF8 | 0xFA..=0xFC | 0xFE | 0xFF => 1,
            0xF0 | 0xF7 => return Some("SysEx is not sent".into()),
            0xF4 | 0xF5 | 0xF9 | 0xFD => {
                return Some(format!("0x{status:02X} is an undefined status byte"))
            }
            _ => return Some(format!(
                "byte {} is {status}, a data byte where a status byte belongs (no running status)",
                i + 1
            )),
        };
        if i + length > bytes.len() {
            return Some(format!(
                "the message at byte {} needs {length} bytes",
                i + 1
            ));
        }
        if let Some(k) = (i + 1..i + length).find(|&k| bytes[k] >= 128.0) {
            return Some(format!(
                "byte {} is {}, where a data byte (0–127) belongs",
                k + 1,
                bytes[k]
            ));
        }
        i += length;
    }
    None
}
