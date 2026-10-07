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
use exact_runner::{DataSource, DeviceLinks, RunnerLinks};
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

/// The kernel's style grammars a native host links by use (LLP 1047.001
/// D3): each `Some` is registered at boot.
#[derive(Clone, Copy)]
pub struct Grammars {
    backdrop: Option<fn()>,
    segments: Option<fn()>,
    timelines: Option<fn()>,
    wide_colors: Option<fn()>,
}

impl Grammars {
    /// Every grammar.
    pub const ALL: Grammars = Grammars::of(ALL);

    /// The grammars `uses` needs. Wide colours are linked always: the web
    /// decides them from the plan's strings and the data crate's source,
    /// which a native entry does not read.
    pub const fn of(uses: Uses) -> Grammars {
        Grammars {
            backdrop: if uses.has(Capability::Backdrop) {
                Some(exact_kernel::style::link_backdrop_filter)
            } else {
                None
            },
            segments: if uses.has(Capability::Segments) {
                Some(exact_kernel::style::link_segments)
            } else {
                None
            },
            timelines: if uses.has(Capability::Timelines) {
                Some(exact_kernel::timeline::link)
            } else {
                None
            },
            wide_colors: Some(exact_kernel::style::link_wide_colors),
        }
    }

    /// Register them.
    pub fn link(self) {
        for link in [
            self.backdrop,
            self.segments,
            self.timelines,
            self.wide_colors,
        ]
        .into_iter()
        .flatten()
        {
            link();
        }
    }
}

/// What a boot links (LLP 1047.001 D3): the runner's capabilities, its
/// device capabilities and the kernel's grammars. An entry makes it in a
/// `const` from its set (`host!`), so the linker sees only what it names;
/// the public boots, which tests and tools use, link everything.
pub struct Links<D: DataSource> {
    /// The runner's capabilities (the router, lists, canvases, …).
    pub runner: RunnerLinks,
    /// The device capabilities (share, documents, pickers, notifications;
    /// authentication, which follows the grants, always).
    pub device: DeviceLinks<D>,
    /// The kernel's style grammars.
    pub grammars: Grammars,
}

impl<D: DataSource> Clone for Links<D> {
    fn clone(&self) -> Self {
        Links {
            runner: self.runner,
            device: self.device,
            grammars: self.grammars,
        }
    }
}

impl<D: DataSource> Links<D> {
    /// Every capability.
    pub const ALL: Links<D> = Links {
        runner: RunnerLinks::ALL,
        device: DeviceLinks::ALL,
        grammars: Grammars::ALL,
    };

    /// What `uses` needs; evaluated in a `const`, so nothing else is named.
    pub const fn of(uses: Uses) -> Links<D> {
        let all = RunnerLinks::ALL;
        Links {
            runner: RunnerLinks {
                surface_answer: if uses.has(Capability::Surfaces) {
                    all.surface_answer
                } else {
                    None
                },
                router: if uses.has(Capability::Router) {
                    all.router
                } else {
                    None
                },
                lists: if uses.has(Capability::Collections) {
                    all.lists
                } else {
                    None
                },
                canvas: if uses.has(Capability::Surfaces) {
                    all.canvas
                } else {
                    None
                },
                format: if uses.has(Capability::Format) {
                    all.format
                } else {
                    None
                },
                geometry: if uses.has(Capability::Geometry) {
                    all.geometry
                } else {
                    None
                },
            },
            device: DeviceLinks {
                auth: DeviceLinks::<D>::ALL.auth,
                share: if uses.has(Capability::Share) {
                    DeviceLinks::<D>::ALL.share
                } else {
                    None
                },
                documents: if uses.has(Capability::Documents) {
                    DeviceLinks::<D>::ALL.documents
                } else {
                    None
                },
                picker: if uses.has(Capability::Picker) {
                    DeviceLinks::<D>::ALL.picker
                } else {
                    None
                },
                notifications: if uses.has(Capability::Notifications) {
                    DeviceLinks::<D>::ALL.notifications
                } else {
                    None
                },
            },
            grammars: Grammars::of(uses),
        }
    }
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
