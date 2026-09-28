//! Stress the shipped Markdown parser and Contract renderer with bounded inputs.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod fixture;
mod native;

pub use native::NativeMarkdownStress;

use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use fixture::{generate, Profile};
use markdown_parse::{Document, Kind};

/// Manual page size; this bounds block count, not text within one giant block.
pub const PAGE_BLOCKS: usize = 40;

struct Parsed {
    key: (Profile, usize, usize),
    bytes: usize,
    /// The generated source, for the one-node mode.
    text: String,
    document: Document,
    largest_block: usize,
    #[cfg(test)]
    drop_witness: Option<DropWitness>,
}

#[cfg(test)]
struct DropWitness(std::sync::Arc<std::sync::atomic::AtomicUsize>);

#[cfg(test)]
impl Drop for DropWitness {
    fn drop(&mut self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// One cached parsed fixture. Page/width/input changes do not reparse it.
/// Parsing is synchronous in this baseline, on every platform. No private worker.
#[derive(Default)]
pub struct MarkdownStress {
    parsed: Option<Parsed>,
}

fn integer(value: &Value, limit: usize) -> Result<usize, DataError> {
    match value.as_number() {
        Some(n) if n.is_finite() && n >= 0.0 && n <= limit as f64 && n.fract() == 0.0 => {
            Ok(n as usize)
        }
        _ => Err(DataError::BadArguments(format!(
            "expected an integer in 0..{limit}"
        ))),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DocumentArgs {
    key: (Profile, usize, usize),
    page: usize,
    eager: bool,
    /// Supply the whole source for one `markup` node (LLP 1045 D4).
    single: bool,
}

impl DocumentArgs {
    fn read(source: &str, args: &[Value]) -> Result<Self, DataError> {
        if source != "document" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let [profile @ exact_plan::str_value!(), budget, revision, page, Value::Bool(eager), Value::Bool(single)] =
            args
        else {
            return Err(DataError::BadArguments(
                "document(profile, budget, revision, page, eager, single)".into(),
            ));
        };
        let profile =
            Profile::parse(profile.text()).map_err(|e| DataError::BadArguments(e.into()))?;
        let budget = integer(budget, fixture::MAX_BYTES)?;
        let revision = integer(revision, 1000)?;
        let page = integer(page, fixture::MAX_BYTES)?;
        if !fixture::SIZES.contains(&budget) {
            return Err(DataError::BadArguments(
                "unsupported source byte budget".into(),
            ));
        }
        Ok(Self {
            key: (profile, budget, revision),
            page,
            eager: *eager,
            single: *single,
        })
    }
}

impl Parsed {
    fn build(key: (Profile, usize, usize)) -> Result<Self, DataError> {
        let text = generate(key.0, key.1).map_err(|e| DataError::BadArguments(e.into()))?;
        let document = markdown_parse::parse(&text, &str::to_owned);
        let largest_block = document
            .blocks
            .iter()
            .map(|b| {
                b.text.len()
                    + b.runs.iter().map(|r| r.text.len()).sum::<usize>()
                    + b.cells
                        .iter()
                        .flatten()
                        .map(|r| r.text.len())
                        .sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        Ok(Self {
            key,
            bytes: text.len(),
            text,
            document,
            largest_block,
            #[cfg(test)]
            drop_witness: None,
        })
    }

    fn value(&self, args: DocumentArgs) -> Value {
        let total = self.document.blocks.len();
        let first = if args.eager {
            0
        } else {
            args.page.min((total - 1) / PAGE_BLOCKS) * PAGE_BLOCKS
        };
        let end = if args.eager {
            total
        } else {
            (first + PAGE_BLOCKS).min(total)
        };
        let blocks = self.document.blocks[first..end]
            .iter()
            .enumerate()
            .map(|(i, block)| markdown_parse::value::block(first + i, block))
            .collect();
        Value::record(vec![
            Value::str(&self.document.title),
            Value::Number(self.bytes as f64),
            Value::Number(total as f64),
            Value::Number((end - first) as f64),
            Value::Number(first as f64),
            Value::Number(end as f64),
            Value::Number(args.key.2 as f64),
            Value::Number(self.largest_block as f64),
            Value::Number(
                self.document
                    .blocks
                    .iter()
                    .filter(|b| b.kind == Kind::TableRow)
                    .count() as f64,
            ),
            Value::list(blocks),
            Value::str(if args.single { &self.text } else { "" }),
        ])
    }
}

impl DataSource for MarkdownStress {
    fn app_id(&self) -> &str {
        "com.exact.markdown-stress"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == "theme" && args.is_empty() {
            return Ok(markdown_parse::theme::value());
        }
        let args = DocumentArgs::read(source, args)?;
        if self.parsed.as_ref().is_none_or(|p| p.key != args.key) {
            self.parsed = Some(Parsed::build(args.key)?);
        }
        Ok(self.parsed.as_ref().unwrap().value(args))
    }
}
