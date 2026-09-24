//! Motion on the web: springs lowered to keyframes and the holds a gesture
//! takes (LLP 1002 D2). CSS plays every other transition, so a plan without
//! a spring or a hold links none of this.

use exact_web::Linked;

/// Link the spring engine into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.motion = Some(exact_web::motion::springs);
    linked
}
