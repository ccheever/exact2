//! Text fields the shell hosts (LLP 1115.001 D8). On Android each `input`
//! of a text type and each `textarea` is the framework's own `EditText`,
//! over the container this host paints (`paint/field.rs`). The text, the
//! caret, the selection, the keyboard and the composition are the
//! platform's; the node, its box, its value as the app sees it and the
//! focus are the presenter's. What crosses is this:
//!
//! - **What the reader is told** ([`Presenter::reader_fields`]), with each
//!   frame: every live field, where its content box was painted and under
//!   which clip, which scroller's rows it moves with, what it shows, and
//!   how it looks and what keyboard it asks for. A field a statement leaves
//!   out is gone. A field's **generation** moves on when its node is
//!   renewed for another row's item (LLP 1078): the reader's view then
//!   starts over, with no selection and no undo of the last item's.
//! - **A touch** in a field's box is the field's, not the presenter's:
//!   [`Presenter::reader_under`] says which hosted field is under a point,
//!   by the presenter's own hit test, so a field under a bar or a sheet is
//!   not touched through it.
//! - **What the reader tells back**: the field took or lost the focus
//!   ([`Presenter::reader_field_focused`]), its text changed
//!   ([`Presenter::reader_field_edited`]), its selection moved
//!   ([`Presenter::reader_field_selected`]), its action key or Tab was
//!   pressed ([`Presenter::reader_field_action`]).
//!
//! **Who owns the text.** The field does, while the person edits it. Each
//! edit the reader tells is numbered, and a statement says the last number
//! that reached here beside the value the app then shows: the reader takes
//! a told value only when it answers its newest edit, and only when it
//! differs from what the field holds. So a commit that sets the value the
//! field already has changes nothing in it (its composition and its
//! selection stand), and one that sets another replaces the text.
//!
//! **The app's value never ends a composition** (LLP 1115.000 §5, ruling 6;
//! LLP 1115.001 D8 as corrected): Contract gives an action no way to know
//! text is composing, so one that rewrites what it hears would end a
//! Japanese or Chinese composition at its first keystroke, and on Android,
//! where the keyboard composes ordinary words, every word. The reader says
//! when a field's composition starts and ends
//! ([`Presenter::reader_field_composing`]). While it composes, a statement
//! says the field's own text where the app's value differs, so the reader
//! changes nothing; the app's value is held here, the last one, which is
//! whatever it shows when the composition ends, committed or cancelled.
//! The statement after that says it, and the reader's field takes it then.
//! An edit that ends the composition reaches the app first.
//!
//! **Composing text is reported as it composes**, as the web reports it and
//! as an Android `TextWatcher` hears it: `input` fires for each change of
//! the field's text, the composing region included. Android's keyboards
//! compose ordinary words (for autocorrect and glide typing), so reporting
//! on commit, as AppKit does, would hold every word back until its space.
//!
//! Desktop Linux hosts nothing and reads none of this ([`crate::fields`]).
use super::field::utf16_len;
use super::*;
use exact_kernel::style::roles;
use exact_kernel::{ColorValue, StyleId, StyleMask};
use exact_runner::{FieldSelection, SelectionDirection};
use std::collections::BTreeSet;

/// The action key or Tab, as the reader names it
/// ([`Presenter::reader_field_action`]): what the key said it does.
pub mod action {
    /// Enter with no action of its own (a hardware keyboard's, or the
    /// `enter` hint).
    pub const ENTER: i32 = 0;
    /// `done`.
    pub const DONE: i32 = 2;
    /// `go`.
    pub const GO: i32 = 3;
    /// `next`.
    pub const NEXT: i32 = 4;
    /// `previous`.
    pub const PREVIOUS: i32 = 5;
    /// `search`.
    pub const SEARCH: i32 = 6;
    /// `send`.
    pub const SEND: i32 = 7;
    /// Tab.
    pub const TAB: i32 = 16;
    /// Shift+Tab.
    pub const BACKTAB: i32 = 17;
}

