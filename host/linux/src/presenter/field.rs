//! A text field's selection (x2apps codeedit #2): the start, end and
//! direction HTML gives an `input` or a `textarea`, in UTF-16 units of its
//! value. Typing replaces it and leaves a caret after the text, the arrows
//! move or (with Shift) extend it, `setSelectionRange` and `select()` set it
//! without moving the focus; each `input` and `change` reports it, and the
//! field's `select` fires as the person or a command makes one.
use super::*;
use crate::text::field::Cursor;
use exact_runner::{ControlValue, FieldSelection, InputDetails, SelectionDirection};
use unicode_segmentation::UnicodeSegmentation;

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
    upstream: bool,
    preferred_x: Option<f32>,
}

/// A string's length in UTF-16 code units, the DOM's offsets' unit.
pub(super) fn utf16_len(text: &str) -> u32 {
    text.encode_utf16().count().try_into().unwrap_or(u32::MAX)
}

/// `text` cut to the room `maxlength` leaves for it in `value` with
/// `selection` replaced, as a browser cuts an insertion: the limit less the
/// code units kept outside the selection, so none in a value already over it.
pub(super) fn fit<'a>(
    text: &'a str,
    value: &str,
    selection: FieldSelection,
    limit: Option<usize>,
) -> &'a str {
    let Some(limit) = limit else { return text };
    // What `replace` removes: offsets inside a surrogate pair fall back to its start.
    let removed = utf16_len(&value[byte_at(value, selection.start)..byte_at(value, selection.end)]);
    let kept = utf16_len(value) - removed;
    let room = u32::try_from(limit)
        .unwrap_or(u32::MAX)
        .saturating_sub(kept);
    &text[..byte_at(text, room)]
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

fn toward(s: FieldSelection, focus: u32, extend: bool) -> FieldSelection {
    if !extend {
        return FieldSelection::caret(focus);
    }
    let anchor = if s.direction == SelectionDirection::Backward {
        s.end
    } else {
        s.start
    };
    FieldSelection {
        start: anchor.min(focus),
        end: anchor.max(focus),
        direction: match focus.cmp(&anchor) {
            std::cmp::Ordering::Less => SelectionDirection::Backward,
            std::cmp::Ordering::Greater => SelectionDirection::Forward,
            std::cmp::Ordering::Equal => SelectionDirection::None,
        },
    }
}

/// Linux word arrows skip whitespace, then cross one word or punctuation group.
fn word_step(text: &str, at: u32, forward: bool) -> u32 {
    let mut words: Vec<(usize, usize, bool)> = Vec::new();
    for (start, part) in text.split_word_bound_indices() {
        if part.chars().all(char::is_whitespace) {
            continue;
        }
        let punctuation = part.unicode_words().next().is_none();
        if let Some(last) = words
            .last_mut()
            .filter(|last| punctuation && last.2 && last.1 == start)
        {
            last.1 += part.len();
        } else {
            words.push((start, start + part.len(), punctuation));
        }
    }
    let byte = byte_at(text, at);
    let next = if forward {
        words
            .iter()
            .find(|w| w.1 > byte)
            .map_or(text.len(), |w| w.1)
    } else {
        words.iter().rev().find(|w| w.0 < byte).map_or(0, |w| w.0)
    };
    utf16_len(&text[..next])
}

impl<D: DataSource> Presenter<D> {
    /// What `id` shows: typed text kept against its bound value, else that
    /// value (LLP 1069.001 D4). A selection indexes this, not the prop a
    /// later write replaced.
    fn field_value(&self, id: ViewId) -> String {
        self.field_text(id)
    }

    /// Compare runtime writes against the selection before the commit. Only
    /// plain fields start at 0-0: Markdown owns its selection on the web.
    pub(super) fn sync_field_values(&mut self) {
        let changed = self.host.values.take_fields();
        let kernel = self.host.kernel();
        self.fields.retain(|id, _| kernel.node(*id).is_some());
        for key in changed {
            let Some(node) = self.host.kernel().node_by_key(key) else {
                continue;
            };
            let id = node.id;
            let markdown = node.props.str(PropId::Markup) == Some("markdown");
            let value = self.field_value(id);
            let after = FieldSelection::at_end(&value);
            let before = match self.fields.get(&id) {
                Some(mark) if mark.value == value => continue,
                Some(mark) => mark.selection,
                None if value.is_empty() => continue,
                None if markdown => after,
                None => FieldSelection::caret(0),
            };
            self.mark_field_from(id, &value, before, after, false);
        }
    }

