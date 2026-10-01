//! Zed theme files and the theme picker (theme.rs `parse_zed_theme`,
//! `parse_zed_file`, `strip_jsonc`, `zed_theme_name`, `initial_theme`,
//! `theme_dirs`, `discover`; workspace.rs `Overlay::Themes`, `theme_matches`,
//! `preview_theme`, `commit_theme`, `cancel_theme`, `open_themes`; ui.rs
//! `render_themes`). No file system here: the host lists the theme
//! directories and hands their files to `discover`.
//!
//! The picker is an `OVERLAY` picker (kind "picker", width 560, glyph "›")
//! whose `PICKER_ROW`s carry `swatch` (the theme's `bg`) and `swatchBorder`
//! (its `border`) for the 12 px square, `label` the name and `detail`
//! "dark|light · file|bundled". Moving previews the theme; Enter keeps it;
//! Esc reverts to the theme the picker opened on.

use crate::picker::{self, fuzzy_score, Mods, PickerKey, PickerState};
use crate::theme::{Rgba, Theme};
use serde_json::{json, Map, Value as Json};

/// The card width.
pub const WIDTH: f64 = 560.0;
/// The placeholder.
pub const PLACEHOLDER: &str = "Type a theme name…";
/// With a query nothing matches.
pub const NO_MATCH: &str = "No matching themes";
/// The footer.
pub const FOOTER: &str = "↑ ↓ preview · Enter keeps · Esc reverts · Zed theme files from ~/.config/zed/themes, Zed extensions, and $FLEET_HOME/themes";
/// Under the home directory: Zed's installed extensions, each with a
/// `themes` folder the host lists (`installed/*/themes`).
pub const ZED_EXTENSIONS: &str = "Library/Application Support/Zed/extensions/installed";

/// A theme and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeEntry {
    /// The theme.
    pub theme: Theme,
    /// The file it was read from; `None` for a bundled theme.
    pub source: Option<String>,
}

impl ThemeEntry {
    /// The file's name, or "bundled".
    pub fn source_label(&self) -> String {
        match &self.source {
            Some(path) => path.rsplit('/').next().unwrap_or(path).to_string(),
            None => "bundled".to_string(),
        }
    }

    /// "dark · ocho.json".
    pub fn detail(&self) -> String {
        format!(
            "{} · {}",
            if self.theme.dark { "dark" } else { "light" },
            self.source_label()
        )
    }
}

fn color(style: &Map<String, Json>, key: &str) -> Option<Rgba> {
    style.get(key).and_then(Json::as_str).and_then(Rgba::parse)
}

/// Build a theme from one entry of a Zed theme family. Missing keys fall
/// back along the same chain Zed's UI uses, so partial themes still look
/// coherent (theme.rs `parse_zed_theme`).
pub fn parse_zed_theme(name: &str, dark: bool, style: &Map<String, Json>) -> Theme {
    let base = Theme::bundled(dark);
    let get =
        |keys: &[&str], default: Rgba| keys.iter().find_map(|k| color(style, k)).unwrap_or(default);
    let background = get(&["background"], base.surface);
    let surface = get(
        &["panel.background", "surface.background", "background"],
        base.surface,
    );
    let elevated = get(
        &[
            "elevated_surface.background",
            "surface.background",
            "background",
        ],
        base.elevated,
    );
    let text = get(&["text"], base.text);
    let term_fg = get(&["terminal.foreground", "text"], text);
    let cursor = style
        .get("players")
        .and_then(Json::as_array)
        .and_then(|p| p.first())
        .and_then(Json::as_object)
        .and_then(|p| color(p, "cursor"))
        .unwrap_or_else(|| get(&["text.accent"], base.accent));
    let ansi_names = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];
    let mut ansi = base.ansi;
    for (i, n) in ansi_names.iter().enumerate() {
        if let Some(c) = color(style, &format!("terminal.ansi.{n}")) {
            ansi[i] = c;
        }
        if let Some(c) = color(style, &format!("terminal.ansi.bright_{n}")) {
            ansi[i + 8] = c;
        }
    }
    Theme {
        name: name.to_string(),
        dark,
        status_bar: get(&["status_bar.background", "background"], background),
        bg: get(&["editor.background", "background"], base.bg),
        surface,
        elevated,
        element: get(
            &["element.background", "elevated_surface.background"],
            elevated,
        ),
        border: get(&["border", "border.variant"], base.border),
        text,
        muted: get(&["text.muted", "text.placeholder"], base.muted),
        placeholder: get(&["text.placeholder", "text.muted"], base.placeholder),
        accent: get(&["text.accent", "icon.accent"], base.accent),
        warn: get(&["warning"], base.warn),
        danger: get(&["error"], base.danger),
        good: get(&["success", "created"], base.good),
        selected: get(&["element.selected", "element.active"], base.selected),
        hover: get(&["element.hover", "ghost_element.hover"], base.hover),
        term_bg: get(
            &["terminal.background", "editor.background", "background"],
            base.term_bg,
        ),
        term_fg,
        cursor,
        ansi,
    }
}

