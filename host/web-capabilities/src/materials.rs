//! `backgroundMaterial` on the web (LLP 1053.000 D4): the schema's stated
//! approximation of the named material, as the CSS variables the page's one
//! `[backgroundMaterial]` rule reads. Linked when a plan names a material,
//! so an app that names none carries neither the table nor this.

use exact_web::Linked;

/// Link the materials into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.materials = Some((css, note));
    linked
}

/// Append material `name`'s variables to `css`. A name the table lacks
/// (only a computed one reaches here) draws `ultra-thin`'s.
pub fn css(css: &mut String, name: &str) {
    use std::fmt::Write as _;
    let m = exact_kernel::generated::material(name)
        .or_else(|| exact_kernel::generated::material("ultra-thin"))
        .expect("the schema declares ultra-thin");
    let hex = |c: [u8; 4]| format!("#{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3]);
    let _ = write!(
        css,
        "--exact-material-blur:{}px;--exact-material-saturate:{}%;--exact-material-light:{};--exact-material-dark:{};",
        m.blur,
        m.saturate,
        hex(m.light),
        hex(m.dark)
    );
}

std::thread_local! {
    static NOTED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The line to log for a name the table lacks, the first time it is seen.
pub fn note(name: &str) -> Option<String> {
    if exact_kernel::generated::material(name).is_some() {
        return None;
    }
    NOTED.with_borrow_mut(|noted| {
        (!noted.iter().any(|n| n == name)).then(|| {
            noted.push(name.into());
            format!("backgroundMaterial `{name}` is not a material; drawing ultra-thin")
        })
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_material_is_its_tables_variables_and_an_unknown_one_ultra_thins() {
        let css = |name| {
            let mut css = String::new();
            super::css(&mut css, name);
            css
        };
        assert_eq!(
            css("sidebar"),
            "--exact-material-blur:30px;--exact-material-saturate:180%;--exact-material-light:#eeeeeeb3;--exact-material-dark:#1e1e1eb3;"
        );
        assert_eq!(css("frosted"), css("ultra-thin"));
        assert_eq!(super::note("sidebar"), None);
        assert!(super::note("frosted").is_some());
        assert_eq!(super::note("frosted"), None, "once");
    }
}
