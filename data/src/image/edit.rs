//! What `transformImage` and `compressImage`'s `rotate`, `flip` and `crop`
//! do to the picture (LLP 1133.019 D14, D15), as pure functions every codec
//! shares: the options read from their JSON, and the one order: the source
//! upright, turned about its centre, flipped in the turned frame, cropped in
//! that frame's pixels, then sized.
use super::failure;
use serde_json::Value;

/// The longest side an output may have: a free turn of the largest source
/// (64 Mi pixels, A1.5) is its bounding box, which the pixel bound refuses
/// before this.
pub const MAX_SIDE: u32 = 16_384;

/// `flip`, in the turned frame: `horizontal` mirrors what the person sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flip {
    Horizontal,
    Vertical,
}

/// A `DOMRect`'s `{x, y, width, height}`, in the turned frame's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The edit both calls share.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edit {
    /// Degrees clockwise, in `[0, 360)`.
    pub rotate: f64,
    pub flip: Option<Flip>,
    pub crop: Option<Rect>,
}

impl Edit {
    /// No turn, flip or crop: the picture as it was.
    pub fn is_identity(&self) -> bool {
        self.rotate == 0.0 && self.flip.is_none() && self.crop.is_none()
    }
}

/// `createImageBitmap`'s `resizeQuality`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResizeQuality {
    Pixelated,
    Low,
    Medium,
    /// Unsaid, the quality a hand-built editor draws with.
    #[default]
    High,
}

/// What an edit keeps of the source's metadata (D15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Metadata {
    /// The source's EXIF, TIFF, GPS, IPTC, XMP and maker notes, with the
    /// orientation reset and the size rewritten.
    #[default]
    Keep,
    /// A conservative set: the standard tags that describe the capture and
    /// the IPTC rights fields; no GPS, maker note or XMP.
    NoLocation,
    None,
}

impl Metadata {
    pub fn word(self) -> &'static str {
        match self {
            Metadata::Keep => "keep",
            Metadata::NoLocation => "no-location",
            Metadata::None => "none",
        }
    }
}

/// `transformImage`'s options, as D14 names them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transform {
    pub edit: Edit,
    pub resize_width: Option<u32>,
    pub resize_height: Option<u32>,
    pub resize_quality: ResizeQuality,
    /// `type`; `None` is the source's where the host encodes it, else JPEG.
    pub media_type: Option<String>,
    /// From 0 to 1; 0.9 unsaid, the picker's conversion.
    pub quality: f64,
    pub metadata: Metadata,
}

fn invalid(call: &str, detail: impl std::fmt::Display) -> String {
    failure_named(call, "invalid", detail)
}

/// A failure of `call` (`compressImage` or `transformImage`).
pub fn failure_named(call: &str, code: &str, detail: impl std::fmt::Display) -> String {
    if call == "compressImage" {
        failure(code, detail)
    } else {
        format!("{call}: {code}: {detail}")
    }
}

/// `rotate`, `flip` and `crop` from `options`, as both calls take them.
pub fn parse_edit(call: &str, options: &Value) -> Result<Edit, String> {
    let rotate = match &options["rotate"] {
        Value::Null => 0.0,
        v => v
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| invalid(call, "rotate must be a finite number of degrees"))?
            .rem_euclid(360.0),
    };
    let flip = match &options["flip"] {
        Value::Null => None,
        Value::String(s) if s == "horizontal" => Some(Flip::Horizontal),
        Value::String(s) if s == "vertical" => Some(Flip::Vertical),
        _ => return Err(invalid(call, "flip must be \"horizontal\" or \"vertical\"")),
    };
    let crop = match &options["crop"] {
        Value::Null => None,
        Value::Object(_) => {
            let side = |k: &str| options["crop"][k].as_f64().filter(|n| n.is_finite());
            let (Some(x), Some(y), Some(width), Some(height)) =
                (side("x"), side("y"), side("width"), side("height"))
            else {
                return Err(invalid(
                    call,
                    "crop is {x, y, width, height}, each a finite number",
                ));
            };
            if width <= 0.0 || height <= 0.0 {
                return Err(invalid(
                    call,
                    "crop's width and height must be greater than 0",
                ));
            }
            Some(Rect {
                x,
                y,
                width,
                height,
            })
        }
        _ => return Err(invalid(call, "crop is {x, y, width, height}")),
    };
    Ok(Edit { rotate, flip, crop })
}

