//! Windows contrast themes use the user's system color pairs, in either scheme.
//! https://learn.microsoft.com/windows/apps/design/accessibility/high-contrast-themes
use super::*;
use exact_kernel::style::roles;
use exact_kernel::{Color, ColorValue};

/// The eight system colors of a Windows contrast theme, as opaque RGBA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContrastPalette {
    /// Page and popup background (`COLOR_WINDOW`).
    pub window: [u8; 4],
    /// Text and borders on that background (`COLOR_WINDOWTEXT`).
    pub text: [u8; 4],
    /// Hyperlinks (`COLOR_HOTLIGHT`).
    pub link: [u8; 4],
    /// Disabled content (`COLOR_GRAYTEXT`).
    pub disabled: [u8; 4],
    /// Selected background (`COLOR_HIGHLIGHT`).
    pub highlight: [u8; 4],
    /// Text on the selected background (`COLOR_HIGHLIGHTTEXT`).
    pub highlighted_text: [u8; 4],
    /// Button background (`COLOR_BTNFACE`).
    pub button: [u8; 4],
    /// Button text (`COLOR_BTNTEXT`).
    pub button_text: [u8; 4],
}

impl ContrastPalette {
    fn report(self) -> Vec<(ColorValue, bool, Color)> {
        let mut out = Vec::new();
        for (names, rgba) in [
            ("Canvas Field -exact-background -exact-secondary-background -exact-tertiary-background -exact-grouped-background -exact-secondary-grouped-background -exact-tertiary-grouped-background -exact-fill -exact-secondary-fill -exact-tertiary-fill -exact-quaternary-fill", self.window),
            ("CanvasText FieldText -exact-label -exact-secondary-label -exact-tertiary-label -exact-quaternary-label -exact-placeholder -exact-separator -exact-opaque-separator -exact-system-red -exact-system-orange -exact-system-yellow -exact-system-green -exact-system-mint -exact-system-teal -exact-system-cyan -exact-system-blue -exact-system-indigo -exact-system-purple -exact-system-pink -exact-system-brown -exact-system-gray", self.text),
            ("LinkText -exact-link", self.link),
            ("GrayText", self.disabled),
            ("Highlight AccentColor", self.highlight),
            ("HighlightText AccentColorText", self.highlighted_text),
            ("ButtonFace", self.button),
            ("ButtonText -exact-symbol", self.button_text),
        ] {
            for name in names.split_whitespace() {
                let role = ColorValue::Role(roles::role(name).expect("system color role"));
                for dark in [false, true] {
                    out.push((role, dark, Color(u32::from_be_bytes(rgba))));
                }
            }
        }
        out
    }
}

#[derive(Default)]
pub(super) struct State {
    pub palette: Option<ContrastPalette>,
    restore: Vec<(ColorValue, bool, Color)>,
}

// Like the role table and Material palette, the platform look is process-wide.
static TOLD: std::sync::Mutex<Option<ContrastPalette>> = std::sync::Mutex::new(None);

pub(crate) fn current() -> Option<ContrastPalette> {
    *TOLD.lock().unwrap_or_else(|e| e.into_inner())
}

impl<D: DataSource> Presenter<D> {
    /// Apply or withdraw the Windows contrast palette. Literal authored colors
    /// keep their values; semantic roles follow the system palette immediately.
    pub fn set_contrast_palette(&mut self, palette: Option<ContrastPalette>) {
        if self.colors.contrast.palette == palette {
            return;
        }
        let report = if let Some(palette) = palette {
            let report = palette.report();
            if self.colors.contrast.palette.is_none() {
                self.colors.contrast.restore = report
                    .iter()
                    .map(|&(role, dark, _)| {
                        let ColorValue::Role(id) = role else {
                            unreachable!()
                        };
                        (
                            role,
                            dark,
                            roles::reported(role, dark)
                                .unwrap_or_else(|| roles::role_fallback(id).resolve(dark)),
                        )
                    })
                    .collect();
            }
            report
        } else {
            // Explicitly restore tint roles too: set_reported retains an omitted tint.
            std::mem::take(&mut self.colors.contrast.restore)
        };
        self.colors.contrast.palette = palette;
        *TOLD.lock().unwrap_or_else(|e| e.into_inner()) = palette;
        roles::set_reported(report);
        self.host.colors_changed(true);
        self.brush.colors_changed();
        self.sync_surface();
        self.dirty = true;
    }
}
