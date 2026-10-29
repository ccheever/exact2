//! `paragraphDirs`' `auto` (LLP 1045 D11): each source line's first strong
//! letter outside its Markdown syntax and atoms, decided here for the web and
//! Apple alike. The syntax is the styler's with nothing revealed, so where the
//! caret is changes no line's answer.

use exact_markdown as md;

/// A line with no strong letter: it takes the field's direction.
pub const AUTO_NONE: u8 = 0;
/// A line whose first strong letter is left to right.
pub const AUTO_LTR: u8 = 1;
/// A line whose first strong letter is right to left.
pub const AUTO_RTL: u8 = 2;

/// Each source line's direction under `auto`, one per line feed plus one:
/// [`AUTO_NONE`], [`AUTO_LTR`] or [`AUTO_RTL`]. Passed over: the syntax the
/// styler hides or dims; a list's, a task's and a rule's marker; each link's
/// and image's `[` and `](target)`; a code fence; and atoms. A footnote
/// mark's label is text, as the web editor shows it.
pub fn auto_directions(source: &str, atoms: &md::Atoms) -> Vec<u8> {
    auto_of(source, &md::style_with(source, None, atoms))
}

/// [`auto_directions`] over `styled`, the styling of `source` with nothing
/// revealed.
pub(crate) fn auto_of(source: &str, styled: &md::Styled) -> Vec<u8> {
    let range = |r: &md::Range| (r.start, r.end);
    let mut skip: Vec<(u32, u32)> = styled
        .hidden
        .iter()
        .map(range)
        .chain(
            styled
                .spans
                .iter()
                .filter(|s| s.style & md::MARKER != 0)
                .map(|s| range(&s.range)),
        )
        .chain(
            styled
                .replaced
                .iter()
                .filter(|r| !matches!(r.with, md::Replacement::Footnote(_)))
                .map(|r| range(&r.range)),
        )
        .chain(
            styled
                .paragraphs
                .iter()
                .filter(|p| p.kind == md::ParagraphKind::Fence)
                .map(|p| range(&p.range)),
        )
        .chain(styled.atoms.iter().map(|a| range(&a.range)))
        .chain(
            crate::link_destinations(source, styled)
                .into_iter()
                .flat_map(|[o, oe, c, ce]| [(o, oe), (c, ce)]),
        )
        .collect();
    skip.sort_unstable();
    let (mut out, mut at, mut k) = (vec![AUTO_NONE], 0u32, 0usize);
    for c in source.chars() {
        if c == '\n' {
            out.push(AUTO_NONE);
        } else {
            // Sorted by start: the first range not yet ended decides.
            while k < skip.len() && skip[k].1 <= at {
                k += 1;
            }
            let line = out.last_mut().expect("a line");
            if *line == AUTO_NONE && !(k < skip.len() && skip[k].0 <= at) {
                *line = strong(c);
            }
        }
        at += c.len_utf16() as u32;
    }
    out
}

/// A strong letter's direction: a letter (Unicode Alphabetic) or an explicit
/// mark, right to left for the right-to-left scripts (UAX #9's R and AL);
/// [`AUTO_NONE`] for anything else.
fn strong(c: char) -> u8 {
    match c {
        '\u{200E}' => AUTO_LTR,
        '\u{200F}' | '\u{061C}' => AUTO_RTL,
        c if !c.is_alphabetic() => AUTO_NONE,
        c if matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF) => {
            AUTO_RTL
        }
        _ => AUTO_LTR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(source: &str) -> Vec<u8> {
        auto_directions(source, &md::Atoms::default())
    }

    #[test]
    fn a_line_takes_its_first_strong_letter_and_none_without_one() {
        assert_eq!(
            dirs("Hello\nمرحبا\n42\n"),
            [AUTO_LTR, AUTO_RTL, AUTO_NONE, AUTO_NONE]
        );
        assert_eq!(dirs("React هو مكتبة"), [AUTO_LTR]);
        assert_eq!(dirs("\u{200F}Hello"), [AUTO_RTL]);
    }

    #[test]
    fn syntax_is_passed_over() {
        // A task's box, a link's syntax and its destination.
        assert_eq!(
            dirs("- [x] مهمة\n[1](https://example.test) مرحبا"),
            [AUTO_RTL, AUTO_RTL]
        );
        // An ordered list's number, a quote's marker, a heading's.
        assert_eq!(
            dirs("1. مرحبا\n\n> مرحبا\n\n# مرحبا"),
            [AUTO_RTL, AUTO_NONE, AUTO_RTL, AUTO_NONE, AUTO_RTL]
        );
        // A code fence and its info string; the code is text.
        assert_eq!(dirs("```js\nمرحبا\n```"), [AUTO_NONE, AUTO_RTL, AUTO_NONE]);
    }

    #[test]
    fn a_footnote_marks_label_is_text() {
        assert_eq!(
            dirs("[^n] مرحبا\n\n[^note]: ملاحظة"),
            [AUTO_LTR, AUTO_NONE, AUTO_LTR]
        );
        assert_eq!(dirs("[^1] مرحبا"), [AUTO_RTL]);
    }

    #[test]
    fn an_atom_is_passed_over_and_the_same_token_without_a_face_is_text() {
        let atoms = md::Atoms::parse(
            r#"[{"name":"mention","prefix":"@"}]"#,
            r#"[{"source":"@src/a.ts","label":"a.ts","icon":"","title":""}]"#,
        );
        assert_eq!(
            auto_directions("@src/a.ts اشرح هذا \n@x اكتب", &atoms),
            [AUTO_RTL, AUTO_LTR]
        );
    }
}
