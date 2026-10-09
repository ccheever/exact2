//! One binding table for every key Ocho itself interprets. Key dispatch, the
//! command palette's key column, button tooltips, menus, and the help overlay
//! all read from it, so a hint can never describe a key the code does not
//! handle.
//!
//! Bindings are resolved against a stack of scopes, innermost first, the way
//! Zed resolves a keystroke against the key contexts from the focused element
//! up to the window: the Sessions page sees `Sessions`, then `Manager`, then
//! `Global`; the rail sees `Rail`, then `Global`.
//!
//! Ported from `fleet/desktop/src/keymap.rs` (origin/main e6adfa8). The table
//! keeps GPUI's keystroke syntax verbatim; keys arrive from the contract as
//! the web's `KeyboardEvent.key` names plus a modifier list, carried by
//! [`Keystroke`]. How the two meet:
//!
//! | GPUI binding    | [`Keystroke`] it matches (`key`, modifiers)                          |
//! |-----------------|----------------------------------------------------------------------|
//! | `cmd-n`         | `"n"` + meta (letter compared ignoring case)                         |
//! | `cmd-shift-p`   | `"P"` + meta + shift (a browser reports the shifted letter)          |
//! | `cmd-alt-p`     | `"p"` + meta + alt                                                   |
//! | `cmd-shift-]`   | `"]"` + meta + shift, or `"}"` + meta (with or without shift)        |
//! | `cmd-shift-[`   | `"["` + meta + shift, or `"{"` + meta                                |
//! | `cmd-.` `cmd-,` | `"."` / `","` + meta                                                 |
//! | `cmd-1`         | `"1"` + meta                                                         |
//! | `ctrl-d`        | `"d"` + ctrl                                                         |
//! | `shift-tab`     | `"Tab"` + shift                                                      |
//! | `tab` `enter`   | `"Tab"`, `"Enter"` with no modifier (shift must be off)              |
//! | `escape`        | `"Escape"`                                                           |
//! | `up` … `right`  | `"ArrowUp"`, `"ArrowDown"`, `"ArrowLeft"`, `"ArrowRight"`            |
//! | `space`         | `" "`                                                                |
//! | `backspace` …   | `"Backspace"`, `"Delete"`, `"Home"`, `"End"`, `"PageUp"`, `"PageDown"` |
//! | `j` `1` `/` `:` | that character with no meta, ctrl or alt (shift ignored: it is       |
//! |                 | already folded into the character the browser reports)               |
//! | `G` `?` `A` `H` | the shifted character itself: `"G"`, `"?"`, …, so `g` ≠ `G`          |
//! | `g g`           | `"g"` twice: the first resolves as `Pending`, the second with it     |
//!
//! Modifier names accepted in a modifier list: `meta` (also `cmd`,
//! `command`, `platform`), `shift`, `alt` (also `option`), `ctrl` (also
//! `control`), joined by `+`.

use serde::Serialize;

use crate::model::Page;
use crate::palette::Command;

/// Where a key is looked up; scopes stack innermost first.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize)]
pub enum Scope {
    /// ⌘ shortcuts that work everywhere, including over a terminal.
    Global,
    /// The three manager pages.
    Manager,
    /// The Sessions page.
    Sessions,
    /// The Machines page.
    Machines,
    /// The Accounts page.
    Accounts,
    /// The rail has focus (NAV): keys move between rail rows.
    Rail,
    /// A terminal tab whose connection ended.
    Disconnected,
    /// Shown in help but not dispatched here: the terminal, launch dialog and
    /// forms interpret these keys themselves.
    Info,
}

impl Scope {
    /// The heading the help overlay gives this scope.
    pub fn title(self) -> &'static str {
        match self {
            Scope::Global => "Everywhere",
            Scope::Manager => "Manager",
            Scope::Sessions => "Sessions",
            Scope::Machines => "Machines",
            Scope::Accounts => "Accounts",
            Scope::Rail => "Rail (NAV)",
            Scope::Disconnected => "Disconnected terminal",
            Scope::Info => "Terminal, dialogs and forms",
        }
    }

    /// The scope for a page's own keys (`Page::scope` in the GPUI workspace).
    pub fn for_page(page: Page) -> Scope {
        match page {
            Page::Machines => Scope::Machines,
            Page::Sessions => Scope::Sessions,
            Page::Accounts => Scope::Accounts,
        }
    }

    /// The scopes the help overlay lists, in its order; `Info` last.
    pub const HELP: [Scope; 8] = [
        Scope::Global,
        Scope::Manager,
        Scope::Sessions,
        Scope::Machines,
        Scope::Accounts,
        Scope::Rail,
        Scope::Disconnected,
        Scope::Info,
    ];
}

/// One key or key sequence bound to a command in a scope.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    /// The scope the binding applies in.
    pub scope: Scope,
    /// GPUI keystroke syntax; a space separates the keys of a sequence
    /// (`"g g"`). Printable keys are matched by the character they produce,
    /// so `"?"` and `"G"` work regardless of keyboard layout.
    pub keys: &'static str,
    /// The command the keys run.
    pub command: Command,
}

const fn b(scope: Scope, keys: &'static str, command: Command) -> Binding {
    Binding {
        scope,
        keys,
        command,
    }
}

use Scope::*;

