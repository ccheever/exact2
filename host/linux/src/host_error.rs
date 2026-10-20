//! Why the host refused, and the dev loop's settle window.
use exact_runner::RunnerError;

/// Why the host refused to boot.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum HostError {
    Plan(exact_plan::PlanError),
    Runner(RunnerError),
    Painter(String),
    Layout(String),
    Asset(String),
    PreparingModule,
    /// A module replacement refused while requests are in flight; a dev
    /// generation asks again, and after [`LET_GO_AFTER`] lets them go
    /// (`reload_module`'s `let_go`), as the web's dev loop does (LLP 1007 §6).
    RequestsPending,
}

/// How long a dev generation waits for requests in flight.
pub const LET_GO_AFTER: std::time::Duration = std::time::Duration::from_secs(1);

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
