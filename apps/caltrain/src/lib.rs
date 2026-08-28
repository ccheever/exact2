//! The Caltrain app as a host sees it: compile, bake, boot.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use exact_kernel::Kernel;
use exact_plan::Plan;
use exact_runner::{Runner, RunnerError};

/// The app's Contract source.
pub const SOURCE: &str = include_str!("../app.contract");

/// Compile the app.
pub fn compile() -> Result<Plan, contract::CompileError> {
    contract::compile(SOURCE)
}

/// Compile and bake: every resource's boot value becomes compiled data.
pub fn build() -> Result<Plan, Box<dyn std::error::Error>> {
    let plan = compile()?;
    contract::bake(plan, caltrain_data::Caltrain).map_err(|e| format!("{e:?}").into())
}

/// Boot a runner for `plan` against the app's data source and a kernel.
pub fn boot(plan: Plan, kernel: Kernel) -> Result<Runner<caltrain_data::Caltrain>, RunnerError> {
    Runner::boot(plan, caltrain_data::Caltrain, kernel)
}
