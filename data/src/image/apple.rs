//! The Apple codec (LLP 1069.002 A1.3): ImageIO decodes and encodes, Core
//! Graphics scales, all through their C interfaces, so it runs on whichever
//! worker thread the storage operation landed on, with no Swift.
//!
//! Per size, ImageIO's thumbnail decode (orientation applied, subsampled
//! while decoding JPEG and HEIC; other formats may be decoded whole) is
//! drawn into an opaque sRGB `w × h` bitmap, filled white: a second scale,
//! since the thumbnail's own size is ImageIO's rounding, not `floor()`.
//! Each quality trial encodes that bitmap. No source property is copied
//! into the JPEG: no EXIF, GPS, TIFF, IPTC, XMP or orientation tag; the
//! encoder writes its own JFIF, sRGB profile and colour-space EXIF.
use super::{failure, search, Compressed, Deadline, MAX_SOURCE_PIXELS};
use std::ffi::c_void;
use std::ptr::{self, NonNull};

type CFTypeRef = *const c_void;
type CFIndex = isize;
type CGFloat = f64;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    x: CGFloat,
    y: CGFloat,
    width: CGFloat,
    height: CGFloat,
}

#[repr(C)]
struct DictionaryCallBacks {
    _private: [usize; 6],
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFTypeDictionaryKeyCallBacks: DictionaryCallBacks;
    static kCFTypeDictionaryValueCallBacks: DictionaryCallBacks;
    static kCFBooleanTrue: CFTypeRef;
    static kCFBooleanFalse: CFTypeRef;
    fn CFRelease(value: CFTypeRef);
    fn CFDataCreate(allocator: CFTypeRef, bytes: *const u8, length: CFIndex) -> CFTypeRef;
    fn CFDataCreateMutable(allocator: CFTypeRef, capacity: CFIndex) -> CFTypeRef;
    fn CFDataGetLength(data: CFTypeRef) -> CFIndex;
    fn CFDataGetBytePtr(data: CFTypeRef) -> *const u8;
    fn CFDictionaryCreate(
        allocator: CFTypeRef,
        keys: *const CFTypeRef,
        values: *const CFTypeRef,
        count: CFIndex,
        key_callbacks: *const DictionaryCallBacks,
        value_callbacks: *const DictionaryCallBacks,
    ) -> CFTypeRef;
    fn CFDictionaryGetValue(dictionary: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn CFNumberCreate(allocator: CFTypeRef, kind: CFIndex, value: *const c_void) -> CFTypeRef;
    fn CFNumberGetValue(number: CFTypeRef, kind: CFIndex, value: *mut c_void) -> u8;
    fn CFGetTypeID(value: CFTypeRef) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFStringCreateWithBytes(
        allocator: CFTypeRef,
        bytes: *const u8,
        length: CFIndex,
        encoding: u32,
        external: u8,
    ) -> CFTypeRef;
}

#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    static kCGImageSourceCreateThumbnailFromImageAlways: CFTypeRef;
    static kCGImageSourceCreateThumbnailWithTransform: CFTypeRef;
    static kCGImageSourceThumbnailMaxPixelSize: CFTypeRef;
    static kCGImageSourceShouldCache: CFTypeRef;
    static kCGImageDestinationLossyCompressionQuality: CFTypeRef;
    static kCGImagePropertyPixelWidth: CFTypeRef;
    static kCGImagePropertyPixelHeight: CFTypeRef;
    static kCGImagePropertyOrientation: CFTypeRef;
    fn CGImageSourceCreateWithData(data: CFTypeRef, options: CFTypeRef) -> CFTypeRef;
    fn CGImageSourceGetType(source: CFTypeRef) -> CFTypeRef;
    fn CGImageSourceGetCount(source: CFTypeRef) -> usize;
    fn CGImageSourceCopyPropertiesAtIndex(
        source: CFTypeRef,
        index: usize,
        options: CFTypeRef,
    ) -> CFTypeRef;
    fn CGImageSourceCreateThumbnailAtIndex(
        source: CFTypeRef,
        index: usize,
        options: CFTypeRef,
    ) -> CFTypeRef;
    fn CGImageDestinationCreateWithData(
        data: CFTypeRef,
        kind: CFTypeRef,
        count: usize,
        options: CFTypeRef,
    ) -> CFTypeRef;
    fn CGImageDestinationAddImage(destination: CFTypeRef, image: CFTypeRef, props: CFTypeRef);
    fn CGImageDestinationFinalize(destination: CFTypeRef) -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    static kCGColorSpaceSRGB: CFTypeRef;
    fn CGColorSpaceCreateWithName(name: CFTypeRef) -> CFTypeRef;
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: CFTypeRef,
        bitmap_info: u32,
    ) -> CFTypeRef;
    fn CGBitmapContextCreateImage(context: CFTypeRef) -> CFTypeRef;
    fn CGContextSetRGBFillColor(context: CFTypeRef, r: CGFloat, g: CGFloat, b: CGFloat, a: CGFloat);
    fn CGContextFillRect(context: CFTypeRef, rect: CGRect);
    fn CGContextSetInterpolationQuality(context: CFTypeRef, quality: i32);
    fn CGContextDrawImage(context: CFTypeRef, rect: CGRect, image: CFTypeRef);
}

