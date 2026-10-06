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
        let Some(f) = self.focus.filter(|f| self.is_field(*f) && self.armed(*f)) else {
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
        self.focusables()
            .into_iter()
            .filter(|id| self.armed(*id))
            .find(|id| {
                kernel.node(*id).is_some_and(|n| {
                    n.props
                        .str(PropId::AccessibilityKeyShortcuts)
                        .is_some_and(|s| {
                            s.split_whitespace().any(|k| k.eq_ignore_ascii_case(chord))
                        })
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
            Key::Named(dir @ ("ArrowUp" | "ArrowDown")) if self.is_textarea(f) => {
                // By visual line, keeping the column; at the first or last
                // line the caret stays (LLP 1101.001 P15): the arrows never
                // leave the field.
                let up = *dir == "ArrowUp";
                let width = self.cells_of(f).map_or(1, |r| r.w.max(1) as usize);
                let value = self.value(f);
                let lines = visual_lines(&value, width);
                let caret = self.caret.min(lines.last().map_or(0, |l| l.1));
                let at = lines
                    .iter()
                    .rposition(|(start, _)| *start <= caret)
                    .unwrap_or(0);
                let column = caret - lines[at].0;
                let target = if up {
                    at.checked_sub(1)
                } else {
                    (at + 1 < lines.len()).then_some(at + 1)
                };
                if let Some(t) = target {
                    let (start, end) = lines[t];
                    self.caret = (start + column).min(end);
                    self.changed();
                }
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
        // A focused control the screen has not shown yet (a dialog this
        // read opened) takes no typed-ahead key: not its handler, not its
        // text, not its submit (LLP 1101.002 §0 P1).
        let focus = self.focus.filter(|f| self.armed(*f));
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
                    self.dismiss(top);
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
                if let Some(f) = focus.filter(|f| self.armed(*f)) {
                    self.press(f);
                }
            }
            Key::Named("Escape") => self.focus(None),
            Key::Named("PageDown") => self.page(true),
            Key::Named("PageUp") => self.page(false),
            _ => {}
        }
        After::Continue
    }
}

/// A textarea's visual lines as (first cluster, end cluster) pairs, broken
/// at newlines and at `width` columns, as the painter lays them out.
fn visual_lines(value: &str, width: usize) -> Vec<(usize, usize)> {
    use unicode_width::UnicodeWidthStr;
    let mut lines = vec![(0, 0)];
    let mut col = 0;
    for (i, g) in value.graphemes(true).enumerate() {
        if g == "\n" || g == "\r\n" {
            lines.last_mut().expect("a line").1 = i;
            lines.push((i + 1, i + 1));
            col = 0;
            continue;
        }
        let w = g.width().max(1);
        if col + w > width && col > 0 {
            lines.last_mut().expect("a line").1 = i;
            lines.push((i, i));
            col = 0;
        }
        col += w;
        lines.last_mut().expect("a line").1 = i + 1;
    }
    lines
}

#[cfg(test)]
mod tests {
    #[test]
    fn visual_lines_break_at_newlines_and_width() {
        assert_eq!(super::visual_lines("ab\ncd", 10), vec![(0, 2), (3, 5)]);
        assert_eq!(super::visual_lines("abcdef", 4), vec![(0, 4), (4, 6)]);
        assert_eq!(super::visual_lines("", 4), vec![(0, 0)]);
    }
}
