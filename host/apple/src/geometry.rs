//! A shaped inline run's viewport union, read from its native presenter.
use exact_runner::geometry::GeometryAnswer;
use std::ffi::c_void;

/// The presenter's reply; an absent run has `available == 0`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct InlineFrame {
    /// Viewport left edge, in points.
    pub x: f64,
    /// Viewport top edge, in points.
    pub y: f64,
    /// Union width, in points.
    pub width: f64,
    /// Union height, in points.
    pub height: f64,
    /// One when the presenter has laid out this run.
    pub available: u32,
}

/// Synchronous, on the runtime owner; the presenter serves UI work on main.
pub type InlineFrameFn = extern "C" fn(*mut c_void, u32) -> InlineFrame;

#[derive(Debug)]
pub(crate) struct InlineGeometry {
    pub read: InlineFrameFn,
    pub ctx: *mut c_void,
}

impl exact_runner::geometry::InlineGeometry for InlineGeometry {
    fn frame(&self, view: u32) -> GeometryAnswer {
        let r = (self.read)(self.ctx, view);
        GeometryAnswer {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
            provisional: false,
            unavailable: r.available == 0,
        }
    }
}
