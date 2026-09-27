//! The wire: the EXWF frame format, the op vocabulary, and the byte codec.

mod animation;
pub mod codec;
pub mod frame;
pub mod ops;

pub use frame::{decode, encode, Frame, FrameBuilder};
pub use ops::Op;
