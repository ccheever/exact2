//! Update Lab sources, composed through the shared language-independent data seam.
//! @ref LLP 1027 D8 / LLP 1029.000 — each language owns one fixed source.
mod probe;
#[cfg(test)]
mod tests;

pub use exact_data::Mixed as Lab;
use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The independently compiled Rust experiment. Its implementation stays in probe.rs.
#[derive(Default)]
pub struct Probe;

impl DataSource for Probe {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "rustProbe" => probe::probe(args),
            "rustExecutor" => Ok(Value::record(vec![Value::str(
                if cfg!(target_arch = "wasm32") {
                    "Wasm"
                } else {
                    "Native"
                },
            )])),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn app_id(&self) -> &str {
        "com.exact.updatelab"
    }
}

/// The app declares ownership without executing either source to discover it.
/// The host explicitly retains its embedded zero-state probe when Rust updates
/// are disabled; the shared composer never assumes `Default` preserves state.
pub fn compose<J: DataSource, R: DataSource>(
    javascript: J,
    rust: R,
    rust_updates: bool,
    retain_rust: fn(&R) -> Result<R, DataError>,
) -> Lab<J, R> {
    let mixed = Lab::new(
        javascript,
        rust,
        &["typescriptProbe"],
        &["rustProbe", "rustExecutor"],
    )
    .expect("Update Lab executors share an identity and distinct sources");
    if rust_updates {
        mixed
    } else {
        mixed.with_embedded_rust(retain_rust)
    }
}
