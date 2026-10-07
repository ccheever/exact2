//! Android's borrowed transaction wire: fixed numeric records for layout and
//! motion and paint deltas, unchanged JSON for infrequent tree/style operations.
//!
//! A presenter consumes one buffer before the next host operation. The Rust
//! buffer and metadata scratch retain capacity across transactions; neither a
//! node nor a property requires a JNI call.

#[path = "paint.rs"]
mod paint;

use std::collections::BTreeMap;

/// The fixed header size, before the length-delimited records.
pub const HEADER_BYTES: usize = 32;
/// The current Android transaction format.
pub const VERSION: u16 = 1;
/// An infrequent operation, as the shared native host wrote it.
pub const JSON: u8 = 1;
/// A view id followed by four little-endian `f32`s.
pub const FRAME: u8 = 2;
/// A view id followed by two little-endian `f32`s.
pub const CONTENT: u8 = 3;
/// A view id, property byte, arity byte and little-endian `f64` values.
pub const PRESENT: u8 = 4;
/// A view id, six-bit color mask and light/dark ARGB pairs in bit order.
pub const PAINT: u8 = 5;
/// A view id whose already-measured paragraph must be redrawn.
pub const INVALIDATE: u8 = 6;
/// One transaction-local style slot followed by its complete JSON dictionary.
pub const STYLE_DEFINE: u8 = 7;
/// A style slot followed by a cold operation with an empty style placeholder.
pub const STYLE_REFERENCE: u8 = 8;

const STYLE_SLOTS: usize = 128;
const STYLE_BYTES: usize = 64 * 1024;

#[derive(Default)]
struct StylePool {
    slots: BTreeMap<Box<[u8]>, u32>,
    bytes: usize,
}

impl StylePool {
    fn write(
        &mut self,
        json: &[u8],
        style: &[u8],
        span: std::ops::Range<usize>,
        out: &mut Vec<u8>,
    ) -> Result<u32, WireError> {
        let mut records = 1;
        let slot = if let Some(slot) = self.slots.get(style) {
            *slot
        } else {
            if self.slots.len() == STYLE_SLOTS || style.len() > STYLE_BYTES - self.bytes {
                record(out, JSON, json)?;
                return Ok(records);
            }
            let slot = self.slots.len() as u32;
            let mut definition = Vec::with_capacity(4 + style.len());
            definition.extend_from_slice(&slot.to_le_bytes());
            definition.extend_from_slice(style);
            record(out, STYLE_DEFINE, &definition)?;
            self.slots.insert(style.into(), slot);
            self.bytes += style.len();
            records += 1;
            slot
        };
        let mut payload = Vec::with_capacity(6 + json.len() - style.len());
        payload.extend_from_slice(&slot.to_le_bytes());
        payload.extend_from_slice(&json[..span.start]);
        payload.extend_from_slice(b"{}");
        payload.extend_from_slice(&json[span.end..]);
        record(out, STYLE_REFERENCE, &payload)?;
        Ok(records)
    }
}

/// A refused or malformed shared-host batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// JSON grammar, the envelope or a fixed operation is invalid.
    Invalid,
    /// An encoded count exceeds the wire's 32-bit length fields.
    TooLarge,
}

/// Reusable metadata scratch for one owner-thread runtime.
#[derive(Default)]
pub struct Encoder {
    metadata: Vec<u8>,
    paint: paint::Cache,
}

impl Encoder {
    /// Transcode one native batch without constructing a JSON object tree.
    ///
    /// Header: `EXA1`, version (`u16`), flags (`u16`), record count (`u32`),
    /// clock and timer deadline (`f64` each), metadata length (`u32`). Flags:
    /// timers=1, motion=2, spatial=4, canvas=8, frames=16, canvasOwed=32,
    /// clock present=64, deadline present=128. Every record is an opcode
    /// (`u8`), payload length (`u32`), payload. Optional metadata JSON follows
    /// the records: fields already in the header and a null error are omitted.
    /// A normal transaction has zero metadata bytes and needs no JSON parser.
    pub fn encode(&mut self, json: &[u8], out: &mut Vec<u8>) -> Result<(), WireError> {
        let result = self.encode_batch(json, out);
        self.paint.finish(result.is_ok());
        result
    }

