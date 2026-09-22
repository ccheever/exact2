//! The editing rules: where typed text lands around hidden syntax, what
//! deletion removes, pending formats, list Return, commands, and undo.
//! A host feeds it input events and draws [`crate::Line`]s; it never edits
//! the source itself except by letting the platform type plain text, which
//! it reports back through [`Editor::reconcile`].

use exact_markdown as md;
use md::{Range, Styled};

use crate::projection::Projection;
use crate::text::{self, clean, splice, utf16, utf8, Map, Rep};

/// A host input event, by its `beforeinput` input type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Anything the platform may do itself and report through `reconcile`.
    Other,
    /// `historyUndo`.
    Undo,
    /// `historyRedo`.
    Redo,
    /// `formatBold`.
    Bold,
    /// `formatItalic`.
    Italic,
    /// `formatStrikeThrough`.
    Strike,
    /// Any other `format*`: refused, so no presentational markup appears.
    Format,
    /// `insertParagraph` or `insertLineBreak`.
    Paragraph,
    /// `insertText`.
    Text,
    /// `insertReplacementText`: autocorrect or a spelling suggestion.
    Replacement,
    /// `insertFromPaste`, `insertFromDrop` or `insertFromYank`.
    Paste,
    /// `deleteContentBackward`.
    Backward,
    /// `deleteContentForward`.
    Forward,
    /// Any other backward deletion: a word, a line, a cut.
    OtherBackward,
    /// Any other forward deletion.
    OtherForward,
}

/// What a call did, for the host to show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// The source changed: redraw it and tell the app.
    pub source: bool,
    /// Put the platform selection at [`Editor::selection`].
    pub place: bool,
}

/// What a platform selection change needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selected {
    /// Nothing: it is the selection the host just placed.
    Ignore,
    /// Report the selection's formats.
    Emit,
    /// The caret moved off a hidden position: place it, then report.
    Place,
}

/// A `beforeinput` answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Let the platform insert it; report the result through `reconcile`.
    Native,
    /// Prevent the platform's default; the editor applied it.
    Handled(Change),
}

/// The toolbar facts a `select` event carries (LLP 1045 D6).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// Space-separated command names, pending formats applied.
    pub formats: String,
    /// The selection spans differing formats.
    pub mixed: bool,
    /// The link under the caret, or empty.
    pub link: String,
    /// Space-separated commands that do nothing here.
    pub unavailable: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    // In name order: the order pending markers nest.
    Bold,
    Code,
    Italic,
    Strike,
}

const FORMATS: [Format; 4] = [Format::Bold, Format::Code, Format::Italic, Format::Strike];

impl Format {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "bold" => Self::Bold,
            "code" => Self::Code,
            "italic" => Self::Italic,
            "strike" => Self::Strike,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Bold => "bold",
            Self::Code => "code",
            Self::Italic => "italic",
            Self::Strike => "strike",
        }
    }

    fn marker(self) -> &'static [u16] {
        match self {
            Self::Bold => &[42, 42],
            Self::Code => &[96],
            Self::Italic => &[42],
            Self::Strike => &[126, 126],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Typing,
    Delete,
    Edit,
}

struct Snapshot {
    source: Vec<u16>,
    sel: (u32, u32),
}

/// Typing within this many milliseconds of the last keystroke is one undo.
const COALESCE_MS: f64 = 1500.0;
/// Undo keeps at most this many steps and this many code units of source.
const UNDO_STEPS: usize = 200;
const UNDO_UNITS: usize = 1 << 22;
/// Syntax left visible by an edit that used to be hidden is removed.
const ORPHANS: &[u8] = b"*_~`[]()#>-+.! ";

struct Sim {
    result: Vec<u16>,
    orphans: usize,
    maps: Vec<Map>,
}

impl Sim {
    fn caret(&self, p: u32) -> u32 {
        self.maps.iter().fold(p, |p, m| m.caret(p))
    }
}

struct Edit16 {
    reps: Vec<Rep>,
    sel: (u32, u32),
}

