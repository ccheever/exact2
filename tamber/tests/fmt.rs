//! `tamber fmt` (LLP 1121 stage 3, commit 1): every design that parses
//! formats to a fixed point, in one step, with the same IR; comments stay.

use common::vocab;
use exact_tamber::{check, format};

mod common;
use std::path::Path;

fn designs() -> Vec<std::path::PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in [
        "tests/corpus",
        "tests/fmt",
        "../apps/theme-fixture",
        "../host/web-js/conformance/tamber-modes",
    ] {
        for e in std::fs::read_dir(root.join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|e| e == "tamber") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn every_design_formats_to_a_fixed_point_with_the_same_ir() {
    let vocab = vocab();
    let mut formatted = 0;
    for path in designs() {
        let src = std::fs::read_to_string(&path).unwrap();
        // Only a refusal fixture (a `.err` beside it) may fail to parse;
        // every other design checks.
        let refusal = path.with_extension("err").exists();
        let Ok(once) = format(&src) else {
            assert!(refusal, "{}: does not format", path.display());
            continue;
        };
        assert!(
            refusal || check(&src, &vocab).is_ok(),
            "{}: does not check",
            path.display()
        );
        formatted += 1;
        let twice = format(&once).unwrap_or_else(|e| panic!("{}: {e:?}\n{once}", path.display()));
        assert_eq!(once, twice, "{}: fmt of fmt differs", path.display());
        if let Ok(before) = check(&src, &vocab) {
            let after =
                check(&once, &vocab).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
            assert_eq!(before.ir(), after.ir(), "{}: the IR moved", path.display());
        }
        let comments = |s: &str| s.matches("//").count();
        assert_eq!(
            comments(&src),
            comments(&once),
            "{}: a comment was lost",
            path.display()
        );
    }
    assert!(formatted > 10, "only {formatted} designs formatted");
}

#[test]
fn the_canonical_layout() {
    let src = "// The brand.\ntamber t\naxes\n    tone =soft|hard   under scheme = dark\n\n\ndials\n    accent:color= #1083fe in 0 .. 1   step=0.1  label=\"Accent\"\ntheme\n\n    // Grounds.\n    surface.page=light-dark(#fff,  #000)   // the page\n          when tone = hard :  #111\n\n\n    ink:color = #ffffff scheme=fixed\nexports\n  ink\n\n";
    let want = "// The brand.\ntamber t\n\naxes\n  tone = soft | hard  under scheme=dark\n\ndials\n  accent: color = #1083fe  in 0..1  step=0.1 label=\"Accent\"\n\ntheme\n  // Grounds.\n  surface.page = light-dark(#fff,  #000)  // the page\n    when tone=hard: #111\n\n  ink: color = #ffffff  scheme=fixed\n\nexports\n  ink\n";
    assert_eq!(format(src).unwrap(), want);
    assert_eq!(format(want).unwrap(), want);
}

#[test]
fn a_design_that_does_not_parse_has_no_layout() {
    let e = format("theme\n  a = #fff\n").unwrap_err();
    assert_eq!(e[0].id, "tamber-header");
}

#[test]
fn a_comment_keeps_its_place_and_a_final_one_is_stable() {
    // A comment over a `when` stays where it is when the `when` goes: an
    // edit that removes the line is a one-line diff.
    let with = "tamber t\n\ntheme\n  ink = #ffffff\n    // darker\n    when tone=hard: #000000\n  other = #000000\n";
    let without = "tamber t\n\ntheme\n  ink = #ffffff\n    // darker\n  other = #000000\n";
    assert_eq!(format(with).unwrap(), with);
    assert_eq!(format(without).unwrap(), without);
    // A comment closing the file, then blank lines: one layout.
    let tail = "tamber t\n\ntheme\n  ink = #ffffff\n// tail\n\n\n";
    let once = format(tail).unwrap();
    assert_eq!(format(&once).unwrap(), once);
    // A block's last comment stays in the block, above the next one.
    let ending =
        "tamber t\n\ntheme\n  ink = #ffffff\n  // the end of the theme\n\nexports\n  ink\n";
    assert_eq!(format(ending).unwrap(), ending);
}

#[test]
fn the_header_and_line_endings_have_one_layout() {
    assert_eq!(
        format("tamber    t\r\ntheme\r\n  ink=#fff\r\n").unwrap(),
        "tamber t\n\ntheme\n  ink = #fff\n"
    );
}

#[test]
fn a_token_called_when_a_quoted_slash_and_a_comment_between_blocks() {
    let src = "tamber t\n\ntheme\n  when=#ffffff\n  font.family = 'a//b'\n// between\n\nexports\n  when\n";
    let out = format(src).unwrap();
    assert_eq!(out, "tamber t\n\ntheme\n  when = #ffffff\n  font.family = 'a//b'\n\n// between\n\nexports\n  when\n");
    assert_eq!(format(&out).unwrap(), out);
}

#[test]
fn a_single_quoted_value_is_kept_whole() {
    let src = "tamber t\n\ntheme\n  font.family = 'A x=y'\n";
    assert_eq!(format(src).unwrap(), src);
}

#[test]
fn an_apostrophe_in_a_word_is_no_quote() {
    // The word's apostrophe opens no string, so the comment after it is one.
    let src = "tamber t\n\ntheme\n  face = People's  // the face\n";
    assert_eq!(format(src).unwrap(), src);
    let design = exact_tamber::parse(src).unwrap();
    assert_eq!(design.tokens[0].value.value, "People's");
}

#[test]
fn an_axis_value_called_under_is_a_value() {
    for src in [
        "tamber t\n\naxes\n  mode=under|over\n",
        "tamber t\n\naxes\n  mode=over|under\n",
    ] {
        let out = format(src).unwrap();
        assert_eq!(format(&out).unwrap(), out);
        assert_eq!(
            exact_tamber::parse(&out).unwrap().axes[0].values.len(),
            2,
            "{out}"
        );
    }
}
