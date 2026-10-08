//! Stateful paint deltas. Cold dictionaries remain the shared host's authority.

use super::{number, record, Reader, WireError, PAINT};
use std::collections::HashMap;
use std::rc::Rc;

const MEMO_ENTRIES: usize = 64;
const MEMO_BYTES: usize = 64 * 1024;

const KEYS: [&[u8]; 6] = [
    b"background_color",
    b"text_color",
    b"border_color_top",
    b"border_color_right",
    b"border_color_bottom",
    b"border_color_left",
];
const DEFAULTS: [[u32; 2]; 6] = [
    [0; 2],
    [0xff000000; 2],
    [0xff000000; 2],
    [0xff000000; 2],
    [0xff000000; 2],
    [0xff000000; 2],
];

#[derive(Clone)]
struct Style {
    other: Rc<[u8]>,
    colors: [[u32; 2]; 6],
}

struct Memo {
    bytes: Box<[u8]>,
    style: Style,
}

impl Memo {
    fn charge(&self) -> usize {
        // Count shared non-paint bytes once per entry conservatively.
        self.bytes.len() + self.style.other.len()
    }
}

enum Undo {
    Replace(u32, Option<Style>),
    Colors(u32, [[u32; 2]; 6]),
}

#[derive(Default)]
pub(super) struct Cache {
    styles: HashMap<u32, Style>,
    scratch: Vec<u8>,
    undo: Vec<Undo>,
    memo: Vec<Memo>,
    memo_bytes: usize,
    #[cfg(test)]
    parses: usize,
}

pub(super) enum ResultKind {
    Cold,
    Paint,
    Unchanged,
}

impl Cache {
    /// A memo contains complete dictionaries already validated at this depth.
    /// Matching every byte can skip their structural walk; the enclosing op
    /// parser still requires a comma or closing brace immediately afterward.
    pub(super) fn value<'a>(&self, reader: &mut Reader<'a>) -> Result<&'a [u8], WireError> {
        reader.whitespace();
        let start = reader.pos;
        if reader.depth == 0 {
            let tail = &reader.bytes[start..];
            if let Some(entry) = self
                .memo
                .iter()
                .find(|entry| tail.starts_with(&entry.bytes))
            {
                reader.pos += entry.bytes.len();
                return Ok(&reader.bytes[start..reader.pos]);
            }
        }
        reader.value()
    }

    fn resolved(&mut self, bytes: &[u8]) -> Result<Option<Style>, WireError> {
        if let Some(index) = self
            .memo
            .iter()
            .position(|entry| entry.bytes.as_ref() == bytes)
        {
            // Sibling styles and alternating paint states stay near the front.
            self.memo[..=index].rotate_right(1);
            return Ok(Some(self.memo[0].style.clone()));
        }
        self.scratch.clear();
        #[cfg(test)]
        {
            self.parses += 1;
        }
        let Some(colors) = parse(bytes, &mut self.scratch)? else {
            return Ok(None);
        };
        let other = self
            .memo
            .iter()
            .find(|entry| entry.style.other.as_ref() == self.scratch.as_slice())
            .map_or_else(
                || Rc::from(self.scratch.as_slice()),
                |entry| entry.style.other.clone(),
            );
        let style = Style { other, colors };
        let charge = bytes.len().saturating_add(style.other.len());
        if charge <= MEMO_BYTES {
            while self.memo.len() >= MEMO_ENTRIES || self.memo_bytes + charge > MEMO_BYTES {
                self.memo_bytes -= self
                    .memo
                    .pop()
                    .expect("nonempty bounded style memo")
                    .charge();
            }
            self.memo.insert(
                0,
                Memo {
                    bytes: bytes.into(),
                    style: style.clone(),
                },
            );
            self.memo_bytes += charge;
        }
        // Pure validated-content memoization may survive a failed batch. Only
        // the per-view baseline is transactional and participates in undo.
        Ok(Some(style))
    }

    pub(super) fn remove(&mut self, id: u32) {
        self.undo.push(Undo::Replace(id, self.styles.remove(&id)));
    }

    pub(super) fn finish(&mut self, success: bool) {
        if success {
            self.undo.clear();
        } else {
            while let Some(undo) = self.undo.pop() {
                match undo {
                    Undo::Replace(id, old) => {
                        self.styles.remove(&id);
                        if let Some(old) = old {
                            self.styles.insert(id, old);
                        }
                    }
                    Undo::Colors(id, colors) => {
                        self.styles.get_mut(&id).expect("paint undo order").colors = colors;
                    }
                }
            }
        }
    }

    pub(super) fn style(
        &mut self,
        id: u32,
        bytes: &[u8],
        delta: bool,
        out: &mut Vec<u8>,
    ) -> Result<ResultKind, WireError> {
        let Some(style) = self.resolved(bytes)? else {
            self.remove(id);
            return Ok(ResultKind::Cold);
        };
        let colors = style.colors;
        if let Some(old) = self.styles.get_mut(&id).filter(|old| {
            delta && (Rc::ptr_eq(&old.other, &style.other) || old.other == style.other)
        }) {
            let mut mask = 0u32;
            for (i, color) in colors.iter().enumerate() {
                if *color != old.colors[i] {
                    mask |= 1 << i;
                }
            }
            if mask == 0 {
                return Ok(ResultKind::Unchanged);
            }
            let mut payload = [0u8; 56];
            payload[..4].copy_from_slice(&id.to_le_bytes());
            payload[4..8].copy_from_slice(&mask.to_le_bytes());
            let mut pos = 8;
            for (i, color) in colors.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    for channel in color {
                        payload[pos..pos + 4].copy_from_slice(&channel.to_le_bytes());
                        pos += 4;
                    }
                }
            }
            record(out, PAINT, &payload[..pos])?;
            self.undo.push(Undo::Colors(id, old.colors));
            old.colors = colors;
            return Ok(ResultKind::Paint);
        }
        let old = self.styles.insert(id, style);
        self.undo.push(Undo::Replace(id, old));
        Ok(ResultKind::Cold)
    }
}

