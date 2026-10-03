//! The Linux host's integration tests that pin the fixture font: one binary.
//! `tests/it/` holds the ones that leave the font environment alone.

mod backdrop;
mod borders;
mod gradients;
mod host;
mod materials;
mod motion_paint;
mod paint;
mod text;
mod tint;
mod visual;

/// The pinned font (LLP 1015 §3): the fixture directory and the family name
/// the driver sets, so every number in these tests is the same on a Mac and
/// on a builder. Set once, before the first engine is made; the environment
/// is process-wide and every test in this binary wants the same values.
fn pin_font() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::env::set_var(
            "EXACT_FONTS",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/fixtures/fonts/assets"
            ),
        );
        std::env::set_var("EXACT_FONT", "DejaVu Sans");
    });
}
