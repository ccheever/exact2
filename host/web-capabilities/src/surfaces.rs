//! GPU canvas surfaces on the web: the app's side of a canvas with a
//! surface, whose records and requests the host answers. The GPU module is
//! its own artifact, loaded at the first canvas; what the app links is the
//! runner's answer for a surface's record and the host's surface exports
//! (`exact_web::surface_exports!`), which the generated entry invokes beside
//! this registration.

use exact_web::Linked;

/// Link surfaces into `linked`: the runner's answer for a surface's record,
/// and Canvas 2D's engine, since a canvas with a surface is a 2D one when
/// its source draws it (LLP 1056).
pub const fn link(mut linked: Linked) -> Linked {
    linked.surface_answer = Some(exact_runner::surface_record::answer);
    linked.canvas = Some(exact_runner::canvas_engine);
    linked
}
