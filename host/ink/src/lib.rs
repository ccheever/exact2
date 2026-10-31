//! Instanced UI geometry over a portable device. @ref LLP 1149 D1, D5.

pub mod atlas;
pub mod counters;
pub mod device;
pub mod glyph;
pub mod image;
pub mod instance;
pub mod layer;
pub mod path;
#[cfg(target_os = "macos")]
mod present;
pub mod rows;

pub use device::wgpu::Wgpu;
pub use instance::{Brush, Encoder, Frame, Instance, Kind};