#[link(name = "objc")]
extern "C" {
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

const NUMBER_SINT64: CFIndex = 4;
const NUMBER_DOUBLE: CFIndex = 13;
const UTF8: u32 = 0x0800_0100;
const ALPHA_NONE_SKIP_LAST: u32 = 5;
const INTERPOLATION_HIGH: i32 = 3;

/// An owned Core Foundation reference, released on drop.
struct Owned(NonNull<c_void>);

impl Owned {
    /// Take a +1 reference, or `None` for null.
    fn new(value: CFTypeRef) -> Option<Self> {
        NonNull::new(value as *mut c_void).map(Owned)
    }
    fn get(&self) -> CFTypeRef {
        self.0.as_ptr()
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: a +1 reference this value owns, released once.
        unsafe { CFRelease(self.get()) }
    }
}

fn number_i64(n: i64) -> Option<Owned> {
    // SAFETY: CFNumberCreate copies the value it is pointed at.
    Owned::new(unsafe { CFNumberCreate(ptr::null(), NUMBER_SINT64, (&n as *const i64).cast()) })
}

fn number_f64(n: f64) -> Option<Owned> {
    // SAFETY: as above.
    Owned::new(unsafe { CFNumberCreate(ptr::null(), NUMBER_DOUBLE, (&n as *const f64).cast()) })
}

/// A dictionary of `pairs`; it retains its keys and values.
fn dictionary(pairs: &[(CFTypeRef, CFTypeRef)]) -> Option<Owned> {
    let keys: Vec<_> = pairs.iter().map(|p| p.0).collect();
    let values: Vec<_> = pairs.iter().map(|p| p.1).collect();
    // SAFETY: the arrays hold `pairs.len()` live CF objects; the standard
    // callbacks retain them for the dictionary's lifetime.
    Owned::new(unsafe {
        CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            pairs.len() as CFIndex,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        )
    })
}

/// An integer property of `properties`, if it is a number.
fn integer(properties: CFTypeRef, key: CFTypeRef) -> Option<i64> {
    // SAFETY: `properties` is a live dictionary; the value is borrowed (Get
    // rule) and checked to be a CFNumber before it is read.
    unsafe {
        let value = CFDictionaryGetValue(properties, key);
        if value.is_null() || CFGetTypeID(value) != CFNumberGetTypeID() {
            return None;
        }
        let mut n: i64 = 0;
        (CFNumberGetValue(value, NUMBER_SINT64, (&mut n as *mut i64).cast()) != 0).then_some(n)
    }
}

