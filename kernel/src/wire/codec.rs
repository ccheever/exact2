//! Bounds-checked little-endian reading and writing.
//!
//! Every read is checked against the remaining length and reports the exact
//! shortfall. Value-level grammars that the generated style codec needs —
//! dimensions, colors, grid tracks and placements, the style mask — live here so
//! the generator emits calls, never byte arithmetic.

use crate::error::DecodeError;
use crate::generated::{StyleId, StyleMask, STYLE_MASK_WORDS};
use crate::style::{
    Color, Dimension, Edge, GridLine, GridPlacement, GridTrack, GridTracks, Transitions, Vec2,
    MAX_GRID_TRACKS,
};
use exact_motion::easing::MAX_LINEAR_STOPS;
use exact_motion::{
    Easing, LinearStop, Property, SpringConfig, StepPosition, TimingFunction, Transition,
    TransitionProperty, MAX_TRANSITIONS,
};

/// Bound on any string field on the wire.
pub const MAX_STRING_BYTES: u32 = 1 << 24;

/// Round `n` up to a multiple of 8.
pub const fn align8(n: usize) -> usize {
    (n + 7) & !7
}

/// A cursor over a byte slice.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Start at the first byte.
    pub const fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, pos: 0 }
    }

    /// Bytes not yet read.
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    /// Current offset.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Whether every byte has been read.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Take `n` bytes.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.remaining() < n {
            return Err(DecodeError::Truncated {
                needed: n,
                available: self.remaining(),
            });
        }
        let out = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    /// Skip to the next 8-byte boundary (relative to the slice start).
    pub fn align8(&mut self) -> Result<(), DecodeError> {
        let target = align8(self.pos);
        let pad = target - self.pos;
        self.bytes(pad).map(|_| ())
    }

    /// Read a byte.
    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.bytes(1)?[0])
    }

    /// Read a little-endian `u16`.
    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    /// Read a little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Read a little-endian `u64`.
    pub fn u64(&mut self) -> Result<u64, DecodeError> {
        let b = self.bytes(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }

    /// Read a little-endian `i16`.
    pub fn i16(&mut self) -> Result<i16, DecodeError> {
        Ok(self.u16()? as i16)
    }

    /// Read a little-endian `i32`.
    pub fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(self.u32()? as i32)
    }

    /// Read a little-endian `i64`.
    pub fn i64(&mut self) -> Result<i64, DecodeError> {
        Ok(self.u64()? as i64)
    }

    /// Read an IEEE-754 binary32.
    pub fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_bits(self.u32()?))
    }

    /// Read an IEEE-754 binary64.
    pub fn f64(&mut self) -> Result<f64, DecodeError> {
        Ok(f64::from_bits(self.u64()?))
    }

    /// Read a length-prefixed UTF-8 string.
    pub fn string(&mut self) -> Result<&'a str, DecodeError> {
        let len = self.u32()?;
        if len > MAX_STRING_BYTES {
            return Err(DecodeError::StringTooLong(len));
        }
        let bytes = self.bytes(len as usize)?;
        std::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)
    }

    /// Read a dimension: kind byte (0 auto, 1 points, 2 percent, 3–6 an
    /// `env()` length at the top/right/bottom/left safe-area inset, 7
    /// `env(keyboard-inset-height)`) then `f32` (the points added to an inset).
    pub fn dimension(
        &mut self,
        style: StyleId,
        admits_auto: bool,
    ) -> Result<Dimension, DecodeError> {
        let kind = self.u8()?;
        let value = self.f32()?;
        let dim = match kind {
            0 => {
                if !admits_auto {
                    return Err(DecodeError::AutoNotAdmitted { style });
                }
                Dimension::Auto
            }
            1 => Dimension::Points(value),
            2 => Dimension::Percent(value),
            3..=6 => Dimension::Env(Edge::from_index(kind - 3).expect("3..=6 is an edge"), value),
            7 => Dimension::Keyboard(value),
            other => return Err(DecodeError::UnknownDimensionKind(other)),
        };
        if kind != 0 && !value.is_finite() {
            return Err(DecodeError::NonFinite(style));
        }
        Ok(dim)
    }

    /// Read a packed RGBA8 color.
    pub fn color(&mut self) -> Result<Color, DecodeError> {
        Ok(Color(self.u32()?))
    }

    /// Read two `f32`s.
    pub fn vec2(&mut self) -> Result<Vec2, DecodeError> {
        Ok(Vec2 {
            x: self.f32()?,
            y: self.f32()?,
        })
    }

    /// Read two colors.
    pub fn color2(&mut self) -> Result<[Color; 2], DecodeError> {
        Ok([self.color()?, self.color()?])
    }

    /// Read a grid track list: count byte, then (kind byte, `f32`) per track.
    pub fn tracks(&mut self) -> Result<GridTracks, DecodeError> {
        let count = self.u8()?;
        if count as usize > MAX_GRID_TRACKS {
            return Err(DecodeError::TooManyTracks(count));
        }
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let kind = self.u8()?;
            let value = self.f32()?;
            out.push(match kind {
                0 => GridTrack::Fr(value),
                1 => GridTrack::Points(value),
                2 => GridTrack::Percent(value),
                3 => GridTrack::Auto,
                4 => GridTrack::MinContent,
                5 => GridTrack::MaxContent,
                other => return Err(DecodeError::UnknownTrackKind(other)),
            });
        }
        Ok(GridTracks(out))
    }

    fn grid_line(&mut self) -> Result<GridLine, DecodeError> {
        let kind = self.u8()?;
        let value = self.i16()?;
        Ok(match kind {
            0 => GridLine::Auto,
            1 => GridLine::Line(value),
            2 => GridLine::Span(value.unsigned_abs()),
            other => return Err(DecodeError::UnknownPlacementKind(other)),
        })
    }

    /// Read a grid placement: start line then end line, each (kind byte, `i16`).
    pub fn placement(&mut self) -> Result<GridPlacement, DecodeError> {
        Ok(GridPlacement {
            start: self.grid_line()?,
            end: self.grid_line()?,
        })
    }

    /// Read a `transition` row (grammar: `schema.json` `_transitions`) and
    /// validate it the way the evaluator will, so an invalid declaration is a
    /// decode rejection, never a later surprise.
    pub fn transitions(&mut self) -> Result<Transitions, DecodeError> {
        let count = self.u8()?;
        if count as usize > MAX_TRANSITIONS {
            return Err(DecodeError::TooManyTransitions(count));
        }
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let property = match self.u8()? {
                0 => TransitionProperty::All,
                p => TransitionProperty::Property(
                    Property::from_wire(p - 1).ok_or(DecodeError::UnknownTransitionProperty(p))?,
                ),
            };
            let duration = self.f32()? as f64;
            let delay = self.f32()? as f64;
            let timing = match self.u8()? {
                0 => TimingFunction::Easing(Easing::Linear),
                1 => TimingFunction::Easing(Easing::Ease),
                2 => TimingFunction::Easing(Easing::EaseIn),
                3 => TimingFunction::Easing(Easing::EaseOut),
                4 => TimingFunction::Easing(Easing::EaseInOut),
                5 => TimingFunction::Easing(Easing::CubicBezier {
                    x1: self.f32()? as f64,
                    y1: self.f32()? as f64,
                    x2: self.f32()? as f64,
                    y2: self.f32()? as f64,
                }),
                6 => {
                    let count = self.u16()?;
                    let position = self.u8()?;
                    TimingFunction::Easing(Easing::Steps {
                        count,
                        position: StepPosition::from_wire(position)
                            .ok_or(DecodeError::UnknownStepPosition(position))?,
                    })
                }
                7 => TimingFunction::Spring(SpringConfig {
                    stiffness: self.f32()? as f64,
                    damping: self.f32()? as f64,
                    mass: self.f32()? as f64,
                }),
                8 => {
                    let stops = self.u8()?;
                    if stops as usize > MAX_LINEAR_STOPS {
                        return Err(DecodeError::TooManyEasingStops(stops));
                    }
                    let mut list = Vec::with_capacity(stops as usize);
                    for _ in 0..stops {
                        list.push(LinearStop {
                            input: self.f32()? as f64,
                            output: self.f32()? as f64,
                        });
                    }
                    TimingFunction::Easing(Easing::PiecewiseLinear(list))
                }
                other => return Err(DecodeError::UnknownEasing(other)),
            };
            out.push(Transition {
                property,
                duration,
                delay,
                timing,
            });
        }
        let transitions = Transitions(out);
        transitions
            .validate()
            .map_err(DecodeError::InvalidTransition)?;
        Ok(transitions)
    }

    /// Read the style mask words and reject reserved bits.
    pub fn style_mask(&mut self) -> Result<StyleMask, DecodeError> {
        let mut words = [0u64; STYLE_MASK_WORDS];
        for word in words.iter_mut() {
            *word = self.u64()?;
        }
        let mask = StyleMask { words };
        if mask.intersects(StyleMask::RESERVED) {
            return Err(DecodeError::ReservedMaskBits);
        }
        Ok(mask)
    }
}

