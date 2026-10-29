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
    /// CSS px: the head indent of the paragraph it is in (LLP 1045 D4).
    pub indent: f32,
    /// 1 for a list marker, hung before the indent with its end at it.
    pub hang: u8,
    /// An inline atom's detail as JSON (LLP 1045.000 D6): `{kind, role,
    /// icon, title, href, source, start, end, color, background}`, its text
    /// the label, its colours its face's in the colour wire form (null for
    /// the kind's); null for any other piece.
    pub atom: *const u8,
    /// Its length.
    pub atom_len: usize,
}

/// An expansion kept alive for its handle: the pieces own the text the
/// flat records point into.
struct Held {
    _pieces: Vec<exact_markdown::Piece>,
    _flat: Vec<CPiece>,
    _atoms: Vec<String>,
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
    static ATOMS: RefCell<HashMap<u64, std::rc::Rc<exact_markdown::Atoms>>> = RefCell::new(HashMap::new());
    static NEXT: RefCell<u64> = const { RefCell::new(1) };
}

fn next_handle() -> u64 {
    NEXT.with_borrow_mut(|n| {
        let h = *n;
        *n += 1;
        h
    })
}

/// A node's `atoms` and `atomFaces` JSON, parsed once for the calls that
/// take its handle (LLP 1045.000 D2). Freed by [`free`]; invalid UTF-8 reads
/// as none.
pub fn atoms(kinds: *const u8, kinds_len: usize, faces: *const u8, faces_len: usize) -> u64 {
    let text = |p, n| {
        slice(p, n)
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
    };
    let parsed = exact_markdown::Atoms::parse(text(kinds, kinds_len), text(faces, faces_len));
    let handle = next_handle();
    ATOMS.with_borrow_mut(|held| held.insert(handle, std::rc::Rc::new(parsed)));
    handle
}

/// The atoms behind `handle`; none for zero or a freed handle.
fn atoms_of(handle: u64) -> std::rc::Rc<exact_markdown::Atoms> {
    ATOMS
        .with_borrow(|held| held.get(&handle).cloned())
        .unwrap_or_default()
}

/// A face's `color` or `backgroundColor` (LLP 1045.000 D2), as the runner
/// wrote it (`atom_face_paint`), in the presenters' colour wire form, which
/// they resolve for the appearance they draw in; null where the kind's
/// paint stands.
fn face_paint(text: &str) -> serde_json::Value {
    // No session's overrides: the runner resolved the session's into the text.
    let _roles = exact_kernel::style::roles::bind(&Default::default());
    exact_runner::decorations::face_color(text)
        .and_then(|color| {
            let mut wire = String::new();
            crate::style::push_color_value(&mut wire, color);
            serde_json::from_str(&wire).ok()
        })
        .unwrap_or_default()
}

/// Expand `len` bytes of Markdown at `text`, with the atoms behind
/// `atoms` (zero for none). Writes the pieces' address and count and returns
/// a handle; zero (and no pieces) for invalid input.
pub fn pieces(
    text: *const u8,
    len: usize,
    atoms: u64,
    out: *mut *const CPiece,
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
    let pieces = exact_markdown::pieces_with(source, &atoms_of(atoms));
    let details: Vec<String> = pieces
        .iter()
        .map(|p| match (&p.atom, p.source) {
            (Some(a), Some(range)) => serde_json::json!({
                "kind": a.kind, "icon": a.icon, "title": a.title, "href": a.href, "source": a.source,
                "role": if a.role == exact_markdown::AtomRole::Link { "link" } else { "button" },
                "start": range.start, "end": range.end,
                "color": face_paint(&a.color), "background": face_paint(&a.background_color),
            })
            .to_string(),
            _ => String::new(),
        })
        .collect();
    let flat: Vec<CPiece> = pieces
        .iter()
        .zip(&details)
        .map(|(p, detail)| CPiece {
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
            indent: p.indent,
            hang: u8::from(p.hang),
            atom: if detail.is_empty() {
                std::ptr::null()
            } else {
                detail.as_ptr()
            },
            atom_len: detail.len(),
        })
        .collect();
    let handle = next_handle();
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
                _atoms: details,
            },
        )
    });
    handle
}

