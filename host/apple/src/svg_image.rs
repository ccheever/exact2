//! An SVG file as an image on Apple (LLP 1011.001 D4): the file parsed and
//! resolved in Rust (`exact-svg-file`, the kernel's own SVG nodes and scene
//! resolver), serialized as the `svg` op's JSON, which Swift renders as it
//! renders an island (`SvgIsland.render`) into the raster loader's bitmap.
//! The header read gives the natural size the loader's metadata carries.

/// What the loader draws from: the scene's JSON and whether it has text,
/// which Swift draws where the session's fonts are (D3).
#[derive(Debug)]
pub struct Picture {
    /// The scene, as the `svg` op carries one.
    pub json: String,
    /// Whether the picture has text.
    pub text: bool,
    /// What the parser dropped or did not read, each named.
    pub warnings: Vec<String>,
}

/// The scene of `text` at a concrete object size of `width` × `height`
/// CSS pixels; `Err` with the parse's reason and line.
pub fn scene(text: &str, width: f32, height: f32) -> Result<Picture, String> {
    let loaded = exact_svg_file::load(text).map_err(|e| e.to_string())?;
    let node = loaded
        .kernel()
        .node(loaded.svg())
        .ok_or("the svg node is not in the kernel")?;
    let json = crate::host::svg::image_scene(loaded.kernel(), &node, (0.0, 0.0, width, height))?;
    Ok(Picture {
        json,
        text: loaded.document.text,
        warnings: loaded.document.warnings.clone(),
    })
}

/// The natural size (CSS pixels, LLP 1011.001 D1) of the SVG `bytes` begin;
/// `None` when they are not an SVG or the root tag is past the prefix.
pub fn header(bytes: &[u8]) -> Option<(f32, f32)> {
    exact_svg_file::natural(bytes).map(|n| n.size())
}

/// A buffer the caller frees with `free`.
#[allow(unsafe_code)]
fn leak(bytes: Vec<u8>, out_len: *mut usize) -> *mut u8 {
    let boxed = bytes.into_boxed_slice();
    // SAFETY: the caller gave a place for the length.
    unsafe { *out_len = boxed.len() };
    Box::into_raw(boxed).cast::<u8>()
}

/// Release a buffer `leak` gave out.
/// # Safety
/// `ptr` and `len` are one buffer from `exact_svg_image_scene`, freed once.
#[allow(unsafe_code)]
pub unsafe fn free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        // SAFETY: the caller returns the buffer leak made, once.
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
    }
}

/// Export the SVG image seam, instantiated in the same app archive as `host!`.
#[macro_export]
macro_rules! svg_image_exports {
    () => {
        /// Whether `bytes` begin an SVG document: 1 and its natural size
        /// (CSS pixels) in `size[0..2]`, else 0.
        /// # Safety
        /// `bytes` holds `len` bytes; `size` has room for two doubles.
        #[no_mangle]
        pub unsafe extern "C" fn exact_svg_image_header(
            bytes: *const u8,
            len: usize,
            size: *mut f64,
        ) -> u32 {
            if bytes.is_null() || size.is_null() {
                return 0;
            }
            // SAFETY: the caller guarantees bytes holds len bytes and size two doubles.
            let slice = unsafe { ::std::slice::from_raw_parts(bytes, len) };
            match $crate::svg_image::header(slice) {
                Some((w, h)) => {
                    unsafe {
                        *size = f64::from(w);
                        *size.add(1) = f64::from(h);
                    }
                    1
                }
                None => 0,
            }
        }
        /// The scene of an SVG file at `width` × `height` CSS pixels, as the
        /// `svg` op's JSON, in a buffer of `*out_len` bytes to free with
        /// `exact_svg_image_free`; `*flags` bit 0 says the picture has text.
        /// Null with the reason in the buffer when the file is refused
        /// (`*flags` bit 1).
        /// # Safety
        /// `bytes` holds `len` bytes; `out_len` and `flags` are writable.
        #[no_mangle]
        pub unsafe extern "C" fn exact_svg_image_scene(
            bytes: *const u8,
            len: usize,
            width: f64,
            height: f64,
            out_len: *mut usize,
            flags: *mut u32,
        ) -> *mut u8 {
            if bytes.is_null() || out_len.is_null() || flags.is_null() {
                return ::std::ptr::null_mut();
            }
            // SAFETY: the caller guarantees bytes holds len bytes.
            let slice = unsafe { ::std::slice::from_raw_parts(bytes, len) };
            let text = match ::std::str::from_utf8(slice) {
                Ok(t) => t,
                Err(_) => {
                    unsafe { *flags = 2 };
                    return $crate::svg_image::leak_message(
                        "the file is not UTF-8".into(),
                        out_len,
                    );
                }
            };
            match $crate::svg_image::scene(text, width as f32, height as f32) {
                Ok(picture) => {
                    unsafe { *flags = u32::from(picture.text) };
                    $crate::svg_image::leak_message(picture.json, out_len)
                }
                Err(why) => {
                    unsafe { *flags = 2 };
                    $crate::svg_image::leak_message(why, out_len)
                }
            }
        }
        /// Free a buffer `exact_svg_image_scene` gave out.
        /// # Safety
        /// `ptr` and `len` are one such buffer, freed once.
        #[no_mangle]
        pub unsafe extern "C" fn exact_svg_image_free(ptr: *mut u8, len: usize) {
            unsafe { $crate::svg_image::free(ptr, len) }
        }
    };
}

/// A message or scene as a buffer the caller frees.
pub fn leak_message(text: String, out_len: *mut usize) -> *mut u8 {
    leak(text.into_bytes(), out_len)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_file_becomes_the_svg_ops_scene() {
        let picture = super::scene("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 2 1\"><rect width=\"1\" height=\"1\" fill=\"#f00\"/><text x=\"1\" y=\"1\">a</text></svg>", 200.0, 100.0).unwrap();
        assert!(picture.text);
        assert!(
            picture
                .json
                .starts_with("{\"box\":[0,0,200,100],\"t\":[100,0,0,100,0,0]"),
            "{}",
            picture.json
        );
        assert!(picture.json.contains("\"els\":["), "{}", picture.json);
        assert_eq!(
            super::header(b"<svg width=\"3\" height=\"4\"/>"),
            Some((3.0, 4.0))
        );
        assert_eq!(super::header(b"\x89PNG"), None);
        assert!(super::scene("<svg>", 1.0, 1.0)
            .unwrap_err()
            .contains("never closed"));
    }

    #[test]
    fn a_file_past_a_cost_limit_is_the_named_error() {
        // Each level uses the one below twice (LLP 1011.001 D2).
        let mut defs = String::from("<path id=\"l0\" d=\"M0 0");
        defs += &"l1 1l1 -1".repeat(15);
        defs += "\"/>";
        for k in 1..=17 {
            defs += &format!(
                "<g id=\"l{k}\"><use href=\"#l{0}\"/><use href=\"#l{0}\"/></g>",
                k - 1
            );
        }
        let file = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 9 9\"><defs>{defs}</defs><use href=\"#l17\"/></svg>");
        let why = super::scene(&file, 64.0, 64.0).unwrap_err();
        assert!(why.contains("expands past"), "{why}");
    }
}
