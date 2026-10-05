//! A portable, noncryptographic 64-bit streaming hash. Eight-byte little-endian
//! lanes use SplitMix64's published avalanche multipliers (Vigna, 2015).
//! Tags frame values; record field names are absent (map keys, and the fields a
//! record written over its default keeps, are present), enum ordinals are present.
//! NaNs use IEEE's positive quiet NaN; negative zero retains its sign bit.
use super::{f32_bits, f64_bits, Data, Number, Writer};

/// Hash the values in declaration order.
pub fn of<T: Data>(value: &T) -> u64 {
    let mut w = Hasher::default();
    value.write(&mut w);
    w.finish()
}

/// Incremental value hash; not suitable for authentication.
#[derive(Clone, Default)]
pub struct Hasher {
    stream: Stream,
}

#[derive(Clone)]
struct Stream {
    state: u64,
    tail: [u8; 8],
    used: usize,
    len: u64,
}
impl Default for Stream {
    fn default() -> Self {
        Self {
            state: 0x9e37_79b9_7f4a_7c15,
            tail: [0; 8],
            used: 0,
            len: 0,
        }
    }
}
fn mix(mut n: u64) -> u64 {
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    n ^ (n >> 31)
}
impl Stream {
    fn lane(&mut self, n: u64) {
        self.state = mix(self.state ^ n).rotate_left(27);
    }
    fn bytes(&mut self, mut b: &[u8]) {
        self.len = self.len.wrapping_add(b.len() as u64);
        if self.used != 0 {
            let n = (8 - self.used).min(b.len());
            self.tail[self.used..self.used + n].copy_from_slice(&b[..n]);
            self.used += n;
            b = &b[n..];
            if self.used == 8 {
                self.lane(u64::from_le_bytes(self.tail));
                self.used = 0;
                self.tail = [0; 8];
            }
        }
        while b.len() >= 8 {
            self.lane(u64::from_le_bytes(b[..8].try_into().unwrap()));
            b = &b[8..];
        }
        if !b.is_empty() {
            self.tail[..b.len()].copy_from_slice(b);
            self.used = b.len();
        }
    }
    /// Finalize without changing the writer, so snapshots are cheap.
    pub fn finish(&self) -> u64 {
        mix(self.state ^ mix(u64::from_le_bytes(self.tail)) ^ self.len)
    }
}
impl Hasher {
    fn raw(&mut self, bytes: &[u8]) {
        self.stream.bytes(bytes);
    }
    /// Finalize the canonical stream without changing it.
    pub fn finish(&self) -> u64 {
        self.stream.finish()
    }
}
impl Writer for Hasher {
    fn digests(&self) -> bool {
        true
    }
    fn bytes(&mut self, value: super::Bulk<'_>) {
        self.raw(&[17 + value.kind() as u8]);
        self.raw(&(value.byte_len() as u64).to_le_bytes());
        value.write_bytes(|bytes| self.raw(bytes));
    }
    fn boolean(&mut self, n: bool) {
        self.raw(&[u8::from(n)]);
    }
    fn number(&mut self, n: Number) {
        match n {
            Number::Unsigned(n) => {
                self.raw(&[2]);
                self.raw(&n.to_le_bytes());
            }
            Number::Signed(n) => {
                self.raw(&[3]);
                self.raw(&n.to_le_bytes());
            }
            Number::F32(n) => {
                self.raw(&[4]);
                self.raw(&f32_bits(n).to_le_bytes());
            }
            Number::F64(n) => {
                self.raw(&[5]);
                self.raw(&f64_bits(n).to_le_bytes());
            }
        }
    }
    fn string(&mut self, s: &str) {
        self.raw(&[6]);
        self.raw(&(s.len() as u64).to_le_bytes());
        self.raw(s.as_bytes());
    }
    fn begin_seq(&mut self, len: usize) {
        self.raw(&[7]);
        self.raw(&(len as u64).to_le_bytes());
    }
    fn item(&mut self) {}
    fn end_seq(&mut self) {
        self.raw(&[12]);
    }
    fn begin_struct(&mut self) {
        self.raw(&[8]);
    }
    fn field(&mut self, _: &str) {}
    fn key(&mut self, key: &str) {
        self.string(key);
    }
    fn end_struct(&mut self) {
        self.raw(&[13]);
    }
    fn variant(&mut self, _: &str, index: u32) {
        self.raw(&[9]);
        self.raw(&index.to_le_bytes());
    }
    fn end_variant(&mut self) {
        self.raw(&[14]);
    }
    fn option(&mut self, some: bool) {
        self.raw(&[if some { 11 } else { 10 }]);
    }
    fn end_option(&mut self) {
        self.raw(&[15]);
    }
}