/// How to draw `len` bytes of Markdown at `text` while editing, with the
/// selection `sel_start..sel_end` (UTF-16; `u32::MAX` start for none), as one
/// JSON document the editor reads: `{"p":[[start,end,kind,level,depth,quote]…],
/// "s":[[start,end,flags,href]…],"h":[[start,end]…],"r":[[start,end,kind,text]…],
/// "l":[[open,open_end,start,end]…]}`.
/// Kinds: paragraph 0 body 1 heading 2 bullet 3 ordered 4 task 5 rule 6 fence
/// 7 code 8 footnote 9 table 10 embed 11 image 12 video; replaced 0 bullet
/// 1 box 2 checked box 3 rule 4 footnote. `a` is each inline atom (LLP
/// 1045.000): `[start,end,kind,label,icon,title,role,href,ink,background]`,
/// the last two its face's paint in the colour wire form (null for the
/// kind's); no hidden range lies inside one and no atom link is in `l`.
/// `l` is each link's opener and its `](target)`, which the editor hides
/// whatever the selection, as the web editor does
/// (`exact_markdown_editor::link_destinations`, LLP 1045 D5). Writes the
/// text's address and length; returns a handle to free. Zero for invalid
/// input.
#[allow(clippy::too_many_arguments)]
pub fn style(
    text: *const u8,
    len: usize,
    atoms: u64,
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
    // One parse for both: the styling, and the link syntax in it.
    let atoms = atoms_of(atoms);
    let styled = exact_markdown::style_with(source, reveal, &atoms);
    let mut json = exact_markdown::wire::styled(&styled);
    // The document is one object: its closing brace makes room for `l`.
    json.pop();
    json.push_str(",\"l\":[");
    for (i, [o, oe, c, ce]) in exact_markdown_editor::link_destinations(source, &styled)
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!("[{o},{oe},{c},{ce}]"));
    }
    json.push_str("],\"a\":");
    let rows: Vec<serde_json::Value> = styled
        .atoms
        .iter()
        .map(|a| {
            let kind = &atoms.kinds[a.kind];
            let face = atoms.face(&a.source).cloned().unwrap_or_default();
            let role = if kind.role == exact_markdown::AtomRole::Link {
                "link"
            } else {
                "button"
            };
            serde_json::json!([
                a.range.start,
                a.range.end,
                kind.name,
                a.label,
                face.icon,
                face.title,
                role,
                a.href,
                face_paint(&face.color),
                face_paint(&face.background_color)
            ])
        })
        .collect();
    json.push_str(&serde_json::Value::Array(rows).to_string());
    json.push('}');
    hold_json(json, out, count)
}

