//! The Apple codec's edit (LLP 1133.019 D16): the source decoded once at its
//! stored size, drawn once through one affine map (its orientation, then
//! D14's turn, flip, crop and size) into a context of the output's size,
//! and encoded once by ImageIO.
//!
//! `transformImage` keeps what the camera made (D15): the source's colour
//! space where it is an RGB space a bitmap can be drawn in; an HDR source
//! (a gain map, PQ or HLG) decoded with `kCGImageSourceDecodeToHDR`, drawn
//! into extended linear Display P3 at half-float and encoded with
//! `kCGImageDestinationEncodeToISOGainmap` (LLP 1100 D13), where the type
//! carries a gain map (JPEG, HEIC) and the OS has the request (macOS 15, iOS
//! 18); and the metadata `metadata` names. A gain-map attempt that ImageIO
//! writes without one, or fails, is retried with the SDR decode and encode
//! requests, drawn into an SDR space, so the retry cannot classify as HDR.
use super::super::edit::{self, Affine, Geometry, Metadata, ResizeQuality, Transform};
use super::super::{Deadline, Transformed, MAX_SOURCE_PIXELS};
use super::{
    dictionary, integer, number_f64, number_i64, put_bytes, CFDataCreate, CFIndex,
    CFStringCreateWithBytes, CFTypeRef, CGBitmapContextCreate, CGBitmapContextCreateImage,
    CGColorSpaceCreateWithName, CGContextDrawImage, CGContextFillRect,
    CGContextSetInterpolationQuality, CGContextSetRGBFillColor, CGDataConsumerCreate,
    CGImageDestinationAddImage, CGImageDestinationCreateWithDataConsumer,
    CGImageDestinationFinalize, CGImageSourceCopyPropertiesAtIndex, CGImageSourceCreateWithData,
    CGImageSourceGetCount, CGImageSourceGetType, CGRect, ConsumerCallbacks, Encoded, Owned, Pool,
    UTF8,
};
use std::ffi::{c_char, c_void};
use std::ptr;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> u8;
    fn CFRetain(value: CFTypeRef) -> CFTypeRef;
    fn CFDictionaryGetValue(dictionary: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn CFStringGetCString(s: CFTypeRef, buffer: *mut u8, size: CFIndex, encoding: u32) -> u8;
}

#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    static kCGImageSourceShouldCache: CFTypeRef;
    static kCGImageSourceDecodeRequest: CFTypeRef;
    static kCGImageSourceDecodeToHDR: CFTypeRef;
    static kCGImageSourceDecodeToSDR: CFTypeRef;
    static kCGImageDestinationLossyCompressionQuality: CFTypeRef;
    static kCGImageAuxiliaryDataTypeHDRGainMap: CFTypeRef;
    static kCGImagePropertyPixelWidth: CFTypeRef;
    static kCGImagePropertyPixelHeight: CFTypeRef;
    static kCGImagePropertyOrientation: CFTypeRef;
    static kCGImagePropertyDepth: CFTypeRef;
    static kCGImagePropertyHasAlpha: CFTypeRef;
    static kCGImagePropertyExifDictionary: CFTypeRef;
    static kCGImagePropertyTIFFDictionary: CFTypeRef;
    static kCGImagePropertyIPTCDictionary: CFTypeRef;
    static kCGImagePropertyTIFFOrientation: CFTypeRef;
    static kCGImagePropertyExifPixelXDimension: CFTypeRef;
    static kCGImagePropertyExifPixelYDimension: CFTypeRef;
    fn CGImageSourceCreateImageAtIndex(s: CFTypeRef, i: usize, o: CFTypeRef) -> CFTypeRef;
    fn CGImageSourceCopyMetadataAtIndex(s: CFTypeRef, i: usize, o: CFTypeRef) -> CFTypeRef;
    fn CGImageSourceCopyAuxiliaryDataInfoAtIndex(
        s: CFTypeRef,
        i: usize,
        kind: CFTypeRef,
    ) -> CFTypeRef;
    fn CGImageMetadataCreateMutableCopy(metadata: CFTypeRef) -> CFTypeRef;
    fn CGImageMetadataSetValueMatchingImageProperty(
        metadata: CFTypeRef,
        dictionary: CFTypeRef,
        property: CFTypeRef,
        value: CFTypeRef,
    ) -> bool;
    fn CGImageDestinationAddImageAndMetadata(
        destination: CFTypeRef,
        image: CFTypeRef,
        metadata: CFTypeRef,
        options: CFTypeRef,
    );
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    static kCGColorSpaceSRGB: CFTypeRef;
    static kCGColorSpaceDisplayP3: CFTypeRef;
    static kCGColorSpaceExtendedLinearDisplayP3: CFTypeRef;
    fn CGImageGetWidth(image: CFTypeRef) -> usize;
    fn CGImageGetHeight(image: CFTypeRef) -> usize;
    fn CGImageGetColorSpace(image: CFTypeRef) -> CFTypeRef;
    fn CGColorSpaceGetModel(space: CFTypeRef) -> i32;
    fn CGColorSpaceGetName(space: CFTypeRef) -> CFTypeRef;
    fn CGColorSpaceUsesITUR_2100TF(space: CFTypeRef) -> bool;
    fn CGColorSpaceUsesExtendedRange(space: CFTypeRef) -> bool;
    fn CGColorSpaceSupportsOutput(space: CFTypeRef) -> bool;
    fn CGContextConcatCTM(context: CFTypeRef, transform: Affine);
}

extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

/// `RTLD_DEFAULT` on Apple platforms.
const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
const MODEL_RGB: i32 = 1;
const ALPHA_NONE_SKIP_LAST: u32 = 5;
const ALPHA_PREMULTIPLIED_LAST: u32 = 1;
const FLOAT_COMPONENTS: u32 = 1 << 8;
const BYTE_ORDER_16_LITTLE: u32 = 1 << 12;

/// Gain-map requests and headroom APIs, available on macOS 15 and iOS 18
/// (exact2's minimums are 14 and 17), looked up rather than linked.
struct GainMap {
    encode_request: CFTypeRef,
    to_gain_map: CFTypeRef,
    to_sdr: CFTypeRef,
    iso: CFTypeRef,
    headroom: unsafe extern "C" fn(CFTypeRef) -> f32,
    set_headroom: unsafe extern "C" fn(CFTypeRef, f32) -> bool,
}

fn gain_map() -> Option<GainMap> {
    let symbol = |name: &[u8]| {
        // SAFETY: `name` is NUL-terminated; the symbol lives for the process.
        let at = unsafe { dlsym(RTLD_DEFAULT, name.as_ptr().cast()) };
        (!at.is_null()).then_some(at)
    };
    let key = |name: &[u8]| {
        // SAFETY: each requested key is a CFStringRef constant.
        symbol(name).map(|at| unsafe { *(at as *const CFTypeRef) })
    };
    Some(GainMap {
        encode_request: key(b"kCGImageDestinationEncodeRequest\0")?,
        to_gain_map: key(b"kCGImageDestinationEncodeToISOGainmap\0")?,
        to_sdr: key(b"kCGImageDestinationEncodeToSDR\0")?,
        iso: key(b"kCGImageAuxiliaryDataTypeISOGainMap\0")?,
        // SAFETY: these macOS 15 / iOS 18 symbols have the declared signatures.
        headroom: unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn(CFTypeRef) -> f32>(symbol(
                b"CGImageGetContentHeadroom\0",
            )?)
        },
        set_headroom: unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn(CFTypeRef, f32) -> bool>(
                symbol(b"CGContextSetEDRTargetHeadroom\0")?,
            )
        },
    })
}

fn cfstring(text: &str) -> Option<Owned> {
    // SAFETY: the bytes are UTF-8 and copied.
    Owned::new(unsafe {
        CFStringCreateWithBytes(ptr::null(), text.as_ptr(), text.len() as CFIndex, UTF8, 0)
    })
}

/// A source opened for reading: its header read, nothing decoded.
pub(super) struct Opened {
    pub source: Owned,
    pub properties: Owned,
    /// The stored grid's size, before orientation.
    pub stored: (u32, u32),
    pub orientation: i64,
    pub depth: i64,
    pub has_alpha: bool,
    _data: Owned,
}

