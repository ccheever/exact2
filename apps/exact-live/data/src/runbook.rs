//! One retained curated document plus the existing latest-only stress source.
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Store};

#[cfg(target_arch = "wasm32")]
type Source = markdown_stress_data::MarkdownStress;
#[cfg(not(target_arch = "wasm32"))]
type Source = markdown_stress_data::NativeMarkdownStress;

pub struct Runbook {
    curated: Value,
    stress: Source,
}

impl Default for Runbook {
    fn default() -> Self {
        let source = include_str!("../../runbook.md");
        let parsed = markdown_parse::parse(source, &str::to_owned);
        Self {
            curated: Value::record(vec![
                Value::str("Outside, together"),
                Value::Number(source.len() as f64),
                markdown_parse::value::blocks(&parsed),
            ]),
            stress: Source::default(),
        }
    }
}

fn args(values: &[Value]) -> Result<Option<Vec<Value>>, DataError> {
    let [Value::Number(bytes)] = values else {
        return Err(DataError::BadArguments("runbook(bytes)".into()));
    };
    match *bytes {
        0. => Ok(None),
        1_048_576. | 4_194_304. => Ok(Some(vec![
            Value::str("paragraph"),
            Value::Number(*bytes),
            Value::Number(0.),
            Value::Number(0.),
            Value::Bool(true),
        ])),
        _ => Err(DataError::BadArguments(
            "runbook bytes must be 0, 1048576 or 4194304".into(),
        )),
    }
}

fn project(value: Value) -> Value {
    let Value::Record(fields) = value else {
        unreachable!("existing Document record")
    };
    Value::record(vec![
        fields[0].clone(),
        fields[1].clone(),
        fields[9].clone(),
    ])
}
fn projected(answer: Answer) -> Answer {
    match answer {
        Answer::Now(value) => Answer::Now(project(value)),
        Answer::Later(request) => Answer::Later(request),
    }
}

impl Runbook {
    fn curated(&mut self) -> Value {
        // Drop the one old native source/cell; its running work stays owned by
        // the ordered executor until it actually exits. No UI join or history.
        self.stress = Source::default();
        self.curated.clone()
    }
    pub fn query(&mut self, values: &[Value]) -> Result<Value, DataError> {
        match args(values)? {
            None => Ok(self.curated()),
            Some(args) => self.stress.query("document", &args).map(project),
        }
    }
    pub fn answer(&mut self, store: &mut Store, values: &[Value]) -> Result<Answer, DataError> {
        match args(values)? {
            None => Ok(Answer::Now(self.curated())),
            Some(args) => self.stress.answer(store, "document", &args).map(projected),
        }
    }
    pub fn parse(
        &mut self,
        store: &mut Store,
        values: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Some(args) = args(values)? else {
            return Err(DataError::BadArguments(
                "curated runbook has no worker completion".into(),
            ));
        };
        self.stress
            .parse(store, "document", &args, outcome)
            .map(projected)
    }
    pub fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        self.stress.continuation(token)
    }
}
