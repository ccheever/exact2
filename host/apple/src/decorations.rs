//! A textarea's decorations for the Apple field (LLP 1114.007): the app's
//! record resolved against the field's text by the kernel's rule, and each
//! visible declaration's paint rows in the batch's style encoding.
#![allow(unsafe_code)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use serde_json::{json, Map, Value};
use std::cell::RefCell;

/// `{provisional, spans: [{name, start, end}], styles: {name: rows}}`.
pub fn resolve(record: &str, current: &str, styles: &str) -> Result<String, String> {
    let resolved = exact_runner::decorations::decode(record)?.resolve(current);
    let table = exact_runner::decorations::StyleTable::decode(styles)?;
    let mut rows = Map::new();
    for name in table.names() {
        let Some(patch) = table.get(name) else {
            continue;
        };
        let (style, _) =
            crate::style::style_json_rows(patch, Some(&exact_kernel::Env::default()), patch.mask);
        rows.insert(
            name.into(),
            serde_json::from_str(&style).map_err(|e| e.to_string())?,
        );
    }
    let spans: Vec<Value> = resolved
        .spans
        .iter()
        .map(|s| json!({"name": s.name, "start": s.range.start, "end": s.range.end}))
        .collect();
    Ok(json!({"provisional": resolved.provisional, "spans": spans, "styles": rows}).to_string())
}

thread_local! {
    static REPLY: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// The UTF-8 JSON request `{record, current, styles}`; the reply is valid
/// until the next call on this thread, and null (with `count` zero) for a
/// request or record that does not parse.
pub fn read(input: *const u8, len: usize, count: *mut usize) -> *const u8 {
    if count.is_null() {
        return std::ptr::null();
    }
    unsafe { *count = 0 };
    if input.is_null() || len > isize::MAX as usize {
        return std::ptr::null();
    }
    let bytes = unsafe { std::slice::from_raw_parts(input, len) };
    let reply = (|| {
        let request: Value = serde_json::from_slice(bytes).ok()?;
        let field = |name| request.get(name)?.as_str();
        resolve(field("record")?, field("current")?, field("styles")?).ok()
    })();
    let Some(reply) = reply else {
        return std::ptr::null();
    };
    REPLY.with_borrow_mut(|held| {
        *held = reply.into_bytes();
        unsafe { *count = held.len() };
        held.as_ptr()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::StyleId;

    #[test]
    fn a_record_resolves_by_the_kernels_rule_with_its_paint_rows() {
        let record =
            json!({"value":"hi @a","spans":[{"name":"mention","start":3,"end":5}]}).to_string();
        let rows = json!({"mention":[[StyleId::TextColor as u16,"light-dark(#123456, #abcdef)"],
            [StyleId::TextDecorationLine as u16,"underline"]]})
        .to_string();
        let reply: Value =
            serde_json::from_str(&resolve(&record, "hi @a", &rows).unwrap()).unwrap();
        assert_eq!(reply["provisional"], false);
        assert_eq!(
            reply["spans"],
            json!([{"name":"mention","start":3,"end":5}])
        );
        assert_eq!(
            reply["styles"]["mention"]["text_color"],
            json!([[18, 52, 86, 255], [171, 205, 239, 255]])
        );
        assert_eq!(
            reply["styles"]["mention"]["text_decoration_line"],
            "underline"
        );
        let stale: Value =
            serde_json::from_str(&resolve(&record, "hi! @a", &rows).unwrap()).unwrap();
        assert_eq!(stale["provisional"], true);
        assert_eq!(
            stale["spans"],
            json!([{"name":"mention","start":4,"end":6}])
        );
    }

    #[test]
    fn a_bad_request_has_no_reply() {
        let mut count = 7;
        for bad in ["", "{}", r#"{"record":"x","current":"","styles":"{}"}"#] {
            assert!(read(bad.as_ptr(), bad.len(), &mut count).is_null(), "{bad}");
            assert_eq!(count, 0);
        }
        let good = json!({"record": json!({"value":"","spans":[]}).to_string(), "current": "", "styles": "{}"})
            .to_string();
        assert!(!read(good.as_ptr(), good.len(), &mut count).is_null());
        assert!(count > 0);
    }
}