    fn has_selection_api(&self, id: ViewId) -> bool {
        self.host.kernel().node(id).is_some_and(|node| {
            node.props.str(PropId::SemanticTag) == Some("textarea")
                || matches!(
                    node.props.str(PropId::Type).unwrap_or("text"),
                    "text" | "search" | "url" | "tel" | "password"
                )
        })
    }

    pub(super) fn reported_selection(
        &self,
        id: ViewId,
        value: &str,
        selection: FieldSelection,
    ) -> FieldSelection {
        if self.has_selection_api(id) {
            selection
        } else {
            // navigation.js uses the value's end when the DOM reports null.
            FieldSelection::at_end(value)
        }
    }

    /// The caret used to edit also exists in email and number inputs,
    /// whose DOM selection API cannot read or write it.
    pub(crate) fn field_selection(&self, id: ViewId) -> FieldSelection {
        let value = self.field_value(id);
        match self.fields.get(&id) {
            Some(mark) if mark.value == value => mark.selection,
            _ => FieldSelection::at_end(&value),
        }
    }

    /// Keep a selection against the value it indexes. Call before replacing
    /// the displayed value so the comparison sees the previous selection.
    pub(crate) fn mark_field(
        &mut self,
        id: ViewId,
        value: &str,
        selection: FieldSelection,
        by_command: bool,
    ) {
        let before = self.field_selection(id);
        self.mark_field_from(id, value, before, selection, by_command);
    }

    fn mark_field_from(
        &mut self,
        id: ViewId,
        value: &str,
        before: FieldSelection,
        selection: FieldSelection,
        by_command: bool,
    ) {
        if before != selection {
            self.field_change_tasks.insert(id);
        }
        self.fields.insert(
            id,
            FieldMark {
                selection,
                value: value.to_owned(),
                by_command,
                upstream: false,
                preferred_x: None,
            },
        );
        self.dirty = true;
        // A field the shell hosts shows a command's selection itself.
        if by_command {
            self.hosted_commanded(id, selection);
        }
    }

    /// [`Presenter::mark_field`] for a selection the shell's own field
    /// already shows (LLP 1115.001 D8): kept for the next `input`, and
    /// nothing here is painted again for it.
    pub(super) fn note_field(&mut self, id: ViewId, value: &str, selection: FieldSelection) {
        let mark = FieldMark {
            selection,
            value: value.to_owned(),
            by_command: false,
            upstream: false,
            preferred_x: None,
        };
        self.fields.insert(id, mark);
    }

    pub(crate) fn field_cursor(&self, id: ViewId) -> Cursor {
        let selection = self.field_selection(id);
        Cursor {
            offset: if selection.direction == SelectionDirection::Backward {
                selection.start
            } else {
                selection.end
            },
            upstream: self
                .fields
                .get(&id)
                .is_some_and(|m| m.value == self.field_value(id) && m.upstream),
        }
    }

    /// A press indexes the same paragraph and placement that paint the value.
    pub(crate) fn press_field(&mut self, id: ViewId, x: f32, y: f32) {
        if !self
            .host
            .kernel()
            .node(id)
            .is_some_and(|n| n.node_type == NodeType::TextInput && !crate::fields::hosts(&n))
        {
            return;
        }
        self.boxes();
        let (x, y) = self
            .boxes
            .iter()
            .find(|b| b.id == id)
            .map_or((x, y), |b| b.field_point(x, y));
        if let Some(cursor) = self.brush.field_text.get(&id).and_then(|f| f.hit(x, y)) {
            let value = self.field_value(id);
            self.mark_field(id, &value, FieldSelection::caret(cursor.offset), false);
            if let Some(mark) = self.fields.get_mut(&id) {
                mark.upstream = cursor.upstream;
            }
        }
    }