    fn encode_batch(&mut self, json: &[u8], out: &mut Vec<u8>) -> Result<(), WireError> {
        std::str::from_utf8(json).map_err(|_| WireError::Invalid)?;
        out.clear();
        out.resize(HEADER_BYTES, 0);
        self.metadata.clear();
        self.metadata.push(b'{');
        let mut r = Reader::new(json);
        r.expect(b'{')?;
        let mut count = None;
        let mut flags = 0u16;
        let mut clock = 0f64;
        let mut deadline = 0f64;
        let mut metadata_fields = 0;
        let mut styles = StylePool::default();
        if !r.take(b'}') {
            loop {
                let start = r.pos;
                let key = r.string()?;
                r.expect(b':')?;
                if key == b"ops" {
                    if count.is_some() {
                        return Err(WireError::Invalid);
                    }
                    count = Some(encode_ops(&mut r, out, &mut self.paint, &mut styles)?);
                } else {
                    let value = r.value()?;
                    match key {
                        b"timers" => flag(value, 1, &mut flags)?,
                        b"motion" => flag(value, 2, &mut flags)?,
                        b"spatial" => flag(value, 4, &mut flags)?,
                        b"canvas" => flag(value, 8, &mut flags)?,
                        b"frames" => flag(value, 16, &mut flags)?,
                        b"controls" => flag(value, 256, &mut flags)?,
                        b"canvasOwed" => flag(value, 32, &mut flags)?,
                        b"clock" => {
                            clock = number(value)?;
                            if !clock.is_finite() {
                                return Err(WireError::Invalid);
                            }
                            flags |= 64;
                        }
                        b"timer_due_ms" => {
                            deadline = number(value)?;
                            if !deadline.is_finite() {
                                return Err(WireError::Invalid);
                            }
                            flags |= 128;
                        }
                        _ => {
                            if key != b"error" || value != b"null" {
                                if metadata_fields != 0 {
                                    self.metadata.push(b',');
                                }
                                self.metadata.extend_from_slice(&json[start..r.pos]);
                                metadata_fields += 1;
                            }
                        }
                    }
                }
                if r.take(b'}') {
                    break;
                }
                r.expect(b',')?;
            }
        }
        r.end()?;
        let count = count.ok_or(WireError::Invalid)?;
        if metadata_fields == 0 {
            self.metadata.clear();
        } else {
            self.metadata.push(b'}');
        }
        let length = u32::try_from(self.metadata.len()).map_err(|_| WireError::TooLarge)?;
        out[..4].copy_from_slice(b"EXA1");
        out[4..6].copy_from_slice(&VERSION.to_le_bytes());
        out[6..8].copy_from_slice(&flags.to_le_bytes());
        out[8..12].copy_from_slice(&count.to_le_bytes());
        out[12..20].copy_from_slice(&clock.to_le_bytes());
        out[20..28].copy_from_slice(&deadline.to_le_bytes());
        out[28..32].copy_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&self.metadata);
        Ok(())
    }
}

fn flag(value: &[u8], bit: u16, flags: &mut u16) -> Result<(), WireError> {
    match value {
        b"true" => *flags |= bit,
        b"false" => *flags &= !bit,
        _ => return Err(WireError::Invalid),
    }
    Ok(())
}

fn number<T: std::str::FromStr>(value: &[u8]) -> Result<T, WireError> {
    std::str::from_utf8(value)
        .map_err(|_| WireError::Invalid)?
        .parse()
        .map_err(|_| WireError::Invalid)
}

