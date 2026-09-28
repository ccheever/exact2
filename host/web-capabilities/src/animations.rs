//! CSS animations on the web (LLP 1055 D5): the `animation` shorthand's and
//! `@keyframes`' grammars, linked when a plan declares keyframes or binds
//! `animation` or `exit-animation`, so an app that animates nothing that way
//! carries neither.

use exact_web::Linked;

/// Link CSS animations into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.animations = Some(exact_kernel::style::link_animations);
    linked
}
