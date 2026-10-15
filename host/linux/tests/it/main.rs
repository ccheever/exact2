//! The Linux host's integration tests that leave the font environment alone:
//! one binary. `tests/pinned/` holds the ones that pin the fixture font, so
//! neither changes the other's process environment.

mod accent;
mod ahead;
mod animation;
mod arrange;
mod colors;
mod field_align;
mod height;
mod height_binding;
mod holds;
mod image;
mod list_rows;
mod media_session;
mod native_buttons;
mod native_fields;
mod presence;
mod svg;
mod timeline;
mod transform_binding;
mod transform_contact;
mod viewport;
