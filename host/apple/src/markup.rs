//! Markdown into display pieces for the Swift text engine (LLP 1045 D3, D4).
//!
//! The kernel hands a `markup="markdown"` node's source to the measurer as
//! one run with the request's `markup` flag set. Swift expands it here, with
//! the one function that also expands it for painting, so what is measured is
//! what is drawn. Pieces borrow a handle the caller frees; the text pointers
//! stay valid until then. Handles are thread-confined; stale and null handles
//! are harmless; pointer buffers are borrowed only during a call, as with the
//! measure callback and the textflow seam.
#![allow(unsafe_code)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::cell::RefCell;
use std::collections::HashMap;

/// One piece, as the callback sees it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CPiece {
    /// UTF-8, not NUL-terminated.
    pub text: *const u8,
    /// Bytes.
    pub len: usize,
    /// Font size relative to the node's own.
    pub scale: f32,
    /// CSS weight; 0 keeps the node's own.
    pub weight: u16,
    /// 1 for italic.
    pub italic: u8,
    /// 1 for monospace.
    pub mono: u8,
    /// 1 for strikethrough.
    pub strike: u8,
    /// 0 ink, 1 code, 2 link, 3 marker, 4 quote.
    pub role: u8,
    /// A link's target, UTF-8; null when none.
    pub href: *const u8,
    /// Its length.
    pub href_len: usize,
}

/// An expansion kept alive for its handle: the pieces own the text the
/// flat records point into.
struct Held {
    _pieces: Vec<exact_markdown::Piece>,
    _flat: Vec<CPiece>,
}

fn slice<'a>(p: *const u8, n: usize) -> Option<&'a [u8]> {
    if n == 0 {
        Some(&[])
    } else if p.is_null() || n > isize::MAX as usize {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(p, n) })
    }
}

thread_local! {
    static HELD: RefCell<HashMap<u64, Held>> = RefCell::new(HashMap::new());
    static STYLES: RefCell<HashMap<u64, String>> = RefCell::new(HashMap::new());
    static NEXT: RefCell<u64> = const { RefCell::new(1) };
}

/// Expand `len` bytes of Markdown at `text`. Writes the pieces' address and
/// count and returns a handle; zero (and no pieces) for invalid input.
pub fn pieces(text: *const u8, len: usize, out: *mut *const CPiece, count: *mut usize) -> u64 {
    if out.is_null() || count.is_null() {
        return 0;
    }
    unsafe {
        *out = std::ptr::null();
        *count = 0;
    }
    let Some(source) = slice(text, len).and_then(|b| std::str::from_utf8(b).ok()) else {
        return 0;
    };
    let pieces = exact_markdown::pieces(source);
    let flat: Vec<CPiece> = pieces
        .iter()
        .map(|p| CPiece {
            text: p.text.as_ptr(),
            len: p.text.len(),
            scale: p.scale,
            weight: p.weight,
            italic: u8::from(p.italic),
            mono: u8::from(p.mono),
            strike: u8::from(p.strike),
            role: match p.role {
                exact_markdown::Role::Ink => 0,
                exact_markdown::Role::Code => 1,
                exact_markdown::Role::Link => 2,
                exact_markdown::Role::Marker => 3,
                exact_markdown::Role::Quote => 4,
            },
            href: if p.href.is_empty() {
                std::ptr::null()
            } else {
                p.href.as_ptr()
            },
            href_len: p.href.len(),
        })
        .collect();
    let handle = NEXT.with_borrow_mut(|n| {
        let h = *n;
        *n += 1;
        h
    });
    unsafe {
        *out = flat.as_ptr();
        *count = flat.len();
    }
    HELD.with_borrow_mut(|held| {
        held.insert(
            handle,
            Held {
                _pieces: pieces,
                _flat: flat,
            },
        )
    });
    handle
}

/// How to draw `len` bytes of Markdown at `text` while editing, with the
/// selection `sel_start..sel_end` (UTF-16; `u32::MAX` start for none), as one
/// JSON document the editor reads: `{"p":[[start,end,kind,level,depth,quote]…],
/// "s":[[start,end,flags,href]…],"h":[[start,end]…],"r":[[start,end,kind,text]…]}`.
/// Kinds: paragraph 0 body 1 heading 2 bullet 3 ordered 4 task 5 rule 6 fence
/// 7 code 8 footnote 9 table 10 embed 11 image 12 video; replaced 0 bullet
/// 1 box 2 checked box 3 rule 4 footnote. Writes the text's address and
/// length; returns a handle to free. Zero for invalid input.
pub fn style(
    text: *const u8,
    len: usize,
    sel_start: u32,
    sel_end: u32,
    out: *mut *const u8,
    count: *mut usize,
) -> u64 {
    if out.is_null() || count.is_null() {
        return 0;
    }
    unsafe {
        *out = std::ptr::null();
        *count = 0;
    }
    let Some(source) = slice(text, len).and_then(|b| std::str::from_utf8(b).ok()) else {
        return 0;
    };
    let reveal = (sel_start != u32::MAX).then(|| exact_markdown::Range::new(sel_start, sel_end));
    let json = exact_markdown::wire::style(source, reveal);
    hold_json(json, out, count)
}