fn record(out: &mut Vec<u8>, opcode: u8, payload: &[u8]) -> Result<(), WireError> {
    let length = u32::try_from(payload.len()).map_err(|_| WireError::TooLarge)?;
    out.push(opcode);
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

fn encode_ops(
    r: &mut Reader<'_>,
    out: &mut Vec<u8>,
    paint: &mut paint::Cache,
    styles: &mut StylePool,
) -> Result<u32, WireError> {
    r.expect(b'[')?;
    let mut count = 0u32;
    if !r.take(b']') {
        loop {
            count = count
                .checked_add(encode_op(r, out, paint, styles)?)
                .ok_or(WireError::TooLarge)?;
            if r.take(b']') {
                break;
            }
            r.expect(b',')?;
        }
    }
    Ok(count)
}

fn encode_op(
    r: &mut Reader<'_>,
    out: &mut Vec<u8>,
    paint: &mut paint::Cache,
    styles: &mut StylePool,
) -> Result<u32, WireError> {
    r.whitespace();
    let start = r.pos;
    r.expect(b'{')?;
    let mut kind = None;
    let mut id = None;
    let mut property = None;
    let mut style = None;
    let mut runs = None;
    let mut style_span = None;
    let mut style_fields = 0;
    let mut escaped_key = false;
    let mut seen_keys: [Option<&[u8]>; 16] = [None; 16];
    let mut fields = 0;
    let mut duplicate_key = false;
    let mut values = [None; 4];
    if !r.take(b'}') {
        loop {
            let key = r.string()?;
            escaped_key |= key.contains(&b'\\');
            duplicate_key |= seen_keys.iter().flatten().any(|seen| *seen == key);
            if let Some(slot) = seen_keys.get_mut(fields) {
                *slot = Some(key);
            } else {
                // Unusual extension dictionaries keep the original cold form.
                duplicate_key = true;
            }
            fields += 1;
            r.expect(b':')?;
            r.whitespace();
            let value_start = r.pos;
            let value = if key == b"style" {
                paint.value(r)?
            } else {
                r.value()?
            };
            match key {
                b"op" => kind = Some(value),
                b"id" => id = Some(value),
                b"property" => property = Some(value),
                b"style" => {
                    style = Some(value);
                    style_fields += 1;
                    style_span = Some(value_start - start..r.pos - start);
                }
                b"runs" => runs = Some(value),
                b"x" => values[0] = Some(value),
                b"y" => values[1] = Some(value),
                b"w" => values[2] = Some(value),
                b"h" => values[3] = Some(value),
                _ => {}
            }
            if r.take(b'}') {
                break;
            }
            r.expect(b',')?;
        }
    }
    let json = &r.bytes[start..r.pos];
    let kind = kind.ok_or(WireError::Invalid)?;
    if kind == b"\"paragraph\"" && runs == Some(b"[]") {
        let id: u32 = number(id.ok_or(WireError::Invalid)?)?;
        return record(out, INVALIDATE, &id.to_le_bytes()).map(|()| 1);
    }
    if matches!(kind, b"\"create\"" | b"\"style\"" | b"\"destroy\"") {
        let id: u32 = number(id.ok_or(WireError::Invalid)?)?;
        if kind == b"\"destroy\"" {
            paint.remove(id);
        } else if let Some(style) = style {
            match paint.style(id, style, kind == b"\"style\"", out)? {
                paint::ResultKind::Paint => return Ok(1),
                paint::ResultKind::Unchanged => return Ok(0),
                paint::ResultKind::Cold => {}
            }
            if style_fields == 1 && !escaped_key && !duplicate_key {
                return styles.write(json, style, style_span.expect("one style field"), out);
            }
        } else {
            paint.remove(id);
        }
    }
    let (opcode, arity, property) = match kind {
        b"\"frame\"" => (FRAME, 4usize, 0u8),
        b"\"content\"" => {
            values.swap(0, 2);
            values.swap(1, 3);
            (CONTENT, 2, 0)
        }
        b"\"present\"" => match property {
            Some(b"\"translate\"") => (PRESENT, 2, 1),
            Some(b"\"scale\"") => (PRESENT, 2, 2),
            Some(b"\"rotate\"") => (PRESENT, 2, 3),
            Some(b"\"opacity\"") => (PRESENT, 2, 4),
            Some(b"\"layout\"") => (PRESENT, 4, 5),
            _ => return record(out, JSON, json).map(|()| 1),
        },
        _ => return record(out, JSON, json).map(|()| 1),
    };
    let mut payload = [0u8; 38];
    let id: u32 = number(id.ok_or(WireError::Invalid)?)?;
    payload[..4].copy_from_slice(&id.to_le_bytes());
    let mut pos = 4;
    if opcode == PRESENT {
        payload[4] = property;
        payload[5] = arity as u8;
        pos = 6;
    }
    for value in &values[..arity] {
        let value = value.ok_or(WireError::Invalid)?;
        if opcode == PRESENT {
            let n: f64 = number(value)?;
            if !n.is_finite() {
                return Err(WireError::Invalid);
            }
            payload[pos..pos + 8].copy_from_slice(&n.to_le_bytes());
            pos += 8;
        } else {
            let n: f32 = number(value)?;
            if !n.is_finite() {
                return Err(WireError::Invalid);
            }
            payload[pos..pos + 4].copy_from_slice(&n.to_le_bytes());
            pos += 4;
        }
    }
    record(out, opcode, &payload[..pos]).map(|()| 1)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: u8,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            depth: 0,
        }
    }
    fn whitespace(&mut self) {
        while self
            .bytes
            .get(self.pos)
            .is_some_and(|b| b" \n\r\t".contains(b))
        {
            self.pos += 1;
        }
    }
    fn take(&mut self, byte: u8) -> bool {
        self.whitespace();
        if self.bytes.get(self.pos) == Some(&byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, byte: u8) -> Result<(), WireError> {
        self.take(byte).then_some(()).ok_or(WireError::Invalid)
    }
    fn end(&mut self) -> Result<(), WireError> {
        self.whitespace();
        (self.pos == self.bytes.len())
            .then_some(())
            .ok_or(WireError::Invalid)
    }
    fn string(&mut self) -> Result<&'a [u8], WireError> {
        self.expect(b'"')?;
        let start = self.pos;
        while let Some(&byte) = self.bytes.get(self.pos) {
            self.pos += 1;
            match byte {
                b'"' => return Ok(&self.bytes[start..self.pos - 1]),
                b'\\' => {
                    let escape = *self.bytes.get(self.pos).ok_or(WireError::Invalid)?;
                    self.pos += 1;
                    if escape == b'u' {
                        let end = self.pos.checked_add(4).ok_or(WireError::Invalid)?;
                        let digits = self.bytes.get(self.pos..end).ok_or(WireError::Invalid)?;
                        if !digits.iter().all(u8::is_ascii_hexdigit) {
                            return Err(WireError::Invalid);
                        }
                        self.pos = end;
                    } else if !b"\"\\/bfnrt".contains(&escape) {
                        return Err(WireError::Invalid);
                    }
                }
                0..=31 => return Err(WireError::Invalid),
                _ => {}
            }
        }
        Err(WireError::Invalid)
    }
    fn value(&mut self) -> Result<&'a [u8], WireError> {
        self.whitespace();
        let start = self.pos;
        match self.bytes.get(self.pos) {
            Some(b'"') => {
                self.string()?;
            }
            Some(b'{') => self.container(b'{', b'}', true)?,
            Some(b'[') => self.container(b'[', b']', false)?,
            Some(b't') => self.literal(b"true")?,
            Some(b'f') => self.literal(b"false")?,
            Some(b'n') => self.literal(b"null")?,
            Some(b'-' | b'0'..=b'9') => self.numeric()?,
            _ => return Err(WireError::Invalid),
        }
        Ok(&self.bytes[start..self.pos])
    }
    fn container(&mut self, open: u8, close: u8, object: bool) -> Result<(), WireError> {
        self.expect(open)?;
        self.depth = self.depth.checked_add(1).ok_or(WireError::Invalid)?;
        if self.depth > 64 {
            return Err(WireError::Invalid);
        }
        if !self.take(close) {
            loop {
                if object {
                    self.string()?;
                    self.expect(b':')?;
                }
                self.value()?;
                if self.take(close) {
                    break;
                }
                self.expect(b',')?;
            }
        }
        self.depth -= 1;
        Ok(())
    }
    fn literal(&mut self, literal: &[u8]) -> Result<(), WireError> {
        let end = self
            .pos
            .checked_add(literal.len())
            .ok_or(WireError::Invalid)?;
        if self.bytes.get(self.pos..end) != Some(literal) {
            return Err(WireError::Invalid);
        }
        self.pos = end;
        Ok(())
    }
    fn digits(&mut self) -> Result<(), WireError> {
        let start = self.pos;
        while self.bytes.get(self.pos).is_some_and(u8::is_ascii_digit) {
            self.pos += 1;
        }
        (self.pos != start).then_some(()).ok_or(WireError::Invalid)
    }
    fn numeric(&mut self) -> Result<(), WireError> {
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'0') {
            self.pos += 1;
        } else {
            self.digits()?;
        }
        if self.bytes.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            self.digits()?;
        }
        if self
            .bytes
            .get(self.pos)
            .is_some_and(|b| *b == b'e' || *b == b'E')
        {
            self.pos += 1;
            if self
                .bytes
                .get(self.pos)
                .is_some_and(|b| *b == b'+' || *b == b'-')
            {
                self.pos += 1;
            }
            self.digits()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(out: &[u8]) -> Vec<(u8, &[u8])> {
        let count = u32::from_le_bytes(out[8..12].try_into().unwrap());
        let mut pos = HEADER_BYTES;
        (0..count)
            .map(|_| {
                let op = out[pos];
                let size = u32::from_le_bytes(out[pos + 1..pos + 5].try_into().unwrap()) as usize;
                let payload = &out[pos + 5..pos + 5 + size];
                pos += 5 + size;
                (op, payload)
            })
            .collect()
    }

    // Resolve the new records back into their original cold JSON bytes. Existing
    // semantic/rollback checks then cover both pooled and legacy publications.
    fn legacy(out: &[u8]) -> Vec<u8> {
        let mut styles = BTreeMap::new();
        let mut resolved = out[..HEADER_BYTES].to_vec();
        let mut count = 0u32;
        for (op, payload) in records(out) {
            let slot = || u32::from_le_bytes(payload[..4].try_into().unwrap());
            match op {
                STYLE_DEFINE => {
                    styles.insert(slot(), &payload[4..]);
                }
                STYLE_REFERENCE => {
                    let json = &payload[4..];
                    let mut reader = Reader::new(json);
                    reader.expect(b'{').unwrap();
                    loop {
                        let key = reader.string().unwrap();
                        reader.expect(b':').unwrap();
                        reader.whitespace();
                        let start = reader.pos;
                        reader.value().unwrap();
                        if key == b"style" {
                            let mut original = json[..start].to_vec();
                            original.extend_from_slice(styles[&slot()]);
                            original.extend_from_slice(&json[reader.pos..]);
                            record(&mut resolved, JSON, &original).unwrap();
                            break;
                        }
                        reader.expect(b',').unwrap();
                    }
                    count += 1;
                }
                _ => {
                    record(&mut resolved, op, payload).unwrap();
                    count += 1;
                }
            }
        }
        let metadata = u32::from_le_bytes(out[28..32].try_into().unwrap()) as usize;
        resolved.extend_from_slice(&out[out.len() - metadata..]);
        resolved[8..12].copy_from_slice(&count.to_le_bytes());
        resolved
    }

    #[test]
    fn hot_records_have_exact_fields_and_preserve_cold_ops() {
        let json = r#"{"ops":[{"op":"frame","id":9,"x":1.25,"y":-2,"w":300,"h":40},{"op":"content","id":9,"w":301,"h":800},{"op":"present","id":9,"property":"translate","x":3.5,"y":4},{"op":"style","id":9,"style":{"color":"é"}}],"timers":true,"timer_due_ms":16,"motion":true,"clock":12,"error":null}"#.as_bytes();
        let mut out = Vec::new();
        Encoder::default().encode(json, &mut out).unwrap();
        let out = legacy(&out);
        assert_eq!(&out[..4], b"EXA1");
        assert_eq!(u16::from_le_bytes(out[6..8].try_into().unwrap()), 195);
        assert_eq!(u32::from_le_bytes(out[8..12].try_into().unwrap()), 4);
        let mut pos = HEADER_BYTES;
        for (opcode, length) in [(FRAME, 20), (CONTENT, 12), (PRESENT, 22), (JSON, 47)] {
            assert_eq!(out[pos], opcode);
            let len = u32::from_le_bytes(out[pos + 1..pos + 5].try_into().unwrap()) as usize;
            if opcode != JSON {
                assert_eq!(len, length);
            }
            if opcode == FRAME {
                assert_eq!(
                    f32::from_le_bytes(out[pos + 9..pos + 13].try_into().unwrap()),
                    1.25
                );
            }
            pos += 5 + len;
        }
        let metadata = std::str::from_utf8(&out[pos..]).unwrap();
        assert!(metadata.is_empty());
        assert_eq!(u32::from_le_bytes(out[28..32].try_into().unwrap()), 0);
    }

    #[test]
    fn error_and_unknown_metadata_survive_without_duplicating_header_fields() {
        let mut out = Vec::new();
        Encoder::default().encode(
            br#"{"ops":[],"timers":false,"motion":true,"clock":42,"error":"unsupported canvas","canvasImages":["asset.png"],"future":{"nested":1}}"#,
            &mut out,
        ).unwrap();
        assert_eq!(u16::from_le_bytes(out[6..8].try_into().unwrap()), 66);
        assert_eq!(f64::from_le_bytes(out[12..20].try_into().unwrap()), 42.);
        assert_eq!(
            std::str::from_utf8(&out[HEADER_BYTES..]).unwrap(),
            r#"{"error":"unsupported canvas","canvasImages":["asset.png"],"future":{"nested":1}}"#
        );
    }

    #[test]
    fn malformed_or_truncated_batches_are_refused() {
        let valid = br#"{"ops":[{"op":"frame","id":1,"x":0,"y":0,"w":1,"h":1}],"clock":0}"#;
        let mut encoder = Encoder::default();
        let mut out = Vec::new();
        for end in 0..valid.len() {
            assert_eq!(
                encoder.encode(&valid[..end], &mut out),
                Err(WireError::Invalid)
            );
        }
        for bad in [
            &br#"{"ops":[],"ops":[]}"#[..],
            &br#"{"ops":[{"op":"frame","id":-1,"x":0,"y":0,"w":1,"h":1}]}"#[..],
            &br#"{"ops":[],"clock":1e999}"#[..],
            &br#"{"ops":[],}"#[..],
        ] {
            assert_eq!(encoder.encode(bad, &mut out), Err(WireError::Invalid));
        }
    }

    fn encode(encoder: &mut Encoder, ops: &str) -> Vec<u8> {
        legacy(&encode_raw(encoder, ops))
    }

    fn encode_raw(encoder: &mut Encoder, ops: &str) -> Vec<u8> {
        let mut out = Vec::new();
        encoder
            .encode(format!("{{\"ops\":[{ops}]}}").as_bytes(), &mut out)
            .unwrap();
        out
    }

    #[test]
    fn paint_deltas_preserve_appearance_and_clear_to_platform_defaults() {
        let mut encoder = Encoder::default();
        let create = r#"{"op":"create","id":4,"style":{"font_size":16,"background_color":[255,255,255,255],"text_color":[[1,2,3,255],[4,5,6,255]]}}"#;
        assert_eq!(encode(&mut encoder, create)[HEADER_BYTES], JSON);
        let paint = r#"{"op":"style","id":4,"style":{"font_size":16,"background_color":[200,210,220,255],"text_color":[[1,2,3,255],[4,5,6,255]]}}"#;
        let out = encode(&mut encoder, paint);
        assert_eq!(out.len(), HEADER_BYTES + 5 + 16);
        assert_eq!(out[HEADER_BYTES], PAINT);
        assert_eq!(u32::from_le_bytes(out[41..45].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(out[45..49].try_into().unwrap()),
            0xffc8d2dc
        );
        assert_eq!(
            u32::from_le_bytes(out[49..53].try_into().unwrap()),
            0xffc8d2dc
        );
        assert_eq!(encode(&mut encoder, paint).len(), HEADER_BYTES);
        let pair = encode(
            &mut encoder,
            r#"{"op":"style","id":4,"style":{"font_size":16,"background_color":[200,210,220,255],"text_color":[[7,8,9,255],[10,11,12,255]]}}"#,
        );
        assert_eq!(u32::from_le_bytes(pair[41..45].try_into().unwrap()), 2);
        assert_eq!(
            u32::from_le_bytes(pair[45..49].try_into().unwrap()),
            0xff070809
        );
        assert_eq!(
            u32::from_le_bytes(pair[49..53].try_into().unwrap()),
            0xff0a0b0c
        );
        let cleared = encode(
            &mut encoder,
            r#"{"op":"style","id":4,"style":{"font_size":16}}"#,
        );
        assert_eq!(u32::from_le_bytes(cleared[41..45].try_into().unwrap()), 3);
        assert_eq!(&cleared[45..53], &[0; 8]);
        assert_eq!(
            u32::from_le_bytes(cleared[53..57].try_into().unwrap()),
            0xff000000
        );
    }

    #[test]
    fn paint_cache_keeps_nonpaint_changes_cold_and_follows_node_lifetime() {
        let mut encoder = Encoder::default();
        let first =
            r#"{"op":"style","id":1,"style":{"font_size":16,"background_color":[1,2,3,255]}}"#;
        assert_eq!(encode(&mut encoder, first)[HEADER_BYTES], JSON);
        let changed =
            r#"{"op":"style","id":1,"style":{"font_size":18,"background_color":[2,3,4,255]}}"#;
        assert_eq!(encode(&mut encoder, changed)[HEADER_BYTES], JSON);
        assert_eq!(
            encode(
                &mut encoder,
                r#"{"op":"style","id":1,"style":{"background_color":[2,3,4,255]}}"#
            )[HEADER_BYTES],
            JSON
        );
        let unknown = r#"{"op":"style","id":1,"style":{"background_color":"unsupported"}}"#;
        assert_eq!(encode(&mut encoder, unknown)[HEADER_BYTES], JSON);
        let alias = r#"{"op":"style","id":1,"style":{"background_color":[2,3,4,255],"background\u005fcolor":[9,8,7,255]}}"#;
        assert_eq!(encode(&mut encoder, alias)[HEADER_BYTES], JSON);
        assert_eq!(encode(&mut encoder, first)[HEADER_BYTES], JSON);
        encode(&mut encoder, r#"{"op":"destroy","id":1}"#);
        assert_eq!(encode(&mut encoder, first)[HEADER_BYTES], JSON);
        // Reused create ids replace the baseline even when the dictionary matches.
        assert_eq!(
            encode(
                &mut encoder,
                r#"{"op":"create","id":1,"style":{"font_size":16,"background_color":[1,2,3,255]}}"#
            )[HEADER_BYTES],
            JSON
        );
        assert_eq!(encode(&mut encoder, first).len(), HEADER_BYTES);
    }

    #[test]
    fn truncated_batches_cannot_advance_the_paint_cache() {
        let mut encoder = Encoder::default();
        let initial = r#"{"op":"create","id":1,"style":{"background_color":[1,2,3,255]}}"#;
        let next = r#"{"op":"style","id":1,"style":{"background_color":[4,5,6,255]}}"#;
        encode(&mut encoder, initial);
        let batch = format!(
            "{{\"ops\":[{next},{{\"op\":\"destroy\",\"id\":1}},{{\"op\":\"create\",\"id\":1,\"style\":{{}}}}]}}"
        );
        let mut out = Vec::new();
        for end in 0..batch.len() {
            assert!(encoder.encode(&batch.as_bytes()[..end], &mut out).is_err());
            assert_eq!(encode(&mut encoder, next)[HEADER_BYTES], PAINT);
            encode(&mut encoder, initial);
        }
    }

    #[test]
    fn memo_hits_require_valid_enclosing_grammar_and_rollback_view_baselines() {
        let a = r#"{"font_size":16,"background_color":[1,2,3,255]}"#;
        let b = r#"{"font_size":16,"background_color":[4,5,6,255]}"#;
        let create_a = format!(r#"{{"op":"create","id":1,"style":{a}}}"#);
        let paint_b = format!(r#"{{"op":"style","id":1,"style":{b}}}"#);
        let mut reference = Encoder::default();
        encode(&mut reference, &create_a);
        let expected = encode(&mut reference, &paint_b);
        assert_eq!(expected[HEADER_BYTES], PAINT);

        let mut encoder = Encoder::default();
        encode(&mut encoder, &create_a);
        // Seed B independently: the failed batch below hits both memo entries.
        let create_b = format!(r#"{{"op":"create","id":2,"style":{b}}}"#);
        encode(&mut encoder, &create_b);
        let mut out = Vec::new();
        for suffix in ["garbage", ":0", ",", r#","bad":"\q""#] {
            let invalid = format!(
                r#"{{"ops":[{paint_b},{{"op":"destroy","id":2}},{{"op":"create","id":3,"style":{a}}},{{"op":"style","id":4,"style":{b}{suffix}}}]}}"#
            );
            assert_eq!(
                encoder.encode(invalid.as_bytes(), &mut out),
                Err(WireError::Invalid)
            );
            assert_eq!(encode(&mut encoder, &paint_b), expected);
            assert_eq!(
                encode(
                    &mut encoder,
                    &format!(r#"{{"op":"style","id":2,"style":{b}}}"#)
                )
                .len(),
                HEADER_BYTES
            );
            // Create 3 must also have rolled back, despite its style memo hit.
            let third = format!(r#"{{"op":"style","id":3,"style":{a}}}"#);
            assert_eq!(encode(&mut encoder, &third)[HEADER_BYTES], JSON);
            encode(&mut encoder, r#"{"op":"destroy","id":3}"#);
            encode(&mut encoder, &create_a);
        }
        let c = r#"{"font_size":18,"background_color":[4,5,6,255]}"#;
        encode(
            &mut encoder,
            &format!(r#"{{"op":"create","id":5,"style":{c}}}"#),
        );
        // A memoized non-paint replacement also rolls back its shared baseline.
        let invalid = format!(r#"{{"ops":[{{"op":"style","id":1,"style":{c}}}],"clock":1e999}}"#);
        assert_eq!(
            encoder.encode(invalid.as_bytes(), &mut out),
            Err(WireError::Invalid)
        );
        assert_eq!(encode(&mut encoder, &paint_b), expected);
    }

    #[test]
    fn memoized_nonpaint_changes_and_ambiguous_styles_keep_cold_wire_bytes() {
        let mut encoder = Encoder::default();
        let a = r#"{"font_size":16,"background_color":[1,2,3,255]}"#;
        let b = r#"{"font_size":18,"background_color":[4,5,6,255]}"#;
        for (id, style) in [(1, a), (2, b)] {
            encode(
                &mut encoder,
                &format!(r#"{{"op":"create","id":{id},"style":{style}}}"#),
            );
        }
        // Both non-paint signatures are memo hits. Changing between them must
        // preserve the complete cold operation rather than emit only colors.
        for style in [b, a, b] {
            let op = format!(r#"{{"op":"style","id":1,"style":{style}}}"#);
            let bytes = encode(&mut encoder, &op);
            assert_eq!(bytes[HEADER_BYTES], JSON);
            assert_eq!(&bytes[HEADER_BYTES + 5..], op.as_bytes());
        }
        for style in [
            r#"{"background_color":"unsupported"}"#,
            r#"{"background_color":[1,2,3,255],"background_color":[4,5,6,255]}"#,
            r#"{"background\u005fcolor":[1,2,3,255]}"#,
        ] {
            let op = format!(r#"{{"op":"style","id":1,"style":{style}}}"#);
            for _ in 0..2 {
                let bytes = encode(&mut encoder, &op);
                assert_eq!(bytes[HEADER_BYTES], JSON);
                assert_eq!(&bytes[HEADER_BYTES + 5..], op.as_bytes());
            }
        }
        // Unsupported styles retire the baseline even if the next signature
        // was already memoized; it must establish itself with a cold record.
        assert_eq!(
            encode(
                &mut encoder,
                &format!(r#"{{"op":"style","id":1,"style":{a}}}"#)
            )[HEADER_BYTES],
            JSON
        );
    }

    #[test]
    fn unchanged_capacity_and_layout_present_record() {
        let json = br#"{"ops":[{"op":"present","id":42,"property":"layout","x":1,"y":2,"w":3,"h":4}],"motion":true}"#;
        let mut encoder = Encoder::default();
        let mut out = Vec::new();
        encoder.encode(json, &mut out).unwrap();
        let ptr = out.as_ptr();
        let bytes = out.clone();
        encoder.encode(json, &mut out).unwrap();
        assert_eq!(ptr, out.as_ptr());
        assert_eq!(out, bytes);
        assert_eq!(out[HEADER_BYTES], PRESENT);
        assert_eq!(out[HEADER_BYTES + 9], 5);
        assert_eq!(out[HEADER_BYTES + 10], 4);
    }

    #[test]
    fn sibling_style_definitions_preserve_complete_cold_bytes() {
        let style = r#"{"font_size":17,"padding_left":3,"text_color":[[1,2,3,255],[4,5,6,255]],"transform_origin":[{"pct":25,"px":2},"center"]}"#;
        let ops = (1..=1000)
            .map(|id| format!(r#"{{"op":"create","id":{id},"kind":"text","props":{{"text":"é 👩‍🚀 style:{{}}"}},"style":{style},"handlers":[]}}"#))
            .collect::<Vec<_>>().join(",");
        let out = encode_raw(&mut Encoder::default(), &ops);
        let emitted = records(&out);
        assert_eq!(emitted.len(), 1001);
        assert_eq!(
            emitted[0],
            (
                STYLE_DEFINE,
                [&0u32.to_le_bytes()[..], style.as_bytes()]
                    .concat()
                    .as_slice()
            )
        );
        assert!(emitted[1..]
            .iter()
            .all(|(op, payload)| *op == STYLE_REFERENCE && payload[..4] == [0; 4]));
        let legacy = legacy(&out);
        let original: Vec<_> = ops.split("},{\"op\"").collect();
        assert_eq!(original.len(), 1000);
        assert_eq!(u32::from_le_bytes(legacy[8..12].try_into().unwrap()), 1000);
        let restored: Vec<_> = records(&legacy)
            .into_iter()
            .map(|(_, bytes)| std::str::from_utf8(bytes).unwrap())
            .collect();
        assert_eq!(restored.join(","), ops);
        assert!(out.len() + style.len() * 900 < legacy.len());
    }

    #[test]
    fn style_pool_is_exact_byte_bounded_and_falls_back_in_mixed_batches() {
        let ops = (1..=130)
            .map(|id| format!(r#"{{"op":"create","id":{id},"style":{{"font_size":{id}}}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let out = encode_raw(&mut Encoder::default(), &ops);
        let emitted = records(&out);
        assert_eq!(
            emitted.iter().filter(|(op, _)| *op == STYLE_DEFINE).count(),
            STYLE_SLOTS
        );
        assert_eq!(
            emitted
                .iter()
                .filter(|(op, _)| *op == STYLE_REFERENCE)
                .count(),
            STYLE_SLOTS
        );
        assert_eq!(emitted.iter().filter(|(op, _)| *op == JSON).count(), 2);

        let big = format!(r#"{{"extra":"{}"}}"#, "x".repeat(40_000));
        let other = format!(r#"{{"extra":"{}"}}"#, "y".repeat(40_000));
        let ops = format!(
            r#"{{"op":"create","id":1,"style":{big}}},{{"op":"create","id":2,"style":{other}}},{{"op":"create","id":3,"style":{big}}}"#
        );
        let out = encode_raw(&mut Encoder::default(), &ops);
        assert_eq!(
            records(&out).iter().map(|(op, _)| *op).collect::<Vec<_>>(),
            [STYLE_DEFINE, STYLE_REFERENCE, JSON, STYLE_REFERENCE]
        );
        let huge = format!(
            r#"{{"op":"create","id":1,"style":{{"extra":"{}"}}}}"#,
            "z".repeat(STYLE_BYTES)
        );
        let out = encode_raw(&mut Encoder::default(), &huge);
        assert_eq!(records(&out), vec![(JSON, huge.as_bytes())]);

        let ops = r#"{"op":"create","id":1,"style":{"font_size":16}},{"op":"create","id":2,"style":{ "font_size":16 }}"#;
        assert_eq!(
            records(&encode_raw(&mut Encoder::default(), ops))
                .iter()
                .filter(|(op, _)| *op == STYLE_DEFINE)
                .count(),
            2
        );
    }

    #[test]
    fn ambiguous_top_level_keys_keep_original_cold_dictionary() {
        for op in [
            r#"{"op":"create","id":1,"style":{"font_size":16},"style":{"font_size":18}}"#,
            r#"{"op":"create","id":1,"style":{"font_size":16},"st\u0079le":{"font_size":18}}"#,
            r#"{"op":"create","id":1,"id":2,"style":{"font_size":16}}"#,
        ] {
            assert_eq!(
                records(&encode_raw(&mut Encoder::default(), op)),
                vec![(JSON, op.as_bytes())]
            );
        }
    }

    #[test]
    fn pooled_styles_reset_each_batch_and_keep_transactional_paint_baselines() {
        let a = r#"{"font_size":16,"background_color":[1,2,3,255]}"#;
        let b = r#"{"font_size":18,"background_color":[4,5,6,255]}"#;
        let create = |id, style| format!(r#"{{"op":"create","id":{id},"style":{style}}}"#);
        let mut encoder = Encoder::default();
        for (id, style) in [(1, a), (2, b), (3, a)] {
            let out = encode_raw(&mut encoder, &create(id, style));
            let emitted = records(&out);
            assert_eq!(emitted.len(), 2);
            assert_eq!(emitted[0].0, STYLE_DEFINE);
            assert_eq!(&emitted[0].1[..4], &[0; 4]);
            assert_eq!(&emitted[0].1[4..], style.as_bytes());
        }
        let failed = format!(r#"{{"ops":[{{"op":"style","id":1,"style":{b}}}],"clock":1e999}}"#);
        let mut out = Vec::new();
        assert_eq!(
            encoder.encode(failed.as_bytes(), &mut out),
            Err(WireError::Invalid)
        );
        let paint =
            r#"{"op":"style","id":1,"style":{"font_size":16,"background_color":[7,8,9,255]}}"#;
        assert_eq!(records(&encode_raw(&mut encoder, paint))[0].0, PAINT);
        // A new publication must define again even after the failed batch had
        // already emitted a definition into its discarded output.
        let out = encode_raw(&mut encoder, &create(4, b));
        assert_eq!(records(&out)[0].0, STYLE_DEFINE);
        assert_eq!(&records(&out)[0].1[..4], &[0; 4]);
    }
}