/// Every theme in a Zed theme family file (JSONC allowed); nothing for a
/// file that is not one.
pub fn parse_zed_file(text: &str, source: Option<&str>) -> Vec<ThemeEntry> {
    let Ok(value) = serde_json::from_str::<Json>(&strip_jsonc(text)) else {
        return Vec::new();
    };
    let Some(themes) = value.get("themes").and_then(Json::as_array) else {
        return Vec::new();
    };
    themes
        .iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?;
            let dark = !matches!(
                entry.get("appearance").and_then(Json::as_str),
                Some("light")
            );
            let style = entry.get("style")?.as_object()?;
            Some(ThemeEntry {
                theme: parse_zed_theme(name, dark, style),
                source: source.map(str::to_string),
            })
        })
        .collect()
}

/// The bundled themes: "Ocho Dark", then "Ocho Light".
pub fn bundled() -> Vec<ThemeEntry> {
    vec![
        ThemeEntry {
            theme: Theme::ocho_dark(),
            source: None,
        },
        ThemeEntry {
            theme: Theme::ocho_light(),
            source: None,
        },
    ]
}

/// Directories searched for Zed theme files, in priority order:
/// `$FLEET_HOME/themes`, `~/.local/share/fleet/themes`,
/// `~/.config/zed/themes`, then every `ZED_EXTENSIONS/*/themes` the host
/// finds under `home`. `fleet_home` defaults to `~/.local/share/fleet`.
pub fn theme_dirs(home: &str, fleet_home: Option<&str>) -> Vec<String> {
    let home = home.trim_end_matches('/');
    let fleet_home = fleet_home
        .map(|f| f.trim_end_matches('/').to_string())
        .unwrap_or_else(|| format!("{home}/.local/share/fleet"));
    let mut dirs = vec![
        format!("{fleet_home}/themes"),
        format!("{home}/.local/share/fleet/themes"),
        format!("{home}/.config/zed/themes"),
    ];
    dirs.dedup();
    dirs
}

/// Bundled themes plus every theme in `files` (path, text), taken in the
/// order given (list each directory's `.json` files sorted, directories in
/// `theme_dirs` order). Later duplicates by name are dropped.
pub fn discover(files: &[(String, String)]) -> Vec<ThemeEntry> {
    let mut out = bundled();
    for (path, text) in files {
        if !path.ends_with(".json") {
            continue;
        }
        for entry in parse_zed_file(text, Some(path)) {
            if !out.iter().any(|t| t.theme.name == entry.theme.name) {
                out.push(entry);
            }
        }
    }
    out
}

/// The theme called `name`, case-insensitively.
pub fn find<'a>(themes: &'a [ThemeEntry], name: &str) -> Option<&'a ThemeEntry> {
    let name = name.trim();
    themes
        .iter()
        .find(|t| t.theme.name.eq_ignore_ascii_case(name))
}

/// The theme Zed itself is using, from its settings (`theme` as a name, or
/// `{mode: light|dark|system, light, dark}`), given the system appearance.
pub fn zed_theme_name(settings: &Json, system_dark: bool) -> Option<String> {
    match settings.get("theme")? {
        Json::String(name) => Some(name.clone()),
        Json::Object(map) => {
            let mode = map.get("mode").and_then(Json::as_str).unwrap_or("system");
            let dark = match mode {
                "light" => false,
                "dark" => true,
                _ => system_dark,
            };
            map.get(if dark { "dark" } else { "light" })
                .and_then(Json::as_str)
                .map(str::to_string)
        }
        _ => None,
    }
}

