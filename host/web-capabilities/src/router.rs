//! The router on the web (LLP 1038): the runner's routing for a plan that
//! declares routes. Its verbs, reads, change publication and link matching
//! are reached only through what this registers.

use exact_web::Linked;

/// Link the router into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.router = Some(exact_runner::routing);
    linked
}