/// A statement's flags for a field: it was painted, so its box is where
/// its view goes; and it takes no touch or focus (disabled, or under a
/// route that covers it).
const PLACED: i32 = 1;
const INERT: i32 = 2;

/// A field's look: a `textarea`, a password, disabled, read-only, no
/// autocorrect, no suggestions, italic, and whether Enter submits.
const MULTILINE: i32 = 1;
const PASSWORD: i32 = 2;
const DISABLED: i32 = 4;
const READONLY: i32 = 8;
const NO_CORRECT: i32 = 16;
const NO_SUGGEST: i32 = 32;
const ITALIC: i32 = 64;
const SUBMITS: i32 = 128;
/// Another field that can take the focus follows it in the tree: an enter
/// key left unsaid then says Next, as the platform's own field's does.
const FOLLOWED: i32 = 256;

/// What the presenter keeps of one field the reader hosts.
#[derive(Debug, Default, Clone, Copy)]
struct Known {
    /// Moved on when the node is renewed for another item.
    generation: u32,
    /// The reader's last edit that reached here.
    seq: u32,
    /// Moved on when a command sets the selection, with what it set.
    select: u32,
    at: (u32, u32),
}

/// What the presenter keeps of the fields its shell hosts.
#[derive(Default)]
pub(super) struct State {
    known: BTreeMap<ViewId, Known>,
    /// The fields that compose now, each with its own text as the reader
    /// last told it: what a statement says of it until the composition
    /// ends, so a value the app writes meanwhile is held.
    composing: BTreeMap<ViewId, String>,
    /// The last statement [`Presenter::reader_fields`] returned.
    last: Vec<i32>,
}

fn bits(v: f32) -> i32 {
    v.to_bits() as i32
}

fn argb(c: [u8; 4]) -> i32 {
    i32::from_be_bytes([c[3], c[0], c[1], c[2]])
}

/// The keyboard a field asks for, from `inputmode`, else its `type`: 0
/// text, 1 email, 2 URL, 3 telephone, 4 digits, 5 a decimal number, 6
/// search, 7 none (`inputmode="none"`: the app has its own keys).
fn keyboard(kind: &str, mode: Option<&str>) -> i32 {
    match mode {
        Some("none") => 7,
        Some("text") => 0,
        Some("email") => 1,
        Some("url") => 2,
        Some("tel") => 3,
        Some("numeric") => 4,
        Some("decimal") => 5,
        Some("search") => 6,
        _ => match kind {
            "email" => 1,
            "url" => 2,
            "tel" => 3,
            "number" => 5,
            "search" => 6,
            _ => 0,
        },
    }
}

impl<D: DataSource> Presenter<D> {
    /// The live text fields the shell hosts, in tree order.
    fn hosted_ids(&self) -> Vec<ViewId> {
        let kernel = self.host.kernel();
        if crate::fields::hosting().is_none() || !kernel.has_type(NodeType::TextInput) {
            return Vec::new();
        }
        kernel.preorder_where(&kernel.roots(), |kind, props| {
            kind == NodeType::TextInput && props.str(PropId::Markup) != Some("markdown")
        })
    }

    /// Whether the shell hosts field `id`, a live one.
    pub(crate) fn hosts_field(&self, id: ViewId) -> bool {
        (self.host.kernel().node(id)).is_some_and(|node| crate::fields::hosts(&node))
    }

    /// The hosted field that holds the focus, if one does: the software
    /// keyboard is then that field's own (`requests.rs`).
    pub(super) fn hosted_focus(&self) -> ViewId {
        self.focus.filter(|id| self.hosts_field(*id)).unwrap_or(0)
    }

    /// The hosted field a touch on `hit` is for: `hit` itself, when the
    /// shell hosts it and it takes touches.
    pub(super) fn hosted_at(&self, hit: ViewId) -> Option<ViewId> {
        let node = self.host.kernel().node(hit)?;
        (crate::fields::hosts(&node)
            && node.props.bool(PropId::Disabled) != Some(true)
            && self.host.route_visibility(hit) == (false, false))
            .then_some(hit)
    }

