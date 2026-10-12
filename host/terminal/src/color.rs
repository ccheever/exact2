//! Terminal colour depth and palette reduction (LLP 1101 D5).

use crate::grid::Rgb;
/// The output palette; `exactSurface("terminal").colors` reports its bit depth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ColorDepth {
    /// Terminal defaults only, retaining non-colour attributes.
    Monochrome = 0,
    /// The terminal's sixteen ANSI colours.
    Ansi16 = 4,
    /// The xterm 256-colour palette.
    Ansi256 = 8,
    /// RGB foreground and background colours.
    #[default]
    TrueColor = 24,
}

impl ColorDepth {
    /// Infer support from `NO_COLOR`, `TERM` and `COLORTERM`, without spawning
    /// a process or delaying first paint for a terminal response. Unknown
    /// terminals conservatively use monochrome; `--color` overrides detection.
    pub fn detect() -> Self {
        Self::from_env(
            std::env::var_os("NO_COLOR").is_some_and(|s| !s.is_empty()),
            &std::env::var("TERM").unwrap_or_default(),
            &std::env::var("COLORTERM").unwrap_or_default(),
        )
    }

    fn from_env(no_color: bool, term: &str, colorterm: &str) -> Self {
        if no_color || matches!(term, "dumb" | "unknown") {
            return Self::Monochrome;
        }
        if matches!(colorterm, "truecolor" | "24bit")
            || term.ends_with("-direct")
            || matches!(term, "xterm-kitty" | "wezterm" | "foot" | "foot-extra")
        {
            Self::TrueColor
        } else if term.contains("256color") {
            Self::Ansi256
        } else if !colorterm.is_empty()
            || ["xterm", "screen", "tmux", "rxvt", "linux", "ansi", "cygwin"]
                .iter()
                .any(|name| term == *name || term.starts_with(&format!("{name}-")))
        {
            Self::Ansi16
        } else {
            Self::Monochrome
        }
    }

    pub(crate) fn reduce(self, rgb: Rgb) -> Option<Rgb> {
        match self {
            Self::Monochrome => None,
            Self::TrueColor => Some(rgb),
            Self::Ansi16 => Some(ANSI[self.index(rgb) as usize]),
            Self::Ansi256 => Some(index_rgb(self.index(rgb))),
        }
    }

    fn index(self, rgb: Rgb) -> u8 {
        if self == Self::Ansi16 {
            return ANSI
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| distance(rgb, **c))
                .map_or(0, |(i, _)| i as u8);
        }
        let component = |v: u8| -> u8 {
            LEVELS
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| v.abs_diff(**c))
                .map_or(0, |(i, _)| i as u8)
        };
        let cube = 16 + 36 * component(rgb.0) + 6 * component(rgb.1) + component(rgb.2);
        let average = (rgb.0 as u16 + rgb.1 as u16 + rgb.2 as u16) / 3;
        let gray = 232 + ((average.saturating_sub(8) + 5) / 10).min(23) as u8;
        if distance(rgb, index_rgb(gray)) < distance(rgb, index_rgb(cube)) {
            gray
        } else {
            cube
        }
    }

    pub(crate) fn write(self, out: &mut String, rgb: Rgb, background: bool) {
        match self {
            Self::Monochrome => {}
            Self::TrueColor => {
                out.push_str(if background { "48;2;" } else { "38;2;" });
                let Rgb(r, g, b) = rgb;
                push_u8(out, r);
                out.push(';');
                push_u8(out, g);
                out.push(';');
                push_u8(out, b);
                out.push(';');
            }
            Self::Ansi256 => {
                out.push_str(if background { "48;5;" } else { "38;5;" });
                push_u8(out, self.index(rgb));
                out.push(';');
            }
            Self::Ansi16 => {
                let i = self.index(rgb);
                let code = if background { 40 } else { 30 } + i % 8 + if i >= 8 { 60 } else { 0 };
                push_u8(out, code);
                out.push(';');
            }
        }
    }
}

fn push_u8(out: &mut String, value: u8) {
    if value >= 100 {
        out.push(char::from(b'0' + value / 100));
    }
    if value >= 10 {
        out.push(char::from(b'0' + value / 10 % 10));
    }
    out.push(char::from(b'0' + value % 10));
}

const ANSI: [Rgb; 16] = [
    Rgb(0, 0, 0),
    Rgb(128, 0, 0),
    Rgb(0, 128, 0),
    Rgb(128, 128, 0),
    Rgb(0, 0, 128),
    Rgb(128, 0, 128),
    Rgb(0, 128, 128),
    Rgb(192, 192, 192),
    Rgb(128, 128, 128),
    Rgb(255, 0, 0),
    Rgb(0, 255, 0),
    Rgb(255, 255, 0),
    Rgb(0, 0, 255),
    Rgb(255, 0, 255),
    Rgb(0, 255, 255),
    Rgb(255, 255, 255),
];
const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

fn index_rgb(i: u8) -> Rgb {
    if i >= 232 {
        let v = 8 + 10 * (i - 232);
        Rgb(v, v, v)
    } else {
        let n = (i - 16) as usize;
        Rgb(LEVELS[n / 36], LEVELS[n / 6 % 6], LEVELS[n % 6])
    }
}

fn distance(a: Rgb, b: Rgb) -> u32 {
    [(a.0, b.0), (a.1, b.1), (a.2, b.2)]
        .into_iter()
        .map(|(x, y)| (x as i32 - y as i32).pow(2) as u32)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_u8_matches_standard_conversion() {
        for value in u8::MIN..=u8::MAX {
            let mut out = String::new();
            push_u8(&mut out, value);
            assert_eq!(out, value.to_string());
        }
    }

    #[test]
    fn environment_detection_is_conservative_and_no_color_wins() {
        for (term, hint, expected) in [
            ("xterm-256color", "truecolor", ColorDepth::TrueColor),
            ("xterm-256color", "", ColorDepth::Ansi256),
            ("screen", "", ColorDepth::Ansi16),
            ("xterm-kitty", "", ColorDepth::TrueColor),
            ("dumb", "truecolor", ColorDepth::Monochrome),
            ("vt100", "", ColorDepth::Monochrome),
            ("", "", ColorDepth::Monochrome),
            ("unknown-terminal", "", ColorDepth::Monochrome),
        ] {
            assert_eq!(ColorDepth::from_env(false, term, hint), expected);
            assert_eq!(
                ColorDepth::from_env(true, term, hint),
                ColorDepth::Monochrome
            );
        }
    }
}