/// Every binding, in display order within each scope. The first binding for
/// a command in a scope is the one tooltips show.
pub const BINDINGS: &[Binding] = &[
    // ----- everywhere ------------------------------------------------------
    b(Global, "cmd-p", Command::Palette),
    b(Global, "cmd-shift-p", Command::Palette),
    b(Global, "cmd-alt-p", Command::PullRequests),
    b(Global, "cmd-shift-f", Command::Conversations),
    b(Global, "cmd-n", Command::New),
    b(Global, "cmd-shift-n", Command::QuickLaunch),
    b(Global, "cmd-alt-n", Command::NewWindow),
    b(Global, "cmd-w", Command::CloseTab),
    b(Global, "cmd-shift-w", Command::CloseWindow),
    b(Global, "cmd-r", Command::Refresh),
    b(Global, "cmd-shift-]", Command::NextTab),
    b(Global, "cmd-shift-[", Command::PrevTab),
    b(Global, "cmd-0", Command::SelectTab(0)),
    b(Global, "cmd-1", Command::SelectTab(1)),
    b(Global, "cmd-2", Command::SelectTab(2)),
    b(Global, "cmd-3", Command::SelectTab(3)),
    b(Global, "cmd-4", Command::SelectTab(4)),
    b(Global, "cmd-5", Command::SelectTab(5)),
    b(Global, "cmd-6", Command::SelectTab(6)),
    b(Global, "cmd-7", Command::SelectTab(7)),
    b(Global, "cmd-8", Command::SelectTab(8)),
    b(Global, "cmd-9", Command::SelectTab(9)),
    b(Global, "cmd-alt-t", Command::ToggleTranscript),
    b(Global, "cmd-shift-t", Command::ReopenClosedTab),
    b(Global, "cmd-k", Command::ClearScrollback),
    b(Global, "cmd-.", Command::Interrupt),
    b(Global, "cmd-,", Command::Settings),
    b(Global, "cmd-z", Command::Undo),
    b(Global, "cmd-/", Command::Help),
    b(Global, "cmd-q", Command::Quit),
    b(Global, "cmd-shift-e", Command::EnterNav),
    // ----- manager pages ---------------------------------------------------
    b(Manager, "cmd-f", Command::Search),
    b(Manager, "j", Command::Down),
    b(Manager, "down", Command::Down),
    b(Manager, "k", Command::Up),
    b(Manager, "up", Command::Up),
    b(Manager, "g g", Command::First),
    b(Manager, "G", Command::Last),
    b(Manager, "ctrl-d", Command::HalfDown),
    b(Manager, "ctrl-u", Command::HalfUp),
    b(Manager, "h", Command::PrevPage),
    b(Manager, "left", Command::PrevPage),
    b(Manager, "shift-tab", Command::PrevPage),
    b(Manager, "l", Command::NextPage),
    b(Manager, "right", Command::NextPage),
    b(Manager, "tab", Command::NextPage),
    b(Manager, "1", Command::Page(0)),
    b(Manager, "2", Command::Page(1)),
    b(Manager, "3", Command::Page(2)),
    b(Manager, "enter", Command::Open),
    b(Manager, "/", Command::Search),
    b(Manager, "escape", Command::ClearFilter),
    b(Manager, "r", Command::Refresh),
    b(Manager, ":", Command::Palette),
    b(Manager, "?", Command::Help),
    b(Manager, "n", Command::New),
    // ----- sessions ---------------------------------------------------------
    b(Sessions, "v", Command::ViewReadOnly),
    b(Sessions, "m", Command::Message),
    b(Sessions, "e", Command::Edit),
    b(Sessions, "a", Command::Track),
    b(Sessions, "u", Command::ToggleOthers),
    b(Sessions, "H", Command::ToggleHistory),
    b(Sessions, "I", Command::ToggleNonRunning),
    b(Sessions, "p", Command::Pin),
    b(Sessions, "A", Command::Archive),
    b(Sessions, "z", Command::Pause),
    b(Sessions, "o", Command::Resume),
    b(Sessions, "M", Command::Move),
    b(Sessions, "x", Command::Stop),
    b(Sessions, "d", Command::Delete),
    // ----- machines ---------------------------------------------------------
    b(Machines, "c", Command::Shell),
    b(Machines, "a", Command::AddMachine),
    b(Machines, "e", Command::Edit),
    b(Machines, "u", Command::UpdateMachine),
    b(Machines, "d", Command::Delete),
    // ----- accounts ---------------------------------------------------------
    b(Accounts, "n", Command::AddAccount),
    b(Accounts, "a", Command::AddAccount),
    b(Accounts, "b", Command::BackupRecovery),
    b(Accounts, "e", Command::Edit),
    // ----- rail (NAV) -------------------------------------------------------
    b(Rail, "j", Command::RailDown),
    b(Rail, "down", Command::RailDown),
    b(Rail, "l", Command::RailDown),
    b(Rail, "right", Command::RailDown),
    b(Rail, "k", Command::RailUp),
    b(Rail, "up", Command::RailUp),
    b(Rail, "h", Command::RailUp),
    b(Rail, "left", Command::RailUp),
    b(Rail, "0", Command::SelectTab(0)),
    b(Rail, "1", Command::SelectTab(1)),
    b(Rail, "2", Command::SelectTab(2)),
    b(Rail, "3", Command::SelectTab(3)),
    b(Rail, "4", Command::SelectTab(4)),
    b(Rail, "5", Command::SelectTab(5)),
    b(Rail, "6", Command::SelectTab(6)),
    b(Rail, "7", Command::SelectTab(7)),
    b(Rail, "8", Command::SelectTab(8)),
    b(Rail, "9", Command::SelectTab(9)),
    b(Rail, "x", Command::CloseTab),
    b(Rail, "r", Command::ReconnectTab),
    b(Rail, "e", Command::Edit),
    b(Rail, "enter", Command::ExitNav),
    b(Rail, "i", Command::ExitNav),
    b(Rail, "escape", Command::ExitNav),
    b(Rail, ":", Command::Palette),
    b(Rail, "?", Command::Help),
    // ----- disconnected terminal -------------------------------------------
    b(Disconnected, "r", Command::ReconnectTab),
    b(Disconnected, "enter", Command::ReconnectTab),
    b(Disconnected, "x", Command::CloseTab),
    b(Disconnected, "e", Command::Edit),
    b(Disconnected, "j", Command::NextTab),
    b(Disconnected, "k", Command::PrevTab),
    b(Disconnected, "0", Command::SelectTab(0)),
    b(Disconnected, ":", Command::Palette),
    b(Disconnected, "?", Command::Help),
];

