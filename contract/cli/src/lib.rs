//! The compiler as one call.
//!
//! @ref LLP 1004 D2 (the driver) / D4 (constant resources are compiled data)
//! / D6 (the corpus)
//!
//! `compile` runs the four passes and returns a validated plan whose bytes
//! are a pure function of the source. `bake` then boots the runner once
//! against the app's data source and writes every resource's boot value into
//! the plan, so the first frame on a device needs no host and no seam. The
//! compiler is, for one frame, a host.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use exact_kernel::Kernel;
use exact_plan::builder::PlanBuilder;
use exact_plan::{Plan, ResourcesId};
use exact_runner::{DataSource, Runner, RunnerError};

/// Any rejection from any pass, with its stable id and span.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    /// Which pass.
    pub pass: &'static str,
    /// Stable id.
    pub id: String,
    /// What went wrong.
    pub message: String,
    /// Line, column.
    pub span: (u32, u32),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{} [{}] {}",
            self.span.0, self.span.1, self.id, self.message
        )
    }
}

impl std::error::Error for CompileError {}

macro_rules! from_pass {
    ($ty:path, $pass:expr) => {
        impl From<$ty> for CompileError {
            fn from(e: $ty) -> Self {
                CompileError {
                    pass: $pass,
                    id: e.id.to_string(),
                    message: e.message,
                    span: (e.span.line, e.span.col),
                }
            }
        }
    };
}

from_pass!(contract_syntax::SyntaxError, "syntax");
from_pass!(contract_types::TypeError, "types");
from_pass!(contract_analyze::AnalyzeError, "analyze");
from_pass!(contract_lower::LowerError, "lower");

/// Compile one source text to a validated plan.
pub fn compile(src: &str) -> Result<Plan, CompileError> {
    let file = contract_syntax::parse(src)?;
    let types = contract_types::check(&file)?;
    let analysis = contract_analyze::check(&file, &types)?;
    Ok(contract_lower::lower(&file, &types, &analysis)?)
}

/// Boot the plan once against `data` and write every resource's boot value
/// into the plan as compiled data. The result still validates and its bytes
/// are a pure function of (source, data).
pub fn bake<D: DataSource>(plan: Plan, data: D) -> Result<Plan, RunnerError> {
    let runner = Runner::boot(plan.clone(), data, Kernel::with_monospace())?;
    let mut b = PlanBuilder::from_plan(plan);
    for i in 0..runner.plan().resources.len() {
        let name = runner
            .plan()
            .str(runner.plan().resources[i].name)
            .to_string();
        if let Some(v) = runner.resource(&name) {
            b.set_resource_initial(ResourcesId(i as u32), v);
        }
    }
    b.finish().map_err(RunnerError::Plan)
}