/// `transformImage`'s options from their JSON record.
pub fn parse_transform(options: &Value) -> Result<Transform, String> {
    const CALL: &str = "transformImage";
    if !(options.is_object() || options.is_null()) {
        return Err(invalid(CALL, "options must be an object"));
    }
    let side = |k: &str| match &options[k] {
        Value::Null => Ok(None),
        v => v
            .as_u64()
            .filter(|n| (1..=u64::from(MAX_SIDE)).contains(n))
            .map(|n| Some(n as u32))
            .ok_or_else(|| invalid(CALL, format!("{k} must be an integer from 1 to {MAX_SIDE}"))),
    };
    let resize_quality = match options["resizeQuality"].as_str() {
        None if options["resizeQuality"].is_null() => ResizeQuality::default(),
        Some("pixelated") => ResizeQuality::Pixelated,
        Some("low") => ResizeQuality::Low,
        Some("medium") => ResizeQuality::Medium,
        Some("high") => ResizeQuality::High,
        _ => {
            return Err(invalid(
                CALL,
                "resizeQuality must be \"pixelated\", \"low\", \"medium\" or \"high\"",
            ))
        }
    };
    let media_type = match &options["type"] {
        Value::Null => None,
        Value::String(t) => Some(t.clone()),
        _ => return Err(invalid(CALL, "type must be a media type")),
    };
    let quality = match &options["quality"] {
        Value::Null => 0.9,
        v => v
            .as_f64()
            .filter(|q| (0.0..=1.0).contains(q))
            .ok_or_else(|| invalid(CALL, "quality must be a number from 0 to 1"))?,
    };
    let metadata = match options["metadata"].as_str() {
        None if options["metadata"].is_null() => Metadata::Keep,
        Some("keep") => Metadata::Keep,
        Some("no-location") => Metadata::NoLocation,
        Some("none") => Metadata::None,
        _ => {
            return Err(invalid(
                CALL,
                "metadata must be \"keep\", \"no-location\" or \"none\"",
            ))
        }
    };
    Ok(Transform {
        edit: parse_edit(CALL, options)?,
        resize_width: side("resizeWidth")?,
        resize_height: side("resizeHeight")?,
        resize_quality,
        media_type,
        quality,
        metadata,
    })
}

/// A 2-D affine map, as Core Graphics and the canvas take one:
/// `x' = a x + c y + tx`, `y' = b x + d y + ty`. Laid out as `CGAffineTransform`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };
    pub fn translate(tx: f64, ty: f64) -> Self {
        Affine {
            tx,
            ty,
            ..Self::IDENTITY
        }
    }
    pub fn scale(sx: f64, sy: f64) -> Self {
        Affine {
            a: sx,
            d: sy,
            ..Self::IDENTITY
        }
    }
    /// `self`, then `next`.
    pub fn then(self, next: Affine) -> Affine {
        Affine {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            tx: next.a * self.tx + next.c * self.ty + next.tx,
            ty: next.b * self.tx + next.d * self.ty + next.ty,
        }
    }
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.tx,
            self.b * x + self.d * y + self.ty,
        )
    }
}

/// The map from a stored picture's pixels to its upright ones (y down),
/// for EXIF orientation `orientation` of a `width × height` stored grid.
pub fn orient(orientation: i64, width: f64, height: f64) -> Affine {
    let m = |a, b, c, d, tx, ty| Affine { a, b, c, d, tx, ty };
    match orientation {
        2 => m(-1.0, 0.0, 0.0, 1.0, width, 0.0),
        3 => m(-1.0, 0.0, 0.0, -1.0, width, height),
        4 => m(1.0, 0.0, 0.0, -1.0, 0.0, height),
        5 => m(0.0, 1.0, 1.0, 0.0, 0.0, 0.0),
        6 => m(0.0, 1.0, -1.0, 0.0, height, 0.0),
        7 => m(0.0, -1.0, -1.0, 0.0, height, width),
        8 => m(0.0, -1.0, 1.0, 0.0, 0.0, width),
        _ => Affine::IDENTITY,
    }
}

/// Where an edit of a `width × height` upright picture lands: the turned
/// frame, the crop in it, and the map from upright pixels to the crop's
/// own (y down), before any size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    /// The turned frame's size: the picture's bounding box.
    pub turned: (u32, u32),
    /// The crop, clipped and rounded: `x`, `y`, `width`, `height`.
    pub crop: (u32, u32, u32, u32),
    /// Upright pixels to the crop's.
    pub map: Affine,
}

