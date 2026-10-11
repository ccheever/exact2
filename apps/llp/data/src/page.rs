//! `page(path, find)`: the open document as the terminal reader draws it
//! (`apps/llp/terminal.contract`, LLP 1101).
//!
//! The same parsed blocks as `document`, with three things a reader in a
//! terminal needs and the windowed reader does not ask for:
//!
//! - **Find in the page.** Every run is split where `find` matches, case
//!   insensitively, and each match carries its ordinal, so the screen can
//!   paint the current one apart from the rest; `marks` is the block each
//!   match is in, which `scrollIntoView` brings to the screen. A match that
//!   crosses two runs (half of it bold) is not found.
//! - **Links as a list.** A terminal cannot follow a click on a run of text,
//!   so the page lists its links, and says which name a document in this
//!   corpus (the reader opens those itself).
//! - **Neighbours.** The documents before and after this one in number
//!   order, for paging through a corpus.
//!
//! Block ids are `b0`, `b1`, …: an HTML `id` the screen puts on each block,
//! and what `marks` and the outline name.

use crate::Entry;
use exact_plan::Value;
use markdown_parse::{Document, Kind, Run};
use std::path::Path;

/// Where the page sits in its corpus, and the text to find in it.
pub(crate) struct Place<'a> {
    pub entry: Option<&'a Entry>,
    pub prev: String,
    pub next: String,
    pub find: &'a str,
    /// A path's document in this corpus, as its number and title, or
    /// `None` for anything else.
    pub local: &'a dyn Fn(&str) -> Option<String>,
}

/// `shape Page`.
pub(crate) fn value(path: &Path, doc: &Document, place: &Place<'_>, message: &str) -> Value {
    let find = place.find.trim().to_lowercase();
    let mut hits = 0usize;
    let mut marks = Vec::new();
    let mut links = Vec::new();
    let mut blocks = Vec::with_capacity(doc.blocks.len());
    let columns = columns(doc);
    for (i, b) in doc.blocks.iter().enumerate() {
        // An LLP's metadata header, one row per field.
        if let Some(fields) = metadata(i, b) {
            for (k, (label, runs)) in fields.into_iter().enumerate() {
                let id = format!("b{i}m{k}");
                let before = hits;
                let mut out = Vec::new();
                for r in &runs {
                    if !r.href.is_empty() && !r.text.trim().is_empty() {
                        links.push((r.text.clone(), r.href.clone(), id.clone()));
                    }
                    split(&id, r, false, &find, &mut hits, &mut out);
                }
                marks.extend(std::iter::repeat_n(id.clone(), hits - before));
                blocks.push(Value::record(vec![
                    Value::str(&id),
                    Value::str("meta"),
                    Value::Number(0.0),
                    Value::str(&label),
                    Value::str(""),
                    Value::Bool(false),
                    Value::list(out),
                    Value::list(vec![]),
                ]));
            }
            continue;
        }
        let id = format!("b{i}");
        let before = hits;
        // Spans are named under `prefix` (the block's id, or a cell's), so a
        // span's id is the page's only one of that name.
        let mut spans_of = |prefix: &str, runs: &[Run], mono: bool| {
            let mut out = Vec::new();
            for r in runs {
                if !r.href.is_empty() && !r.text.trim().is_empty() {
                    links.push((r.text.clone(), r.href.clone(), id.clone()));
                }
                split(prefix, r, mono, &find, &mut hits, &mut out);
            }
            Value::list(out)
        };
        let (spans, cells) = match b.kind {
            Kind::Code | Kind::Html => {
                let run = Run {
                    text: b.text.clone(),
                    code: true,
                    ..Run::default()
                };
                (
                    spans_of(&id, std::slice::from_ref(&run), true),
                    Value::list(vec![]),
                )
            }
            Kind::TableRow => {
                let cells = b
                    .cells
                    .iter()
                    .enumerate()
                    .map(|(c, runs)| {
                        let grow = columns[i].get(c).copied().unwrap_or(1);
                        let cell = format!("{id}c{c}");
                        Value::record(vec![
                            Value::str(&cell),
                            spans_of(&cell, runs, false),
                            Value::Number(grow as f64),
                        ])
                    })
                    .collect();
                (Value::list(vec![]), Value::list(cells))
            }
            Kind::Image => {
                let run = Run {
                    text: b.text.clone(),
                    ..Run::default()
                };
                (
                    spans_of(&id, std::slice::from_ref(&run), false),
                    Value::list(vec![]),
                )
            }
            _ => (spans_of(&id, &b.runs, false), Value::list(vec![])),
        };
        marks.extend(std::iter::repeat_n(id.clone(), hits - before));
        blocks.push(Value::record(vec![
            Value::str(&id),
            Value::str(b.kind.name()),
            Value::Number(b.depth as f64),
            Value::str(&b.marker),
            Value::str(if b.kind == Kind::Code { &b.href } else { "" }),
            Value::Bool(b.header),
            spans,
            cells,
        ]));
    }
    let outline = doc
        .outline()
        .iter()
        .map(|(level, text, at)| {
            Value::record(vec![
                Value::str(&format!("b{at}")),
                Value::str(text),
                Value::Number(*level as f64),
            ])
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let links = links
        .into_iter()
        // A document's own number (its title's `LLP 0001`) goes nowhere new.
        .filter(|(_, href, _)| Path::new(href) != path)
        .filter(|(text, href, _)| seen.insert((text.clone(), href.clone())))
        .enumerate()
        .map(|(i, (text, href, block))| {
            let local = (place.local)(&href);
            Value::record(vec![
                Value::str(&i.to_string()),
                Value::str(&text),
                Value::Bool(local.is_some()),
                Value::str(&href),
                Value::str(&block),
                Value::str(local.as_deref().unwrap_or(&href)),
            ])
        })
        .collect();
    let entry = place.entry;
    Value::record(vec![
        Value::str(&path.to_string_lossy()),
        Value::str(
            &path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        Value::str(entry.map(|e| e.number.as_str()).unwrap_or("")),
        // The index's title, without its own `LLP NNNN:` prefix.
        Value::str(entry.map(|e| e.title.as_str()).unwrap_or(&doc.title)),
        Value::str(entry.map(|e| e.kind.as_str()).unwrap_or("")),
        Value::str(entry.map(|e| e.status.as_str()).unwrap_or("")),
        Value::str(message),
        Value::Bool(!message.is_empty()),
        Value::list(blocks),
        Value::list(outline),
        Value::Number(hits as f64),
        Value::list(marks.iter().map(|m| Value::str(m)).collect()),
        Value::list(links),
        Value::str(&place.prev),
        Value::str(&place.next),
    ])
}

/// Each table row's column weights, by block: a column's widest cell in its
/// table, in characters, from 2 to 48. The screen grows each cell by its
/// column's weight from nothing, so the columns line up across rows and a
/// `#` column is not as wide as a description.
fn columns(doc: &Document) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); doc.blocks.len()];
    let mut i = 0;
    while i < doc.blocks.len() {
        if doc.blocks[i].kind != Kind::TableRow {
            i += 1;
            continue;
        }
        let end = (i..doc.blocks.len())
            .find(|&j| doc.blocks[j].kind != Kind::TableRow)
            .unwrap_or(doc.blocks.len());
        let mut widths: Vec<usize> = Vec::new();
        for b in &doc.blocks[i..end] {
            for (c, runs) in b.cells.iter().enumerate() {
                let w: usize = runs.iter().map(|r| r.text.chars().count()).sum();
                if widths.len() <= c {
                    widths.push(2);
                }
                widths[c] = widths[c].max(w.clamp(2, 48));
            }
        }
        for row in &mut out[i..end] {
            *row = widths.clone();
        }
        i = end;
    }
    out
}

