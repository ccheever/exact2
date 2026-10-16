//! How the runner's callees' errors become its own (moved from runner.rs for its cap).

use super::RunnerError;
use crate::instance::InstanceError;
use crate::vm::Trap;
use exact_kernel::KernelError;

impl From<Trap> for RunnerError {
    fn from(t: Trap) -> Self {
        RunnerError::Trap(t)
    }
}

impl From<InstanceError> for RunnerError {
    fn from(e: InstanceError) -> Self {
        match e {
            InstanceError::SlotType { slot } => RunnerError::SlotType { slot },
            other => RunnerError::Instance(other),
        }
    }
}

impl From<KernelError> for RunnerError {
    fn from(e: KernelError) -> Self {
        RunnerError::Kernel(e)
    }
}
