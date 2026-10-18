//! The device's colours, from its shell (LLP 1115.001 D4): on Android the
//! reader resolves Material 3's colour roles for the device, light and
//! dark, from the system's dynamic palette, and tells them whole at start
//! and whenever they change. Here each of Exact's colour roles is given the
//! Material role D4's table names for it, and the set is reported through
//! the door the Apple host reports its own through
//! (`roles::set_reported`), so that a role an author wrote, and one the
//! platform's defaults name, resolve to the device's colour.
//!
//! The names an author writes do not change, and nothing is tabled: the
//! values are the device's, computed there. A host whose shell tells no
//! palette (desktop Linux, a headless run) reports nothing and keeps each
//! role's fallback pair.
use super::*;
use exact_kernel::style::{roles, Color};
use exact_kernel::ColorValue;

/// Material 3's colour roles, every role of Compose's `ColorScheme`, in the
/// order a shell tells them (`Native.colors`: all of them light, then all of
/// them dark). The first twelve are those D4 maps Exact's own roles to; the
/// rest the Material look reads by Material's names ([`material`]; LLP
/// 1115.001.001 D2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    /// `primary`.
    Primary,
    /// `onPrimary`.
    OnPrimary,
    /// `secondaryContainer`.
    SecondaryContainer,
    /// `surface`.
    Surface,
    /// `onSurface`.
    OnSurface,
    /// `onSurfaceVariant`.
    OnSurfaceVariant,
    /// `surfaceContainerLow`.
    SurfaceContainerLow,
    /// `surfaceContainer`.
    SurfaceContainer,
    /// `surfaceContainerHigh`.
    SurfaceContainerHigh,
    /// `surfaceContainerHighest`.
    SurfaceContainerHighest,
    /// `outlineVariant`.
    OutlineVariant,
    /// `error`.
    Error,
    /// `primaryContainer`.
    PrimaryContainer,
    /// `onPrimaryContainer`.
    OnPrimaryContainer,
    /// `inversePrimary`.
    InversePrimary,
    /// `secondary`.
    Secondary,
    /// `onSecondary`.
    OnSecondary,
    /// `onSecondaryContainer`.
    OnSecondaryContainer,
    /// `tertiary`.
    Tertiary,
    /// `onTertiary`.
    OnTertiary,
    /// `tertiaryContainer`.
    TertiaryContainer,
    /// `onTertiaryContainer`.
    OnTertiaryContainer,
    /// `background`.
    Background,
    /// `onBackground`.
    OnBackground,
    /// `surfaceVariant`.
    SurfaceVariant,
    /// `surfaceTint`.
    SurfaceTint,
    /// `inverseSurface`.
    InverseSurface,
    /// `inverseOnSurface`.
    InverseOnSurface,
    /// `onError`.
    OnError,
    /// `errorContainer`.
    ErrorContainer,
    /// `onErrorContainer`.
    OnErrorContainer,
    /// `outline`.
    Outline,
    /// `scrim`.
    Scrim,
    /// `surfaceBright`.
    SurfaceBright,
    /// `surfaceDim`.
    SurfaceDim,
    /// `surfaceContainerLowest`.
    SurfaceContainerLowest,
}

/// How many roles a palette holds for each scheme.
pub const MATERIAL_ROLES: usize = 36;

/// The device's palette: each [`Material`] role's colour under the light
/// scheme and under the dark one, as Android has a colour (`0xAARRGGBB`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// The light scheme's, indexed by [`Material`].
    pub light: [u32; MATERIAL_ROLES],
    /// The dark scheme's.
    pub dark: [u32; MATERIAL_ROLES],
}

use Material::*;

