//! `text-transform` on the web (LLP 1064 D5): the kernel's Unicode case
//! mapping, linked when a plan binds the row, so an app that transforms no
//! text carries none of the case tables.

use exact_web::Linked;

/// Link `text-transform` into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.text_transform = Some(exact_kernel::link_text_transform);
    linked
}
