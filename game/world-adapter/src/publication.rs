use exact_plan::Value;
use exact_world::{Data, DataError, Number, Published, World, Writer};
use std::rc::Rc;

// Admission precedes each allocation; no conversion may reserve an unbounded list.
fn admit(
    remaining: &mut usize,
    depth: usize,
    text: usize,
    children: usize,
) -> Result<(), DataError> {
    *remaining = remaining
        .checked_sub(64usize.saturating_add(text.saturating_mul(6)))
        .ok_or_else(|| DataError::new("Contract conversion budget"))?;
    if depth > 256 || children > *remaining / 64 {
        return Err(DataError::new("Contract conversion depth/size limit"));
    }
    Ok(())
}
/// Preserve positional record/list tags within 65,536 bytes/visits and depth 256.
pub fn from_contract(value: Value) -> Result<Published, DataError> {
    let mut remaining = exact_world::json::LIMIT;
    from_value(&value, &mut remaining, 0)
}
fn from_value(value: &Value, remaining: &mut usize, depth: usize) -> Result<Published, DataError> {
    let text = value.as_str().map_or(0, str::len);
    let children = match value {
        Value::List(v) | Value::Record(v) => v.len(),
        _ => 0,
    };
    admit(remaining, depth, text, children)?;
    Ok(match value {
        Value::Unit => Published::Unit,
        Value::Number(n) => Published::Number(*n),
        Value::Bool(b) => Published::Bool(*b),
        Value::Str(s) => Published::Str(s.to_string()),
        Value::Option(v) => Published::Option(
            v.as_ref()
                .map(|v| from_value(v, remaining, depth + 1).map(Box::new))
                .transpose()?,
        ),
        Value::List(v) => Published::List(
            v.iter()
                .map(|v| from_value(v, remaining, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Value::Record(v) => Published::Record(
            v.iter()
                .map(|v| from_value(v, remaining, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
    })
}
/// Named objects require the app's declared shape and refuse positional conversion.
pub fn to_contract(value: &Published) -> Result<Value, DataError> {
    let mut remaining = exact_world::json::LIMIT;
    to_value(value, &mut remaining, 0)
}
fn to_value(value: &Published, remaining: &mut usize, depth: usize) -> Result<Value, DataError> {
    let text = match value {
        Published::Str(s) => s.len(),
        _ => 0,
    };
    let children = match value {
        Published::List(v) | Published::Record(v) => v.len(),
        _ => 0,
    };
    admit(remaining, depth, text, children)?;
    Ok(match value {
        Published::Unit => Value::Unit,
        Published::Number(n) => Value::Number(*n),
        Published::Bool(b) => Value::Bool(*b),
        Published::Str(s) => Value::str(s),
        Published::Option(v) => Value::Option(
            v.as_ref()
                .map(|v| to_value(v, remaining, depth + 1).map(Rc::new))
                .transpose()?,
        ),
        Published::List(v) => Value::list(
            v.iter()
                .map(|v| to_value(v, remaining, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Published::Record(v) => Value::record(
            v.iter()
                .map(|v| to_value(v, remaining, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Published::Object(_) => {
            return Err(DataError::new("named record requires a Contract shape"))
        }
    })
}
// One Data traversal; field names remain names until the app's shape decoder.
#[derive(Default)]
struct RecordWriter {
    stack: Vec<Published>,
    fields: Vec<String>,
    result: Published,
    spent: usize,
}
impl RecordWriter {
    fn push(&mut self, value: Published) {
        self.claim_decoded(64);
        match self.stack.last_mut() {
            Some(Published::Object(fields)) => {
                fields.insert(self.fields.pop().expect("record field"), value);
            }
            Some(Published::List(items)) => items.push(value),
            Some(Published::Option(item)) => *item = Some(Box::new(value)),
            None => self.result = value,
            _ => unreachable!(),
        }
    }
    fn begin(&mut self, value: Published) {
        assert!(self.stack.len() < 256, "publication nesting limit");
        self.stack.push(value);
    }
    fn end(&mut self) {
        let value = self.stack.pop().expect("Data container");
        self.push(value);
    }
}
impl Writer for RecordWriter {
    fn claim_decoded(&mut self, bytes: usize) {
        self.spent = self.spent.saturating_add(bytes);
        assert!(
            self.spent <= exact_world::json::LIMIT,
            "publication traversal limit"
        );
    }
    fn unit(&mut self) {
        self.push(Published::Unit);
    }
    fn boolean(&mut self, v: bool) {
        self.push(Published::Bool(v));
    }
    fn number(&mut self, v: Number) {
        use Number::*;
        self.push(Published::Number(match v {
            Unsigned(n) => {
                assert!(
                    n <= 9_007_199_254_740_991,
                    "publish_record: integer outside Contract safe range"
                );
                n as f64
            }
            Signed(n) => {
                assert!(
                    (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n),
                    "publish_record: integer outside Contract safe range"
                );
                n as f64
            }
            F32(n) => n as f64,
            F64(n) => n,
        }));
    }
    fn string(&mut self, v: &str) {
        self.claim_decoded(v.len());
        self.push(Published::Str(v.into()));
    }
    fn bytes(&mut self, value: exact_world::data::Bulk<'_>) {
        self.claim_decoded(value.numbers().size_hint().0.saturating_mul(64));
        self.push(Published::List(
            value.numbers().map(Published::Number).collect(),
        ));
    }
    fn begin_seq(&mut self, len: usize) {
        self.claim_decoded(len.saturating_mul(64));
        self.begin(Published::List(Vec::with_capacity(len)));
    }
    fn item(&mut self) {}
    fn end_seq(&mut self) {
        self.end();
    }
    fn begin_struct(&mut self) {
        self.begin(Published::Object(Default::default()));
    }
    fn field(&mut self, name: &'static str) {
        self.key(name);
    }
    fn key(&mut self, name: &str) {
        self.claim_decoded(name.len());
        self.fields.push(name.into());
    }
    fn end_struct(&mut self) {
        self.end();
    }
    fn variant(&mut self, _: &'static str, _: u32) {
        panic!("publish_record expects ordinary records, not enum variants");
    }
    fn end_variant(&mut self) {}
    fn option(&mut self, _: bool) {
        self.begin(Published::Option(None));
    }
    fn end_option(&mut self) {
        assert!(
            !matches!(self.stack.last(), Some(Published::Option(Some(v))) if matches!(v.as_ref(), Published::Unit)),
            "publish_record: Some(()) is ambiguous with None in Contract JSON"
        );
        self.end();
    }
}
/// Publish named Data fields together. Records, lists, options and scalars use
/// Contract JSON; the app validates field types against its declared shape.
/// Typed numeric vectors become arrays; enum variants are not Contract values
/// and panic. The argument itself must be a named record.
pub fn publish_record(world: &World, record: &impl Data) {
    let mut writer = RecordWriter::default();
    record.write(&mut writer);
    let Published::Object(fields) = writer.result else {
        panic!("publish_record expects a named record");
    };
    world
        .publish_batch(fields)
        .expect("publication batch admission");
}
#[cfg(test)]
mod tests {
    use super::*;
    use exact_world::{bin, hash};

    #[test]
    fn contract_values_keep_engine_wire_and_hash_including_numeric_edges() {
        let shared = Rc::new(Value::record(vec![
            Value::str("quote \" slash \\ line\n héllo 🌕"),
            Value::Number(-0.0),
            Value::list(vec![Value::Unit, Value::Bool(true)]),
        ]));
        let mut cases = vec![
            Value::Unit,
            Value::Bool(false),
            Value::Bool(true),
            Value::str(""),
            Value::str("héllo 🌕"),
            Value::Option(None),
            Value::Option(Some(Rc::new(Value::Unit))),
            Value::Option(Some(shared.clone())),
            Value::list(vec![]),
            Value::record(vec![]),
            shared.as_ref().clone(),
            Value::list(vec![
                Value::Option(Some(shared.clone())),
                shared.as_ref().clone(),
            ]),
        ];
        cases.extend(
            [
                0.0,
                -0.0,
                1.0,
                -1.0,
                f64::MAX,
                f64::MIN_POSITIVE,
                f64::from_bits(1),
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NAN,
                f64::from_bits(0xfff8_0000_0000_0001),
            ]
            .into_iter()
            .map(Value::Number),
        );
        let owners = Rc::strong_count(&shared);
        for value in &cases {
            let stored = from_contract(value.clone()).unwrap();
            let expected = bin::to_vec(&stored).unwrap();
            assert_eq!(
                bin::to_vec(&from_contract(to_contract(&stored).unwrap()).unwrap()).unwrap(),
                expected
            );
            assert_eq!(expected, exact_game::bin::to_vec(value), "{value:?}");
            assert_eq!(
                hash::of(&stored).unwrap(),
                exact_game::hash::of(value),
                "{value:?}"
            );
            let decoded: Published = bin::from_slice(&expected).unwrap();
            assert_eq!(
                bin::to_vec(&decoded).unwrap(),
                expected,
                "round trip {value:?}"
            );
        }
        assert_eq!(Rc::strong_count(&shared), owners);
        // List and Record have equal public JSON shapes but distinct saved tags.
        assert_ne!(
            bin::to_vec(&Published::List(vec![])).unwrap(),
            bin::to_vec(&Published::Record(vec![])).unwrap()
        );
        assert_ne!(
            hash::of(&Published::Number(0.)),
            hash::of(&Published::Number(-0.))
        );
    }
}

#[cfg(test)]
mod record_tests {
    use super::*;
    #[test]
    fn named_objects_and_bounded_conversion() {
        #[derive(Default, Data)]
        struct Record {
            values: Vec<u32>,
            enabled: bool,
        }
        let w = World::new(60, 0);
        publish_record(
            &w,
            &Record {
                values: vec![7, 9],
                enabled: true,
            },
        );
        assert_eq!(json(&w).unwrap(), r#"{"enabled":true,"values":[7.0,9.0]}"#);
        assert_eq!(
            to_contract(w.publications().get("values").unwrap()).unwrap(),
            Value::list(vec![Value::Number(7.), Value::Number(9.)])
        );
        let before = w.save().unwrap();
        let record = Record {
            values: vec![0; 1_000_000],
            enabled: false,
        };
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| publish_record(&w, &record)))
                .is_err()
        );
        assert_eq!(w.save().unwrap(), before);
    }
}

pub(crate) fn inspect(value: &Published, w: &mut dyn Writer) {
    match value {
        Published::Object(fields) => {
            w.begin_struct();
            for (k, v) in fields {
                w.key(k);
                inspect(v, w);
            }
            w.end_struct();
        }
        Published::List(items) | Published::Record(items) => {
            w.begin_seq(items.len());
            for v in items {
                w.item();
                inspect(v, w);
            }
            w.end_seq();
        }
        Published::Option(Some(v)) => inspect(v, w),
        Published::Unit | Published::Option(None) => w.unit(),
        Published::Str(s) => w.string(s),
        Published::Number(n) => w.number(Number::F64(*n)),
        Published::Bool(b) => w.boolean(*b),
    }
}
/// Strip saved variant tags for the Contract message envelope.
pub fn json(world: &World) -> Result<String, exact_world::DataError> {
    let mut out = exact_world::json::Encoder::default();
    out.begin_struct();
    for (key, value) in world.publications().iter() {
        out.key(key);
        inspect(value, &mut out);
    }
    out.end_struct();
    out.finish()
}

#[cfg(test)]
mod conversion_bounds {
    use super::*;
    #[test]
    fn oversized_lists_strings_and_nesting_refuse_with_positive_controls() {
        assert!(from_contract(Value::list(vec![Value::Unit; 65536])).is_err());
        assert!(to_contract(&Published::List(vec![Published::Unit; 65536])).is_err());
        assert!(from_contract(Value::str(&"a".repeat(65536))).is_err());
        assert!(to_contract(&Published::Object(Default::default())).is_err());
        let mut nested = Value::Unit;
        for _ in 0..258 {
            nested = Value::Option(Some(Rc::new(nested)));
        }
        assert!(from_contract(nested).is_err());
        let small = Published::Record(vec![Published::Bool(true), Published::Number(7.)]);
        assert_eq!(from_contract(to_contract(&small).unwrap()).unwrap(), small);
    }
}