/// One editor: its source, selection, pending formats and history.
pub struct Editor {
    source: Vec<u16>,
    utf8: String,
    pub(crate) styled: Styled,
    pub(crate) proj: Projection,
    sel: (u32, u32),
    /// A caret deliberately after an inline closer.
    sticky: Option<u32>,
    /// Formats toggled on or off at a caret, for the next insertion.
    pending: [Option<bool>; 4],
    undo: Vec<Snapshot>,
    undo_units: usize,
    redo: Vec<Snapshot>,
    last: Option<Kind>,
    last_at: f64,
    now: f64,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    /// An empty editor.
    pub fn new() -> Self {
        let mut editor = Self {
            source: Vec::new(),
            utf8: String::new(),
            styled: Styled::default(),
            proj: Projection::default(),
            sel: (0, 0),
            sticky: None,
            pending: [None; 4],
            undo: Vec::new(),
            undo_units: 0,
            redo: Vec::new(),
            last: None,
            last_at: f64::NEG_INFINITY,
            now: 0.0,
        };
        editor.restyle();
        editor
    }

    /// Start over on `text`, the caret at its end.
    pub fn load(&mut self, text: &[u16]) {
        self.source = clean(text);
        self.forget();
        self.restyle();
        let end = self.len();
        self.sel = (end, end);
    }

    /// The Markdown source.
    pub fn source(&self) -> &[u16] {
        &self.source
    }

    /// The selection, in source code units, start before end.
    pub fn selection(&self) -> (u32, u32) {
        self.sel
    }

    /// The source with every hidden code unit removed: what a person reads.
    pub fn visible_text(&self) -> String {
        utf8(&self.proj.visible_text(&self.source))
    }

    /// Which side of hidden syntax a caret at `p` is drawn on: true for after.
    pub fn draws_after(&self, p: u32) -> bool {
        let c = self.proj.cluster(p);
        c.0 != c.1 && p == c.1
    }

