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
use std::io::Read;
use std::ops::{Deref, Range};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// One document's uncompressed text in a shared corpus buffer.
/// Cloning it keeps the snapshot alive without copying its text.
#[derive(Clone, Default)]
pub struct CorpusText {
    text: Arc<String>,
    range: Range<usize>,
    lowercase_expands_to_ascii: OnceLock<bool>,
}

impl CorpusText {
    fn count_lowercase_matches(&self, query: &str) -> usize {
        count_lowercase_matches_with_guard(self, query, || {
            *self
                .lowercase_expands_to_ascii
                .get_or_init(|| lowercase_expands_to_ascii(self))
        })
    }
}

impl Deref for CorpusText {
    type Target = str;

    fn deref(&self) -> &str {
        &self.text[self.range.clone()]
    }
}

impl std::fmt::Debug for CorpusText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, f)
    }
}

impl PartialEq for CorpusText {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for CorpusText {}

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
    pub body: CorpusText,
}

impl Entry {
    /// Search an already lowercased query in the metadata and body.
    /// A match carries its body hit count; metadata alone can match with
    /// zero hits. The empty query includes every entry with zero hits.
    pub fn search(&self, query: &str) -> Option<usize> {
        if query.is_empty() {
            return Some(0);
        }
        let hits = self.body.count_lowercase_matches(query);
        (hits > 0
            || self.number.contains(query)
            || self.title.to_lowercase().contains(query)
            || self.kind.contains(query)
            || self.status.to_lowercase().contains(query))
        .then_some(hits)
    }
}

fn lowercase_expands_to_ascii(text: &str) -> bool {
    text.contains('\u{130}') || text.contains('\u{212a}')
}

#[cfg(test)]
fn count_lowercase_matches(text: &str, query: &str) -> usize {
    count_lowercase_matches_with_guard(text, query, || lowercase_expands_to_ascii(text))
}

// ASCII needles can be counted directly without allocating a lowercase
// copy of every body. Unicode queries and the two non-ASCII characters
// whose lowercase contains ASCII keep str::to_lowercase's full semantics.
fn count_lowercase_matches_with_guard(
    text: &str,
    query: &str,
    lowercase_expands_to_ascii: impl FnOnce() -> bool,
) -> usize {
    if query.is_empty()
        || !query.is_ascii()
        || query.bytes().any(|b| b.is_ascii_uppercase())
        || lowercase_expands_to_ascii()
    {
        return text.to_lowercase().matches(query).count();
    }
    let needle = query;
    let query = query.as_bytes();
    let mut rest = text.as_bytes();
    let mut hits = 0;
    let first = query[0];
    // A repetitive body and needle must not turn this into quadratic work.
    // Charge each candidate's maximum comparison length, then use std's
    // substring search once that work would exceed one pass over the body.
    let mut candidates = text.len() / query.len();
    while let Some(at) = memchr::memchr2(first, first.to_ascii_uppercase(), rest) {
        rest = &rest[at..];
        if rest.len() < query.len() {
            break;
        }
        if candidates == 0 {
            return text.to_lowercase().matches(needle).count();
        }
        candidates -= 1;
        if rest[..query.len()].eq_ignore_ascii_case(query) {
            hits += 1;
            rest = &rest[query.len()..];
        } else {
            rest = &rest[1..];
        }
    }
    hits
}