fn parse(bytes: &[u8], other: &mut Vec<u8>) -> Result<Option<[[u32; 2]; 6]>, WireError> {
    let mut reader = Reader::new(bytes);
    reader.expect(b'{')?;
    let mut colors = DEFAULTS;
    let mut seen = 0u8;
    if !reader.take(b'}') {
        loop {
            let key = reader.string()?;
            // Escaped aliases could denote the same key after JSON decoding;
            // never delta-encode a dictionary whose key identity is ambiguous.
            if key.contains(&b'\\') {
                return Ok(None);
            }
            reader.expect(b':')?;
            let value = reader.value()?;
            if let Some(index) = KEYS.iter().position(|name| *name == key) {
                // Unsupported values and duplicate colors keep the entire cold dictionary.
                if seen & (1 << index) != 0 {
                    return Ok(None);
                }
                seen |= 1 << index;
                let Some(color) = color(value)? else {
                    return Ok(None);
                };
                colors[index] = color;
            } else {
                // Length delimiters distinguish keys/values without allocating a map.
                other.extend_from_slice(&(key.len() as u64).to_le_bytes());
                other.extend_from_slice(key);
                other.extend_from_slice(&(value.len() as u64).to_le_bytes());
                other.extend_from_slice(value);
            }
            if reader.take(b'}') {
                break;
            }
            reader.expect(b',')?;
        }
    }
    reader.end()?;
    Ok(Some(colors))
}

fn color(bytes: &[u8]) -> Result<Option<[u32; 2]>, WireError> {
    let mut reader = Reader::new(bytes);
    if !reader.take(b'[') {
        return Ok(None);
    }
    let pair = if reader.take(b'[') {
        let Some(light) = channels(&mut reader)? else {
            return Ok(None);
        };
        reader.expect(b',')?;
        reader.expect(b'[')?;
        let Some(dark) = channels(&mut reader)? else {
            return Ok(None);
        };
        reader.expect(b']')?;
        [light, dark]
    } else {
        let Some(solid) = channels(&mut reader)? else {
            return Ok(None);
        };
        [solid; 2]
    };
    reader.end()?;
    Ok(Some(pair))
}