impl Opened {
    pub fn new(bytes: &[u8], undecodable: &dyn Fn() -> String) -> Result<Self, String> {
        // SAFETY: CFDataCreate copies the bytes.
        let data = Owned::new(unsafe {
            CFDataCreate(ptr::null(), bytes.as_ptr(), bytes.len() as CFIndex)
        })
        .ok_or_else(undecodable)?;
        // The source keeps no decoded copy (A1.5).
        // SAFETY: the key and value are immutable statics.
        let uncached =
            dictionary(unsafe { &[(kCGImageSourceShouldCache, super::kCFBooleanFalse)] })
                .ok_or_else(undecodable)?;
        // SAFETY: `data` and `uncached` are live; the source retains `data`.
        let source = Owned::new(unsafe { CGImageSourceCreateWithData(data.get(), uncached.get()) })
            .ok_or_else(undecodable)?;
        // SAFETY: `source` is live.
        if unsafe {
            CGImageSourceGetType(source.get()).is_null() || CGImageSourceGetCount(source.get()) == 0
        } {
            return Err(undecodable());
        }
        // SAFETY: as above; the copy is +1.
        let properties =
            Owned::new(unsafe { CGImageSourceCopyPropertiesAtIndex(source.get(), 0, ptr::null()) })
                .ok_or_else(undecodable)?;
        // SAFETY: the keys are immutable statics.
        let (width, height, orientation, depth) = unsafe {
            (
                integer(properties.get(), kCGImagePropertyPixelWidth),
                integer(properties.get(), kCGImagePropertyPixelHeight),
                integer(properties.get(), kCGImagePropertyOrientation).unwrap_or(1),
                integer(properties.get(), kCGImagePropertyDepth).unwrap_or(32),
            )
        };
        let (Some(width), Some(height)) = (width, height) else {
            return Err(undecodable());
        };
        if width <= 0 || height <= 0 || width > i64::from(u32::MAX) || height > i64::from(u32::MAX)
        {
            return Err(undecodable());
        }
        // SAFETY: the key is an immutable static; the value is borrowed.
        let has_alpha = unsafe {
            let value = CFDictionaryGetValue(properties.get(), kCGImagePropertyHasAlpha);
            !value.is_null() && CFEqual(value, super::kCFBooleanTrue) != 0
        };
        Ok(Opened {
            source,
            properties,
            has_alpha,
            stored: (width as u32, height as u32),
            orientation,
            depth,
            _data: data,
        })
    }

    /// The upright size: EXIF 5–8 turn the picture a quarter.
    pub fn upright(&self) -> (u32, u32) {
        if (5..=8).contains(&self.orientation) {
            (self.stored.1, self.stored.0)
        } else {
            self.stored
        }
    }

    /// The source's own media type, where this codec writes it.
    fn media_type(&self) -> Option<&'static str> {
        let mut buffer = [0u8; 64];
        // SAFETY: the type is a live CFString of the source; the buffer is ours.
        let ok = unsafe {
            CFStringGetCString(
                CGImageSourceGetType(self.source.get()),
                buffer.as_mut_ptr(),
                64,
                UTF8,
            ) != 0
        };
        let uti = std::str::from_utf8(&buffer[..buffer.iter().position(|&b| b == 0)?]).ok()?;
        match (ok, uti) {
            (true, "public.jpeg") => Some("image/jpeg"),
            (true, "public.png") => Some("image/png"),
            (true, "public.heic" | "public.heif") => Some("image/heic"),
            _ => None,
        }
    }

    /// A gain map of either kind, or a PQ or HLG transfer (LLP 1100 D4).
    fn is_hdr(&self, keys: Option<&GainMap>) -> bool {
        let aux = |kind: CFTypeRef| {
            // SAFETY: `source` is live; the copy is +1 and released at once.
            Owned::new(unsafe {
                CGImageSourceCopyAuxiliaryDataInfoAtIndex(self.source.get(), 0, kind)
            })
            .is_some()
        };
        // SAFETY: the key is an immutable static.
        if aux(unsafe { kCGImageAuxiliaryDataTypeHDRGainMap }) || keys.is_some_and(|k| aux(k.iso)) {
            return true;
        }
        // SAFETY: the image is created lazily, not decoded, and released.
        let image = Owned::new(unsafe {
            CGImageSourceCreateImageAtIndex(self.source.get(), 0, ptr::null())
        });
        image.is_some_and(|i| unsafe {
            let space = CGImageGetColorSpace(i.get());
            !space.is_null() && CGColorSpaceUsesITUR_2100TF(space)
        })
    }
}

