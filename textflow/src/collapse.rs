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
//! Only U+0020, U+0009, U+000A and U+000D collapse; no-break, ideographic and
//! other spaces, and form feeds, are text. The walker (`Prepared`) collapses
//! flowed text itself and is handed the source, not this.
//!
//! Every removed character is ASCII, so the offset shift is the same count in
//! UTF-8 bytes and UTF-16 units; [`Collapsed`] maps either way between the
//! collapsed text (what is shaped, selected and copied, as the browser copies
//! it) and the source (what links, lists and the runner address).

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

/// Collapse `runs` as CSS `white-space-collapse: collapse` does. `None` when
/// the text is already collapsed, which is the common case and allocates nothing.
pub fn collapse<S: AsRef<str>>(runs: &[S]) -> Option<Collapsed> {
    if !needs_collapse(runs) {
        return None;
    }
    let mut out: Vec<String> = runs
        .iter()
        .map(|r| String::with_capacity(r.as_ref().len()))
        .collect();
    let mut edits = Vec::new();
    let (mut byte, mut utf16, mut removed) = (0usize, 0usize, 0usize);
    // The pending whitespace sequence: its run, whether it holds a segment
    // break, and its length.
    let mut pending: Option<(usize, bool, usize)> = None;
    let mut previous: Option<char> = None;
    for (index, run) in runs.iter().enumerate() {
        for ch in run.as_ref().chars() {
            if collapsible(ch) {
                let (_, breaks, count) = pending.get_or_insert((index, false, 0));
                *breaks |= ch == '\n';
                *count += 1;
                continue;
            }
            if let Some((run, breaks, count)) = pending.take() {
                let at_start = previous.is_none();
                let beside_zwsp = breaks && (previous == Some('\u{200b}') || ch == '\u{200b}');
                if at_start || beside_zwsp {
                    drop(count, byte, utf16, &mut removed, &mut edits);
                } else {
                    out[run].push(' ');
                    byte += 1;
                    utf16 += 1;
                    drop(count - 1, byte, utf16, &mut removed, &mut edits);
                }
            }
            out[index].push(ch);
            byte += ch.len_utf8();
            utf16 += ch.len_utf16();
            previous = Some(ch);
        }
    }
    if let Some((_, _, count)) = pending {
        drop(count, byte, utf16, &mut removed, &mut edits);
    }
    Some(Collapsed { runs: out, edits })
}

/// Remove `count` source units at a collapsed offset.
fn drop(count: usize, byte: usize, utf16: usize, removed: &mut usize, edits: &mut Vec<Edit>) {
    if count == 0 {
        return;
    }
    *removed += count;
    match edits.last_mut() {
        Some(last) if last.byte == byte => last.removed = *removed,
        _ => edits.push(Edit {
            byte,
            utf16,
            removed: *removed,
        }),
    }
}

/// Whether any collapsing would change `runs`: a tab, a line feed or a
/// carriage return, two spaces in a row (across runs), or a space at either end.
fn needs_collapse<S: AsRef<str>>(runs: &[S]) -> bool {
    let mut previous_space = true; // a leading space collapses away
    let mut any = false;
    for run in runs {
        for &b in run.as_ref().as_bytes() {
            match b {
                b'\t' | b'\n' | b'\r' => return true,
                b' ' if previous_space => return true,
                b' ' => previous_space = true,
                _ => previous_space = false,
            }
            any = true;
        }
    }
    any && previous_space
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
        collapse(&[text]).map_or_else(|| text.to_string(), |c| c.text())
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
    fn already_collapsed_text_allocates_nothing() {
        assert_eq!(collapse(&["a b", " c"]), collapse(&["x"]));
        assert!(collapse(&["a b", " c"]).is_none());
        assert!(collapse(&["a ", "", "b"]).is_none());
        assert!(collapse(&[""]).is_none());
    }

    #[test]
    fn the_first_space_is_kept_in_its_own_run_across_boundaries() {
        let c = collapse(&["a ", " b", "  ", "c "]).unwrap();
        assert_eq!(c.runs, ["a ", "b", " ", "c"]);
        let c = collapse(&["a", "  ", "b"]).unwrap();
        assert_eq!(c.runs, ["a", " ", "b"]);
        let c = collapse(&["  ", "a"]).unwrap();
        assert_eq!(c.runs, ["", "a"]);
    }

    #[test]
    fn offsets_map_both_ways() {
        // source: "  a \n\t b  é  c  " → "a b é c"
        let source = "  a \n\t b  é  c  ";
        let c = collapse(&[source]).unwrap();
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
