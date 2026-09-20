//! Bounded inspection output only. Request parsing belongs to adapters.
use super::{BulkKind, Data, DataError, Number, Writer};
use std::fmt::Write;
/// Maximum output bytes and value visits for one inspection.
pub const LIMIT: usize = 65_536;
pub fn to_string<T: Data>(value: &T) -> Result<String, DataError> {
    let mut w = Encoder::default();
    value.write(&mut w);
    w.finish()
}
#[derive(Default)]
pub struct Encoder {
    text: String,
    frames: Vec<usize>,
    error: Option<DataError>,
    visits: usize,
    rounded: bool,
}
impl Encoder {
    pub fn rounded() -> Self {
        Self {
            rounded: true,
            ..Self::default()
        }
    }
    pub fn finish(self) -> Result<String, DataError> {
        self.error.map_or(Ok(self.text), Err)
    }
    fn fail(&mut self, reason: &str) {
        if self.error.is_none() {
            self.error = Some(DataError::new(reason));
        }
    }
    fn push(&mut self, text: &str) {
        self.visits = self.visits.saturating_add(1);
        if self.text.len().saturating_add(text.len()) > LIMIT || self.visits > LIMIT {
            self.fail("inspection exceeds 65536 bytes/visits");
        }
        if self.error.is_none() {
            self.text.push_str(text);
        }
    }
    fn quote(&mut self, s: &str) {
        if s.len() > LIMIT {
            self.fail("inspection string exceeds limit");
            return;
        }
        self.push("\"");
        for ch in s.chars() {
            if self.error.is_some() {
                break;
            }
            match ch {
                '"' => self.push("\\\""),
                '\\' => self.push("\\\\"),
                '\n' => self.push("\\n"),
                '\r' => self.push("\\r"),
                '\t' => self.push("\\t"),
                c if c < ' ' => self.push(&format!("\\u{:04x}", c as u32)),
                c => self.push(c.encode_utf8(&mut [0; 4])),
            }
        }
        self.push("\"");
    }
    fn begin(&mut self, text: &str) {
        if self.frames.len() == 256 {
            self.fail("inspection nesting exceeds 256");
        }
        if self.error.is_none() {
            self.frames.push(0);
            self.push(text);
        }
    }
    fn end(&mut self, text: &str) {
        if self.error.is_none() {
            self.frames.pop();
            self.push(text);
        }
    }
}
impl Writer for Encoder {
    fn unit(&mut self) {
        self.push("null");
    }
    fn bytes(&mut self, kind: BulkKind, value: &[u8]) {
        if value.len() > LIMIT {
            self.fail("inspection bulk exceeds limit");
            return;
        }
        let mut hash = super::hash::Hasher::default();
        hash.bytes(kind, value);
        self.push(&format!(
            "{{\"bytes\":{},\"hash\":\"0x{:016x}\"}}",
            value.len(),
            hash.finish()
        ));
    }
    fn boolean(&mut self, n: bool) {
        self.push(if n { "true" } else { "false" });
    }
    fn number(&mut self, n: Number) {
        let mut text = String::new();
        match n {
            Number::Unsigned(n) => write!(text, "{n}").unwrap(),
            Number::Signed(n) => write!(text, "{n}").unwrap(),
            Number::F32(n) if n.is_finite() => {
                if self.rounded {
                    super::text::shortest(&mut text, rounded(n as f64), false).unwrap();
                } else {
                    super::text::shortest(&mut text, n, true).unwrap();
                }
            }
            Number::F64(n) if n.is_finite() => super::text::shortest(
                &mut text,
                if self.rounded { rounded(n) } else { n },
                !self.rounded,
            )
            .unwrap(),
            _ => self.fail("non-finite number is not JSON"),
        }
        self.push(&text);
    }
    fn string(&mut self, s: &str) {
        self.quote(s);
    }
    fn begin_seq(&mut self, len: usize) {
        if len > LIMIT {
            self.fail("inspection sequence exceeds limit");
        }
        self.begin("[");
    }
    fn item(&mut self) {
        if self.error.is_some() {
            return;
        }
        let count = self.frames.last_mut().expect("Data container");
        let comma = *count != 0;
        *count += 1;
        if comma {
            self.push(",");
        }
    }
    fn end_seq(&mut self) {
        self.end("]");
    }
    fn begin_struct(&mut self) {
        self.begin("{");
    }
    fn field(&mut self, name: &str) {
        self.item();
        self.quote(name);
        self.push(":");
    }
    fn end_struct(&mut self) {
        self.end("}");
    }
    fn variant(&mut self, name: &str, _: u32) {
        self.begin_struct();
        self.field(name);
    }
    fn end_variant(&mut self) {
        self.end_struct();
    }
    fn option(&mut self, some: bool) {
        self.begin_seq(usize::from(some));
        if some {
            self.item();
        }
    }
    fn end_option(&mut self) {
        self.end_seq();
    }
}
pub(crate) fn rounded(n: f64) -> f64 {
    let scaled = n * 10000.;
    if scaled.is_finite() {
        scaled.round() / 10000.
    } else {
        n
    }
}
