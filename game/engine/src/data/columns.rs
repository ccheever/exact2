//! Columnar rows for world saves. Each row's Data walk splits into a shape (tags,
//! field and variant names, sequence lengths) and the scalars it carries. Rows of
//! one shape form a group whose scalars are columns, each run-length or dictionary
//! coded; a group that repeats a few distinct rows stores them once. Names are
//! interned once per payload, so nothing is repeated per row. Encoding is a pure
//! function of the rows: byte-identical on every host.
use super::limits::MAX_LOAD_STRING;
use super::{f32_bits, f64_bits, Bulk, BulkKind, DataError, Number, Reader, Writer};
use std::collections::BTreeMap;

const END: u8 = 0;
const FIELD: u8 = 1;
const UNSIGNED: u8 = 2;
const SIGNED: u8 = 3;
const F32: u8 = 4;
const F64: u8 = 5;
const STRING: u8 = 6;
const SEQ: u8 = 7;
const STRUCT: u8 = 8;
const VARIANT: u8 = 9;
const NONE: u8 = 10;
const SOME: u8 = 11;
const BULK: u8 = 12; // + BulkKind
const BOOL: u8 = 16;

fn var(out: &mut Vec<u8>, mut n: u64) {
    while n >= 128 {
        out.push(n as u8 | 128);
        n >>= 7;
    }
    out.push(n as u8);
}
fn zigzag(n: i64) -> u64 {
    ((n as u64) << 1) ^ ((n >> 63) as u64)
}