fn hold_json(json: String, out: *mut *const u8, count: *mut usize) -> u64 {
    let handle = next_handle();
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
/// The selected atom's source is `atom` (LLP 1045.000 D3).
pub fn selection(
    text: *const u8,
    len: usize,
    atoms: u64,
    start: u32,
    end: u32,
    out: *mut *const u8,
    count: *mut usize,
) -> u64 {
    let Some(source) = text_input(text, len, out, count) else {
        return 0;
    };
    hold_json(
        exact_markdown::wire::selection_with(
            source,
            exact_markdown::Range::new(start, end),
            &atoms_of(atoms),
        ),
        out,
        count,
    )
}

/// Copy the UTF-16 range `start..end` of Markdown as plain UTF-8 text, each
/// atom the whole source recognizes inside it as its label; the caller frees
/// the returned handle.
pub fn plain(
    text: *const u8,
    len: usize,
    atoms: u64,
    start: u32,
    end: u32,
    out: *mut *const u8,
    count: *mut usize,
) -> u64 {
    let Some(source) = text_input(text, len, out, count) else {
        return 0;
    };
    hold_json(
        exact_markdown::plain_range(
            source,
            exact_markdown::Range::new(start, end),
            &atoms_of(atoms),
        ),
        out,
        count,
    )
}

/// Release one expansion or styling. Zero, stale, and repeated frees are no-ops.
pub fn free(handle: u64) {
    // A handle freed from a thread's own destructors at its exit (the
    // atoms' per-thread cache) may find these tables already gone, and
    // with them everything there was to free.
    let _ = STYLES.try_with(|held| held.borrow_mut().remove(&handle));
    let _ = HELD.try_with(|held| held.borrow_mut().remove(&handle));
    let _ = ATOMS.try_with(|held| held.borrow_mut().remove(&handle));
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
            atoms: u64,
            out: *mut *const $crate::markup::CPiece,
            count: *mut usize,
        ) -> u64 {
            $crate::markup::pieces(text, len, atoms, out, count)
        }
        /// Parse a node's `atoms` and `atomFaces` for the calls that take atoms.
        #[no_mangle]
        pub extern "C" fn exact_markup_atoms(
            kinds: *const u8,
            kinds_len: usize,
            faces: *const u8,
            faces_len: usize,
        ) -> u64 {
            $crate::markup::atoms(kinds, kinds_len, faces, faces_len)
        }
        /// Style Markdown source for editing, as JSON.
        #[no_mangle]
        pub extern "C" fn exact_markup_style(
            text: *const u8,
            len: usize,
            atoms: u64,
            sel_start: u32,
            sel_end: u32,
            out: *mut *const u8,
            count: *mut usize,
        ) -> u64 {
            $crate::markup::style(text, len, atoms, sel_start, sel_end, out, count)
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
            atoms: u64,
            start: u32,
            end: u32,
            out: *mut *const u8,
            count: *mut usize,
        ) -> u64 {
            $crate::markup::selection(text, len, atoms, start, end, out, count)
        }
        /// Copy a range of Markdown as plain UTF-8 text.
        #[no_mangle]
        pub extern "C" fn exact_markup_plain(
            text: *const u8,
            len: usize,
            atoms: u64,
            start: u32,
            end: u32,
            out: *mut *const u8,
            count: *mut usize,
        ) -> u64 {
            $crate::markup::plain(text, len, atoms, start, end, out, count)
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
        let handle = pieces(source.as_ptr(), source.len(), 0, &mut out, &mut count);
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
        // A list item's paragraph attribute crosses too (LLP 1045 D4).
        let source = "- a";
        let handle = pieces(source.as_ptr(), source.len(), 0, &mut out, &mut count);
        let flat = unsafe { std::slice::from_raw_parts(out, count) };
        assert_eq!((flat[0].hang, flat[0].indent), (1, 40.0));
        assert_eq!((flat[1].hang, flat[1].indent), (0, 40.0));
        free(handle);
        assert_eq!(pieces(b"\xff".as_ptr(), 1, 0, &mut out, &mut count), 0);
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
        let handle = selection(source.as_ptr(), source.len(), 0, 3, 3, &mut out, &mut count);
        let json = std::str::from_utf8(unsafe { std::slice::from_raw_parts(out, count) }).unwrap();
        assert_eq!(
            json,
            r#"{"formats":"bold","mixed":false,"link":"","unavailable":"","atom":""}"#
        );
        free(handle);
        let handle = plain(
            source.as_ptr(),
            source.len(),
            0,
            0,
            u32::MAX,
            &mut out,
            &mut count,
        );
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
            0,
            u32::MAX,
            0,
            &mut out,
            &mut count,
        );
        assert!(handle != 0);
        let json = std::str::from_utf8(unsafe { std::slice::from_raw_parts(out, count) }).unwrap();
        assert_eq!(
            json,
            r#"{"p":[[0,3,1,1,0,0],[5,21,0,0,0,0]],"s":[[7,8,1,""],[12,13,16,"u"]],"h":[[0,2],[5,7],[8,10],[11,12],[13,21]],"r":[],"l":[[11,12,13,21]],"a":[]}"#
        );
        // The caret inside the bold reveals its markers as MARKER spans.
        let revealed = style(source.as_ptr(), source.len(), 0, 7, 7, &mut out, &mut count);
        let json = std::str::from_utf8(unsafe { std::slice::from_raw_parts(out, count) }).unwrap();
        assert!(
            json.contains(r#"[5,7,64,""]"#) && json.contains(r#"[8,10,64,""]"#),
            "{json}"
        );
        // A caret in the link's paragraph reveals its brackets, never its
        // destination.
        let linked = style(
            source.as_ptr(),
            source.len(),
            0,
            12,
            12,
            &mut out,
            &mut count,
        );
        let json = std::str::from_utf8(unsafe { std::slice::from_raw_parts(out, count) }).unwrap();
        assert!(json.ends_with(r#""l":[[11,12,13,21]],"a":[]}"#), "{json}");
        free(linked);
        free(handle);
        free(revealed);
    }

    /// LLP 1045.000 D2: a face's colour as the runner wrote it crosses in
    /// the presenters' wire form: a role as its platform colour, resolved
    /// with no session's overrides even while two sessions live on the
    /// thread (each would be a guess); a colour in a standard space the
    /// platforms ship, and one in a wide space, each in its own. A profile
    /// the app declares is none here, as the runner never writes one.
    #[test]
    fn a_faces_paint_crosses_in_the_wire_form() {
        use exact_kernel::style::roles;
        let (a, b) = (roles::Roles::new(Vec::new()), roles::Roles::new(Vec::new()));
        roles::install(&a);
        roles::install(&b);
        let role = face_paint("-exact-separator");
        assert!(role["sys"].is_string(), "{role}");
        let built_in = face_paint("color(--dci-p3 1 0 0)");
        assert_eq!(
            built_in["cs"][0]["s"], "cg:kCGColorSpaceDCIP3",
            "{built_in}"
        );
        let wide = face_paint("color(display-p3 1 0 0)");
        assert_eq!(wide["cs"][0]["s"], "display-p3", "{wide}");
        assert!(face_paint("color(--press 0.1 0.8 0.2)").is_null());
        assert!(face_paint("not a colour").is_null());
        drop((a, b));
    }

    /// LLP 1045.000: a node's atoms, parsed once, reach styling, pieces,
    /// selection facts and plain copy through one handle.
    #[test]
    fn atoms_cross_the_seam_through_one_handle() {
        let kinds = r#"[{"name":"citation","link":"t3-citation:"}]"#;
        let faces = r##"[{"source":"[q](t3-citation:1)","label":"Quote","icon":"i.png","title":"T","color":"light-dark(#d52c36, #ff6762)","backgroundColor":"no colour"}]"##;
        let atoms = atoms(kinds.as_ptr(), kinds.len(), faces.as_ptr(), faces.len());
        let source = "a [q](t3-citation:1) b";
        let (mut out, mut count) = (std::ptr::null(), 0usize);
        let read = |out: *const u8, count: usize| {
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(out, count) })
                .unwrap()
                .to_string()
        };
        let styled = style(
            source.as_ptr(),
            source.len(),
            atoms,
            u32::MAX,
            0,
            &mut out,
            &mut count,
        );
        let json = read(out, count);
        assert!(
            json.ends_with(
                r#""l":[],"a":[[2,20,"citation","Quote","i.png","T","button","t3-citation:1",[[213,44,54,255],[255,103,98,255]],null]]}"#
            ),
            "{json}"
        );
        assert!(json.contains(r#""h":[]"#), "{json}");
        free(styled);
        let facts = selection(
            source.as_ptr(),
            source.len(),
            atoms,
            2,
            20,
            &mut out,
            &mut count,
        );
        let json = read(out, count);
        assert!(
            json.contains(r#""link":"t3-citation:1""#)
                && json.ends_with(r#""unavailable":"link","atom":"[q](t3-citation:1)"}"#),
            "{json}"
        );
        free(facts);
        let copied = plain(
            source.as_ptr(),
            source.len(),
            atoms,
            0,
            u32::MAX,
            &mut out,
            &mut count,
        );
        assert_eq!(read(out, count), "a Quote b");
        free(copied);
        // A selection of exactly the atom copies its label.
        let copied = plain(
            source.as_ptr(),
            source.len(),
            atoms,
            2,
            20,
            &mut out,
            &mut count,
        );
        assert_eq!(read(out, count), "Quote");
        free(copied);
        let (mut flat, mut n) = (std::ptr::null(), 0usize);
        let expanded = pieces(source.as_ptr(), source.len(), atoms, &mut flat, &mut n);
        let flat = unsafe { std::slice::from_raw_parts(flat, n) };
        let atom = flat.iter().find(|p| !p.atom.is_null()).unwrap();
        assert_eq!(read(atom.text, atom.len), "Quote");
        let detail: serde_json::Value =
            serde_json::from_str(&read(atom.atom, atom.atom_len)).unwrap();
        assert_eq!(detail["source"], "[q](t3-citation:1)");
        assert_eq!(
            (detail["start"].as_u64(), detail["end"].as_u64()),
            (Some(2), Some(20))
        );
        // The face's colour in the wire form; one that is no colour, null.
        assert_eq!(
            detail["color"],
            serde_json::json!([[213, 44, 54, 255], [255, 103, 98, 255]])
        );
        assert!(detail["background"].is_null());
        free(expanded);
        free(atoms);
        // A freed handle reads as no atoms.
        let styled = style(
            source.as_ptr(),
            source.len(),
            atoms,
            u32::MAX,
            0,
            &mut out,
            &mut count,
        );
        assert!(read(out, count).ends_with(r#""a":[]}"#));
        free(styled);
    }
}