fn hold_json(json: String, out: *mut *const u8, count: *mut usize) -> u64 {
    let handle = NEXT.with_borrow_mut(|n| {
        let h = *n;
        *n += 1;
        h
    });
    unsafe {
        *out = json.as_ptr();
        *count = json.len();
    }
    STYLES.with_borrow_mut(|held| held.insert(handle, json));
    handle
}

fn text_input<'a>(
    text: *const u8,
    len: usize,
    out: *mut *const u8,
    count: *mut usize,
) -> Option<&'a str> {
    if out.is_null() || count.is_null() {
        return None;
    }
    unsafe {
        *out = std::ptr::null();
        *count = 0;
    }
    slice(text, len).and_then(|b| std::str::from_utf8(b).ok())
}

/// Shared source-relative edits, serialized as JSON; the caller frees the handle.
#[allow(clippy::too_many_arguments)]
pub fn edit(
    text: *const u8,
    len: usize,
    start: u32,
    end: u32,
    command: *const u8,
    command_len: usize,
    argument: *const u8,
    argument_len: usize,
    out: *mut *const u8,
    count: *mut usize,
) -> u64 {
    let Some(source) = text_input(text, len, out, count) else {
        return 0;
    };
    let Some(command) = slice(command, command_len).and_then(|b| std::str::from_utf8(b).ok())
    else {
        return 0;
    };
    let Some(argument) = slice(argument, argument_len).and_then(|b| std::str::from_utf8(b).ok())
    else {
        return 0;
    };
    hold_json(
        exact_markdown::wire::edit(
            source,
            exact_markdown::Range::new(start, end),
            command,
            argument,
        ),
        out,
        count,
    )
}

/// Shared toolbar facts, serialized as JSON; no offsets cross into app state.
pub fn selection(
    text: *const u8,
    len: usize,
    start: u32,
    end: u32,
    out: *mut *const u8,
    count: *mut usize,
) -> u64 {
    let Some(source) = text_input(text, len, out, count) else {
        return 0;
    };
    hold_json(
        exact_markdown::wire::selection(source, exact_markdown::Range::new(start, end)),
        out,
        count,
    )
}

/// Copy Markdown as plain UTF-8 text; the caller frees the returned handle.
pub fn plain(text: *const u8, len: usize, out: *mut *const u8, count: *mut usize) -> u64 {
    let Some(source) = text_input(text, len, out, count) else {
        return 0;
    };
    hold_json(exact_markdown::plain(source), out, count)
}

/// Release one expansion or styling. Zero, stale, and repeated frees are no-ops.
pub fn free(handle: u64) {
    STYLES.with_borrow_mut(|held| {
        held.remove(&handle);
    });
    HELD.with_borrow_mut(|held| {
        held.remove(&handle);
    });
}