/// What a draw writes into.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Surface {
    Eight,
    Sixteen,
    /// Extended range, for an HDR picture.
    Half,
}

impl Surface {
    fn bytes_per_pixel(self) -> u64 {
        match self {
            Surface::Eight => 4,
            Surface::Sixteen | Surface::Half => 8,
        }
    }
}

fn interpolation(quality: ResizeQuality) -> i32 {
    // kCGInterpolationNone, Low, Medium, High.
    match quality {
        ResizeQuality::Pixelated => 1,
        ResizeQuality::Low => 2,
        ResizeQuality::Medium => 4,
        ResizeQuality::High => 3,
    }
}

/// `image` (`iw × ih` pixels) drawn into a new `out` context in `space`,
/// through `to_output`, a map from the image's pixels (y down) to the
/// output's: on white where there is no `alpha`, else on clear.
pub(super) fn render(
    image: CFTypeRef,
    to_output: Affine,
    out: (u32, u32),
    space: CFTypeRef,
    surface: Surface,
    alpha: bool,
    quality: ResizeQuality,
) -> Option<Owned> {
    // SAFETY: `image` is live.
    let (iw, ih) = unsafe {
        (
            CGImageGetWidth(image) as f64,
            CGImageGetHeight(image) as f64,
        )
    };
    let (bits, info) = match surface {
        Surface::Eight => (
            8,
            if alpha {
                ALPHA_PREMULTIPLIED_LAST
            } else {
                ALPHA_NONE_SKIP_LAST
            },
        ),
        Surface::Sixteen => (
            16,
            if alpha {
                ALPHA_PREMULTIPLIED_LAST
            } else {
                ALPHA_NONE_SKIP_LAST
            },
        ),
        Surface::Half => (
            16,
            ALPHA_PREMULTIPLIED_LAST | FLOAT_COMPONENTS | BYTE_ORDER_16_LITTLE,
        ),
    };
    // SAFETY: Core Graphics allocates the backing; `space` is live.
    let context = Owned::new(unsafe {
        CGBitmapContextCreate(
            ptr::null_mut(),
            out.0 as usize,
            out.1 as usize,
            bits,
            0,
            space,
            info,
        )
    })?;
    if surface == Surface::Half {
        let hdr = gain_map()?;
        // A bitmap without headroom metadata encodes as SDR even when its
        // floating-point pixels exceed reference white. Keep the source range.
        // SAFETY: both objects are live and the function signatures match.
        if !unsafe { (hdr.set_headroom)(context.get(), (hdr.headroom)(image)) } {
            return None;
        }
    }
    // Core Graphics draws an image right way up into its rect with y up:
    // the image's own flip, then the map, then the output's flip.
    let ctm = Affine {
        d: -1.0,
        ty: ih,
        ..Affine::IDENTITY
    }
    .then(to_output)
    .then(Affine {
        d: -1.0,
        ty: f64::from(out.1),
        ..Affine::IDENTITY
    });
    let all = CGRect {
        x: 0.0,
        y: 0.0,
        width: f64::from(out.0),
        height: f64::from(out.1),
    };
    // SAFETY: `context` and `image` are live; a new context is clear.
    unsafe {
        if !alpha {
            CGContextSetRGBFillColor(context.get(), 1.0, 1.0, 1.0, 1.0);
            CGContextFillRect(context.get(), all);
        }
        CGContextSetInterpolationQuality(context.get(), interpolation(quality));
        CGContextConcatCTM(context.get(), ctm);
        CGContextDrawImage(
            context.get(),
            CGRect {
                x: 0.0,
                y: 0.0,
                width: iw,
                height: ih,
            },
            image,
        );
        Owned::new(CGBitmapContextCreateImage(context.get()))
    }
}

