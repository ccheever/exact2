//! The shape shared by every list-with-a-query overlay (picker.rs) and by
//! every popup menu (popup.rs): typing goes to the query, the selection
//! wraps, and the same list keys move it everywhere, like Zed's `Picker`
//! and `ContextMenu`. Pure state: the contract sends key names, this
//! answers what they meant. The row and item builders at the bottom are the
//! `PICKER_ROW`, `MENU_ITEM`, `POPUP`, `FIELD` and `BUTTON` JSON the launch
//! and quick launch views share.

use serde_json::{json, Value as Json};

/// The modifiers held with a key, parsed from the contract's `meta+shift`
/// text (GPUI's `Modifiers` without `function`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    /// ⌘ (GPUI `platform`).
    pub meta: bool,
    /// ⇧.
    pub shift: bool,
    /// ⌥.
    pub alt: bool,
    /// ⌃ (GPUI `control`).
    pub ctrl: bool,
}

impl Mods {
    /// No modifier.
    pub const NONE: Mods = Mods {
        meta: false,
        shift: false,
        alt: false,
        ctrl: false,
    };
    /// ⌃ alone.
    pub const CTRL: Mods = Mods {
        ctrl: true,
        ..Mods::NONE
    };
    /// ⇧ alone.
    pub const SHIFT: Mods = Mods {
        shift: true,
        ..Mods::NONE
    };
    /// ⌘ alone.
    pub const META: Mods = Mods {
        meta: true,
        ..Mods::NONE
    };

    /// `meta+shift`, `ctrl`, `cmd-k`'s `cmd`: any spelling the host may use.
    pub fn parse(text: &str) -> Mods {
        let mut m = Mods::NONE;
        for part in text.split(['+', '-', ' ']) {
            match part.trim().to_ascii_lowercase().as_str() {
                "meta" | "cmd" | "command" | "platform" | "super" => m.meta = true,
                "shift" => m.shift = true,
                "alt" | "option" => m.alt = true,
                "ctrl" | "control" => m.ctrl = true,
                _ => {}
            }
        }
        m
    }
}

/// The key's canonical name: the web's `KeyboardEvent.key` names and GPUI's
/// both become GPUI's lowercase set (`down`, `space`, `escape`, …), while a
/// printable character keeps its case.
pub fn canon(key: &str) -> String {
    match key {
        " " | "Space" | "space" => "space".into(),
        "ArrowDown" | "Down" | "down" => "down".into(),
        "ArrowUp" | "Up" | "up" => "up".into(),
        "ArrowLeft" | "Left" | "left" => "left".into(),
        "ArrowRight" | "Right" | "right" => "right".into(),
        "Escape" | "Esc" | "escape" => "escape".into(),
        "Enter" | "Return" | "enter" => "enter".into(),
        "Tab" | "tab" => "tab".into(),
        "Backspace" | "backspace" => "backspace".into(),
        "Delete" | "delete" => "delete".into(),
        "Home" | "home" => "home".into(),
        "End" | "end" => "end".into(),
        "PageUp" | "pageup" => "pageup".into(),
        "PageDown" | "pagedown" => "pagedown".into(),
        other => other.to_string(),
    }
}

/// Printable text for a key without command/control modifiers (input.rs
/// `typed`): a single character (⇧ upper-cases it), or a space.
pub fn typed(key: &str, mods: &Mods) -> Option<String> {
    if mods.meta || mods.ctrl {
        return None;
    }
    let key = canon(key);
    if key == "space" {
        return Some(" ".into());
    }
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() || c.is_control() || mods.alt {
        return None;
    }
    Some(if mods.shift { key.to_uppercase() } else { key })
}

/// `index + delta` modulo `count`, or 0 for an empty list.
pub fn wrapped(index: usize, delta: i64, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        (index as i64 + delta).rem_euclid(count as i64) as usize
    }
}

/// What a key means to a list (picker.rs `PickerKey`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKey {
    /// Enter.
    Confirm,
    /// Esc or ⌃C.
    Cancel,
    /// ↓, Tab, ⌃N, ⌃J.
    Next,
    /// ↑, ⇧Tab, ⌃P, ⌃K.
    Prev,
    /// Home, PgUp, ⌘↑.
    First,
    /// End, PgDn, ⌘↓.
    Last,
    /// Not a list key.
    Other,
}

