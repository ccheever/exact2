//! The session driven through the `DataSource` API, with the mock model.

use super::*;
use crate::shapes::{ContractValue, Session};
use crate::state::Block;
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
    let mut h = Harness::with_options(Options::offline(None));
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
            "more",
            "more",
            "more",
            "more",
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
    // The first reply is five entries, one per block (the lists are one),
    // and together they are exactly the whole reply parsed at once.
    let first: Vec<Block> = entries[2..7]
        .iter()
        .flat_map(|e| e.blocks.clone())
        .collect();
    for kind in ["h2", "p", "li", "code", "quote"] {
        assert!(first.iter().any(|b| b.kind == kind), "no {kind}");
    }
    let reply = h.shared.lock().history[1].text();
    assert_eq!(first, crate::markdown::blocks(&reply));
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
    let mut h = Harness::with_options(Options::offline(None));
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

#[test]
fn a_saved_key_switches_the_model_and_never_shows() {
    use std::os::unix::fs::PermissionsExt;
    const KEY: &str = "sk-or-v1-TESTONLY-0123456789abcdef";
    let dir = std::env::temp_dir().join(format!("exact-harness-setkey-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("keys"), "OTHER=kept\n").unwrap();
    let mut h = Harness::with_options(Options::offline(Some(dir.clone())));
    assert!(send(&mut h, "setModel", &["mock"]));
    assert!(send(
        &mut h,
        "setKey",
        &["openrouter", &format!("  {KEY}\n")]
    ));
    {
        let s = h.shared.lock();
        assert_eq!(s.model, "openrouter:anthropic/claude-sonnet-4.5");
        assert!(s
            .toast
            .starts_with("OpenRouter key saved — model: Anthropic: Claude Sonnet 4.5"));
        assert!(s
            .models
            .iter()
            .filter(|m| m.provider == "openrouter")
            .all(|m| m.available));
        assert!(
            !s.models
                .iter()
                .any(|m| m.provider == "anthropic" && m.available)
                || std::env::var_os("ANTHROPIC_API_KEY").is_some()
        );
    }
    let text = std::fs::read_to_string(dir.join("keys")).unwrap();
    assert_eq!(text, format!("OTHER=kept\nOPENROUTER_API_KEY={KEY}\n"));
    let mode = std::fs::metadata(dir.join("keys"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    // A turn that cannot connect still names no key.
    h.shared.lock().openrouter_base = "http://127.0.0.1:9".into();
    assert!(send(&mut h, "submit", &["hello"]));
    drive(&mut h, "no");
    for cmd in ["/models", "/help"] {
        assert!(send(&mut h, "submit", &[cmd]));
    }
    let shown = format!("{:?}", h.session());
    assert!(
        !shown.contains(KEY) && !shown.contains("0123456789abcdef"),
        "a key reached the session"
    );
    assert!(shown.contains("OpenRouter"));
    assert!(!format!("{:?}", h.shared.lock().history).contains(KEY));
    // A key loads back from the file; an empty key removes it.
    let again = Harness::with_options(Options::offline(Some(dir.clone())));
    if std::env::var_os("OPENROUTER_API_KEY").is_none() {
        assert_eq!(
            again.shared.lock().keys.get("OPENROUTER_API_KEY"),
            Some(KEY)
        );
    }
    assert!(!send(&mut h, "setKey", &["nobody", "x"]));
    assert!(send(&mut h, "setKey", &["openrouter", ""]));
    assert_eq!(
        std::fs::read_to_string(dir.join("keys")).unwrap(),
        "OTHER=kept\n"
    );
    assert_eq!(h.shared.lock().model, "mock");
    let _ = std::fs::remove_dir_all(&dir);
}

fn ids(entries: &[Entry]) -> Vec<u64> {
    entries.iter().map(|e| e.id.parse().unwrap()).collect()
}

#[test]
fn a_huge_reply_settles_while_it_streams() {
    let mut h = mock();
    assert!(send(&mut h, "submit", &["something huge"]));
    let start = Instant::now();
    let mut settled_while_busy = 0;
    let mut tails = 0;
    loop {
        let s = h.shared.lock();
        if !s.busy() {
            break;
        }
        settled_while_busy = settled_while_busy.max(
            s.entries
                .iter()
                .filter(|e| e.kind == "more" && !e.busy)
                .count(),
        );
        tails = tails.max(s.entries.iter().filter(|e| e.busy).count());
        drop(s);
        assert!(start.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        settled_while_busy > 20,
        "only {settled_while_busy} settled before the end"
    );
    assert!(tails <= 1, "one busy tail at most");
    let s = h.shared.lock();
    let reply: Vec<&Entry> = s.entries.iter().skip(2).collect();
    assert_eq!(reply[0].kind, "assistant");
    assert!(reply[1..].iter().all(|e| e.kind == "more"));
    assert!(reply.len() > 300, "{}", reply.len());
    let joined: Vec<Block> = reply.iter().flat_map(|e| e.blocks.clone()).collect();
    assert_eq!(joined, crate::markdown::blocks(&s.history[1].text()));
    let code = reply
        .iter()
        .find(|e| e.blocks.iter().any(|b| b.kind == "code"))
        .unwrap();
    assert_eq!(code.blocks[0].lines.len(), 300, "the fence settled whole");
    let ids = ids(&s.entries);
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids increase");
}

#[test]
fn retire_drops_printed_entries_from_the_session() {
    let mut h = mock();
    for cmd in ["/help", "/tools", "/unicode"] {
        assert!(send(&mut h, "submit", &[cmd]));
    }
    let (second, last) = {
        let s = h.shared.lock();
        (s.entries[1].id.clone(), s.entries[3].id.clone())
    };
    assert!(send(&mut h, "retire", &[&second]));
    assert_eq!(h.shared.lock().entries.len(), 2);
    assert_eq!(h.shared.lock().retired, 2.0);
    // Idempotent; an older or unknown id changes nothing.
    assert!(send(&mut h, "retire", &[&second]));
    assert!(send(&mut h, "retire", &["1"]));
    assert!(!send(&mut h, "retire", &["nope"]));
    assert_eq!(h.shared.lock().retired, 2.0);
    // The snapshot answers what is left, and says how many went.
    let snapshot = Session::from_value(&h.session()).unwrap();
    assert_eq!(snapshot.entries.len(), 2);
    assert_eq!(snapshot.retired, 2.0);
    // New entries still append, with larger ids; /clear still works.
    assert!(send(&mut h, "submit", &["/diff"]));
    let after = ids(&h.shared.lock().entries);
    assert!(after.windows(2).all(|w| w[0] < w[1]));
    assert!(after[0] > second.parse::<u64>().unwrap());
    assert!(send(&mut h, "retire", &[&last]));
    assert_eq!(h.shared.lock().entries.len(), 1);
    // /clear starts a new transcript: a fresh banner under a new epoch.
    assert!(send(&mut h, "submit", &["/clear"]));
    {
        let s = h.shared.lock();
        assert_eq!(kinds(&s.entries), ["banner"]);
        assert_eq!(s.epoch, 1.0);
    }
    assert!(send(&mut h, "submit", &["/help"]));
    let id: u64 = h.shared.lock().entries[0].id.parse().unwrap();
    assert!(id > *after.last().unwrap(), "ids are never reused");
    // A busy entry is never retired.
    assert!(send(&mut h, "submit", &["hi"]));
    let start = Instant::now();
    while h.shared.lock().approval.tool != "bash" {
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    let busy = h.shared.lock().entries.last().unwrap().id.clone();
    assert!(send(&mut h, "retire", &[&busy]));
    let s = h.shared.lock();
    assert_eq!(s.entries.len(), 1);
    assert!(s.entries[0].busy);
}

#[test]
fn prompts_queue_while_a_turn_runs() {
    let mut h = mock();
    assert!(send(&mut h, "submit", &["first"]));
    assert!(send(&mut h, "submit", &["second"]));
    assert!(send(&mut h, "submit", &["third"]));
    {
        let s = h.shared.lock();
        assert!(s.busy());
        assert_eq!(Vec::from(s.queue.clone()), ["second", "third"]);
    }
    let snapshot = Session::from_value(&h.session()).unwrap();
    assert_eq!(snapshot.queued, ["second", "third"]);
    // Commands run at once during a turn; /clear waits.
    let before = h.shared.lock().entries.len();
    assert!(send(&mut h, "submit", &["/help"]));
    assert!(h.shared.lock().entries.len() > before);
    assert!(!send(&mut h, "submit", &["/clear"]));
    assert!(h.shared.lock().toast.contains("clear"));
    // An interrupt stops the turn and keeps the queue: the next one starts.
    while h.shared.lock().approval.tool != "bash" {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(send(&mut h, "interrupt", &[]));
    {
        let s = h.shared.lock();
        assert!(s.busy(), "the queued prompt started");
        assert_eq!(Vec::from(s.queue.clone()), ["third"]);
    }
    let entries = drive(&mut h, "yes");
    let users: Vec<String> = entries
        .iter()
        .filter(|e| e.kind == "user")
        .map(|e| e.blocks[0].lines[0].text())
        .collect();
    assert_eq!(users, ["first", "second", "third"]);
    assert!(h.shared.lock().queue.is_empty());
}

#[test]
fn the_chaos_run_ends_bounded() {
    let mut h = mock();
    assert!(send(&mut h, "submit", &["chaos"]));
    let entries = drive(&mut h, "yes");
    let tool = entries.iter().find(|e| e.kind == "tool").unwrap();
    assert_eq!(tool.status, "ok");
    let s = h.shared.lock();
    let result = s
        .history
        .iter()
        .flat_map(|m| &m.parts)
        .find_map(|p| match p {
            state::Part::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .unwrap();
    assert!(result.len() < 100_000, "{}", result.len());
    assert!(
        result.contains("bytes omitted"),
        "the flood was bounded while read"
    );
    assert!(entries.last().unwrap().kind == "assistant");
    assert!(entries.iter().filter(|e| e.kind == "more").count() > 300);
}

#[test]
fn a_turn_keeps_the_model_it_started_with() {
    let dir = std::env::temp_dir().join(format!("exact-harness-model-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    providers::mock::FAST.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut h = Harness::with_options(Options::offline(Some(dir.clone())));
    // A closed port: the next turn fails fast, without the network.
    h.shared.lock().openrouter_base = "http://127.0.0.1:9".into();
    assert!(send(&mut h, "setModel", &["mock"]));
    assert!(send(&mut h, "submit", &["hi"]));
    let start = Instant::now();
    while h.shared.lock().approval.tool != "bash" {
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    // The picker changes mid-turn: an OpenRouter key arrives.
    assert!(send(
        &mut h,
        "setKey",
        &["openrouter", "test-only-not-a-key"]
    ));
    assert!(h.shared.lock().model.starts_with("openrouter:"));
    let entries = drive(&mut h, "yes");
    let turn: Vec<&Entry> = entries.iter().skip(2).collect();
    assert!(turn.len() > 5);
    for e in &turn {
        assert_eq!(e.model, "mock", "{} {}", e.kind, e.title);
    }
    assert_eq!(entries[0].model, "", "the banner is outside any turn");
    assert_eq!(entries[1].model, "", "so is the prompt");
    // The next turn carries the new model, even when it fails.
    assert!(send(&mut h, "submit", &["again"]));
    let entries = drive(&mut h, "yes");
    let last = entries.last().unwrap();
    assert_eq!(last.kind, "error");
    assert_eq!(last.model, "Claude Sonnet 4.5");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn paced_stress_releases_every_entry_in_batches() {
    let mut h = mock();
    let before = h.shared.lock().entries.len();
    assert!(send(&mut h, "submit", &["/stress 95 paced"]));
    let start = Instant::now();
    let mut seen = vec![];
    loop {
        let n = h.shared.lock().entries.len() - before;
        if seen.last() != Some(&n) {
            seen.push(n);
        }
        if n == 95 {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "stuck at {n}");
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(seen.len() >= 4, "released in batches: {seen:?}");
    assert!(seen
        .iter()
        .all(|n| *n <= 95 && (*n % commands::BATCH == 0 || *n == 95)));
    assert!(!send(&mut h, "submit", &["/stress 5 fast"]));
}

#[test]
fn paced_stress_waits_for_each_batch_to_retire_and_stops_at_clear() {
    let mut h = mock();
    let before = h.shared.lock().entries.len();
    assert!(send(&mut h, "submit", &["/stress 100 paced"]));
    let count = |h: &Harness| h.shared.lock().entries.len() - before;
    let wait_for = |h: &Harness, n: usize| {
        let start = Instant::now();
        while count(h) < n {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "stuck at {}",
                count(h)
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    wait_for(&h, commands::BATCH);
    // Nothing retired: the next batch waits for the host.
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(count(&h), commands::BATCH);
    // The host printed and retired the batch: the next one comes at once.
    let last = h.shared.lock().entries.last().unwrap().id.clone();
    assert!(send(&mut h, "retire", &[&last]));
    let retired = h.shared.lock().entries.len();
    let start = Instant::now();
    while h.shared.lock().entries.len() < retired + commands::BATCH {
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "the next batch did not follow"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    // A new transcript ends the workload.
    assert!(send(&mut h, "submit", &["/clear"]));
    let cleared = h.shared.lock().entries.len();
    std::thread::sleep(Duration::from_millis(1300));
    assert_eq!(
        h.shared.lock().entries.len(),
        cleared,
        "paced work ran on into the new transcript"
    );
}
