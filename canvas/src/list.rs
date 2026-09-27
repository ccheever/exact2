//! The recorded list (LLP 1056 D3, D7): what crosses from the executor to
//! the replayer, on every host the same bytes.
//!
//! Little-endian and 8-byte aligned. A header, then records:
//!
//! ```text
//! header:  u32 MAGIC ("EC2D")  u32 VERSION
//! record:  u32 op  u32 n  then n × f64 operands
//! ```
//!
//! Geometry arrives resolved: path points are in canvas coordinates (CSS px
//! of the content box, or bitmap px for an explicit bitmap), already
//! transformed by the author matrix current when each was added, and arcs,
//! ellipses and rounded corners are cubic segments. Paints, clips and the
//! three rectangle operations run under [`Op::SetTransform`]'s matrix. Colours
//! are resolved: four operands, sRGB 0–255 and alpha 0–1, non-premultiplied.
//!
//! A reader in each host decodes this table by number, so an opcode is never
//! renumbered: a new one takes the next free number and bumps [`VERSION`]
//! only when an existing record changes meaning.

/// The first four bytes, `EC2D` read as a little-endian u32.
pub const MAGIC: u32 = u32::from_le_bytes(*b"EC2D");
/// The format's version.
pub const VERSION: u32 = 1;
/// Bytes before the first record.
pub const HEADER: usize = 8;
/// A list is sealed at this size and the draw continues in the next, applied
/// in order (LLP 1056 D4, as Chrome flushes its recording).
pub const SEAL_BYTES: usize = 1 << 20;