/// What of the source an encode writes (D15).
enum Written {
    /// The source's metadata object, its orientation reset and size rewritten.
    Kept(Owned),
    /// A properties dictionary of the conservative set.
    Properties(Vec<(Owned, Owned)>),
    Nothing,
}

/// The standard tags that describe the capture, and the IPTC rights fields:
/// `no-location`'s whole set (D15). ImageIO's GPS switch would leave maker
/// notes and XMP, so nothing is subtracted from the source's; this is copied.
const EXIF_KEPT: &[&str] = &[
    "DateTimeOriginal",
    "DateTimeDigitized",
    "OffsetTime",
    "OffsetTimeOriginal",
    "OffsetTimeDigitized",
    "SubsecTimeOriginal",
    "SubsecTimeDigitized",
    "ExposureTime",
    "FNumber",
    "ExposureProgram",
    "ISOSpeedRatings",
    "ExposureBiasValue",
    "ApertureValue",
    "ShutterSpeedValue",
    "BrightnessValue",
    "MeteringMode",
    "Flash",
    "FocalLength",
    "FocalLenIn35mmFilm",
    "LensMake",
    "LensModel",
    "LensSpecification",
    "ColorSpace",
    "WhiteBalance",
    "ExposureMode",
    "SceneCaptureType",
    "DigitalZoomRatio",
];
const TIFF_KEPT: &[&str] = &[
    "Make",
    "Model",
    "Software",
    "DateTime",
    "Artist",
    "Copyright",
];
const IPTC_KEPT: &[&str] = &[
    "CopyrightNotice",
    "Byline",
    "Credit",
    "Source",
    "RightsUsageTerms",
];

fn written(opened: &Opened, metadata: Metadata, out: (u32, u32)) -> Option<Written> {
    match metadata {
        Metadata::None => Some(Written::Nothing),
        Metadata::Keep => {
            // SAFETY: `source` is live; both copies are +1.
            let Some(source) = Owned::new(unsafe {
                CGImageSourceCopyMetadataAtIndex(opened.source.get(), 0, ptr::null())
            }) else {
                return Some(Written::Nothing);
            };
            let kept = Owned::new(unsafe { CGImageMetadataCreateMutableCopy(source.get()) })?;
            let (one, w, h) = (
                number_i64(1)?,
                number_i64(out.0.into())?,
                number_i64(out.1.into())?,
            );
            // SAFETY: the keys are immutable statics; the values live across the calls.
            unsafe {
                for (dictionary, property, value) in [
                    (
                        kCGImagePropertyTIFFDictionary,
                        kCGImagePropertyTIFFOrientation,
                        &one,
                    ),
                    (
                        kCGImagePropertyExifDictionary,
                        kCGImagePropertyExifPixelXDimension,
                        &w,
                    ),
                    (
                        kCGImagePropertyExifDictionary,
                        kCGImagePropertyExifPixelYDimension,
                        &h,
                    ),
                ] {
                    CGImageMetadataSetValueMatchingImageProperty(
                        kept.get(),
                        dictionary,
                        property,
                        value.get(),
                    );
                }
            }
            Some(Written::Kept(kept))
        }
        Metadata::NoLocation => {
            let mut groups = Vec::new();
            // SAFETY: the group keys are immutable statics.
            for (group, keys) in unsafe {
                [
                    (kCGImagePropertyExifDictionary, EXIF_KEPT),
                    (kCGImagePropertyTIFFDictionary, TIFF_KEPT),
                    (kCGImagePropertyIPTCDictionary, IPTC_KEPT),
                ]
            } {
                // SAFETY: `properties` is live; the group is borrowed (Get rule).
                let from = unsafe { CFDictionaryGetValue(opened.properties.get(), group) };
                if from.is_null() {
                    continue;
                }
                let mut pairs = Vec::new();
                for key in keys {
                    let name = cfstring(key)?;
                    // SAFETY: `from` is a live dictionary; the value is retained below.
                    let value = unsafe { CFDictionaryGetValue(from, name.get()) };
                    if !value.is_null() {
                        pairs.push((name, Owned::new(unsafe { CFRetain(value) })?));
                    }
                }
                if pairs.is_empty() {
                    continue;
                }
                let raw: Vec<_> = pairs.iter().map(|(k, v)| (k.get(), v.get())).collect();
                // SAFETY: `group` is a static CFString, retained here to be owned.
                groups.push((Owned::new(unsafe { CFRetain(group) })?, dictionary(&raw)?));
            }
            Some(Written::Properties(groups))
        }
    }
}