    /// What the reader is told of the text fields it hosts, in its pixels
    /// (`scale` to a point), when that is not what it was last told
    /// (`None`). A count, then for each field, in tree order:
    ///
    /// - its id, its generation, flags (bit 0 it was painted and the box
    ///   that follows is where it goes, bit 1 it takes no touch and no
    ///   focus), and the scroller whose rows it was painted among (0 none);
    /// - as float bits its content box (x, y, width, height) and the clip
    ///   it was painted under (x, y, width, height; a negative width for
    ///   none), where the last paint put them;
    /// - the number of the reader's last edit that reached here, and the
    ///   value the app shows as of it (a string: its bytes' count, then
    ///   UTF-8 padded to whole words);
    /// - its look, eighteen words: what it is (bit 0 a `textarea`, 1 a
    ///   password, 2 disabled, 3 read-only, 4 no autocorrect, 5 no
    ///   suggestions, 6 italic, 7 Enter submits, 8 a field that can take
    ///   the focus follows it); its keyboard
    ///   ([`keyboard`]); its capitalization (0 none, 1 sentences, 2 words,
    ///   3 characters); its enter key (0 unsaid, 1 enter, 2 done, 3 go, 4
    ///   next, 5 previous, 6 search, 7 send); its `maxlength` (-1 none);
    ///   its alignment (0 start, 1 centre, 2 end); the colours of its
    ///   text, its placeholder, its caret, its handles and its selection
    ///   (ARGB); its font's size in pixels, weight, tracking in ems and
    ///   line height in pixels (0: the font's own) as float bits (the
    ///   weight a count); and the revision, start and end of the selection
    ///   a command last set;
    /// - three strings: its placeholder, its `autocomplete` and its font's
    ///   file ([`crate::fields::FieldFace::family`]).
    ///
    /// A field the reader knew that a statement leaves out is gone.
    pub fn reader_fields(&mut self, scale: f32) -> Option<Vec<i32>> {
        // The fields' colours resolve its session's roles (LLP 1121 stage 5).
        let _roles = self.host.runner().bind_roles();
        let ids = self.hosted_ids();
        if ids.is_empty() {
            if !self.hosted.known.is_empty() {
                self.hosted.known.clear();
                self.hosted.composing.clear();
                self.brush.field_content.clear();
            }
            // The last field went: said once, with nothing in it.
            return (self.hosted.last.len() > 1).then(|| {
                self.hosted.last = vec![0];
                vec![0]
            });
        }
        let wanted: BTreeSet<ViewId> = ids.iter().copied().collect();
        self.hosted.known.retain(|id, _| wanted.contains(id));
        self.hosted.composing.retain(|id, _| wanted.contains(id));
        self.brush.field_content.retain(|id, _| wanted.contains(id));
        // Where the last paint put each: its box, among which rows.
        let mut painted: BTreeMap<ViewId, usize> = BTreeMap::new();
        for (at, b) in self.boxes.iter().enumerate() {
            if wanted.contains(&b.id) {
                painted.entry(b.id).or_insert(at);
            }
        }
        // The last field that can take the focus: every one before it is
        // followed by a field.
        let last = ids.iter().rposition(|id| {
            self.focusable(*id) && self.host.route_visibility(*id) == (false, false)
        });
        let mut out = vec![ids.len() as i32];
        for (at, id) in ids.into_iter().enumerate() {
            let followed = last.is_some_and(|last| at < last);
            self.field_words(id, painted.get(&id).copied(), followed, scale, &mut out);
        }
        if out == self.hosted.last {
            return None;
        }
        self.hosted.last.clone_from(&out);
        Some(out)
    }

