//! `filter` and `clip-path` on the web (LLP 1055.000 D10, D14): the kernel's
//! grammars for both rows, linked when a plan binds either, so an app that
//! filters and clips nothing carries neither.

use exact_web::Linked;

/// Link `filter` and `clip-path` into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.effects = Some(exact_kernel::style::link_effects);
    linked
}
