//! The wire: the EXWF frame format, the op vocabulary, and the byte codec.
//!
//! A wide colour crosses as its text, never its `ColorValue::Wide` id: ids
//! are this process's, reclaimed by the table's sweep (`style::wide`, LLP
//! 1121 stage 4), and only kernels and runners hold them. The Apple and web
//! hosts convert colours to text or components at once; the Linux host
//! converts the paint it retains at capture (`wide::Settle`, `wide::Values`).
//! An id on the wire would need a holder at the far end.

mod animation;
pub mod codec;
pub mod frame;
pub mod ops;

pub use frame::{decode, encode, Frame, FrameBuilder};
pub use ops::{look_rows, Op, LOOK_FOCUS_VISIBLE, LOOK_HOVER};
