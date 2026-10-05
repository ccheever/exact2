//! A text field's selection (x2apps codeedit #2): the start, end and
//! direction HTML gives an `input` or a `textarea`, in UTF-16 units of its
//! value. Typing replaces it and leaves a caret after the text, the arrows
//! move or (with Shift) extend it, `setSelectionRange` and `select()` set it
//! without moving the focus; each `input` and `change` reports it, and the
//! field's `select` fires as the person or a command makes one.
use super::*;
use exact_runner::{ControlValue, FieldSelection, SelectionDirection};

/// A field's selection as this host keeps it, beside the value it indexes.
#[derive(Debug, Clone)]
pub(crate) struct FieldMark {
    selection: FieldSelection,
    /// The value the selection was made in: one rewritten since (an action
    /// that writes another) puts the caret at its end, as HTML's `value`
    /// setter does, and an offset never outlives the text it counted.
    value: String,
    /// Set by `setSelectionRange` or `select()`: a command that sets that
    /// same selection again fires no `select`.
    by_command: bool,
}

/// A string's length in UTF-16 code units, the DOM's offsets' unit.
pub(super) fn utf16_len(text: &str) -> u32 {
    text.encode_utf16().count().try_into().unwrap_or(u32::MAX)
}

/// The byte index of UTF-16 offset `at` in `text`: clamped to its end, and
/// inside a surrogate pair the character's start.
pub(super) fn byte_at(text: &str, at: u32) -> usize {
    let mut units = 0u32;
    for (i, c) in text.char_indices() {
        units += c.len_utf16() as u32;
        if units > at {
            return i;
        }
    }
    text.len()
}

/// `text` with `selection` replaced by `with`, and the caret after it.
pub(super) fn replace(
    text: &str,
    selection: FieldSelection,
    with: &str,
) -> (String, FieldSelection) {
    let (a, b) = (byte_at(text, selection.start), byte_at(text, selection.end));
    let next = format!("{}{with}{}", &text[..a], &text[b..]);
    let caret = utf16_len(&text[..a]) + utf16_len(with);
    (next, FieldSelection::caret(caret))
}

/// Backspace (`forward`: Delete): the selection, else the character before
/// (after) the caret; `None` when there is nothing to delete.
pub(super) fn delete(
    text: &str,
    selection: FieldSelection,
    forward: bool,
) -> Option<(String, FieldSelection)> {
    let mut s = selection;
    if s.start == s.end {
        let other = step(text, s.start, forward);
        if other == s.start {
            return None;
        }
        (s.start, s.end) = (s.start.min(other), s.start.max(other));
    }
    Some(replace(text, s, ""))
}

/// The offset one character from `at`, toward the end when `forward`.
fn step(text: &str, at: u32, forward: bool) -> u32 {
    let i = byte_at(text, at);
    let at = utf16_len(&text[..i]);
    let c = if forward {
        text[i..].chars().next()
    } else {
        text[..i].chars().next_back()
    };
    match c {
        Some(c) if forward => at + c.len_utf16() as u32,
        Some(c) => at - c.len_utf16() as u32,
        None => at,
    }
}

/// The start (`end`: the end) of the line `at` is on: a textarea's line
/// between its newlines, an input's whole value.
fn line_edge(text: &str, at: u32, end: bool, textarea: bool) -> u32 {
    if !textarea {
        return if end { utf16_len(text) } else { 0 };
    }
    let i = byte_at(text, at);
    if end {
        utf16_len(&text[..text[i..].find('\n').map_or(text.len(), |n| i + n)])
    } else {
        utf16_len(&text[..text[..i].rfind('\n').map_or(0, |n| n + 1)])
    }
}

/// Where ArrowLeft, ArrowRight, Home or End leaves `selection`, as browsers
/// move it: a plain arrow collapses a selection to its start or end, else
/// moves the caret a character; with `extend` (Shift) the selection's
/// moving end goes, `backward` when it is then before the end that stays.
/// `None` for another key.
pub(super) fn moved(
    text: &str,
    selection: FieldSelection,
    key: &str,
    extend: bool,
    textarea: bool,
) -> Option<FieldSelection> {
    let s = selection;
    let (anchor, focus) = match s.direction {
        SelectionDirection::Backward => (s.end, s.start),
        _ => (s.start, s.end),
    };
    let to = |at: u32| match key {
        "ArrowLeft" => Some(step(text, at, false)),
        "ArrowRight" => Some(step(text, at, true)),
        "Home" => Some(line_edge(text, at, false, textarea)),
        "End" => Some(line_edge(text, at, true, textarea)),
        _ => None,
    };
    if !extend {
        return Some(FieldSelection::caret(match key {
            "ArrowLeft" if s.start != s.end => s.start,
            "ArrowRight" if s.start != s.end => s.end,
            _ => to(focus)?,
        }));
    }
    let focus = to(focus)?;
    Some(FieldSelection {
        start: anchor.min(focus),
        end: anchor.max(focus),
        direction: match focus.cmp(&anchor) {
            std::cmp::Ordering::Less => SelectionDirection::Backward,
            std::cmp::Ordering::Greater => SelectionDirection::Forward,
            std::cmp::Ordering::Equal => SelectionDirection::None,
        },
    })
}

