//! CSS white space collapsing for a paragraph's runs, before shaping.
//!
//! @ref LLP 1053 §0 G5 — native engines shape strings as given; the browser
//! collapses first. One preparation, shared by every native host, so that
//! `white-space: normal` and `nowrap` text reaches CoreText and cosmic-text as
//! Chrome renders it (CSS Text 3 §4.1.1, as Chrome implements it):
//!
//! - tabs and carriage returns are spaces; line feeds are segment breaks;
//! - spaces and tabs around a segment break go; a segment break beside a
//!   zero-width space goes, any other becomes a space (Chrome keeps the space
//!   between two CJK letters too);
//! - a space after a space goes, across run boundaries; the first is kept, in
//!   its own run;
//! - spaces at the start and end of the paragraph go.
//!
//! `pre-line` (`white-space-collapse: preserve-breaks`) keeps every line feed
//! and removes the spaces around it; the rest collapses as above.
//!
//! Only U+0020, U+0009, U+000A and U+000D collapse; no-break, ideographic and
//! other spaces, and form feeds, are text. The walker (`Prepared`) collapses
//! flowed text itself and is handed the source, not this.
//!
//! Every removed character is ASCII, so the offset shift is the same count in
//! UTF-8 bytes and UTF-16 units; [`Collapsed`] maps either way between the
//! collapsed text (what is shaped, selected and copied, as the browser copies
//! it) and the source (what links, lists and the runner address).

use crate::WhiteSpace;

/// A paragraph's runs after collapsing, with the map back to the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collapsed {
    /// Each run's collapsed text, in order; a run may become empty.
    pub runs: Vec<String>,
    /// Where the shift between collapsed and source offsets changes, in
    /// collapsed order: from each point on, `source = collapsed + removed`.
    edits: Vec<Edit>,
}

/// One change of shift, at a collapsed offset (in both encodings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edit {
    /// Collapsed UTF-8 byte offset.
    pub byte: usize,
    /// Collapsed UTF-16 offset.
    pub utf16: usize,
    /// Source units removed before this point (bytes = UTF-16 units: ASCII).
    pub removed: usize,
}

fn collapsible(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r')
}

/// Collapse `runs` as CSS `white-space` does before shaping: `normal` and
/// `nowrap` collapse, `pre-line` keeps line feeds, `pre-wrap` preserves.
/// `None` when nothing changes, which is the common case and allocates nothing.
pub fn collapse<S: AsRef<str>>(runs: &[S], white_space: WhiteSpace) -> Option<Collapsed> {
    let keep_breaks = white_space.preserves_breaks();
    if white_space.preserves() || !needs_collapse(runs, keep_breaks) {
        return None;
    }
    let mut out: Vec<String> = runs
        .iter()
        .map(|r| String::with_capacity(r.as_ref().len()))
        .collect();
    let mut at = Offsets::default();
    // The pending whitespace sequence, each character with its run.
    let mut pending: Vec<(usize, char)> = Vec::new();
    let mut previous: Option<char> = None;
    for (index, run) in runs.iter().enumerate() {
        for ch in run.as_ref().chars() {
            if collapsible(ch) {
                pending.push((index, ch));
                continue;
            }
            if !pending.is_empty() {
                let breaks = pending.iter().any(|&(_, c)| c == '\n');
                let beside_zwsp = breaks && (previous == Some('\u{200b}') || ch == '\u{200b}');
                if keep_breaks && breaks {
                    at.keep_breaks(&pending, &mut out);
                } else if previous.is_none() || beside_zwsp {
                    at.drop(pending.len());
                } else {
                    out[pending[0].0].push(' ');
                    at.byte += 1;
                    at.utf16 += 1;
                    at.drop(pending.len() - 1);
                }
                pending.clear();
            }
            out[index].push(ch);
            at.byte += ch.len_utf8();
            at.utf16 += ch.len_utf16();
            previous = Some(ch);
        }
    }
    if keep_breaks {
        at.keep_breaks(&pending, &mut out);
    } else {
        at.drop(pending.len());
    }
    Some(Collapsed {
        runs: out,
        edits: at.edits,
    })
}