/// Startup resolution (theme.rs `initial_theme`): `FLEET_THEME`, Ocho's own
/// saved choice, Zed's settings (JSONC text), then the bundled theme for
/// the system appearance.
pub fn initial_theme(
    themes: &[ThemeEntry],
    env_theme: Option<&str>,
    saved: Option<&str>,
    zed_settings: Option<&str>,
    system_dark: bool,
) -> Theme {
    let zed = zed_settings
        .and_then(|text| serde_json::from_str::<Json>(&strip_jsonc(text)).ok())
        .and_then(|value| zed_theme_name(&value, system_dark));
    let candidates = [
        env_theme.map(str::to_string),
        saved.map(str::to_string),
        zed,
    ];
    for name in candidates.into_iter().flatten() {
        if let Some(entry) = find(themes, &name) {
            return entry.theme.clone();
        }
    }
    Theme::bundled(system_dark)
}

/// Remove `//` and `/* */` comments plus trailing commas so Zed's JSONC
/// settings parse.
pub fn strip_jsonc(text: &str) -> String {
    remove_trailing_commas(&remove_comments(text))
}

fn remove_comments(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
        } else if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn remove_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
        } else if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
        } else if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if !(j < chars.len() && (chars[j] == '}' || chars[j] == ']')) {
                out.push(c);
            }
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

// ----- picker ----------------------------------------------------------------

/// What a theme picker key did.
#[derive(Clone, Debug, PartialEq)]
pub enum ThemeKey {
    /// The highlight moved: apply this theme as a preview.
    Preview(Theme),
    /// Enter: keep the theme in use; save its name; toast "Theme: {name}"
    /// (`from_settings` reopens Settings).
    Keep {
        /// The theme kept.
        theme: Theme,
        /// Reopen the Settings page.
        from_settings: bool,
    },
    /// Esc: apply this theme again (the one the picker opened on); `changed`
    /// says a preview had moved away from it. `from_settings` reopens
    /// Settings.
    Revert {
        /// The theme to restore.
        theme: Theme,
        /// A preview was showing.
        changed: bool,
        /// Reopen the Settings page.
        from_settings: bool,
    },
    /// Printable text the query input holds (`input` brings the value).
    Typed(String),
    /// Nothing.
    None,
}

/// The theme picker (`Overlay::Themes`).
#[derive(Clone, Debug, PartialEq)]
pub struct ThemePicker {
    /// The query and highlight.
    pub picker: PickerState,
    /// Every theme, bundled first.
    pub themes: Vec<ThemeEntry>,
    /// The theme in use when the picker opened.
    pub previous: Theme,
    /// The name of the theme applied now (a preview, or `previous`).
    pub current: String,
    /// Opened from the Settings page: Enter and Esc return there.
    pub from_settings: bool,
}

impl ThemePicker {
    /// Open over `themes` on the theme in use.
    pub fn open(themes: Vec<ThemeEntry>, current: &Theme, from_settings: bool) -> Self {
        let mut picker = PickerState::new();
        picker.select(
            themes
                .iter()
                .position(|t| t.theme.name == current.name)
                .unwrap_or(0),
            themes.len(),
        );
        ThemePicker {
            picker,
            themes,
            previous: current.clone(),
            current: current.name.clone(),
            from_settings,
        }
    }

    /// Indexes into `themes` matching the query, best first when there is one.
    pub fn matches(&self) -> Vec<usize> {
        let query = self.picker.query.as_str();
        let mut scored: Vec<(usize, i32)> = self
            .themes
            .iter()
            .enumerate()
            .filter_map(|(i, t)| fuzzy_score(query, &t.theme.name).map(|s| (i, s)))
            .collect();
        if !query.trim().is_empty() {
            scored.sort_by_key(|(_, s)| *s);
        }
        scored.into_iter().map(|(i, _)| i).collect()
    }

    /// Highlight the match at `position` and preview it.
    pub fn preview(&mut self, position: usize) -> Option<Theme> {
        let matches = self.matches();
        if matches.is_empty() {
            return None;
        }
        self.picker.select(position, matches.len());
        let next = self.themes[matches[self.picker.index]].theme.clone();
        self.current = next.name.clone();
        Some(next)
    }

