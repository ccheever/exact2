//! Resident Contract compiler for apps with no external data sources.
use exact_runner::{DataError, DataSource, Value};
#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}
fn main() -> std::process::ExitCode {
    exact_web::dev::main::<NoData>()
}
