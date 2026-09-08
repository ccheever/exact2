//! `LLP 1234` in running prose, resolved to the document it names.
//!
//! A corpus cites itself constantly — "LLP 1030 D7", "RFC 0491 §3" — and a
//! reader that leaves those as text is a reader you navigate by scrolling
//! the sidebar. Splitting a run at each reference is all this is; whether a
//! number resolves is the index's business, and one that does not stays
//! plain text rather than becoming a link to nothing. @ref LLP 1033

use markdown_parse::Run;

/// The prefixes a reference is written with in this corpus.
const PREFIXES: [&str; 2] = ["LLP ", "RFC "];

/// Split every run at each reference whose number the corpus knows,
/// returning the runs a paragraph is then made of.
pub fn link(runs: Vec<Run>, find: &dyn Fn(&str) -> Option<String>) -> Vec<Run> {
    let mut out = Vec::with_capacity(runs.len());
    for run in runs {
        // A code span is code, and a link already goes somewhere.
        if run.code || !run.href.is_empty() {
            out.push(run);
            continue;
        }
        split(&run, find, &mut out);
    }
    out
}

fn split(run: &Run, find: &dyn Fn(&str) -> Option<String>, out: &mut Vec<Run>) {
    let text = &run.text;
    let mut at = 0;
    let mut plain = 0;
    while at < text.len() {
        let Some((start, prefix)) = next_reference(text, at) else {
            break;
        };
        let number_at = start + prefix.len();
        let number = number(&text[number_at..]);
        let Some(path) = (if number.is_empty() {
            None
        } else {
            find(&number)
        }) else {
            at = number_at.max(at + 1);
            continue;
        };
        if plain < start {
            out.push(Run {
                text: text[plain..start].to_string(),
                ..run.clone()
            });
        }
        let end = number_at + number.len();
        out.push(Run {
            text: text[start..end].to_string(),
            href: path,
            ..run.clone()
        });
        plain = end;
        at = end;
    }
    if plain < text.len() {
        out.push(Run {
            text: text[plain..].to_string(),
            ..run.clone()
        });
    }
}

/// The next `LLP `/`RFC ` at or after `from` that begins a word.
fn next_reference(text: &str, from: usize) -> Option<(usize, &'static str)> {
    let mut best: Option<(usize, &'static str)> = None;
    for prefix in PREFIXES {
        let mut at = from;
        while let Some(offset) = text[at..].find(prefix) {
            let start = at + offset;
            let before = text[..start].chars().next_back();
            if before.map(|c| c.is_alphanumeric()).unwrap_or(false) {
                at = start + prefix.len();
                continue;
            }
            if best.map(|(b, _)| start < b).unwrap_or(true) {
                best = Some((start, prefix));
            }
            break;
        }
    }
    best
}

/// The digits (and one dotted part) a reference's number is made of.
fn number(text: &str) -> String {
    let major: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    if major.is_empty() {
        return major;
    }
    let rest = &text[major.len()..];
    match rest.strip_prefix('.') {
        Some(after) => {
            let minor: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if minor.is_empty() {
                major
            } else {
                format!("{major}.{minor}")
            }
        }
        None => major,
    }
}