/// Exact's roles as Material's (LLP 1115.001 D4's table): the name an
/// author writes, the Material role, and its opacity in percent.
///
/// Four rows are not in D4's table and were chosen here: `FieldText` and
/// `HighlightText` are text on a surface (`onSurface`),
/// `-exact-opaque-separator` is the separator (`outlineVariant`, which is
/// opaque already), and `-exact-tertiary-grouped-background` is the step
/// above the secondary one (`surfaceContainerHigh`). The system hues other
/// than red and blue are left out, as D4 rules: they keep the table's pair.
const ROLES: &[(&str, Material, u32)] = &[
    ("Canvas", Surface, 100),
    ("-exact-background", Surface, 100),
    ("CanvasText", OnSurface, 100),
    ("-exact-label", OnSurface, 100),
    ("FieldText", OnSurface, 100),
    ("HighlightText", OnSurface, 100),
    ("-exact-secondary-label", OnSurfaceVariant, 100),
    ("-exact-placeholder", OnSurfaceVariant, 100),
    // Material's disabled content.
    ("-exact-tertiary-label", OnSurface, 38),
    ("GrayText", OnSurface, 38),
    ("-exact-quaternary-label", OnSurface, 12),
    ("-exact-separator", OutlineVariant, 100),
    ("-exact-opaque-separator", OutlineVariant, 100),
    ("AccentColor", Primary, 100),
    ("LinkText", Primary, 100),
    ("-exact-link", Primary, 100),
    ("ButtonText", Primary, 100),
    ("-exact-system-blue", Primary, 100),
    // A symbol nobody tinted (LLP 1115.000 D3.9): `primary`, as it was
    // while its default was `AccentColor`. Material's own answer for an
    // icon is LLP 1115.001's to give.
    ("-exact-symbol", Primary, 100),
    ("AccentColorText", OnPrimary, 100),
    ("Field", SurfaceContainerHighest, 100),
    ("-exact-fill", SurfaceContainerHighest, 100),
    ("-exact-secondary-fill", SurfaceContainerHighest, 100),
    ("-exact-tertiary-fill", SurfaceContainerHighest, 100),
    ("-exact-quaternary-fill", SurfaceContainerHighest, 100),
    ("ButtonFace", SecondaryContainer, 100),
    ("-exact-secondary-background", SurfaceContainerLow, 100),
    ("-exact-grouped-background", SurfaceContainerLow, 100),
    ("-exact-secondary-grouped-background", SurfaceContainer, 100),
    ("-exact-tertiary-background", SurfaceContainer, 100),
    (
        "-exact-tertiary-grouped-background",
        SurfaceContainerHigh,
        100,
    ),
    ("Highlight", Primary, 40),
    ("-exact-system-red", Error, 100),
];

impl Palette {
    /// A palette from the shell's colours, every role light and then
    /// every role dark; `None` for any other count.
    pub fn from_words(words: &[i32]) -> Option<Palette> {
        let (light, dark) = words.split_at_checked(MATERIAL_ROLES)?;
        let colors = |half: &[i32]| -> Option<[u32; MATERIAL_ROLES]> {
            let half: [i32; MATERIAL_ROLES] = half.try_into().ok()?;
            Some(half.map(|argb| argb as u32))
        };
        Some(Palette {
            light: colors(light)?,
            dark: colors(dark)?,
        })
    }

    /// What `role` is on this device under a scheme, at an opacity in
    /// percent: the device's colour is opaque, and the opacity is the role's.
    fn color(&self, role: Material, dark: bool, percent: u32) -> Color {
        let table = if dark { &self.dark } else { &self.light };
        let [_, r, g, b] = table[role as usize].to_be_bytes();
        let alpha = ((255 * percent.min(100) + 50) / 100) as u8;
        Color(u32::from_be_bytes([r, g, b, alpha]))
    }

    /// Each of Exact's roles on this device, light and dark: what
    /// `roles::set_reported` takes.
    pub fn report(&self) -> Vec<(ColorValue, bool, Color)> {
        let mut out = Vec::with_capacity(ROLES.len() * 2);
        for (name, material, percent) in ROLES {
            let Some(id) = roles::role(name) else {
                debug_assert!(false, "{name} is no colour role");
                continue;
            };
            for dark in [false, true] {
                let color = self.color(*material, dark, *percent);
                out.push((ColorValue::Role(id), dark, color));
            }
        }
        out
    }

