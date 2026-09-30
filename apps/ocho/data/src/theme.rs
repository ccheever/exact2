//! The theme: the GPUI desktop's `theme.rs` palettes (the bundled "Ocho
//! Dark" and "Ocho Light", themes/ocho.json) and the blends the views use,
//! as hex text the contract paints with.

use serde::Serialize;

/// One color, straight sRGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub f64);

impl Rgba {
    /// `#rrggbb` or `#rrggbbaa`.
    pub fn parse(hex: &str) -> Option<Rgba> {
        let h = hex.trim().trim_start_matches('#');
        let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        match h.len() {
            6 => Some(Rgba(v(0)?, v(2)?, v(4)?, 1.0)),
            8 => Some(Rgba(v(0)?, v(2)?, v(4)?, v(6)? as f64 / 255.0)),
            _ => None,
        }
    }

    /// The same color at `alpha`.
    pub fn alpha(self, alpha: f64) -> Rgba {
        Rgba(self.0, self.1, self.2, alpha)
    }

    /// `over` composited onto this opaque color (GPUI's `Hsla::blend`, done in RGB).
    pub fn blend(self, over: Rgba) -> Rgba {
        let a = over.3;
        let mix = |b: u8, o: u8| ((b as f64) * (1.0 - a) + (o as f64) * a).round() as u8;
        Rgba(
            mix(self.0, over.0),
            mix(self.1, over.1),
            mix(self.2, over.2),
            1.0,
        )
    }

    /// CSS text: `#rrggbb`, or `rgba(r, g, b, a)` when translucent.
    pub fn css(self) -> String {
        if (self.3 - 1.0).abs() < 1e-6 {
            format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
        } else {
            format!("rgba({}, {}, {}, {})", self.0, self.1, self.2, trim(self.3))
        }
    }
}

