//! ExactKit's ResetGate names, by `#function`, every Runtime call held over a
//! restart's reset (ExactCore `Runtime.holds`), and drops or replays each.
//! A renamed call fell out of its table and was dropped by default (#1141 and
//! #1144 renamed `tickAsync`, `advanceAsync` and `frame`): every held call is
//! in one table, every table key names a held call that exists, and the
//! known losses only shrink.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn swift_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            swift_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "swift") {
            out.push(path);
        }
    }
}

/// `src` with comments (`//`, `/* */`) blanked, lengths kept; with `strings`, string contents (`"…"`, `"""…"""`)
/// too, so their braces and parentheses don't count.
fn blank(src: &str, strings: bool) -> Vec<u8> {
    let b = src.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    let wipe = |out: &mut Vec<u8>, from: usize, to: usize| {
        for c in &mut out[from..to] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            let end = (i..b.len()).find(|&j| b[j] == b'\n').unwrap_or(b.len());
            wipe(&mut out, i, end);
            i = end;
        } else if b[i..].starts_with(b"/*") {
            let end = (i + 2..b.len())
                .find(|&j| b[j..].starts_with(b"*/"))
                .map_or(b.len(), |j| j + 2);
            wipe(&mut out, i, end);
            i = end;
        } else if b[i..].starts_with(b"\"\"\"") {
            let end = (i + 3..b.len())
                .find(|&j| b[j..].starts_with(b"\"\"\""))
                .map_or(b.len(), |j| j + 3);
            if strings {
                wipe(&mut out, i + 3, end.saturating_sub(3).max(i + 3));
            }
            i = end;
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' && b[j] != b'\n' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            if strings {
                wipe(&mut out, i + 1, j.min(b.len()));
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// The index of the bracket closing the one at `at`.
fn close(src: &[u8], at: usize, open: u8, shut: u8) -> usize {
    let mut depth = 0;
    for (j, &c) in src.iter().enumerate().skip(at) {
        if c == open {
            depth += 1;
        } else if c == shut {
            depth -= 1;
            if depth == 0 {
                return j;
            }
        }
    }
    src.len()
}

fn find_all(hay: &[u8], needle: &str) -> Vec<usize> {
    let n = needle.as_bytes();
    (0..hay.len().saturating_sub(n.len() - 1))
        .filter(|&i| &hay[i..i + n.len()] == n)
        .collect()
}

fn word_before(src: &[u8], at: usize) -> bool {
    at == 0 || !(src[at - 1].is_ascii_alphanumeric() || src[at - 1] == b'_')
}

struct Func {
    sig: String,
    body: (usize, usize),
}

/// Each `func`'s signature as `#function` spells it (`name(label:_:)`), and its body.
fn funcs(src: &[u8]) -> Vec<Func> {
    let mut out = Vec::new();
    for at in find_all(src, "func ") {
        if !word_before(src, at) {
            continue;
        }
        let name_start = at + 5;
        let name_end = (name_start..src.len())
            .find(|&i| !(src[i].is_ascii_alphanumeric() || src[i] == b'_'))
            .unwrap();
        let mut p0 = name_end;
        while src[p0] == b' ' {
            p0 += 1;
        }
        if src[p0] == b'<' {
            p0 = close(src, p0, b'<', b'>') + 1;
        }
        if src[p0] != b'(' {
            continue;
        }
        let p1 = close(src, p0, b'(', b')');
        let Some(b0) = (p1..src.len()).find(|&i| src[i] == b'{') else {
            continue;
        };
        let b1 = close(src, b0, b'{', b'}');
        let params = String::from_utf8_lossy(&src[p0 + 1..p1]).replace("->", "  ");
        let (mut labels, mut depth, mut cur) = (Vec::new(), 0i32, String::new());
        for c in params.chars() {
            match c {
                '(' | '[' | '<' => depth += 1,
                ')' | ']' | '>' => depth -= 1,
                _ => {}
            }
            if c == ',' && depth == 0 {
                labels.push(std::mem::take(&mut cur));
            } else {
                cur.push(c);
            }
        }
        if !cur.trim().is_empty() {
            labels.push(cur);
        }
        let labels: String = labels
            .iter()
            .map(|l| format!("{}:", l.trim().split([' ', ':']).next().unwrap()))
            .collect();
        let name = String::from_utf8_lossy(&src[name_start..name_end]);
        out.push(Func {
            sig: format!("{name}({labels})"),
            body: (b0, b1),
        });
    }
    out
}

/// Every held Runtime call, by the name `heldCalls` gives it: a function that calls `holds()`; a name given to
/// `runtime.holds("…")`; and a function that calls `on { … }` (any receiver) with a closure that reads a batch
/// (`read(`), as `Runtime.on(_:caller:)` records its caller. Each with where it is.
fn held_calls(sources: &Path) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    swift_files(sources, &mut files);
    let mut held = BTreeMap::new();
    for file in files {
        let raw = std::fs::read_to_string(&file).unwrap();
        let src = blank(&raw, true);
        let where_ = file.strip_prefix(sources).unwrap().display().to_string();
        let fs = funcs(&src);
        let innermost = |at: usize| {
            fs.iter()
                .filter(|f| f.body.0 < at && at < f.body.1)
                .min_by_key(|f| f.body.1 - f.body.0)
        };
        let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        for at in find_all(&src, "holds()") {
            if at > 0 && ident(src[at - 1]) {
                continue;
            }
            if let Some(f) = innermost(at).filter(|f| !f.sig.starts_with("holds(")) {
                held.entry(f.sig.clone())
                    .or_insert(format!("{where_}: holds()"));
            }
        }
        // A name given to `runtime.holds("…")`: the call found where strings are blank (so none inside a string
        // counts), its argument read from the source.
        for at in find_all(&src, "runtime.holds(\"") {
            let start = at + "runtime.holds(\"".len();
            let end = start + raw[start..].find('"').unwrap();
            held.entry(raw[start..end].to_string())
                .or_insert(format!("{where_}: runtime.holds(name)"));
        }
        for at in find_all(&src, "on") {
            if (at > 0 && ident(src[at - 1])) || src.get(at + 2).is_some_and(|&c| ident(c)) {
                continue;
            }
            let mut j = at + 2;
            while j < src.len() && src[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < src.len() && src[j] == b'(' {
                j += 1;
                while j < src.len() && src[j].is_ascii_whitespace() {
                    j += 1;
                }
            }
            if j >= src.len() || src[j] != b'{' {
                continue;
            }
            let closure =
                String::from_utf8_lossy(&src[j..=close(&src, j, b'{', b'}').min(src.len() - 1)])
                    .into_owned();
            let reads = closure
                .match_indices("read(")
                .any(|(i, _)| i == 0 || !ident(closure.as_bytes()[i - 1]));
            if closure[1..].trim_start().starts_with("() -> Void") || !reads {
                continue;
            }
            if let Some(f) = innermost(at) {
                held.entry(f.sig.clone()).or_insert(format!("{where_}: on"));
            }
        }
    }
    held
}

/// ResetGate's two tables' keys.
fn gate_keys(gate: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let gate = String::from_utf8(blank(gate, false)).unwrap();
    for line in gate.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix('"') {
            if let Some((key, after)) = rest.split_once('"') {
                if after.starts_with(':') && key.contains('(') {
                    keys.insert(key.to_string());
                }
            }
        }
    }
    keys
}

/// ResetGate's known losses on 2026-10-29: an entry may leave, none may join.
const KNOWN_LOSS: &[&str] = &[
    "activities(_:)",
    "canvasDisplay(scale:memory:)",
    "canvasImage(_:width:height:ok:)",
    "change(_:_:selection:now:)",
    "controlTextChanged()",
    "deliverySync()",
    "designRows(_:)",
    "hostGo(_:now:)",
    "midi(_:)",
    "navigate(_:_:now:)",
    "picked(_:_:now:)",
    "preferExtension(_:)",
    "setModes(_:check:)",
    "share(_:now:)",
    "storeChanged(retryOnly:)",
    "textReady(index:generation:revision:)",
];

#[test]
fn every_held_runtime_call_is_in_one_reset_gate_table_and_every_key_is_held() {
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("Sources");
    let held = held_calls(&sources);
    let gate = std::fs::read_to_string(sources.join("ExactKit/ResetGate.swift")).unwrap();
    let keys = gate_keys(&gate);
    assert!(
        held.len() > 50 && keys.len() > 50,
        "the scan found the calls: {} held, {} keys",
        held.len(),
        keys.len()
    );
    let unlisted: Vec<_> = held
        .iter()
        .filter(|(k, _)| !keys.contains(*k))
        .map(|(k, w)| format!("{k} ({w})"))
        .collect();
    assert!(unlisted.is_empty(), "held calls ResetGate lists in no table (each dropped by default; say why, or replay it):\n  {}", unlisted.join("\n  "));
    let stale: Vec<_> = keys.iter().filter(|k| !held.contains_key(*k)).collect();
    // `knownLoss` (each a held arrival whose content a reset loses, to triage) only shrinks.
    let known = &gate[gate
        .find("static let knownLoss")
        .expect("the knownLoss table")..];
    let known = gate_keys(&known[..known.find("    ]").unwrap()]);
    let grown: Vec<_> = known
        .difference(&KNOWN_LOSS.iter().map(|k| k.to_string()).collect())
        .cloned()
        .collect();
    assert!(
        grown.is_empty(),
        "a new known loss: replay it, or drop it with a reason; knownLoss only shrinks: {grown:?}"
    );
    assert!(
        stale.is_empty(),
        "ResetGate keys naming no held call (a renamed method falls out of its table): {stale:?}"
    );
}

#[test]
fn the_scan_reads_each_holding_form_and_no_other() {
    let dir = std::env::temp_dir().join(format!("reset-gate-scan-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ExactCore")).unwrap();
    let swift = r#"package final class Runtime {
    package func holds(_ caller: String = #function) -> Bool { held }
    package func tickAsync(now: Double, capture: UInt64 = 0, done: @escaping (Batch) -> Void) {
        if holds() { return } // a comment's { brace
    }
    package func frame(now: Double) -> Batch { on { read(exact_frame(rt, now)) } }
    package func later(_ calls: [(Runtime) -> Batch], done: @escaping (Batch) -> Void) { if holds() { return } }
    package func generic<T>(_ x: T) -> Batch {
        return self.on {
            read(exact_g(rt))
        }
    }
    package func spaced(_ x: Int) -> Batch { on(
        { read(exact_s(rt)) }) }
    package func setThing(_ x: Int) { on { () -> Void in _ = read(x) } }
    package func asks() -> Bool { on(busy: false) { read(exact_b(rt)) } }
    /* package func commented() -> Batch { on { read(x) } } */
    package func script() -> String { """
        on { read(x) } holds() runtime.holds("inString()")
        """ }
    package func thread() -> Batch { on { Thread(x) } }
}
extension ExactSession {
    func tellSoon(_ calls: [(Runtime) -> Batch]) { guard !runtime.holds("tellSoon(_:)") else { return } }
    // runtime.holds("obsolete()")
    func controlTextChanged() {
        func nested() -> Int { 0 }
        apply(runtime.on { runtime.read(exact_control_text_changed(runtime.rt)) })
    }
    func flag() { if ShadowCheck.on { out["x"] = 1 } }
}
"#;
    std::fs::write(dir.join("ExactCore/Bridge.swift"), swift).unwrap();
    let held = held_calls(&dir);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        held.keys().cloned().collect::<Vec<_>>(),
        [
            "controlTextChanged()",
            "frame(now:)",
            "generic(_:)",
            "later(_:done:)",
            "spaced(_:)",
            "tellSoon(_:)",
            "tickAsync(now:capture:done:)",
        ]
    );
    let keys = gate_keys(concat!(
        "        \"tickAsync(now:done:)\": (.advance, \"x\"),\n",
        "        \"frame(now:)\": \"y\",\n",
        "        // \"gone(_:)\": \"a key in a line comment\",\n",
        "    /*\n        \"commented(_:)\": \"a key in a block comment\",\n    */\n",
    ));
    assert_eq!(
        keys.into_iter().collect::<Vec<_>>(),
        ["frame(now:)", "tickAsync(now:done:)"],
        "a renamed key is read as it is written, and none in a comment"
    );
}