    fn field_words(
        &mut self,
        id: ViewId,
        painted: Option<usize>,
        followed: bool,
        scale: f32,
        out: &mut Vec<i32>,
    ) {
        let known = *self.hosted.known.entry(id).or_default();
        let (hidden, inert) = self.host.route_visibility(id);
        let kernel = self.host.kernel();
        let disabled =
            (kernel.node(id)).is_some_and(|n| n.props.bool(PropId::Disabled) == Some(true));
        let place = painted
            .filter(|_| !hidden && self.display.allows(kernel, id))
            .zip(self.brush.field_content.get(&id).copied())
            .map(|(at, within)| {
                let b = self.boxes[at];
                let groups = self.brush.row_groups();
                let group = groups.iter().find(|g| g.1 <= at && at < g.2);
                (b, within, group.map_or(0, |g| g.0))
            });
        let mut flags = 0;
        if place.is_some() {
            flags |= PLACED;
        }
        if inert || disabled {
            flags |= INERT;
        }
        let group = place.map_or(0, |p| p.2);
        out.extend([id as i32, known.generation as i32, flags, group as i32]);
        let (rect, clip) = match place {
            Some((b, within, _)) => (
                [
                    b.rect.0 + within[0],
                    b.rect.1 + within[1],
                    within[2],
                    within[3],
                ],
                b.clip
                    .map_or([0.0, 0.0, -1.0, -1.0], |c| [c.0, c.1, c.2, c.3]),
            ),
            None => ([0.0; 4], [0.0, 0.0, -1.0, -1.0]),
        };
        out.extend(rect.iter().chain(&clip).map(|v| bits(v * scale)));
        out.push(known.seq as i32);
        // While it composes the field is told its own text: the app's
        // value, where it differs, is held until the composition ends.
        match self.hosted.composing.get(&id) {
            Some(own) => requests::string(out, own),
            None => requests::string(out, &self.field_text(id)),
        }
        self.field_look(id, known, followed, scale, out);
    }

