//! `saveFile` and the three file pickers on the web (LLP 1069.010): the
//! runner's rulings, linked when the plan runs one.

use exact_web::Linked;

/// Link documents into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.documents = true;
    linked
}
