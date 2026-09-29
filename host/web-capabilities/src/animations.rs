//! CSS animations on the web (LLP 1055 D5): the `animation` shorthand's and
//! `@keyframes`' grammars, and the page's CSS for them (the lists a node
//! declares, the rules they name), linked when a plan declares keyframes or
//! binds `animation` or `exit-animation`, so an app that animates nothing
//! that way carries none of it.

use exact_web::{AnimationsLink, Linked};

/// Link CSS animations into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.animations = Some(AnimationsLink {
        grammars: exact_kernel::style::link_animations,
        list: exact_web::css::animations_css,
        name: exact_web::css::keyframes_name,
        body: exact_web::css::keyframes_css,
    });
    linked
}
