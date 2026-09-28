//! The web host's linked capabilities, above the core host.
//!
//! @ref LLP 1047 D3 (the generated entry names what the plan uses)
//! @ref LLP 1047 D4 (core crates stop depending on capabilities)
//!
//! `exact-web` names no capability's crate; each capability's adapter lives
//! here, over both. An app's generated entry names the ones its plan uses
//! with [`linked!`], and the linker drops the rest: an app that renders no
//! Markdown carries no `exact_markdown` code.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod auth;
pub mod backdrop;
pub mod canvas_colors;
pub mod collections;
pub mod documents;
pub mod drag;
pub mod effects;
pub mod format;
pub mod inspection;
pub mod markdown;
pub mod materials;
pub mod motion;
pub mod picker;
pub mod router;
pub mod share;
pub mod surfaces;
pub mod text_transform;
pub mod timelines;

/// The [`exact_web::Linked`] of the named capabilities, as a constant:
/// `linked!(markdown)`. The generated entry writes one; nothing else should.
#[macro_export]
macro_rules! linked {
    ($($capability:ident),* $(,)?) => {{
        let linked = ::exact_web::Linked::CORE;
        $(let linked = $crate::$capability::link(linked);)*
        linked
    }};
}

/// Every capability: what a development build and a native renderer link
/// (LLP 1047 D7).
pub const ALL: exact_web::Linked = linked!(
    markdown,
    motion,
    collections,
    drag,
    surfaces,
    router,
    format,
    inspection,
    canvas_colors,
    materials,
    backdrop,
    auth,
    share,
    documents,
    picker,
    timelines,
    text_transform,
    effects
);