/// Collects rows of one storage, in order.
#[derive(Default)]
pub(crate) struct Columns {
    names: BTreeMap<String, u64>,
    shape_ids: BTreeMap<Vec<u8>, u64>,
    shapes: Vec<Vec<u8>>,
    groups: Vec<Group>,
    row_shapes: Vec<u64>,
    shape: Vec<u8>,
    nums: Vec<(u8, u64)>,
    blobs: Vec<Vec<u8>>,
    // Runs of row indices, (gap from the previous run's end, length).
    runs: Vec<(u32, u32)>,
    end: u32,
}
#[derive(Default)]
struct Group {
    rows: usize,
    kinds: Vec<u8>,
    nums: Vec<Vec<u64>>,
    blobs: Vec<Vec<Vec<u8>>>,
}
impl Columns {
    /// Record the row of an ascending entity index; the payload keeps index runs.
    pub(crate) fn row_at(&mut self, index: u32, write: impl FnOnce(&mut dyn Writer)) {
        match self.runs.last_mut() {
            Some((_, len)) if index == self.end => *len += 1,
            _ => self.runs.push((index - self.end, 1)),
        }
        self.end = index + 1;
        self.row(write);
    }
    /// Record one row through its Data walk.
    pub(crate) fn row(&mut self, write: impl FnOnce(&mut dyn Writer)) {
        self.shape.clear();
        self.nums.clear();
        self.blobs.clear();
        write(&mut Recorder(self));
        let next = self.shapes.len() as u64;
        let id = match self.shape_ids.get(&self.shape) {
            Some(&id) => id,
            None => {
                self.shape_ids.insert(self.shape.clone(), next);
                next
            }
        };
        if id == next {
            self.shapes.push(self.shape.clone());
            self.groups.push(Group {
                kinds: self.nums.iter().map(|&(k, _)| k).collect(),
                nums: vec![Vec::new(); self.nums.len()],
                blobs: vec![Vec::new(); self.blobs.len()],
                ..Group::default()
            });
        }
        self.row_shapes.push(id);
        let group = &mut self.groups[id as usize];
        group.rows += 1;
        for (column, &(_, n)) in group.nums.iter_mut().zip(&self.nums) {
            column.push(n);
        }
        for (column, blob) in group.blobs.iter_mut().zip(self.blobs.drain(..)) {
            column.push(blob);
        }
    }
    fn name(&mut self, name: &str) -> u64 {
        let next = self.names.len() as u64;
        *self.names.entry(name.into()).or_insert(next)
    }
    pub(crate) fn finish(self) -> Vec<u8> {
        let mut out = Vec::new();
        var(&mut out, self.runs.len() as u64);
        for &(gap, len) in &self.runs {
            var(&mut out, gap.into());
            var(&mut out, len.into());
        }
        let mut names = vec![""; self.names.len()];
        for (name, &i) in &self.names {
            names[i as usize] = name;
        }
        var(&mut out, names.len() as u64);
        for name in names {
            var(&mut out, name.len() as u64);
            out.extend_from_slice(name.as_bytes());
        }
        var(&mut out, self.shapes.len() as u64);
        for shape in &self.shapes {
            var(&mut out, shape.len() as u64);
            out.extend_from_slice(shape);
        }
        var(&mut out, self.row_shapes.len() as u64);
        if self.shapes.len() > 1 {
            column(&mut out, UNSIGNED, &self.row_shapes);
        }
        for group in &self.groups {
            let mut plain = Vec::new();
            columns(&mut plain, &group.kinds, &group.nums);
            for blobs in &group.blobs {
                blob_runs(&mut plain, blobs);
            }
            match group
                .blobs
                .is_empty()
                .then(|| distinct_rows(group))
                .flatten()
            {
                Some(mut packed) if packed.len() < plain.len() => {
                    out.push(1);
                    out.append(&mut packed);
                }
                _ => {
                    out.push(0);
                    out.append(&mut plain);
                }
            }
        }
        out
    }
}
fn columns(out: &mut Vec<u8>, kinds: &[u8], nums: &[Vec<u64>]) {
    for (&kind, values) in kinds.iter().zip(nums) {
        column(out, kind, values);
    }
}
// A group that repeats at most a quarter as many distinct rows (and at most
// 4,096): the distinct rows as columns, then each row's index into them.
fn distinct_rows(group: &Group) -> Option<Vec<u8>> {
    if group.nums.is_empty() || group.rows < 8 {
        return None;
    }
    let limit = (group.rows / 4).min(4096);
    let mut index = BTreeMap::<Vec<u64>, u64>::new();
    let mut rows = Vec::with_capacity(group.rows);
    let mut key = Vec::with_capacity(group.nums.len());
    for row in 0..group.rows {
        key.clear();
        key.extend(group.nums.iter().map(|c| c[row]));
        let next = index.len() as u64;
        let id = match index.get(&key) {
            Some(&id) => id,
            None => {
                if index.len() == limit {
                    return None;
                }
                index.insert(key.clone(), next);
                next
            }
        };
        rows.push(id);
    }
    let mut table = vec![vec![0; index.len()]; group.nums.len()];
    for (key, &id) in &index {
        for (column, &n) in table.iter_mut().zip(key) {
            column[id as usize] = n;
        }
    }
    let mut out = Vec::new();
    var(&mut out, index.len() as u64);
    columns(&mut out, &group.kinds, &table);
    column(&mut out, UNSIGNED, &rows);
    Some(out)
}
fn value(out: &mut Vec<u8>, kind: u8, n: u64) {
    match kind {
        F32 => out.extend_from_slice(&(n as u32).to_le_bytes()),
        F64 => out.extend_from_slice(&n.to_le_bytes()),
        _ => var(out, n),
    }
}
// Runs: a header n << 1 | repeated, then one value repeated n times or n values.
fn runs(out: &mut Vec<u8>, kind: u8, values: &[u64]) {
    let mut at = 0;
    while at < values.len() {
        let same = values[at..]
            .iter()
            .take_while(|&&v| v == values[at])
            .count();
        if same >= 3 {
            var(out, (same as u64) << 1 | 1);
            value(out, kind, values[at]);
            at += same;
            continue;
        }
        let mut end = at;
        while end < values.len() {
            let same = values[end..]
                .iter()
                .take_while(|&&v| v == values[end])
                .count();
            if same >= 3 {
                break;
            }
            end += same;
        }
        var(out, ((end - at) as u64) << 1);
        for &v in &values[at..end] {
            value(out, kind, v);
        }
        at = end;
    }
}
// Mode 0: runs of values. Mode 1: up to 256 distinct values, then runs of indices.
fn column(out: &mut Vec<u8>, kind: u8, values: &[u64]) {
    let mut plain = vec![0];
    runs(&mut plain, kind, values);
    let mut index = BTreeMap::<u64, u64>::new();
    let mut table = Vec::new();
    let mut ids = Vec::with_capacity(values.len());
    for &v in values {
        let next = table.len() as u64;
        let id = *index.entry(v).or_insert(next);
        if id == next {
            if table.len() == 256 {
                out.append(&mut plain);
                return;
            }
            table.push(v);
        }
        ids.push(id);
    }
    let mut packed = vec![1];
    var(&mut packed, table.len() as u64);
    for &v in &table {
        value(&mut packed, kind, v);
    }
    runs(&mut packed, UNSIGNED, &ids);
    out.append(if packed.len() < plain.len() {
        &mut packed
    } else {
        &mut plain
    });
}
fn blob_runs(out: &mut Vec<u8>, values: &[Vec<u8>]) {
    let mut at = 0;
    while at < values.len() {
        let same = values[at..]
            .iter()
            .take_while(|v| **v == values[at])
            .count();
        let (n, repeated) = if same >= 2 { (same, 1) } else { (1, 0) };
        var(out, (n as u64) << 1 | repeated);
        for v in &values[at..at + if repeated == 1 { 1 } else { n }] {
            var(out, v.len() as u64);
            out.extend_from_slice(v);
        }
        at += n;
    }
}

