//! The final allocation owner is also Peniko's zero-copy byte provider.
use exact_raster::AllocationCharge;
use tiny_skia::Pixmap;

/// One immutable premultiplied raster, retaining its debit through every CPU,
/// GPU Blob, scene and renderer-cache owner. Natural pixels are layout units.
pub struct Bitmap {
    // Field order matters: pixels are destroyed before the last charge drops.
    pixels: Pixels,
    natural: (u32, u32),
    /// The part of the picture the pixels are, when not all of it
    /// ([`Bitmap::placed`]).
    crop: exact_raster::Crop,
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
            crop: exact_raster::Crop::default(),
            _charge: charge,
            _source: None,
        }
    }
    /// Pixels a package's implementation decoded (`package.rs`), charged
    /// to the session as a decode is.
    pub(super) fn decoded(pixels: Pixmap, natural: (u32, u32), charge: AllocationCharge) -> Self {
        Self {
            pixels: pixels.into(),
            natural,
            crop: exact_raster::Crop::default(),
            _charge: charge,
            _source: None,
        }
    }
    pub(super) fn from_source(
        pixels: Pixels,
        natural: (u32, u32),
        crop: exact_raster::Crop,
        charge: AllocationCharge,
        source: std::sync::Arc<super::workers::SourceOwner>,
    ) -> Self {
        Self {
            pixels,
            natural,
            crop,
            _charge: charge,
            _source: Some(source),
        }
    }
    /// The GIF or WebP file this is the first frame of, which a reader with an
    /// animated drawable of its own may play in its place.
    pub fn animation_file(&self) -> Option<std::path::PathBuf> {
        let source = self._source.as_ref()?;
        let name = source.name().to_ascii_lowercase();
        // A URL's path, not its query.
        let name = name.split(['?', '#']).next().unwrap_or_default();
        if !(name.ends_with(".gif") || name.ends_with(".webp")) {
            return None;
        }
        source.file()
    }
    /// Where the pixels go when the whole picture goes to `whole`: there,
    /// or, for a picture decoded as the part of it a box shows
    /// (`image::crop`), the same part of `whole`. A painter places a
    /// picture by its natural size and draws the pixels here, so a part is
    /// drawn where the whole's pixels of it would be, at the same scale.
    pub fn placed(&self, whole: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
        if self.crop.whole() {
            return whole;
        }
        let (fw, fh) = (self.crop.full.width as f32, self.crop.full.height as f32);
        let (w, h) = self.pixels.size();
        (
            whole.0 + whole.2 * self.crop.x as f32 / fw,
            whole.1 + whole.3 * self.crop.y as f32 / fh,
            whole.2 * w as f32 / fw,
            whole.3 * h as f32 / fh,
        )
    }
    /// The part of the picture the pixels are (the whole, by default).
    pub fn crop(&self) -> exact_raster::Crop {
        self.crop
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
    fn bilinear_cropped_bitmap_matches_two_pixel_margin_byte_exactly() {
        use crate::image::crop;
        use crate::paint::{object_fit, Backend, Shape};
        use crate::raster::Raster;
        use exact_kernel::ObjectFit;
        use exact_raster::{Crop, PixelSize};
        use tiny_skia::{Color, IntRect, Transform};

        let gate = Gate::new();
        let session = gate.session();
        for transpose in [false, true] {
            let natural = if transpose { (72, 128) } else { (128, 72) };
            let full = PixelSize {
                width: natural.0,
                height: natural.1,
            };
            let mut source = Pixmap::new(natural.0, natural.1).unwrap();
            for (i, pixel) in source.pixels_mut().iter_mut().enumerate() {
                let column = if transpose {
                    i as u32 / natural.0
                } else {
                    i as u32 % natural.0
                };
                let value = (column % 2 * 255) as u8;
                *pixel = Color::from_rgba8(value, value, value, 255)
                    .premultiply()
                    .to_color_u8();
            }
            // Crops chosen by MARGIN = 2 at draw scales 0.5, 50/72 and 1.
            for (height, start, width) in [(36., 44, 40), (50., 49, 30), (72., 53, 22)] {
                let (box_rect, expected) = if transpose {
                    ((0., 0.1, height, 17.3), (0, start, (72, width)))
                } else {
                    ((0.1, 0., 17.3, height), (start, 0, (width, 72)))
                };
                let actual =
                    crop::part(natural, full, (box_rect.2, box_rect.3), ObjectFit::Cover).unwrap();
                let dst = object_fit(natural, ObjectFit::Cover, box_rect).unwrap();
                let mut drawn = Vec::new();
                for (x, y, (w, h)) in [actual, expected] {
                    let pixels = source
                        .clone_rect(IntRect::from_xywh(x as i32, y as i32, w, h).unwrap())
                        .unwrap();
                    let bytes = u64::from(w) * u64::from(h) * 4;
                    let reservation = session.reserve_allocation(bytes).unwrap();
                    let mut bitmap = Bitmap::new(pixels, natural, reservation.charge());
                    bitmap.crop = Crop { x, y, full };
                    let bitmap = Arc::new(bitmap);
                    reservation.commit(bytes).unwrap();
                    let placed = bitmap.placed(dst);
                    assert!(placed.2 / w as f32 >= 0.5 && placed.3 / h as f32 >= 0.5);
                    let mut raster = Raster::transparent();
                    raster.begin(80., 80., 1.);
                    raster.image(
                        &bitmap,
                        placed,
                        &[Shape::rect(box_rect)],
                        Transform::identity(),
                        None,
                    );
                    drawn.push(raster.finish().unwrap());
                }
                if !transpose && height == 50. {
                    let pixel = drawn[1].pixel(7, 2).unwrap();
                    assert_eq!(
                        [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
                        [76, 76, 76, 255]
                    );
                }
                assert!(
                    drawn[0].data() == drawn[1].data(),
                    "transpose={transpose} height={height}"
                );
            }
        }
    }

    #[test]
    fn shrinking_cropped_decode_matches_whole_inside_fractional_box() {
        use crate::image::{
            crop,
            png_decode::{decode_rows, inspect, DecodePlan},
        };
        use crate::paint::{object_fit, Backend, Shape};
        use crate::raster::Raster;
        use exact_kernel::ObjectFit;
        use std::io::Cursor;
        use tiny_skia::{Color, Transform};

        let gate = Gate::new();
        let session = gate.session();
        for transpose in [false, true] {
            let natural = if transpose { (256, 1024) } else { (1024, 256) };
            let full = if transpose { (32, 128) } else { (128, 32) };
            for patterned in [false, true] {
                let mut source = Pixmap::new(natural.0, natural.1).unwrap();
                source.fill(Color::WHITE);
                if patterned {
                    for (i, pixel) in source.pixels_mut().iter_mut().enumerate() {
                        let x = i as u32 % natural.0 / 8;
                        let y = i as u32 / natural.0 / 8;
                        *pixel = Color::from_rgba8(
                            (x * 71) as u8,
                            (y * 53) as u8,
                            ((x + y) * 29) as u8,
                            ((x * 17 + y * 31) % 256) as u8,
                        )
                        .premultiply()
                        .to_color_u8();
                    }
                }
                let bytes = source.encode_png().unwrap();
                let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
                let plan = DecodePlan::new(header, full).unwrap();
                let whole = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
                for scale in [1., 1.25, 1.5] {
                    let box_rect = if transpose {
                        (0., 0.75, 8., 8.)
                    } else {
                        (0.75, 0., 8., 8.)
                    };
                    let (x, y, size) = crop::part(
                        natural,
                        plan.pixels,
                        (8. * scale, 8. * scale),
                        ObjectFit::Cover,
                    )
                    .unwrap();
                    let part = plan.part(x, y, size).unwrap();
                    let cropped = decode_rows(Cursor::new(&bytes), &part, || false).unwrap();
                    let dst = object_fit(natural, ObjectFit::Cover, box_rect).unwrap();
                    let mut drawn = Vec::new();
                    for (pixels, crop) in [(cropped, part.crop), (whole.clone(), plan.crop)] {
                        let bytes = u64::from(pixels.width()) * u64::from(pixels.height()) * 4;
                        let reservation = session.reserve_allocation(bytes).unwrap();
                        let mut bitmap = Bitmap::new(pixels, natural, reservation.charge());
                        bitmap.crop = crop;
                        let bitmap = Arc::new(bitmap);
                        reservation.commit(bytes).unwrap();
                        let mut raster = Raster::transparent();
                        raster.begin(12., 12., scale);
                        raster.image(
                            &bitmap,
                            bitmap.placed(dst),
                            &[Shape::rect(box_rect)],
                            Transform::identity(),
                            None,
                        );
                        drawn.push(raster.finish().unwrap());
                    }
                    if !transpose && !patterned && scale == 1. {
                        assert_eq!(drawn[0].pixel(0, 0).unwrap().alpha(), 64);
                        assert_eq!(drawn[1].pixel(0, 0).unwrap().alpha(), 64);
                    }
                    assert_eq!(
                        drawn[0].data(),
                        drawn[1].data(),
                        "transpose={transpose} patterned={patterned} scale={scale}"
                    );
                    assert!(drawn[0]
                        .data()
                        .chunks_exact(4)
                        .all(|p| p[..3].iter().all(|c| *c <= p[3])));
                }
            }
        }
    }

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