/// Export the markup seam from the application's static archive.
#[macro_export]
macro_rules! markup_exports {
    () => {
        /// Expand Markdown source into display pieces.
        #[no_mangle]
        pub extern "C" fn exact_markup_pieces(
            text: *const u8,
            len: usize,
            out: *mut *const $crate::markup::CPiece,
            count: *mut usize,
        ) -> u64 {
            $crate::markup::pieces(text, len, out, count)
        }
        /// Style Markdown source for editing, as JSON.
        #[no_mangle]
        pub extern "C" fn exact_markup_style(
            text: *const u8,
            len: usize,
            sel_start: u32,
            sel_end: u32,
            out: *mut *const u8,
            count: *mut usize,
        ) -> u64 {
            $crate::markup::style(text, len, sel_start, sel_end, out, count)
        }
        /// Apply a source-relative formatting command, as JSON replacements.
        #[no_mangle]
        pub extern "C" fn exact_markup_edit(
            text: *const u8,
            len: usize,
            start: u32,
            end: u32,
            command: *const u8,
            command_len: usize,
            argument: *const u8,
            argument_len: usize,
            out: *mut *const u8,
            count: *mut usize,
        ) -> u64 {
            $crate::markup::edit(
                text,
                len,
                start,
                end,
                command,
                command_len,
                argument,
                argument_len,
                out,
                count,
            )
        }
        /// Read toolbar selection facts as JSON.
        #[no_mangle]
        pub extern "C" fn exact_markup_selection(
            text: *const u8,
            len: usize,
            start: u32,
            end: u32,
            out: *mut *const u8,
            count: *mut usize,
        ) -> u64 {
            $crate::markup::selection(text, len, start, end, out, count)
        }
        /// Copy Markdown as plain UTF-8 text.
        #[no_mangle]
        pub extern "C" fn exact_markup_plain(
            text: *const u8,
            len: usize,
            out: *mut *const u8,
            count: *mut usize,
        ) -> u64 {
            $crate::markup::plain(text, len, out, count)
        }
        /// Release one expansion or styling.
        #[no_mangle]
        pub extern "C" fn exact_markup_free(handle: u64) {
            $crate::markup::free(handle)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_cross_the_seam_and_free() {
        let source = "# T\n\n**b** [l](u)";
        let (mut out, mut count) = (std::ptr::null(), 0usize);
        let handle = pieces(source.as_ptr(), source.len(), &mut out, &mut count);
        assert!(handle != 0 && count >= 4);
        let flat = unsafe { std::slice::from_raw_parts(out, count) };
        let first = unsafe { std::slice::from_raw_parts(flat[0].text, flat[0].len) };
        assert_eq!(first, b"T");
        assert_eq!(flat[0].weight, 700);
        let link = flat.iter().find(|p| p.role == 2).unwrap();
        assert_eq!(
            unsafe { std::slice::from_raw_parts(link.href, link.href_len) },
            b"u"
        );
        free(handle);
        free(handle);
        assert_eq!(pieces(b"\xff".as_ptr(), 1, &mut out, &mut count), 0);
        assert_eq!(count, 0);
    }

    #[test]
    fn editing_selection_and_plain_share_the_handle_lifetime() {
        let source = "😀 word";
        let (mut out, mut count) = (std::ptr::null(), 0usize);
        let handle = edit(
            source.as_ptr(),
            source.len(),
            3,
            7,
            b"bold".as_ptr(),
            4,
            std::ptr::null(),
            0,
            &mut out,
            &mut count,
        );
        assert_ne!(handle, 0);
        let json = std::str::from_utf8(unsafe { std::slice::from_raw_parts(out, count) }).unwrap();
        assert_eq!(
            json,
            r#"{"replacements":[[3,3,"**"],[7,7,"**"]],"selection":[5,9]}"#
        );
        free(handle);
        let source = "**bold**";
        let handle = selection(source.as_ptr(), source.len(), 3, 3, &mut out, &mut count);
        let json = std::str::from_utf8(unsafe { std::slice::from_raw_parts(out, count) }).unwrap();
        assert_eq!(
            json,
            r#"{"formats":"bold","mixed":false,"link":"","unavailable":""}"#
        );
        free(handle);
        let handle = plain(source.as_ptr(), source.len(), &mut out, &mut count);
        assert_eq!(unsafe { std::slice::from_raw_parts(out, count) }, b"bold");
        free(handle);
        assert_eq!(
            edit(
                source.as_ptr(),
                source.len(),
                0,
                0,
                b"\xff".as_ptr(),
                1,
                std::ptr::null(),
                0,
                &mut out,
                &mut count
            ),
            0
        );
        assert!(out.is_null());
        assert_eq!(count, 0);
    }

    #[test]
    fn styling_crosses_the_seam_as_json() {
        let source = "# T\n\n**b** [l](u \"q\")";
        let (mut out, mut count) = (std::ptr::null(), 0usize);
        let handle = style(
            source.as_ptr(),
            source.len(),
            u32::MAX,
            0,
            &mut out,
            &mut count,
        );
        assert!(handle != 0);
        let json = std::str::from_utf8(unsafe { std::slice::from_raw_parts(out, count) }).unwrap();
        assert_eq!(
            json,
            r#"{"p":[[0,3,1,1,0,0],[5,21,0,0,0,0]],"s":[[7,8,1,""],[12,13,16,"u"]],"h":[[0,2],[5,7],[8,10],[11,12],[13,21]],"r":[]}"#
        );
        // The caret inside the bold reveals its markers as MARKER spans.
        let revealed = style(source.as_ptr(), source.len(), 7, 7, &mut out, &mut count);
        let json = std::str::from_utf8(unsafe { std::slice::from_raw_parts(out, count) }).unwrap();
        assert!(
            json.contains(r#"[5,7,64,""]"#) && json.contains(r#"[8,10,64,""]"#),
            "{json}"
        );
        free(handle);
        free(revealed);
    }
}
