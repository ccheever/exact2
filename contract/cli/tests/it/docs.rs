//! Every example in the guides compiles (LLP 1086 D10): each fenced
//! `contract` block in `docs/*.md` and `README.md` compiles with no
//! diagnostics, and each `contract-test` block parses. A deliberately
//! partial block is fenced `text`. Every failure is reported in one run.

use std::path::{Path, PathBuf};

/// One fenced block: its document, the line of its opening fence, its info
/// string, its body, and the nearest `contract` block before it in the same
/// section (the one a `ts` block's types come from).
pub(crate) struct Block {
    pub(crate) doc: PathBuf,
    pub(crate) line: usize,
    pub(crate) info: String,
    pub(crate) body: String,
    pub(crate) contract: Option<String>,
    /// The `<!-- check: … -->` marker on the line before the fence, if any:
    /// `app`, `route <name>` or `file`. Marked blocks are parts of one app
    /// and are compiled assembled, not one by one.
    pub(crate) check: Option<String>,
}

impl Block {
    pub(crate) fn at(&self) -> String {
        format!("{}:{}", self.doc.display(), self.line)
    }
}

/// `README.md` and every `docs/*.md`, by path from the repo root.
pub(crate) fn documents() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut docs: Vec<PathBuf> = std::fs::read_dir(root.join("docs"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    docs.sort();
    docs.insert(0, root.join("README.md"));
    docs
}

/// The fenced blocks of every document, in order. A heading outside a fence
/// ends a section, so a `ts` block never pairs with an earlier section's
/// Contract.
pub(crate) fn blocks() -> Vec<Block> {
    let mut out = Vec::new();
    for doc in documents() {
        let text = std::fs::read_to_string(&doc).unwrap();
        let shown = doc
            .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .unwrap_or(&doc)
            .to_path_buf();
        let mut open: Option<(usize, String, String)> = None;
        let mut contract: Option<String> = None;
        let mut marker: Option<String> = None;
        let mut pending: Option<String> = None;
        for (index, line) in text.lines().enumerate() {
            match &mut open {
                Some((start, info, body)) => {
                    if line.trim() == "```" {
                        let block = Block {
                            doc: shown.clone(),
                            line: *start,
                            info: std::mem::take(info),
                            body: std::mem::take(body),
                            contract: contract.clone(),
                            check: pending.take(),
                        };
                        if block.info == "contract" {
                            contract = Some(block.body.clone());
                        }
                        out.push(block);
                        open = None;
                    } else {
                        body.push_str(line);
                        body.push('\n');
                    }
                }
                None => {
                    if let Some(info) = line.strip_prefix("```") {
                        pending = marker.take();
                        open = Some((index + 1, info.trim().to_string(), String::new()));
                    } else {
                        marker = line
                            .trim()
                            .strip_prefix("<!-- check:")
                            .and_then(|rest| rest.strip_suffix("-->"))
                            .map(|m| m.trim().to_string());
                        if line.starts_with('#') {
                            contract = None;
                        }
                    }
                }
            }
        }
        assert!(open.is_none(), "{}: unclosed fence", shown.display());
    }
    out
}

#[test]
fn every_contract_example_in_the_guides_compiles_and_every_test_parses() {
    let dir = std::env::temp_dir().join(format!("exact-docs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    let (mut compiled, mut parsed) = (0, 0);
    let mut failures = Vec::new();
    for block in blocks() {
        // A diagnostic's line is the block's, so the document line is the
        // fence's plus it.
        let at = |line: u32| format!("{}:{}", block.doc.display(), block.line + line as usize);
        match block.info.as_str() {
            "contract" if block.check.is_some() => {}
            "contract" => {
                compiled += 1;
                // A declared sound the example names is a short WAV here
                // (LLP 1096 D1: the compiler reads it).
                for line in block.body.lines() {
                    if let Some(src) = line
                        .strip_prefix("sound \"")
                        .and_then(|r| r.strip_suffix('"'))
                    {
                        let file = dir.join(src);
                        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                        std::fs::write(&file, wav()).unwrap();
                    }
                }
                if let Err(all) = contract::compile_path_source_all(&path, &block.body, false) {
                    for e in all {
                        failures.push(format!("{}: [{}] {}", at(e.span.line), e.id, e.message));
                    }
                }
            }
            "contract-test" => {
                parsed += 1;
                if let Err(e) = contract::tests(&block.body) {
                    failures.push(format!("{}: [{}] {}", at(e.span.line), e.id, e.message));
                }
            }
            _ => {}
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(compiled > 0 && parsed > 0, "no examples found");
    assert!(
        failures.is_empty(),
        "{} of {compiled} contract and {parsed} contract-test blocks fail; fence a deliberately partial block as `text`:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Blocks marked `<!-- check: app -->`, `<!-- check: route <name> -->` and
/// `<!-- check: file -->` are one app split across a guide (LLP 1115 D7,
/// docs/start-here.md): the `app` blocks in order, each `route` block in
/// place of the placeholder under `when e.name == "<name>"`, and each `file`
/// block appended. The assembled app compiles with no diagnostics.
#[test]
fn every_split_app_in_the_guides_compiles_assembled() {
    let dir = std::env::temp_dir().join(format!("exact-docs-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    let mut docs: Vec<(PathBuf, Vec<Block>)> = Vec::new();
    for block in blocks() {
        if block.info != "contract" || block.check.is_none() {
            continue;
        }
        match docs.last_mut() {
            Some((doc, list)) if *doc == block.doc => list.push(block),
            _ => docs.push((block.doc.clone(), vec![block])),
        }
    }
    assert!(
        !docs.is_empty(),
        "no split apps found (docs/start-here.md has one)"
    );
    let mut failures = Vec::new();
    for (doc, list) in &docs {
        let mut app = String::new();
        for b in list.iter().filter(|b| b.check.as_deref() == Some("app")) {
            app.push_str(&b.body);
        }
        for b in list {
            let check = b.check.as_deref().unwrap_or("");
            if let Some(name) = check.strip_prefix("route ") {
                let name = name.trim();
                match fill_route(&app, name, &b.body) {
                    Some(filled) => app = filled,
                    None => failures.push(format!(
                        "{}: no `when e.name == \"{name}\"` placeholder in the app blocks",
                        b.at()
                    )),
                }
            }
        }
        for b in list.iter().filter(|b| b.check.as_deref() == Some("file")) {
            app.push('\n');
            app.push_str(&b.body);
        }
        if let Err(all) = contract::compile_path_source_all(&path, &app, false) {
            let lines: Vec<&str> = app.lines().collect();
            for e in all {
                let at = lines
                    .get(e.span.line.saturating_sub(1) as usize)
                    .copied()
                    .unwrap_or("");
                failures.push(format!(
                    "{} (assembled line {}: `{}`): [{}] {}",
                    doc.display(),
                    e.span.line,
                    at.trim(),
                    e.id,
                    e.message
                ));
            }
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        failures.is_empty(),
        "{} failures in assembled guide apps:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The app with the route's block in place of the lines under
/// `when e.name == "<name>"` (the placeholder screen and its comment).
fn fill_route(app: &str, name: &str, route: &str) -> Option<String> {
    let lines: Vec<&str> = app.lines().collect();
    let head = format!("when e.name == \"{name}\"");
    let at = lines.iter().position(|l| l.trim() == head)?;
    let depth = lines[at].len() - lines[at].trim_start().len();
    let mut end = at + 1;
    while end < lines.len() {
        let l = lines[end];
        if !l.trim().is_empty() && l.len() - l.trim_start().len() <= depth {
            break;
        }
        end += 1;
    }
    let pad = " ".repeat(depth + 2);
    let mut out: Vec<String> = lines[..=at].iter().map(|l| l.to_string()).collect();
    for l in route.lines() {
        out.push(if l.trim().is_empty() {
            String::new()
        } else {
            format!("{pad}{l}")
        });
    }
    out.extend(lines[end..].iter().map(|l| l.to_string()));
    let mut s = out.join("\n");
    s.push('\n');
    Some(s)
}

/// A 10 ms, 48 kHz, 16-bit mono WAV of silence.
fn wav() -> Vec<u8> {
    let data = 480u32 * 2;
    let mut b = b"RIFF".to_vec();
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&[1, 0, 1, 0]);
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    b.extend_from_slice(&[2, 0, 16, 0]);
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    b.resize(b.len() + data as usize, 0);
    b
}
