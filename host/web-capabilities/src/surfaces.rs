//! GPU canvas surfaces on the web: the app's side of a canvas with a
//! surface, whose records and requests the host answers. The GPU module is
//! its own artifact, loaded at the first canvas; what the app links is the
//! host's surface exports (`exact_web::surface_exports!`), which the
//! generated entry invokes beside this registration.

use exact_web::Linked;

/// Link surfaces into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.surfaces = true;
    linked
}
