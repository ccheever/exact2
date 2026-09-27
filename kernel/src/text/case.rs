//! CSS `text-transform` (CSS Text 3 §2.1): the case mapping a run's string
//! takes before anything measures or paints it.
//!
//! @ref LLP 1064 D5 — applied once, where the kernel produces a paragraph's
//! runs ([`crate::arena::Arena::text_runs`]), so every measurer and every
//! painter that reads runs reads the same string. Full Unicode mappings with
//! no language tailoring, as a browser maps a page with no `lang`: `ß`
//! uppercases to `SS`, a final `Σ` lowercases to `ς`.

use crate::generated::TextTransform;
use crate::NodeRef;
use std::borrow::Cow;

impl<'a> NodeRef<'a> {
    /// The string this `Text` leaf shows, `text-transform` applied as its
    /// paragraph's runs apply it; `None` without a `text` prop.
    pub fn shown_text(&self) -> Option<Cow<'a, str>> {
        self.arena.shown_text(self.slot)
    }
}

impl TextTransform {
    /// `text` as this transform shows it. `before` is the paragraph's text
    /// ahead of this run, so `capitalize` sees a word split across two runs
    /// as one word.
    pub fn apply<'a>(self, text: &'a str, before: &str) -> Cow<'a, str> {
        let mapped = match self {
            TextTransform::None => return Cow::Borrowed(text),
            TextTransform::Uppercase => text.to_uppercase(),
            TextTransform::Lowercase => text.to_lowercase(),
            TextTransform::Capitalize => capitalize(text, before),
        };
        if mapped == text {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(mapped)
        }
    }
}

/// The first letter of each word in titlecase, everything else untouched.
/// A word is what UAX #29 would keep together for ordinary prose: letters,
/// digits, marks and `_`, joined across an apostrophe, `.`, `:` or `·` that
/// sits between two letters — so "don't", "e.g." and "3d" each stay one word
/// ("3d", as Chromium leaves it) and "hello-world" is two.
fn capitalize(text: &str, before: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut behind = before.chars().rev();
    let (mut last, mut prior) = (behind.next(), behind.next());
    for c in text.chars() {
        let joined = match (prior, last) {
            (_, Some(l)) if word(l) => true,
            (Some(p), Some(l)) => joiner(l) && p.is_alphabetic() && c.is_alphabetic(),
            _ => false,
        };
        if c.is_alphabetic() && !joined {
            titlecase(c, &mut out);
        } else {
            out.push(c);
        }
        (prior, last) = (last, Some(c));
    }
    out
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || mark(c)
}

fn joiner(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}' | '.' | ':' | '\u{b7}')
}

/// The combining-mark blocks: a decomposed `é` is one letter.
fn mark(c: char) -> bool {
    matches!(c,
        '\u{300}'..='\u{36f}'
        | '\u{1ab0}'..='\u{1aff}'
        | '\u{1dc0}'..='\u{1dff}'
        | '\u{20d0}'..='\u{20ff}'
        | '\u{fe20}'..='\u{fe2f}')
}

/// Unicode's titlecase mapping: the uppercase one, except where
/// `SpecialCasing.txt`/`UnicodeData.txt` give titlecase its own (the Latin
/// digraphs, `ß` and the ligatures, Armenian ligatures, Greek with
/// ypogegrammeni) or none at all (Georgian, whose uppercase is Mtavruli).
fn titlecase(c: char, out: &mut String) {
    let special: &str = match c {
        '\u{1c4}'..='\u{1c6}' => "\u{1c5}",
        '\u{1c7}'..='\u{1c9}' => "\u{1c8}",
        '\u{1ca}'..='\u{1cc}' => "\u{1cb}",
        '\u{1f1}'..='\u{1f3}' => "\u{1f2}",
        'ß' => "Ss",
        '\u{fb00}' => "Ff",
        '\u{fb01}' => "Fi",
        '\u{fb02}' => "Fl",
        '\u{fb03}' => "Ffi",
        '\u{fb04}' => "Ffl",
        '\u{fb05}' | '\u{fb06}' => "St",
        '\u{587}' => "\u{535}\u{582}",
        '\u{fb13}' => "\u{544}\u{576}",
        '\u{fb14}' => "\u{544}\u{565}",
        '\u{fb15}' => "\u{544}\u{56b}",
        '\u{fb16}' => "\u{54e}\u{576}",
        '\u{fb17}' => "\u{544}\u{56d}",
        '\u{1fb2}' => "\u{1fba}\u{345}",
        '\u{1fb4}' => "\u{386}\u{345}",
        '\u{1fb7}' => "\u{391}\u{342}\u{345}",
        '\u{1fc2}' => "\u{1fca}\u{345}",
        '\u{1fc4}' => "\u{389}\u{345}",
        '\u{1fc7}' => "\u{397}\u{342}\u{345}",
        '\u{1ff2}' => "\u{1ffa}\u{345}",
        '\u{1ff4}' => "\u{38f}\u{345}",
        '\u{1ff7}' => "\u{3a9}\u{342}\u{345}",
        // With ypogegrammeni: the titlecase letter is the prosgegrammeni
        // form, 8 above the small letter in each block; it maps to itself.
        '\u{1f80}'..='\u{1faf}' => {
            out.push(char::from_u32(c as u32 | 8).unwrap_or(c));
            return;
        }
        '\u{1fb3}' | '\u{1fbc}' => "\u{1fbc}",
        '\u{1fc3}' | '\u{1fcc}' => "\u{1fcc}",
        '\u{1ff3}' | '\u{1ffc}' => "\u{1ffc}",
        '\u{10d0}'..='\u{10fa}' | '\u{10fd}'..='\u{10ff}' => {
            out.push(c);
            return;
        }
        _ => {
            out.extend(c.to_uppercase());
            return;
        }
    };
    out.push_str(special);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(transform: TextTransform, text: &str) -> String {
        transform.apply(text, "").into_owned()
    }

    #[test]
    fn full_mappings_without_language_tailoring() {
        use TextTransform::*;
        assert_eq!(t(Uppercase, "straße ǆ ﬁ"), "STRASSE Ǆ FI");
        assert_eq!(t(Lowercase, "ὈΔΥΣΣΕΎΣ İ"), "ὀδυσσεύς i\u{307}");
        assert_eq!(t(None, "MiXeD"), "MiXeD");
        assert!(matches!(Uppercase.apply("ABC 123", ""), Cow::Borrowed(_)));
    }

    #[test]
    fn capitalize_titlecases_the_first_letter_of_each_word() {
        use TextTransform::Capitalize;
        for (text, want) in [
            ("hello world", "Hello World"),
            ("hello-world (again)", "Hello-World (Again)"),
            ("don't stop, e.g. now", "Don't Stop, E.g. Now"),
            ("3d printing", "3d Printing"),
            ("snake_case mIXED", "Snake_case MIXED"),
            ("e\u{301}te\u{301}", "E\u{301}te\u{301}"),
            ("ǆungla ßa ﬁx ᾳ", "ǅungla Ssa Fix ᾼ"),
            ("ქართული", "ქართული"),
        ] {
            assert_eq!(t(Capitalize, text), want, "{text}");
        }
        // A word split across runs is one word; a space ending the run is not.
        assert_eq!(Capitalize.apply("lo there", "hel"), "lo There");
        assert_eq!(Capitalize.apply("there", "hello "), "There");
        assert_eq!(Capitalize.apply("t stop", "don'"), "t Stop");
    }
}