/// The list keys every picker and menu shares: arrows, Tab, Ctrl+N / Ctrl+P
/// (and Ctrl+J / Ctrl+K), Enter and Esc. Bare letters are never list keys
/// here because the query is being typed into.
pub fn list_key(key: &str, m: &Mods) -> ListKey {
    let key = canon(key);
    if m.meta || m.alt {
        return match key.as_str() {
            "up" if m.meta => ListKey::First,
            "down" if m.meta => ListKey::Last,
            _ => ListKey::Other,
        };
    }
    if m.ctrl {
        return match key.as_str() {
            "n" | "j" => ListKey::Next,
            "p" | "k" => ListKey::Prev,
            "c" => ListKey::Cancel,
            _ => ListKey::Other,
        };
    }
    match key.as_str() {
        "enter" => ListKey::Confirm,
        "escape" => ListKey::Cancel,
        "down" => ListKey::Next,
        "up" => ListKey::Prev,
        "tab" if m.shift => ListKey::Prev,
        "tab" => ListKey::Next,
        "home" | "pageup" => ListKey::First,
        "end" | "pagedown" => ListKey::Last,
        _ => ListKey::Other,
    }
}

/// What a picker did with a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerKey {
    /// The highlight moved.
    Moved,
    /// Enter: the highlighted row.
    Chosen,
    /// Esc.
    Closed,
    /// Printable text while the query is being typed (the host's input
    /// already holds it; `Event::Input` brings the whole value).
    Typed(String),
    /// Nothing the list cares about.
    None,
}

/// A list with a query and a wrapping highlight.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PickerState {
    /// The filter text.
    pub query: String,
    /// The highlighted row.
    pub index: usize,
    /// The query has the keyboard, so bare letters type instead of moving.
    pub typing: bool,
}

impl PickerState {
    /// An empty query with the first row highlighted, typing.
    pub fn new() -> Self {
        PickerState {
            typing: true,
            ..Default::default()
        }
    }

    /// Highlight `index` (clamped).
    pub fn select(&mut self, index: usize, count: usize) {
        self.index = if count == 0 { 0 } else { index.min(count - 1) };
    }

    /// Move the highlight by `delta`, wrapping at either end.
    pub fn step(&mut self, delta: i64, count: usize) {
        let next = wrapped(self.index, delta, count);
        self.select(next, count);
    }

    /// One key on a list of `len` rows: the shared list keys, plus j / k /
    /// G when the query is not being typed into.
    pub fn key(&mut self, key: &str, mods: &Mods, len: usize) -> PickerKey {
        match list_key(key, mods) {
            ListKey::Confirm => return PickerKey::Chosen,
            ListKey::Cancel => return PickerKey::Closed,
            ListKey::Next => self.step(1, len),
            ListKey::Prev => self.step(-1, len),
            ListKey::First => self.select(0, len),
            ListKey::Last => self.select(len.saturating_sub(1), len),
            ListKey::Other => {
                let text = typed(key, mods);
                if self.typing {
                    return text.map(PickerKey::Typed).unwrap_or(PickerKey::None);
                }
                match text.as_deref() {
                    Some("j") => self.step(1, len),
                    Some("k") => self.step(-1, len),
                    Some("G") => self.select(len.saturating_sub(1), len),
                    _ => return PickerKey::None,
                }
            }
        }
        PickerKey::Moved
    }
}

/// Which popup is open (popup.rs `PopupKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupKind {
    /// The "⋯" / right-click menu of the manager row at this index.
    RowMenu(usize),
    /// The right-click menu of the rail tab at this position.
    TabMenu(usize),
    /// The Sessions header's "View" filters.
    ViewMenu,
    /// The dropdown of the form field at this index.
    FieldChoice(usize),
    /// Retired: the launch dialog's account chip. The composer's menus open
    /// inside its card now (`launch::Menu`); nothing opens this, and it goes
    /// once model/menus.rs stops naming it.
    LaunchAccount,
    /// Retired with `LaunchAccount`: the launch dialog's permissions chip.
    LaunchPermissions,
}

/// One popup at a time, with its highlight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Popup {
    /// Which one.
    pub kind: PopupKind,
    /// The highlighted item.
    pub index: usize,
    /// Where a right-click menu opens; `None` anchors under its button.
    pub at: Option<(f64, f64)>,
}

impl Popup {
    /// Open `kind` on `items`' current value.
    pub fn open(kind: PopupKind, items: &[PopupItem], at: Option<(f64, f64)>) -> Popup {
        Popup {
            kind,
            index: initial_index(items),
            at,
        }
    }

    /// The menu keys: ↑ ↓ (and j / k), Enter, Esc, over `len` items.
    pub fn key(&mut self, key: &str, mods: &Mods, len: usize) -> PickerKey {
        let mut list = PickerState {
            index: self.index,
            ..Default::default()
        };
        let outcome = list.key(key, mods, len);
        self.index = list.index;
        outcome
    }
}

/// What a rail tab's menu item does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabAction {
    /// Rename the tab.
    Rename,
    /// Reconnect its session.
    Reconnect,
    /// Close it.
    Close,
    /// Move it.
    Move,
    /// Collapse the folder.
    Collapse,
    /// New folder.
    NewFolder,
}

