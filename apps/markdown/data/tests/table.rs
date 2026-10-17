//! A pipe table drawn by the real app, as a browser lays out a table: one
//! width per column, shared by every row and wide enough for the column's
//! longest word, and a table wider than the reading column scrolls inside it
//! (issues/20261016-markdown-on-macos-table-columns-do.md).
//!
//! The kernel measures with its monospace reference measurer, so a word of
//! `n` characters at 13 px is `n × 7.8` px wide wherever it is bold or not.

use exact_kernel::{Kernel, NodeRef, NodeType, Offer, PropId};
use exact_plan::Value;
use exact_runner::{Dispatch, Outcome, Runner, Viewport, Work};
use markdown_data::Markdown;
use serde_json::{json, Value as Json};

const WIDE: &str = "| Framework | App | Typing ms | Scroll ms | Find ms | Resize ms | Startup ms | Frames | Dropped | Sampled peak RAM MiB | Observed limits |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| ink | harness | 4.68 | 97.94 | 1,779,030.00 | 12.40 | 310.22 | 1,204 | 3 | 182.4 | redraws the whole tree on every keystroke |
| bubbletea | llp | 1.02 | 14.33 | 2,410.75 | 3.10 | 41.80 | 1,240 | 0 | 24.9 | wide tables wrap |
| exact | shelf | 0.61 | 6.20 | 410.00 | 1.40 | 18.30 | 1,238 | 1 | 31.7 | none seen |";

const NARROW: &str = "| Column | What it holds |
| --- | --- |
| path | where the file came from |
| blocks | what the parser made of it |";

/// The app at 1000 × 3000 with `text` open: tall enough that the
/// virtualized document mounts every block, too narrow for the folder pane
/// to matter (a file opened alone has none).
fn open(text: &str) -> Runner<Markdown> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../app.contract");
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e}"));
    let plan = contract::bake(plan, Markdown::new()).unwrap();
    let mut r = Runner::boot(
        plan,
        Markdown::new(),
        Kernel::with_monospace(),
        Viewport::sized(1000., 3000.),
        "/",
    )
    .unwrap();
    r.act("openFile", vec![Value::str("doc:/1/t/table.md")])
        .unwrap();
    // The host's half of an open: the storage it asks for, then the parse
    // on the worker. A watch is never answered: nothing changes on disk.
    for _ in 0..16 {
        let requests = r.take_requests();
        if requests.is_empty() {
            break;
        }
        for out in requests {
            let outcome = if let Some(token) = out.request.continuation {
                let Dispatch::Run(Work::Now(work)) = r.dispatch_work(token) else {
                    panic!("the parse is worker work")
                };
                work()
            } else if let Some(storage) = &out.request.storage {
                let asked: Json = serde_json::from_slice(storage).unwrap();
                let reply = match asked["op"].as_str().unwrap() {
                    "fs.stat" => {
                        json!({"size": text.len(), "isFile": true, "isDirectory": false, "modifiedMs": 0})
                    }
                    "fs.readFile" => {
                        json!({"base64": exact_runner::agent::base64(text.as_bytes())})
                    }
                    "fs.readdir" => json!([]),
                    _ => continue,
                };
                Outcome::Storage(serde_json::to_vec(&reply).unwrap())
            } else {
                continue;
            };
            r.fulfill(out.ticket, outcome).unwrap();
        }
    }
    let k = r.kernel_mut();
    let root = k
        .node_by_key(k.find_by_test_id("markdown-main")[0])
        .unwrap()
        .id;
    k.compute_layout(root, Offer::definite(1000., 3000.))
        .unwrap();
    r
}

/// The cell that holds `text`: the box around the run that says it.
fn cell<'a>(k: &'a Kernel, text: &str) -> NodeRef<'a> {
    let list = k.node_by_key(k.find_by_test_id("document")[0]).unwrap();
    let mut found = Vec::new();
    let mut stack = vec![list];
    while let Some(node) = stack.pop() {
        if node.props.str(PropId::Text) == Some(text) {
            found.push(k.node(node.parent.unwrap()).unwrap());
        }
        stack.extend(node.child_nodes());
    }
    assert_eq!(found.len(), 1, "one cell says {text:?}");
    found[0]
}

/// The reading column's content box, (left, right): 740 wide at most,
/// centred in the document, 32 of gutter on each side.
fn column(k: &Kernel) -> (f32, f32) {
    let list = k
        .node_by_key(k.find_by_test_id("document")[0])
        .unwrap()
        .frame;
    let width = list.width.min(740.);
    let left = list.x + (list.width - width) / 2. + 32.;
    (left, left + width - 64.)
}

/// The table's cells by column, from its header and body lines.
fn columns(table: &str) -> Vec<Vec<String>> {
    let rows: Vec<Vec<String>> = table
        .lines()
        .filter(|l| !l.contains("---"))
        .map(|l| {
            l.trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_owned())
                .collect()
        })
        .collect();
    (0..rows[0].len())
        .map(|c| rows.iter().map(|row| row[c].clone()).collect())
        .collect()
}

/// What holds for any table: a column's cells share its x and width, the
/// header spans what the body spans, and no cell is narrower than its
/// longest word and its padding, so no word runs into the next column.
fn lines_up(k: &Kernel, table: &str) -> Vec<(f32, f32)> {
    columns(table)
        .iter()
        .map(|cells| {
            let first = cell(k, &cells[0]).frame;
            for text in cells {
                let frame = cell(k, text).frame;
                assert_eq!(
                    (frame.x, frame.width),
                    (first.x, first.width),
                    "{text:?} lines up with {:?}",
                    cells[0]
                );
                let word = text.split(' ').map(|w| w.chars().count()).max().unwrap();
                assert!(
                    frame.width + 0.01 >= word as f32 * 7.8 + 16.,
                    "{text:?} fits its cell, {} wide",
                    frame.width
                );
            }
            (first.x, first.x + first.width)
        })
        .collect()
}

#[test]
fn a_tables_columns_fit_their_words_and_line_up_across_rows() {
    let r = open(&format!("# Report\n\n{WIDE}\n"));
    let k = r.kernel();
    let spans = lines_up(k, WIDE);
    let (left, right) = column(k);
    assert_eq!(spans[0].0, left, "the table starts at the reading column");
    // The words of eleven columns are wider than the reading column: what
    // runs past it scrolls inside it, never past its edge.
    let end = spans.last().unwrap().1;
    assert!(
        end > right,
        "the columns, {end}, outgrow the column, {right}"
    );
    let mut at = cell(k, "Framework");
    let scroller = loop {
        at = k
            .node(at.parent.expect("a scroller holds the table"))
            .unwrap();
        if at.node_type == NodeType::ScrollView {
            break at;
        }
    };
    let frame = scroller.frame;
    assert_eq!(
        (frame.x, frame.x + frame.width),
        (left, right),
        "the scroller is the reading column"
    );
    assert!(
        frame.x + scroller.content.0 >= end,
        "it scrolls to the last column: {:?} {frame:?}",
        scroller.content
    );
}

#[test]
fn a_table_that_fits_fills_the_column_with_columns_sized_by_their_content() {
    let r = open(&format!("# Notes\n\n{NARROW}\n"));
    let k = r.kernel();
    let spans = lines_up(k, NARROW);
    assert_eq!(
        (spans[0].0, spans[1].1),
        column(k),
        "the table fills the column"
    );
    let (short, long) = (spans[0].1 - spans[0].0, spans[1].1 - spans[1].0);
    assert!(
        short < long,
        "the column of single words, {short}, is narrower than the column of phrases, {long}"
    );
}
