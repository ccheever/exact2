//! UTF-16 source text: the unit the DOM, TextKit and the styler count in.

/// A replacement: `start..end` of the original becomes the code units.
pub(crate) type Rep = (u32, u32, Vec<u16>);

pub(crate) fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

pub(crate) fn utf8(text: &[u16]) -> String {
    String::from_utf16_lossy(text)
}

pub(crate) fn high(c: u16) -> bool {
    (0xD800..0xDC00).contains(&c)
}

pub(crate) fn low(c: u16) -> bool {
    (0xDC00..0xE000).contains(&c)
}

/// The source as the editor keeps it: unpaired surrogates become U+FFFD, so an
/// offset means the same code unit here and in the UTF-8 the styler reads, and
/// `\r\n` or a lone `\r` becomes `\n`, as a browser's textarea value does.
pub(crate) fn clean(text: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        if high(c) && text.get(i + 1).is_some_and(|&d| low(d)) {
            out.extend_from_slice(&text[i..i + 2]);
            i += 2;
            continue;
        }
        if c == 13 {
            out.push(10);
            i += if text.get(i + 1) == Some(&10) { 2 } else { 1 };
            continue;
        }
        out.push(if high(c) || low(c) { 0xFFFD } else { c });
        i += 1;
    }
    out
}

/// How positions move through one [`splice`].
pub(crate) struct Map {
    reps: Vec<(u32, u32, u32)>,
}

/// Apply replacements given in original coordinates (sorted here, stably).
pub(crate) fn splice(source: &[u16], reps: &[Rep]) -> (Vec<u16>, Map) {
    let len = source.len() as u32;
    // A handful at most: an insertion sort, stable, and no sort code to ship.
    let mut order: Vec<&Rep> = Vec::with_capacity(reps.len());
    for r in reps {
        let k = order.partition_point(|o| o.0 <= r.0);
        order.insert(k, r);
    }
    let mut out = Vec::with_capacity(source.len());
    let mut at = 0u32;
    let mut mapped = Vec::with_capacity(order.len());
    for (from, to, text) in order {
        let (from, to) = ((*from).min(len), (*to).min(len));
        let start = from.max(at);
        out.extend_from_slice(&source[at as usize..start as usize]);
        out.extend_from_slice(text);
        mapped.push((start, to.max(start), text.len() as u32));
        at = at.max(to);
    }
    out.extend_from_slice(&source[at as usize..]);
    (out, Map { reps: mapped })
}

impl Map {
    /// Where an unreplaced code unit went; `None` if a replacement removed it.
    pub(crate) fn char(&self, c: u32) -> Option<u32> {
        let mut delta = 0i64;
        for &(s, e, n) in &self.reps {
            if s == e {
                if s <= c {
                    delta += i64::from(n);
                    continue;
                }
                break;
            }
            if c >= e {
                delta += i64::from(n) - i64::from(e - s);
                continue;
            }
            if c >= s {
                return None;
            }
            break;
        }
        Some((i64::from(c) + delta).max(0) as u32)
    }

    /// Where a caret went: after an insertion at its own position.
    pub(crate) fn caret(&self, p: u32) -> u32 {
        let mut delta = 0i64;
        for &(s, e, n) in &self.reps {
            if p < s {
                break;
            }
            if p <= e {
                return (i64::from(s) + delta + i64::from(n)).max(0) as u32;
            }
            delta += i64::from(n) - i64::from(e - s);
        }
        (i64::from(p) + delta).max(0) as u32
    }
}
