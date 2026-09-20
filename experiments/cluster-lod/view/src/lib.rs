//! Portable offscreen reference renderer. Files, clocks, polling and PNGs live in the CLI.
mod compute;
mod gpu;
mod hero;
mod render;
pub mod scene;
pub mod select;
pub use compute::Selector;
pub use gpu::{Baseline, Mode, Renderer, View, device_descriptor, request_device};
pub use render::{Frame, FrameStats};
pub use wgpu;

pub mod timing;