/// The fields of an LLP's metadata header — a paragraph near the top that
/// starts `**Type:**` and runs on as `**Status:** … **Author:** …` (its
/// line breaks are soft, so Markdown joins them) — as (label, value runs).
/// Block ids for these are `b{i}m{k}`. Not a header: `None`.
fn metadata(i: usize, b: &markdown_parse::Block) -> Option<Vec<(String, Vec<Run>)>> {
    let label = |r: &Run| r.bold && r.text.trim_end().ends_with(':');
    if i > 4 || b.kind != Kind::Paragraph || !b.runs.first().is_some_and(label) {
        return None;
    }
    let mut fields: Vec<(String, Vec<Run>)> = Vec::new();
    for r in &b.runs {
        if label(r) {
            fields.push((r.text.trim().trim_end_matches(':').to_string(), Vec::new()));
        } else if let Some((_, runs)) = fields.last_mut() {
            let mut r = r.clone();
            if runs.is_empty() {
                r.text = r.text.trim_start().to_string();
            }
            runs.push(r);
        }
    }
    (fields.len() >= 2).then_some(fields)
}

/// One run as spans: the stretches between matches, and each match with its
/// ordinal (from 1). Text whose lowercase form changes length is not split
/// (its byte offsets would not line up), so it finds nothing.
fn split(prefix: &str, run: &Run, mono: bool, find: &str, hits: &mut usize, out: &mut Vec<Value>) {
    let push = |text: &str, hit: usize, out: &mut Vec<Value>| {
        if text.is_empty() {
            return;
        }
        out.push(Value::record(vec![
            Value::str(&format!("{prefix}s{}", out.len())),
            Value::str(text),
            Value::Number(if run.bold { 700.0 } else { 400.0 }),
            Value::str(if run.italic { "italic" } else { "normal" }),
            Value::Bool(mono || run.code),
            Value::str(&run.href),
            Value::Number(hit as f64),
        ]));
    };
    let lower = run.text.to_lowercase();
    if find.is_empty() || lower.len() != run.text.len() {
        push(&run.text, 0, out);
        return;
    }
    let mut at = 0;
    while let Some(found) = lower[at..].find(find) {
        let start = at + found;
        let end = start + find.len();
        push(&run.text[at..start], 0, out);
        *hits += 1;
        push(&run.text[start..end], *hits, out);
        at = end;
    }
    push(&run.text[at..], 0, out);
}