impl<D: DataSource> Presenter<D> {
    /// `id`'s value as the kernel holds it.
    fn field_value(&self, id: ViewId) -> String {
        self.host
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value).map(str::to_owned))
            .unwrap_or_default()
    }

    /// A text field's selection: the one kept for its value, else a caret
    /// at its end (where a field the person has not moved in types).
    pub(crate) fn field_selection(&self, id: ViewId) -> FieldSelection {
        let value = self
            .host
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value))
            .unwrap_or("");
        match self.fields.get(&id) {
            Some(mark) if mark.value == value => mark.selection,
            _ => FieldSelection::at_end(value),
        }
    }

    /// Keep `selection` for `id` while its value is `value`; marks of nodes
    /// gone are dropped.
    pub(crate) fn mark_field(
        &mut self,
        id: ViewId,
        value: &str,
        selection: FieldSelection,
        by_command: bool,
    ) {
        let kernel = self.host.kernel();
        self.fields.retain(|id, _| kernel.node(*id).is_some());
        self.fields.insert(
            id,
            FieldMark {
                selection,
                value: value.to_owned(),
                by_command,
            },
        );
        self.dirty = true;
    }

    /// A press puts the caret at a text field's end: this host finds no
    /// character under the pointer, and typing there appends as before.
    pub(crate) fn press_field(&mut self, id: ViewId) {
        if self
            .host
            .kernel()
            .node(id)
            .is_some_and(|n| n.node_type == NodeType::TextInput)
        {
            let value = self.field_value(id);
            self.mark_field(id, &value, FieldSelection::at_end(&value), false);
        }
    }

    /// HTML's `select` at a text field, where it has a handler.
    fn field_select(&mut self, id: ViewId, value: &str, selection: FieldSelection, now_ms: f64) {
        if !self
            .host
            .runner()
            .handlers_of(id)
            .contains(&EventKind::Select)
        {
            return;
        }
        let event = Event::FieldSelect(value.to_owned(), selection);
        if let Some(e) = self.host.dispatch_at(id, event, now_ms) {
            eprintln!("exact: {e}");
        }
        if let Some(e) = self.after_commit() {
            eprintln!("exact: {e}");
        }
    }

    /// A command's selection (`setSelectionRange`, `select()`): kept, the
    /// focus left where it is, and `select` fired unless a command had set
    /// this same one. Chrome fires it for a range where typing left the
    /// caret (the forms fixture's collapse after typing).
    fn command_select(&mut self, id: ViewId, selection: FieldSelection) {
        let value = self.field_value(id);
        let same = self
            .fields
            .get(&id)
            .is_some_and(|m| m.by_command && m.value == value && m.selection == selection);
        self.mark_field(id, &value, selection, true);
        if !same {
            let now = self.host.now();
            self.field_select(id, &value, selection, now);
        }
    }

    /// The live text field whose `id` is `name`, or why there is none.
    fn text_field(&self, name: &str) -> Result<ViewId, &'static str> {
        let kernel = self.host.kernel();
        let id = kernel
            .rows(None)
            .unwrap_or_default()
            .into_iter()
            .map(|row| row.id)
            .find(|&id| {
                kernel
                    .node(id)
                    .is_some_and(|n| n.props.str(PropId::Id) == Some(name))
            })
            .ok_or("no live node with that id")?;
        match kernel.node(id) {
            Some(n) if n.node_type == NodeType::TextInput => Ok(id),
            _ => Err("not a text field"),
        }
    }

    /// `setSelectionRange(id, start, end[, direction])` (x2apps codeedit
    /// #2): clamped as HTML clamps it, the focus unmoved; an unfocused field
    /// keeps it, and the next typing there edits at it.
    pub(crate) fn set_selection_range(&mut self, args: &[exact_plan::Value]) {
        let Some(name) = args.first().and_then(exact_plan::Value::as_str) else {
            eprintln!("exact: setSelectionRange requires an element id");
            return;
        };
        let id = match self.text_field(name) {
            Ok(id) => id,
            Err(reason) => {
                self.host
                    .log(format!("setSelectionRange \"{name}\" refused: {reason}"));
                return;
            }
        };
        let at = |i: usize| {
            args.get(i)
                .and_then(exact_plan::Value::as_number)
                .unwrap_or(0.0)
        };
        let direction = args
            .get(3)
            .and_then(exact_plan::Value::as_str)
            .map(SelectionDirection::of)
            .unwrap_or_default();
        let len = utf16_len(&self.field_value(id));
        self.command_select(id, FieldSelection::clamped(at(1), at(2), direction, len));
    }

    /// `selectText(id)`: focused, as `focus()` does, and its whole text
    /// selected, as `select()` does — which the next key replaces.
    pub(crate) fn select_text(&mut self, args: &[exact_plan::Value]) {
        let Some(id) = self.focus_command(args) else {
            return;
        };
        if self
            .host
            .kernel()
            .node(id)
            .is_some_and(|n| n.node_type == NodeType::TextInput)
        {
            let len = utf16_len(&self.field_value(id));
            self.command_select(
                id,
                FieldSelection {
                    start: 0,
                    end: len,
                    direction: SelectionDirection::None,
                },
            );
        }
    }

    /// An edit's new value and the caret it leaves: kept, the field marked
    /// typed into (its `change` waits for blur or Enter), and its `input`.
    fn field_edit(&mut self, id: ViewId, next: String, after: FieldSelection, now_ms: f64) {
        self.edited = Some(id);
        self.mark_field(id, &next, after, false);
        if self
            .host
            .runner()
            .handlers_of(id)
            .contains(&EventKind::Input)
        {
            let event = Event::Input(ControlValue::Field(next, after));
            if let Some(e) = self.host.dispatch_at(id, event, now_ms) {
                eprintln!("exact: {e}");
            }
            if let Some(e) = self.after_commit() {
                eprintln!("exact: {e}");
            }
        }
    }

    /// A key's default action in the focused text field: Enter submits an
    /// input or breaks a textarea's line, Backspace and Delete delete, the
    /// arrows, Home and End move the caret (Shift: extend the selection,
    /// firing `select` when it is a range), a character is typed — each
    /// edit at the selection.
    pub(crate) fn field_key(&mut self, id: ViewId, name: &str, now_ms: f64) {
        let Some(node) = self.host.kernel().node(id) else {
            return;
        };
        if node.node_type != NodeType::TextInput || node.props.bool(PropId::Editable) == Some(false)
        {
            return;
        }
        let textarea = node.props.str(PropId::SemanticTag) == Some("textarea");
        let limit = exact_kernel::control::text_maxlength(node.props);
        let value = node.props.str(PropId::Value).unwrap_or("").to_owned();
        let selection = self.field_selection(id);
        // A Control or Meta chord types nothing, as in a browser.
        let chord = self.held & 0b1100_1100 != 0;
        let (next, after) = match name {
            "Enter" if !textarea => {
                if let Some(Some(e)) = self.commit_text(id, now_ms) {
                    eprintln!("exact: {e}");
                }
                if let Some(e) = self.submit_event(id, now_ms) {
                    eprintln!("exact: {e}");
                }
                return;
            }
            "ArrowLeft" | "ArrowRight" | "Home" | "End" if !chord => {
                let extend = self.modifiers().shift;
                let Some(to) = moved(&value, selection, name, extend, textarea) else {
                    return;
                };
                self.mark_field(id, &value, to, false);
                if extend && to.start != to.end && to != selection {
                    self.field_select(id, &value, to, now_ms);
                }
                return;
            }
            "Enter" => replace(&value, selection, "\n"),
            "Backspace" | "Delete" => match delete(&value, selection, name == "Delete") {
                Some(edit) => edit,
                None => return,
            },
            s if s.chars().count() == 1 && !chord => replace(&value, selection, s),
            _ => return,
        };
        // maxlength stops typing that would lengthen past it, as HTML's does.
        if limit.is_some_and(|limit| {
            next.encode_utf16().count() > limit
                && next.encode_utf16().count() > value.encode_utf16().count()
        }) {
            return;
        }
        self.field_edit(id, next, after, now_ms);
    }

    /// A paste's default action at an editable text field: the text in at
    /// its selection (a field's own paste still inserts, beside any `paste`
    /// handler), within its `maxlength`.
    pub(crate) fn paste_field(&mut self, id: ViewId, text: &str, now_ms: f64) {
        let Some(node) = self.host.kernel().node(id) else {
            return;
        };
        if node.node_type != NodeType::TextInput
            || node.props.bool(PropId::Editable) == Some(false)
            || node.props.bool(PropId::Disabled) == Some(true)
        {
            return;
        }
        let limit = exact_kernel::control::text_maxlength(node.props);
        let value = node.props.str(PropId::Value).unwrap_or("").to_owned();
        let selection = self.field_selection(id);
        let mut text = text.to_owned();
        if let Some(limit) = limit {
            let room = (limit as u32)
                .saturating_sub(utf16_len(&value) - (selection.end - selection.start));
            text.truncate(byte_at(&text, room));
        }
        let (next, after) = replace(&value, selection, &text);
        self.field_edit(id, next, after, now_ms);
    }
}
