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
                        open = Some((index + 1, info.trim().to_string(), String::new()));
                    } else if line.starts_with('#') {
                        contract = None;
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
            "contract" => {
                compiled += 1;
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