fn trim(a: f64) -> String {
    let s = format!("{a:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The theme's fields (theme.rs:15-42), plus its name and appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub dark: bool,
    pub status_bar: Rgba,
    pub bg: Rgba,
    pub surface: Rgba,
    pub elevated: Rgba,
    pub element: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub placeholder: Rgba,
    pub accent: Rgba,
    pub warn: Rgba,
    pub danger: Rgba,
    pub good: Rgba,
    pub selected: Rgba,
    pub hover: Rgba,
    pub term_bg: Rgba,
    pub term_fg: Rgba,
    pub cursor: Rgba,
    pub ansi: [Rgba; 16],
}

fn c(hex: &str) -> Rgba {
    Rgba::parse(hex).unwrap_or(Rgba(255, 0, 255, 1.0))
}

impl Theme {
    /// "Ocho Dark" (themes/ocho.json; theme.rs `fallback_dark`).
    pub fn ocho_dark() -> Theme {
        Theme {
            name: "Ocho Dark".into(),
            dark: true,
            status_bar: c("#282c34"),
            bg: c("#1e2127"),
            surface: c("#282c34"),
            elevated: c("#2f333d"),
            element: c("#2f333d"),
            border: c("#3b4048"),
            text: c("#c8ccd4"),
            muted: c("#8b919c"),
            placeholder: c("#6b717c"),
            accent: c("#5fd7d7"),
            warn: c("#ffaf5f"),
            danger: c("#e06c75"),
            good: c("#98c379"),
            selected: c("#353b47"),
            hover: c("#2c313a"),
            term_bg: c("#1b1e24"),
            term_fg: c("#d7dae0"),
            cursor: c("#5fd7d7"),
            ansi: [
                c("#282c34"),
                c("#e06c75"),
                c("#98c379"),
                c("#e5c07b"),
                c("#61afef"),
                c("#c678dd"),
                c("#56b6c2"),
                c("#abb2bf"),
                c("#5c6370"),
                c("#ef7a82"),
                c("#a9d685"),
                c("#f0cc8a"),
                c("#74bcff"),
                c("#d48eea"),
                c("#63c9d6"),
                c("#ffffff"),
            ],
        }
    }

    /// "Ocho Light".
    pub fn ocho_light() -> Theme {
        Theme {
            name: "Ocho Light".into(),
            dark: false,
            status_bar: c("#f0f0f1"),
            bg: c("#fafafa"),
            surface: c("#f0f0f1"),
            elevated: c("#ffffff"),
            element: c("#ffffff"),
            border: c("#d4d5d9"),
            text: c("#383a42"),
            muted: c("#7c7f87"),
            placeholder: c("#a0a1a7"),
            accent: c("#0184bc"),
            warn: c("#c18401"),
            danger: c("#e45649"),
            good: c("#50a14f"),
            selected: c("#dfe1e6"),
            hover: c("#e9eaee"),
            term_bg: c("#fafafa"),
            term_fg: c("#383a42"),
            cursor: c("#526fff"),
            ansi: [
                c("#000000"),
                c("#e45649"),
                c("#50a14f"),
                c("#c18401"),
                c("#4078f2"),
                c("#a626a4"),
                c("#0997b3"),
                c("#a0a1a7"),
                c("#5c6370"),
                c("#e45649"),
                c("#50a14f"),
                c("#c18401"),
                c("#4078f2"),
                c("#a626a4"),
                c("#0997b3"),
                c("#ffffff"),
            ],
        }
    }

    /// The bundled theme for an appearance (main.rs:223-229).
    pub fn bundled(dark: bool) -> Theme {
        if dark {
            Theme::ocho_dark()
        } else {
            Theme::ocho_light()
        }
    }

    /// The colors the contract paints with, every blend precomputed.
    pub fn view(&self) -> ThemeView {
        ThemeView {
            name: self.name.clone(),
            dark: self.dark,
            bg: self.bg.css(),
            surface: self.surface.css(),
            elevated: self.elevated.css(),
            element: self.element.css(),
            border: self.border.css(),
            text: self.text.css(),
            muted: self.muted.css(),
            placeholder: self.placeholder.css(),
            accent: self.accent.css(),
            warn: self.warn.css(),
            danger: self.danger.css(),
            good: self.good.css(),
            selected: self.selected.css(),
            hover: self.hover.css(),
            term_bg: self.term_bg.css(),
            term_fg: self.term_fg.css(),
            cursor: self.cursor.css(),
            status_bar: self.status_bar.css(),
            selected_bg: self.bg.blend(self.selected.alpha(0.55)).css(),
            hover_bg: self.bg.blend(self.hover.alpha(0.6)).css(),
            accent12: self.accent.alpha(0.12).css(),
            accent15: self.accent.alpha(0.15).css(),
            accent18: self.accent.alpha(0.18).css(),
            accent35: self.accent.alpha(0.35).css(),
            accent55: self.accent.alpha(0.55).css(),
            warn60: self.warn.alpha(0.6).css(),
            warn70: self.warn.alpha(0.7).css(),
            muted70: self.muted.alpha(0.7).css(),
            border72: self.border.alpha(0.72).css(),
            backdrop: "rgba(0, 0, 0, 0.45)".into(),
        }
    }

    /// The color of a manager badge for a session state (ui.rs:474-482).
    pub fn badge(&self, state: &str) -> Rgba {
        match state {
            "RUNNING" => self.good,
            "BLOCKED" | "LIMITED" => self.warn,
            "TERMINAL" => self.accent,
            _ => self.muted,
        }
    }
}

/// The theme as the contract's `Theme` shape.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeView {
    pub name: String,
    pub dark: bool,
    pub bg: String,
    pub surface: String,
    pub elevated: String,
    pub element: String,
    pub border: String,
    pub text: String,
    pub muted: String,
    pub placeholder: String,
    pub accent: String,
    pub warn: String,
    pub danger: String,
    pub good: String,
    pub selected: String,
    pub hover: String,
    pub term_bg: String,
    pub term_fg: String,
    pub cursor: String,
    pub status_bar: String,
    pub selected_bg: String,
    pub hover_bg: String,
    pub accent12: String,
    pub accent15: String,
    pub accent18: String,
    pub accent35: String,
    pub accent55: String,
    pub warn60: String,
    pub warn70: String,
    pub muted70: String,
    pub border72: String,
    pub backdrop: String,
}

/// Usage-bar colors by provider (ui.rs:484-491).
pub fn provider_usage_color(provider: &str, theme: &Theme) -> String {
    match provider {
        "claude" => "#D97757".into(),
        "codex" => "#7B85FE".into(),
        "opencode" => "#5C9CF5".into(),
        _ => theme.accent.css(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blends_match_the_inventory() {
        let d = Theme::ocho_dark().view();
        assert_eq!(d.selected_bg, "#2b2f39");
        assert_eq!(d.hover_bg, "#262b32");
        let l = Theme::ocho_light().view();
        assert_eq!(l.selected_bg, "#ebecef");
        assert_eq!(l.hover_bg, "#f0f0f3");
        assert_eq!(Rgba::parse("#5fd7d73d").unwrap().3, 61.0 / 255.0);
    }
}
