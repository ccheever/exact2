//! The parsed document as the plan's own value, in the shapes both readers
//! declare. One conversion, so `apps/markdown` and `apps/llp` cannot drift
//! into two documents that render differently. @ref LLP 1033

use crate::{Block, Document, Run};
use exact_plan::Value;

/// `shape Run` — one styled span. `weight` and `slant` are the CSS values
/// the run's `text` node takes directly; `mono` is a bool because
/// `font-family` is literal-only in Contract v1 and the reader branches on it.
pub fn run(index: usize, run: &Run) -> Value {
    Value::record(vec![
        Value::str(&index.to_string()),
        Value::str(&run.text),
        Value::Number(if run.bold { 700.0 } else { 400.0 }),
        Value::str(if run.italic { "italic" } else { "normal" }),
        Value::Bool(run.code),
        Value::str(&run.href),
    ])
}

fn runs(list: &[Run]) -> Value {
    Value::list(list.iter().enumerate().map(|(i, r)| run(i, r)).collect())
}

/// `shape Cell` — one table cell.
fn cells(list: &[Vec<Run>]) -> Value {
    Value::list(
        list.iter()
            .enumerate()
            .map(|(i, c)| Value::record(vec![Value::str(&i.to_string()), runs(c)]))
            .collect(),
    )
}

/// `shape Block`.
pub fn block(index: usize, b: &Block) -> Value {
    Value::record(vec![
        Value::str(&index.to_string()),
        Value::str(b.kind.name()),
        Value::Number(b.depth as f64),
        Value::str(&b.marker),
        Value::str(&b.text),
        Value::str(&b.href),
        Value::Bool(b.header),
        runs(&b.runs),
        cells(&b.cells),
    ])
}

/// The `list<Block>` a document's `blocks` field takes. The top-level
/// document record is each app's own — `apps/markdown` carries the folder
/// beside the file, `apps/llp` the document's place in the index — and this
/// is the part they must not write twice.
pub fn blocks(doc: &Document) -> Value {
    Value::list(
        doc.blocks
            .iter()
            .enumerate()
            .map(|(i, b)| block(i, b))
            .collect(),
    )
}
