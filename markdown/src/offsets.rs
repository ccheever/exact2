//! Byte offsets inside, UTF-16 code units at the boundary.

use crate::Range;

/// A source's byte-to-UTF-16 table; absent when the source is ASCII.
pub(crate) struct Offsets {
    table: Option<Vec<u32>>,
    bytes: usize,
}

impl Offsets {
    pub(crate) fn new(source: &str) -> Self {
        let bytes = source.len();
        if source.is_ascii() {
            return Self { table: None, bytes };
        }
        // Every byte of a character carries the character's own offset, so a
        // search for a code unit lands on a character boundary.
        let mut table = Vec::with_capacity(bytes + 1);
        let mut units = 0u32;
        for ch in source.chars() {
            for _ in 0..ch.len_utf8() {
                table.push(units);
            }
            units += ch.len_utf16() as u32;
        }
        table.push(units);
        Self {
            table: Some(table),
            bytes,
        }
    }

    pub(crate) fn to16(&self, byte: usize) -> u32 {
        let byte = byte.min(self.bytes);
        match &self.table {
            None => byte as u32,
            Some(table) => table[byte],
        }
    }

    pub(crate) fn to8(&self, unit: u32) -> usize {
        match &self.table {
            None => (unit as usize).min(self.bytes),
            Some(table) => table.partition_point(|&v| v < unit).min(self.bytes),
        }
    }

    pub(crate) fn range16(&self, bytes: &std::ops::Range<usize>) -> Range {
        Range::new(self.to16(bytes.start), self.to16(bytes.end))
    }

    pub(crate) fn range8(&self, range: Range) -> std::ops::Range<usize> {
        let start = self.to8(range.start);
        start..self.to8(range.end).max(start)
    }
}
