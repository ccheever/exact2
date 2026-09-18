use exact_plan::Value;
use exact_runner::DataError;
use std::rc::Rc;

const NOTES: [(&str, &str, &str); 8] = [
    ("Maya · Director", "North shore opens the film. Hold on the water before the title arrives.", "09:41"),
    ("Jules · Camera", "Six selects are in the rundown. Coast, dunes, lake, ridge, glasshouse, last light.", "09:42"),
    ("Ari · Sound", "I have a clean wind bed for the first scene. Keep the cut quiet; the room should hear the coast.", "09:43"),
    ("Noor · Producer", "Runbook is updated: review the framing, agree the order, then hand off the six selects.", "09:44"),
    ("Maya · Director", "Try the glasshouse before the ridge. The greens give the snow room to breathe.", "09:45"),
    ("You", "I'll compare both sequences and keep the original available.", "09:46"),
    ("Jules · Camera", "Open any scene to check the crop. Fit shows the whole frame; 2× lets you study a detail.", "09:47"),
    ("Noor · Producer", "Ready when you are. Leave a note here while we work through the selects.", "09:48"),
];

pub struct Crew {
    rows: Vec<Value>,
    body_bytes: usize,
    latest: Option<(Rc<str>, Value)>,
}

impl Default for Crew {
    fn default() -> Self {
        Self {
            rows: NOTES
                .iter()
                .enumerate()
                .map(|(i, (sender, text, at))| {
                    row(&format!("crew-{i}"), sender, text, *sender == "You", at)
                })
                .collect(),
            body_bytes: NOTES.iter().map(|(_, text, _)| text.len()).sum(),
            latest: None,
        }
    }
}

fn row(id: &str, sender: &str, text: &str, outgoing: bool, at: &str) -> Value {
    Value::record(vec![
        Value::str(id),
        Value::str(sender),
        Value::str(text),
        Value::Bool(outgoing),
        Value::str(at),
    ])
}

impl Crew {
    pub fn query(&mut self, args: &[Value]) -> Result<Value, DataError> {
        let [Value::Number(count), Value::Number(revision), Value::Number(batch), Value::Str(echo), Value::Number(offset), Value::Bool(full)] =
            args
        else {
            return Err(DataError::BadArguments(
                "curated history uses the existing six-argument history shape".into(),
            ));
        };
        if *count != 0.
            || *revision != 0.
            || ![1., 8., 32.].contains(batch)
            || *offset != 0.
            || !full
            || echo.chars().take(513).count() > 512
        {
            return Err(DataError::BadArguments(
                "curated history: revision0, full, offset0 and at most512 characters".into(),
            ));
        }
        if let Some((key, value)) = &self.latest {
            if key == echo {
                return Ok(value.clone());
            }
        }
        let mut rows = self.rows.clone();
        if !echo.is_empty() {
            rows.push(row("local-echo", "You", echo, true, "Device-local note"));
        }
        let count = rows.len();
        let value = Value::record(vec![
            Value::list(rows),
            Value::Number(count as f64),
            Value::Number(0.),
            Value::Number(0.),
            Value::Number((self.body_bytes + echo.len()) as f64),
            Value::str(""),
            Value::str(""),
            Value::Bool(false),
            Value::Bool(false),
        ]);
        self.latest = Some((echo.clone(), value.clone()));
        Ok(value)
    }
}