    fn move_field(
        &mut self,
        id: ViewId,
        key: &str,
        value: &str,
        selection: FieldSelection,
        textarea: bool,
    ) {
        let modifiers = self.modifiers();
        let cursor = self.field_cursor(id);
        let mut preferred_x = None;
        let (to, upstream) = if modifiers.ctrl {
            let at = word_step(value, cursor.offset, key == "ArrowRight");
            (toward(selection, at, modifiers.shift), false)
        } else if textarea && matches!(key, "ArrowUp" | "ArrowDown" | "Home" | "End") {
            self.boxes();
            let Some(field) = self.brush.field_text.get(&id) else {
                return;
            };
            let to = if matches!(key, "ArrowUp" | "ArrowDown") {
                let x = self
                    .fields
                    .get(&id)
                    .and_then(|m| m.preferred_x)
                    .unwrap_or_else(|| field.geometry.caret(cursor).0);
                preferred_x = Some(x);
                field.geometry.vertical(cursor, x, key == "ArrowDown")
            } else {
                field.geometry.edge(cursor, key == "End")
            };
            (toward(selection, to.offset, modifiers.shift), to.upstream)
        } else {
            let Some(to) = moved(value, selection, key, modifiers.shift, textarea) else {
                return;
            };
            (to, false)
        };
        self.mark_field(id, value, to, false);
        if let Some(mark) = self.fields.get_mut(&id) {
            mark.upstream = upstream;
            mark.preferred_x = preferred_x;
        }
        if modifiers.shift && to.start != to.end && to != selection {
            self.field_select(id, value, to, self.host.now());
        }
    }

