//! Drags on the web: height, transform and reorder handles, which the host
//! tracks, reconciles and publishes at every commit. Their hooks are generic
//! over the app's data source, so the `host!` macro builds them from this
//! flag as `exact_web::HostLinks::of(EXACT_LINKED)`.

use exact_web::Linked;

/// Link drags into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.drag = true;
    linked
}