/// The sine and cosine of `degrees`, exact for a quarter turn.
fn turn(degrees: f64) -> (f64, f64) {
    match degrees {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        180.0 => (0.0, -1.0),
        270.0 => (-1.0, 0.0),
        d => d.to_radians().sin_cos(),
    }
}

impl Geometry {
    /// `edit` of a `width × height` upright picture. A crop that misses the
    /// turned frame fails `outside`, named for `call`.
    pub fn new(call: &str, width: u32, height: u32, edit: &Edit) -> Result<Self, String> {
        let (w, h) = (f64::from(width), f64::from(height));
        let (sin, cos) = turn(edit.rotate);
        // The bounding box; a hair under a whole pixel is that pixel, so a
        // turn's rounding error adds no row of nothing.
        let side = |n: f64| ((n - 1e-6).ceil().max(1.0)) as u32;
        let turned = (
            side(w * cos.abs() + h * sin.abs()),
            side(w * sin.abs() + h * cos.abs()),
        );
        let (tw, th) = (f64::from(turned.0), f64::from(turned.1));
        // About the centre, clockwise with y down.
        let mut map = Affine::translate(-w / 2.0, -h / 2.0)
            .then(Affine {
                a: cos,
                b: sin,
                c: -sin,
                d: cos,
                tx: 0.0,
                ty: 0.0,
            })
            .then(Affine::translate(tw / 2.0, th / 2.0));
        map = match edit.flip {
            Some(Flip::Horizontal) => map.then(Affine {
                a: -1.0,
                tx: tw,
                ..Affine::IDENTITY
            }),
            Some(Flip::Vertical) => map.then(Affine {
                d: -1.0,
                ty: th,
                ..Affine::IDENTITY
            }),
            None => map,
        };
        let crop = match edit.crop {
            None => (0, 0, turned.0, turned.1),
            Some(r) => clip(call, r, turned)?,
        };
        let map = map.then(Affine::translate(-f64::from(crop.0), -f64::from(crop.1)));
        Ok(Geometry { turned, crop, map })
    }

    /// The crop's size.
    pub fn size(&self) -> (u32, u32) {
        (self.crop.2, self.crop.3)
    }

    /// Upright pixels to an output of `out` pixels, the crop scaled to fill it.
    pub fn to_output(&self, out: (u32, u32)) -> Affine {
        self.map.then(Affine::scale(
            f64::from(out.0) / f64::from(self.crop.2),
            f64::from(out.1) / f64::from(self.crop.3),
        ))
    }
}

/// LLP 1114.003 D2: clipped to the frame as `drawImage` clips a source
/// rectangle, each edge rounded to a whole pixel, at least one each way.
fn clip(call: &str, r: Rect, (tw, th): (u32, u32)) -> Result<(u32, u32, u32, u32), String> {
    let (fw, fh) = (f64::from(tw), f64::from(th));
    if r.x >= fw || r.y >= fh || r.x + r.width <= 0.0 || r.y + r.height <= 0.0 {
        return Err(failure_named(
            call,
            "outside",
            format!("the crop misses the {tw}×{th} image"),
        ));
    }
    let edges = |at: f64, len: f64, max: u32| {
        let lo = at.max(0.0).round() as u32;
        let hi = ((at + len).min(f64::from(max)).round() as u32).min(max);
        let lo = lo.min(max - 1);
        (lo, hi.max(lo + 1) - lo)
    };
    let (x, width) = edges(r.x, r.width, tw);
    let (y, height) = edges(r.y, r.height, th);
    Ok((x, y, width, height))
}

/// The output size for a `width × height` crop: `createImageBitmap`'s rule
/// (one side keeps the aspect, rounded up; both set the size; neither keeps
/// the crop's).
pub fn resized(width: u32, height: u32, rw: Option<u32>, rh: Option<u32>) -> (u32, u32) {
    let ratio = |n: u32, num: u32, den: u32| {
        ((f64::from(n) * f64::from(num) / f64::from(den)).ceil() as u32).clamp(1, MAX_SIDE)
    };
    match (rw, rh) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, ratio(height, w, width)),
        (None, Some(h)) => (ratio(width, h, height), h),
        (None, None) => (width, height),
    }
}

/// The media types an edit writes, with the extension a `to` without one
/// gains; `None` for one this table does not know.
pub fn extension(media_type: &str) -> Option<&'static str> {
    match media_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/heic" => Some("heic"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