/// What a popup item does when chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PopupAction {
    /// A palette command, by its id.
    Command(String),
    /// A rail tab action.
    Tab(usize, TabAction),
    /// Set the form field at this index to the value.
    Choice(usize, String),
    /// Retired with `PopupKind::LaunchAccount`: set the launch account.
    LaunchAccount(String),
    /// Retired with `PopupKind::LaunchPermissions`: set the launch mode.
    LaunchPermissions(String),
}

/// One row of a popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopupItem {
    /// The text.
    pub label: String,
    /// Key hint shown at the right, from the keymap; empty when unbound.
    pub hint: String,
    /// The Lucide icon, when the menu has icons.
    pub icon: Option<&'static str>,
    /// `Some(true)` draws a check mark; `Some(false)` leaves room for one.
    pub checked: Option<bool>,
    /// What choosing it does.
    pub action: PopupAction,
}

impl PopupItem {
    /// A command row with an icon.
    pub fn command(
        label: impl Into<String>,
        hint: String,
        icon: &'static str,
        command: &str,
    ) -> Self {
        PopupItem {
            label: label.into(),
            hint,
            icon: Some(icon),
            checked: None,
            action: PopupAction::Command(command.into()),
        }
    }

    /// The same item with this key hint.
    pub fn with_hint(mut self, hint: String) -> Self {
        self.hint = hint;
        self
    }

    /// The same item doing something else.
    pub fn with_action(mut self, action: PopupAction) -> Self {
        self.action = action;
        self
    }

    /// A checkable row.
    pub fn choice(label: impl Into<String>, checked: bool, action: PopupAction) -> Self {
        PopupItem {
            label: label.into(),
            hint: String::new(),
            icon: None,
            checked: Some(checked),
            action,
        }
    }
}

/// The item to highlight when a popup opens: the checked one, else the first.
pub fn initial_index(items: &[PopupItem]) -> usize {
    items
        .iter()
        .position(|item| item.checked == Some(true))
        .unwrap_or(0)
}

/// Subsequence match with a small bonus for word starts; lower is better
/// (palette.rs `fuzzy_score`).
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query
        .chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| !c.is_whitespace())
        .collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut score = 0;
    let mut ti = 0;
    let mut last: Option<usize> = None;
    for qc in q {
        let at = t[ti..].iter().position(|c| *c == qc)? + ti;
        let word_start = at == 0 || !t[at - 1].is_alphanumeric();
        score += match last {
            Some(prev) if at == prev + 1 => 0,
            _ if word_start => 1,
            Some(prev) => (at - prev) as i32 + 2,
            None => at as i32 + 2,
        };
        last = Some(at);
        ti = at + 1;
    }
    Some(score)
}

/// A `PICKER_ROW` with every field, the rest zero.
pub fn row(id: &str, chip: &str, label: &str, detail: &str, hint: &str, selected: bool) -> Json {
    json!({
        "id": id, "chip": chip, "label": label, "detail": detail, "hint": hint, "second": "",
        "selected": selected, "swatch": "", "swatchBorder": "", "accent": false,
    })
}

/// A `MENU_ITEM` for a popup row.
pub fn item(item: &PopupItem, id: &str, selected: bool) -> Json {
    json!({
        "id": id,
        "label": item.label,
        "icon": item.icon.unwrap_or(""),
        "hint": item.hint,
        "checked": item.checked == Some(true),
        "checkable": item.checked.is_some(),
        "selected": selected,
        "disabled": false,
    })
}

/// A `POPUP`.
pub fn popup(
    kind: &str,
    anchor: &str,
    at: Option<(f64, f64)>,
    title: &str,
    items: Vec<Json>,
    empty: &str,
) -> Json {
    let (x, y) = at.unwrap_or((0.0, 0.0));
    json!({ "kind": kind, "anchor": anchor, "x": x, "y": y, "title": title, "items": items, "empty": empty })
}

/// A `FIELD`.
pub fn field(
    id: &str,
    label: &str,
    value: &str,
    placeholder: &str,
    kind: &str,
    focused: bool,
) -> Json {
    json!({
        "id": id, "label": label, "value": value, "placeholder": placeholder, "kind": kind,
        "focused": focused, "options": [], "hint": "", "multiline": false, "suggestions": [],
    })
}

/// A `BUTTON`.
pub fn button(id: &str, label: &str, hint: &str, primary: bool) -> Json {
    json!({ "id": id, "label": label, "hint": hint, "primary": primary, "disabled": false })
}

