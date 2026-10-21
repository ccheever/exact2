//! The colours the page draws Exact's colour roles in (Apple's palette, host/web/look.mjs), reported to the kernel
//! as a platform host reports its own (`roles::set_reported`): what the kernel resolves itself, such as a canvas's
//! ink, is then what the page draws.

use exact_kernel::style::roles;
use exact_kernel::{Color, ColorValue};

/// The page's resolution of each role it holds, `[[name, dark, css colour], …]`, the names look.mjs gives
/// (`--exact-apple-roles`); an entry naming no role or no colour is left out.
pub fn report(json: &str) {
    let Ok(entries) = serde_json::from_str::<Vec<(String, bool, String)>>(json) else {
        return;
    };
    roles::set_reported(entries.into_iter().filter_map(|(name, dark, css)| {
        let id = roles::role(&format!("-exact-{name}"))?;
        Some((ColorValue::Role(id), dark, Color::parse(&css)?))
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A role's report is what the kernel then resolves it to, in its scheme, under the theme's replacement of it
    /// (LLP 1095 D4), which still wins. The reported table is process-wide, so the test reports a role no other test
    /// of this crate names, and withdraws it; the theme is the thread's own.
    #[test]
    fn a_reported_role_is_what_the_kernel_resolves_under_the_theme() {
        report(
            r#"[["system-mint", false, "rgb(0, 200, 179)"], ["system-mint", true, "rgba(0, 218, 195, 0.5)"], ["nope", false, "red"]]"#,
        );
        let id = roles::role("-exact-system-mint").unwrap();
        let mint = ColorValue::Role(id);
        assert_eq!(mint.resolve(false), Color::rgba(0, 200, 179, 255));
        assert_eq!(mint.resolve(true), Color::rgba(0, 218, 195, 128));
        roles::set_overrides(vec![(id, ColorValue::Fixed(Color::rgba(255, 0, 0, 255)))]);
        assert_eq!(mint.resolve(true), Color::rgba(255, 0, 0, 255));
        roles::set_overrides(Vec::new());
        roles::set_reported([]);
        assert_ne!(
            mint.resolve(true),
            Color::rgba(0, 218, 195, 128),
            "a withdrawn report"
        );
    }
}
