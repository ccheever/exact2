//! Instance realization failures.

use super::*;

/// Why an instance could not be realized.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceError {
    Trap(Trap),
    Bridge(bridge::BridgeError),
    UnknownNodeType(u8),
    List(&'static str),
    SubjectKind {
        region: RegionsId,
    },
    KeyKind {
        region: RegionsId,
    },
    DuplicateKey {
        region: RegionsId,
    },
    NonFiniteValue {
        name: String,
    },
    SlotType {
        slot: String,
    },
    Collection(String),
    /// Host geometry rejected before changing any collection or kernel state.
    InvalidCollectionFeedback,
}

impl InstanceError {
    pub(crate) fn slot_type(
        plan: &exact_plan::Plan,
        value: &Value,
        ty: exact_plan::TypesId,
        name: &str,
    ) -> Self {
        if value.typed(plan, ty) {
            Self::NonFiniteValue { name: name.into() }
        } else {
            Self::SlotType { slot: name.into() }
        }
    }
}

impl From<Trap> for InstanceError {
    fn from(t: Trap) -> Self {
        InstanceError::Trap(t)
    }
}
