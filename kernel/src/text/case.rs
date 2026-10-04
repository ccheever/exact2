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

/// The case mapping, once linked ([`link`]). Only a wasm artifact reads
/// it: every other build maps directly.
#[cfg(target_arch = "wasm32")]
static LINKED: std::sync::OnceLock<Shown> = std::sync::OnceLock::new();

/// [`TextTransform::apply_after`]'s signature.
#[cfg(target_arch = "wasm32")]
type Shown = for<'t> fn(TextTransform, &'t str, WordBoundary) -> Cow<'t, str>;

/// [`lowercase_bounded`], once linked: the roster's `toLowerCase` reaches
/// the case tables only through this pointer on the web (LLP 1088 D2).
#[cfg(target_arch = "wasm32")]
static LOWERCASE: std::sync::OnceLock<Lowercase> = std::sync::OnceLock::new();

/// [`lowercase_bounded`]'s signature.
pub type Lowercase = fn(&str, usize) -> Option<String>;

/// Link `text-transform`'s case mapping (LLP 1047 D2, linked by use): its
/// Unicode case tables are ~3–6 KiB of a web core, and an app's `<option>`
/// label reaches them through its runs. A web artifact links it when its
/// plan binds the row or calls `toLowerCase` (LLP 1088 D2), and a plan that
/// does either unlinked is refused at boot (D6), so an unlinked artifact
/// never holds a transform but `none`. Native artifacts and the compiler map
/// without it.
pub fn link() {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = LINKED.set(TextTransform::apply_after);
        let _ = LOWERCASE.set(lowercase_bounded);
    }
}

/// The roster's `toLowerCase`, once [`link`]ed on the web, directly
/// elsewhere; `None` when an artifact never linked the case tables (the
/// runner traps — the plan was refused at boot first).
pub fn linked_lowercase() -> Option<Lowercase> {
    #[cfg(target_arch = "wasm32")]
    return LOWERCASE.get().copied();
    #[cfg(not(target_arch = "wasm32"))]
    Some(lowercase_bounded)
}

/// JavaScript's `toLowerCase` (Unicode's default mapping with final
/// sigma, no language tailoring), or `None` when the result would pass
/// `max_bytes` of UTF-8 (LLP 1088 D2). A preflight counts the result first
/// without allocating — each character's mapping, expansion included
/// (`"İ"` grows from 2 bytes to 3); final sigma never changes a length, as
/// `ς` and `σ` are both 2 bytes — and only a result that fits is mapped,
/// whole, so `"ΟΣ"` is `"ος"`: no allocation passes the bound, and the
/// result is always `str::to_lowercase`'s.
pub fn lowercase_bounded(text: &str, max_bytes: usize) -> Option<String> {
    let mut bytes = 0usize;
    for c in text.chars() {
        bytes += c.to_lowercase().map(char::len_utf8).sum::<usize>();
        if bytes > max_bytes {
            return None;
        }
    }
    Some(text.to_lowercase())
}

/// `text` as `transform` shows it in a run: [`TextTransform::apply_after`],
/// on the web once [`link`]ed.
pub(crate) fn shown(transform: TextTransform, text: &str, boundary: WordBoundary) -> Cow<'_, str> {
    if transform == TextTransform::None {
        return Cow::Borrowed(text);
    }
    #[cfg(target_arch = "wasm32")]
    return LINKED.get().map_or(Cow::Borrowed(text), |shown| {
        shown(transform, text, boundary)
    });
    #[cfg(not(target_arch = "wasm32"))]
    transform.apply_after(text, boundary)
}

impl TextTransform {
    /// `text` as this transform shows it. `before` is the paragraph's text
    /// ahead of this run, so `capitalize` sees a word split across two runs
    /// as one word.
    pub fn apply<'a>(self, text: &'a str, before: &str) -> Cow<'a, str> {
        let mut boundary = WordBoundary::default();
        boundary.push(before);
        self.apply_after(text, boundary)
    }

    pub(crate) fn apply_after(self, text: &str, boundary: WordBoundary) -> Cow<'_, str> {
        let mapped = match self {
            TextTransform::None => return Cow::Borrowed(text),
            TextTransform::Uppercase => text.to_uppercase(),
            TextTransform::Lowercase => text.to_lowercase(),
            TextTransform::Capitalize => capitalize(text, boundary),
        };
        if mapped == text {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(mapped)
        }
    }
}

/// Chrome carries the last source character across runs, skipping empty
/// ones. An apostrophe joins letters within a run, but not across its end.
#[derive(Clone, Copy, Default)]
pub(crate) struct WordBoundary(Option<char>);

impl WordBoundary {
    pub(crate) fn push(&mut self, text: &str) {
        if let Some(last) = text.chars().next_back() {
            self.0 = Some(last);
        }
    }
}

/// The first letter of each word in titlecase, everything else untouched.
/// A word is what UAX #29 would keep together for ordinary prose: letters,
/// digits, marks and `_`, joined across an apostrophe, `.`, `:` or `·` that
/// sits between two letters — so "don't", "e.g." and "3d" each stay one word
/// ("3d", as Chromium leaves it) and "hello-world" is two.
fn capitalize(text: &str, boundary: WordBoundary) -> String {
    let mut out = String::with_capacity(text.len());
    let (mut last, mut prior) = (boundary.0, None::<char>);
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

    /// LLP 1088 D2: final sigma decided on the whole string, expansion
    /// counted, the bound exact, and `to_lowercase`'s result when it fits.
    #[test]
    fn lowercase_bounded_is_to_lowercase_within_its_bound() {
        assert_eq!(lowercase_bounded("ΟΣ", 64).as_deref(), Some("ος"));
        assert_eq!(lowercase_bounded("ΟΣ Α", 64).as_deref(), Some("ος α"));
        assert_eq!(lowercase_bounded("İİ", 6).map(|s| s.len()), Some(6));
        assert_eq!(lowercase_bounded("İİ", 5), None);
        assert_eq!(lowercase_bounded("", 0).as_deref(), Some(""));
        for text in ["MiXeD ÀÉ", "ὈΔΥΣΣΕΎΣ İ", "Σ", "AΣ'B", "ΣΑΣ.", "Ⱥ K"] {
            let want = text.to_lowercase();
            assert_eq!(
                lowercase_bounded(text, want.len()),
                Some(want.clone()),
                "{text}"
            );
            assert_eq!(lowercase_bounded(text, want.len() - 1), None, "{text}");
        }
        assert_eq!(
            linked_lowercase().map(|f| f("ABC", 3)),
            Some(Some("abc".into()))
        );
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
        assert_eq!(Capitalize.apply("t stop", "don'"), "T Stop");
        assert_eq!(Capitalize.apply("'t stop", "don"), "'t Stop");
    }
}
