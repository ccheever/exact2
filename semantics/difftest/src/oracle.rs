//! The data seam both sides share: a source that answers any call with a
//! value of the declared shape, a pure function of (seed, source, arguments),
//! and keeps a transcript. The Lean side is handed the transcript as its
//! `Contract.Oracle`, so a call the semantics makes that the runner did not
//! is a divergence, not a guess.

use crate::observe::value as canonical;
use crate::rng::Rng;
use exact_plan::{Plan, TypeKind, TypesId, Value};
use exact_runner::{DataError, DataSource};
use std::cell::RefCell;
use std::rc::Rc;

/// One call the runner made, and its answer.
#[derive(Debug, Clone)]
pub struct Call {
    /// The source.
    pub source: String,
    /// The arguments, with their declared types.
    pub args: Vec<(TypesId, Value)>,
    /// The answer, with its declared type.
    pub answer: (TypesId, Value),
}

/// The data source.
pub struct Oracle {
    seed: u64,
    /// source → (parameter types, result type), from the plan's `sources`.
    signatures: Vec<(String, Vec<TypesId>, TypesId)>,
    plan: Plan,
    /// Every distinct call, in the order first made; shared by every
    /// handle, so it survives a boot that consumes the source and fails.
    pub transcript: Rc<RefCell<Vec<Call>>>,
    /// By resource, what each whose source the runner answers itself held
    /// at boot.
    pub facts: Rc<RefCell<Vec<(String, TypesId, Value)>>>,
    /// The device's store at launch (LLP 1116 D5): a persisted state's kept
    /// value, by the state's name. Empty, a fresh install's, unless
    /// [`Oracle::with_store`].
    pub stored: Rc<Vec<(String, TypesId, Value)>>,
}

/// The sources the runner answers itself, from the host's facts.
pub const HOST_SOURCES: &[&str] = &[
    exact_runner::delivery::SOURCE,
    exact_runner::page::SOURCE,
    exact_runner::time::SOURCE,
    exact_runner::surface_record::SOURCE,
    exact_runner::viewport::SOURCE,
];

impl Oracle {
    /// A source for `plan`'s signatures, answering from `seed`.
    pub fn new(plan: &Plan, seed: u64) -> Self {
        let signatures = plan
            .sources
            .iter()
            .map(|s| {
                (
                    plan.str(s.name).to_string(),
                    s.params.iter().map(|p| plan.source_param(p).ty).collect(),
                    s.ty,
                )
            })
            .collect();
        Self {
            seed,
            signatures,
            plan: plan.clone(),
            transcript: Rc::default(),
            facts: Rc::default(),
            stored: Rc::default(),
        }
    }

    /// The source a case runs against: the empty store an authored test
    /// starts from, or the device store [`Oracle::with_store`] fills.
    pub fn for_case(plan: &Plan, case: &crate::script::Case) -> Self {
        let oracle = Oracle::new(plan, crate::seed_of(&case.name));
        if case.stored {
            oracle.with_store()
        } else {
            oracle
        }
    }

    /// This source with a device store at launch: for each persisted state,
    /// a pure function of the seed and its name, nothing kept, a value of
    /// its type, or one of another (LLP 1116 D5).
    pub fn with_store(mut self) -> Self {
        let mut stored = Vec::new();
        for row in self.plan.slots.iter().filter(|s| s.persist) {
            let name = self.plan.str(row.name).to_string();
            let mut rng = Rng::new(hash(self.seed, &format!("stored {name}")));
            let v = match rng.below(3) {
                0 => continue,
                1 => generate(&self.plan, row.ty, &mut rng, 0),
                _ => mismatch(&self.plan, row.ty),
            };
            stored.push((name, row.ty, v));
        }
        self.stored = Rc::new(stored);
        self
    }

    /// The device store as the host hands it to the runner: each value as
    /// the runner's persisted text, under its runner-owned name.
    pub fn snapshot(&self) -> Vec<(String, String)> {
        self.stored
            .iter()
            .map(|(name, _, v)| {
                let mut text = String::new();
                exact_runner::runner::persist::encode(v, &mut text);
                (format!("{}{name}", exact_runner::Store::STATE), text)
            })
            .collect()
    }

    /// Another handle on the same transcript and facts.
    pub fn handle(&self) -> Self {
        Self {
            seed: self.seed,
            signatures: self.signatures.clone(),
            plan: self.plan.clone(),
            transcript: self.transcript.clone(),
            facts: self.facts.clone(),
            stored: self.stored.clone(),
        }
    }

    /// The transcript in `Contract.OracleText`'s format.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for c in self.transcript.borrow().iter() {
            out.push_str("answer ");
            out.push_str(&crate::observe::quote(&c.source));
            out.push_str(&format!(" {}", c.args.len()));
            for (t, v) in &c.args {
                out.push(' ');
                out.push_str(&text_value(&self.plan, *t, v));
            }
            out.push(' ');
            out.push_str(&text_value(&self.plan, c.answer.0, &c.answer.1));
            out.push('\n');
        }
        for (name, t, v) in self.facts.borrow().iter() {
            out.push_str("fact ");
            out.push_str(&crate::observe::quote(name));
            out.push(' ');
            out.push_str(&text_value(&self.plan, *t, v));
            out.push('\n');
        }
        for (name, t, v) in self.stored.iter() {
            out.push_str("stored ");
            out.push_str(&crate::observe::quote(name));
            out.push(' ');
            out.push_str(&text_value(&self.plan, *t, v));
            out.push('\n');
        }
        out
    }
}