struct Recorder<'a>(&'a mut Columns);
impl Recorder<'_> {
    fn token(&mut self, t: u8) {
        self.0.shape.push(t);
    }
    fn num(&mut self, kind: u8, n: u64) {
        self.token(kind);
        self.0.nums.push((kind, n));
    }
    fn named(&mut self, name: &str) {
        let id = self.0.name(name);
        var(&mut self.0.shape, id);
    }
}
impl Writer for Recorder<'_> {
    fn boolean(&mut self, value: bool) {
        self.num(BOOL, value.into());
    }
    fn number(&mut self, n: Number) {
        match n {
            Number::Unsigned(n) => self.num(UNSIGNED, n),
            Number::Signed(n) => self.num(SIGNED, zigzag(n)),
            Number::F32(n) => self.num(F32, f32_bits(n).into()),
            Number::F64(n) => self.num(F64, f64_bits(n)),
        }
    }
    fn bytes(&mut self, value: Bulk<'_>) {
        self.token(BULK + value.kind() as u8);
        let mut blob = Vec::with_capacity(value.byte_len());
        value.write_bytes(|b| blob.extend_from_slice(b));
        self.0.blobs.push(blob);
    }
    fn string(&mut self, value: &str) {
        self.token(STRING);
        self.0.blobs.push(value.as_bytes().to_vec());
    }
    fn begin_seq(&mut self, len: usize) {
        self.token(SEQ);
        var(&mut self.0.shape, len as u64);
    }
    fn item(&mut self) {}
    fn end_seq(&mut self) {}
    fn begin_struct(&mut self) {
        self.token(STRUCT);
    }
    fn field(&mut self, name: &str) {
        self.token(FIELD);
        self.named(name);
    }
    fn end_struct(&mut self) {
        self.token(END);
    }
    fn variant(&mut self, name: &str, index: u32) {
        self.token(VARIANT);
        var(&mut self.0.shape, index.into());
        self.named(name);
    }
    fn end_variant(&mut self) {}
    fn option(&mut self, some: bool) {
        self.token(if some { SOME } else { NONE });
    }
    fn end_option(&mut self) {}
}

