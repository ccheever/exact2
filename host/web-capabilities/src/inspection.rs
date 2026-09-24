//! Inspection on the web (LLP 1012): the agent API's reads over the runner.
//! It is not a plan's use; the generated entry links it by policy, in
//! production too (LLP 1047 §10, Q3), so the smoked artifact is the shipped
//! one. The `host!` macro builds its hook from this flag.

use exact_web::Linked;

/// Link inspection into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.inspection = true;
    linked
}
