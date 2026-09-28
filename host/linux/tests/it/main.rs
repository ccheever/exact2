//! The Linux host's integration tests that leave the font environment alone:
//! one binary. `tests/pinned/` holds the ones that pin the fixture font, so
//! neither changes the other's process environment.

mod animation;
mod arrange;
mod height;
mod height_binding;
mod holds;
mod image;
mod presence;
mod svg;
mod timeline;
mod transform_binding;
mod transform_contact;
mod viewport;
