//! Runner failures and conversions from callees.

use super::{DataError, ViewId};
use crate::instance::InstanceError;
use crate::vm::Trap;
use exact_kernel::KernelError;
use exact_plan::{Plan, PlanError, TypesId, Value};

impl From<Trap> for RunnerError {
    fn from(t: Trap) -> Self {
        RunnerError::Trap(t)
    }
}

impl From<InstanceError> for RunnerError {
    fn from(e: InstanceError) -> Self {
        match e {
            InstanceError::SlotType { slot } => RunnerError::SlotType { slot },
            InstanceError::NonFiniteValue { name } => RunnerError::NonFiniteValue { name },
            other => RunnerError::Instance(other),
        }
    }
}

impl From<KernelError> for RunnerError {
    fn from(e: KernelError) -> Self {
        RunnerError::Kernel(e)
    }
}

/// Why the runner refused. The kernel is unchanged.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum RunnerError {
    KernelSchemaMismatch {
        plan: u64,
        kernel: u64,
    },
    /// The plan belongs to another app (LLP 1023 D5): its header names one identity, this binary's data crate another.
    AppMismatch {
        plan: String,
        host: String,
    },
    Plan(PlanError),
    NotOneRoot(usize),
    Trap(Trap),
    Instance(InstanceError),
    Kernel(KernelError),
    Data {
        resource: String,
        error: DataError,
    },
    Shape {
        resource: String,
        /// Where the answer first differs from the declared shape.
        why: String,
    },
    UnknownView(ViewId),
    /// A typed host event carries invalid numeric values. No action ran.
    InvalidEvent {
        event: &'static str,
    },
    /// A control's `input` or `change` carries a value it could never report (LLP 1069.001 D4): a select's value no enabled option has.
    InvalidValue {
        event: &'static str,
        reason: String,
    },
    NoHandler {
        view: ViewId,
        event: &'static str,
    },
    Arity {
        action: String,
        expected: usize,
        actual: usize,
    },
    /// Derives and resources depend on each other in a cycle; nothing settles.
    Cycle,
    /// An earlier update failed after the instance tree had begun to change; the runner no longer matches its kernel and must be restarted (D5).
    Poisoned,
    /// `advance` was given a non-finite time.
    NonFiniteClock,
    /// A viewport dimension is non-finite or non-positive.
    InvalidViewport,
    /// A date fact is non-finite, negative, or its offset past ±18 hours.
    InvalidTime,
    /// A locale or time zone is empty, over-long, or not in its form.
    InvalidPlace,
    /// The declared router shapes, table, launch fallback or value is invalid.
    Router(String),
    /// A clock value exceeds the exact integer-millisecond domain.
    ClockOutOfRange,
    /// Adding a timer interval did not advance its next due time.
    ClockDidNotAdvance {
        timer: usize,
    },
    /// One seek reached the bounded number of timer commits it may perform.
    TimerFireLimit {
        limit: usize,
    },
    /// A send would be the 65th waiting for a `queue` mutation (LLP 1092 D4).
    QueueFull {
        mutation: String,
    },
    /// A gated task's `key=` is no key (a non-finite number; LLP 1092 D8).
    TaskKey {
        task: String,
    },
    /// A region sits at the plan root; v1 requires one root node.
    RootRegion,
    /// Curried argument evaluation refused before this handler's action ran.
    HandlerArguments {
        action: String,
        error: Box<RunnerError>,
    },
    /// A slot initializer or write does not conform to the slot's declared type.
    SlotType {
        slot: String,
    },
    /// A derive's value does not conform to its declared type.
    DeriveType {
        derive: String,
    },
    /// An action argument does not conform to the parameter's declared type.
    ArgumentType {
        action: String,
        param: String,
    },
    /// A slot initializer, write, derive or argument contains a non-finite number.
    NonFiniteValue {
        name: String,
    },
    /// An action argument or a slot write is a string longer than [`crate::vm::MAX_STRING`] bytes.
    StringTooLong {
        name: String,
    },
}

impl RunnerError {
    pub(super) fn slot_type(plan: &Plan, value: &Value, ty: TypesId, name: &str) -> Self {
        if value.typed(plan, ty) {
            Self::NonFiniteValue { name: name.into() }
        } else {
            Self::SlotType { slot: name.into() }
        }
    }
}