macro_rules! ops {
    ($($(#[$doc:meta])* $name:ident = $code:literal, $arity:expr;)*) => {
        /// An operation. The operand count is fixed except where it is `None`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u32)]
        pub enum Op { $($(#[$doc])* $name = $code,)* }

        impl Op {
            /// The operation for a code, if it is one.
            pub fn from_code(code: u32) -> Option<Op> {
                match code { $($code => Some(Op::$name),)* _ => None }
            }
            /// Its fixed operand count, or `None` when variable.
            pub fn arity(self) -> Option<usize> {
                match self { $(Op::$name => $arity,)* }
            }
            /// The web name it records, for `layout`'s readable form.
            pub fn name(self) -> &'static str {
                match self { $(Op::$name => stringify!($name),)* }
            }
        }
    };
}

ops! {
    /// `save()`.
    Save = 1, Some(0);
    /// `restore()`; an unmatched one does nothing.
    Restore = 2, Some(0);
    /// `reset()`: the bitmap cleared, the state and path at their defaults.
    Reset = 3, Some(0);
    /// The author matrix `a b c d e f`, replacing the last.
    SetTransform = 4, Some(6);
    /// `fillStyle` as a colour: `r g b a`.
    FillColor = 10, Some(4);
    /// `fillStyle` as gradient `id`.
    FillGradient = 11, Some(1);
    /// `strokeStyle` as a colour: `r g b a`.
    StrokeColor = 12, Some(4);
    /// `strokeStyle` as gradient `id`.
    StrokeGradient = 13, Some(1);
    /// `lineWidth`.
    LineWidth = 14, Some(1);
    /// `lineCap`: 0 butt, 1 round, 2 square.
    LineCap = 15, Some(1);
    /// `lineJoin`: 0 miter, 1 round, 2 bevel.
    LineJoin = 16, Some(1);
    /// `miterLimit`.
    MiterLimit = 17, Some(1);
    /// `setLineDash`: the list, already doubled when odd.
    LineDash = 18, None;
    /// `lineDashOffset`.
    LineDashOffset = 19, Some(1);
    /// `globalAlpha`.
    GlobalAlpha = 20, Some(1);
    /// `globalCompositeOperation`, by [`crate::COMPOSITE`]'s index.
    Composite = 21, Some(1);
    /// `shadowColor`: `r g b a`.
    ShadowColor = 22, Some(4);
    /// `shadowBlur`, in canvas coordinate units (never the author matrix's).
    ShadowBlur = 23, Some(1);
    /// `shadowOffsetX shadowOffsetY`, likewise.
    ShadowOffset = 24, Some(2);
    /// `imageSmoothingEnabled` (0 or 1) and `imageSmoothingQuality` (0 low,
    /// 1 medium, 2 high).
    ImageSmoothing = 25, Some(2);
    /// `createLinearGradient`: `id x0 y0 x1 y1`.
    LinearGradient = 30, Some(5);
    /// `createRadialGradient`: `id x0 y0 r0 x1 y1 r1`.
    RadialGradient = 31, Some(7);
    /// `addColorStop`: `id offset r g b a`.
    ColorStop = 32, Some(6);
    /// `createConicGradient`: `id startAngle x y`.
    ConicGradient = 33, Some(4);
    /// `createPattern`: `id image repetition` (0 repeat, 1 repeat-x,
    /// 2 repeat-y, 3 no-repeat) over image `image` ([`Op::Image`]).
    Pattern = 34, Some(3);
    /// `CanvasPattern.setTransform`: `id a b c d e f`.
    PatternTransform = 35, Some(7);
    /// `fillStyle` as pattern `id`.
    FillPattern = 36, Some(1);
    /// `strokeStyle` as pattern `id`.
    StrokePattern = 37, Some(1);
    /// `beginPath()`.
    BeginPath = 40, Some(0);
    /// A new subpath at `x y`.
    MoveTo = 41, Some(2);
    /// A line to `x y`.
    LineTo = 42, Some(2);
    /// A quadratic to `x y` through `cx cy`.
    QuadTo = 43, Some(4);
    /// A cubic to `x y` through `c1x c1y c2x c2y`.
    CubicTo = 44, Some(6);
    /// `closePath()`.
    ClosePath = 45, Some(0);
    /// `fill(rule)`: 0 nonzero, 1 evenodd.
    Fill = 50, Some(1);
    /// `stroke()`.
    Stroke = 51, Some(0);
    /// `clip(rule)`: 0 nonzero, 1 evenodd.
    Clip = 52, Some(1);
    /// `fillRect(x, y, w, h)` under the author matrix.
    FillRect = 53, Some(4);
    /// `strokeRect(x, y, w, h)` under the author matrix.
    StrokeRect = 54, Some(4);
    /// `clearRect(x, y, w, h)` under the author matrix and the clip.
    ClearRect = 55, Some(4);
    /// The text style: `size weight style stretch caps kerning rendering
    /// letterSpacing wordSpacing`, then the family list (names joined by
    /// `,`) as Unicode scalar values. Sizes and spacings in canvas units;
    /// style 0 normal, 1 italic, 2 oblique; stretch a percentage; caps by
    /// [`crate::font::CAPS`], kerning by [`crate::font::KERNING`], rendering
    /// by [`crate::font::RENDERING`].
    Font = 60, None;
    /// `fillText`: `x y scaleX rtl`, then the text as Unicode scalar values.
    /// `x y` is the run's left end on its alphabetic baseline in user space
    /// (alignment and `textBaseline` already resolved); `scaleX` is
    /// `maxWidth`'s squeeze, about that point.
    FillText = 61, None;
    /// `strokeText`, as [`Op::FillText`].
    StrokeText = 62, None;
    /// An image handle for this generation: `id`, then its source (the URL
    /// or asset an `image` node's `src` takes) as Unicode scalar values.
    Image = 70, None;
    /// `drawImage`: `image sx sy sw sh dx dy dw dh`, the source rectangle in
    /// image pixels, already normalised and clipped; the destination in user
    /// space under the author matrix.
    DrawImage = 71, Some(9);
    /// `putImageData`: `x y w h` in backing pixels, then `w × h` pixels, each
    /// `r·2²⁴ + g·2¹⁶ + b·2⁸ + a`, non-premultiplied. The transform, clip,
    /// alpha, compositing and shadows do not apply.
    PutImageData = 72, None;
    /// A `Path2D`'s segments for the next path paint, in canvas coordinates
    /// (the matrix at the paint applied): a new subpath at `x y`.
    PathMoveTo = 80, Some(2);
    /// … a line to `x y`.
    PathLineTo = 81, Some(2);
    /// … a quadratic to `x y` through `cx cy`.
    PathQuadTo = 82, Some(4);
    /// … a cubic to `x y` through `c1x c1y c2x c2y`.
    PathCubicTo = 83, Some(6);
    /// … `closePath()`.
    PathClose = 84, Some(0);
    /// `fill(path, rule)`: the segments since the last path paint, which are
    /// then dropped; the current path is untouched.
    FillPath = 85, Some(1);
    /// `stroke(path)`, likewise.
    StrokePath = 86, Some(0);
    /// `clip(path, rule)`, likewise.
    ClipPath = 87, Some(1);
}

/// A string as list operands: its Unicode scalar values.
pub fn text_operands(s: &str, out: &mut Vec<f64>) {
    out.extend(s.chars().map(|c| c as u32 as f64));
}

/// The string at operands `from..` of a record.
pub fn text_at(r: &Record<'_>, from: usize) -> String {
    (from..r.len())
        .filter_map(|i| char::from_u32(r.at(i) as u32))
        .collect()
}

/// Appends records to a list.
#[derive(Debug, Clone)]
pub struct Writer {
    bytes: Vec<u8>,
    ops: usize,
}

impl Default for Writer {
    fn default() -> Self {
        Writer::new()
    }
}

impl Writer {
    /// An empty list: the header alone.
    pub fn new() -> Writer {
        let mut bytes = Vec::with_capacity(256);
        bytes.extend_from_slice(&MAGIC.to_le_bytes());
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        Writer { bytes, ops: 0 }
    }

    /// One record.
    pub fn op(&mut self, op: Op, operands: &[f64]) {
        debug_assert!(op.arity().is_none_or(|n| n == operands.len()));
        self.bytes.extend_from_slice(&(op as u32).to_le_bytes());
        self.bytes
            .extend_from_slice(&(operands.len() as u32).to_le_bytes());
        for v in operands {
            self.bytes.extend_from_slice(&v.to_le_bytes());
        }
        self.ops += 1;
    }

    /// Records written.
    pub fn ops(&self) -> usize {
        self.ops
    }

    /// Bytes so far, header included.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether no record has been written.
    pub fn is_empty(&self) -> bool {
        self.ops == 0
    }

    /// The finished list.
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// One decoded record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Record<'a> {
    /// The operation.
    pub op: Op,
    /// Its operands, still little-endian f64s.
    raw: &'a [u8],
}

impl Record<'_> {
    /// Operand `i`.
    pub fn at(&self, i: usize) -> f64 {
        f64::from_le_bytes(self.raw[i * 8..i * 8 + 8].try_into().unwrap())
    }

    /// The operand count.
    pub fn len(&self) -> usize {
        self.raw.len() / 8
    }

    /// Whether it has no operands.
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// Every operand.
    pub fn operands(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len()).map(|i| self.at(i))
    }
}

/// Why a list was refused by [`check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed(pub String);

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "malformed canvas list: {}", self.0)
    }
}

