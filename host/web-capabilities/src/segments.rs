//! `env(viewport-segment-*)` on the web (LLP 1077 D3): the kernel's segment
//! grammar, resolution and wire decode, linked when a plan names a segment,
//! so an app that lays nothing out by the fold carries none of it. The
//! fold's `exactViewport` fields and `layout.env` are the core's.

use exact_web::Linked;

/// Link the viewport segment grammar into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.segments = Some(exact_kernel::style::link_segments);
    linked
}