/// The collapsed offset reached, and the edits so far.
#[derive(Default)]
struct Offsets {
    byte: usize,
    utf16: usize,
    removed: usize,
    edits: Vec<Edit>,
}

impl Offsets {
    /// Remove `count` source units here.
    fn drop(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        self.removed += count;
        match self.edits.last_mut() {
            Some(last) if last.byte == self.byte => last.removed = self.removed,
            _ => self.edits.push(Edit {
                byte: self.byte,
                utf16: self.utf16,
                removed: self.removed,
            }),
        }
    }

    /// `pre-line`: keep each line feed of a whitespace sequence, in its run,
    /// and remove the spaces and tabs around them.
    fn keep_breaks(&mut self, pending: &[(usize, char)], out: &mut [String]) {
        for &(run, ch) in pending {
            if ch == '\n' {
                out[run].push('\n');
                self.byte += 1;
                self.utf16 += 1;
            } else {
                self.drop(1);
            }
        }
    }
}

/// Whether any collapsing would change `runs`: a tab or a carriage return, a
/// line feed (unless kept), two spaces in a row (across runs), a space beside a
/// kept line feed, or a space at either end.
fn needs_collapse<S: AsRef<str>>(runs: &[S], keep_breaks: bool) -> bool {
    #[derive(PartialEq)]
    enum Before {
        Start,
        Space,
        Break,
        Text,
    }
    let mut before = Before::Start;
    for run in runs {
        for &b in run.as_ref().as_bytes() {
            before = match b {
                b'\t' | b'\r' => return true,
                b'\n' if !keep_breaks || before == Before::Space => return true,
                b'\n' => Before::Break,
                b' ' if before != Before::Text => return true,
                b' ' => Before::Space,
                _ => Before::Text,
            };
        }
    }
    before == Before::Space
}

impl Collapsed {
    /// The collapsed paragraph as one string.
    pub fn text(&self) -> String {
        self.runs.concat()
    }

