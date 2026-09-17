//! Stress the shipped Markdown parser and Contract renderer with bounded inputs.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod fixture;

use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use fixture::{generate, Profile};
use markdown_parse::{Document, Kind};

/// Manual page size; this bounds block count, not text within one giant block.
pub const PAGE_BLOCKS: usize = 40;

struct Parsed {
    key: (Profile, usize, usize),
    bytes: usize,
    document: Document,
    largest_block: usize,
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

impl DataSource for MarkdownStress {
    fn app_id(&self) -> &str {
        "com.exact.markdown-stress"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == "theme" && args.is_empty() {
            return Ok(markdown_parse::theme::value());
        }
        if source != "document" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let [Value::Str(profile), budget, revision, page, Value::Bool(eager)] = args else {
            return Err(DataError::BadArguments(
                "document(profile, budget, revision, page, eager)".into(),
            ));
        };
        let profile = Profile::parse(profile).map_err(|e| DataError::BadArguments(e.into()))?;
        let budget = integer(budget, fixture::MAX_BYTES)?;
        let revision = integer(revision, 1000)?;
        let page = integer(page, fixture::MAX_BYTES)?;
        if !fixture::SIZES.contains(&budget) {
            return Err(DataError::BadArguments(
                "unsupported source byte budget".into(),
            ));
        }
        let key = (profile, budget, revision);
        if self.parsed.as_ref().is_none_or(|p| p.key != key) {
            let text = generate(profile, budget).map_err(|e| DataError::BadArguments(e.into()))?;
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
            self.parsed = Some(Parsed {
                key,
                bytes: text.len(),
                document,
                largest_block,
            });
        }
        let parsed = self.parsed.as_ref().unwrap();
        let total = parsed.document.blocks.len();
        let first = if *eager {
            0
        } else {
            page.min((total - 1) / PAGE_BLOCKS) * PAGE_BLOCKS
        };
        let end = if *eager {
            total
        } else {
            (first + PAGE_BLOCKS).min(total)
        };
        let blocks = parsed.document.blocks[first..end]
            .iter()
            .enumerate()
            .map(|(i, block)| markdown_parse::value::block(first + i, block))
            .collect();
        Ok(Value::record(vec![
            Value::str(&parsed.document.title),
            Value::Number(parsed.bytes as f64),
            Value::Number(total as f64),
            Value::Number((end - first) as f64),
            Value::Number(first as f64),
            Value::Number(end as f64),
            Value::Number(revision as f64),
            Value::Number(parsed.largest_block as f64),
            Value::Number(
                parsed
                    .document
                    .blocks
                    .iter()
                    .filter(|b| b.kind == Kind::TableRow)
                    .count() as f64,
            ),
            Value::list(blocks),
        ]))
    }
}