/// `image` encoded as `uti` with `options`, and `meta`, keeping at most
/// `cap + 1` bytes: `None` when ImageIO wrote nothing; the flag, when the
/// output ran past `cap`.
fn encode(
    image: CFTypeRef,
    uti: &str,
    options: &[(CFTypeRef, CFTypeRef)],
    meta: &Written,
    cap: usize,
) -> Option<(Vec<u8>, bool)> {
    let limit = cap.checked_add(1)?;
    let mut out = Encoded {
        bytes: Vec::new(),
        limit,
        exceeded: false,
    };
    let callbacks = ConsumerCallbacks {
        put_bytes,
        release: None,
    };
    // SAFETY: `out` and callbacks outlive destination and consumer; ImageIO
    // writes synchronously during add/finalize and never retains this stack.
    let consumer =
        Owned::new(unsafe { CGDataConsumerCreate((&mut out as *mut Encoded).cast(), &callbacks) })?;
    let kind = cfstring(uti)?;
    // SAFETY: the consumer and kind remain live through finalization.
    let destination = Owned::new(unsafe {
        CGImageDestinationCreateWithDataConsumer(consumer.get(), kind.get(), 1, ptr::null())
    })?;
    let mut pairs = options.to_vec();
    if let Written::Properties(groups) = meta {
        pairs.extend(groups.iter().map(|(k, v)| (k.get(), v.get())));
    }
    let props = dictionary(&pairs)?;
    // SAFETY: every reference stays live until the encoder returns.
    let finished = unsafe {
        match meta {
            Written::Kept(m) => CGImageDestinationAddImageAndMetadata(
                destination.get(),
                image,
                m.get(),
                props.get(),
            ),
            _ => CGImageDestinationAddImage(destination.get(), image, props.get()),
        }
        CGImageDestinationFinalize(destination.get())
    };
    drop(destination);
    drop(consumer);
    (finished || out.exceeded).then_some((out.bytes, out.exceeded))
}

/// The word D14's result says for `space`.
fn space_word(space: CFTypeRef) -> &'static str {
    // SAFETY: the names are immutable statics; `space` is live.
    unsafe {
        let name = CGColorSpaceGetName(space);
        if name.is_null() {
            "other"
        } else if CFEqual(name, kCGColorSpaceSRGB) != 0 {
            "srgb"
        } else if CFEqual(name, kCGColorSpaceDisplayP3) != 0
            || CFEqual(name, kCGColorSpaceExtendedLinearDisplayP3) != 0
        {
            "display-p3"
        } else {
            "other"
        }
    }
}

/// The space an SDR draw of `image` writes: its own where it is an RGB space
/// a bitmap draws in and is SDR, Display P3 for a wide or HDR one, else sRGB.
fn sdr_space(image: CFTypeRef) -> Option<Owned> {
    // SAFETY: `image` is live; its space is borrowed and retained to be owned.
    unsafe {
        let own = CGImageGetColorSpace(image);
        if !own.is_null()
            && CGColorSpaceGetModel(own) == MODEL_RGB
            && CGColorSpaceSupportsOutput(own)
            && !CGColorSpaceUsesITUR_2100TF(own)
            && !CGColorSpaceUsesExtendedRange(own)
        {
            return Owned::new(CFRetain(own));
        }
        let wide = !own.is_null() && CGColorSpaceGetModel(own) == MODEL_RGB;
        Owned::new(CGColorSpaceCreateWithName(if wide {
            kCGColorSpaceDisplayP3
        } else {
            kCGColorSpaceSRGB
        }))
    }
}