    /// The query changed (the highlight stays where it is, clamped).
    pub fn input(&mut self, text: &str) {
        self.picker.query = text.to_string();
        let count = self.matches().len();
        self.picker.select(self.picker.index, count);
    }

    /// Enter: the theme in use stays.
    pub fn keep(&self) -> ThemeKey {
        let theme = find(&self.themes, &self.current)
            .map(|t| t.theme.clone())
            .unwrap_or_else(|| self.previous.clone());
        ThemeKey::Keep {
            theme,
            from_settings: self.from_settings,
        }
    }

    /// Esc: back to the theme the picker opened on.
    pub fn cancel(&self) -> ThemeKey {
        ThemeKey::Revert {
            theme: self.previous.clone(),
            changed: self.current != self.previous.name,
            from_settings: self.from_settings,
        }
    }

    /// One key.
    pub fn key(&mut self, name: &str, mods: &Mods) -> ThemeKey {
        let count = self.matches().len();
        match self.picker.key(name, mods, count) {
            PickerKey::Moved => match self.preview(self.picker.index) {
                Some(theme) => ThemeKey::Preview(theme),
                None => ThemeKey::None,
            },
            PickerKey::Chosen => self.keep(),
            PickerKey::Closed => self.cancel(),
            PickerKey::Typed(text) => ThemeKey::Typed(text),
            PickerKey::None => ThemeKey::None,
        }
    }

    /// A press on row `theme-N`: preview and keep the match at N.
    pub fn press(&mut self, id: &str) -> ThemeKey {
        let Some(position) = id.strip_prefix("theme-").and_then(|n| n.parse().ok()) else {
            return ThemeKey::None;
        };
        if self.preview(position).is_none() {
            return ThemeKey::None;
        }
        self.keep()
    }

    /// The `OVERLAY` card.
    pub fn view(&self) -> Json {
        let matches = self.matches();
        let rows: Vec<Json> = matches
            .iter()
            .enumerate()
            .map(|(position, theme_index)| {
                let entry = &self.themes[*theme_index];
                let mut row = picker::row(
                    &format!("theme-{position}"),
                    "",
                    &entry.theme.name,
                    &entry.detail(),
                    "",
                    position == self.picker.index,
                );
                picker::set(&mut row, "swatch", json!(entry.theme.bg.css()));
                picker::set(&mut row, "swatchBorder", json!(entry.theme.border.css()));
                row
            })
            .collect();
        let status = if matches.is_empty() { NO_MATCH } else { "" };
        json!({
            "kind": "picker", "width": WIDTH, "top": true, "title": "", "subtitle": "", "pill": "", "pillColor": "",
            "glyph": "›", "placeholder": PLACEHOLDER, "query": self.picker.query, "status": status, "statusError": false,
            "rows": rows, "index": self.picker.index, "footer": FOOTER, "body": "", "bodyMarkdown": false, "blocks": [], "fields": [],
            "buttons": [], "hint": "", "focusId": "overlay-query-input",
        })
    }
}

/// The toast after keeping a theme.
pub fn kept_message(name: &str) -> String {
    format!("Theme: {name}")
}

