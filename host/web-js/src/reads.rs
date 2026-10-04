//! What an emitted closure reads of a row (LLP 1071.000): the fields of the
//! row's item a binding reads, for its effect's mask (D3), and whether a key
//! reads nothing but the row's item and index (D2). Both read the emitted
//! JavaScript, whose item reads are `i<r>()` and field reads `[k]` (code.rs);
//! string literals are skipped, and anything else is read conservatively.

/// The code's words outside string literals, each with the text after it.
fn words(code: &str) -> Vec<(&str, &str)> {
    let b = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    while i < b.len() {
        match b[i] {
            q @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            c if word(c) => {
                let start = i;
                while i < b.len() && word(b[i]) {
                    i += 1;
                }
                // A property name (`.i0`) is not a read of a variable.
                if start == 0 || b[start - 1] != b'.' {
                    out.push((&code[start..i], &code[i..]));
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// A row item's variable: `i` and digits.
fn is_item(w: &str) -> bool {
    w.len() > 1 && w.starts_with('i') && w[1..].bytes().all(|c| c.is_ascii_digit())
}

/// The mask of the item fields `code` reads, when it reads one row item and
/// only as `i<r>()[k]` with `k` below 31; `None` otherwise.
pub fn field_mask(code: &str) -> Option<u32> {
    let mut item: Option<&str> = None;
    let mut mask = 0u32;
    for (w, rest) in words(code) {
        if !is_item(w) {
            continue;
        }
        if item.is_some_and(|i| i != w) {
            return None;
        }
        item = Some(w);
        let k: u32 = rest
            .strip_prefix("()[")
            .and_then(|r| r.split_once(']'))
            .and_then(|(k, _)| k.parse().ok())
            .filter(|k| *k < 31)?;
        mask |= 1 << k;
    }
    item.map(|_| mask)
}

/// Whether a key's code reads nothing but `item`, `index` and the runtime's
/// pure helpers (`x_…`, budget.js's checked `cc` and `K`).
pub fn pure_key(code: &str, item: &str, index: &str) -> bool {
    words(code).into_iter().all(|(w, _)| {
        w == item
            || w == index
            || w.starts_with("x_")
            || matches!(w, "cc" | "K")
            || w.as_bytes()[0].is_ascii_digit()
            || matches!(w, "true" | "false" | "null" | "undefined")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binding_reading_fields_of_one_item_has_their_mask() {
        assert_eq!(field_mask("()=>i0()[2]"), Some(1 << 2));
        assert_eq!(
            field_mask("()=>\"row-\"+x_toString(i0()[0])+i0()[7]"),
            Some(1 | 1 << 7)
        );
        // The item's own literal text is not a read.
        assert_eq!(field_mask("()=>\"i0\"+i0()[1]"), Some(2));
        assert_eq!(
            field_mask("()=>i0()[6].map((l0,l1)=>l0+i0()[9])"),
            Some(1 << 6 | 1 << 9)
        );
    }

    #[test]
    fn any_other_read_of_an_item_has_no_mask() {
        // The whole item, a dynamic field, a field past 30, two items, none.
        assert_eq!(field_mask("()=>f(i0())"), None);
        assert_eq!(field_mask("()=>i0()[l0]"), None);
        assert_eq!(field_mask("()=>i0()[31]"), None);
        assert_eq!(field_mask("()=>i0()[1]+i4()[1]"), None);
        assert_eq!(field_mask("()=>d_1()[2]"), None);
        // `i10` is not `i1`.
        assert_eq!(field_mask("()=>i10()[3]"), Some(8));
    }

    #[test]
    fn a_key_is_pure_when_it_reads_only_its_row() {
        assert!(pure_key("i0()[0]", "i0", "x0"));
        assert!(pure_key("\"k\"+x_toString(i0()[0])+x0()", "i0", "x0"));
        assert!(pure_key("cc(\"k\",x_toString(i0()[0]),3)", "i0", "x0"));
        assert!(!pure_key("i0()[0]+s_3()", "i0", "x0"));
        assert!(!pure_key("i0()[0]+d_1()", "i0", "x0"));
        assert!(!pure_key("i1()[0]", "i0", "x0"));
    }
}
