//! ImageIO thumbnails into the painter's premultiplied sRGB pixels. @ref LLP 1149 D2.6.
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use std::{ffi::c_void, ptr::NonNull};

type Ref = *const c_void;
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(object: Ref);
    fn CFDataCreate(allocator: Ref, bytes: *const u8, length: isize) -> Ref;
    fn CFNumberCreate(allocator: Ref, kind: isize, value: Ref) -> Ref;
    fn CFDictionaryCreate(
        allocator: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        key_callbacks: Ref,
        value_callbacks: Ref,
    ) -> Ref;
    static kCFBooleanTrue: Ref;
    static kCFBooleanFalse: Ref;
}
#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    fn CGImageSourceCreateWithData(data: Ref, options: Ref) -> Ref;
    fn CGImageSourceCreateThumbnailAtIndex(source: Ref, index: usize, options: Ref) -> Ref;
    static kCGImageSourceShouldCache: Ref;
    static kCGImageSourceShouldCacheImmediately: Ref;
    static kCGImageSourceCreateThumbnailFromImageAlways: Ref;
    static kCGImageSourceCreateThumbnailWithTransform: Ref;
    static kCGImageSourceThumbnailMaxPixelSize: Ref;
}
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCreateWithName(name: Ref) -> Ref;
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits: usize,
        stride: usize,
        color: Ref,
        bitmap_info: u32,
    ) -> Ref;
    fn CGContextSetInterpolationQuality(context: Ref, quality: i32);
    fn CGContextDrawImage(context: Ref, rect: CGRect, image: Ref);
    static kCGColorSpaceSRGB: Ref;
}
struct Owned(NonNull<c_void>);
impl Owned {
    fn new(raw: Ref) -> Option<Self> {
        NonNull::new(raw.cast_mut()).map(Self)
    }
    fn raw(&self) -> Ref {
        self.0.as_ptr()
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.raw());
        }
    }
}

/// Decode frame zero, applying its profile and EXIF orientation, then sample
/// into a crop of `full`. The caller reserves encoded bytes, decoder backing,
/// thumbnail and output storage before calling (LLP 1100 D7).
pub fn decode(bytes: &[u8], full: [u32; 2], crop: [u32; 4]) -> Option<tiny_skia::Pixmap> {
    objc2::rc::autoreleasepool(|_| decode_inner(bytes, full, crop))
}

fn decode_inner(bytes: &[u8], full: [u32; 2], crop: [u32; 4]) -> Option<tiny_skia::Pixmap> {
    let [x, y, width, height] = crop;
    if full.contains(&0)
        || width == 0
        || height == 0
        || x.checked_add(width)? > full[0]
        || y.checked_add(height)? > full[1]
    {
        return None;
    }
    let mut output = tiny_skia::Pixmap::new(width, height)?;
    let maximum = i64::from(full[0].max(full[1]));
    // SAFETY: all Create results are owned and released in reverse order. The
    // nonretaining dictionaries borrow keys/values alive through each call.
    // Core Graphics writes only the checked pixmap extent, as premultiplied RGBA.
    unsafe {
        let nil = std::ptr::null();
        let data = Owned::new(CFDataCreate(
            nil,
            bytes.as_ptr(),
            bytes.len().try_into().ok()?,
        ))?;
        let source_options = Owned::new(CFDictionaryCreate(
            nil,
            [kCGImageSourceShouldCache].as_ptr(),
            [kCFBooleanFalse].as_ptr(),
            1,
            nil,
            nil,
        ))?;
        let source = Owned::new(CGImageSourceCreateWithData(
            data.raw(),
            source_options.raw(),
        ))?;
        let size = Owned::new(CFNumberCreate(nil, 4, (&maximum as *const i64).cast()))?;
        let keys = [
            kCGImageSourceCreateThumbnailFromImageAlways,
            kCGImageSourceCreateThumbnailWithTransform,
            kCGImageSourceShouldCacheImmediately,
            kCGImageSourceThumbnailMaxPixelSize,
        ];
        let values = [kCFBooleanTrue, kCFBooleanTrue, kCFBooleanTrue, size.raw()];
        let options = Owned::new(CFDictionaryCreate(
            nil,
            keys.as_ptr(),
            values.as_ptr(),
            4,
            nil,
            nil,
        ))?;
        let image = Owned::new(CGImageSourceCreateThumbnailAtIndex(
            source.raw(),
            0,
            options.raw(),
        ))?;
        let color = Owned::new(CGColorSpaceCreateWithName(kCGColorSpaceSRGB))?;
        // kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big.
        let context = Owned::new(CGBitmapContextCreate(
            output.data_mut().as_mut_ptr().cast(),
            width as usize,
            height as usize,
            8,
            width as usize * 4,
            color.raw(),
            1 | (4 << 12),
        ))?;
        CGContextSetInterpolationQuality(context.raw(), 3); // kCGInterpolationHigh
        CGContextDrawImage(
            context.raw(),
            CGRect {
                origin: CGPoint::new(-(x as f64), f64::from(y + height) - f64::from(full[1])),
                size: CGSize::new(full[0] as f64, full[1] as f64),
            },
            image.raw(),
        );
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imageio_keeps_rows_crop_channels_and_premultiplied_alpha() {
        let mut source = tiny_skia::Pixmap::new(4, 4).unwrap();
        for (i, pixel) in source.data_mut().chunks_exact_mut(4).enumerate() {
            pixel.copy_from_slice(&[i as u8 * 8, 32, 64, 128]);
        }
        let bytes = source.encode_png().unwrap();
        let whole = decode(&bytes, [4, 4], [0, 0, 4, 4]).unwrap();
        assert_eq!(whole.data(), source.data());
        let cut = decode(&bytes, [4, 4], [1, 0, 2, 2]).unwrap();
        assert_eq!(
            cut.data(),
            [&source.data()[4..12], &source.data()[20..28]].concat()
        );
        assert!(decode(&bytes, [4, 4], [3, 3, 2, 2]).is_none());
        assert!(decode(b"not an image", [4, 4], [0, 0, 4, 4]).is_none());
        let jpeg = include_bytes!("../../../apple/tests/ExactKitTests/ColorFixtures/adobe-rgb.jpg");
        let pixels = decode(jpeg, [12, 8], [0, 0, 12, 8]).unwrap();
        assert!(pixels.data().chunks_exact(4).all(|pixel| pixel[3] == 255));
        assert!(pixels
            .data()
            .chunks_exact(4)
            .any(|pixel| pixel[..3] != [0; 3]));
    }
}