    /// A field's look and its keyboard, as [`Presenter::reader_fields`] has
    /// them: computed from the node, with the colours the roles resolve to
    /// in its scheme and an authored `color`, `caret-color` or
    /// `accent-color` winning.
    fn field_look(&self, id: ViewId, known: Known, followed: bool, scale: f32, out: &mut Vec<i32>) {
        let kernel = self.host.kernel();
        let Some(node) = kernel.node(id) else {
            return;
        };
        let props = node.props;
        let style = node.computed_style(StyleMask::INHERITED);
        let dark = node.color_scheme_dark().unwrap_or(self.brush.dark);
        let textarea = props.str(PropId::SemanticTag) == Some("textarea");
        let kind = props.str(PropId::Type).unwrap_or("text");
        let disabled = props.bool(PropId::Disabled) == Some(true);
        let off = |id: PropId| matches!(props.str(id), Some("off" | "false"));
        let face = crate::fields::face(
            &mut self.text.borrow_mut(),
            &node.text_style(),
            textarea,
            scale,
        );
        let submits = (self.host.runner().handlers_of(id)).contains(&EventKind::Submit);
        let mut what = 0;
        for (bit, on) in [
            (MULTILINE, textarea),
            (PASSWORD, kind == "password"),
            (DISABLED, disabled),
            (READONLY, props.bool(PropId::Editable) == Some(false)),
            (NO_CORRECT, off(PropId::Autocorrect)),
            (NO_SUGGEST, off(PropId::Spellcheck)),
            (ITALIC, face.italic),
            (SUBMITS, submits),
            (FOLLOWED, followed),
        ] {
            if on {
                what |= bit;
            }
        }
        let capitalize = match props.str(PropId::Autocapitalize) {
            Some("sentences" | "on") => 1,
            Some("words") => 2,
            Some("characters") => 3,
            _ => 0,
        };
        let enter = match props.str(PropId::EnterKeyHint) {
            Some("enter") => 1,
            Some("done") => action::DONE,
            Some("go") => action::GO,
            Some("next") => action::NEXT,
            Some("previous") => action::PREVIOUS,
            Some("search") => action::SEARCH,
            Some("send") => action::SEND,
            _ => 0,
        };
        let maxlength = exact_kernel::control::text_maxlength(props).map_or(-1, |n| n as i32);
        let align = match style.text_align.physical(style.direction) {
            exact_kernel::TextAlign::Center => 1,
            exact_kernel::TextAlign::Right => 2,
            _ => 0,
        };
        let rgba = crate::paint::rgba;
        let _accent = crate::paint::accent_scope(&node);
        let role = |name: &str| roles::role(name).map(|r| rgba(ColorValue::Role(r).resolve(dark)));
        let mut ink = rgba(node.text_color().resolve(dark));
        if disabled
            && node.style.appearance != exact_kernel::Appearance::None
            && !node.style.mask.has(StyleId::TextColor)
        {
            ink = crate::paint::control::disabled_field_ink(ink, dark);
        }
        let hint = crate::paint::control::authored_placeholder(node.style, dark)
            .or_else(|| role("-exact-placeholder"))
            .unwrap_or([0x75, 0x75, 0x75, 0xff]);
        let authored = crate::paint::control::accent(&node, dark);
        let accent = crate::paint::control::accents(authored, dark).0;
        let caret = style.caret_color.map_or(accent, |c| rgba(c.resolve(dark)));
        // The selection is the accent, thinned: the `Highlight` role where
        // the accent is the platform's, as Material's is `primary` at 40%.
        let thinned = [accent[0], accent[1], accent[2], 0x66];
        let highlight = match authored {
            Some(_) => thinned,
            None => role("Highlight").unwrap_or(thinned),
        };
        out.extend([
            what,
            keyboard(kind, props.str(PropId::InputMode)),
            capitalize,
            enter,
        ]);
        out.extend([maxlength, align]);
        out.extend([ink, hint, caret, accent, highlight].map(argb));
        out.extend([
            bits(face.size),
            i32::from(face.weight),
            bits(face.spacing),
            bits(face.line),
        ]);
        out.extend([known.select as i32, known.at.0 as i32, known.at.1 as i32]);
        requests::string(out, props.str(PropId::Placeholder).unwrap_or(""));
        requests::string(out, props.str(PropId::Autocomplete).unwrap_or(""));
        requests::string(out, &face.family);
    }

    /// Whether `id` is a field the reader hosts and knows at `generation`:
    /// what it says of a field renewed since, or gone, is dropped.
    fn hosted_live(&self, id: ViewId, generation: u32) -> bool {
        (self.hosted.known.get(&id)).is_some_and(|known| known.generation == generation)
            && self.hosts_field(id)
    }

    /// The reader's field took the focus (a finger tapped it), or lost it
    /// with nothing else of the page taking it: the presenter's focus
    /// follows, with the `blur`, `change` and `focus` that move makes.
    pub fn reader_field_focused(
        &mut self,
        id: ViewId,
        generation: u32,
        focused: bool,
        now_ms: f64,
    ) {
        if !self.hosted_live(id, generation) {
            return;
        }
        // Under an open surface of the top layer the page is inert (LLP
        // 1115.001 D10): a field there is not given the focus, whatever
        // made its view take it (a hardware key ends touch mode, and the
        // platform then focuses the window's first focusable view). The
        // shell is told again that no field holds it, which blurs the view.
        if focused && self.top_open() {
            self.keyboard_changed();
            return;
        }
        let next = match focused {
            true if self.focusable(id) => Some(id),
            false if self.focus == Some(id) => None,
            _ => return,
        };
        if self.focus == next {
            return;
        }
        if let Some(e) = self.set_focus(next, now_ms) {
            eprintln!("exact: {e}");
        }
        self.queue_collections();
    }

