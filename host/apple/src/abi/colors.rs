//! The presenter's colour report (LLP 1095 D1): which references it should
//! resolve, and what it resolved them to. See `colors.rs` in the host.

use super::Bridge;
use exact_kernel::style::{roles, Color, ColorValue};
use exact_runner::DataSource;

/// `[[kind, id, "name"], …]`: every reference the presenter may resolve
/// (kind 0 a role, 1 a `-exact-platform-color()`), with its name on this platform.
fn references_json() -> String {
    let mut out = String::from("[");
    for (i, (c, name)) in roles::references(cfg!(target_os = "macos"))
        .into_iter()
        .enumerate()
    {
        let (kind, id) = match c {
            ColorValue::Role(id) => (0, u16::from(id)),
            ColorValue::Platform(id) => (1, id),
            _ => continue,
        };
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("[{kind},{id},{:?}]", &*name));
    }
    out.push(']');
    out
}

/// The presenter's report: LE records of (u8 kind, u8 dark, u16 id, u8 r,
/// g, b, a). `None` when the bytes are not whole records; a record naming
/// no role or interned `-exact-platform-color()` is dropped.
pub(crate) fn parse_report(bytes: &[u8]) -> Option<Vec<(ColorValue, bool, Color)>> {
    if !bytes.len().is_multiple_of(8) {
        return None;
    }
    Some(
        bytes
            .chunks_exact(8)
            .filter_map(|r| {
                let id = u16::from_le_bytes([r[2], r[3]]);
                let c = match r[0] {
                    0 => ColorValue::Role(u8::try_from(id).ok()?),
                    1 => ColorValue::Platform(id),
                    _ => return None,
                };
                if !roles::is_known_reference(c) {
                    return None;
                }
                let rgba = (u32::from(r[4]) << 24)
                    | (u32::from(r[5]) << 16)
                    | (u32::from(r[6]) << 8)
                    | u32::from(r[7]);
                Some((c, r[1] != 0, Color(rgba)))
            })
            .collect(),
    )
}

impl<D: DataSource> Bridge<D> {
    /// Every reference the presenter may resolve, as JSON in the output
    /// buffer; returns its length.
    pub fn color_references(&mut self) -> u32 {
        self.emit(references_json())
    }

    /// The presenter's resolutions, as records in the input buffer's first
    /// `len` bytes; returns the batch's length.
    pub fn colors(&mut self, len: usize) -> u32 {
        let bytes = self.input.get(..len).unwrap_or(&[]);
        let out = match parse_report(bytes) {
            None => "{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"colors: truncated record\"}".to_string(),
            Some(report) => self
                .host
                .as_mut()
                .map_or_else(super::not_booted, |h| h.set_colors(report)),
        };
        self.emit(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::generated::COLOR_ROLES;

    fn record(kind: u8, id: u16) -> [u8; 8] {
        let [lo, hi] = id.to_le_bytes();
        [kind, 0, lo, hi, 255, 0, 0, 255]
    }

    #[test]
    fn a_report_drops_ids_that_name_no_reference() {
        let past = u16::try_from(COLOR_ROLES.len()).unwrap();
        let mut bytes = Vec::new();
        for r in [
            record(0, 0),
            record(0, past),
            record(0, 300),
            record(1, u16::MAX),
            record(2, 0),
        ] {
            bytes.extend(r);
        }
        let report = parse_report(&bytes).unwrap();
        assert_eq!(
            report,
            vec![(ColorValue::Role(0), false, Color(0xff00_00ff))],
            "only the role inside the table"
        );
        assert!(parse_report(&bytes[..7]).is_none(), "a partial record");
    }
}