    /// Report this palette for the process (`roles::set_reported`), before
    /// a host boots, so everything boot resolves is the device's from the
    /// start. A running presenter is told through
    /// [`Presenter::set_palette`], which also paints again.
    pub fn tell(&self) -> u64 {
        *TOLD.lock().unwrap_or_else(|e| e.into_inner()) = Some(*self);
        roles::set_reported(self.report())
    }
}

/// The palette this process was last told.
static TOLD: std::sync::Mutex<Option<Palette>> = std::sync::Mutex::new(None);

/// A Material role as the shell told it, under a scheme: what the Material
/// look reads where Exact has no role of its own for it (`outline`,
/// `onSecondaryContainer`). `None` where no palette was told.
pub fn material(role: Material, dark: bool) -> Option<[u8; 4]> {
    let told = (*TOLD.lock().unwrap_or_else(|e| e.into_inner()))?;
    Some(told.rgba(role, dark))
}

impl Palette {
    /// `role` under a scheme, as RGBA.
    fn rgba(&self, role: Material, dark: bool) -> [u8; 4] {
        let table = if dark { self.dark } else { self.light };
        let [a, r, g, b] = table[role as usize].to_be_bytes();
        [r, g, b, a]
    }
}

/// The accent a control paints in where its node sets no `accent-color`
/// (LLP 1115.001 D4, "one accent"), and what is drawn on it: the
/// `AccentColor` and `AccentColorText` roles as the host reported them.
/// `None` on a host that reports nothing, whose painter keeps its own.
pub(crate) fn reported_accent(dark: bool) -> Option<([u8; 4], [u8; 4])> {
    static TINT: std::sync::LazyLock<Option<(u8, u8)>> = std::sync::LazyLock::new(|| {
        Some((roles::role("AccentColor")?, roles::role("AccentColorText")?))
    });
    let (accent, on) = (*TINT)?;
    let accent = roles::reported(ColorValue::Role(accent), dark)?;
    let on = ColorValue::Role(on).resolve(dark);
    Some((crate::paint::rgba(accent), crate::paint::rgba(on)))
}

/// What a presenter keeps of the reported colours.
#[derive(Default)]
pub(super) struct State {
    /// The reported table's generation this presenter last painted from.
    seen: Option<u64>,
}

impl<D: DataSource> Presenter<D> {
    /// The device's palette, as its shell resolved it (LLP 1115.001 D4):
    /// told before the first frame and again, whole, whenever it changes (a
    /// new wallpaper changes the dynamic palette). Every role resolves to
    /// it from here on, and when anything resolves differently the page is
    /// painted again: paint motion re-targets (the first report corrects
    /// in place, a later one transitions, LLP 1095 D6) and nothing kept
    /// from a paint under the old colours stands.
    pub fn set_palette(&mut self, palette: &Palette) {
        let generation = palette.tell();
        if self.colors.seen == Some(generation) {
            return;
        }
        let first = self.colors.seen.is_none();
        self.colors.seen = Some(generation);
        self.host.colors_changed(first);
        self.brush.colors_changed();
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_palette_is_every_material_role_light_then_dark_and_reads_by_its_name() {
        assert_eq!(
            Palette::from_words(&[0; 24]),
            None,
            "twelve roles a scheme is the old count"
        );
        let mut words = vec![0xFF00_0000u32 as i32; MATERIAL_ROLES * 2];
        words[Material::Outline as usize] = 0xFF79_747Eu32 as i32;
        words[MATERIAL_ROLES + Material::Outline as usize] = 0xFF93_8F99u32 as i32;
        // Not told: telling a palette changes every role in the process.
        let palette = Palette::from_words(&words).expect("every role, light and dark");
        assert_eq!(
            palette.rgba(Material::Outline, false),
            [0x79, 0x74, 0x7E, 0xFF]
        );
        assert_eq!(
            palette.rgba(Material::Outline, true),
            [0x93, 0x8F, 0x99, 0xFF]
        );
    }
}
