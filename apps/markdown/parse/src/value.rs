//! The parsed document as the plan's own value, in the shapes both readers
//! declare. One conversion, so `apps/markdown` and `apps/llp` cannot drift
//! into two documents that render differently. @ref LLP 1033

use crate::{Block, Document, Run};
use exact_plan::Value;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Sharing is scoped to one conversion: no global pool retains closed files.
/// Owned lookup keys let a consuming conversion release parsed blocks as it goes.
struct Values {
    strings: HashSet<Rc<str>>,
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
        let text = self.strings.get(text).cloned().unwrap_or_else(|| {
            let text: Rc<str> = Rc::from(text);
            self.strings.insert(text.clone());
            text
        });
        Value::Str(text)
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

    fn cells(&mut self, list: &[Vec<Run>]) -> Value {
        if list.is_empty() {
            return self.empty_list.clone();
        }
        Value::list(
            list.iter()
                .enumerate()
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
            self.cells(&b.cells),
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

/// The `list<Block>` a document's `blocks` field takes. The top-level
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

/// Convert an owned document, releasing each parsed block after conversion so
/// its allocations can serve later values instead of retaining both full forms.
pub fn into_blocks(doc: Document) -> Value {
    let mut values = Values::new();
    // An in-place IntoIter collect can keep the much larger Vec<Block>
    // allocation as the Value list's spare capacity. Allocate its exact size.
    let mut blocks = Vec::with_capacity(doc.blocks.len());
    for (i, block) in doc.blocks.into_iter().enumerate() {
        blocks.push(values.block(i, &block));
    }
    Value::list(blocks)
}
