//! `background-image` on the web (LLP 1066): the kernel's gradient grammar,
//! linked when a plan binds the row, so an app that paints no gradient
//! carries none of it.

use exact_web::Linked;

/// Link `background-image`'s gradients into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.gradients = Some(exact_kernel::style::link_gradients);
    linked
}
