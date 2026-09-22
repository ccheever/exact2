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

/// Release one expansion. Zero, stale, and repeated frees are no-ops.
pub fn free(handle: u64) {
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
        /// Release one expansion.
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
}
