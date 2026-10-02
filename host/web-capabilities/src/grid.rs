//! CSS grid on the web: the kernel's grammar, validation and serialization,
//! linked when a plan binds any grid row so an app with no grid carries none
//! of them.

use exact_web::Linked;

/// Link CSS grid's grammar, validation and serialization into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.grid = Some(exact_kernel::style::link_grid);
    linked
}