    /// HTML's `select` at a text field, where it has a handler.
    pub(super) fn field_select(
        &mut self,
        id: ViewId,
        value: &str,
        selection: FieldSelection,
        now_ms: f64,
    ) {
        if !self
            .host
            .runner()
            .handlers_of(id)
            .contains(&EventKind::Select)
        {
            return;
        }
        let event = Event::FieldSelect(
            value.to_owned(),
            self.reported_selection(id, value, selection),
        );
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
        if !self.has_selection_api(id) {
            self.host.log(format!(
                "setSelectionRange \"{name}\" refused: input has no text selection"
            ));
            return;
        }
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

    /// HTML's scripted range edit: no user-change/undo transaction, one
    /// replacement input, and a selection that never moves the focus.
    pub(crate) fn set_range_text(&mut self, args: &[exact_plan::Value]) {
        let Some(name) = args.first().and_then(exact_plan::Value::as_str) else {
            return;
        };
        let result = (|| {
            let id = self.text_field(name)?;
            let node = self.host.kernel().node(id).ok_or("no live field")?;
            let textarea = node.props.str(PropId::SemanticTag) == Some("textarea");
            if node.props.str(PropId::Markup) == Some("markdown")
                || (!textarea
                    && !matches!(
                        node.props.str(PropId::Type).unwrap_or("text"),
                        "text" | "search" | "url" | "tel" | "password"
                    ))
            {
                return Err("not a plain text field");
            }
            let text = args
                .get(1)
                .and_then(exact_plan::Value::as_str)
                .ok_or("replacement required")?;
            let old = self.field_selection(id);
            let (start, end) = if args.len() == 2 {
                (old.start, old.end)
            } else {
                let number = |i: usize| {
                    args.get(i)
                        .and_then(exact_plan::Value::as_number)
                        .ok_or("offset required")
                };
                let at =
                    |n| FieldSelection::clamped(n, n, SelectionDirection::None, u32::MAX).start;
                (at(number(2)?), at(number(3)?))
            };
            if start > end {
                return Err("start exceeds end");
            }
            let mode = args
                .get(4)
                .and_then(exact_plan::Value::as_str)
                .unwrap_or("preserve");
            let value: Vec<u16> = self.field_value(id).encode_utf16().collect();
            let start = (start as usize).min(value.len());
            let end = (end as usize).min(value.len());
            let units: Vec<u16> = text.encode_utf16().collect();
            let after = start + units.len();
            let delta = units.len() as i64 - (end - start) as i64;
            let (a, b) = match mode {
                "select" => (start as u32, after as u32),
                "start" => (start as u32, start as u32),
                "end" => (after as u32, after as u32),
                "preserve" => (
                    if old.start > end as u32 {
                        (i64::from(old.start) + delta) as u32
                    } else if old.start > start as u32 {
                        start as u32
                    } else {
                        old.start
                    },
                    if old.end > end as u32 {
                        (i64::from(old.end) + delta) as u32
                    } else if old.end > start as u32 {
                        after as u32
                    } else {
                        old.end
                    },
                ),
                _ => return Err("invalid selectionMode"),
            };
            let next = String::from_utf16_lossy(&[&value[..start], &units, &value[end..]].concat());
            let next = if textarea {
                next.replace("\r\n", "\n").replace('\r', "\n")
            } else {
                next.replace(['\r', '\n'], "")
            };
            let next = if !textarea && node.props.str(PropId::Type) == Some("url") {
                next.trim_matches([' ', '\t', '\n', '\r', '\u{c}'])
                    .to_owned()
            } else {
                next
            };
            let selection = FieldSelection::clamped(
                f64::from(a),
                f64::from(b),
                SelectionDirection::None,
                utf16_len(&next),
            );
            Ok((id, next, selection, old != selection))
        })();
        let (id, next, selection, moved) = match result {
            Ok(edit) => edit,
            Err(reason) => {
                self.host
                    .log(format!("setRangeText \"{name}\" refused: {reason}"));
                return;
            }
        };
        let bound = self.bound_text(id);
        self.mark_field(id, &next, selection, true);
        self.host.values.watch(id, Some(bound.clone()));
        self.chosen.insert(id, (next.clone(), bound.clone()));
        if self
            .host
            .runner()
            .handlers_of(id)
            .contains(&EventKind::Input)
        {
            if let Some(error) = self.host.dispatch_at(
                id,
                Event::Input(ControlValue::Edit(
                    next.clone(),
                    selection,
                    InputDetails {
                        input_type: "insertReplacementText".into(),
                        data: args.get(1).and_then(|v| v.as_str()).map(str::to_owned),
                        ..Default::default()
                    },
                )),
                self.host.now(),
            ) {
                self.host.log(format!("setRangeText: {error}"));
            }
            if let Some(error) = self.after_commit() {
                self.host.log(format!("setRangeText: {error}"));
            }
        }
        self.keep_typed(id, next.clone(), bound);
        if moved {
            self.field_select_tasks.push(id);
        }
    }

    pub(super) fn drain_field_select_tasks(&mut self) {
        for id in std::mem::take(&mut self.field_change_tasks) {
            if self
                .host
                .runner()
                .handlers_of(id)
                .contains(&EventKind::Selectionchange)
            {
                let value = self.field_value(id);
                let selection = self.reported_selection(id, &value, self.field_selection(id));
                let event = Event::FieldSelectionChange(value, selection);
                if let Some(e) = self
                    .host
                    .dispatch_at(id, event, self.host.now())
                    .or(self.after_commit())
                {
                    eprintln!("exact: {e}");
                }
            }
        }
        for id in std::mem::take(&mut self.field_select_tasks) {
            if self.host.kernel().node(id).is_some() {
                self.field_select(
                    id,
                    &self.field_value(id),
                    self.field_selection(id),
                    self.host.now(),
                );
            }
        }
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
    /// typed into (its `change` waits for blur or Enter), and its `input`,
    /// which carries the selection. The watch is the bound value from before
    /// the dispatch, so a write during the edit replaces the typed text and
    /// one that writes nothing leaves it (LLP 1069.001 D4).
    pub(super) fn field_edit(
        &mut self,
        id: ViewId,
        next: String,
        after: FieldSelection,
        bound: String,
        now_ms: f64,
    ) {
        if let Some(e) =
            self.apply_field_edit(id, next, after, bound, now_ms, InputDetails::default())
        {
            eprintln!("exact: {e}");
        }
    }

    pub(super) fn edit_field(
        &mut self,
        id: ViewId,
        next: String,
        after: FieldSelection,
        now: f64,
        mut edit: InputDetails,
    ) -> Option<String> {
        let bound = self.bound_text(id);
        match self.before_input(id, &edit, now) {
            Err(e) => Some(e),
            Ok(false) => None,
            Ok(true) => {
                edit.cancelable = false;
                self.apply_field_edit(id, next, after, bound, now, edit)
            }
        }
    }

    /// The field's `beforeinput` for `edit`: whether the edit goes ahead
    /// (not prevented, and the field still there with the value it had).
    pub(super) fn before_input(
        &mut self,
        id: ViewId,
        edit: &InputDetails,
        now: f64,
    ) -> Result<bool, String> {
        if !self
            .host
            .runner()
            .handlers_of(id)
            .contains(&EventKind::Beforeinput)
        {
            return Ok(true);
        }
        let value = self.field_text(id);
        let selection = self.reported_selection(id, &value, self.field_selection(id));
        let event = Event::BeforeInput(value.clone(), selection, edit.clone());
        if let Some(e) = self
            .host
            .dispatch_at(id, event, now)
            .or(self.after_commit())
        {
            return Err(e);
        }
        let queued = self.commands.len();
        self.commands.retain(|c| c.name != "preventDefault");
        Ok(!((edit.cancelable && queued != self.commands.len())
            || self.host.kernel().node(id).is_none()
            || self.field_text(id) != value))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_field_edit(
        &mut self,
        id: ViewId,
        next: String,
        after: FieldSelection,
        bound: String,
        now_ms: f64,
        edit: InputDetails,
    ) -> Option<String> {
        self.edited = Some(id);
        self.mark_field(id, &next, after, false);
        self.host.values.watch(id, Some(bound.clone()));
        // The input handler sees the edited field. An unchanged binding
        // must not reset its selection while that handler commits.
        self.chosen.insert(id, (next.clone(), bound.clone()));
        let error = if self
            .host
            .runner()
            .handlers_of(id)
            .contains(&EventKind::Input)
        {
            let reported = self.reported_selection(id, &next, after);
            let event = Event::Input(ControlValue::Edit(next.clone(), reported, edit));
            self.host.dispatch_at(id, event, now_ms)
        } else {
            None
        };
        let error = error.or(self.after_commit());
        self.keep_typed(id, next, bound);
        error
    }

    /// A key's default action in the focused text field: Enter submits an
    /// input or breaks a textarea's line, Backspace and Delete delete, the
    /// arrows, Home and End move the caret (Shift: extend the selection,
    /// firing `select` when it is a range), a character is typed — each
    /// edit at the selection.
    pub(crate) fn field_key(&mut self, id: ViewId, name: &str, now_ms: f64) {
        let (textarea, limit) = {
            let Some(node) = self.host.kernel().node(id) else {
                return;
            };
            if node.node_type != NodeType::TextInput
                || node.props.bool(PropId::Editable) == Some(false)
            {
                return;
            }
            (
                node.props.str(PropId::SemanticTag) == Some("textarea"),
                exact_kernel::control::text_maxlength(node.props),
            )
        };
        // What earlier commits replaced goes first: this edit starts from what shows.
        self.forget_replaced_choices();
        let value = self.field_text(id);
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
            "ArrowLeft" | "ArrowRight" | "Home" | "End" | "ArrowUp" | "ArrowDown"
                if !chord
                    || (self.modifiers().ctrl
                        && !self.modifiers().meta
                        && matches!(name, "ArrowLeft" | "ArrowRight")) =>
            {
                self.move_field(id, name, &value, selection, textarea);
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
        let (input_type, data) = match name {
            "Backspace" => ("deleteContentBackward", None),
            "Delete" => ("deleteContentForward", None),
            "Enter" => ("insertLineBreak", None),
            _ => ("insertText", Some(name.to_owned())),
        };
        if let Some(e) = self.edit_field(
            id,
            next,
            after,
            now_ms,
            InputDetails {
                input_type: input_type.into(),
                data,
                cancelable: true,
                is_composing: false,
            },
        ) {
            eprintln!("exact: {e}");
        }
    }

    /// A paste's default action at an editable text field: the text in at
    /// its selection (a field's own paste still inserts, beside any `paste`
    /// handler), within its `maxlength`.
    pub(crate) fn paste_field(&mut self, id: ViewId, text: &str, now_ms: f64) {
        let limit = {
            let Some(node) = self.host.kernel().node(id) else {
                return;
            };
            if node.node_type != NodeType::TextInput
                || node.props.bool(PropId::Editable) == Some(false)
                || node.props.bool(PropId::Disabled) == Some(true)
            {
                return;
            }
            exact_kernel::control::text_maxlength(node.props)
        };
        self.forget_replaced_choices();
        let value = self.field_text(id);
        let selection = self.field_selection(id);
        let text = fit(text, &value, selection, limit).to_owned();
        let (next, after) = replace(&value, selection, &text);
        if let Some(e) = self.edit_field(
            id,
            next,
            after,
            now_ms,
            InputDetails {
                input_type: "insertFromPaste".into(),
                data: Some(text),
                cancelable: true,
                is_composing: false,
            },
        ) {
            eprintln!("exact: {e}");
        }
    }
}

#[cfg(test)]
#[path = "field_geometry_tests.rs"]
mod field_geometry_tests;
