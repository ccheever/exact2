//! Keys and paste, routed as LLP 1101 D6 says.
//!
//! A key goes to the focused node's (or its nearest ancestor's) `key`
//! handler first, then to its default, as on the web: unless the handler
//! called `preventDefault()`, a field edits, a dialog's Escape closes it,
//! `aria-keyshortcuts` presses its node, Tab and the arrows move the focus,
//! and Ctrl-C leaves — so an app that wants Ctrl-C (an agent interrupting
//! a reply) claims it with `preventDefault()`.

use crate::host::{After, Host, Key};
use exact_kernel::{PropId, ViewId};
use exact_plan::EventKind;
use exact_runner::{ControlValue, DataSource, Event};
use unicode_segmentation::UnicodeSegmentation;

impl<D: DataSource> Host<D> {
    fn edit(&mut self, id: ViewId, change: impl FnOnce(&mut Vec<String>, &mut usize)) {
        let mut clusters: Vec<String> =
            self.value(id).graphemes(true).map(str::to_string).collect();
        let mut caret = self.caret.min(clusters.len());
        change(&mut clusters, &mut caret);
        let next = clusters.concat();
        self.caret = caret;
        // `input=` hears every keystroke; `change=` hears the commit (Enter, blur).
        self.dispatch(id, Event::Input(ControlValue::Text(next)));
    }

    fn insert(&mut self, id: ViewId, text: &str) {
        let pieces: Vec<String> = text.graphemes(true).map(str::to_string).collect();
        self.edit(id, |v, at| {
            for (i, p) in pieces.iter().enumerate() {
                v.insert(*at + i, p.clone());
            }
            *at += pieces.len();
        });
    }

    /// A paste: text for the focused field, never keys (a pasted letter
    /// must not fire a shortcut). A single-line field takes line breaks as
    /// spaces.
    pub fn paste(&mut self, text: &str) {
        let Some(f) = self.focus.filter(|f| self.is_field(*f)) else {
            return;
        };
        let text: String = text
            .chars()
            .filter_map(|c| match c {
                '\r' => None,
                '\n' if !self.is_textarea(f) => Some(' '),
                '\t' => Some(' '),
                c if c.is_control() && c != '\n' => None,
                c => Some(c),
            })
            .collect();
        self.insert(f, &text);
    }

    /// The node whose `aria-keyshortcuts` names this chord, among the
    /// focusable (and so displayed, enabled, in the open layer) nodes.
    fn shortcut(&self, chord: &str) -> Option<ViewId> {
        let kernel = self.kernel();
        self.focusables().into_iter().find(|id| {
            kernel.node(*id).is_some_and(|n| {
                n.props
                    .str(PropId::AccessibilityKeyShortcuts)
                    .is_some_and(|s| s.split_whitespace().any(|k| k.eq_ignore_ascii_case(chord)))
            })
        })
    }

    /// Field editing; `true` when the key was the field's.
    fn field_key(&mut self, f: ViewId, key: &Key) -> bool {
        let count = |h: &Self| h.value(f).graphemes(true).count();
        match key {
            Key::Char(c) => {
                self.insert(f, &c.to_string());
            }
            Key::Named("Backspace") => self.edit(f, |v, at| {
                if *at > 0 {
                    v.remove(*at - 1);
                    *at -= 1;
                }
            }),
            Key::Named("Delete") | Key::Ctrl('d') if !self.value(f).is_empty() => {
                self.edit(f, |v, at| {
                    if *at < v.len() {
                        v.remove(*at);
                    }
                })
            }
            Key::Ctrl('u') => self.edit(f, |v, at| {
                v.drain(..*at);
                *at = 0;
            }),
            Key::Ctrl('w') => self.edit(f, |v, at| {
                let mut start = *at;
                while start > 0 && v[start - 1].trim().is_empty() {
                    start -= 1;
                }
                while start > 0 && !v[start - 1].trim().is_empty() {
                    start -= 1;
                }
                v.drain(start..*at);
                *at = start;
            }),
            Key::Named("ArrowLeft") | Key::Ctrl('b') => {
                self.caret = self.caret.saturating_sub(1);
                self.changed();
            }
            Key::Named("ArrowRight") | Key::Ctrl('f') => {
                self.caret = (self.caret + 1).min(count(self));
                self.changed();
            }
            Key::Named("Home") | Key::Ctrl('a') => {
                self.caret = 0;
                self.changed();
            }
            Key::Named("End") | Key::Ctrl('e') => {
                self.caret = count(self);
                self.changed();
            }
            Key::Chord(c) if c.ends_with("+Enter") && self.is_textarea(f) => self.insert(f, "\n"),
            Key::Ctrl('j') if self.is_textarea(f) => self.insert(f, "\n"),
            Key::Named("Enter") if self.is_textarea(f) => self.insert(f, "\n"),
            Key::Named("Enter") => {
                if self.runner.handlers_of(f).contains(&EventKind::Submit) {
                    self.dispatch(f, Event::Submit);
                    self.caret = count(self);
                }
            }
            _ => return false,
        }
        true
    }

    /// A key, routed as the module comment says.
    pub fn key(&mut self, key: Key) -> After {
        let chord = key.chord();
        let focus = self.focus;
        // The focused node's (or its nearest ancestor's) `key` handler first.
        self.prevented = false;
        if let Some(target) = focus.and_then(|f| self.handler(f, EventKind::Key)) {
            self.dispatch(target, Event::key(&chord));
        }
        if self.quit {
            return After::Quit;
        }
        if std::mem::take(&mut self.prevented) {
            return After::Continue;
        }
        if let Some(f) = focus.filter(|f| self.is_field(*f)) {
            if self.field_key(f, &key) {
                return After::Continue;
            }
        }
        if let Some((top, _)) = self.layers.last().copied() {
            if key == Key::Named("Escape") {
                let none = self
                    .kernel()
                    .node(top)
                    .and_then(|n| n.props.str(PropId::Closedby).map(str::to_string));
                if none.as_deref() != Some("none") {
                    self.close_layer(top);
                }
                return After::Continue;
            }
        }
        if let Some(id) = self.shortcut(&chord) {
            if self.is_field(id) {
                self.focus(Some(id));
            } else {
                self.press(id);
            }
            return After::Continue;
        }
        match key {
            // Ctrl-\ always leaves; Ctrl-C and Ctrl-D leave when nothing took them.
            Key::Ctrl('\\') => return After::Quit,
            Key::Ctrl('c') | Key::Ctrl('d') => return After::Quit,
            Key::Named("Tab") | Key::Named("ArrowDown") => self.step_focus(false),
            Key::BackTab | Key::Named("ArrowUp") => self.step_focus(true),
            Key::Named("Enter") | Key::Char(' ') => {
                if let Some(f) = focus {
                    self.press(f);
                }
            }
            Key::Named("Escape") => self.focus(None),
            Key::Named("PageDown") | Key::Named("PageUp") => {
                let rows = self.rows as i32 - 2;
                let sign = if key == Key::Named("PageDown") { 1 } else { -1 };
                let target = self.frame().scrollers.first().map(|(_, r, _)| (r.x, r.y));
                if let Some((x, y)) = target {
                    self.wheel(x, y, sign * rows);
                }
            }
            _ => {}
        }
        After::Continue
    }
}
