//! The index of an LLP directory: what the sidebar lists and what search
//! looks through.
//!
//! An LLP corpus has a shape the general reader knows nothing about, and
//! that shape is the whole reason this app is not the general reader: a
//! document is `NNNN-slug.type.md`, a dotted number is a sub-document of
//! the number before the dot, the metadata header carries a Type and a
//! Status, and `current/` and `foundation/` are symlink overlays that say
//! which documents are the declared working set. @ref LLP 1033

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One document in the corpus.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// `1030`, or `1030.000` for a sub-document.
    pub number: String,
    /// The heading, with its own `LLP NNNN:` prefix removed.
    pub title: String,
    /// Where it is.
    pub path: PathBuf,
    /// The filename's declared kind: `spec`, `rfc`, `plan`, `explainer`, …
    pub kind: String,
    /// The metadata header's `**Status:**`.
    pub status: String,
    /// Nesting: 0 for a top-level document, 1 for a sub-document of one.
    pub depth: u32,
    /// `current`, `foundation`, or empty — which overlay links it.
    pub overlay: String,
    /// The document's text, kept so search does not read the disk again.
    pub body: String,
}

impl Entry {
    /// Whether `query` (already lowercased) appears in the number, the
    /// title, the status, or the text.
    pub fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || self.number.contains(query)
            || self.title.to_lowercase().contains(query)
            || self.kind.contains(query)
            || self.status.to_lowercase().contains(query)
            || self.body.to_lowercase().contains(query)
    }

    /// How many times `query` appears in the body — what the sidebar counts.
    pub fn hits(&self, query: &str) -> usize {
        if query.is_empty() {
            return 0;
        }
        self.body.to_lowercase().matches(query).count()
    }
}

/// Read every Markdown document under `root`, in number order.
///
/// Documents that do not follow the convention are still listed — a corpus
/// with a `README.md` in it should not look empty — they simply sort last
/// and carry no number.
pub fn read(root: &Path) -> Result<Vec<Entry>, String> {
    let overlays = overlays(root);
    let mut entries: Vec<Entry> = std::fs::read_dir(root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md"))
        .filter_map(|path| entry(&path, &overlays))
        .collect();
    entries.sort_by(|a, b| {
        sort_key(&a.number)
            .cmp(&sort_key(&b.number))
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(entries)
}

/// Which documents each overlay directory links. The overlays are symlinks
/// (archiving is removing a link), so the *name* is what identifies the
/// document — following the link would only lead back here.
fn overlays(root: &Path) -> Vec<(String, BTreeSet<String>)> {
    ["current", "foundation"]
        .iter()
        .map(|name| {
            let names = std::fs::read_dir(root.join(name))
                .map(|dir| {
                    dir.flatten()
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            (name.to_string(), names)
        })
        .collect()
}

fn entry(path: &Path, overlays: &[(String, BTreeSet<String>)]) -> Option<Entry> {
    let file = path.file_name()?.to_string_lossy().into_owned();
    let body = std::fs::read_to_string(path).ok()?;
    let (number, kind) = name_parts(&file);
    let overlay = overlays
        .iter()
        .find(|(_, names)| names.contains(&file))
        .map(|(name, _)| name.clone())
        .unwrap_or_default();
    Some(Entry {
        depth: if number.contains('.') { 1 } else { 0 },
        title: title(&body).unwrap_or_else(|| file.clone()),
        status: status(&body),
        number,
        kind,
        path: path.to_path_buf(),
        overlay,
        body,
    })
}

/// `1030.000-dev-server-as-deployer.rfc.md` → (`1030.000`, `rfc`).
fn name_parts(file: &str) -> (String, String) {
    let stem = file.strip_suffix(".md").unwrap_or(file);
    let number: String = stem
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    // A trailing dot belongs to the `.type` suffix, not to the number.
    let number = number.trim_end_matches('.').to_string();
    let kind = stem
        .rsplit('.')
        .next()
        .filter(|k| !k.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or("")
        .to_string();
    (number, kind)
}

/// `1030.000` sorts after `1030` and before `1031`, and an unnumbered
/// document sorts last.
fn sort_key(number: &str) -> (u32, u32, u32) {
    if number.is_empty() {
        return (u32::MAX, 0, 0);
    }
    let mut parts = number.split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    match parts.next() {
        Some(minor) => (major, 1, minor.parse().unwrap_or(0)),
        None => (major, 0, 0),
    }
}

/// The first `# ` heading, with an `LLP NNNN:` prefix taken off — the
/// sidebar shows the number in its own column and does not want it twice.
fn title(body: &str) -> Option<String> {
    let heading = body.lines().find_map(|l| l.strip_prefix("# "))?.trim();
    let after = heading
        .strip_prefix("LLP ")
        .and_then(|rest| rest.split_once(american_colon))
        .map(|(_, title)| title.trim())
        .unwrap_or(heading);
    Some(after.replace('`', ""))
}

fn american_colon(c: char) -> bool {
    c == ':'
}

/// The status *word*. A header's `**Status:**` in this corpus can run to a
/// paragraph — "Review (super-refine loop 2026-08-28 at Charlie's request,
/// closed the same day …)" — and a sidebar column is not where that is
/// read. The parenthetical is the document's; the word is the index's.
fn status(body: &str) -> String {
    let full = field(body, "Status").unwrap_or_default();
    let word = full.split(" (").next().unwrap_or(&full).trim();
    match word.char_indices().nth(24) {
        Some((cut, _)) => format!("{}…", &word[..cut]),
        None => word.to_string(),
    }
}

/// A metadata header field: `**Status:** Draft` → `Draft`.
pub fn field(body: &str, name: &str) -> Option<String> {
    let want = format!("**{name}:**");
    body.lines()
        .take(30)
        .find_map(|l| l.trim().strip_prefix(&want))
        .map(|v| v.trim().to_string())
}
