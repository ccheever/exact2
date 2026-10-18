//! Shared field reconciliation and CSS codecs for the optional native editor.
#![allow(unsafe_code)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use exact_kernel::text::decorations::{Decorations, Range};
#[cfg(test)]
use exact_kernel::StyleId;
use serde_json::{json, Map, Value};

/// Resolve a source-backed record and its declaration table for the native field.
pub fn resolve(
    record: &str,
    current: &str,
    styles: &str,
    selection: Range,
    root: f32,
    font: f32,
    scale: f32,
) -> Result<String, String> {
    let record = exact_runner::decorations::decode(record)?;
    let resolved = record.resolve(current);
    let revealed: Vec<bool> = resolved
        .hidden
        .iter()
        .map(|hidden| hidden.revealed(selection))
        .collect();
    let record = Decorations {
        value: current.into(),
        spans: resolved.spans,
        blocks: resolved.blocks,
        hidden: resolved.hidden,
        atoms: resolved.atoms,
    };
    let table = exact_runner::decorations::StyleTable::decode(styles)?;
    let mut output = Map::new();
    for name in table.names() {
        let patch = table
            .resolve(name, root, font, scale)
            .ok_or("decoration styles: unresolved length")?;
        let (style, _) =
            crate::style::style_json_rows(&patch, Some(&exact_kernel::Env::default()), patch.mask);
        output.insert(
            name.into(),
            serde_json::from_str(&style).map_err(|e| e.to_string())?,
        );
    }
    let record: Value = serde_json::from_str(&exact_runner::decorations::encode(&record))
        .map_err(|e| e.to_string())?;
    Ok(json!({"record":record,"provisional":resolved.provisional,"revealed":revealed,"styles":output}).to_string())
}

/// The UTF-8 JSON request carries record/current/styles/selection and font facts.
/// A successful reply uses the existing Markdown handle lifetime.
pub fn read(input: *const u8, len: usize, out: *mut *const u8, count: *mut usize) -> u64 {
    if out.is_null() || count.is_null() {
        return 0;
    }
    unsafe {
        *out = std::ptr::null();
        *count = 0;
    }
    if input.is_null() || len > isize::MAX as usize {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(input, len) };
    let result = (|| {
        let request: Value = serde_json::from_slice(bytes).ok()?;
        let record = request.get("record")?.as_str()?;
        let current = request.get("current")?.as_str()?;
        let styles = request.get("styles")?.as_str()?;
        let selection = Range {
            start: request.get("start")?.as_u64()?.try_into().ok()?,
            end: request.get("end")?.as_u64()?.try_into().ok()?,
        };
        let root = request.get("root")?.as_f64()? as f32;
        let font = request.get("font")?.as_f64()? as f32;
        let scale = request.get("scale")?.as_f64()? as f32;
        if ![root, font, scale]
            .iter()
            .all(|v| v.is_finite() && *v > 0.0)
        {
            return None;
        }
        resolve(record, current, styles, selection, root, font, scale).ok()
    })();
    result.map_or(0, |json| crate::markup::hold_json(json, out, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_record_uses_shared_source_identity_reveal_and_stale_atom_rules() {
        let record = json!({"value":"**a** @x","spans":[{"name":"bold","start":2,"end":3}],
            "blocks":[],"hidden":[{"start":0,"end":2,"reveal":{"start":0,"end":5}}],
            "atoms":[{"name":"chip","start":6,"end":8,"label":"x"}]})
        .to_string();
        let first: Value = serde_json::from_str(
            &resolve(
                &record,
                "**a** @x",
                "{}",
                Range { start: 2, end: 2 },
                16.,
                14.,
                2.,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(first["provisional"], false);
        assert_eq!(first["revealed"], json!([true]));
        let stale: Value = serde_json::from_str(
            &resolve(
                &record,
                "**b** @xx",
                "{}",
                Range { start: 9, end: 9 },
                16.,
                14.,
                2.,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(stale["provisional"], true);
        assert_eq!(stale["record"]["value"], "**b** @xx");
        assert_eq!(stale["record"]["atoms"], json!([]));
        assert_eq!(stale["revealed"], json!([false]));
    }
    #[test]
    fn native_styles_use_the_kernel_length_color_and_line_height_codecs() {
        let record = r#"{"value":"hi","spans":[],"blocks":[],"hidden":[],"atoms":[]}"#;
        let rows = json!({"heading":[[StyleId::FontSize as u16,"1.25rem"],
            [StyleId::PaddingLeft as u16,"0.8em"],[StyleId::LineHeight as u16,1.3],
            [StyleId::TextColor as u16,"light-dark(#123456, #abcdef)"],
            [StyleId::MarginTop as u16,"1rem"]]})
        .to_string();
        let reply: Value = serde_json::from_str(
            &resolve(record, "hi", &rows, Range::default(), 20., 14., 2.).unwrap(),
        )
        .unwrap();
        assert_eq!(reply["styles"]["heading"]["font_size"], 25);
        assert_eq!(reply["styles"]["heading"]["padding_left"], 20);
        assert_eq!(reply["styles"]["heading"]["margin_top"], 20);
        assert_eq!(reply["styles"]["heading"]["line_height"], 1.3);
        assert_eq!(
            reply["styles"]["heading"]["text_color"],
            json!([[18, 52, 86, 255], [171, 205, 239, 255]])
        );
    }
}