    /// The shift changes, in collapsed order.
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }

    fn removed_before(&self, key: impl Fn(&Edit) -> usize, at: usize) -> usize {
        let i = self.edits.partition_point(|e| key(e) <= at);
        if i == 0 {
            0
        } else {
            self.edits[i - 1].removed
        }
    }

    /// A collapsed UTF-8 offset's source offset. Removed characters belong to
    /// the offset after them: the start of the next kept character.
    pub fn source_byte(&self, collapsed: usize) -> usize {
        collapsed + self.removed_before(|e| e.byte, collapsed)
    }

    /// A collapsed UTF-16 offset's source offset, as [`Self::source_byte`].
    pub fn source_utf16(&self, collapsed: usize) -> usize {
        collapsed + self.removed_before(|e| e.utf16, collapsed)
    }

    /// A source UTF-16 offset's collapsed offset; a removed character maps to
    /// the collapsed offset where it would have been.
    pub fn collapsed_utf16(&self, source: usize) -> usize {
        let k = self
            .edits
            .partition_point(|e| e.utf16 + e.removed <= source);
        let removed = k.checked_sub(1).map_or(0, |k| self.edits[k].removed);
        let at = source - removed;
        match self.edits.get(k) {
            Some(next) if at >= next.utf16 => next.utf16,
            _ => at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> String {
        collapse(&[text], WhiteSpace::Normal).map_or_else(|| text.to_string(), |c| c.text())
    }

    #[test]
    fn collapses_as_chrome_renders() {
        // Chrome 153's innerText for each source under `white-space: normal`.
        for (source, rendered) in [
            ("a\r\nb", "a b"),
            ("a\rb", "a b"),
            ("a \n b", "a b"),
            ("中\n文", "中 文"),
            ("a\u{200b}\nb", "a\u{200b}b"),
            ("a\n\u{200b}b", "a\u{200b}b"),
            ("\t a\t\tb ", "a b"),
            ("  lead", "lead"),
            ("trail  ", "trail"),
            ("a\u{a0}\u{a0}b", "a\u{a0}\u{a0}b"),
            ("a\u{c}b", "a\u{c}b"),
            ("x \u{3000} y", "x \u{3000} y"),
            ("   ", ""),
            ("plain words", "plain words"),
        ] {
            assert_eq!(one(source), rendered, "{source:?}");
        }
    }

    #[test]
    fn pre_line_keeps_line_feeds_as_chrome_renders() {
        // Chrome 154's innerText for each source under `white-space: pre-line`.
        let pre_line = |text: &str| {
            collapse(&[text], WhiteSpace::PreLine).map_or_else(|| text.to_string(), |c| c.text())
        };
        for (source, rendered) in [
            ("a    b", "a b"),
            ("a\nb", "a\nb"),
            ("a\n\nb", "a\n\nb"),
            ("a  \n  b", "a\nb"),
            ("  lead\n  mid  \ntrail  ", "lead\nmid\ntrail"),
            ("\nfirst", "\nfirst"),
            ("last\n", "last\n"),
            ("a\rb", "a b"),
            ("a\r\nb", "a\nb"),
            ("a\tb\t\nc", "a b\nc"),
            ("中\n文", "中\n文"),
            ("a\u{200b}\nb", "a\u{200b}\nb"),
            ("a\u{c}b", "a\u{c}b"),
        ] {
            assert_eq!(pre_line(source), rendered, "{source:?}");
        }
        assert!(collapse(&["a\nb", "\n\nc"], WhiteSpace::PreLine).is_none());
        assert!(collapse(&["a  b\n"], WhiteSpace::PreWrap).is_none());
        // Line feeds stay in their runs; offsets map both ways.
        let c = collapse(&["a \n", " \nb "], WhiteSpace::PreLine).unwrap();
        assert_eq!(c.runs, ["a\n", "\nb"]);
        assert_eq!(c.source_utf16(2), 4); // the second line feed
        assert_eq!(c.collapsed_utf16(4), 2);
        assert_eq!(c.source_utf16(3), 5); // b
    }

    #[test]
    fn already_collapsed_text_allocates_nothing() {
        assert_eq!(
            collapse(&["a b", " c"], WhiteSpace::Normal),
            collapse(&["x"], WhiteSpace::Normal)
        );
        assert!(collapse(&["a b", " c"], WhiteSpace::Normal).is_none());
        assert!(collapse(&["a ", "", "b"], WhiteSpace::Normal).is_none());
        assert!(collapse(&[""], WhiteSpace::Normal).is_none());
    }

    #[test]
    fn the_first_space_is_kept_in_its_own_run_across_boundaries() {
        let c = collapse(&["a ", " b", "  ", "c "], WhiteSpace::Normal).unwrap();
        assert_eq!(c.runs, ["a ", "b", " ", "c"]);
        let c = collapse(&["a", "  ", "b"], WhiteSpace::Normal).unwrap();
        assert_eq!(c.runs, ["a", " ", "b"]);
        let c = collapse(&["  ", "a"], WhiteSpace::Normal).unwrap();
        assert_eq!(c.runs, ["", "a"]);
    }

    #[test]
    fn offsets_map_both_ways() {
        // source: "  a \n\t b  é  c  " → "a b é c"
        let source = "  a \n\t b  é  c  ";
        let c = collapse(&[source], WhiteSpace::Normal).unwrap();
        let text = c.text();
        assert_eq!(text, "a b é c");
        let s16: Vec<u16> = source.encode_utf16().collect();
        let c16: Vec<u16> = text.encode_utf16().collect();
        for (i, unit) in c16.iter().enumerate() {
            let at = c.source_utf16(i);
            // A kept space maps to the first whitespace of its sequence.
            let expected = if *unit == b' ' as u16 {
                b" \t\n".iter().any(|b| s16[at] == *b as u16)
            } else {
                s16[at] == *unit
            };
            assert!(expected, "collapsed {i} -> source {at}");
            assert_eq!(c.collapsed_utf16(at), i, "round trip at {i}");
        }
        assert_eq!(c.source_utf16(c16.len()), s16.len());
        // Byte offsets shift by the same counts.
        let b = text.find('é').unwrap();
        assert_eq!(&source[c.source_byte(b)..c.source_byte(b) + 2], "é");
        // A removed character maps to where it would have been.
        assert_eq!(c.collapsed_utf16(0), 0);
        assert_eq!(c.collapsed_utf16(4), 2);
    }
}
