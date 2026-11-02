//! `formatDate` and `formatNumber` on the web: the
//! runner's bodies, reached only through what this registers, so an app
//! that calls neither carries none of them. `formatTime` is the core's.

use exact_web::Linked;

/// Link the `format` capability into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.format = Some(exact_runner::formatting);
    linked
}
