//! What this archive links beyond the core (LLP 1047.001 D2, D5).
//!
//! @ref LLP 1047 D6 (a host refuses a plan that uses what it doesn't link)
//! @ref LLP 1047.001 D2 (one composition per app), D5 (refused before anything changes)
//!
//! An app archive is one composition (LLP 1031: an embedder links one), so
//! the set is the process's: the generated entry names it before the first
//! runtime exists, and every boot — the first, a reload, a delivered
//! candidate — admits its plan against it after decoding.

pub use exact_runner::{Capability, Uses};
use std::sync::OnceLock;

static LINKED: OnceLock<Uses> = OnceLock::new();

/// Every capability: what an entry that names none links (a hand-written
/// `host!`, a development build).
pub const ALL: Uses = {
    let mut uses = Uses::NONE;
    let mut i = 0;
    while i < Capability::ALL.len() {
        uses = uses.with(Capability::ALL[i]);
        i += 1;
    }
    uses
};

/// Name this archive's linked set. The first call wins: the entry makes it
/// at the first `exact_create`, before any boot.
pub fn set(linked: Uses) {
    let _ = LINKED.set(linked);
}

/// This archive's linked set; every capability until an entry names one.
pub fn linked() -> Uses {
    LINKED.get().copied().unwrap_or(ALL)
}

/// The capabilities `plan` uses that `linked` lacks, by name; `None` when
/// it links them all.
pub fn missing(plan: &exact_plan::Plan, linked: Uses) -> Option<String> {
    let missing = exact_runner::uses(plan).beyond(linked);
    (!missing.is_empty()).then(|| missing.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_holds_every_capability() {
        assert!(Capability::ALL.iter().all(|c| ALL.has(*c)));
    }

    /// D5: a plan that uses what the archive doesn't link is named and
    /// refused; one within it is admitted.
    #[test]
    fn a_plan_beyond_the_linked_set_names_what_is_missing() {
        let grouped = contract::compile(
            "component A\n  view\n    list appearance=\"auto\"\n      section\n        text \"a\"\n",
        )
        .unwrap();
        let plain = contract::compile("component A\n  view\n    text \"a\"\n").unwrap();
        let without = Capability::ALL
            .into_iter()
            .filter(|c| *c != Capability::GroupedLists)
            .fold(Uses::NONE, Uses::with);
        assert_eq!(missing(&grouped, without).as_deref(), Some("grouped_lists"));
        assert_eq!(missing(&grouped, ALL), None);
        assert_eq!(missing(&plain, Uses::NONE), None);
    }
}
