//! Strings as JavaScript has them (LLP 1088 D1, D2): order, `slice` and
//! `replaceAll` over UTF-16 code units, each result made well formed once.
//!
//! The runner holds Unicode scalar values (Rust's `str`), so a cut that
//! JavaScript would leave as a lone surrogate half is U+FFFD here, as
//! `toWellFormed()` makes it — a declared deviation (LLP 1006 §2). A
//! fragment is never normalized on its own: an empty `find` splits `"😀"`
//! into its halves, and `replaceAll("😀", "", "")` joins them again.

use std::cmp::Ordering;

/// JavaScript's `IsLessThan` for two Strings: UTF-16 code units in order,
/// a proper prefix first, no locale and no normalization (LLP 1088 D1).
/// Rust's `str` order is code-point order, which differs between U+E000…
/// U+FFFF and the astral planes.
pub fn order(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The result would pass the byte bound.
#[derive(Debug, PartialEq, Eq)]
pub struct TooLong;

/// One result built from UTF-16 code units and well-formed pieces, counted
/// as its concatenated, normalized UTF-8 (LLP 1088 D2): a high surrogate is
/// held until the next unit — a low one completing it adds the pair's 4
/// bytes, anything else first settles it as U+FFFD's 3 — and the end
/// settles a held half the same way, so the count is exactly the UTF-8
/// length of the well-formed result, checked before every byte is kept.
struct Built {
    out: String,
    pending: Option<u16>,
    limit: usize,
}

impl Built {
    fn new(limit: usize) -> Self {
        Built {
            out: String::new(),
            pending: None,
            limit,
        }
    }

    fn keep(&mut self, c: char) -> Result<(), TooLong> {
        if self.out.len() + c.len_utf8() > self.limit {
            return Err(TooLong);
        }
        self.out.push(c);
        Ok(())
    }

    /// A held high half that nothing completed.
    fn settle(&mut self) -> Result<(), TooLong> {
        match self.pending.take() {
            Some(_) => self.keep('\u{FFFD}'),
            None => Ok(()),
        }
    }

    /// A well-formed piece: it begins with no low half, so a held high half
    /// before it stays lone.
    fn text(&mut self, s: &str) -> Result<(), TooLong> {
        if s.is_empty() {
            return Ok(());
        }
        self.settle()?;
        if self.out.len() + s.len() > self.limit {
            return Err(TooLong);
        }
        self.out.push_str(s);
        Ok(())
    }

    fn unit(&mut self, u: u16) -> Result<(), TooLong> {
        match u {
            0xD800..=0xDBFF => {
                self.settle()?;
                self.pending = Some(u);
                Ok(())
            }
            0xDC00..=0xDFFF => match self.pending.take() {
                Some(high) => {
                    let c = 0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(u) - 0xDC00);
                    self.keep(char::from_u32(c).unwrap_or('\u{FFFD}'))
                }
                None => self.keep('\u{FFFD}'),
            },
            _ => {
                self.settle()?;
                self.keep(char::from_u32(u32::from(u)).unwrap_or('\u{FFFD}'))
            }
        }
    }

    fn units(&mut self, units: &[u16]) -> Result<(), TooLong> {
        units.iter().try_for_each(|&u| self.unit(u))
    }

    fn finish(mut self) -> Result<String, TooLong> {
        self.settle()?;
        Ok(self.out)
    }
}

/// ECMA-262's ToIntegerOrInfinity, then a relative index clamped to
/// `0..=len` as `slice` clamps it: NaN is 0, a fraction truncates toward
/// zero, a negative index counts from the end.
pub(crate) fn clamp(index: f64, len: usize) -> usize {
    let len_f = len as f64;
    let i = if index.is_nan() { 0.0 } else { index.trunc() };
    if i < 0.0 {
        (len_f + i).max(0.0) as usize
    } else {
        i.min(len_f) as usize
    }
}

