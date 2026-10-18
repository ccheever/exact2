//! A plan's bytes at boot, and how they decode.

use exact_plan::Plan;

/// A plan's bytes at boot: copied from while decoding, linked into the
/// program (whose data pool the decoded plan then keeps in place), or a
/// snapshot entry's by its kind (LLP 1126 D2): the headless session's boot,
/// which the running hosts never take.
#[derive(Clone, Copy)]
pub(crate) enum PlanBytes<'a> {
    Copied(&'a [u8]),
    Static(&'static [u8]),
    Entry(&'a [u8], &'a str),
}

impl<'a> PlanBytes<'a> {
    /// Bytes from the input buffer: an entry's when a kind names one.
    pub(crate) fn input(bytes: &'a [u8], entry: Option<&'a str>) -> Self {
        match entry {
            Some(kind) => PlanBytes::Entry(bytes, kind),
            None => PlanBytes::Copied(bytes),
        }
    }

    pub(crate) fn decode(self) -> Result<Plan, exact_plan::PlanError> {
        match self {
            PlanBytes::Copied(bytes) => Plan::decode(bytes),
            PlanBytes::Static(bytes) => Plan::decode_static(bytes),
            PlanBytes::Entry(bytes, kind) => Plan::decode_snapshot(bytes, kind),
        }
    }
}
