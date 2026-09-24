//! What this artifact links beyond the core.
//!
//! @ref LLP 1047 D3 (the generated entry links what the plan uses)
//! @ref LLP 1047 D6 (using an unlinked capability is a named refusal)
//!
//! The core host never names a capability's crate. It reaches one only
//! through [`Linked`], which the app's generated entry fills with the
//! capabilities its plan uses (`exact-web-capabilities` holds them), and
//! registers with [`link`] before every boot. What no entry names, the linker
//! drops. A plan that uses more than the artifact links is refused at boot,
//! by name, before anything runs.

use crate::HostError;
use exact_runner::uses::{Capability, Uses};
use std::cell::Cell;

/// The capabilities an artifact links: one registration per capability.
#[derive(Clone, Copy)]
pub struct Linked {
    /// Markdown: a source as the batch's `markupPieces` JSON.
    pub markup: Option<fn(&str) -> String>,
    /// Motion: the spring engine a host holds instead of
    /// [`crate::motion::Still`].
    pub motion: Option<fn() -> Box<dyn crate::motion::Motion>>,
    /// Lists the host windows: their exports (`list_exports!`) are in.
    pub collections: bool,
}

impl Linked {
    /// The core alone.
    pub const CORE: Linked = Linked {
        markup: None,
        motion: None,
        collections: false,
    };

    /// The capabilities registered here.
    pub fn uses(&self) -> Uses {
        let mut uses = Uses::NONE;
        if self.markup.is_some() {
            uses = uses.with(Capability::Markdown);
        }
        if self.motion.is_some() {
            uses = uses.with(Capability::Motion);
        }
        if self.collections {
            uses = uses.with(Capability::Collections);
        }
        uses
    }
}

thread_local! {
    // On the web, one thread and a plain static; natively, a renderer
    // registers on the thread that projects.
    static LINKED: Cell<Linked> = const { Cell::new(Linked::CORE) };
}

/// Register what this artifact links: its entry does before every boot.
pub fn link(linked: Linked) {
    LINKED.with(|cell| cell.set(linked));
}

/// What this artifact links; the core alone until an entry registers.
pub fn linked() -> Linked {
    LINKED.with(Cell::get)
}

/// Link, on this test's thread, what this crate can register itself: the
/// spring engine and lists (Markdown's adapter lives above it), as an entry
/// whose plan uses them does.
#[cfg(test)]
pub(crate) fn link_for_tests() {
    link(Linked {
        motion: Some(crate::motion::springs),
        collections: true,
        ..linked()
    });
}

/// Admit a plan only if this artifact links everything it uses (D6).
pub(crate) fn admit(plan: &exact_plan::Plan) -> Result<(), HostError> {
    let missing = exact_runner::uses(plan).beyond(linked().uses());
    if missing.is_empty() {
        Ok(())
    } else {
        Err(HostError::Unlinked(missing.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{Host, HostError};

    /// Nothing registers on this test's thread, so it is the core alone.
    #[test]
    fn a_plan_that_uses_an_unlinked_capability_is_refused_by_name() {
        let markdown =
            contract::compile("component A\n  view\n    text \"**b**\" markup=\"markdown\"\n")
                .unwrap()
                .encode();
        let refused = Host::boot(&markdown, (), Default::default(), "/").map(|_| ());
        assert!(
            matches!(&refused, Err(HostError::Unlinked(names)) if names == "markdown"),
            "{refused:?}"
        );
        let plain = contract::compile("component A\n  view\n    text \"b\"\n").unwrap();
        assert!(Host::boot(&plain.encode(), (), Default::default(), "/").is_ok());
    }
}