/// `String.prototype.slice(start, end)`, well formed (LLP 1088 D2). An
/// omitted `end` is `Number.MAX_VALUE`, which clamps as `undefined` does. A slice
/// never grows its input — a cut half's U+FFFD is 3 bytes of its pair's 4 —
/// so it needs no bound of its own.
pub fn slice(s: &str, start: f64, end: f64) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    let (from, to) = (clamp(start, units.len()), clamp(end, units.len()));
    if from >= to {
        return String::new();
    }
    let mut b = Built::new(usize::MAX);
    let _ = b.units(&units[from..to]);
    b.finish().unwrap_or_default()
}

/// A `replaceAll` replacement, read once: ECMA-262's `GetSubstitution` for
/// a string pattern, which has no captures — `$$` is `$`, `$&` the match,
/// `` $` `` what precedes it, `$'` what follows; `$1`, `$<` and a lone `$`
/// are themselves.
enum Part {
    Text(String),
    Before,
    Match,
    After,
}

fn template(with: &str) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut chars = with.chars().peekable();
    while let Some(c) = chars.next() {
        let part = match (c, chars.peek()) {
            ('$', Some('$')) => {
                chars.next();
                text.push('$');
                continue;
            }
            ('$', Some('&')) => Part::Match,
            ('$', Some('`')) => Part::Before,
            ('$', Some('\'')) => Part::After,
            _ => {
                text.push(c);
                continue;
            }
        };
        chars.next();
        if !text.is_empty() {
            parts.push(Part::Text(std::mem::take(&mut text)));
        }
        parts.push(part);
    }
    if !text.is_empty() {
        parts.push(Part::Text(text));
    }
    parts
}