/// A checked cursor over one payload.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn err(&self, what: &str) -> DataError {
        DataError::new(format!("{what} at column byte {}", self.pos))
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], DataError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.bytes.len());
        let end = end.ok_or_else(|| self.err("truncated column"))?;
        let b = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(b)
    }
    fn byte(&mut self) -> Result<u8, DataError> {
        Ok(self.take(1)?[0])
    }
    fn var(&mut self) -> Result<u64, DataError> {
        let mut n = 0;
        for shift in (0..70).step_by(7) {
            let b = self.byte()?;
            if shift == 63 && b > 1 {
                return Err(self.err("varint overflow"));
            }
            n |= u64::from(b & 127) << shift;
            if b < 128 {
                return Ok(n);
            }
        }
        Err(self.err("varint overflow"))
    }
    fn len(&mut self) -> Result<usize, DataError> {
        let n = self.var()?;
        usize::try_from(n)
            .ok()
            .filter(|&n| n <= self.bytes.len() - self.pos)
            .ok_or_else(|| self.err("length exceeds the payload"))
    }
    fn count(&mut self, limit: usize) -> Result<usize, DataError> {
        let n = self.var()?;
        usize::try_from(n)
            .ok()
            .filter(|&n| n <= limit)
            .ok_or_else(|| self.err("count exceeds load limit"))
    }
    fn value(&mut self, kind: u8) -> Result<u64, DataError> {
        Ok(match kind {
            F32 => u32::from_le_bytes(self.take(4)?.try_into().unwrap()).into(),
            F64 => u64::from_le_bytes(self.take(8)?.try_into().unwrap()),
            _ => self.var()?,
        })
    }
    fn runs(&mut self, kind: u8, n: usize, r: &mut dyn Reader) -> Result<Vec<u64>, DataError> {
        let mut out = Vec::new();
        r.claim(n.saturating_mul(8))?;
        out.try_reserve_exact(n)
            .map_err(super::limits::allocation)?;
        while out.len() < n {
            let header = self.var()?;
            let count = usize::try_from(header >> 1)
                .ok()
                .filter(|&c| c != 0 && c <= n - out.len())
                .ok_or_else(|| self.err("run exceeds its column"))?;
            if header & 1 == 1 {
                let v = self.value(kind)?;
                out.extend(std::iter::repeat_n(v, count));
            } else {
                for _ in 0..count {
                    out.push(self.value(kind)?);
                }
            }
        }
        Ok(out)
    }
    fn column(&mut self, kind: u8, n: usize, r: &mut dyn Reader) -> Result<Vec<u64>, DataError> {
        match self.byte()? {
            0 => self.runs(kind, n, r),
            1 => {
                let d = self.count(256)?;
                let table = (0..d)
                    .map(|_| self.value(kind))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut ids = self.runs(UNSIGNED, n, r)?;
                for id in &mut ids {
                    *id = *table
                        .get(*id as usize)
                        .ok_or_else(|| self.err("dictionary index out of range"))?;
                }
                Ok(ids)
            }
            _ => Err(self.err("unknown column mode")),
        }
    }
    fn blobs(&mut self, n: usize, r: &mut dyn Reader) -> Result<Vec<&'a [u8]>, DataError> {
        let mut out = Vec::new();
        r.claim(n.saturating_mul(16))?;
        out.try_reserve_exact(n)
            .map_err(super::limits::allocation)?;
        while out.len() < n {
            let header = self.var()?;
            let count = usize::try_from(header >> 1)
                .ok()
                .filter(|&c| c != 0 && c <= n - out.len())
                .ok_or_else(|| self.err("run exceeds its column"))?;
            let repeated = header & 1 == 1;
            for i in 0..count {
                if i == 0 || !repeated {
                    let len = self.len()?;
                    out.push(self.take(len)?);
                } else {
                    out.push(out[out.len() - 1]);
                }
            }
        }
        Ok(out)
    }
}

// Scalar slots a shape carries, in walk order, checked once per shape.
fn slots(shape: &[u8], names: usize) -> Result<(Vec<u8>, usize), DataError> {
    let mut c = Cursor {
        bytes: shape,
        pos: 0,
    };
    let (mut kinds, mut blobs) = (Vec::new(), 0);
    while c.pos < shape.len() {
        match c.byte()? {
            END | STRUCT | NONE | SOME => {}
            FIELD => {
                if c.var()? >= names as u64 {
                    return Err(c.err("unknown name index"));
                }
            }
            SEQ => {
                c.var()?;
            }
            VARIANT => {
                c.var()?;
                if c.var()? >= names as u64 {
                    return Err(c.err("unknown name index"));
                }
            }
            t @ (UNSIGNED | SIGNED | F32 | F64 | BOOL) => kinds.push(t),
            STRING | 12..=15 => blobs += 1,
            _ => return Err(c.err("unknown shape token")),
        }
    }
    Ok((kinds, blobs))
}

