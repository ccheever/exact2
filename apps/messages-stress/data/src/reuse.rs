//! Default synchronous runtime source; the stateless generator remains the control.
//! Keep one latest immutable result, not a cache of revisions or visited pages.

use exact_plan::{Items, Value};
use exact_runner::{DataError, DataSource};

use crate::{
    integer,
    model::{page, Controls, Row, MAX_DRAFT_CHARS, PAGE_SIZE},
    MessagesStress,
};

/// Runtime source with exactly the stateless control's history values.
///
/// Cold/range/count replacement uses the original generator. Same-range updates
/// copy O(N) row handles and replace only changed records. Fresh answer shape,
/// key/index reconciliation and layout remain Runner work; this is not O(batch)
/// total work or a frame-time guarantee.
/// Cursor-selected windows use the same bounded generator as the control;
/// both modes share one latest result and the nine-field History shape.
#[derive(Default)]
pub struct ReusableMessagesStress {
    control: MessagesStress,
    latest: Option<Latest>,
}

struct Latest {
    input: Vec<Value>,
    args: Option<Args>,
    value: Value,
}

#[derive(PartialEq, Eq)]
struct Args {
    count: usize,
    revision: usize,
    batch: usize,
    echo: exact_plan::Str,
    offset: usize,
    full: bool,
}

impl Args {
    fn parse(values: &[Value]) -> Result<Self, DataError> {
        let [count, revision, batch, echo @ exact_plan::str_value!(), offset, Value::Bool(full)] =
            values
        else {
            return Err(DataError::BadArguments(
                "history(count, revision, batch, echo, offset, eager)".into(),
            ));
        };
        let controls = Controls::new(integer(count)?, integer(revision)?, integer(batch)?)
            .map_err(bad_arguments)?;
        let offset = integer(offset)?;
        // Same validation and ordering as model::page, before cache lookup/write.
        if echo.text().chars().take(MAX_DRAFT_CHARS + 1).count() > MAX_DRAFT_CHARS {
            return Err(bad_arguments(
                "local echo is limited to 512 Unicode scalar values",
            ));
        }
        if offset >= controls.count || !offset.is_multiple_of(PAGE_SIZE) {
            return Err(bad_arguments(
                "page offset must be a multiple of 100 below the history size",
            ));
        }
        Ok(Self {
            count: controls.count,
            revision: controls.revision,
            batch: controls.batch,
            echo: echo.to_shared_str().unwrap_or_default(),
            offset,
            full: *full,
        })
    }

    fn range(&self) -> (usize, usize) {
        if self.full {
            (0, self.count)
        } else {
            (self.offset, (self.offset + PAGE_SIZE).min(self.count))
        }
    }

    fn controls(&self) -> Controls {
        Controls {
            count: self.count,
            revision: self.revision,
            batch: self.batch,
        }
    }
}

fn bad_arguments(message: &str) -> DataError {
    DataError::BadArguments(message.into())
}

fn record(value: &Value) -> &Items {
    let Value::Record(fields) = value else {
        unreachable!("only this source's canonical records enter its cache")
    };
    fields
}

fn encode_row(row: &Row) -> Value {
    Value::record(vec![
        Value::str(&row.id),
        Value::str(&row.sender),
        Value::str(&row.body),
        Value::Bool(row.outgoing),
        Value::str(&row.meta),
    ])
}

impl ReusableMessagesStress {
    fn changed_value(&mut self, args: &Args, original: &[Value]) -> Result<Value, DataError> {
        let Some(old) = self.latest.as_ref() else {
            return self.control.query("history", original);
        };
        let Some(old_args) = &old.args else {
            return self.control.query("history", original);
        };
        if old_args.count != args.count || old_args.range() != args.range() {
            return self.control.query("history", original);
        }

        let old_fields = record(&old.value);
        let Value::List(old_rows) = &old_fields[0] else {
            unreachable!("canonical rows")
        };
        let (start, end) = args.range();
        let base_count = end - start;
        let suffix_start = args.count - args.batch.max(old_args.batch);
        let bodies_change = (old_args.revision != args.revision || old_args.batch != args.batch)
            && (old_args.revision > 0 || args.revision > 0)
            && end > suffix_start;
        let new_echo = !args.echo.is_empty() && args.echo != old_args.echo;

        // Reuse the canonical generator instead of duplicating body formatting.
        // At most one 100-row tail page (+ echo) is temporary scratch. This is
        // bounded payload generation, not a claim to allocate only batch rows.
        let tail_start = args.count - PAGE_SIZE;
        let generated = if bodies_change || new_echo {
            page(args.controls(), &args.echo, tail_start, false).map_err(bad_arguments)?
        } else {
            Vec::new()
        };
        let mut rows = Vec::with_capacity(base_count + usize::from(!args.echo.is_empty()));
        rows.extend(old_rows[..base_count].iter().cloned());
        let mut body_bytes = old_fields[4].as_number().expect("canonical byte count") as usize;
        if bodies_change {
            for index in start.max(suffix_start)..end {
                let position = index - start;
                let body = &generated[index - tail_start].body;
                let old_row = record(&rows[position]);
                let old_body = old_row[2].as_str().expect("canonical body");
                if old_body != body {
                    body_bytes = body_bytes - old_body.len() + body.len();
                    let mut fields = old_row.to_vec();
                    fields[2] = Value::str(body);
                    rows[position] = Value::record(fields);
                }
            }
        }
        body_bytes = body_bytes - old_args.echo.len() + args.echo.len();
        if !args.echo.is_empty() {
            rows.push(if new_echo {
                encode_row(generated.last().expect("canonical echo follows the page"))
            } else {
                old_rows[base_count].clone()
            });
        }
        let changed = if args.revision == 0 {
            0
        } else {
            end.saturating_sub(start.max(args.count - args.batch))
        };
        let materialized = rows.len();
        Ok(Value::record(vec![
            Value::list(rows),
            Value::Number(materialized as f64),
            Value::Number(args.revision as f64),
            Value::Number(changed as f64),
            Value::Number(body_bytes as f64),
            old_fields[5].clone(),
            old_fields[6].clone(),
            old_fields[7].clone(),
            old_fields[8].clone(),
        ]))
    }
}

impl DataSource for ReusableMessagesStress {
    fn app_id(&self) -> &str {
        self.control.app_id()
    }

    fn query(&mut self, source: &str, values: &[Value]) -> Result<Value, DataError> {
        if source != "history" {
            return Err(DataError::UnknownSource(source.into()));
        }
        // Keep the explicit manual/full controls on the existing reuse path.
        // Cursor validation and bounded generation remain canonical, including
        // their deliberately ignored page offset/eager controls.
        let args = match values {
            [_, _, _, _, _, _] => Some(Args::parse(values)?),
            [_, _, _, _, _, _, Value::Option(None)] => Some(Args::parse(&values[..6])?),
            _ => None,
        };
        if let Some(old) = &self.latest {
            let same_page = matches!((&old.args, &args), (Some(old), Some(new)) if old == new);
            if same_page || old.input.as_slice() == values {
                return Ok(old.value.clone());
            }
        }
        let value = if let Some(args) = &args {
            self.changed_value(args, values)?
        } else {
            self.control.query(source, values)?
        };
        // All validation/construction precedes publication. Old accepted Rc
        // owners remain immutable and outlive either this cache or the source.
        self.latest = Some(Latest {
            input: values.to_vec(),
            args,
            value: value.clone(),
        });
        Ok(value)
    }
}