/// A growable little-endian byte buffer.
#[derive(Debug, Default, Clone)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// An empty buffer.
    pub fn new() -> Self {
        Writer { buf: Vec::new() }
    }

    /// Bytes written so far.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether nothing was written.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    /// Take the bytes.
    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    /// Append raw bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }

    /// Zero-pad to the next 8-byte boundary.
    pub fn pad8(&mut self) {
        let target = align8(self.buf.len());
        self.buf.resize(target, 0);
    }

    /// Append a byte.
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Append a `u16`.
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Append a `u32`.
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Append a `u64`.
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Append an `i16`.
    pub fn i16(&mut self, v: i16) {
        self.u16(v as u16);
    }

    /// Append an `i32`.
    pub fn i32(&mut self, v: i32) {
        self.u32(v as u32);
    }

    /// Append an `i64`.
    pub fn i64(&mut self, v: i64) {
        self.u64(v as u64);
    }

    /// Append an `f32`.
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }

    /// Append an `f64`.
    pub fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }

    /// Append a length-prefixed string.
    pub fn string(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.bytes(s.as_bytes());
    }

    /// Overwrite a `u32` at `pos` (for back-patching lengths).
    pub fn put_u32_at(&mut self, pos: usize, v: u32) {
        self.buf[pos..pos + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// Append a dimension.
    pub fn dimension(&mut self, d: Dimension) {
        match d {
            Dimension::Auto => {
                self.u8(0);
                self.f32(0.0);
            }
            Dimension::Points(v) => {
                self.u8(1);
                self.f32(v);
            }
            Dimension::Env(edge, v) => {
                self.u8(3 + edge as u8);
                self.f32(v);
            }
            Dimension::Percent(v) => {
                self.u8(2);
                self.f32(v);
            }
            Dimension::Keyboard(v) => {
                self.u8(7);
                self.f32(v);
            }
        }
    }

    /// Append a color.
    pub fn color(&mut self, c: Color) {
        self.u32(c.0);
    }

    /// Append two `f32`s.
    pub fn vec2(&mut self, v: Vec2) {
        self.f32(v.x);
        self.f32(v.y);
    }

    /// Append two colors.
    pub fn color2(&mut self, c: [Color; 2]) {
        self.color(c[0]);
        self.color(c[1]);
    }

    /// Append a grid track list.
    pub fn tracks(&mut self, t: &GridTracks) {
        debug_assert!(t.0.len() <= MAX_GRID_TRACKS);
        self.u8(t.0.len() as u8);
        for track in &t.0 {
            let (kind, value) = match *track {
                GridTrack::Fr(v) => (0, v),
                GridTrack::Points(v) => (1, v),
                GridTrack::Percent(v) => (2, v),
                GridTrack::Auto => (3, 0.0),
                GridTrack::MinContent => (4, 0.0),
                GridTrack::MaxContent => (5, 0.0),
            };
            self.u8(kind);
            self.f32(value);
        }
    }

    /// Append a `transition` row.
    pub fn transitions(&mut self, t: &Transitions) {
        debug_assert!(t.0.len() <= MAX_TRANSITIONS);
        self.u8(t.0.len() as u8);
        for transition in &t.0 {
            self.u8(match transition.property {
                TransitionProperty::All => 0,
                TransitionProperty::Property(p) => p as u8 + 1,
            });
            self.f32(transition.duration as f32);
            self.f32(transition.delay as f32);
            match &transition.timing {
                TimingFunction::Easing(Easing::Linear) => self.u8(0),
                TimingFunction::Easing(Easing::Ease) => self.u8(1),
                TimingFunction::Easing(Easing::EaseIn) => self.u8(2),
                TimingFunction::Easing(Easing::EaseOut) => self.u8(3),
                TimingFunction::Easing(Easing::EaseInOut) => self.u8(4),
                TimingFunction::Easing(Easing::CubicBezier { x1, y1, x2, y2 }) => {
                    self.u8(5);
                    for v in [x1, y1, x2, y2] {
                        self.f32(*v as f32);
                    }
                }
                TimingFunction::Easing(Easing::Steps { count, position }) => {
                    self.u8(6);
                    self.u16(*count);
                    self.u8(StepPosition::ALL
                        .iter()
                        .position(|p| p == position)
                        .unwrap_or(1) as u8);
                }
                TimingFunction::Spring(config) => {
                    self.u8(7);
                    self.f32(config.stiffness as f32);
                    self.f32(config.damping as f32);
                    self.f32(config.mass as f32);
                }
                TimingFunction::Easing(Easing::PiecewiseLinear(stops)) => {
                    self.u8(8);
                    debug_assert!(stops.len() <= MAX_LINEAR_STOPS);
                    self.u8(stops.len() as u8);
                    for stop in stops {
                        self.f32(stop.input as f32);
                        self.f32(stop.output as f32);
                    }
                }
            }
        }
    }

    fn grid_line(&mut self, line: GridLine) {
        match line {
            GridLine::Auto => {
                self.u8(0);
                self.i16(0);
            }
            GridLine::Line(n) => {
                self.u8(1);
                self.i16(n);
            }
            GridLine::Span(n) => {
                self.u8(2);
                self.i16(n as i16);
            }
        }
    }

    /// Append a grid placement.
    pub fn placement(&mut self, p: GridPlacement) {
        self.grid_line(p.start);
        self.grid_line(p.end);
    }

    /// Append the style mask words.
    pub fn style_mask(&mut self, m: StyleMask) {
        for word in m.words {
            self.u64(word);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_round_trip() {
        let mut w = Writer::new();
        w.u8(7);
        w.u16(0x1234);
        w.u32(0xdead_beef);
        w.u64(0x0102_0304_0506_0708);
        w.i32(-5);
        w.f32(1.5);
        w.f64(-2.25);
        w.string("héllo");
        let bytes = w.into_vec();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u8().unwrap(), 7);
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert_eq!(r.u32().unwrap(), 0xdead_beef);
        assert_eq!(r.u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(r.i32().unwrap(), -5);
        assert_eq!(r.f32().unwrap(), 1.5);
        assert_eq!(r.f64().unwrap(), -2.25);
        assert_eq!(r.string().unwrap(), "héllo");
        assert!(r.is_empty());
    }

    #[test]
    fn truncation_reports_shortfall() {
        let bytes = [1u8, 2, 3];
        let mut r = Reader::new(&bytes);
        assert_eq!(
            r.u32(),
            Err(DecodeError::Truncated {
                needed: 4,
                available: 3
            })
        );
    }

    #[test]
    fn env_lengths_round_trip_by_edge() {
        let mut w = Writer::new();
        for (i, edge) in Edge::ALL.iter().enumerate() {
            w.dimension(Dimension::Env(*edge, i as f32 * 1.5));
        }
        let bytes = w.into_vec();
        assert_eq!(bytes[0], 3, "top is kind 3");
        assert_eq!(bytes[15], 6, "left is kind 6");
        let mut r = Reader::new(&bytes);
        for (i, edge) in Edge::ALL.iter().enumerate() {
            assert_eq!(
                r.dimension(StyleId::PaddingTop, false),
                Ok(Dimension::Env(*edge, i as f32 * 1.5))
            );
        }
        let mut w = Writer::new();
        w.dimension(Dimension::Keyboard(12.0));
        let bytes = w.into_vec();
        assert_eq!(bytes[0], 7, "keyboard is kind 7");
        let mut r = Reader::new(&bytes);
        assert_eq!(
            r.dimension(StyleId::PaddingBottom, false),
            Ok(Dimension::Keyboard(12.0))
        );
        let mut r = Reader::new(&[8u8, 0, 0, 0, 0]);
        assert_eq!(
            r.dimension(StyleId::Width, true),
            Err(DecodeError::UnknownDimensionKind(8))
        );
    }

    #[test]
    fn auto_is_rejected_where_not_admitted() {
        let mut w = Writer::new();
        w.dimension(Dimension::Auto);
        let bytes = w.into_vec();
        let mut r = Reader::new(&bytes);
        assert_eq!(
            r.dimension(StyleId::PaddingTop, false),
            Err(DecodeError::AutoNotAdmitted {
                style: StyleId::PaddingTop
            })
        );
        let mut r = Reader::new(&bytes);
        assert_eq!(r.dimension(StyleId::Width, true), Ok(Dimension::Auto));
    }

    #[test]
    fn tracks_and_placement_round_trip() {
        let tracks = GridTracks(vec![
            GridTrack::Fr(1.0),
            GridTrack::Points(20.0),
            GridTrack::Percent(50.0),
            GridTrack::Auto,
            GridTrack::MinContent,
            GridTrack::MaxContent,
        ]);
        let placement = GridPlacement {
            start: GridLine::Line(2),
            end: GridLine::Span(3),
        };
        let mut w = Writer::new();
        w.tracks(&tracks);
        w.placement(placement);
        let bytes = w.into_vec();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.tracks().unwrap(), tracks);
        assert_eq!(r.placement().unwrap(), placement);
    }

    #[test]
    fn reserved_mask_bits_are_rejected() {
        let mut w = Writer::new();
        w.style_mask(StyleMask::RESERVED);
        let bytes = w.into_vec();
        assert_eq!(
            Reader::new(&bytes).style_mask(),
            Err(DecodeError::ReservedMaskBits)
        );
        let mut w = Writer::new();
        w.style_mask(StyleMask::ALL);
        let bytes = w.into_vec();
        assert_eq!(Reader::new(&bytes).style_mask(), Ok(StyleMask::ALL));
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let mut w = Writer::new();
        w.u32(2);
        w.bytes(&[0xff, 0xfe]);
        let bytes = w.into_vec();
        assert_eq!(Reader::new(&bytes).string(), Err(DecodeError::InvalidUtf8));
    }
}