/// Set one key of a JSON object in place.
pub fn set(target: &mut Json, key: &str, value: Json) {
    if let Some(map) = target.as_object_mut() {
        map.insert(key.into(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_wraps_at_both_ends() {
        assert_eq!(wrapped(0, -1, 5), 4);
        assert_eq!(wrapped(4, 1, 5), 0);
        assert_eq!(wrapped(2, 1, 5), 3);
        assert_eq!(wrapped(3, -1, 0), 0);
    }

    #[test]
    fn list_keys_are_the_shared_set() {
        let k = |key: &str, mods: &str| list_key(key, &Mods::parse(mods));
        assert_eq!(k("ArrowDown", ""), ListKey::Next);
        assert_eq!(k("n", "ctrl"), ListKey::Next);
        assert_eq!(k("j", "ctrl"), ListKey::Next);
        assert_eq!(k("Tab", ""), ListKey::Next);
        assert_eq!(k("Tab", "shift"), ListKey::Prev);
        assert_eq!(k("p", "ctrl"), ListKey::Prev);
        assert_eq!(k("Enter", ""), ListKey::Confirm);
        assert_eq!(k("Escape", ""), ListKey::Cancel);
        assert_eq!(k("ArrowUp", "meta"), ListKey::First);
        assert_eq!(k("PageDown", ""), ListKey::Last);
        assert_eq!(k("Home", ""), ListKey::First);
        assert_eq!(k("j", ""), ListKey::Other);
        assert_eq!(k("a", "meta"), ListKey::Other);
    }

    #[test]
    fn typed_text_follows_the_modifiers() {
        assert_eq!(typed("j", &Mods::NONE).as_deref(), Some("j"));
        assert_eq!(typed("j", &Mods::SHIFT).as_deref(), Some("J"));
        assert_eq!(typed(" ", &Mods::NONE).as_deref(), Some(" "));
        assert_eq!(typed("j", &Mods::CTRL), None);
        assert_eq!(typed("Enter", &Mods::NONE), None);
        assert_eq!(typed("/", &Mods::NONE).as_deref(), Some("/"));
    }

    #[test]
    fn stepping_clamps_and_wraps() {
        let mut p = PickerState::new();
        p.step(-1, 3);
        assert_eq!(p.index, 2);
        p.step(1, 3);
        assert_eq!(p.index, 0);
        p.select(10, 3);
        assert_eq!(p.index, 2);
        p.select(1, 0);
        assert_eq!(p.index, 0);
    }

    #[test]
    fn keys_move_choose_close_or_type() {
        let mut p = PickerState::new();
        assert_eq!(p.key("ArrowDown", &Mods::NONE, 3), PickerKey::Moved);
        assert_eq!(p.index, 1);
        assert_eq!(p.key("j", &Mods::NONE, 3), PickerKey::Typed("j".into()));
        assert_eq!(p.index, 1);
        p.typing = false;
        assert_eq!(p.key("j", &Mods::NONE, 3), PickerKey::Moved);
        assert_eq!(p.index, 2);
        assert_eq!(p.key("G", &Mods::NONE, 3), PickerKey::Moved);
        assert_eq!(p.key("k", &Mods::NONE, 3), PickerKey::Moved);
        assert_eq!(p.index, 1);
        assert_eq!(p.key("Enter", &Mods::NONE, 3), PickerKey::Chosen);
        assert_eq!(p.key("Escape", &Mods::NONE, 3), PickerKey::Closed);
        assert_eq!(p.key("q", &Mods::NONE, 3), PickerKey::None);
        assert_eq!(p.key("ArrowDown", &Mods::META, 3), PickerKey::Moved);
        assert_eq!(p.index, 2);
    }

    #[test]
    fn a_popup_opens_on_its_current_value() {
        let items = vec![
            PopupItem::choice("a", false, PopupAction::Choice(0, "a".into())),
            PopupItem::choice("b", true, PopupAction::Choice(0, "b".into())),
        ];
        assert_eq!(initial_index(&items), 1);
        let none = vec![PopupItem::command("x", String::new(), "tag", "edit")];
        assert_eq!(initial_index(&none), 0);
        let mut popup = Popup::open(PopupKind::FieldChoice(0), &items, None);
        assert_eq!(popup.index, 1);
        assert_eq!(popup.key("ArrowDown", &Mods::NONE, 2), PickerKey::Moved);
        assert_eq!(popup.index, 0);
    }

    #[test]
    fn fuzzy_prefers_contiguous_and_word_starts() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
        assert!(fuzzy_score("xyz", "attach").is_none());
        let exact = fuzzy_score("attach", "Attach to session").unwrap();
        let spread = fuzzy_score("ats", "Attach to session").unwrap();
        assert!(exact <= spread);
        assert!(
            fuzzy_score("ls", "Launch session").unwrap()
                < fuzzy_score("ls", "Toggle history").unwrap_or(i32::MAX)
        );
    }
}
