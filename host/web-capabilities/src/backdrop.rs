//! `backdrop-filter` on the web (LLP 1053.000 D1): the row's grammar and its
//! named refusals, linked when a plan sets the row, so an app that blurs no
//! backdrop carries neither.

use exact_web::Linked;

/// Link `backdrop-filter` into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.backdrop = Some(exact_kernel::style::link_backdrop_filter);
    linked
}
