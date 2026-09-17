//! The final allocation owner is also Peniko's zero-copy byte provider.
use exact_raster::AllocationCharge;
use tiny_skia::Pixmap;

/// One immutable premultiplied raster, retaining its debit through every CPU,
/// GPU Blob, scene and renderer-cache owner. Natural pixels are layout units.
pub struct Bitmap {
    // Field order matters: pixels are destroyed before the last charge drops.
    pixels: Pixmap,
    natural: (u32, u32),
    _charge: AllocationCharge,
    // No backedge to the backend, session, payload or runtime.
    _source: Option<std::sync::Arc<super::workers::SourceOwner>>,
}
impl Bitmap {
    #[cfg(test)]
    pub(crate) fn new(pixels: Pixmap, natural: (u32, u32), charge: AllocationCharge) -> Self {
        Self {
            pixels,
            natural,
            _charge: charge,
            _source: None,
        }
    }
    pub(super) fn from_source(
        pixels: Pixmap,
        natural: (u32, u32),
        charge: AllocationCharge,
        source: std::sync::Arc<super::workers::SourceOwner>,
    ) -> Self {
        Self {
            pixels,
            natural,
            _charge: charge,
            _source: Some(source),
        }
    }
    /// Original source dimensions, independent of the chosen decode resolution.
    pub fn natural(&self) -> (u32, u32) {
        self.natural
    }
    /// Decoded width, used only for sampling the backing pixels.
    pub fn width(&self) -> u32 {
        self.pixels.width()
    }
    /// Decoded height, used only for sampling the backing pixels.
    pub fn height(&self) -> u32 {
        self.pixels.height()
    }
    /// Borrow the immutable pixels; this never detaches them from their charge.
    pub fn pixels(&self) -> tiny_skia::PixmapRef<'_> {
        self.pixels.as_ref()
    }
}
impl AsRef<[u8]> for Bitmap {
    fn as_ref(&self) -> &[u8] {
        self.pixels.data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_raster::Gate;
    use std::sync::Arc;
    use vello::peniko::Blob;

    #[test]
    fn scene_blob_and_second_view_keep_the_unique_allocation_charged() {
        let gate = Gate::new();
        let session = gate.session();
        let reservation = session.reserve_allocation(400).unwrap();
        let charge = reservation.charge(); // BEFORE any pixels exist.
        let bitmap = Arc::new(Bitmap::new(
            Pixmap::new(10, 10).unwrap(),
            (4000, 4000),
            charge,
        ));
        let charge = reservation.commit(400).unwrap();
        drop(charge);
        let second_view = bitmap.clone();
        let blob = Blob::new(bitmap.clone());
        assert_eq!(bitmap.as_ref().as_ref().as_ptr(), blob.data().as_ptr());
        drop(bitmap);
        session.reset();
        assert_eq!(session.stats().resident_bytes, 400);
        drop(second_view);
        assert_eq!(session.stats().resident_bytes, 400);
        std::thread::spawn(move || drop(blob)).join().unwrap();
        assert_eq!(session.stats().resident_bytes, 0);
    }
}
