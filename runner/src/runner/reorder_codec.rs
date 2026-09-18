//! Synthesized terminal delivery only; physical keys never come from this packet.
use super::Event;
use exact_plan::bytes::{Reader, Writer, MAX_COUNT};

impl Event {
    /// Version 1 LE: u32 version, u32 UTF-8 byte count + item, u8 Option tag,
    /// then (only for Some) u32 UTF-8 byte count + before. No trailing bytes.
    /// Non-reorder events or strings above the existing codec count bound refuse.
    pub fn reorder_drop_bytes(&self) -> Option<Vec<u8>> {
        let Self::ReorderDrop { item, before } = self else {
            return None;
        };
        if item.len() > MAX_COUNT || before.as_ref().is_some_and(|s| s.len() > MAX_COUNT) {
            return None;
        }
        let mut w = Writer::default();
        w.u32(1);
        w.u32(item.len() as u32);
        w.bytes(item.as_bytes());
        w.u8(u8::from(before.is_some()));
        if let Some(s) = before {
            w.u32(s.len() as u32);
            w.bytes(s.as_bytes());
        }
        Some(w.into_vec())
    }
    /// Strict decoder for explicitly synthesized terminal events. Preserves empty
    /// keys, Unicode, commas and NUL; no clock/action or physical certification.
    pub fn reorder_drop_payload(bytes: &[u8]) -> Option<Self> {
        fn text(r: &mut Reader<'_>) -> Option<String> {
            let n = r.count().ok()?;
            Some(std::str::from_utf8(r.bytes(n).ok()?).ok()?.to_owned())
        }
        let mut r = Reader::new(bytes);
        if r.u32().ok()? != 1 {
            return None;
        }
        let item = text(&mut r)?;
        let before = match r.u8().ok()? {
            0 => None,
            1 => Some(text(&mut r)?),
            _ => return None,
        };
        r.is_empty().then_some(Self::ReorderDrop { item, before })
    }
}
