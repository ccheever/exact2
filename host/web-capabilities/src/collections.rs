//! Lists the host windows: virtualized collections and lists with row
//! heights. The engine is the runner's; what an app links for them is the
//! host's list exports (`exact_web::list_exports!`), which the generated
//! entry invokes beside this registration.

use exact_web::Linked;

/// Link lists into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.collections = true;
    linked
}
