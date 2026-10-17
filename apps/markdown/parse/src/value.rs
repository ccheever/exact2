//! The parsed document as the plan's own value, in the shapes both readers
//! declare. One conversion, so `apps/markdown` and `apps/llp` cannot drift
//! into two documents that render differently; a table is the one place
//! they still differ ([`into_blocks_with_tables`]). @ref LLP 1033

use crate::{Block, Document, Kind, Run};
use exact_plan::{InlineStr, Str, Value};
use std::collections::{HashMap, HashSet};

/// Sharing is scoped to one conversion: no global pool retains closed files.
/// Owned lookup keys let a consuming conversion release parsed blocks as it goes.
struct Values {
    strings: HashSet<Str>,
    indices: HashMap<usize, Value>,
    empty_list: Value,
}

impl Values {
    fn new() -> Self {
        Self {
            strings: HashSet::new(),
            indices: HashMap::new(),
            empty_list: Value::list(Vec::new()),
        }
    }

    fn string(&mut self, text: &str) -> Value {
        // Short text is held in the value itself: nothing to share.
        if text.len() <= InlineStr::CAP {
            return Value::str(text);
        }
        let text = self.strings.get(text).cloned().unwrap_or_else(|| {
            let text = Str::from(text);
            self.strings.insert(text.clone());
            text
        });
        Value::from(text)
    }

    fn index(&mut self, index: usize) -> Value {
        self.indices
            .entry(index)
            .or_insert_with(|| Value::str(&index.to_string()))
            .clone()
    }

    fn run(&mut self, index: usize, run: &Run) -> Value {
        Value::record(vec![
            self.index(index),
            self.string(&run.text),
            Value::Number(if run.bold { 700.0 } else { 400.0 }),
            self.string(if run.italic { "italic" } else { "normal" }),
            Value::Bool(run.code),
            self.string(&run.href),
        ])
    }

    fn runs(&mut self, list: &[Run]) -> Value {
        if list.is_empty() {
            return self.empty_list.clone();
        }
        Value::list(
            list.iter()
                .enumerate()
                .map(|(i, r)| self.run(i, r))
                .collect(),
        )
    }

    fn cells<'a>(&mut self, list: impl ExactSizeIterator<Item = &'a [Run]>) -> Value {
        if list.len() == 0 {
            return self.empty_list.clone();
        }
        Value::list(
            list.enumerate()
                .map(|(i, c)| Value::record(vec![self.index(i), self.runs(c)]))
                .collect(),
        )
    }

    fn block(&mut self, index: usize, b: &Block) -> Value {
        Value::record(vec![
            self.index(index),
            self.string(b.kind.name()),
            Value::Number(b.depth as f64),
            self.string(&b.marker),
            self.string(&b.text),
            self.string(&b.href),
            Value::Bool(b.header),
            self.runs(&b.runs),
            self.cells(b.cells.iter().map(Vec::as_slice)),
        ])
    }

    /// One table's rows as one block (see [`into_blocks_with_tables`]).
    fn table(&mut self, index: usize, rows: &[Block]) -> Value {
        let columns = rows[0].cells.len();
        let cells: Vec<&[Run]> = rows
            .iter()
            .flat_map(|row| (0..columns).map(|c| row.cells.get(c).map_or(&[][..], Vec::as_slice)))
            .collect();
        Value::record(vec![
            self.index(index),
            self.string("table"),
            Value::Number(columns as f64),
            self.string(""),
            self.string(""),
            self.string(""),
            Value::Bool(false),
            self.empty_list.clone(),
            self.cells(cells.into_iter()),
        ])
    }
}

/// `shape Run` — one styled span. `weight` and `slant` are the CSS values
/// the run's `text` node takes directly; `mono` is a bool because
/// `font-family` is literal-only in Contract v1 and the reader branches on it.
pub fn run(index: usize, run: &Run) -> Value {
    Values::new().run(index, run)
}

/// `shape Block`.
pub fn block(index: usize, b: &Block) -> Value {
    Values::new().block(index, b)
}

/// The `Block[]` a document's `blocks` field takes. The top-level
/// document record is each app's own — `apps/markdown` carries the folder
/// beside the file, `apps/llp` the document's place in the index — and this
/// is the part they must not write twice.
pub fn blocks(doc: &Document) -> Value {
    let mut values = Values::new();
    Value::list(
        doc.blocks
            .iter()
            .enumerate()
            .map(|(i, b)| values.block(i, b))
            .collect(),
    )
}

/// `apps/markdown`'s `Block[]`, from an owned document: [`blocks`], except
/// that each pipe table is one block, which the reader draws as one grid so
/// that every row shares its columns, as a browser's table does. That block
/// is `kind` `"table"`, `depth` its column count (its header row's cells),
/// and `cells` the header's cells and then each row's, a row padded with
/// empty cells or cut to the header's count, as GitHub Flavored Markdown
/// says (<https://github.github.com/gfm/#tables-extension->). A block's id
/// is its place in this list. `apps/llp` still draws a table row by row.
///
/// Each parsed block is released once it is converted (a table's rows once
/// the table ends), so its allocations can serve later values instead of
/// retaining both full forms.
pub fn into_blocks_with_tables(doc: Document) -> Value {
    let mut values = Values::new();
    // An in-place IntoIter collect can keep the much larger Vec<Block>
    // allocation as the Value list's spare capacity. Allocate it apart.
    let mut blocks = Vec::with_capacity(doc.blocks.len());
    let mut rows: Vec<Block> = Vec::new();
    for block in doc.blocks {
        // A header row starts a table, also right after another table.
        if !rows.is_empty() && (block.kind != Kind::TableRow || block.header) {
            blocks.push(values.table(blocks.len(), &rows));
            rows.clear();
        }
        if block.kind == Kind::TableRow {
            rows.push(block);
        } else {
            blocks.push(values.block(blocks.len(), &block));
        }
    }
    if !rows.is_empty() {
        blocks.push(values.table(blocks.len(), &rows));
    }
    Value::list(blocks)
}