/// Help entries for keys the terminal, dialogs and forms interpret themselves.
pub const INFO: &[(&str, &str)] = &[
    ("Ctrl+C", "interrupt the agent in its terminal"),
    (
        "⌘⇧E",
        "focus the rail (NAV); press again to return to the terminal",
    ),
    ("⌘⌥1–9", "launch the profile with that shortcut"),
    ("Drag · ⌘C", "select and copy terminal text"),
    ("⌘V", "paste text or an image into the terminal"),
    (
        "Shift+Enter",
        "newline in Codex, Claude and OpenCode (CSI-u)",
    ),
    (
        "Right-click",
        "menu for a session, machine, profile or rail tab",
    ),
    ("Double-click", "open a row · rename a rail tab"),
    (
        "Drag a rail tab",
        "reorder · drop onto a tab or folder to nest it",
    ),
    (
        "Launch dialog",
        "type the prompt · Tab moves between controls · Enter launches · Esc keeps a draft · Permissions chip overrides the machine default",
    ),
    (
        "Pickers",
        "type to filter · ↑ ↓ or Ctrl+N Ctrl+P choose · Enter opens · Esc closes",
    ),
    (
        "Forms",
        "Tab / Shift+Tab move · Enter submits on the last field · Esc keeps a draft",
    ),
    (
        "Dropdowns",
        "Enter, Space or ↓ open · ↑ ↓ choose · Enter picks · Esc closes",
    ),
    (
        "Directory field",
        "Ctrl+N / Ctrl+P choose a folder · Tab completes",
    ),
];

// ----- keystrokes ----------------------------------------------------------

/// A key as the contract reports it: the web's `KeyboardEvent.key` name
/// (`"n"`, `"P"`, `"?"`, `"ArrowDown"`, `"Enter"`, `" "`) and the
/// modifiers held. Letters carry their case: `"G"` is shift-g.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct Keystroke {
    /// The key's name.
    pub key: String,
    /// ⌘ held.
    pub meta: bool,
    /// ⇧ held.
    pub shift: bool,
    /// ⌥ held.
    pub alt: bool,
    /// ⌃ held.
    pub ctrl: bool,
}

/// GPUI key names that are not the character they type, and the web name
/// for each.
const NAMED: &[(&str, &str)] = &[
    ("enter", "Enter"),
    ("escape", "Escape"),
    ("tab", "Tab"),
    ("space", " "),
    ("up", "ArrowUp"),
    ("down", "ArrowDown"),
    ("left", "ArrowLeft"),
    ("right", "ArrowRight"),
    ("backspace", "Backspace"),
    ("delete", "Delete"),
    ("home", "Home"),
    ("end", "End"),
    ("pageup", "PageUp"),
    ("pagedown", "PageDown"),
];

fn web_key(gpui: &str) -> String {
    NAMED
        .iter()
        .find(|(g, _)| *g == gpui)
        .map(|(_, w)| (*w).to_string())
        .unwrap_or_else(|| gpui.to_string())
}

impl Keystroke {
    /// A key from the contract: its web name and a `+`-joined modifier list
    /// (`"meta+shift"`; empty for none).
    pub fn new(key: &str, mods: &str) -> Keystroke {
        let mut ks = Keystroke {
            key: key.to_string(),
            ..Default::default()
        };
        for m in mods.split(['+', ',', ' ']).filter(|m| !m.is_empty()) {
            match m.to_ascii_lowercase().as_str() {
                "meta" | "cmd" | "command" | "platform" | "super" => ks.meta = true,
                "shift" => ks.shift = true,
                "alt" | "option" => ks.alt = true,
                "ctrl" | "control" => ks.ctrl = true,
                _ => {}
            }
        }
        ks
    }