pub(super) fn compress(
    source: &[u8],
    max_dimension: u32,
    max_bytes: u64,
    deadline: Deadline,
) -> Result<Compressed, String> {
    // ImageIO autoreleases internally; a storage worker lives long, so each
    // call drains its own pool.
    struct Pool(*mut c_void);
    impl Drop for Pool {
        fn drop(&mut self) {
            // SAFETY: the token the matching push returned, popped once on
            // the thread that pushed it.
            unsafe { objc_autoreleasePoolPop(self.0) }
        }
    }
    // SAFETY: pushes a pool on this thread; `Pool` pops it.
    let _pool = Pool(unsafe { objc_autoreleasePoolPush() });
    deadline.check()?;
    let undecodable = || failure("undecodable", "not an image this host decodes");
    // SAFETY: CFDataCreate copies the bytes.
    let data =
        Owned::new(unsafe { CFDataCreate(ptr::null(), source.as_ptr(), source.len() as CFIndex) })
            .ok_or_else(undecodable)?;
    // The source keeps no decoded copy between sizes (A1.5).
    // SAFETY: the key and value are immutable statics.
    let uncached = dictionary(unsafe { &[(kCGImageSourceShouldCache, kCFBooleanFalse)] })
        .ok_or_else(undecodable)?;
    // SAFETY: `data` and `uncached` are live; the source retains `data`.
    let image_source =
        Owned::new(unsafe { CGImageSourceCreateWithData(data.get(), uncached.get()) })
            .ok_or_else(undecodable)?;
    // SAFETY: `image_source` is live.
    if unsafe {
        CGImageSourceGetType(image_source.get()).is_null()
            || CGImageSourceGetCount(image_source.get()) == 0
    } {
        return Err(undecodable());
    }
    // SAFETY: as above; the copy is +1.
    let properties = Owned::new(unsafe {
        CGImageSourceCopyPropertiesAtIndex(image_source.get(), 0, ptr::null())
    })
    .ok_or_else(undecodable)?;
    // SAFETY: the keys are immutable statics.
    let (width, height, orientation) = unsafe {
        (
            integer(properties.get(), kCGImagePropertyPixelWidth),
            integer(properties.get(), kCGImagePropertyPixelHeight),
            integer(properties.get(), kCGImagePropertyOrientation).unwrap_or(1),
        )
    };
    let (Some(width), Some(height)) = (width, height) else {
        return Err(undecodable());
    };
    if width <= 0 || height <= 0 || width > u32::MAX as i64 || height > u32::MAX as i64 {
        return Err(undecodable());
    }
    if (width as u64) * (height as u64) > MAX_SOURCE_PIXELS {
        return Err(failure(
            "too-large",
            format!("{width}×{height} pixels is over {MAX_SOURCE_PIXELS}"),
        ));
    }
    // EXIF 5–8 turn the picture a quarter: the oriented size is transposed.
    let (width, height) = if (5..=8).contains(&orientation) {
        (height as u32, width as u32)
    } else {
        (width as u32, height as u32)
    };
    let mut drawn: Option<(u32, u32, Owned)> = None;
    let found = search(width, height, max_dimension, max_bytes, |w, h, quality| {
        deadline.check()?;
        if drawn.as_ref().is_none_or(|d| (d.0, d.1) != (w, h)) {
            drawn = None; // release the last size's bitmap before decoding the next
            drawn = Some((
                w,
                h,
                draw(image_source.get(), w, h).ok_or_else(undecodable)?,
            ));
        }
        let image = drawn.as_ref().expect("drawn above").2.get();
        encode(image, quality)
            .ok_or_else(|| failure("unsupported", "ImageIO did not encode a JPEG"))
    })?;
    found.ok_or_else(|| {
        failure(
            "unfit",
            format!("no JPEG of this image fits {max_bytes} bytes"),
        )
    })
}

/// The oriented image at exactly `w × h`, on white, as a CGImage.
fn draw(source: CFTypeRef, w: u32, h: u32) -> Option<Owned> {
    let side = number_i64(i64::from(w.max(h)))?;
    // SAFETY: the keys and kCFBooleanTrue are immutable statics; `side` lives
    // until the dictionary has retained it.
    let options = dictionary(unsafe {
        &[
            (kCGImageSourceCreateThumbnailFromImageAlways, kCFBooleanTrue),
            (kCGImageSourceCreateThumbnailWithTransform, kCFBooleanTrue),
            (kCGImageSourceShouldCache, kCFBooleanFalse),
            (kCGImageSourceThumbnailMaxPixelSize, side.get()),
        ]
    })?;
    // SAFETY: `source` and `options` are live; the thumbnail is +1.
    let thumbnail =
        Owned::new(unsafe { CGImageSourceCreateThumbnailAtIndex(source, 0, options.get()) })?;
    let rect = CGRect {
        x: 0.0,
        y: 0.0,
        width: f64::from(w),
        height: f64::from(h),
    };
    // sRGB on every host, as the web's canvas is (A1.3): a Display P3 or
    // HDR source is converted, and a grey or CMYK one too.
    // SAFETY: kCGColorSpaceSRGB is an immutable static; the space is +1.
    let srgb = Owned::new(unsafe { CGColorSpaceCreateWithName(kCGColorSpaceSRGB) })?;
    let context = bitmap(w, h, srgb.get())?;
    // SAFETY: `context` and `thumbnail` are live; drawing scales into `rect`.
    unsafe {
        CGContextSetRGBFillColor(context.get(), 1.0, 1.0, 1.0, 1.0);
        CGContextFillRect(context.get(), rect);
        CGContextSetInterpolationQuality(context.get(), INTERPOLATION_HIGH);
        CGContextDrawImage(context.get(), rect, thumbnail.get());
    }
    drop(thumbnail);
    // SAFETY: `context` is live; the image is +1 and copies on write.
    Owned::new(unsafe { CGBitmapContextCreateImage(context.get()) })
}