/// A value of another kind than persisted type `ty`'s scalar, whose text
/// the runner's decoder refuses for `ty` as the semantics' `conforms` does:
/// never a string a number decodes from (`"NaN"`, `"Infinity"`).
fn mismatch(plan: &Plan, ty: TypesId) -> Value {
    let row = plan.type_(ty);
    let scalar = match row.kind {
        TypeKind::Option | TypeKind::List => row.elem.map_or(row.kind, |e| plan.type_(e).kind),
        k => k,
    };
    match scalar {
        TypeKind::String => Value::Number(7.0),
        _ => Value::str("x"),
    }
}

fn hash(seed: u64, text: &str) -> u64 {
    // FNV-1a over the text, then mixed with the seed.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Numbers a source answers with: the edges of the double format and of
/// JavaScript's printing, mixed with ordinary ones.
const NUMBERS: &[f64] = &[
    0.0,
    -0.0,
    1.0,
    -1.0,
    2.0,
    3.0,
    7.0,
    10.0,
    42.0,
    100.0,
    0.1,
    0.5,
    1.5,
    -2.5,
    1e21,
    1e-7,
    123.456,
    9007199254740993.0,
    1e300,
    5e-324,
];

const STRINGS: &[&str] = &[
    "", "a", "b", "ab", "Hello", " padded ", "x y", "ü", "日本", "😀", "a,b", "\"q\"", "0", "-",
];

fn generate(plan: &Plan, ty: TypesId, rng: &mut Rng, depth: u32) -> Value {
    let row = plan.type_(ty);
    match row.kind {
        TypeKind::Number => {
            if rng.chance(1, 2) {
                Value::Number(rng.below(20) as f64)
            } else {
                Value::Number(*rng.pick(NUMBERS))
            }
        }
        TypeKind::Bool => Value::Bool(rng.chance(1, 2)),
        TypeKind::String => Value::str(rng.pick(STRINGS)),
        TypeKind::Unit => Value::Unit,
        TypeKind::Option => match row.elem {
            Some(e) if depth < 4 && rng.chance(2, 3) => {
                Value::some(generate(plan, e, rng, depth + 1))
            }
            _ => Value::Option(None),
        },
        TypeKind::List => {
            let n = if depth < 4 { rng.below(5) } else { 0 };
            let items = match row.elem {
                Some(e) => (0..n).map(|_| generate(plan, e, rng, depth + 1)).collect(),
                None => Vec::new(),
            };
            Value::list(items)
        }
        TypeKind::Record => Value::record(
            row.fields
                .iter()
                .map(|f| generate(plan, plan.field(f).ty, rng, depth + 1))
                .collect(),
        ),
    }
}

impl DataSource for Oracle {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let Some((_, params, result)) = self.signatures.iter().find(|(n, _, _)| n == source) else {
            return Err(DataError::UnknownSource(source.into()));
        };
        let (params, result) = (params.clone(), *result);
        if let Some(c) = self.transcript.borrow().iter().find(|c| {
            c.source == source
                && c.args.len() == args.len()
                && c.args
                    .iter()
                    .zip(args)
                    .all(|((_, a), b)| canonical(a) == canonical(b))
        }) {
            return Ok(c.answer.1.clone());
        }
        let key: Vec<String> = args.iter().map(canonical).collect();
        let mut rng = Rng::new(hash(self.seed, &format!("{source}({})", key.join(","))));
        let answer = generate(&self.plan, result, &mut rng, 0);
        self.transcript.borrow_mut().push(Call {
            source: source.to_string(),
            args: params.iter().copied().zip(args.iter().cloned()).collect(),
            answer: (result, answer.clone()),
        });
        Ok(answer)
    }
}

/// A value in `Contract.OracleText`'s format, its records named by `ty`.
pub fn text_value(plan: &Plan, ty: TypesId, v: &Value) -> String {
    let row = plan.type_(ty);
    let elem = |t: Option<TypesId>| t.unwrap_or(ty);
    match v {
        Value::Number(n) => format!("n{:016x}", n.to_bits()),
        Value::Bool(b) => (if *b { "t" } else { "f" }).into(),
        v if v.is_str() => crate::observe::quote(v.as_str().unwrap_or_default()),
        Value::Unit => "u".into(),
        Value::Option(None) => "none".into(),
        Value::Option(Some(inner)) => format!("some({})", text_value(plan, elem(row.elem), inner)),
        Value::List(items) => {
            let parts: Vec<String> = items
                .iter()
                .map(|i| text_value(plan, elem(row.elem), i))
                .collect();
            format!("[{}]", parts.join(","))
        }
        Value::Record(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .zip(row.fields.iter())
                .map(|(f, id)| text_value(plan, plan.field(id).ty, f))
                .collect();
            format!(
                "R{}{{{}}}",
                crate::observe::quote(plan.str(row.name)),
                parts.join(",")
            )
        }
        _ => "u".into(),
    }
}
