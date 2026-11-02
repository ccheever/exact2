//! `share(…)` on the web: the runner's decision on the command
//! and an agent's answer, linked when the plan runs `share`.

use exact_web::Linked;

/// Link share into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.share = true;
    linked
}