    fn pending(&self) -> impl Iterator<Item = (Format, bool)> + '_ {
        FORMATS
            .into_iter()
            .filter_map(|f| self.pending[f as usize].map(|on| (f, on)))
    }

    fn len(&self) -> u32 {
        self.source.len() as u32
    }

    fn clamp(&self, (a, b): (u32, u32)) -> (u32, u32) {
        let len = self.len();
        (a.min(b).min(len), a.max(b).min(len))
    }

    fn forget(&mut self) {
        self.undo.clear();
        self.undo_units = 0;
        self.redo.clear();
        self.pending = [None; 4];
        self.sticky = None;
        self.last = None;
    }

    fn restyle(&mut self) {
        self.utf8 = utf8(&self.source);
        self.styled = md::style(&self.utf8, None);
        self.proj = Projection::new(&self.source, &self.styled);
    }

    fn snap(&self, p: u32) -> u32 {
        self.proj.canonical(p)
    }

    /// The app wrote a value. The changed middle is replaced and the selection
    /// carried through it; the history resets, as it does on Apple (LLP 1045 D1).
    pub fn set_value(&mut self, text: &[u16]) -> Change {
        let text = clean(text);
        if text == self.source {
            return Change::default();
        }
        let old = &self.source;
        let (mut a, mut z) = (0, 0);
        while a < old.len() && a < text.len() && old[a] == text[a] {
            a += 1;
        }
        while z < old.len() - a
            && z < text.len() - a
            && old[old.len() - 1 - z] == text[text.len() - 1 - z]
        {
            z += 1;
        }
        if a > 0 && text::high(old[a - 1]) {
            a -= 1;
        }
        if z > 0 && text::low(old[old.len() - z]) {
            z -= 1;
        }
        let (a, end, inserted) = (
            a as i64,
            (old.len() - z) as i64,
            (text.len() - z) as i64 - a as i64,
        );
        let carry = |p: u32| {
            let p = i64::from(p);
            (if p <= a {
                p
            } else if p >= end {
                p + inserted - (end - a)
            } else {
                a + inserted
            }) as u32
        };
        let (s, e) = (carry(self.sel.0), carry(self.sel.1));
        self.source = text;
        self.forget();
        self.restyle();
        let (s, e) = self.clamp((s, e));
        self.sel = if s == e {
            (self.snap(s), self.snap(s))
        } else {
            (s, e)
        };
        Change {
            source: true,
            place: true,
        }
    }

    /// Select everything.
    pub fn select_all(&mut self) -> Change {
        self.sel = (0, self.len());
        self.pending = [None; 4];
        self.sticky = None;
        Change {
            source: false,
            place: true,
        }
    }

    /// The platform selection moved to `from..to`. `placed` is the selection
    /// the host last put there itself, whose echo changes nothing.
    pub fn select(&mut self, from: u32, to: u32, placed: Option<(u32, u32)>) -> Selected {
        let (from, to) = self.clamp((from, to));
        let proj = &self.proj;
        if let Some(p) = placed {
            let p = self.clamp(p);
            if p == (from, to)
                || (proj.visible(p.0) == proj.visible(from)
                    && proj.visible(p.1) == proj.visible(to))
            {
                return Selected::Ignore;
            }
        }
        let moved = proj.visible(from) != proj.visible(self.sel.0)
            || to != from
            || self.sel.1 != self.sel.0;
        if moved {
            self.pending = [None; 4];
            self.sticky = None;
        }
        if from == to {
            if self
                .sticky
                .is_some_and(|s| proj.visible(s) == proj.visible(from))
            {
                return Selected::Emit;
            }
            let c = proj.canonical(from);
            self.sel = (c, c);
            if c != from {
                return Selected::Place;
            }
        } else {
            let inside = |p: u32| Some(proj.cluster(p)).filter(|c| c.0 < p && p < c.1);
            self.sel = (
                inside(from).map_or(from, |c| c.0),
                inside(to).map_or(to, |c| c.1),
            );
        }
        Selected::Emit
    }

    /// A `beforeinput` event. `data` is its text; `target` its first target
    /// range, when the platform gave one, in source code units.
    pub fn before_input(
        &mut self,
        input: Input,
        data: &[u16],
        target: Option<(u32, u32)>,
        now: f64,
    ) -> Outcome {
        self.now = now;
        let handled = Outcome::Handled;
        match input {
            Input::Other => return Outcome::Native,
            Input::Undo => return handled(self.undo()),
            Input::Redo => return handled(self.redo()),
            Input::Bold => return handled(self.command("bold", "", now)),
            Input::Italic => return handled(self.command("italic", "", now)),
            Input::Strike => return handled(self.command("strike", "", now)),
            Input::Format => return handled(Change::default()),
            _ => {}
        }
        let (mut from, mut to) = self.sel;
        let deleting = matches!(
            input,
            Input::Backward | Input::Forward | Input::OtherBackward | Input::OtherForward
        );
        let collapsed = matches!(input, Input::Backward | Input::Forward) && from == to;
        if let Some(t) = target.filter(|_| !collapsed && (deleting || input == Input::Replacement))
        {
            (from, to) = self.clamp(t);
        }
        match input {
            Input::Paragraph => handled(self.newline(from, to)),
            Input::Text | Input::Replacement | Input::Paste => {
                let data = clean(data);
                let p = self.sticky.unwrap_or(from);
                let c = self.proj.cluster(p);
                let quiet = self.pending().next().is_none()
                    && from == to
                    && c.0 == c.1
                    && !data.contains(&10);
                if quiet && input == Input::Text {
                    return Outcome::Native;
                }
                let (a, b) = if p == from { (from, to) } else { (p, p) };
                handled(self.insert(&data, a, b))
            }
            _ => {
                let backward = matches!(input, Input::Backward | Input::OtherBackward);
                handled(self.remove(from, to, backward, target.map(|t| self.clamp(t))))
            }
        }
    }

    /// The platform typed or composed text itself; `text` is the result and
    /// `sel` its selection.
    pub fn reconcile(&mut self, text: &[u16], sel: Option<(u32, u32)>, now: f64) -> Change {
        self.now = now;
        let text = clean(text);
        if text == self.source {
            if let Some(s) = sel {
                self.sel = self.clamp(s);
            }
            return Change::default();
        }
        self.push_undo(Kind::Typing);
        let before = std::mem::take(&mut self.proj);
        self.source = text;
        self.restyle();
        self.sel = self.clamp(sel.unwrap_or(self.sel));
        let p = self.sel.0;
        if self.sel.0 == self.sel.1 {
            // Typing a closer (a Markdown shortcut) leaves the caret outside it.
            if p > 0 && self.proj.is_hidden(p - 1) && !before.is_hidden(p - 1) {
                self.sticky = Some(p);
            } else {
                self.sticky = None;
                let c = self.snap(p);
                self.sel = (c, c);
            }
        }
        Change {
            source: true,
            place: true,
        }
    }

    /// A toolbar or shortcut command, by the shared vocabulary (LLP 1045 D6),
    /// plus `undo` and `redo`. Bold, italic, code and strike at a caret
    /// toggle a pending format for the next insertion.
    pub fn command(&mut self, name: &str, argument: &str, now: f64) -> Change {
        self.now = now;
        let (from, to) = self.sel;
        match name {
            "undo" => return self.undo(),
            "redo" => return self.redo(),
            _ => {}
        }
        if let Some(format) = Format::named(name).filter(|_| from == to) {
            let slot = format as usize;
            self.pending[slot] = match self.pending[slot] {
                Some(_) => None,
                None => Some(!self.facts().formats.split(' ').any(|f| f == name)),
            };
            return Change::default();
        }
        let (name, argument) = match name.strip_prefix("heading") {
            Some(level) if !level.is_empty() => ("heading", level),
            _ => (name, argument),
        };
        if name == "link" && from == to && self.facts().link.is_empty() {
            return self.insert_link(argument);
        }
        let Some(edit) = self.edit(name, argument, (from, to)) else {
            return Change::default();
        };
        let sim = self.simulate(&edit.reps, false);
        let change = self.commit(sim.result, edit.sel, false, Kind::Edit);
        let (s, e) = self.sel;
        self.sel = (self.snap(s), if s == e { self.snap(e) } else { e });
        change
    }

    /// Toggle the task box of the item starting at `at`.
    pub fn toggle_task(&mut self, at: u32, now: f64) -> Change {
        self.now = now;
        let Some(edit) = self.edit("toggleTask", "", (at, at)) else {
            return Change::default();
        };
        let (result, _) = splice(&self.source, &edit.reps);
        self.commit(result, self.sel, false, Kind::Edit)
    }

    /// Remove the selection's visible text, after a host copied it.
    pub fn cut(&mut self, now: f64) -> Change {
        self.now = now;
        let (from, to) = self.sel;
        if from == to {
            return Change::default();
        }
        self.remove(from, to, true, None)
    }

    /// The source a copy of the selection takes: whole syntax at its edges.
    pub fn copy_range(&self) -> Option<(u32, u32)> {
        let (mut a, mut b) = self.sel;
        if a == b {
            return None;
        }
        let ca = self.proj.cluster(a);
        if ca.1 == a {
            a = ca.0;
        }
        let cb = self.proj.cluster(b);
        if cb.0 == b {
            b = cb.1;
        }
        Some((a, b))
    }

    /// The selection's formats for a toolbar, pending formats applied.
    pub fn facts(&self) -> Facts {
        let s = md::selection(&self.utf8, Range::new(self.sel.0, self.sel.1));
        let mut formats: Vec<&str> = s.formats.split_whitespace().collect();
        for (format, on) in self.pending() {
            formats.retain(|f| *f != format.name());
            if on {
                formats.push(format.name());
            }
        }
        Facts {
            formats: formats.join(" "),
            mixed: s.mixed,
            link: s.link,
            unavailable: s.unavailable,
        }
    }

    /// Undo one step.
    pub fn undo(&mut self) -> Change {
        let Some(prev) = self.undo.pop() else {
            return Change::default();
        };
        self.undo_units -= prev.source.len();
        let current = Snapshot {
            source: std::mem::take(&mut self.source),
            sel: self.sel,
        };
        self.redo.push(current);
        self.restore(prev)
    }

    /// Redo one step.
    pub fn redo(&mut self) -> Change {
        let Some(next) = self.redo.pop() else {
            return Change::default();
        };
        let current = Snapshot {
            source: std::mem::take(&mut self.source),
            sel: self.sel,
        };
        self.keep(current);
        self.restore(next)
    }

    fn restore(&mut self, to: Snapshot) -> Change {
        self.last = None;
        self.source = to.source;
        self.sticky = None;
        self.pending = [None; 4];
        self.restyle();
        self.sel = self.clamp(to.sel);
        Change {
            source: true,
            place: true,
        }
    }

    fn keep(&mut self, snapshot: Snapshot) {
        self.undo_units += snapshot.source.len();
        self.undo.push(snapshot);
        while self.undo.len() > UNDO_STEPS || (self.undo_units > UNDO_UNITS && self.undo.len() > 1)
        {
            let old = self.undo.remove(0);
            self.undo_units -= old.source.len();
        }
    }

    fn push_undo(&mut self, kind: Kind) {
        let coalesce = kind == Kind::Typing
            && self.last == Some(Kind::Typing)
            && self.now - self.last_at < COALESCE_MS;
        if !coalesce {
            self.keep(Snapshot {
                source: self.source.clone(),
                sel: self.sel,
            });
        }
        self.redo.clear();
        self.last = Some(kind);
        self.last_at = self.now;
    }

    fn commit(&mut self, result: Vec<u16>, sel: (u32, u32), sticky: bool, kind: Kind) -> Change {
        let changed = result != self.source;
        if changed {
            self.push_undo(kind);
            self.source = result;
            self.restyle();
        }
        self.sel = self.clamp(sel);
        self.sticky = sticky.then_some(self.sel.0);
        Change {
            source: changed,
            place: true,
        }
    }

    fn commit_reps(&mut self, reps: Vec<Rep>, caret: u32, cleanup: bool, kind: Kind) -> Change {
        let sim = self.simulate(&reps, cleanup);
        let c = sim.caret(caret);
        self.commit(sim.result, (c, c), false, kind)
    }

    /// Apply replacements, then remove syntax that was hidden before and is
    /// visible after (half a bold pair, say): it would read as stray text.
    fn simulate(&self, reps: &[Rep], cleanup: bool) -> Sim {
        let (mut text, first) = splice(&self.source, reps);
        let mut sim = Sim {
            result: Vec::new(),
            orphans: 0,
            maps: Vec::new(),
        };
        let mut survivors: Vec<u32> = if cleanup {
            self.proj
                .hidden_indices()
                .filter_map(|i| first.char(i))
                .collect()
        } else {
            Vec::new()
        };
        sim.maps.push(first);
        for _ in 0..3 {
            if survivors.is_empty() {
                break;
            }
            let styled = md::style(&utf8(&text), None);
            let proj = Projection::new(&text, &styled);
            let orphans: Vec<u32> = survivors
                .iter()
                .copied()
                .filter(|&i| {
                    text.get(i as usize)
                        .is_some_and(|&c| c < 128 && ORPHANS.contains(&(c as u8)))
                        && !proj.is_hidden(i)
                })
                .collect();
            if orphans.is_empty() {
                break;
            }
            sim.orphans += orphans.len();
            let reps: Vec<Rep> = orphans.iter().map(|&i| (i, i + 1, Vec::new())).collect();
            let (next, map) = splice(&text, &reps);
            text = next;
            survivors = survivors.iter().filter_map(|&i| map.char(i)).collect();
            sim.maps.push(map);
        }
        sim.result = text;
        sim
    }

    /// A shared edit command against the current source.
    fn edit(&self, name: &str, argument: &str, (a, b): (u32, u32)) -> Option<Edit16> {
        let command = md::wire::command(name, argument)?;
        let range = Range::new(a, b);
        if md::selection(&self.utf8, range)
            .unavailable
            .split_whitespace()
            .any(|s| s == name)
        {
            return None;
        }
        let edit = md::edit(&self.utf8, range, command);
        Some(Edit16 {
            reps: edit
                .replacements
                .into_iter()
                .map(|(r, t)| (r.start, r.end, utf16(&t)))
                .collect(),
            sel: (edit.selection.start, edit.selection.end),
        })
    }

    /// A link at a caret: its host name as the label, linked by the shared
    /// command so its target is escaped as any other link's is.
    fn insert_link(&mut self, url: &str) -> Change {
        if url.is_empty() {
            return Change::default();
        }
        let at = self.sel.0;
        let bare = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url);
        let label = utf16(bare.split('/').next().unwrap_or(bare));
        let (with_label, _) = splice(&self.source, &[(at, at, label.clone())]);
        let range = Range::new(at, at + label.len() as u32);
        let edit = md::edit(&utf8(&with_label), range, md::Command::Link(url.into()));
        let reps: Vec<Rep> = edit
            .replacements
            .into_iter()
            .map(|(r, t)| (r.start, r.end, utf16(&t)))
            .collect();
        let (result, map) = splice(&with_label, &reps);
        let end = map.caret(range.end);
        self.commit(result, (end, end), false, Kind::Edit)
    }

    fn insert(&mut self, text: &[u16], from: u32, to: u32) -> Change {
        if from != to {
            return self.replace_visible(from, to, text);
        }
        if self.pending().next().is_some() {
            return self.insert_pending(text, from);
        }
        let c = self.proj.cluster(from);
        // (replacements, caret, sticky), best first.
        let mut candidates: Vec<(Vec<Rep>, u32, bool)> =
            vec![(vec![(from, from, text.to_vec())], from, false)];
        let core = trimmed(text);
        if core < text.len() && core > 0 && c.1 > from {
            let split = vec![
                (from, from, text[..core].to_vec()),
                (c.1, c.1, text[core..].to_vec()),
            ];
            candidates.push((split, c.1, true));
        }
        if c.1 > from {
            candidates.push((vec![(c.1, c.1, text.to_vec())], c.1, true));
        }
        if c.0 < from {
            candidates.push((vec![(c.0, c.0, text.to_vec())], c.0, false));
        }
        for (reps, caret, sticky) in &candidates {
            let sim = self.simulate(reps, true);
            if sim.orphans == 0 {
                let caret = sim.caret(*caret);
                return self.commit(sim.result, (caret, caret), *sticky, Kind::Typing);
            }
        }
        // No clean placement: as typed, its syntax showing until it completes.
        let sim = self.simulate(&candidates[0].0, false);
        let caret = sim.caret(from);
        self.commit(sim.result, (caret, caret), false, Kind::Typing)
    }

    fn insert_pending(&mut self, text: &[u16], p: u32) -> Change {
        let (mut open, mut close, mut place, mut stick) = (Vec::new(), Vec::new(), p, false);
        let pending: Vec<_> = self.pending().collect();
        self.pending = [None; 4];
        for (format, on) in pending {
            let m = format.marker();
            if on {
                open.extend_from_slice(m);
                close.splice(0..0, m.iter().copied());
                continue;
            }
            // Off inside the format: step past its closer, or close and reopen.
            let c = self.proj.cluster(p);
            if c.1 > c.0 && c.0 == p && self.source[c.0 as usize..c.1 as usize].starts_with(m) {
                place = c.1;
                stick = true;
            } else {
                open.splice(0..0, m.iter().copied());
                close.extend_from_slice(m);
            }
        }
        let inserted = [open.as_slice(), text, close.as_slice()].concat();
        let sim = self.simulate(&[(place, place, inserted)], true);
        let caret = sim.caret(place).saturating_sub(close.len() as u32);
        self.commit(
            sim.result,
            (caret, caret),
            stick && open.is_empty(),
            Kind::Typing,
        )
    }

    fn replace_visible(&mut self, from: u32, to: u32, text: &[u16]) -> Change {
        let parts = self.proj.visible_parts(from, to);
        let Some(&(first, _)) = parts.first() else {
            let at = self.snap(from);
            return self.commit_reps(vec![(at, at, text.to_vec())], at, true, Kind::Edit);
        };
        let mut reps: Vec<Rep> = parts.iter().map(|&(a, b)| (a, b, Vec::new())).collect();
        reps[0].2 = text.to_vec();
        self.commit_reps(reps, first, true, Kind::Edit)
    }

    fn remove(
        &mut self,
        mut from: u32,
        mut to: u32,
        backward: bool,
        target: Option<(u32, u32)>,
    ) -> Change {
        let proj = &self.proj;
        if from == to {
            // At a line's text, Backspace drops its prefix: heading, bullet, quote.
            if let Some(prefix) = proj.prefix_run(from.saturating_sub(1), from) {
                if backward && prefix.end == from {
                    return self.commit_reps(
                        vec![(prefix.start, prefix.end, Vec::new())],
                        prefix.start,
                        false,
                        Kind::Edit,
                    );
                }
            }
            // One visible character in the direction: the platform's own
            // target when it is plain visible text (a whole grapheme), else
            // one code point past any hidden syntax.
            let whole = |(a, b): (u32, u32)| a < b && proj.visible_parts(a, b) == [(a, b)];
            let c = proj.cluster(from);
            let src = &self.source;
            if backward {
                let at = c.0;
                if at == 0 {
                    return Change::default();
                }
                (from, to) = match target {
                    Some(t) if t.1 == at && whole(t) => t,
                    _ => (
                        at - if at >= 2 && text::low(src[at as usize - 1]) {
                            2
                        } else {
                            1
                        },
                        at,
                    ),
                };
            } else {
                let at = c.1;
                if at >= self.len() {
                    return Change::default();
                }
                (from, to) = match target {
                    Some(t) if t.0 == at && whole(t) => t,
                    _ => (
                        at,
                        at + if text::high(src[at as usize]) && at + 1 < self.len() {
                            2
                        } else {
                            1
                        },
                    ),
                };
            }
        }
        let parts = self.proj.visible_parts(from, to);
        if parts.is_empty() {
            return match self.proj.prefix_run(from, to) {
                Some(prefix) => self.commit_reps(
                    vec![(prefix.start, prefix.end, Vec::new())],
                    prefix.start,
                    false,
                    Kind::Edit,
                ),
                None => Change::default(),
            };
        }
        let first = parts[0].0;
        let reps = parts.into_iter().map(|(a, b)| (a, b, Vec::new())).collect();
        self.commit_reps(reps, first, true, Kind::Delete)
    }

    /// Return: continue or end a list or quote. At a caret before hidden
    /// closers, try after them too, and keep whichever leaves no orphans.
    fn newline(&mut self, from: u32, to: u32) -> Change {
        let mut positions = vec![(from, to)];
        let c = self.proj.cluster(from);
        if from == to && c.1 > from {
            positions.push((c.1, c.1));
        }
        let last = positions.len() - 1;
        for (i, &(a, b)) in positions.iter().enumerate() {
            let Some(edit) = self.edit("newline", "", (a, b)) else {
                continue;
            };
            let sim = self.simulate(&edit.reps, true);
            if sim.orphans == 0 || i == last {
                let sel = if sim.orphans == 0 {
                    edit.sel
                } else {
                    (sim.caret(a + 1), sim.caret(a + 1))
                };
                let change = self.commit(sim.result, sel, false, Kind::Edit);
                self.sel = (self.snap(self.sel.0), self.snap(self.sel.1));
                return change;
            }
        }
        Change::default()
    }
}

/// The length of `text` without trailing white space.
fn trimmed(text: &[u16]) -> usize {
    let s = utf8(text);
    s.trim_end_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}')
        .encode_utf16()
        .count()
}