/// Read every Markdown document under `root`, in number order.
///
/// Documents that do not follow the convention are still listed — a corpus
/// with a `README.md` in it should not look empty — they simply sort last
/// and carry no number.
pub fn read(root: &Path) -> Result<Vec<Entry>, String> {
    let overlays = overlays(root);
    let files: Vec<_> = std::fs::read_dir(root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .filter_map(|p| {
            let meta = p.metadata().ok()?;
            meta.is_file().then_some((p, meta.len() as usize))
        })
        .collect();
    // One allocation avoids rounding each document up to an allocator size
    // class and leaving holes between their pages. Reserve a little spare
    // space for the read that confirms EOF; growing files may still extend it.
    let bytes: usize = files.iter().map(|(_, len)| len).sum();
    let mut text = String::with_capacity(bytes.saturating_add(32));
    let mut ranges = Vec::with_capacity(files.len());
    for (path, _) in files {
        let start = text.len();
        if std::fs::File::open(&path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .is_err()
        {
            text.truncate(start);
            continue;
        }
        ranges.push((path, start..text.len()));
    }
    // Arc<String> moves the existing buffer; Arc<str> would copy it, briefly
    // retaining the corpus twice at startup.
    let text = Arc::new(text);
    let mut entries: Vec<Entry> = ranges
        .into_iter()
        .filter_map(|(path, range)| entry(&path, &text, range, &overlays))
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

fn entry(
    path: &Path,
    text: &Arc<String>,
    range: Range<usize>,
    overlays: &[(String, BTreeSet<String>)],
) -> Option<Entry> {
    let file = path.file_name()?.to_string_lossy().into_owned();
    let body = &text[range.clone()];
    let (number, kind) = name_parts(&file);
    let overlay = overlays
        .iter()
        .find(|(_, names)| names.contains(&file))
        .map(|(name, _)| name.clone())
        .unwrap_or_default();
    Some(Entry {
        depth: if number.contains('.') { 1 } else { 0 },
        title: title(body).unwrap_or_else(|| file.clone()),
        status: status(body),
        number,
        kind,
        path: path.to_path_buf(),
        overlay,
        body: CorpusText {
            text: Arc::clone(text),
            range,
            lowercase_expands_to_ascii: OnceLock::new(),
        },
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

#[cfg(test)]
mod search_tests {
    use super::{count_lowercase_matches, CorpusText};
    use std::sync::{Arc, OnceLock};

    #[test]
    fn corpus_text_caches_the_guard_for_its_exact_range() {
        let text = Arc::new("İplainK".to_string());
        let plain = CorpusText {
            text: Arc::clone(&text),
            range: 2..7,
            lowercase_expands_to_ascii: OnceLock::new(),
        };
        assert_eq!(plain.count_lowercase_matches("K"), 0);
        assert!(plain.lowercase_expands_to_ascii.get().is_none());
        assert_eq!(plain.count_lowercase_matches("i"), 1);
        assert_eq!(plain.lowercase_expands_to_ascii.get(), Some(&false));

        let kelvin = CorpusText {
            text,
            range: 7..10,
            lowercase_expands_to_ascii: OnceLock::new(),
        };
        assert_eq!(kelvin.count_lowercase_matches("k"), 1);
        assert_eq!(kelvin.lowercase_expands_to_ascii.get(), Some(&true));
        assert_eq!(kelvin.clone().lowercase_expands_to_ascii.get(), Some(&true));
    }

    #[test]
    fn ascii_search_keeps_unicode_lowercase_and_nonoverlapping_counts() {
        for text in [
            "",
            "AaAaA",
            "kernel KERNEL Kernel",
            "a kernelk",
            "ΟΣ ΟΣΑ Σ ΣΣ",
            "İstanbul I i\u{307} Kernel KERNEL",
            "aKa aKa",
            "aİa aIa",
            "界e\u{301}日本語 К k K",
            "Σ'Σ \u{301}Σ 𐐀𐐨",
            "!?!?\n!?!?",
        ] {
            for query in [
                "", "a", "aa", "kernel", "k", "i", "aka", "aia", "K", "i\u{307}", "ος", "σ", "!?",
            ] {
                assert_eq!(
                    count_lowercase_matches(text, query),
                    text.to_lowercase().matches(query).count(),
                    "text {text:?}, query {query:?}",
                );
            }
        }
        let mut seed = 0xabad_1deau64;
        let chars = [
            'A', 'a', 'k', 'K', 'I', 'i', 'Σ', 'Ο', 'Α', 'İ', 'K', '\u{307}', '\u{301}', '\'', ' ',
            '!', '\n', '界',
        ];
        for _ in 0..4000 {
            let text: String = (0..100)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    chars[seed as usize % chars.len()]
                })
                .collect();
            for query in [
                "a", "aa", "aka", "aia", "i", "k", "a k", "\n", "'", "α", "ος", "οσ", "i\u{307}",
                "K",
            ] {
                assert_eq!(
                    count_lowercase_matches(&text, query),
                    text.to_lowercase().matches(query).count(),
                    "{text:?} {query:?}"
                );
            }
        }
    }

    #[test]
    fn repetitive_needles_keep_the_count_when_the_comparison_budget_is_exhausted() {
        for width in [7, 31, 256] {
            let query = format!("{}b{}", "a".repeat(width), "a".repeat(width));
            for tail in ["", query.as_str()] {
                let text = format!("{}{}", "a".repeat(100_000), tail);
                assert_eq!(
                    count_lowercase_matches(&text, &query),
                    text.to_lowercase().matches(query.as_str()).count()
                );
            }
        }
    }

    #[test]
    fn every_non_ascii_lowercase_that_can_match_an_ascii_query_uses_the_fallback() {
        // Changing Unicode versions must not silently widen the set for which
        // matching the original bytes would miss an ASCII result.
        let produces_ascii: Vec<_> = (128..=0x10ffff)
            .filter_map(char::from_u32)
            .filter(|c| c.to_lowercase().any(|lower| lower.is_ascii()))
            .collect();
        assert_eq!(produces_ascii, ['İ', 'K']);
    }
}
