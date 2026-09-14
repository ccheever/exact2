//! The Rust experiment: compiled into wasm on web and machine code on Apple.
use exact_plan::Value;
use exact_runner::DataError;

// Edit these two values for the Rust update experiment.
const VERSION: &str = "Rust v9";
const MULTIPLIER: f64 = 2.0;

pub fn probe(args: &[Value]) -> Result<Value, DataError> {
    let [Value::Number(input)] = args else {
        return Err(DataError::BadArguments(
            "rustProbe expects one number".into(),
        ));
    };
    Ok(Value::record(vec![
        Value::str(VERSION),
        Value::Number(*input),
        Value::Number(input * MULTIPLIER),
    ]))
}