/// Decode `bytes` into records, refusing a bad header, an unknown opcode, a
/// wrong operand count or a truncated record. Structural only (LLP 1056 D3):
/// the recorder settled semantics at the call.
pub fn records(bytes: &[u8]) -> Result<Vec<Record<'_>>, Malformed> {
    let bad = |m: String| Err(Malformed(m));
    if bytes.len() < HEADER {
        return bad("shorter than its header".into());
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    if word(0) != MAGIC {
        return bad("not a canvas list".into());
    }
    if word(4) != VERSION {
        return bad(format!("version {}, this host reads {VERSION}", word(4)));
    }
    let mut out = Vec::new();
    let mut at = HEADER;
    while at < bytes.len() {
        if at + 8 > bytes.len() {
            return bad(format!("record at byte {at} is truncated"));
        }
        let (code, n) = (word(at), word(at + 4) as usize);
        let Some(op) = Op::from_code(code) else {
            return bad(format!("unknown opcode {code} at byte {at}"));
        };
        if op.arity().is_some_and(|k| k != n) {
            return bad(format!("{} with {n} operands at byte {at}", op.name()));
        }
        let end = at + 8 + n * 8;
        if end > bytes.len() {
            return bad(format!("{} at byte {at} is truncated", op.name()));
        }
        out.push(Record {
            op,
            raw: &bytes[at + 8..end],
        });
        at = end;
    }
    Ok(out)
}

/// The per-canvas structural check: [`records`], plus object ids
/// (gradients, patterns, images: one space) that were created before use in
/// this generation, and finite enum operands in range. `ids` carries the
/// ids created by earlier lists of the generation.
pub fn check(bytes: &[u8], ids: &mut Vec<u32>) -> Result<usize, Malformed> {
    let recs = records(bytes)?;
    for r in &recs {
        let known = |id: f64, g: &Vec<u32>| g.contains(&(id as u32));
        let unknown = |at: usize| {
            Err(Malformed(format!(
                "{} names object {} before it was created",
                r.op.name(),
                r.at(at)
            )))
        };
        match r.op {
            Op::LinearGradient | Op::RadialGradient | Op::ConicGradient | Op::Image => {
                if r.is_empty() {
                    return Err(Malformed(format!("{} with no id", r.op.name())));
                }
                ids.push(r.at(0) as u32)
            }
            Op::Pattern if !known(r.at(1), ids) => return unknown(1),
            Op::Pattern if r.at(2) > 3.0 => {
                return Err(Malformed(format!("Pattern repetition {}", r.at(2))))
            }
            Op::Pattern => ids.push(r.at(0) as u32),
            Op::ImageSmoothing if r.at(0) > 1.0 || r.at(1) > 2.0 => {
                return Err(Malformed(format!("ImageSmoothing {} {}", r.at(0), r.at(1))))
            }
            Op::DrawImage if !known(r.at(0), ids) => return unknown(0),
            Op::FillGradient
            | Op::StrokeGradient
            | Op::ColorStop
            | Op::FillPattern
            | Op::StrokePattern
            | Op::PatternTransform
                if !known(r.at(0), ids) =>
            {
                return unknown(0)
            }
            Op::Font | Op::FillText | Op::StrokeText
                if r.len() < if r.op == Op::Font { 9 } else { 4 } =>
            {
                return Err(Malformed(format!(
                    "{} with {} operands",
                    r.op.name(),
                    r.len()
                )))
            }
            Op::PutImageData
                if r.len() < 4 || (r.at(2).max(0.0) * r.at(3).max(0.0)) as usize != r.len() - 4 =>
            {
                return Err(Malformed(format!("PutImageData with {} operands", r.len())))
            }
            Op::Composite if r.at(0) as usize >= crate::COMPOSITE.len() => {
                return Err(Malformed(format!("composite {}", r.at(0))))
            }
            Op::LineCap | Op::LineJoin | Op::Fill | Op::Clip | Op::FillPath | Op::ClipPath
                if r.at(0) > 2.0 =>
            {
                return Err(Malformed(format!("{} {}", r.op.name(), r.at(0))))
            }
            _ => {}
        }
        if r.operands().any(|v| !v.is_finite()) {
            return Err(Malformed(format!(
                "{} with a non-finite operand",
                r.op.name()
            )));
        }
    }
    Ok(recs.len())
}