fn channels(reader: &mut Reader<'_>) -> Result<Option<u32>, WireError> {
    let mut rgba = [0u8; 4];
    for (i, channel) in rgba.iter_mut().enumerate() {
        let value = reader.value()?;
        let Ok(byte) = number::<u8>(value) else {
            return Ok(None);
        };
        *channel = byte;
        if i < 3 {
            reader.expect(b',')?;
        }
    }
    reader.expect(b']')?;
    Ok(Some(u32::from_be_bytes([
        rgba[3], rgba[0], rgba[1], rgba[2],
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(cache: &mut Cache, id: u32, bytes: &[u8], delta: bool) -> ResultKind {
        let result = cache.style(id, bytes, delta, &mut Vec::new()).unwrap();
        cache.finish(true);
        result
    }

    #[test]
    fn siblings_and_alternating_paints_parse_each_complete_style_once() {
        let mut cache = Cache::default();
        let a = br#"{"font_size":16,"background_color":[1,2,3,255]}"#;
        let b = br#"{"font_size":16,"background_color":[4,5,6,255]}"#;
        for id in 0..1000 {
            assert!(matches!(apply(&mut cache, id, a, false), ResultKind::Cold));
        }
        for turn in 0..20 {
            let bytes = if turn % 2 == 0 { &b[..] } else { &a[..] };
            for id in 0..1000 {
                assert!(matches!(
                    apply(&mut cache, id, bytes, true),
                    ResultKind::Paint
                ));
            }
        }
        assert_eq!(cache.parses, 2);
        assert_eq!(cache.memo.len(), 2);
        assert!(Rc::ptr_eq(
            &cache.memo[0].style.other,
            &cache.memo[1].style.other
        ));
        for style in cache.styles.values() {
            assert!(Rc::ptr_eq(&style.other, &cache.memo[0].style.other));
        }
    }

    #[test]
    fn memo_is_bounded_and_eviction_keeps_live_view_baselines() {
        let mut cache = Cache::default();
        let a = br#"{"font_size":16,"background_color":[1,2,3,255]}"#;
        let b = br#"{"font_size":16,"background_color":[4,5,6,255]}"#;
        apply(&mut cache, 1, a, false);
        for id in 2..258 {
            let bytes = format!(r#"{{"font_size":{id},"background_color":[1,2,3,255]}}"#);
            apply(&mut cache, id, bytes.as_bytes(), false);
            assert!(cache.memo.len() <= MEMO_ENTRIES);
            assert!(cache.memo_bytes <= MEMO_BYTES);
        }
        assert!(!cache.memo.iter().any(|entry| entry.bytes.as_ref() == a));
        assert!(matches!(apply(&mut cache, 1, b, true), ResultKind::Paint));
        for id in 300..310 {
            let bytes = format!(r#"{{"label":"{id}{}"}}"#, "x".repeat(8192));
            apply(&mut cache, id, bytes.as_bytes(), false);
            assert!(cache.memo.len() <= MEMO_ENTRIES);
            assert!(cache.memo_bytes <= MEMO_BYTES);
        }
        assert_eq!(
            cache.memo_bytes,
            cache.memo.iter().map(Memo::charge).sum::<usize>()
        );
        let huge = format!(r#"{{"label":"{}"}}"#, "x".repeat(MEMO_BYTES));
        apply(&mut cache, 900, huge.as_bytes(), false);
        assert!(!cache
            .memo
            .iter()
            .any(|entry| entry.bytes.as_ref() == huge.as_bytes()));
        assert!(matches!(
            apply(&mut cache, 900, huge.as_bytes(), true),
            ResultKind::Unchanged
        ));
        assert!(cache.memo_bytes <= MEMO_BYTES);
    }
}