fn failure(code: &str, detail: impl std::fmt::Display) -> String {
    edit::failure_named("transformImage", code, detail)
}

/// A conservative charge for one attempt's explicitly sized surfaces: the
/// source, its decode (twice, for decoder workspace, as compression charges
/// it), the output and its encoding.
fn charge(
    source_bytes: usize,
    stored: (u32, u32),
    decoded_bpp: u64,
    out: (u32, u32),
    out_bpp: u64,
    max_bytes: u64,
) -> Option<u64> {
    let area =
        |(w, h): (u32, u32), bpp: u64| u64::from(w).checked_mul(bpp)?.checked_mul(u64::from(h));
    let encoded = area(out, out_bpp)?
        .saturating_add(1 << 20)
        .min(max_bytes.saturating_add(1));
    (source_bytes as u64)
        .checked_add(area(stored, decoded_bpp)?.checked_mul(2)?)?
        .checked_add(area(out, out_bpp)?)?
        .checked_add(encoded.checked_mul(2)?)
}

pub(super) fn transform(
    bytes: &[u8],
    t: &Transform,
    max_bytes: u64,
    deadline: Deadline<'_>,
) -> Result<Transformed, String> {
    let _pool = Pool::new();
    deadline.check()?;
    let undecodable = || failure("undecodable", "not an image this host decodes");
    let opened = Opened::new(bytes, &undecodable)?;
    let (sw, sh) = opened.stored;
    if u64::from(sw) * u64::from(sh) > deadline.max_pixels {
        return Err(failure(
            "too-large",
            format!("{sw}×{sh} pixels is over {}", deadline.max_pixels),
        ));
    }
    let (w, h) = opened.upright();
    let geometry = Geometry::new("transformImage", w, h, &t.edit)?;
    let (cw, ch) = geometry.size();
    let out = edit::resized(cw, ch, t.resize_width, t.resize_height);
    if u64::from(out.0) * u64::from(out.1) > MAX_SOURCE_PIXELS {
        return Err(failure(
            "too-large",
            format!("{}×{} pixels is over {MAX_SOURCE_PIXELS}", out.0, out.1),
        ));
    }
    let media_type = match t.media_type.as_deref() {
        Some(t) => t,
        None => opened.media_type().unwrap_or("image/jpeg"),
    };
    // A free turn's corners are clear, as the source's own transparency is;
    // JPEG has no alpha, so either is white there.
    let clear = opened.has_alpha || t.edit.rotate % 90.0 != 0.0;
    let (media_type, uti, lossy, alpha) = match media_type {
        "image/jpeg" => ("image/jpeg", "public.jpeg", true, false),
        "image/png" => ("image/png", "public.png", false, clear),
        "image/heic" => ("image/heic", "public.heic", true, clear),
        other => {
            return Err(failure(
                "unsupported",
                format!("this host writes JPEG, PNG and HEIC, not {other}"),
            ))
        }
    };
    let keys = gain_map();
    let budget = deadline.max_scratch_bytes - deadline.max_scratch_bytes / 4;
    let src_bpp = match opened.depth {
        1..=8 => 4,
        9..=16 => 8,
        _ => 16,
    };
    let fits = |decoded: u64, surface: Surface| {
        charge(
            bytes.len(),
            opened.stored,
            decoded,
            out,
            surface.bytes_per_pixel(),
            max_bytes,
        )
        .is_some_and(|n| n <= budget)
    };
    let sdr_surface = if opened.depth > 8 && media_type == "image/png" {
        Surface::Sixteen
    } else {
        Surface::Eight
    };
    let hdr = keys.is_some() && lossy && opened.is_hdr(keys.as_ref()) && fits(8, Surface::Half);
    if !hdr && !fits(src_bpp, sdr_surface) {
        return Err(failure(
            "too-large",
            "image exceeds its codec memory reservation",
        ));
    }
    let meta = written(&opened, t.metadata, out)
        .ok_or_else(|| failure("unsupported", "ImageIO did not copy the metadata"))?;
    // The stored grid to the output: its orientation, then the edit.
    let to_output = |image: CFTypeRef| {
        // SAFETY: `image` is live.
        let (iw, ih) = unsafe {
            (
                CGImageGetWidth(image) as f64,
                CGImageGetHeight(image) as f64,
            )
        };
        Affine::scale(f64::from(sw) / iw, f64::from(sh) / ih)
            .then(edit::orient(
                opened.orientation,
                f64::from(sw),
                f64::from(sh),
            ))
            .then(geometry.to_output(out))
    };
    let quality = number_f64(t.quality).ok_or_else(undecodable)?;
    let attempt = |hdr: bool| -> Result<Transformed, String> {
        let _pool = Pool::new();
        deadline.check()?;
        // SAFETY: the keys and values are immutable statics.
        let decode = dictionary(unsafe {
            &[
                (kCGImageSourceShouldCache, super::kCFBooleanFalse),
                (
                    kCGImageSourceDecodeRequest,
                    if hdr {
                        kCGImageSourceDecodeToHDR
                    } else {
                        kCGImageSourceDecodeToSDR
                    },
                ),
            ]
        })
        .ok_or_else(undecodable)?;
        // SAFETY: `source` and `decode` are live; the image is +1.
        let image = Owned::new(unsafe {
            CGImageSourceCreateImageAtIndex(opened.source.get(), 0, decode.get())
        })
        .ok_or_else(undecodable)?;
        let space = if hdr {
            // SAFETY: the name is an immutable static.
            Owned::new(unsafe { CGColorSpaceCreateWithName(kCGColorSpaceExtendedLinearDisplayP3) })
        } else {
            sdr_space(image.get())
        }
        .ok_or_else(undecodable)?;
        let surface = if hdr { Surface::Half } else { sdr_surface };
        let drawn = render(
            image.get(),
            to_output(image.get()),
            out,
            space.get(),
            surface,
            alpha,
            t.resize_quality,
        )
        .ok_or_else(undecodable)?;
        drop(image);
        deadline.check()?;
        let mut options: Vec<(CFTypeRef, CFTypeRef)> = Vec::new();
        if lossy {
            // SAFETY: the key is an immutable static.
            options.push((
                unsafe { kCGImageDestinationLossyCompressionQuality },
                quality.get(),
            ));
        }
        if let Some(k) = &keys {
            options.push((k.encode_request, if hdr { k.to_gain_map } else { k.to_sdr }));
        }
        let cap = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        let (encoded, over) = encode(drawn.get(), uti, &options, &meta, cap).ok_or_else(|| {
            failure(
                "unsupported",
                format!("ImageIO did not encode {media_type}"),
            )
        })?;
        if over {
            return Err(failure(
                "too-large",
                format!("the {media_type} is over {max_bytes} bytes"),
            ));
        }
        let gain_map = hdr
            && keys.as_ref().is_some_and(|k| {
                Opened::new(&encoded, &undecodable).is_ok_and(|r| {
                    // SAFETY: `r.source` is live; the copy is +1 and released.
                    Owned::new(unsafe {
                        CGImageSourceCopyAuxiliaryDataInfoAtIndex(r.source.get(), 0, k.iso)
                    })
                    .is_some()
                })
            });
        Ok(Transformed {
            bytes: encoded,
            width: out.0,
            height: out.1,
            media_type,
            color_space: space_word(space.get()),
            hdr: gain_map,
            metadata: t.metadata,
        })
    };
    let mut fallback = None;
    if hdr {
        match attempt(true) {
            Ok(written) if written.hdr => return Ok(written),
            // A gain-map request ImageIO wrote as SDR is kept should the SDR attempt fail.
            Ok(sdr) => fallback = Some(sdr),
            Err(e) if e.contains(": timeout: ") => return Err(e),
            Err(_) => {}
        }
    }
    match attempt(false) {
        Ok(written) => Ok(written),
        Err(e) => fallback.ok_or(e),
    }
}

/// `image`'s pixel size.
pub(super) fn size(image: CFTypeRef) -> (f64, f64) {
    // SAFETY: `image` is live.
    unsafe {
        (
            CGImageGetWidth(image) as f64,
            CGImageGetHeight(image) as f64,
        )
    }
}

#[cfg(test)]
#[path = "apple_transform_tests.rs"]
mod tests;
