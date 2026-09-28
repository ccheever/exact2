//! Drag timelines on the web (LLP 1057.003): the three rows' grammar,
//! linked when a plan sets one, so an app that binds no animation to a drag
//! carries none of it.

use exact_web::Linked;

/// Link drag timelines into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.timelines = Some(exact_kernel::timeline::link);
    linked
}
