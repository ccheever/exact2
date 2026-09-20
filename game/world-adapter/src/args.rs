//! Contract values enter typed Args through the existing Data reader.
use exact_plan::Value;
use exact_world::{args::ArgumentRef, bin, Args, DataError, Game, Number, Sim, Writer};
const SAFE: f64 = 9_007_199_254_740_991.;

pub fn decode_args<A: Args>(values: &[Value]) -> Result<A, DataError> {
    if values.len() > A::FIELDS.len() {
        return Err(DataError::new(format!(
            "expected at most {} arguments, got {}",
            A::FIELDS.len(),
            values.len()
        )));
    }
    let mut args = A::default();
    if values.is_empty() {
        args.check_scalars().map_err(DataError::new)?;
        return Ok(args);
    }
    let mut out = bin::Encoder::default();
    out.begin_struct();
    for ((name, _), value) in A::FIELDS.iter().zip(values) {
        out.field(name);
        match (args.argument(name), value) {
            (Some(ArgumentRef::Bool(_)), Value::Bool(v)) => out.boolean(*v),
            (Some(ArgumentRef::Text(_)), Value::Str(value)) => out.string(value),
            (
                Some(
                    kind @ (ArgumentRef::Unsigned(_)
                    | ArgumentRef::Signed(_)
                    | ArgumentRef::Float(_)),
                ),
                Value::Number(v),
            ) => {
                if !v.is_finite() {
                    return Err(DataError::new("expected finite number").at(name));
                }
                match kind {
                    ArgumentRef::Unsigned(_) if *v >= 0. && *v <= SAFE && v.fract() == 0. => {
                        out.number(Number::Unsigned(*v as u64))
                    }
                    ArgumentRef::Signed(_) if v.abs() <= SAFE && v.fract() == 0. => {
                        out.number(Number::Signed(*v as i64))
                    }
                    ArgumentRef::Float(_) => out.number(Number::F64(*v)),
                    _ => return Err(DataError::new("integer outside Contract safe range").at(name)),
                }
            }
            _ => return Err(DataError::new("argument type differs").at(name)),
        }
    }
    out.end_struct();
    bin::read_into(&out.finish()?, &mut args)?;
    args.check_scalars().map_err(DataError::new)?;
    Ok(args)
}

pub fn argument_values<A: Args>(args: &A) -> Result<Vec<Value>, DataError> {
    args.check_scalars().map_err(DataError::new)?;
    A::FIELDS
        .iter()
        .map(|(name, _)| {
            Ok(match args.argument(name) {
                Some(ArgumentRef::Bool(v)) => Value::Bool(v),
                Some(ArgumentRef::Text(v)) => Value::str(v),
                Some(ArgumentRef::Float(v)) if v.is_finite() => Value::Number(v),
                Some(ArgumentRef::Unsigned(v)) if v <= SAFE as u64 => Value::Number(v as f64),
                Some(ArgumentRef::Signed(v)) if v.unsigned_abs() <= SAFE as u64 => {
                    Value::Number(v as f64)
                }
                _ => return Err(DataError::new("argument outside Contract range").at(name)),
            })
        })
        .collect()
}

pub fn from_values<G: Game>(values: &[Value]) -> Result<Sim<G>, DataError> {
    Sim::new(decode_args(values)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default, exact_world::Args)]
    struct Options {
        count: u32,
        #[live]
        value: f32,
        label: String,
        large: u64,
    }
    #[test]
    fn data_driven_argument_conversion_refuses_invalid_values() {
        let values = [
            Value::Number(7.),
            Value::Number(-0.),
            Value::str("hello"),
            Value::Number(SAFE),
        ];
        let args = decode_args::<Options>(&values).unwrap();
        assert_eq!(args.count, 7);
        assert_eq!(args.value.to_bits(), (-0f32).to_bits());
        assert_eq!(argument_values(&args).unwrap(), values);
        assert!(decode_args::<Options>(&[Value::Number(4_294_967_296.)]).is_err());
        assert!(decode_args::<Options>(&[Value::Number(-1.)]).is_err());
        assert!(decode_args::<Options>(&[Value::Number(1.5)]).is_err());
        assert!(decode_args::<Options>(&[Value::Number(0.), Value::Number(f64::MAX)]).is_err());
        assert!(decode_args::<Options>(&[Value::Bool(true)]).is_err());
        assert!(decode_args::<Options>(&[const { Value::Number(1.) }; 5]).is_err());
        assert_eq!(decode_args::<Options>(&[]).unwrap().count, 0);
        assert!(argument_values(&Options {
            large: u64::MAX,
            ..Default::default()
        })
        .is_err());
        assert!(decode_args::<()>(&[Value::Bool(true)]).is_err());
    }
}
