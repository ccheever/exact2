//! Instanced UI geometry over a portable device. @ref LLP 1149 D1, D5.

pub mod device;
pub mod instance;

pub use device::wgpu::Wgpu;
pub use instance::{Brush, Encoder, Frame, Instance, Kind};
