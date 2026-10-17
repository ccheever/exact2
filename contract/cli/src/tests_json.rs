//! The tests as JSON for the driver (LLP 1012).

use contract_syntax::{Expr, Step, TapForm, TestDecl};

/// A JSON string.
fn q(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The tests as JSON for the driver: `[{"name":…,"steps":[{"op":…}]}]`,
/// written by hand — no serde anywhere in the runtime (LLP 1012).
pub fn tests_json(tests: &[TestDecl]) -> String {
    let mut s = String::from("[");
    for (ti, t) in tests.iter().enumerate() {
        if ti > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        q(&t.name, &mut s);
        s.push_str(",\"steps\":[");
        for (si, step) in t.steps.iter().enumerate() {
            if si > 0 {
                s.push(',');
            }
            let line = step.span().line;
            match step {
                Step::Tap {
                    target,
                    form,
                    modifiers,
                    ..
                } => {
                    s.push_str("{\"op\":\"tap\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"form\":");
                    match form {
                        TapForm::Press => s.push_str("\"press\""),
                        TapForm::Hover => s.push_str("\"hover\""),
                        TapForm::Dblclick => s.push_str("\"dblclick\""),
                        TapForm::Contextmenu => s.push_str("\"contextmenu\""),
                        TapForm::Into(key) => {
                            s.push_str("\"into\",\"key\":");
                            q(key, &mut s);
                        }
                        TapForm::Pinch { scale, at } => {
                            s.push_str(&format!("\"pinch\",\"scale\":{scale}"));
                            if let Some((x, y)) = at {
                                s.push_str(&format!(",\"at\":[{x},{y}]"));
                            }
                        }
                        // @ref LLP 1098 D10 — the platform's action, never a press.
                        TapForm::MediaSession { action, seconds } => {
                            s.push_str("\"mediasession\",\"action\":");
                            q(action, &mut s);
                            if let Some(n) = seconds {
                                s.push_str(&format!(",\"seconds\":{n}"));
                            }
                        }
                    }
                    if !modifiers.is_empty() {
                        s.push_str(",\"modifiers\":");
                        q(modifiers, &mut s);
                    }
                }
                Step::Drag {
                    target,
                    dx,
                    dy,
                    to,
                    from,
                    mouse,
                    press,
                    over,
                    hold,
                    during,
                    ..
                } => {
                    s.push_str("{\"op\":\"drag\",\"target\":");
                    q(target, &mut s);
                    match to {
                        // LLP 1094 D12: the driver's `drag to`.
                        Some((to, at)) => {
                            s.push_str(",\"to\":");
                            q(to, &mut s);
                            if let Some((x, y)) = at {
                                s.push_str(&format!(",\"at\":[{x},{y}]"));
                            }
                        }
                        None => s.push_str(&format!(",\"dx\":{dx},\"dy\":{dy}")),
                    }
                    if let Some((x, y)) = from {
                        s.push_str(&format!(",\"from\":[{x},{y}]"));
                    }
                    if *mouse {
                        s.push_str(",\"mouse\":true");
                    }
                    for (name, ms) in [("press", press), ("over", over), ("hold", hold)] {
                        if let Some(ms) = ms {
                            s.push_str(&format!(",\"{name}\":{ms}"));
                        }
                    }
                    if !during.is_empty() {
                        s.push_str(",\"during\":[");
                        for (i, op) in during.iter().enumerate() {
                            if i > 0 {
                                s.push(',');
                            }
                            q(op, &mut s);
                        }
                        s.push(']');
                    }
                }
                Step::Size { width, height, .. } => {
                    s.push_str(&format!(
                        "{{\"op\":\"size\",\"width\":{width},\"height\":{height}"
                    ));
                }
                Step::Epoch { value, .. } => {
                    s.push_str("{\"op\":\"epoch\",\"value\":");
                    q(value, &mut s);
                }
                Step::TimeZone { zone, .. } => {
                    s.push_str("{\"op\":\"time-zone\",\"value\":");
                    q(zone, &mut s);
                }
                Step::Locale { tag, .. } => {
                    s.push_str("{\"op\":\"locale\",\"value\":");
                    q(tag, &mut s);
                }
                Step::Seed { seed, .. } => {
                    s.push_str(&format!("{{\"op\":\"seed\",\"value\":{seed}"));
                }
                Step::Fresh { seed, .. } => {
                    s.push_str("{\"op\":\"fresh\",\"seed\":");
                    match seed {
                        Some(path) => q(path, &mut s),
                        None => s.push_str("null"),
                    }
                }
                Step::Type {
                    target,
                    text,
                    append,
                    ..
                } => {
                    s.push_str("{\"op\":\"type\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"text\":");
                    q(text, &mut s);
                    s.push_str(&format!(",\"append\":{append}"));
                }
                Step::Reload { .. } => s.push_str("{\"op\":\"reload\""),
                Step::Close { .. } => s.push_str("{\"op\":\"close\""),
                Step::Back { .. } => s.push_str("{\"op\":\"back\""),
                Step::BeforeData { .. } => s.push_str("{\"op\":\"before-data\""),
                Step::FailFetch { prefix, times, .. } => {
                    s.push_str("{\"op\":\"fail-fetch\",\"prefix\":");
                    q(prefix, &mut s);
                    match times {
                        Some(n) => s.push_str(&format!(",\"times\":{n}")),
                        None => s.push_str(",\"times\":null"),
                    }
                }
                Step::PassFetch { prefix, .. } => {
                    s.push_str("{\"op\":\"pass-fetch\",\"prefix\":");
                    q(prefix, &mut s);
                }
                // @ref LLP 1116.006 D8 — the body as JSON text, the persona as `as`.
                Step::Request {
                    method,
                    path,
                    body,
                    persona,
                    status,
                    ..
                } => {
                    s.push_str("{\"op\":\"request\",\"method\":");
                    q(method, &mut s);
                    s.push_str(",\"path\":");
                    q(path, &mut s);
                    for (key, value) in [("body", body), ("as", persona)] {
                        s.push_str(&format!(",\"{key}\":"));
                        match value {
                            Some(v) => q(v, &mut s),
                            None => s.push_str("null"),
                        }
                    }
                    // LLP 1116.010 D3: only when written, so a plain request's line is as it was.
                    if let Some(status) = status {
                        s.push_str(&format!(",\"expect\":{status}"));
                    }
                }
                Step::Resize { width, height, .. } => {
                    s.push_str(&format!(
                        "{{\"op\":\"resize\",\"width\":{width},\"height\":{height}"
                    ));
                }
                Step::Key {
                    target,
                    key,
                    phase,
                    duration,
                    ..
                } => {
                    s.push_str("{\"op\":\"key\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"key\":");
                    q(key, &mut s);
                    if let Some(phase) = phase {
                        s.push_str(",\"phase\":");
                        q(phase, &mut s);
                    }
                    if let Some(ms) = duration {
                        s.push_str(&format!(",\"for\":{ms}"));
                    }
                }
                Step::Pick { target, paths, .. } => {
                    s.push_str("{\"op\":\"pick\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"paths\":[");
                    for (i, path) in paths.iter().enumerate() {
                        if i > 0 {
                            s.push(',');
                        }
                        q(path, &mut s);
                    }
                    s.push(']');
                }
                Step::Clipboard {
                    target, edit, text, ..
                } => {
                    s.push_str("{\"op\":\"clipboard\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"edit\":");
                    q(edit, &mut s);
                    s.push_str(",\"text\":");
                    q(text, &mut s);
                }
                Step::Clock { arg, .. } => {
                    s.push_str("{\"op\":\"clock\",\"arg\":");
                    q(arg, &mut s);
                }
                Step::Screenshot { path, .. } => {
                    s.push_str("{\"op\":\"screenshot\",\"path\":");
                    q(path, &mut s);
                }
                Step::ExpectTree {
                    target, present, ..
                } => {
                    s.push_str("{\"op\":\"expect-tree\",\"target\":");
                    q(target, &mut s);
                    s.push_str(&format!(",\"present\":{present}"));
                }
                Step::ExpectDisabled {
                    target, disabled, ..
                } => {
                    s.push_str("{\"op\":\"expect-disabled\",\"target\":");
                    q(target, &mut s);
                    s.push_str(&format!(",\"disabled\":{disabled}"));
                }
                Step::ExpectText { target, value, .. } => {
                    s.push_str("{\"op\":\"expect-text\",\"target\":");
                    q(target, &mut s);
                    s.push_str(",\"value\":");
                    q(value, &mut s);
                }
                Step::ExpectSound {
                    src,
                    present,
                    at,
                    gain,
                    ends,
                    playback_rate,
                    pan,
                    by,
                    ..
                } => {
                    s.push_str("{\"op\":\"expect-sound\",\"src\":");
                    q(src, &mut s);
                    s.push_str(&format!(",\"present\":{present}"));
                    for (key, n) in [
                        ("at", at),
                        ("gain", gain),
                        ("ends", ends),
                        ("playbackRate", playback_rate),
                        ("pan", pan),
                    ] {
                        if let Some(n) = n {
                            s.push_str(&format!(",\"{key}\":{n}"));
                        }
                    }
                    if let Some(by) = by {
                        s.push_str(",\"by\":");
                        q(by, &mut s);
                    }
                }
                Step::ExpectMediaSession { field, value, .. } => {
                    s.push_str("{\"op\":\"expect-mediasession\",\"field\":");
                    q(field, &mut s);
                    s.push_str(",\"value\":");
                    match value {
                        Some(v) => q(v, &mut s),
                        None => s.push_str("null"),
                    }
                }
                Step::ExpectMediaSessionAction {
                    action, present, ..
                } => {
                    s.push_str("{\"op\":\"expect-mediasession\",\"action\":");
                    q(action, &mut s);
                    s.push_str(&format!(",\"present\":{present}"));
                }
                // @ref LLP 1125 §4 — the driver's `prefer midi.*` and `tap midi`, and `state.midi.sent`.
                Step::Midi { step, .. } => midi_step(step, &mut s),
                Step::ExpectState { name, value, .. } => {
                    s.push_str("{\"op\":\"expect-state\",\"name\":");
                    q(name, &mut s);
                    s.push_str(",\"value\":");
                    match value {
                        Expr::Number(n, _) => s.push_str(&format!("{n}")),
                        Expr::Str(t, _) => q(t, &mut s),
                        Expr::Bool(b, _) => s.push_str(&format!("{b}")),
                        Expr::List(items, _) if items.is_empty() => s.push_str("[]"),
                        _ => s.push_str("null"),
                    }
                }
            }
            s.push_str(&format!(",\"line\":{line}}}"));
        }
        s.push_str("]}");
    }
    s.push(']');
    s
}

/// A MIDI step's JSON: `prefer` with its `midi` facts, `tap-midi`, or
/// `expect-midi`, the bytes as numbers.
fn midi_step(step: &contract_syntax::MidiStep, s: &mut String) {
    use contract_syntax::MidiStep;
    let bytes = |data: &[u8]| {
        let b: Vec<String> = data.iter().map(|b| b.to_string()).collect();
        format!("[{}]", b.join(","))
    };
    let names = |list: &[String], s: &mut String| {
        s.push('[');
        for (i, n) in list.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            q(n, s);
        }
        s.push(']');
    };
    match step {
        MidiStep::Prefer {
            state,
            inputs,
            outputs,
        } => {
            s.push_str("{\"op\":\"prefer-midi\"");
            if let Some(state) = state {
                s.push_str(",\"state\":");
                q(state, s);
            }
            if let Some(inputs) = inputs {
                s.push_str(",\"inputs\":");
                names(inputs, s);
            }
            if let Some(outputs) = outputs {
                s.push_str(",\"outputs\":");
                names(outputs, s);
            }
        }
        MidiStep::Tap { input, data } => {
            s.push_str("{\"op\":\"tap-midi\",\"input\":");
            q(input, s);
            s.push_str(&format!(",\"data\":{}", bytes(data)));
        }
        MidiStep::TapClock { input, bpm, ms } => {
            s.push_str("{\"op\":\"tap-midi\",\"input\":");
            q(input, s);
            s.push_str(&format!(",\"clock\":{{\"bpm\":{bpm},\"ms\":{ms}}}"));
        }
        MidiStep::Expect {
            output,
            data,
            at,
            present,
        } => {
            s.push_str("{\"op\":\"expect-midi\",\"output\":");
            q(output, s);
            s.push_str(&format!(",\"data\":{},\"present\":{present}", bytes(data)));
            if let Some(at) = at {
                s.push_str(&format!(",\"at\":{at}"));
            }
        }
    }
}