    /// The reader's field holds `value` after its edit number `seq`, with
    /// the selection at `start` to `end` (UTF-16 units): an edit of the
    /// person's, composing text included. It is the field's text from now
    /// on and its `input` fires, as for a key typed on desktop Linux; the
    /// field's `change` waits for the focus to leave or the action key. A
    /// text the field already shows changes only where the selection is.
    pub fn reader_field_edited(
        &mut self,
        id: ViewId,
        (generation, seq): (u32, u32),
        value: &str,
        (start, end): (u32, u32),
        now_ms: f64,
    ) {
        if !self.hosted_live(id, generation) {
            return;
        }
        if let Some(known) = self.hosted.known.get_mut(&id) {
            known.seq = seq;
        }
        if let Some(own) = self.hosted.composing.get_mut(&id) {
            value.clone_into(own);
        }
        let len = utf16_len(value);
        let selection =
            FieldSelection::clamped(start.into(), end.into(), SelectionDirection::None, len);
        if self.field_text(id) == value {
            self.note_field(id, value, selection);
            return;
        }
        let editable = (self.host.kernel().node(id)).is_some_and(|n| {
            n.props.bool(PropId::Editable) != Some(false)
                && n.props.bool(PropId::Disabled) != Some(true)
        });
        if !editable {
            // Told back what it shows: the next statement answers this edit.
            return;
        }
        let bound = self.bound_text(id);
        self.forget_replaced_choices();
        self.field_edit(id, value.to_owned(), selection, bound, now_ms);
    }

    /// The reader's field began to compose, holding `value`, or its
    /// composition ended (committed or cancelled), before any edit that
    /// began or ended it is told. While a field composes, a value the app
    /// writes is held: a statement says the field's own text
    /// ([`Self::reader_fields`]). When it ends the next statement says the
    /// app's value as it then stands, the last it wrote, after the app has
    /// heard the edit that ended it.
    pub fn reader_field_composing(
        &mut self,
        id: ViewId,
        generation: u32,
        composing: bool,
        value: &str,
    ) {
        if !self.hosted_live(id, generation) {
            return;
        }
        if composing {
            self.hosted.composing.insert(id, value.to_owned());
        } else {
            self.hosted.composing.remove(&id);
        }
    }

    /// The reader's field moved its selection with no edit: kept for the
    /// next `input` and `change`, and `select` fires for a range the
    /// person made, as it does for Shift and an arrow on desktop Linux.
    pub fn reader_field_selected(
        &mut self,
        id: ViewId,
        generation: u32,
        (start, end): (u32, u32),
        now_ms: f64,
    ) {
        if !self.hosted_live(id, generation) {
            return;
        }
        let value = self.field_text(id);
        let selection = FieldSelection::clamped(
            start.into(),
            end.into(),
            SelectionDirection::None,
            utf16_len(&value),
        );
        let before = self.field_selection(id);
        if (before.start, before.end) == (selection.start, selection.end) {
            return;
        }
        self.note_field(id, &value, selection);
        if selection.start != selection.end {
            self.field_select(id, &value, selection, now_ms);
        }
    }