    /// The keystroke a GPUI chord names, as a browser would report it:
    /// `"cmd-shift-p"` is `"P"` + meta + shift, `"shift-tab"` is `"Tab"` +
    /// shift, `"G"` is `"G"` + shift, `"down"` is `"ArrowDown"`. One key only;
    /// split a sequence on spaces first.
    pub fn parse(text: &str) -> Keystroke {
        let (mods, key) = split_chord(text);
        let mut ks = Keystroke::new(&web_key(key), &mods.replace('-', "+"));
        let is_letter = ks.key.chars().count() == 1 && ks.key.chars().all(|c| c.is_alphabetic());
        if is_letter {
            if ks.shift {
                ks.key = ks.key.to_uppercase();
            } else if ks.key.chars().all(char::is_uppercase) {
                // A bare `G` in the table means shift-g.
                ks.shift = true;
            }
        }
        ks
    }

    /// The character this key types, if it is a plain printable one (no ⌘,
    /// ⌃ or ⌥; shift is already folded into the name).
    pub fn typed(&self) -> Option<&str> {
        if self.meta || self.ctrl || self.alt {
            return None;
        }
        let mut chars = self.key.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) if !c.is_control() => Some(self.key.as_str()),
            _ => None,
        }
    }

    /// The display form in Apple's modifier order: `⇧⌘P`, `⌃D`, `Enter`, `↓`.
    pub fn display(&self) -> String {
        let mut out = modifier_glyphs(self.ctrl, self.alt, self.shift, self.meta);
        let name = NAMED
            .iter()
            .find(|(_, w)| *w == self.key)
            .map(|(g, _)| *g)
            .unwrap_or(self.key.as_str());
        out.push_str(&key_name(name));
        out
    }
}

/// `"cmd-shift-]"` → (`"cmd-shift"`, `"]"`); `"-"` and `"cmd--"` keep the
/// dash as the key.
fn split_chord(text: &str) -> (&str, &str) {
    match text.rsplit_once('-') {
        Some((mods, "")) => (mods.trim_end_matches('-'), "-"),
        Some((mods, key)) => (mods, key),
        None => ("", text),
    }
}

/// One key of a binding: the character it produces (for printable keys) or a
/// modifier + key name (for everything else).
#[derive(Clone, PartialEq, Eq, Debug)]
enum KeySpec {
    /// A printable character as typed, so `?` matches whichever physical key
    /// produces it and `G` means shift-g.
    Char(String),
    /// Modifiers plus a key, in web names.
    Chord(Keystroke),
}

fn parse_spec(text: &str) -> KeySpec {
    let is_named = NAMED.iter().any(|(g, _)| *g == text);
    if !text.contains('-') && !is_named {
        return KeySpec::Char(text.to_string());
    }
    if text == "-" {
        return KeySpec::Char("-".to_string());
    }
    let (mods, key) = split_chord(text);
    KeySpec::Chord(Keystroke::new(&web_key(key), &mods.replace('-', "+")))
}

fn spec_matches(spec: &KeySpec, ks: &Keystroke) -> bool {
    match spec {
        KeySpec::Char(c) => ks.typed() == Some(c.as_str()),
        KeySpec::Chord(want) => {
            let (want_key, want_shift) = unshift(&want.key, want.shift);
            let (have_key, have_shift) = unshift(&ks.key, ks.shift);
            ks.meta == want.meta
                && ks.ctrl == want.ctrl
                && ks.alt == want.alt
                && have_shift == want_shift
                && have_key.eq_ignore_ascii_case(&want_key)
        }
    }
}

/// `⌘⇧]` arrives as `]` with shift on some layouts and as `}` without it on
/// others; compare both as `]` plus shift.
fn unshift(key: &str, shift: bool) -> (String, bool) {
    match key {
        "}" => ("]".into(), true),
        "{" => ("[".into(), true),
        other => (other.to_string(), shift),
    }
}

fn parse_sequence(keys: &str) -> Vec<KeySpec> {
    keys.split(' ')
        .filter(|k| !k.is_empty())
        .map(parse_spec)
        .collect()
}

/// What a keystroke meant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// A binding matched.
    Command(Command),
    /// The keystroke is the start of a longer sequence; wait for the rest.
    Pending,
    /// Nothing in these scopes.
    None,
}

/// Resolve a keystroke, after any `pending` prefix, against `scopes` from the
/// innermost outward. Scopes not listed are never consulted.
pub fn resolve(scopes: &[Scope], ks: &Keystroke, pending: Option<&Keystroke>) -> Resolution {
    let mut partial = false;
    for scope in scopes {
        for binding in BINDINGS.iter().filter(|b| b.scope == *scope) {
            let sequence = parse_sequence(binding.keys);
            match (pending, sequence.as_slice()) {
                (None, [only]) => {
                    if spec_matches(only, ks) {
                        return Resolution::Command(binding.command);
                    }
                }
                (None, [first, ..]) => {
                    if spec_matches(first, ks) {
                        partial = true;
                    }
                }
                (Some(prefix), [first, second])
                    if spec_matches(first, prefix) && spec_matches(second, ks) =>
                {
                    return Resolution::Command(binding.command);
                }
                _ => {}
            }
        }
        // A key that only partially matches in the inner scope still waits
        // for its sequence, like Zed's pending keystrokes, instead of falling
        // through to an outer scope's single-key binding.
        if partial {
            return Resolution::Pending;
        }
    }
    Resolution::None
}

// ----- display -------------------------------------------------------------

