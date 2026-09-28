//! `share(…)` on the web (LLP 1069.003): the runner's ruling on the command
//! and an agent's answer, linked when the plan runs `share`.

use exact_web::Linked;

/// Link share into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.share = true;
    linked
}
