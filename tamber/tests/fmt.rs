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
        let Ok(once) = format(&src) else {
            continue;
        };
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
