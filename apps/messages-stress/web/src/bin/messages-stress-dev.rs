//! The shared resident Contract compiler over the synthetic data source.
include!(concat!(env!("OUT_DIR"), "/factory.rs"));

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<AppData>()
}