/// The toast when the theme applied but its name could not be saved.
pub fn not_saved_message(name: &str, error: &str) -> String {
    format!("Theme {name} applied but not saved: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(h: &str) -> Rgba {
        Rgba::parse(h).unwrap()
    }

    /// The bundled family as themes/ocho.json spells it (inventory §3).
    const OCHO_JSON: &str = r##"{
      "name": "Ocho", "author": "Ocho", // JSONC is fine
      "themes": [
        {"name": "Ocho Dark", "appearance": "dark", "style": {
          "background": "#282c34ff", "title_bar.background": "#282c34ff", "status_bar.background": "#282c34ff",
          "panel.background": "#282c34ff", "surface.background": "#282c34ff",
          "elevated_surface.background": "#2f333dff", "editor.background": "#1e2127ff",
          "element.background": "#2f333dff", "element.hover": "#2c313aff", "element.selected": "#353b47ff",
          "border": "#3b4048ff", "text": "#c8ccd4ff", "text.muted": "#8b919cff", "text.placeholder": "#6b717cff",
          "text.accent": "#5fd7d7ff", "warning": "#ffaf5fff", "error": "#e06c75ff", "success": "#98c379ff",
          "terminal.background": "#1b1e24ff", "terminal.foreground": "#d7dae0ff",
          "terminal.ansi.black": "#282c34ff", "terminal.ansi.red": "#e06c75ff", "terminal.ansi.green": "#98c379ff",
          "terminal.ansi.yellow": "#e5c07bff", "terminal.ansi.blue": "#61afefff", "terminal.ansi.magenta": "#c678ddff",
          "terminal.ansi.cyan": "#56b6c2ff", "terminal.ansi.white": "#abb2bfff",
          "terminal.ansi.bright_black": "#5c6370ff", "terminal.ansi.bright_red": "#ef7a82ff",
          "terminal.ansi.bright_green": "#a9d685ff", "terminal.ansi.bright_yellow": "#f0cc8aff",
          "terminal.ansi.bright_blue": "#74bcffff", "terminal.ansi.bright_magenta": "#d48eeaff",
          "terminal.ansi.bright_cyan": "#63c9d6ff", "terminal.ansi.bright_white": "#ffffffff",
          "players": [{"cursor": "#5fd7d7ff", "background": "#5fd7d7ff", "selection": "#5fd7d73d"}],
        }},
        {"name": "Ocho Light", "appearance": "light", "style": {
          "background": "#f0f0f1ff", "elevated_surface.background": "#ffffffff", "editor.background": "#fafafaff",
          "element.background": "#ffffffff", "element.hover": "#e9eaeeff", "element.selected": "#dfe1e6ff",
          "border": "#d4d5d9ff", "text": "#383a42ff", "text.muted": "#7c7f87ff", "text.placeholder": "#a0a1a7ff",
          "text.accent": "#0184bcff", "warning": "#c18401ff", "error": "#e45649ff", "success": "#50a14fff",
          "terminal.background": "#fafafaff", "terminal.foreground": "#383a42ff",
          "terminal.ansi.black": "#000000ff", "terminal.ansi.red": "#e45649ff", "terminal.ansi.green": "#50a14fff",
          "terminal.ansi.yellow": "#c18401ff", "terminal.ansi.blue": "#4078f2ff", "terminal.ansi.magenta": "#a626a4ff",
          "terminal.ansi.cyan": "#0997b3ff", "terminal.ansi.white": "#a0a1a7ff",
          "terminal.ansi.bright_black": "#5c6370ff", "terminal.ansi.bright_red": "#e45649ff",
          "terminal.ansi.bright_green": "#50a14fff", "terminal.ansi.bright_yellow": "#c18401ff",
          "terminal.ansi.bright_blue": "#4078f2ff", "terminal.ansi.bright_magenta": "#a626a4ff",
          "terminal.ansi.bright_cyan": "#0997b3ff", "terminal.ansi.bright_white": "#ffffffff",
          "players": [{"cursor": "#526fffff", "background": "#526fffff", "selection": "#526fff3d"}],
        }},
      ],
    }"##;

    #[test]
    fn bundled_themes_parse_to_the_bundled_palettes() {
        let themes = parse_zed_file(OCHO_JSON, Some("/x/ocho.json"));
        let names: Vec<&str> = themes.iter().map(|t| t.theme.name.as_str()).collect();
        assert_eq!(names, ["Ocho Dark", "Ocho Light"]);
        assert!(!themes[1].theme.dark);
        assert_eq!(themes[0].theme, Theme::ocho_dark());
        assert_eq!(themes[1].theme, Theme::ocho_light());
        assert_eq!(themes[0].source_label(), "ocho.json");
        assert_eq!(themes[0].detail(), "dark · ocho.json");
        assert_eq!(bundled()[1].detail(), "light · bundled");
    }

    #[test]
    fn zed_style_keys_map_onto_fleet_colors() {
        let text = r##"{"themes":[{"name":"T","appearance":"dark","style":{
            "background":"#4c4642ff","editor.background":"#1d2021ff","panel.background":"#393634ff",
            "title_bar.background":"#4c4642ff","text":"#fbf1c7ff","terminal.ansi.red":"#cc241dff",
            "players":[{"cursor":"#83a598ff"}]}}]}"##;
        let t = &parse_zed_file(text, None)[0].theme;
        assert_eq!(t.bg, hex("#1d2021"));
        assert_eq!(t.surface, hex("#393634"));
        assert_eq!(t.status_bar, hex("#4c4642"));
        assert_eq!(t.ansi[1], hex("#cc241d"));
        assert_eq!(t.cursor, hex("#83a598"));
        assert_eq!(t.term_bg, hex("#1d2021"));
        assert_eq!(t.term_fg, hex("#fbf1c7"));
        assert_eq!(
            t.elevated,
            hex("#4c4642"),
            "elevated falls back to background"
        );
        assert_eq!(t.element, t.elevated);
        assert_eq!(
            t.warn,
            Theme::ocho_dark().warn,
            "missing keys keep the fallback"
        );
    }

    #[test]
    fn jsonc_comments_and_trailing_commas_are_removed() {
        let text = "{\n  // comment\n  \"theme\": {\"mode\": \"dark\", \"dark\": \"Gruvbox Dark Hard\",},\n  \"x\": \"a//b\", /* c */\n}";
        let value: Json = serde_json::from_str(&strip_jsonc(text)).unwrap();
        assert_eq!(value["theme"]["dark"], "Gruvbox Dark Hard");
        assert_eq!(value["x"], "a//b");
    }

    #[test]
    fn placeholder_falls_back_to_muted() {
        let text = r##"{"themes":[{"name":"T","appearance":"dark","style":{
            "text.muted":"#c5b597ff","text.placeholder":"#998b78ff"}},
            {"name":"U","appearance":"dark","style":{"text.muted":"#c5b597ff"}}]}"##;
        let themes = parse_zed_file(text, None);
        assert_eq!(themes[0].theme.placeholder, hex("#998b78"));
        assert_eq!(themes[1].theme.placeholder, hex("#c5b597"));
        assert_eq!(themes[1].theme.muted, hex("#c5b597"));
    }

    #[test]
    fn hex_parsing() {
        assert!(Rgba::parse("#123456").is_some());
        assert!(Rgba::parse("#12345678").is_some());
        assert!(Rgba::parse("#1234").is_none());
        assert!(Rgba::parse("nope").is_none());
        assert!(parse_zed_file("not json", None).is_empty());
        assert!(parse_zed_file(r#"{"name":"x"}"#, None).is_empty());
    }

    #[test]
    fn discovery_keeps_the_first_of_a_name_and_bundled_themes_first() {
        let gruvbox = r##"{"themes":[{"name":"Gruvbox","appearance":"dark","style":{"background":"#282828ff"}},
            {"name":"Ocho Dark","appearance":"dark","style":{"background":"#000000ff"}}]}"##;
        let again = r##"{"themes":[{"name":"gruvbox","appearance":"light","style":{}},{"name":"Solar","appearance":"light","style":{}}]}"##;
        let themes = discover(&[
            ("/a/gruvbox.json".into(), gruvbox.into()),
            ("/a/notes.txt".into(), again.into()),
            ("/b/again.json".into(), again.into()),
        ]);
        let names: Vec<&str> = themes.iter().map(|t| t.theme.name.as_str()).collect();
        assert_eq!(
            names,
            ["Ocho Dark", "Ocho Light", "Gruvbox", "gruvbox", "Solar"]
        );
        assert_eq!(themes[0].theme, Theme::ocho_dark());
        assert_eq!(themes[2].source.as_deref(), Some("/a/gruvbox.json"));
        assert_eq!(find(&themes, " solar ").unwrap().theme.name, "Solar");
        assert_eq!(
            theme_dirs("/Users/me/", None),
            vec![
                "/Users/me/.local/share/fleet/themes",
                "/Users/me/.config/zed/themes"
            ]
        );
        assert_eq!(
            theme_dirs("/Users/me", Some("/srv/fleet"))[0],
            "/srv/fleet/themes"
        );
    }

    #[test]
    fn startup_resolution_follows_env_saved_zed_then_system() {
        let themes = discover(&[(
            "/a/t.json".into(),
            r##"{"themes":[{"name":"Night","appearance":"dark","style":{}},{"name":"Day","appearance":"light","style":{}}]}"##.into(),
        )]);
        let zed = r#"{"theme": {"mode": "system", "light": "Day", "dark": "Night"}, // c
        }"#;
        assert_eq!(
            initial_theme(&themes, Some("Day"), Some("Night"), Some(zed), true).name,
            "Day"
        );
        assert_eq!(
            initial_theme(&themes, Some("Missing"), Some("Night"), Some(zed), false).name,
            "Night"
        );
        assert_eq!(
            initial_theme(&themes, None, None, Some(zed), false).name,
            "Day"
        );
        assert_eq!(
            initial_theme(&themes, None, None, Some(zed), true).name,
            "Night"
        );
        assert_eq!(
            initial_theme(&themes, None, None, None, false).name,
            "Ocho Light"
        );
        let value: Json = serde_json::from_str(r#"{"theme": "One Dark"}"#).unwrap();
        assert_eq!(zed_theme_name(&value, true).as_deref(), Some("One Dark"));
        let value: Json =
            serde_json::from_str(r#"{"theme": {"mode": "light", "light": "L", "dark": "D"}}"#)
                .unwrap();
        assert_eq!(zed_theme_name(&value, true).as_deref(), Some("L"));
        assert_eq!(zed_theme_name(&json!({}), true), None);
    }

    #[test]
    fn picker_previews_on_move_keeps_on_enter_and_reverts_on_escape() {
        let themes = discover(&[(
            "/a/t.json".into(),
            r##"{"themes":[{"name":"Night","appearance":"dark","style":{"background":"#111111ff","border":"#222222ff"}}]}"##.into(),
        )]);
        let light = Theme::ocho_light();
        let mut picker = ThemePicker::open(themes.clone(), &light, true);
        assert_eq!(picker.picker.index, 1, "opens on the theme in use");
        let v = picker.view();
        assert_eq!(v["kind"], "picker");
        assert_eq!(v["width"], 560.0);
        assert_eq!(v["footer"], FOOTER);
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2]["label"], "Night");
        assert_eq!(rows[2]["detail"], "dark · t.json");
        assert_eq!(rows[2]["swatch"], "#111111");
        assert_eq!(rows[2]["swatchBorder"], "#222222");
        assert_eq!(rows[1]["selected"], true);
        let ThemeKey::Preview(theme) = picker.key("ArrowDown", &Mods::NONE) else {
            panic!("moving previews");
        };
        assert_eq!(theme.name, "Night");
        assert_eq!(picker.current, "Night");
        assert_eq!(
            picker.cancel(),
            ThemeKey::Revert {
                theme: light.clone(),
                changed: true,
                from_settings: true
            }
        );
        let ThemeKey::Keep {
            theme,
            from_settings,
        } = picker.key("Enter", &Mods::NONE)
        else {
            panic!("enter keeps");
        };
        assert_eq!(theme.name, "Night");
        assert!(from_settings);
        // Typing filters; the query is typed into, so j is text.
        let mut picker = ThemePicker::open(themes.clone(), &Theme::ocho_dark(), false);
        assert_eq!(picker.key("j", &Mods::NONE), ThemeKey::Typed("j".into()));
        picker.input("nig");
        assert_eq!(picker.matches(), vec![2]);
        assert_eq!(picker.view()["rows"].as_array().unwrap().len(), 1);
        picker.input("zzz");
        assert_eq!(picker.view()["status"], NO_MATCH);
        assert_eq!(picker.key("ArrowDown", &Mods::NONE), ThemeKey::None);
        assert_eq!(
            picker.key("Escape", &Mods::NONE),
            ThemeKey::Revert {
                theme: Theme::ocho_dark(),
                changed: false,
                from_settings: false
            }
        );
        picker.input("");
        let ThemeKey::Keep { theme, .. } = picker.press("theme-1") else {
            panic!("press keeps");
        };
        assert_eq!(theme.name, "Ocho Light");
        assert_eq!(picker.press("theme-x"), ThemeKey::None);
        assert_eq!(kept_message("Night"), "Theme: Night");
        assert_eq!(
            not_saved_message("Night", "read-only"),
            "Theme Night applied but not saved: read-only"
        );
    }
}