/// `String.prototype.replaceAll(find, with)` with a string `find`, well
/// formed, in at most `limit` bytes (LLP 1088 D2): every match left to
/// right, none overlapping; an empty `find` matches at every code-unit
/// boundary, between a pair's halves too. The result is built piece by piece
/// through one counter, so a `` $` ``/`$'` replacement that grows it
/// quadratically stops at the bound, before anything past it is kept.
pub fn replace_all(s: &str, find: &str, with: &str, limit: usize) -> Result<String, TooLong> {
    let parts = template(with);
    let mut b = Built::new(limit);
    if find.is_empty() {
        let units: Vec<u16> = s.encode_utf16().collect();
        for at in 0..=units.len() {
            if at > 0 {
                b.unit(units[at - 1])?;
            }
            for part in &parts {
                match part {
                    Part::Text(t) => b.text(t)?,
                    Part::Before => b.units(&units[..at])?,
                    Part::Match => {}
                    Part::After => b.units(&units[at..])?,
                }
            }
        }
        return b.finish();
    }
    // A non-empty `find` is well formed, so its matches begin and end on
    // character boundaries: the same matches as in UTF-16, found in UTF-8.
    let mut end = 0;
    for (at, m) in s.match_indices(find) {
        b.text(&s[end..at])?;
        for part in &parts {
            match part {
                Part::Text(t) => b.text(t)?,
                Part::Before => b.text(&s[..at])?,
                Part::Match => b.text(m)?,
                Part::After => b.text(&s[at + m.len()..])?,
            }
        }
        end = at + m.len();
    }
    b.text(&s[end..])?;
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected values from `bun -e`, `….toWellFormed()`.
    #[test]
    fn slice_is_javascripts_over_code_units() {
        let inf = f64::INFINITY;
        for (s, a, b, want) in [
            ("hello", 1.0, 3.0, "el"),
            ("hello", -3.0, inf, "llo"),
            ("hello", 1.9, -1.2, "ell"),
            ("hello", f64::NAN, 2.0, "he"),
            ("hello", -inf, inf, "hello"),
            ("hello", inf, 2.0, ""),
            ("hello", 3.0, 1.0, ""),
            ("a😀b", 1.0, 2.0, "\u{FFFD}"),
            ("a😀b", 2.0, 4.0, "\u{FFFD}b"),
            ("a😀b", 0.0, -1.0, "a😀"),
            ("ab", -0.0, 1.0, "a"),
            ("calc", 0.0, -1.0, "cal"),
            ("", 0.0, inf, ""),
        ] {
            assert_eq!(slice(s, a, b), want, "{s:?}.slice({a}, {b})");
        }
    }

    #[test]
    fn replace_all_is_javascripts_with_get_substitution() {
        for (s, find, with, want) in [
            ("aXbXc", "X", "-", "a-b-c"),
            ("aaa", "aa", "b", "ba"),
            ("abc", "", "-", "-a-b-c-"),
            ("😀", "", "", "😀"),
            ("😀", "", "-", "-\u{FFFD}-\u{FFFD}-"),
            ("😀😀", "", "", "😀😀"),
            (
                "abc",
                "b",
                "[$&|$`|$'|$$|$1|$<n>|$]",
                "a[b|a|c|$|$1|$<n>|$]c",
            ),
            ("abc", "", "$`", "aababcabc"),
            ("x.y", ".", "$$", "x$y"),
            ("-5", "-", "", "5"),
            ("abc", "d", "x", "abc"),
            ("", "", " ", " "),
        ] {
            assert_eq!(
                replace_all(s, find, with, usize::MAX).as_deref(),
                Ok(want),
                "{s:?}.replaceAll({find:?}, {with:?})"
            );
        }
    }

    /// The bound is the normalized result's UTF-8 length, exactly: a pair
    /// an empty pattern split and rejoined counts 4, a lone half 3.
    #[test]
    fn replace_all_counts_split_pairs_at_the_byte_boundary() {
        assert_eq!(replace_all("😀😀", "", "", 8).as_deref(), Ok("😀😀"));
        assert_eq!(replace_all("😀😀", "", "", 7), Err(TooLong));
        let many = "😀".repeat(100);
        assert_eq!(
            replace_all(&many, "", "", 400).as_deref(),
            Ok(many.as_str())
        );
        assert_eq!(replace_all(&many, "", "", 399), Err(TooLong));
        // Each half alone, then a hyphen: 3 + 1 per half, 1 more at the start.
        assert_eq!(replace_all("😀", "", "-", 9).map(|s| s.len()), Ok(9));
        assert_eq!(replace_all("😀", "", "-", 8), Err(TooLong));
        // The halves `` $` `` carries across pieces: h, h, l, h, l is a lone
        // half and two pairs, 3 + 4 + 4.
        assert_eq!(
            replace_all("😀", "", "$`", 11).as_deref(),
            Ok("\u{FFFD}😀😀")
        );
        assert_eq!(replace_all("😀", "", "$`", 10), Err(TooLong));
        // A slice's half cut at the end is settled there, 3 bytes.
        assert_eq!(slice("😀", 0.0, 1.0).len(), 3);
    }

    /// `` $` `` doubles the string at every boundary: the result is
    /// quadratic, and the bound stops it before it is built.
    #[test]
    fn a_quadratic_substitution_traps_at_the_bound() {
        let s = "x".repeat(10_000);
        assert_eq!(replace_all(&s, "", "$`$'", 1 << 20), Err(TooLong));
    }

    /// Order across U+E000 and U+10000, where code-point order differs.
    #[test]
    fn order_is_code_unit_order() {
        assert_eq!(order("09:30", "10:00"), Ordering::Less);
        assert_eq!(order("\u{E000}", "\u{10000}"), Ordering::Greater);
        assert!("\u{E000}" < "\u{10000}", "Rust's own order differs");
        assert_eq!(order("a", "ab"), Ordering::Less);
        assert_eq!(order("b", "ab"), Ordering::Greater);
        assert_eq!(order("", "a"), Ordering::Less);
        assert_eq!(order("é", "é"), Ordering::Equal);
    }
}