/// A readable form for the agent's `layout` (LLP 1056 §5): one call a line,
/// at most `max_lines` lines and `max_bytes` bytes.
pub fn describe(bytes: &[u8], max_lines: usize, max_bytes: usize) -> Vec<String> {
    let Ok(recs) = records(bytes) else {
        return vec!["(malformed)".into()];
    };
    let mut out = Vec::new();
    let mut used = 0;
    for (i, r) in recs.iter().enumerate() {
        let line = match r.op {
            Op::Composite => format!(
                "Composite {}",
                crate::COMPOSITE.get(r.at(0) as usize).unwrap_or(&"?")
            ),
            Op::Font => format!(
                "Font {} {}",
                r.operands().take(9).map(trim).collect::<Vec<_>>().join(" "),
                text_at(r, 9)
            ),
            Op::FillText | Op::StrokeText => format!(
                "{} {} {:?}",
                r.op.name(),
                r.operands().take(4).map(trim).collect::<Vec<_>>().join(" "),
                text_at(r, 4)
            ),
            Op::Image => format!("Image {} {:?}", trim(r.at(0)), text_at(r, 1)),
            Op::PutImageData => format!(
                "PutImageData {} ({} pixels)",
                r.operands().take(4).map(trim).collect::<Vec<_>>().join(" "),
                r.len().saturating_sub(4)
            ),
            _ => {
                let args: Vec<String> = r.operands().map(trim).collect();
                if args.is_empty() {
                    r.op.name().to_string()
                } else {
                    format!("{} {}", r.op.name(), args.join(" "))
                }
            }
        };
        if out.len() + 1 >= max_lines || used + line.len() > max_bytes {
            out.push(format!("… {} more", recs.len() - i));
            break;
        }
        used += line.len() + 1;
        out.push(line);
    }
    out
}

fn trim(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_round_trips_and_is_aligned() {
        let mut w = Writer::new();
        w.op(Op::MoveTo, &[1.0, 2.0]);
        w.op(Op::LineDash, &[3.0, 4.0, 5.0, 3.0, 4.0, 5.0]);
        w.op(Op::Stroke, &[]);
        let bytes = w.finish();
        assert_eq!(bytes.len() % 8, 0);
        let recs = records(&bytes).unwrap();
        assert_eq!(recs.len(), 3);
        assert_eq!((recs[0].op, recs[0].at(1)), (Op::MoveTo, 2.0));
        assert_eq!(recs[1].len(), 6);
    }

    #[test]
    fn the_check_refuses_structure_it_cannot_replay() {
        let mut g = Vec::new();
        assert!(check(b"nope", &mut g).is_err());
        let mut w = Writer::new();
        w.op(Op::FillGradient, &[7.0]);
        assert!(check(&w.finish(), &mut g)
            .unwrap_err()
            .0
            .contains("before it was created"));
        let mut w = Writer::new();
        w.op(Op::LinearGradient, &[7.0, 0.0, 0.0, 1.0, 1.0]);
        w.op(Op::FillGradient, &[7.0]);
        assert_eq!(check(&w.finish(), &mut g), Ok(2));
        let mut bytes = Writer::new().finish();
        bytes.extend_from_slice(&41u32.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        assert!(check(&bytes, &mut g)
            .unwrap_err()
            .0
            .contains("MoveTo with 3"));
    }

    #[test]
    fn describe_is_capped() {
        let mut w = Writer::new();
        for i in 0..500 {
            w.op(Op::LineTo, &[i as f64, 0.5]);
        }
        let lines = describe(&w.finish(), 200, 16 * 1024);
        assert_eq!(lines.len(), 200);
        assert_eq!(lines[0], "LineTo 0 0.5");
        assert!(lines[199].starts_with("… "));
    }
}
