//! `showNotification` and `closeNotification` on the web: the runner's
//! ruling on the commands (runner/src/notify.rs), linked when the plan runs
//! one.

use exact_web::Linked;

/// Link notifications into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.notifications = true;
    linked
}