/// An opaque 8-bit RGB bitmap context in `space`.
fn bitmap(w: u32, h: u32, space: CFTypeRef) -> Option<Owned> {
    // SAFETY: Core Graphics allocates the backing; `space` is live.
    Owned::new(unsafe {
        CGBitmapContextCreate(
            ptr::null_mut(),
            w as usize,
            h as usize,
            8,
            0,
            space,
            ALPHA_NONE_SKIP_LAST,
        )
    })
}

/// `image` as a JPEG at `quality` percent, with no properties but that one.
fn encode(image: CFTypeRef, quality: u32) -> Option<Vec<u8>> {
    // SAFETY: the buffer is +1 and grows as ImageIO writes.
    let out = Owned::new(unsafe { CFDataCreateMutable(ptr::null(), 0) })?;
    let jpeg = b"public.jpeg";
    // SAFETY: the bytes are UTF-8 and copied.
    let kind = Owned::new(unsafe {
        CFStringCreateWithBytes(ptr::null(), jpeg.as_ptr(), jpeg.len() as CFIndex, UTF8, 0)
    })?;
    // SAFETY: `out` and `kind` are live; the destination is +1.
    let destination = Owned::new(unsafe {
        CGImageDestinationCreateWithData(out.get(), kind.get(), 1, ptr::null())
    })?;
    let quality = number_f64(f64::from(quality) / 100.0)?;
    // SAFETY: the key is an immutable static.
    let props = dictionary(&[(
        unsafe { kCGImageDestinationLossyCompressionQuality },
        quality.get(),
    )])?;
    // SAFETY: every reference is live; finalizing writes into `out`.
    unsafe {
        CGImageDestinationAddImage(destination.get(), image, props.get());
        if !CGImageDestinationFinalize(destination.get()) {
            return None;
        }
    }
    drop(destination);
    // SAFETY: `out` is live; its bytes are copied out before it is released.
    let bytes = unsafe {
        let length = CFDataGetLength(out.get());
        std::slice::from_raw_parts(CFDataGetBytePtr(out.get()), length as usize).to_vec()
    };
    Some(bytes)
}

/// For tests: an image's pixel size, orientation, and whether it has GPS or
/// EXIF dictionaries, as ImageIO reads them back.
#[cfg(test)]
pub(super) fn inspect(bytes: &[u8]) -> Option<Inspected> {
    extern "C" {
        static kCGImagePropertyGPSDictionary: CFTypeRef;
        static kCGImagePropertyExifDictionary: CFTypeRef;
        static kCGImagePropertyExifDateTimeOriginal: CFTypeRef;
        static kCGImagePropertyTIFFDictionary: CFTypeRef;
    }
    // SAFETY: as in `compress`.
    unsafe {
        let data = Owned::new(CFDataCreate(
            ptr::null(),
            bytes.as_ptr(),
            bytes.len() as CFIndex,
        ))?;
        let source = Owned::new(CGImageSourceCreateWithData(data.get(), ptr::null()))?;
        let properties = Owned::new(CGImageSourceCopyPropertiesAtIndex(
            source.get(),
            0,
            ptr::null(),
        ))?;
        let kind = CGImageSourceGetType(source.get());
        Some(Inspected {
            jpeg: !kind.is_null() && {
                extern "C" {
                    fn CFStringGetCString(
                        s: CFTypeRef,
                        buffer: *mut u8,
                        size: CFIndex,
                        encoding: u32,
                    ) -> u8;
                }
                let mut buffer = [0u8; 64];
                CFStringGetCString(kind, buffer.as_mut_ptr(), 64, UTF8) != 0
                    && buffer.starts_with(b"public.jpeg\0")
            },
            width: integer(properties.get(), kCGImagePropertyPixelWidth)? as u32,
            height: integer(properties.get(), kCGImagePropertyPixelHeight)? as u32,
            orientation: integer(properties.get(), kCGImagePropertyOrientation),
            gps: !CFDictionaryGetValue(properties.get(), kCGImagePropertyGPSDictionary).is_null(),
            exif_date: {
                let exif = CFDictionaryGetValue(properties.get(), kCGImagePropertyExifDictionary);
                !exif.is_null()
                    && !CFDictionaryGetValue(exif, kCGImagePropertyExifDateTimeOriginal).is_null()
            },
            tiff: !CFDictionaryGetValue(properties.get(), kCGImagePropertyTIFFDictionary).is_null(),
        })
    }
}

