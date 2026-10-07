//! The final allocation owner is also Peniko's zero-copy byte provider.
use exact_raster::AllocationCharge;
use tiny_skia::Pixmap;

/// One immutable premultiplied raster, retaining its debit through every CPU,
/// GPU Blob, scene and renderer-cache owner. Natural pixels are layout units.
pub struct Bitmap {
    // Field order matters: pixels are destroyed before the last charge drops.
    pixels: Pixels,
    natural: (u32, u32),
    _charge: AllocationCharge,
    // No backedge to the backend, session, payload or runtime.
    _source: Option<std::sync::Arc<super::workers::SourceOwner>>,
}
/// Where a decoded picture's pixels are.
pub(super) enum Pixels {
    /// On the heap.
    Cpu(Pixmap),
    /// In a GPU buffer a reader draws from ([`super::hardware`]).
    #[cfg(target_os = "android")]
    Hardware(super::hardware::Hardware),
}
impl From<Pixmap> for Pixels {
    fn from(pixels: Pixmap) -> Self {
        Pixels::Cpu(pixels)
    }
}
impl Pixels {
    fn cpu(&self) -> &Pixmap {
        match self {
            Pixels::Cpu(p) => p,
            #[cfg(target_os = "android")]
            Pixels::Hardware(h) => h.pixels(),
        }
    }
    fn size(&self) -> (u32, u32) {
        match self {
            Pixels::Cpu(p) => (p.width(), p.height()),
            #[cfg(target_os = "android")]
            Pixels::Hardware(h) => h.size(),
        }
    }
}
impl Bitmap {
    #[cfg(test)]
    pub(crate) fn new(pixels: Pixmap, natural: (u32, u32), charge: AllocationCharge) -> Self {
        Self {
            pixels: pixels.into(),
            natural,
            _charge: charge,
            _source: None,
        }
    }
    pub(super) fn from_source(
        pixels: Pixels,
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
    /// The GIF or WebP file this is the first frame of, which a reader with an
    /// animated drawable of its own may play in its place.
    pub fn animation_file(&self) -> Option<std::path::PathBuf> {
        let source = self._source.as_ref()?;
        let name = source.name().to_ascii_lowercase();
        if !(name.ends_with(".gif") || name.ends_with(".webp")) {
            return None;
        }
        source.file()
    }
    /// Original source dimensions, independent of the chosen decode resolution.
    pub fn natural(&self) -> (u32, u32) {
        self.natural
    }
    /// Decoded width, used only for sampling the backing pixels.
    pub fn width(&self) -> u32 {
        self.pixels.size().0
    }
    /// Decoded height, used only for sampling the backing pixels.
    pub fn height(&self) -> u32 {
        self.pixels.size().1
    }
    /// The decoded pixels' bytes, wherever they are.
    pub fn bytes(&self) -> u64 {
        let (w, h) = self.pixels.size();
        u64::from(w) * u64::from(h) * 4
    }
    /// Whether a reader can draw from these pixels where they are (a GPU
    /// buffer) instead of copying them.
    pub fn shared(&self) -> bool {
        #[cfg(target_os = "android")]
        if matches!(self.pixels, Pixels::Hardware(_)) {
            return true;
        }
        false
    }
    /// The GPU buffer holding the pixels (an `AHardwareBuffer*`, alive while
    /// this is), when they were decoded into one.
    #[cfg(target_os = "android")]
    pub fn hardware(&self) -> Option<*mut std::ffi::c_void> {
        match &self.pixels {
            Pixels::Hardware(h) => Some(h.buffer()),
            Pixels::Cpu(_) => None,
        }
    }
    /// Borrow the immutable pixels; this never detaches them from their charge.
    pub fn pixels(&self) -> tiny_skia::PixmapRef<'_> {
        self.pixels.cpu().as_ref()
    }
}
impl AsRef<[u8]> for Bitmap {
    fn as_ref(&self) -> &[u8] {
        self.pixels.cpu().data()
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
