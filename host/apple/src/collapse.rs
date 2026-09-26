//! CSS white space collapsing for the Swift painter (LLP 1053 §0 G5).
//!
//! The measurer collapses a request's runs in Rust before the callback
//! (`measure.rs`); the presenter builds its own paint runs from node rows and
//! asks here, so both sides shape exactly the same text with one algorithm
//! (`exact_textflow::collapse`). Buffers are borrowed for the call only.
#![allow(unsafe_code)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

/// One change of shift between collapsed and source UTF-16 offsets: from
/// collapsed `utf16` on, `source = collapsed + removed`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CEdit {
    /// Collapsed UTF-16 offset.
    pub utf16: usize,
    /// Source units removed before it.
    pub removed: usize,
}

fn slice<'a, T>(p: *const T, n: usize) -> Option<&'a [T]> {
    if n == 0 {
        Some(&[])
    } else if p.is_null() || n > isize::MAX as usize / std::mem::size_of::<T>() {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(p, n) })
    }
}

/// Collapse `count` runs whose UTF-8 text is `utf8` joined and whose byte
/// lengths are `lens`, under CSS `white-space` `white_space` (the measure
/// ABI's code: 0 normal, 1 pre-wrap, 2 nowrap, 3 pre-line). Returns 0 when nothing collapses or the input is
/// invalid, leaving the outputs untouched. Otherwise returns the edit count
/// plus one and, when the outputs are non-null, writes the collapsed text
/// (never longer than `len`) to `out`, each run's collapsed byte length to
/// `out_lens`, and at most `edit_cap` edits.
#[allow(clippy::too_many_arguments)]
pub fn collapse(
    utf8: *const u8,
    len: usize,
    lens: *const usize,
    count: usize,
    white_space: u8,
    out: *mut u8,
    out_lens: *mut usize,
    edits: *mut CEdit,
    edit_cap: usize,
) -> usize {
    let (Some(bytes), Some(lens)) = (slice(utf8, len), slice(lens, count)) else {
        return 0;
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return 0;
    };
    let mut runs = Vec::with_capacity(count);
    let mut at = 0usize;
    for &n in lens {
        let Some(run) = at.checked_add(n).and_then(|end| text.get(at..end)) else {
            return 0;
        };
        runs.push(run);
        at += n;
    }
    if at != len {
        return 0;
    }
    let mode = crate::textflow::white_space_mode(u32::from(white_space));
    let Some(collapsed) = exact_textflow::collapse(&runs, mode) else {
        return 0;
    };
    if !out.is_null() && !out_lens.is_null() {
        let mut offset = 0;
        for (i, run) in collapsed.runs.iter().enumerate() {
            unsafe {
                std::ptr::copy_nonoverlapping(run.as_ptr(), out.add(offset), run.len());
                *out_lens.add(i) = run.len();
            }
            offset += run.len();
        }
    }
    if !edits.is_null() {
        for (i, e) in collapsed.edits().iter().take(edit_cap).enumerate() {
            unsafe {
                *edits.add(i) = CEdit {
                    utf16: e.utf16,
                    removed: e.removed,
                };
            }
        }
    }
    collapsed.edits().len() + 1
}

/// Export the collapsing seam from the application's static archive.
#[macro_export]
macro_rules! collapse_exports {
    () => {
        /// CSS white space collapsing of a paragraph's runs.
        #[no_mangle]
        #[allow(clippy::too_many_arguments)]
        pub extern "C" fn exact_text_collapse(
            utf8: *const u8,
            len: usize,
            lens: *const usize,
            count: usize,
            white_space: u8,
            out: *mut u8,
            out_lens: *mut usize,
            edits: *mut $crate::collapse::CEdit,
            edit_cap: usize,
        ) -> usize {
            $crate::collapse::collapse(
                utf8,
                len,
                lens,
                count,
                white_space,
                out,
                out_lens,
                edits,
                edit_cap,
            )
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_across_runs_and_reports_edits() {
        let text = "a  \n".to_string() + " b\tc  ";
        let lens = [4usize, 6];
        let mut out = vec![0u8; text.len()];
        let mut out_lens = [0usize; 2];
        let mut edits = [CEdit::default(); 8];
        let n = collapse(
            text.as_ptr(),
            text.len(),
            lens.as_ptr(),
            2,
            0,
            out.as_mut_ptr(),
            out_lens.as_mut_ptr(),
            edits.as_mut_ptr(),
            8,
        );
        assert!(n > 1);
        let total: usize = out_lens.iter().sum();
        assert_eq!(std::str::from_utf8(&out[..total]).unwrap(), "a b c");
        assert_eq!(out_lens, [2, 3]);
        assert_eq!(
            edits[0],
            CEdit {
                utf16: 2,
                removed: 3
            }
        );
        // `pre-line` (3) keeps the line feed and drops the spaces beside it.
        let mut out = vec![0u8; text.len()];
        let n = collapse(
            text.as_ptr(),
            text.len(),
            lens.as_ptr(),
            2,
            3,
            out.as_mut_ptr(),
            out_lens.as_mut_ptr(),
            edits.as_mut_ptr(),
            8,
        );
        assert!(n > 1);
        let total: usize = out_lens.iter().sum();
        assert_eq!(std::str::from_utf8(&out[..total]).unwrap(), "a\nb c");
        assert_eq!(out_lens, [2, 3]);
        // Nothing to collapse: 0 and the outputs untouched.
        let clean = "a b";
        assert_eq!(
            collapse(
                clean.as_ptr(),
                3,
                [3usize].as_ptr(),
                1,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0
            ),
            0
        );
        // A run boundary inside a character is refused.
        let e = "é";
        assert_eq!(
            collapse(
                e.as_ptr(),
                2,
                [1usize, 1].as_ptr(),
                2,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0
            ),
            0
        );
    }
}