/// The display form of one binding in Apple's modifier order: `⇧⌘P`, `g g`,
/// `⌃D`, `Enter`.
pub fn display(keys: &str) -> String {
    keys.split(' ')
        .filter(|k| !k.is_empty())
        .map(display_one)
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_one(key: &str) -> String {
    match parse_spec(key) {
        KeySpec::Char(c) => c,
        KeySpec::Chord(_) => {
            let (mods, name) = split_chord(key);
            let mods: Vec<&str> = mods.split('-').collect();
            let mut out = modifier_glyphs(
                mods.contains(&"ctrl"),
                mods.contains(&"alt"),
                mods.contains(&"shift"),
                mods.contains(&"cmd"),
            );
            out.push_str(&key_name(name));
            out
        }
    }
}

fn modifier_glyphs(ctrl: bool, alt: bool, shift: bool, meta: bool) -> String {
    let mut out = String::new();
    if ctrl {
        out.push('⌃');
    }
    if alt {
        out.push('⌥');
    }
    if shift {
        out.push('⇧');
    }
    if meta {
        out.push('⌘');
    }
    out
}

/// The display name of a GPUI key name.
fn key_name(key: &str) -> String {
    match key {
        "enter" => "Enter".into(),
        "escape" => "Esc".into(),
        "tab" => "Tab".into(),
        "space" => "Space".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        "backspace" => "⌫".into(),
        "delete" => "⌦".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        "pageup" => "PgUp".into(),
        "pagedown" => "PgDn".into(),
        other if other.chars().count() == 1 => other.to_uppercase(),
        other => other.to_string(),
    }
}

/// Every display form bound to `command` within `scopes`, innermost scope
/// first, without duplicates.
pub fn keys_for(command: Command, scopes: &[Scope]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for scope in scopes {
        for binding in BINDINGS
            .iter()
            .filter(|b| b.scope == *scope && b.command == command)
        {
            let shown = display(binding.keys);
            if !out.contains(&shown) {
                out.push(shown);
            }
        }
    }
    out
}

/// The single best hint for a command in `scopes`: the innermost scope's
/// first binding, or nothing.
pub fn hint(command: Command, scopes: &[Scope]) -> String {
    keys_for(command, scopes)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// Every binding shown in one line: `j · ↓`.
pub fn hints(command: Command, scopes: &[Scope]) -> String {
    keys_for(command, scopes).join(" · ")
}

// ----- help ----------------------------------------------------------------

/// One scope's help rows: each command once, its keys merged with ` · `
/// (`"j · ↓"`, `"Next row"`); `Info` gives the [`INFO`] rows.
pub fn help_rows(scope: Scope) -> Vec<(String, String)> {
    if scope == Info {
        return INFO
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
    }
    let mut rows: Vec<(String, String)> = Vec::new();
    for binding in BINDINGS.iter().filter(|b| b.scope == scope) {
        let label = crate::palette::label(binding.command).to_string();
        if let Some(row) = rows.iter_mut().find(|(_, l)| *l == label) {
            let shown = display(binding.keys);
            if !row.0.split(" · ").any(|k| k == shown) {
                row.0.push_str(" · ");
                row.0.push_str(&shown);
            }
        } else {
            rows.push((display(binding.keys), label));
        }
    }
    rows
}

/// Help overlay sections for `scopes`, in that order: each scope's title with
/// its [`help_rows`]. The GPUI dialog shows [`Scope::HELP`].
pub fn help_sections_for(scopes: &[Scope]) -> Vec<(&'static str, Vec<(String, String)>)> {
    scopes
        .iter()
        .map(|scope| (scope.title(), help_rows(*scope)))
        .collect()
}

/// Help overlay sections: each scope's commands with their keys, then the
/// keys interpreted elsewhere.
pub fn help_sections() -> Vec<(&'static str, Vec<(String, String)>)> {
    help_sections_for(&Scope::HELP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ks(text: &str) -> Keystroke {
        Keystroke::parse(text)
    }

    fn typed_ks(c: &str, shift: bool) -> Keystroke {
        Keystroke {
            key: c.to_string(),
            shift,
            ..Default::default()
        }
    }

    #[test]
    fn keystrokes_parse_from_gpui_syntax_and_the_contract() {
        assert_eq!(
            ks("cmd-shift-]"),
            Keystroke {
                key: "]".into(),
                meta: true,
                shift: true,
                ..Default::default()
            }
        );
        assert_eq!(
            ks("cmd-shift-p"),
            Keystroke {
                key: "P".into(),
                meta: true,
                shift: true,
                ..Default::default()
            }
        );
        assert_eq!(ks("G"), typed_ks("G", true));
        assert_eq!(ks("g"), typed_ks("g", false));
        assert_eq!(ks("down").key, "ArrowDown");
        assert_eq!(ks("shift-tab"), Keystroke::new("Tab", "shift"));
        assert_eq!(ks("space").key, " ");
        assert_eq!(ks("cmd-."), Keystroke::new(".", "meta"));
        assert_eq!(ks("ctrl-d"), Keystroke::new("d", "ctrl"));
        assert_eq!(Keystroke::new("n", "meta+alt"), ks("cmd-alt-n"));
        let all = Keystroke::new("x", "control+option+cmd");
        assert!(all.ctrl && all.alt && all.meta && !all.shift);
    }

    #[test]
    fn keystrokes_display_like_bindings() {
        assert_eq!(ks("cmd-shift-p").display(), "⇧⌘P");
        assert_eq!(ks("ctrl-d").display(), "⌃D");
        assert_eq!(ks("down").display(), "↓");
        assert_eq!(ks("enter").display(), "Enter");
        assert_eq!(ks("escape").display(), "Esc");
        assert_eq!(ks("shift-tab").display(), "⇧Tab");
        assert_eq!(ks("cmd-.").display(), "⌘.");
        assert_eq!(Keystroke::new("?", "shift").display(), "⇧?");
    }

    #[test]
    fn quick_launch_is_global_and_distinct_from_the_normal_launcher() {
        assert_eq!(resolve(&[Global], &ks("ctrl-n"), None), Resolution::None);
        assert_eq!(
            resolve(&[Global], &ks("cmd-shift-n"), None),
            Resolution::Command(Command::QuickLaunch)
        );
        assert_eq!(
            resolve(&[Global], &ks("cmd-alt-n"), None),
            Resolution::Command(Command::NewWindow)
        );
        assert_eq!(
            resolve(&[Global], &ks("cmd-n"), None),
            Resolution::Command(Command::New)
        );
        assert_eq!(resolve(&[Global], &ks("n"), None), Resolution::None);
    }

    #[test]
    fn inner_scope_wins_over_outer() {
        // `a` tracks a session on the Sessions page and adds a machine on Machines.
        assert_eq!(
            resolve(&[Sessions, Manager, Global], &typed_ks("a", false), None),
            Resolution::Command(Command::Track)
        );
        assert_eq!(
            resolve(&[Machines, Manager, Global], &typed_ks("a", false), None),
            Resolution::Command(Command::AddMachine)
        );
        assert_eq!(
            resolve(&[Accounts, Manager, Global], &typed_ks("a", false), None),
            Resolution::Command(Command::AddAccount)
        );
        // `n` launches on Sessions and Machines but adds on Accounts.
        assert_eq!(
            resolve(&[Sessions, Manager, Global], &typed_ks("n", false), None),
            Resolution::Command(Command::New)
        );
        assert_eq!(
            resolve(&[Accounts, Manager, Global], &typed_ks("n", false), None),
            Resolution::Command(Command::AddAccount)
        );
        assert_eq!(
            resolve(&[Accounts, Manager, Global], &typed_ks("b", false), None),
            Resolution::Command(Command::BackupRecovery)
        );
    }

    #[test]
    fn letters_never_interrupt_or_leak() {
        assert_eq!(
            resolve(&[Sessions, Manager, Global], &typed_ks("i", false), None),
            Resolution::None
        );
        assert_eq!(
            resolve(&[Sessions, Manager, Global], &typed_ks("q", false), None),
            Resolution::None
        );
    }

    #[test]
    fn shifted_characters_match_what_was_typed() {
        assert_eq!(
            resolve(&[Manager, Global], &typed_ks("G", true), None),
            Resolution::Command(Command::Last)
        );
        assert_eq!(
            resolve(&[Manager, Global], &typed_ks("?", true), None),
            Resolution::Command(Command::Help)
        );
        assert_eq!(
            resolve(&[Sessions, Manager, Global], &typed_ks("A", true), None),
            Resolution::Command(Command::Archive)
        );
        // Plain `a` is not `A`.
        assert_ne!(
            resolve(&[Sessions, Manager, Global], &typed_ks("a", false), None),
            Resolution::Command(Command::Archive)
        );
    }

    #[test]
    fn sequences_wait_for_their_second_key() {
        let g = typed_ks("g", false);
        assert_eq!(resolve(&[Manager, Global], &g, None), Resolution::Pending);
        assert_eq!(
            resolve(&[Manager, Global], &g, Some(&g)),
            Resolution::Command(Command::First)
        );
        assert_eq!(
            resolve(&[Manager, Global], &typed_ks("j", false), Some(&g)),
            Resolution::None
        );
        assert_eq!(
            resolve(&[Global], &ks("cmd-k"), None),
            Resolution::Command(Command::ClearScrollback)
        );
        // Outside the manager `g` means nothing and never waits.
        assert_eq!(resolve(&[Rail, Global], &g, None), Resolution::None);
    }

    #[test]
    fn command_chords_match_modifiers_exactly() {
        assert_eq!(
            resolve(&[Global], &ks("cmd-shift-p"), None),
            Resolution::Command(Command::Palette)
        );
        assert_eq!(
            resolve(&[Global], &ks("cmd-alt-p"), None),
            Resolution::Command(Command::PullRequests)
        );
        assert_eq!(
            resolve(&[Global], &ks("cmd-p"), None),
            Resolution::Command(Command::Palette)
        );
        assert_eq!(resolve(&[Global], &ks("ctrl-e"), None), Resolution::None);
        assert_eq!(
            resolve(&[Global], &ks("ctrl-space"), None),
            Resolution::None
        );
        assert_eq!(resolve(&[Global], &ks("cmd-f"), None), Resolution::None);
        assert_eq!(
            resolve(&[Manager, Global], &ks("cmd-f"), None),
            Resolution::Command(Command::Search)
        );
        assert_eq!(
            resolve(&[Global], &ks("cmd-."), None),
            Resolution::Command(Command::Interrupt)
        );
        assert_eq!(resolve(&[Global], &ks("cmd-i"), None), Resolution::None);
        // ⌘⇧] arrives as either "]" with shift or "}" without.
        assert_eq!(
            resolve(&[Global], &ks("cmd-shift-]"), None),
            Resolution::Command(Command::NextTab)
        );
        assert_eq!(
            resolve(&[Global], &ks("cmd-}"), None),
            Resolution::Command(Command::NextTab)
        );
        // …and, from a browser, as "}" with shift still reported.
        assert_eq!(
            resolve(&[Global], &Keystroke::new("}", "meta+shift"), None),
            Resolution::Command(Command::NextTab)
        );
        assert_eq!(
            resolve(&[Global], &Keystroke::new("{", "meta+shift"), None),
            Resolution::Command(Command::PrevTab)
        );
        // A chord with shift never matches the unshifted key.
        assert_eq!(
            resolve(&[Manager, Global], &Keystroke::new("Tab", ""), None),
            Resolution::Command(Command::NextPage)
        );
        assert_eq!(
            resolve(&[Manager, Global], &Keystroke::new("Tab", "shift"), None),
            Resolution::Command(Command::PrevPage)
        );
    }

    #[test]
    fn web_key_names_resolve() {
        let manager = [Sessions, Manager, Global];
        assert_eq!(
            resolve(&manager, &Keystroke::new("ArrowDown", ""), None),
            Resolution::Command(Command::Down)
        );
        assert_eq!(
            resolve(&manager, &Keystroke::new("Enter", ""), None),
            Resolution::Command(Command::Open)
        );
        assert_eq!(
            resolve(&manager, &Keystroke::new("Escape", ""), None),
            Resolution::Command(Command::ClearFilter)
        );
        assert_eq!(
            resolve(&manager, &Keystroke::new("P", "meta+shift"), None),
            Resolution::Command(Command::Palette)
        );
        assert_eq!(
            resolve(&manager, &Keystroke::new("/", ""), None),
            Resolution::Command(Command::Search)
        );
        assert_eq!(
            resolve(&manager, &Keystroke::new(":", "shift"), None),
            Resolution::Command(Command::Palette)
        );
        assert_eq!(
            resolve(&manager, &Keystroke::new("d", "ctrl"), None),
            Resolution::Command(Command::HalfDown)
        );
        assert_eq!(
            resolve(&manager, &Keystroke::new(" ", ""), None),
            Resolution::None
        );
        assert_eq!(
            resolve(&[Rail, Global], &Keystroke::new("ArrowRight", ""), None),
            Resolution::Command(Command::RailDown)
        );
        assert_eq!(
            resolve(&[Rail, Global], &Keystroke::new("Escape", ""), None),
            Resolution::Command(Command::ExitNav)
        );
        assert_eq!(
            resolve(&[Disconnected], &Keystroke::new("Enter", ""), None),
            Resolution::Command(Command::ReconnectTab)
        );
    }

    #[test]
    fn every_binding_resolves_to_its_command_in_its_scope() {
        for binding in BINDINGS {
            let keys: Vec<Keystroke> = binding
                .keys
                .split(' ')
                .filter(|k| !k.is_empty())
                .map(Keystroke::parse)
                .collect();
            let scopes = [binding.scope];
            match keys.as_slice() {
                [only] => assert_eq!(
                    resolve(&scopes, only, None),
                    Resolution::Command(binding.command),
                    "{:?} {}",
                    binding.scope,
                    binding.keys
                ),
                [first, second] => {
                    assert_eq!(
                        resolve(&scopes, first, None),
                        Resolution::Pending,
                        "{:?} {}",
                        binding.scope,
                        binding.keys
                    );
                    assert_eq!(
                        resolve(&scopes, second, Some(first)),
                        Resolution::Command(binding.command),
                        "{:?} {}",
                        binding.scope,
                        binding.keys
                    );
                }
                _ => panic!("{} is not a one- or two-key binding", binding.keys),
            }
        }
    }

    #[test]
    fn command_digits_select_tabs() {
        for n in 0..10 {
            let chord = format!("cmd-{n}");
            assert_eq!(
                resolve(&[Sessions, Manager, Global], &ks(&chord), None),
                Resolution::Command(Command::SelectTab(n)),
                "{chord}"
            );
            assert_eq!(
                resolve(&[Rail, Global], &Keystroke::new(&n.to_string(), ""), None),
                Resolution::Command(Command::SelectTab(n))
            );
        }
        // Bare digits on the manager go to pages, not tabs.
        assert_eq!(
            resolve(&[Manager, Global], &Keystroke::new("2", ""), None),
            Resolution::Command(Command::Page(1))
        );
        assert_eq!(
            resolve(&[Manager, Global], &Keystroke::new("5", ""), None),
            Resolution::None
        );
    }

    #[test]
    fn hints_come_from_the_same_table() {
        assert_eq!(hint(Command::Palette, &[Global]), "⌘P");
        assert_eq!(hints(Command::Down, &[Manager]), "j · ↓");
        assert_eq!(hint(Command::First, &[Manager]), "g g");
        assert_eq!(hint(Command::HalfDown, &[Manager]), "⌃D");
        assert_eq!(hint(Command::Track, &[Sessions, Manager, Global]), "a");
        assert!(hint(Command::Themes, &[Global]).is_empty());
        assert_eq!(hint(Command::QuickLaunch, &[Global]), "⇧⌘N");
        assert_eq!(hint(Command::EnterNav, &[Global]), "⇧⌘E");
        assert_eq!(hint(Command::Undo, &[Global]), "⌘Z");
        assert!(hint(Command::Interrupt, &[Sessions, Manager]).is_empty());
        assert_eq!(hint(Command::Interrupt, &[Sessions, Manager, Global]), "⌘.");
    }

    #[test]
    fn hints_match_the_inventory() {
        let g = [Global];
        let m = [Sessions, Manager, Global];
        assert_eq!(hints(Command::Palette, &g), "⌘P · ⇧⌘P");
        assert_eq!(hint(Command::PullRequests, &g), "⌥⌘P");
        assert_eq!(hint(Command::Conversations, &g), "⇧⌘F");
        assert_eq!(hint(Command::New, &g), "⌘N");
        assert_eq!(hint(Command::NewWindow, &g), "⌥⌘N");
        assert_eq!(hint(Command::CloseTab, &g), "⌘W");
        assert_eq!(hint(Command::CloseWindow, &g), "⇧⌘W");
        assert_eq!(hint(Command::Refresh, &g), "⌘R");
        assert_eq!(hint(Command::NextTab, &g), "⇧⌘]");
        assert_eq!(hint(Command::PrevTab, &g), "⇧⌘[");
        assert_eq!(hint(Command::SelectTab(0), &g), "⌘0");
        assert_eq!(hint(Command::SelectTab(7), &g), "⌘7");
        assert_eq!(hint(Command::ToggleTranscript, &g), "⌥⌘T");
        assert_eq!(hint(Command::ReopenClosedTab, &g), "⇧⌘T");
        assert_eq!(hint(Command::ClearScrollback, &g), "⌘K");
        assert_eq!(hint(Command::Settings, &g), "⌘,");
        assert_eq!(hint(Command::Help, &g), "⌘/");
        assert_eq!(hint(Command::Quit, &g), "⌘Q");
        assert_eq!(hints(Command::Search, &m), "⌘F · /");
        assert_eq!(hints(Command::Up, &m), "k · ↑");
        assert_eq!(hint(Command::Last, &m), "G");
        assert_eq!(hint(Command::HalfUp, &m), "⌃U");
        assert_eq!(hints(Command::PrevPage, &m), "h · ← · ⇧Tab");
        assert_eq!(hints(Command::NextPage, &m), "l · → · Tab");
        assert_eq!(hint(Command::Open, &m), "Enter");
        assert_eq!(hint(Command::ClearFilter, &m), "Esc");
        assert_eq!(hint(Command::Page(2), &m), "3");
        assert_eq!(hints(Command::Help, &m), "? · ⌘/");
        assert_eq!(hint(Command::Archive, &m), "A");
        assert_eq!(hint(Command::ToggleHistory, &m), "H");
        assert_eq!(hints(Command::RailDown, &[Rail, Global]), "j · ↓ · l · →");
        assert_eq!(hints(Command::ExitNav, &[Rail, Global]), "Enter · i · Esc");
        assert_eq!(hint(Command::Edit, &[Rail, Global]), "e");
        assert_eq!(hints(Command::ReconnectTab, &[Disconnected]), "r · Enter");
        // The same command shows the innermost scope's key first.
        assert_eq!(hints(Command::CloseTab, &[Rail, Global]), "x · ⌘W");
        assert_eq!(hint(Command::Refresh, &m), "r");
    }

    #[test]
    fn help_lists_every_scope_with_merged_keys() {
        let sections = help_sections();
        let manager = sections
            .iter()
            .find(|(title, _)| *title == "Manager")
            .unwrap();
        let down = manager.1.iter().find(|(_, l)| l == "Next row").unwrap();
        assert_eq!(down.0, "j · ↓");
        assert!(sections.iter().any(|(t, _)| *t == Info.title()));
        assert_eq!(sections.len(), Scope::HELP.len());
        assert_eq!(sections.last().unwrap().1.len(), INFO.len());
        // Rows keep the table's order and merge by label.
        let global = &sections[0].1;
        assert_eq!(
            global[0],
            ("⌘P · ⇧⌘P".to_string(), "Command palette".to_string())
        );
        assert_eq!(global[1].0, "⌥⌘P");
        assert_eq!(
            help_rows(Rail)[0],
            ("j · ↓ · l · →".to_string(), "Next rail row".to_string())
        );
        assert_eq!(help_rows(Info)[0].0, "Ctrl+C");
        let sessions = help_sections_for(&[Sessions]);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].0, "Sessions");
        assert_eq!(
            sessions[0].1[0],
            ("v".to_string(), "Attach read-only".to_string())
        );
    }

    #[test]
    fn every_command_with_a_key_has_a_label() {
        for binding in BINDINGS {
            assert!(
                !crate::palette::label(binding.command).is_empty(),
                "{:?} has no label",
                binding.command
            );
        }
    }

    #[test]
    fn pages_map_to_their_scopes() {
        assert_eq!(Scope::for_page(Page::Sessions), Sessions);
        assert_eq!(Scope::for_page(Page::Machines), Machines);
        assert_eq!(Scope::for_page(Page::Accounts), Accounts);
    }
}
