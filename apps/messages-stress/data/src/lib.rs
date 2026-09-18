//! Messages stress: synthetic data behind the same seam as the shipped apps.
#![forbid(unsafe_code)]

pub mod model;

use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use model::{page, window, Controls};

/// Pure, stateless, binary-bound source. It has no grants or external dependencies.
#[derive(Default)]
pub struct MessagesStress;

fn integer(value: &Value) -> Result<usize, DataError> {
    match value.as_number() {
        Some(n) if n.is_finite() && (0.0..=100_000.0).contains(&n) && n.fract() == 0.0 => {
            Ok(n as usize)
        }
        _ => Err(DataError::BadArguments(
            "expected a finite integer in 0..100000".into(),
        )),
    }
}

impl DataSource for MessagesStress {
    fn app_id(&self) -> &str {
        "com.exact.messages-stress"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source != "history" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let [count, revision, batch, Value::Str(echo), offset, Value::Bool(eager), selection @ ..] =
            args
        else {
            return Err(DataError::BadArguments(
                "history(count, revision, batch, echo, offset, eager[, cursor: option<string>])"
                    .into(),
            ));
        };
        let controls = Controls::new(integer(count)?, integer(revision)?, integer(batch)?)
            .map_err(|e| DataError::BadArguments(e.into()))?;
        let offset = integer(offset)?;
        // An omitted/none cursor keeps the original manual/eager source controls.
        // Some("") explicitly selects the bounded tail window.
        let cursor = match selection {
            [] | [Value::Option(None)] => None,
            [Value::Option(Some(value))] if value.as_str().is_some() => value.as_str(),
            _ => {
                return Err(DataError::BadArguments(
                    "expected an optional string cursor".into(),
                ))
            }
        };
        let (rows, earlier, later, has_earlier, has_later) = if let Some(cursor) = cursor {
            let answer =
                window(controls, echo, cursor).map_err(|e| DataError::BadArguments(e.into()))?;
            (
                answer.rows,
                answer.earlier,
                answer.later,
                answer.has_earlier,
                answer.has_later,
            )
        } else {
            (
                page(controls, echo, offset, *eager)
                    .map_err(|e| DataError::BadArguments(e.into()))?,
                String::new(),
                String::new(),
                false,
                false,
            )
        };
        let row_count = rows.len();
        let body_bytes: usize = rows.iter().map(|r| r.body.len()).sum();
        let changed = if controls.revision == 0 {
            0
        } else {
            rows.iter()
                .filter(|r| r.id != "local-echo" && r.body.contains("Synthetic stream revision"))
                .count()
        };
        Ok(Value::record(vec![
            Value::list(
                rows.into_iter()
                    .map(|r| {
                        Value::record(vec![
                            Value::str(&r.id),
                            Value::str(&r.sender),
                            Value::str(&r.body),
                            Value::Bool(r.outgoing),
                            Value::str(&r.meta),
                        ])
                    })
                    .collect(),
            ),
            Value::Number(row_count as f64),
            Value::Number(controls.revision as f64),
            Value::Number(changed as f64),
            Value::Number(body_bytes as f64),
            Value::str(&earlier),
            Value::str(&later),
            Value::Bool(has_earlier),
            Value::Bool(has_later),
        ]))
    }
}