/// Decoded rows of one storage, replayed one row at a time.
pub(crate) struct Rows<'a> {
    names: Vec<&'a str>,
    shapes: Vec<&'a [u8]>,
    row_shapes: Vec<u64>,
    groups: Vec<Decoded<'a>>,
    next: usize,
    frames: Vec<Frame>,
}
struct Decoded<'a> {
    nums: Vec<Vec<u64>>,
    blobs: Vec<Vec<&'a [u8]>>,
    row: usize,
}
impl<'a> Rows<'a> {
    /// Decode a payload of at most `limit` rows, and its row indices when `indexed`.
    pub(crate) fn decode(
        bytes: &'a [u8],
        limit: usize,
        indexed: bool,
        r: &mut dyn Reader,
    ) -> Result<(Vec<u32>, Self), DataError> {
        let mut c = Cursor { bytes, pos: 0 };
        let mut indices = Vec::new();
        let mut end = 0u64;
        for _ in 0..c.count(bytes.len())? {
            // Runs ascend: each starts at or after the previous one's end.
            let start = c.var()?.checked_add(end);
            let len = c.var()?;
            let Some(start) = start.filter(|s| s.checked_add(len).is_some()) else {
                return Err(c.err("index run overflows"));
            };
            end = start + len;
            if len == 0 || end > limit as u64 {
                return Err(c.err("index run exceeds the entity limit"));
            }
            r.claim((len as usize).saturating_mul(4))?;
            indices.extend(start as u32..end as u32);
        }
        let names = (0..c.count(bytes.len())?)
            .map(|_| {
                let len = c.len()?;
                let b = c.take(len)?;
                std::str::from_utf8(b).map_err(|_| c.err("invalid UTF-8 name"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let shapes = (0..c.count(bytes.len())?)
            .map(|_| {
                let len = c.len()?;
                c.take(len)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rows = c.count(limit)?;
        if (indexed && rows != indices.len()) || (!indexed && !indices.is_empty()) {
            return Err(c.err("rows disagree with their indices"));
        }
        let row_shapes = if shapes.len() > 1 {
            c.column(UNSIGNED, rows, r)?
        } else {
            let n = if shapes.is_empty() { 0 } else { rows };
            r.claim(n.saturating_mul(8))?;
            vec![0; n]
        };
        if rows != 0 && shapes.is_empty() {
            return Err(c.err("rows without a shape"));
        }
        let mut counts = vec![0usize; shapes.len()];
        for &s in &row_shapes {
            *counts
                .get_mut(s as usize)
                .ok_or_else(|| c.err("unknown shape"))? += 1;
        }
        let mut groups = Vec::with_capacity(shapes.len());
        for (shape, &n) in shapes.iter().zip(&counts) {
            let (kinds, blob_slots) = slots(shape, names.len())?;
            let (nums, blobs) = match c.byte()? {
                0 => (
                    kinds
                        .iter()
                        .map(|&k| c.column(k, n, r))
                        .collect::<Result<Vec<_>, _>>()?,
                    (0..blob_slots)
                        .map(|_| c.blobs(n, r))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                1 if blob_slots == 0 => {
                    let d = c.count(n)?;
                    let table = kinds
                        .iter()
                        .map(|&k| c.column(k, d, r))
                        .collect::<Result<Vec<_>, _>>()?;
                    let ids = c.column(UNSIGNED, n, r)?;
                    let mut nums = Vec::with_capacity(kinds.len());
                    for column in &table {
                        // Each expanded column holds n values: account it first.
                        r.claim(n.saturating_mul(8))?;
                        let mut out = Vec::new();
                        out.try_reserve_exact(n)
                            .map_err(super::limits::allocation)?;
                        for &id in &ids {
                            out.push(
                                *column
                                    .get(id as usize)
                                    .ok_or_else(|| c.err("row index out of range"))?,
                            );
                        }
                        nums.push(out);
                    }
                    (nums, Vec::new())
                }
                _ => return Err(c.err("unknown group mode")),
            };
            groups.push(Decoded {
                nums,
                blobs,
                row: 0,
            });
        }
        if c.pos != bytes.len() {
            return Err(c.err("trailing column data"));
        }
        Ok((
            indices,
            Self {
                names,
                shapes,
                row_shapes,
                groups,
                next: 0,
                frames: Vec::new(),
            },
        ))
    }
    pub(crate) fn len(&self) -> usize {
        self.row_shapes.len()
    }
    /// Replay the next row into `read`, which must consume it exactly.
    pub(crate) fn read<T>(
        &mut self,
        outer: &mut dyn Reader,
        read: impl FnOnce(&mut dyn Reader) -> Result<T, DataError>,
    ) -> Result<T, DataError> {
        let id = self.row_shapes[self.next] as usize;
        self.next += 1;
        let mut replay = Replay {
            c: Cursor {
                bytes: self.shapes[id],
                pos: 0,
            },
            names: &self.names,
            group: &self.groups[id],
            num: 0,
            blob: 0,
            frames: &mut self.frames,
            outer,
        };
        replay.frames.clear();
        let value = read(&mut replay)?;
        if replay.c.pos != replay.c.bytes.len() || !replay.frames.is_empty() {
            return Err(DataError::new("row not fully read"));
        }
        self.groups[id].row += 1;
        Ok(value)
    }
}

enum Frame {
    Seq(u64),
    Struct,
    Variant,
    Option,
}
struct Replay<'r, 'a> {
    c: Cursor<'a>,
    names: &'r [&'a str],
    group: &'r Decoded<'a>,
    num: usize,
    blob: usize,
    frames: &'r mut Vec<Frame>,
    outer: &'r mut dyn Reader,
}
impl Replay<'_, '_> {
    fn peek(&self) -> Option<u8> {
        self.c.bytes.get(self.c.pos).copied()
    }
    fn expect(&mut self, token: u8, what: &str) -> Result<(), DataError> {
        if self.c.byte()? == token {
            Ok(())
        } else {
            Err(self.c.err(what))
        }
    }
    fn scalar(&mut self) -> u64 {
        let v = self.group.nums[self.num][self.group.row];
        self.num += 1;
        v
    }
    fn blob(&mut self) -> &[u8] {
        let v = self.group.blobs[self.blob][self.group.row];
        self.blob += 1;
        v
    }
    fn name(&mut self) -> Result<String, DataError> {
        let i = self.c.var()? as usize;
        let name = self.names[i];
        self.outer.claim(name.len())?;
        Ok(name.into())
    }
}
impl Reader for Replay<'_, '_> {
    fn claim(&mut self, bytes: usize) -> Result<(), DataError> {
        self.outer.claim(bytes)
    }
    fn sequence_len(&self) -> Option<usize> {
        match self.frames.last() {
            Some(Frame::Seq(n)) => usize::try_from(*n).ok(),
            _ => None,
        }
    }
    fn boolean(&mut self) -> Result<bool, DataError> {
        self.expect(BOOL, "expected a boolean")?;
        Ok(self.scalar() != 0)
    }
    fn number(&mut self) -> Result<Number, DataError> {
        let t = self.c.byte()?;
        Ok(match t {
            UNSIGNED => Number::Unsigned(self.scalar()),
            SIGNED => {
                let n = self.scalar();
                Number::Signed(((n >> 1) as i64) ^ -((n & 1) as i64))
            }
            F32 => Number::F32(f32::from_bits(self.scalar() as u32)),
            F64 => Number::F64(f64::from_bits(self.scalar())),
            _ => return Err(self.c.err("expected a number")),
        })
    }
    fn bytes(&mut self, kind: BulkKind) -> Result<Option<&[u8]>, DataError> {
        if self.peek() == Some(SEQ) {
            return Ok(None);
        }
        self.expect(BULK + kind as u8, "expected a bulk payload")?;
        let len = self.group.blobs[self.blob][self.group.row].len();
        self.outer.claim(len)?;
        Ok(Some(self.blob()))
    }
    fn string(&mut self) -> Result<String, DataError> {
        self.expect(STRING, "expected a string")?;
        let b = self.blob();
        if b.len() > MAX_LOAD_STRING {
            return Err(DataError::new("string exceeds load limit"));
        }
        let s = std::str::from_utf8(b)
            .map_err(|_| DataError::new("invalid UTF-8"))?
            .to_owned();
        self.outer.claim(s.len())?;
        Ok(s)
    }
    fn begin_seq(&mut self) -> Result<(), DataError> {
        self.expect(SEQ, "expected a sequence")?;
        let n = self.c.var()?;
        self.frames.push(Frame::Seq(n));
        Ok(())
    }
    fn item(&mut self) -> Result<bool, DataError> {
        match self.frames.last_mut() {
            Some(Frame::Seq(0)) => {
                self.frames.pop();
                Ok(false)
            }
            Some(Frame::Seq(n)) => {
                *n -= 1;
                Ok(true)
            }
            _ => Err(self.c.err("not inside a sequence")),
        }
    }
    fn begin_struct(&mut self) -> Result<(), DataError> {
        self.expect(STRUCT, "expected a record")?;
        self.frames.push(Frame::Struct);
        Ok(())
    }
    fn field(&mut self) -> Result<Option<String>, DataError> {
        if !matches!(self.frames.last(), Some(Frame::Struct)) {
            return Err(self.c.err("not inside a record"));
        }
        match self.c.byte()? {
            END => {
                self.frames.pop();
                Ok(None)
            }
            FIELD => self.name().map(Some),
            _ => Err(self.c.err("expected a field")),
        }
    }
    fn variant(&mut self) -> Result<String, DataError> {
        self.expect(VARIANT, "expected an enum")?;
        self.c.var()?;
        let name = self.name()?;
        self.frames.push(Frame::Variant);
        Ok(name)
    }
    fn end_variant(&mut self) -> Result<(), DataError> {
        match self.frames.pop() {
            Some(Frame::Variant) => Ok(()),
            _ => Err(self.c.err("not inside an enum")),
        }
    }
    fn option(&mut self) -> Result<bool, DataError> {
        let some = match self.c.byte()? {
            NONE => false,
            SOME => true,
            _ => return Err(self.c.err("expected an option")),
        };
        self.frames.push(Frame::Option);
        Ok(some)
    }
    fn end_option(&mut self) -> Result<(), DataError> {
        match self.frames.pop() {
            Some(Frame::Option) => Ok(()),
            _ => Err(self.c.err("not inside an option")),
        }
    }
    fn skip(&mut self) -> Result<(), DataError> {
        match self.peek() {
            Some(BOOL) => {
                self.boolean()?;
            }
            Some(UNSIGNED..=F64) => {
                self.number()?;
            }
            Some(STRING | 12..=15) => {
                self.c.byte()?;
                self.blob += 1;
            }
            Some(SEQ) => {
                self.begin_seq()?;
                while self.item()? {
                    self.skip()?;
                }
            }
            Some(STRUCT) => {
                self.begin_struct()?;
                while self.field()?.is_some() {
                    self.skip()?;
                }
            }
            Some(VARIANT) => {
                self.variant()?;
                self.skip()?;
                self.end_variant()?;
            }
            Some(NONE | SOME) => {
                if self.option()? {
                    self.skip()?;
                }
                self.end_option()?;
            }
            _ => return Err(self.c.err("unknown or missing value tag")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bin, Data, Entity, Material, Mesh, Transform, Vec3};

    #[derive(Clone, Debug, Default, PartialEq, Data)]
    enum Stage {
        #[default]
        Seed,
        Growing {
            at: f32,
        },
        Ripe(u8, String),
    }
    #[derive(Clone, Debug, Default, PartialEq, Data)]
    struct Row {
        id: u32,
        delta: i64,
        wide: f64,
        alive: bool,
        name: Option<String>,
        stage: Stage,
        fruit: Vec<Entity>,
        bytes: Vec<u8>,
        floats: Vec<f32>,
    }
    fn round_trip<T: Data + PartialEq + std::fmt::Debug>(rows: &[T]) -> usize {
        let mut columns = Columns::default();
        for row in rows {
            columns.row(|w| row.write(w));
        }
        let bytes = columns.finish();
        let mut outer = bin::Decoder::new(&[]);
        let mut decoded = Rows::decode(&bytes, rows.len(), false, &mut outer)
            .unwrap()
            .1;
        assert_eq!(decoded.len(), rows.len());
        for (i, expected) in rows.iter().enumerate() {
            let got = decoded
                .read(&mut outer, |r| {
                    let mut v = T::default();
                    v.read(r).map(|()| v)
                })
                .unwrap();
            assert_eq!(&got, expected, "row {i}");
        }
        bytes.len()
    }
    #[test]
    fn every_data_form_round_trips_through_shapes_and_columns() {
        let mut world = crate::World::new(60, 0);
        let es: Vec<Entity> = (0..4).map(|_| world.spawn(())).collect();
        let rows: Vec<Row> = (0..300u32)
            .map(|i| Row {
                id: i * 7,
                delta: -(i as i64) * 1_000_003,
                wide: i as f64 / 3.0,
                alive: i % 3 == 0,
                name: (i % 5 == 0).then(|| format!("plant-{}", i % 2)),
                stage: match i % 3 {
                    0 => Stage::Seed,
                    1 => Stage::Growing { at: -0.0 },
                    _ => Stage::Ripe(i as u8, "gold".into()),
                },
                fruit: es[..(i % 4) as usize].to_vec(),
                bytes: vec![i as u8; (i % 3) as usize],
                floats: vec![f32::NAN, i as f32],
            })
            .collect();
        let mut canonical = rows.clone();
        for row in &mut canonical {
            row.floats[0] = f32::from_bits(0x7fc0_0000);
        }
        let mut columns = Columns::default();
        for row in &rows {
            columns.row(|w| row.write(w));
        }
        let bytes = columns.finish();
        let mut outer = bin::Decoder::new(&[]);
        let mut decoded = Rows::decode(&bytes, rows.len(), false, &mut outer)
            .unwrap()
            .1;
        for expected in &canonical {
            let got = decoded
                .read(&mut outer, |r| {
                    let mut v = Row::default();
                    v.read(r).map(|()| v)
                })
                .unwrap();
            assert_eq!(bin::to_vec(&got), bin::to_vec(expected));
            assert!(got.floats[0].is_nan() && got.stage == expected.stage);
        }
        assert!(round_trip(&[Stage::Ripe(1, "x".into())]) > 0);
        assert!(round_trip::<Row>(&[]) > 0);
    }
    #[test]
    fn repeated_scene_rows_cost_bytes_per_change_not_per_row() {
        let n = 10_000;
        let poses: Vec<_> = (0..n)
            .map(|i| Transform::at((i % 250) as f32, 0.0, (i / 250) as f32))
            .collect();
        let materials: Vec<_> = (0..n)
            .map(|i| Material::rgb(i as f32 % 14.0 / 14.0, 0.5, 0.2))
            .collect();
        let meshes = vec![Mesh::Sphere { radius: 0.1 }; n];
        let row = |v: &dyn Fn(&mut dyn Writer)| {
            let mut w = bin::Encoder::default();
            v(&mut w);
            w.finish().len()
        };
        for (columnar, rows) in [
            (
                round_trip(&poses),
                row(&|w| poses.iter().for_each(|p| p.write(w))),
            ),
            (
                round_trip(&materials),
                row(&|w| materials.iter().for_each(|p| p.write(w))),
            ),
            (
                round_trip(&meshes),
                row(&|w| meshes.iter().for_each(|p| p.write(w))),
            ),
        ] {
            assert!(
                columnar * 8 < rows,
                "{columnar} columnar bytes against {rows} tagged"
            );
        }
    }
    #[test]
    fn a_reader_with_another_schema_skips_and_defaults_by_field_name() {
        #[derive(Default, Data)]
        struct Old {
            keep: u32,
            gone: Vec<String>,
            also: Stage,
        }
        #[derive(Default, Debug, PartialEq, Data)]
        struct New {
            keep: u32,
            added: Vec3,
        }
        let mut columns = Columns::default();
        let old = Old {
            keep: 4,
            gone: vec!["a".into(), "b".into()],
            also: Stage::Growing { at: 1.0 },
        };
        columns.row(|w| old.write(w));
        let bytes = columns.finish();
        let mut outer = bin::Decoder::new(&[]);
        let mut decoded = Rows::decode(&bytes, 1, false, &mut outer).unwrap().1;
        let got = decoded
            .read(&mut outer, |r| {
                let mut v = New::default();
                v.read(r).map(|()| v)
            })
            .unwrap();
        assert_eq!(
            got,
            New {
                keep: 4,
                added: Vec3::ZERO
            }
        );
    }
    // A sub-megabyte payload naming one shape of 10,000 scalars repeated over 4M
    // rows from a one-row dictionary must refuse under the load budget, not
    // expand 10,000 columns of 4M values.
    #[test]
    fn a_dictionary_group_cannot_expand_past_the_load_budget() {
        let (kinds, rows) = (10_000u64, 1u64 << 22);
        let mut bytes = vec![0, 0, 1];
        var(&mut bytes, kinds);
        bytes.extend(std::iter::repeat_n(UNSIGNED, kinds as usize));
        var(&mut bytes, rows);
        bytes.push(1);
        var(&mut bytes, 1);
        for _ in 0..kinds {
            bytes.extend([0, 3, 0]); // runs mode, one repeated run of one value 0
        }
        bytes.push(0);
        var(&mut bytes, rows << 1 | 1);
        bytes.push(0);
        assert!(bytes.len() < 1 << 20);
        let mut outer = bin::Decoder::new(&[]);
        let error = Rows::decode(&bytes, rows as usize, false, &mut outer)
            .err()
            .expect("refused");
        assert!(error.to_string().contains("budget"), "{error}");
    }
    // Review S1: a gap that wraps would have produced descending indices.
    #[test]
    fn index_runs_cannot_wrap_or_descend() {
        let mut bytes = vec![2];
        var(&mut bytes, 5);
        var(&mut bytes, 1);
        var(&mut bytes, u64::MAX - 2);
        var(&mut bytes, 1);
        bytes.extend([0, 1, 0, 2, 0]);
        let mut outer = bin::Decoder::new(&[]);
        let error = Rows::decode(&bytes, 1 << 20, true, &mut outer)
            .err()
            .unwrap();
        assert!(error.to_string().contains("overflows"), "{error}");
    }
    #[test]
    fn truncated_or_corrupt_payloads_refuse() {
        let rows: Vec<_> = (0..64).map(|i| Transform::at(i as f32, 1.0, 2.0)).collect();
        let mut columns = Columns::default();
        for row in &rows {
            columns.row(|w| row.write(w));
        }
        let bytes = columns.finish();
        for end in 0..bytes.len() {
            let mut outer = bin::Decoder::new(&[]);
            assert!(
                Rows::decode(&bytes[..end], 64, false, &mut outer).is_err(),
                "prefix {end}"
            );
        }
        let mut outer = bin::Decoder::new(&[]);
        assert!(
            Rows::decode(&bytes, 63, false, &mut outer).is_err(),
            "row limit"
        );
    }
}
