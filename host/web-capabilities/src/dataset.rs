//! `data-*` words on the web (LLP 1075.003 §3.3): a `dataset` row read back
//! into one attribute per word, linked when a plan binds the row, so an app
//! that names no words carries none of the reading.

use exact_web::Linked;

/// Link `data-*` words into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.dataset = Some(exact_web::document::dataset);
    linked
}
