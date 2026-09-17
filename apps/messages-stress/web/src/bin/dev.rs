//! The shared resident Contract compiler over the synthetic data source.
fn main() -> std::process::ExitCode {
    exact_web::dev::main::<messages_stress_data::MessagesStress>()
}
