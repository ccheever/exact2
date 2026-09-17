//! Native reference ownership: Rust never dereferences the opaque Swift owner.
use std::ffi::c_void;
/// Release one native artifact retain on the owning runtime/UI thread.
pub type RegionRelease = extern "C" fn(*mut c_void);
/// A retained native artifact. Its provider/worker/source owners can outlive the
/// Runtime; destruction calls the paired release exactly once, without a lookup.
pub struct NativeRegionOwner {
    pointer: *mut c_void,
    release: RegionRelease,
}
impl NativeRegionOwner {
    /// Takes one caller retain; even rejected/stale completion drops it.
    pub fn new(pointer: *mut c_void, release: RegionRelease) -> Self {
        Self { pointer, release }
    }
}
impl Drop for NativeRegionOwner {
    fn drop(&mut self) {
        (self.release)(self.pointer);
    }
}