#[cfg(test)]
#[derive(Debug)]
pub(super) struct Inspected {
    pub jpeg: bool,
    pub width: u32,
    pub height: u32,
    pub orientation: Option<i64>,
    pub gps: bool,
    /// The capture date: the EXIF a source carries (ImageIO writes its own
    /// colour-space and size EXIF into any JPEG; that is not the source's).
    pub exif_date: bool,
    pub tiff: bool,
}

/// For tests: `rgba` (`w × h`, straight alpha) encoded by ImageIO as a PNG.
#[cfg(test)]
pub(super) fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    extern "C" {
        fn CGDataProviderCreateWithData(
            info: *mut c_void,
            data: *const c_void,
            size: usize,
            release: *const c_void,
        ) -> CFTypeRef;
        fn CGImageCreate(
            w: usize,
            h: usize,
            bpc: usize,
            bpp: usize,
            row: usize,
            space: CFTypeRef,
            info: u32,
            provider: CFTypeRef,
            decode: *const CGFloat,
            interpolate: bool,
            intent: i32,
        ) -> CFTypeRef;
    }
    const ALPHA_LAST: u32 = 3; // kCGImageAlphaLast: straight alpha
                               // SAFETY: the provider borrows `rgba`, which outlives the encode below.
    unsafe {
        let provider = Owned::new(CGDataProviderCreateWithData(
            ptr::null_mut(),
            rgba.as_ptr().cast(),
            rgba.len(),
            ptr::null(),
        ))
        .unwrap();
        let space = Owned::new(CGColorSpaceCreateWithName(kCGColorSpaceSRGB)).unwrap();
        let image = Owned::new(CGImageCreate(
            w as usize,
            h as usize,
            8,
            32,
            w as usize * 4,
            space.get(),
            ALPHA_LAST,
            provider.get(),
            ptr::null(),
            false,
            0,
        ))
        .unwrap();
        let out = Owned::new(CFDataCreateMutable(ptr::null(), 0)).unwrap();
        let png = b"public.png";
        let kind = Owned::new(CFStringCreateWithBytes(
            ptr::null(),
            png.as_ptr(),
            png.len() as CFIndex,
            UTF8,
            0,
        ))
        .unwrap();
        let destination = Owned::new(CGImageDestinationCreateWithData(
            out.get(),
            kind.get(),
            1,
            ptr::null(),
        ))
        .unwrap();
        CGImageDestinationAddImage(destination.get(), image.get(), ptr::null());
        assert!(CGImageDestinationFinalize(destination.get()));
        drop(destination);
        std::slice::from_raw_parts(
            CFDataGetBytePtr(out.get()),
            CFDataGetLength(out.get()) as usize,
        )
        .to_vec()
    }
}

/// For tests: the first pixel of a JPEG, decoded, as RGB.
#[cfg(test)]
pub(super) fn first_pixel(bytes: &[u8]) -> [u8; 3] {
    // SAFETY: as in `compress`; the context's backing is ours.
    unsafe {
        let data = Owned::new(CFDataCreate(
            ptr::null(),
            bytes.as_ptr(),
            bytes.len() as CFIndex,
        ))
        .unwrap();
        let source = Owned::new(CGImageSourceCreateWithData(data.get(), ptr::null())).unwrap();
        extern "C" {
            fn CGImageSourceCreateImageAtIndex(s: CFTypeRef, i: usize, o: CFTypeRef) -> CFTypeRef;
            fn CGImageGetWidth(image: CFTypeRef) -> usize;
            fn CGImageGetHeight(image: CFTypeRef) -> usize;
        }
        let image = Owned::new(CGImageSourceCreateImageAtIndex(
            source.get(),
            0,
            ptr::null(),
        ))
        .unwrap();
        let mut pixel = [0u8; 4];
        let space = Owned::new(CGColorSpaceCreateWithName(kCGColorSpaceSRGB)).unwrap();
        let context = Owned::new(CGBitmapContextCreate(
            pixel.as_mut_ptr().cast(),
            1,
            1,
            8,
            4,
            space.get(),
            ALPHA_NONE_SKIP_LAST,
        ))
        .unwrap();
        // Draw the whole image into one pixel is an average; draw it at its
        // own size offset so that only its top-left pixel lands in the 1×1.
        let (w, h) = (
            CGImageGetWidth(image.get()) as f64,
            CGImageGetHeight(image.get()) as f64,
        );
        CGContextDrawImage(
            context.get(),
            CGRect {
                x: 0.0,
                y: 1.0 - h,
                width: w,
                height: h,
            },
            image.get(),
        );
        [pixel[0], pixel[1], pixel[2]]
    }
}
