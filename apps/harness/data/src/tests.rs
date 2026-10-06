//! The session driven through the `DataSource` API, with the mock model.

use super::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

fn send(h: &mut Harness, source: &str, args: &[&str]) -> bool {
    let mut store = Store::new("", []);
    let args: Vec<Value> = args.iter().map(|a| Value::str(a)).collect();
    match h.answer(&mut store, source, &args).unwrap() {
        Answer::Now(v) => v == value::ack(true),
        Answer::Later(_) => panic!("a send answers now"),
    }
}

fn mock() -> Harness {
    providers::mock::FAST.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut h = Harness::new();
    assert!(send(&mut h, "setModel", &["mock"]));
    h
}

/// Drive until idle, answering every approval with `choice`, and check
/// that an entry once settled never changes again.
fn drive(h: &mut Harness, choice: &str) -> Vec<Entry> {
    let start = Instant::now();
    let mut settled: HashMap<String, Entry> = HashMap::new();
    loop {
        let (entries, approval, busy) = {
            let s = h.shared.lock();
            (s.entries.clone(), s.approval.id.clone(), s.busy())
        };
        for e in &entries {
            if let Some(before) = settled.get(&e.id) {
                assert_eq!(before, e, "a settled entry changed");
            } else if !e.busy {
                settled.insert(e.id.clone(), e.clone());
            }
        }
        if !approval.is_empty() {
            assert!(send(h, "approve", &[&approval, choice]));
        }
        if !busy {
            return entries;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the turn never ended"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn kinds(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|e| e.kind.as_str()).collect()
}

#[test]
fn a_mock_turn_streams_runs_tools_and_settles() {
    let mut h = mock();
    assert!(send(&mut h, "submit", &["hello there"]));
    let entries = drive(&mut h, "yes");
    assert_eq!(
        kinds(&entries),
        [
            "banner",
            "user",
            "assistant",
            "tool",
            "assistant",
            "tool",
            "assistant",
            "tool",
            "assistant"
        ]
    );
    let tools: Vec<(&str, &str)> = entries
        .iter()
        .filter(|e| e.kind == "tool")
        .map(|e| (e.title.as_str(), e.status.as_str()))
        .collect();
    assert_eq!(
        tools,
        [
            ("List(.)", "ok"),
            ("Read(Cargo.toml)", "ok"),
            ("Bash(ls -la)", "ok")
        ]
    );
    assert!(entries.iter().all(|e| !e.busy));
    let first = &entries[2].blocks;
    for kind in ["h2", "p", "li", "code", "quote"] {
        assert!(first.iter().any(|b| b.kind == kind), "no {kind}");
    }
    let read = entries
        .iter()
        .find(|e| e.title.starts_with("Read"))
        .unwrap();
    assert!(read.blocks[0].lines.len() <= tools::SHOWN);
    let s = h.shared.lock();
    assert_eq!(s.phase, "idle");
    assert!(s.tokens_in > 0.0 && s.tokens_out > 0.0);
    // user, then (assistant, results) × 3, then the closing assistant.
    assert_eq!(s.history.len(), 8);
    drop(s);
    // The snapshot converts.
    assert!(matches!(h.session(), Value::Record(_)));
}

#[test]
fn a_denied_edit_settles_denied_and_the_model_hears_it() {
    let mut h = mock();
    assert!(send(&mut h, "submit", &["please edit something"]));
    let entries = drive(&mut h, "no");
    let edit = entries
        .iter()
        .find(|e| e.title.starts_with("Edit("))
        .unwrap();
    assert_eq!(edit.status, "denied");
    let last = entries.last().unwrap();
    assert_eq!(last.kind, "assistant");
    let s = h.shared.lock();
    assert!(s.history.iter().any(|m| m.parts.iter().any(|p| matches!(
        p,
        state::Part::ToolResult { content, .. } if content == "denied by user"
    ))));
}

#[test]
fn an_approved_edit_shows_a_diff() {
    let mut h = mock();
    assert!(send(&mut h, "submit", &["edit please"]));
    let entries = drive(&mut h, "always");
    let edit = entries
        .iter()
        .find(|e| e.title.starts_with("Edit("))
        .unwrap();
    assert_eq!(edit.status, "ok");
    let lines = &edit.blocks[0].lines;
    assert!(lines
        .iter()
        .any(|l| l.runs.iter().any(|r| r.bg == tools::ADD_BG)));
}

#[test]
fn a_failing_stream_becomes_an_error_entry() {
    let mut h = mock();
    assert!(send(&mut h, "submit", &["make an error"]));
    let entries = drive(&mut h, "yes");
    assert_eq!(kinds(&entries), ["banner", "user", "assistant", "error"]);
    assert!(!h.shared.lock().busy());
}

#[test]
fn an_interrupt_settles_what_was_busy() {
    let mut h = mock();
    assert!(send(&mut h, "submit", &["hi"]));
    let start = Instant::now();
    // Wait for the command's approval, then interrupt instead of answering.
    while h.shared.lock().approval.tool != "bash" {
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(send(&mut h, "interrupt", &[]));
    let s = h.shared.lock();
    assert!(!s.busy());
    assert!(s.approval.id.is_empty());
    let last = s.entries.last().unwrap();
    assert_eq!(last.kind, "tool");
    assert!(!last.busy);
    assert_eq!(last.blocks.last().unwrap().runs[0].text, "[interrupted]");
    // The history stays well formed: the call has a result.
    let tail = s.history.last().unwrap();
    assert!(
        matches!(&tail.parts[0], state::Part::ToolResult { content, .. } if content == "interrupted by user")
    );
    let entries = s.entries.clone();
    drop(s);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        h.shared.lock().entries,
        entries,
        "the stopped thread changed nothing"
    );
    assert!(!send(&mut h, "interrupt", &[]));
}

#[test]
fn slash_commands() {
    let mut h = mock();
    for (cmd, kind) in [
        ("/help", "notice"),
        ("/models", "notice"),
        ("/tools", "notice"),
        ("/ascii", "notice"),
        ("/ansi", "notice"),
        ("/unicode", "notice"),
        ("/diff", "notice"),
        ("/image", "image"),
    ] {
        assert!(send(&mut h, "submit", &[cmd]), "{cmd}");
        let s = h.shared.lock();
        let last = s.entries.last().unwrap();
        assert_eq!(last.kind, kind, "{cmd}");
        assert!(!last.busy);
    }
    {
        let s = h.shared.lock();
        let image = s.entries.last().unwrap();
        assert!(std::path::Path::new(&image.image).is_file());
        assert_eq!((image.cols, image.rows), (48.0, 15.0));
    }
    assert!(!send(&mut h, "submit", &["/bogus"]));
    assert_eq!(h.shared.lock().entries.last().unwrap().kind, "error");
    assert!(send(&mut h, "submit", &["/stress 50"]));
    assert!(h.shared.lock().entries.len() > 50);
    assert!(send(&mut h, "submit", &["/clear"]));
    assert_eq!(kinds(&h.shared.lock().entries), ["banner"]);
    assert!(!send(&mut h, "submit", &["/model nope"]));
    assert!(h.shared.lock().toast.contains("nope"));
    // Every entry converts.
    assert!(matches!(h.session(), Value::Record(_)));
}

#[test]
fn bake_style_query_answers_without_side_effects() {
    let mut h = Harness::new();
    let before = h.shared.lock().entries.len();
    assert!(matches!(h.query("session", &[]).unwrap(), Value::Record(_)));
    let frame = h
        .query(
            "animation",
            &[
                Value::str("plasma"),
                Value::Number(3.0),
                Value::Number(10.0),
                Value::Number(2.0),
            ],
        )
        .unwrap();
    assert!(matches!(frame, Value::Record(_)));
    assert_eq!(h.shared.lock().entries.len(), before);
    assert!(matches!(
        h.query("nope", &[]),
        Err(DataError::UnknownSource(_))
    ));
}
