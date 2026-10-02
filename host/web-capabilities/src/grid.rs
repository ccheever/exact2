//! CSS grid on the web: the kernel's track, placement and keyword grammars,
//! linked when a plan binds any grid row so an app with no grid carries none
//! of them.

use exact_web::Linked;

/// Link CSS grid's grammars into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.grid = Some(exact_kernel::style::link_grid);
    linked
}