    /// The reader's field had its action key pressed, or Enter or Tab on a
    /// hardware keyboard. `action` is what the key said it does
    /// ([`action`]), and the key does that, as the other hosts' return key
    /// does:
    ///
    /// - Enter first, as a key: the `key` handlers hear it, and unless one
    ///   prevents it a single-line field commits (`change`) and submits;
    /// - then `next` and `previous` move to the field after or before this
    ///   one, and with none there the focus leaves; `done`, `go`, `search`
    ///   and `send` leave the field, which puts the keyboard away;
    /// - Tab and Shift+Tab are the page's own Tab.
    pub fn reader_field_action(&mut self, id: ViewId, generation: u32, action: i32, now_ms: f64) {
        if !self.hosted_live(id, generation) || !self.focusable(id) {
            return;
        }
        if self.focus != Some(id) {
            if let Some(e) = self.set_focus(Some(id), now_ms) {
                eprintln!("exact: {e}");
            }
        }
        if matches!(action, action::TAB | action::BACKTAB) {
            let shift = self.held & 1;
            self.hold_modifier("ShiftLeft", action == action::BACKTAB);
            self.key_down_with("Tab", "Tab", false, now_ms);
            self.hold_modifier("ShiftLeft", shift != 0);
            return;
        }
        let (error, prevented) = self.key_event(EventKind::Key, "Enter", "Enter", false, now_ms);
        if let Some(e) = error {
            eprintln!("exact: {e}");
        }
        if prevented || self.focus != Some(id) {
            return;
        }
        let textarea = (self.host.kernel().node(id))
            .is_some_and(|n| n.props.str(PropId::SemanticTag) == Some("textarea"));
        if !textarea {
            self.field_key(id, "Enter", now_ms);
        }
        if self.focus != Some(id) {
            return;
        }
        let next = match action {
            action::NEXT | action::PREVIOUS => self.field_beside(id, action == action::PREVIOUS),
            action::DONE | action::GO | action::SEARCH | action::SEND => None,
            _ => return,
        };
        if let Some(e) = self.set_focus(next, now_ms) {
            eprintln!("exact: {e}");
        }
        if let Some(next) = next {
            self.keep_in_view(next, reveal::Cause::FieldNavigation);
        }
        self.queue_collections();
    }

    /// The text field after `id` in the tree (`backward`: before it) that
    /// can take the focus.
    fn field_beside(&self, id: ViewId, backward: bool) -> Option<ViewId> {
        let kernel = self.host.kernel();
        let fields = kernel.preorder_where(&kernel.roots(), |kind, _| kind == NodeType::TextInput);
        let at = fields.iter().position(|f| *f == id)?;
        let takes =
            |f: &&ViewId| self.focusable(**f) && self.host.route_visibility(**f) == (false, false);
        match backward {
            true => fields[..at].iter().rev().find(takes).copied(),
            false => fields[at + 1..].iter().find(takes).copied(),
        }
    }

    /// A command set field `id`'s selection (`setSelectionRange`,
    /// `select()`): the reader's field takes it.
    pub(super) fn hosted_commanded(&mut self, id: ViewId, selection: FieldSelection) {
        if let Some(known) = self.hosted.known.get_mut(&id) {
            known.select += 1;
            known.at = (selection.start, selection.end);
        }
    }

    /// Views were renewed (LLP 1078: a list's row rebound to another
    /// item). A hosted field among them is another field from here on: its
    /// generation moves on, and nothing kept for the last item's text
    /// (typed text, its selection, a `change` owed) stands.
    pub(super) fn hosted_renewed(&mut self, renewed: &[ViewId]) {
        if self.hosted.known.is_empty() {
            return;
        }
        for id in renewed {
            let Some(known) = self.hosted.known.get_mut(id) else {
                continue;
            };
            *known = Known {
                generation: known.generation + 1,
                ..Default::default()
            };
            self.hosted.composing.remove(id);
            self.fields.remove(id);
            self.chosen.remove(id);
            if self.edited == Some(*id) {
                self.edited = None;
            }
        }
    }

    /// The reader was made again: it is told every field anew, and its new
    /// views compose nothing.
    pub(super) fn hosted_reset(&mut self) {
        self.hosted.last.clear();
        self.hosted.composing.clear();
    }

    /// A text field's own starting font follows the device's font scale
    /// where the shell hosts it (LLP 1115.001 D8): the platform's field is
    /// set at 16 scaled pixels, as its body text is, where desktop Linux
    /// keeps the page's initial 16.
    pub(super) fn scale_field_font(&mut self, font_scale: f32) -> Option<String> {
        crate::fields::hosting()?;
        match self.host.set_field_font_size(16.0 * font_scale) {
            Ok(true) => self.after_commit(),
            Ok(false) => None,
            Err(e) => Some(e),
        }
    }
}

#[cfg(test)]
#[path = "hosted_tests.rs"]
mod tests;
