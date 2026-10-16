//! Edit operations (LLP 1121 stage 3, commit 2): every operation on the
//! clone's formatted design is a small diff in fmt's layout, and its inverse
//! gives the bytes back; a refused edit says why.

use common::vocab;
use exact_tamber::{apply, apply_all, format, undo, Op};

use crate::common;

fn clone() -> String {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fmt/bluesky-clone.tamber"
    ))
    .unwrap();
    format(&src).unwrap()
}

/// The lines `a` and `b` differ in: removed from `a`, added in `b`.
fn diff(a: &str, b: &str) -> (Vec<String>, Vec<String>) {
    let (x, y): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let start = x.iter().zip(&y).take_while(|(p, q)| p == q).count();
    let end = x[start..]
        .iter()
        .rev()
        .zip(y[start..].iter().rev())
        .take_while(|(p, q)| p == q)
        .count();
    (
        x[start..x.len() - end]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        y[start..y.len() - end]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    )
}

/// `op` on `src`: its diff, and that its inverse restores `src` exactly and
/// the result is in fmt's layout.
fn round_trip(src: &str, op: Op) -> (Vec<String>, Vec<String>) {
    let vocab = vocab();
    let edited = apply(src, &op, &vocab).unwrap_or_else(|e| panic!("{op:?}: {e:?}"));
    // A formatted design stays formatted.
    if format(src).unwrap() == src {
        assert_eq!(
            format(&edited.text).unwrap(),
            edited.text,
            "{op:?}: not in fmt's layout"
        );
    }
    let back = undo(&edited.text, &edited.inverse, &vocab)
        .unwrap_or_else(|e| panic!("{op:?} undone: {e:?}"));
    assert_eq!(
        back.text, src,
        "{op:?}: the inverse did not restore the bytes"
    );
    diff(src, &edited.text)
}

fn conds(c: &[(&str, &str)]) -> Vec<(String, String)> {
    c.iter()
        .map(|(a, v)| (a.to_string(), v.to_string()))
        .collect()
}

#[test]
fn each_operation_on_the_clone_is_a_small_diff_and_undoes() {
    let src = clone();
    let one = |op: Op| {
        let (removed, added) = round_trip(&src, op.clone());
        assert!(
            removed.len() <= 1 && added.len() <= 1,
            "{op:?}: -{removed:?} +{added:?}"
        );
        (removed, added)
    };
    let (_, added) = one(Op::SetToken {
        name: "contrast_0".into(),
        when: vec![],
        value: "light-dark(#fefefe, #151d28)".into(),
        at: None,
        comment: None,
    });
    assert_eq!(added, ["  contrast_0 = light-dark(#fefefe, #151d28)"]);
    // A `when` line set, added (and undone by removing it), removed.
    one(Op::SetToken {
        name: "contrast_0".into(),
        when: conds(&[("dark-style", "black")]),
        value: "#010101".into(),
        at: None,
        comment: None,
    });
    let (removed, added) = one(Op::SetToken {
        name: "negative".into(),
        when: conds(&[("dark-style", "black")]),
        value: "#e91646".into(),
        at: None,
        comment: None,
    });
    assert!(
        removed.is_empty() && added == ["    when dark-style=black: #e91646"],
        "{added:?}"
    );
    let (removed, _) = one(Op::UnsetTokenWhen {
        name: "contrast_0".into(),
        when: conds(&[("dark-style", "black")]),
    });
    assert_eq!(removed, ["    when dark-style=black: #000000"]);
    // A token added at the end and before another; a rename; an axis value.
    one(Op::AddToken {
        name: "ink.fresh".into(),
        kind: None,
        value: "light-dark(#000000, #ffffff)".into(),
        meta: vec![],
        whens: vec![],
        comment: None,
        before: None,
    });
    one(Op::AddToken {
        name: "ink.first".into(),
        kind: None,
        value: "#123456".into(),
        meta: vec!["scheme=fixed".into()],
        whens: vec![],
        comment: Some("// mine".into()),
        before: Some("contrast_0".into()),
    });
    let (removed, added) = round_trip(
        &src,
        Op::RenameToken {
            from: "mute_banner".into(),
            to: "banner.mute".into(),
        },
    );
    assert_eq!((removed.len(), added.len()), (1, 1));
    one(Op::AddAxisValue {
        axis: "dark-style".into(),
        value: "oled".into(),
        before: None,
    });
    one(Op::AddAxisValue {
        axis: "dark-style".into(),
        value: "grey".into(),
        before: Some("black".into()),
    });
}

#[test]
fn a_token_and_its_whens_go_and_come_back_whole() {
    let src = clone();
    let (removed, added) = round_trip(
        &src,
        Op::RemoveToken {
            name: "contrast_25_light".into(),
        },
    );
    assert!(added.is_empty() && removed.len() == 1, "{removed:?}");
    let src = "tamber t\n\naxes\n  tone = soft | hard\n\ntheme\n  a = #000000\n  ink = #ffffff  // the ink\n    when tone=hard: #eeeeee  // harder\n  z = #111111\n";
    let (removed, _) = round_trip(src, Op::RemoveToken { name: "ink".into() });
    assert_eq!(removed.len(), 2);
    round_trip(
        src,
        Op::UnsetTokenWhen {
            name: "ink".into(),
            when: conds(&[("tone", "hard")]),
        },
    );
    round_trip(
        src,
        Op::RemoveAxisValue {
            axis: "tone".into(),
            value: "soft".into(),
        }
        .clone(),
    )
    .0
    .len();
}

#[test]
fn a_dial_is_set_and_comes_back() {
    let src = format(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/corpus/dials-influence.tamber"
        ))
        .unwrap(),
    )
    .unwrap();
    let design = exact_tamber::parse(&src).unwrap();
    let dial = &design.dials[0];
    let (removed, added) = round_trip(
        &src,
        Op::SetDial {
            name: dial.name.value.clone(),
            value: dial.value.value.clone() + " ",
        },
    );
    assert!(removed.len() <= 1 && added.len() <= 1);
}

#[test]
fn an_edit_the_checker_refuses_or_naming_nothing_is_refused() {
    let src = clone();
    let vocab = vocab();
    let refused = |op: Op| apply(&src, &op, &vocab).unwrap_err();
    // Removing a value a `when` names leaves a refused design.
    assert!(!refused(Op::RemoveAxisValue {
        axis: "dark-style".into(),
        value: "black".into()
    })
    .is_empty());
    assert_eq!(
        refused(Op::SetToken {
            name: "nope".into(),
            when: vec![],
            value: "#fff".into(),
            at: None,
            comment: None
        })[0]
            .id,
        "tamber-edit"
    );
    assert_eq!(
        refused(Op::RemoveToken {
            name: "nope".into()
        })[0]
            .id,
        "tamber-edit"
    );
    assert_eq!(
        refused(Op::AddToken {
            name: "negative".into(),
            kind: None,
            value: "#fff".into(),
            meta: vec![],
            whens: vec![],
            comment: None,
            before: None
        })[0]
            .id,
        "tamber-edit"
    );
    assert!(
        !refused(Op::SetToken {
            name: "negative".into(),
            when: vec![],
            value: "light-dark(#000, #fff)".into(),
            at: None,
            comment: None
        })
        .is_empty(),
        "scheme=fixed on two sides"
    );
    assert_eq!(
        refused(Op::RenameToken {
            from: "negative".into(),
            to: "like".into()
        })[0]
            .id,
        "tamber-edit"
    );
}

#[test]
fn the_inverse_restores_any_design_byte_for_byte() {
    // Unformatted, CRLF, comments inside, metadata quoted, a block created.
    let src = "tamber t\r\naxes\r\n  tone =soft|hard\r\ntheme\r\n  ink=#ffffff   label=\"Ink here\"  // the ink\r\n  // about the hard side\r\n    when tone=hard: #eeeeee\r\n  z = #111111\r\n";
    round_trip(src, Op::RemoveToken { name: "ink".into() });
    round_trip(
        src,
        Op::UnsetTokenWhen {
            name: "ink".into(),
            when: conds(&[("tone", "hard")]),
        },
    );
    round_trip(
        src,
        Op::SetToken {
            name: "ink".into(),
            when: vec![],
            value: "#fefefe".into(),
            at: None,
            comment: None,
        },
    );
    round_trip(
        src,
        Op::AddToken {
            name: "after".into(),
            kind: None,
            value: "#000000".into(),
            meta: vec![],
            whens: vec![],
            comment: None,
            before: None,
        },
    );
    round_trip(
        "tamber t\n\n",
        Op::AddToken {
            name: "first".into(),
            kind: None,
            value: "#000000".into(),
            meta: vec![],
            whens: vec![],
            comment: None,
            before: None,
        },
    );
    // A sequence: each inverse applies to the text the next one left.
    let vocab = vocab();
    let ops = [
        Op::AddToken {
            name: "a.b".into(),
            kind: None,
            value: "#000000".into(),
            meta: vec![],
            whens: vec![],
            comment: None,
            before: Some("z".into()),
        },
        Op::SetToken {
            name: "a.b".into(),
            when: conds(&[("tone", "hard")]),
            value: "#010101".into(),
            at: None,
            comment: Some("// new".into()),
        },
        Op::RenameToken {
            from: "a.b".into(),
            to: "a.c".into(),
        },
    ];
    let edited = apply_all(src, &ops, &vocab).unwrap();
    assert_eq!(
        undo(&edited.text, &edited.inverse, &vocab).unwrap().text,
        src
    );
}

#[test]
fn a_rename_touches_only_names_and_a_field_writes_one_line() {
    let src = "tamber t\n\naxes\n  tone = soft | hard\n\ntheme\n  fff = #000000\n  hard = #ffffff\n    when tone=hard: fff\n  ink = light-dark(#fff, fff)\n";
    let vocab = vocab();
    let renamed = apply(
        src,
        &Op::RenameToken {
            from: "fff".into(),
            to: "base".into(),
        },
        &vocab,
    )
    .unwrap()
    .text;
    assert!(
        renamed.contains("light-dark(#fff, base)") && renamed.contains("  base = #000000"),
        "{renamed}"
    );
    let renamed = apply(
        src,
        &Op::RenameToken {
            from: "hard".into(),
            to: "strong".into(),
        },
        &vocab,
    )
    .unwrap()
    .text;
    assert!(
        renamed.contains("when tone=hard: fff") && renamed.contains("tone = soft | hard"),
        "{renamed}"
    );
    let refused = |op: Op| apply(src, &op, &vocab).unwrap_err()[0].id;
    assert_eq!(
        refused(Op::SetToken {
            name: "ink".into(),
            when: vec![],
            value: "#000\n  extra = #fff".into(),
            at: None,
            comment: None
        }),
        "tamber-edit"
    );
    assert_eq!(
        refused(Op::AddAxisValue {
            axis: "tone".into(),
            value: "dim | oled".into(),
            before: None
        }),
        "tamber-edit"
    );
    assert_eq!(
        refused(Op::AddToken {
            name: "x".into(),
            kind: None,
            value: "#000".into(),
            meta: vec![],
            whens: vec![],
            comment: Some("not a comment".into()),
            before: None
        }),
        "tamber-edit"
    );
    // A role override keeps its role's name.
    let role = "tamber t\n\ntheme\n  -exact-label = #101010\n";
    assert_eq!(
        apply(
            role,
            &Op::RenameToken {
                from: "-exact-label".into(),
                to: "brand.ink".into()
            },
            &vocab
        )
        .unwrap_err()[0]
            .id,
        "tamber-edit"
    );
}

#[test]
fn a_token_added_last_goes_after_the_comments_inside_the_block() {
    let src = "tamber t\n\naxes\n  tone = soft | hard\n\ntheme\n  ink = #ffffff\n// a note at the margin\n    when tone=hard: #eeeeee\n  // the end\n\nexports\n  ink\n";
    let vocab = vocab();
    let edited = apply(
        src,
        &Op::AddToken {
            name: "last".into(),
            kind: None,
            value: "#000000".into(),
            meta: vec![],
            whens: vec![],
            comment: None,
            before: None,
        },
        &vocab,
    )
    .unwrap();
    assert!(
        edited
            .text
            .contains("    when tone=hard: #eeeeee\n  // the end\n  last = #000000\n"),
        "{}",
        edited.text
    );
    assert_eq!(
        undo(&edited.text, &edited.inverse, &vocab).unwrap().text,
        src
    );
}

#[test]
fn a_line_added_to_a_crlf_design_ends_as_its_lines_do() {
    let src = "tamber t\r\n\r\ntheme\r\n  ink = #ffffff\r\n";
    let edited = apply(
        src,
        &Op::AddToken {
            name: "z".into(),
            kind: None,
            value: "#000000".into(),
            meta: vec![],
            whens: vec![],
            comment: None,
            before: None,
        },
        &vocab(),
    )
    .unwrap();
    assert_eq!(
        edited.text,
        "tamber t\r\n\r\ntheme\r\n  ink = #ffffff\r\n  z = #000000\r\n"
    );
}

#[test]
fn a_rename_leaves_metadata_alone() {
    let src =
        "tamber t\n\ntheme\n  fixed = #000000\n  ink = #ffffff  scheme=fixed\n  edge = fixed\n";
    let renamed = apply(
        src,
        &Op::RenameToken {
            from: "fixed".into(),
            to: "base".into(),
        },
        &vocab(),
    )
    .unwrap()
    .text;
    assert_eq!(
        renamed,
        "tamber t\n\ntheme\n  base = #000000\n  ink = #ffffff  scheme=fixed\n  edge = base\n"
    );
}

#[test]
fn a_restore_is_an_inverse_and_a_value_carries_nothing_after_it() {
    let src = "tamber t\n\ntheme\n  ink = #ffffff  scheme=fixed\n";
    let vocab = vocab();
    let restore = Op::Restore {
        line: 1,
        remove: 1,
        lines: vec!["tamber u".into()],
        newline: true,
    };
    assert_eq!(
        apply(src, &restore, &vocab).unwrap_err()[0].id,
        "tamber-edit"
    );
    let huge = Op::Restore {
        line: u32::MAX,
        remove: 3,
        lines: vec![],
        newline: true,
    };
    assert_eq!(undo(src, &[huge], &vocab).unwrap_err()[0].id, "tamber-edit");
    for value in [
        "light-dark(#000, #fff) //",
        "#000000  scheme=fixed",
        "#000 in 0..1",
    ] {
        let op = Op::SetToken {
            name: "ink".into(),
            when: vec![],
            value: value.into(),
            at: None,
            comment: None,
        };
        assert_eq!(
            apply(src, &op, &vocab).unwrap_err()[0].id,
            "tamber-edit",
            "{value}"
        );
    }
    let op = Op::AddToken {
        name: "x".into(),
        kind: None,
        value: "#000000".into(),
        meta: vec!["label=\"x\" scheme=fixed".into()],
        whens: vec![],
        comment: None,
        before: None,
    };
    assert_eq!(apply(src, &op, &vocab).unwrap_err()[0].id, "tamber-edit");
    let quoted = Op::AddToken {
        name: "x".into(),
        kind: None,
        value: "#000000".into(),
        meta: vec!["label=\"two words\"".into()],
        whens: vec![],
        comment: None,
        before: None,
    };
    apply(src, &quoted, &vocab).unwrap();
}

#[test]
fn a_rename_that_would_fold_differently_is_refused_and_a_role_is_found_by_its_spelling() {
    let vocab = vocab();
    let src = "tamber t\n\ntheme\n  r = #ff0000\n  ink = rgb(from #123456 r g b)\n";
    assert_eq!(
        apply(
            src,
            &Op::RenameToken {
                from: "r".into(),
                to: "b".into()
            },
            &vocab
        )
        .unwrap_err()[0]
            .id,
        "tamber-edit"
    );
    // A role override, declared `label`, by the IR's `-exact-label`; never renamed, even in a design that does not check.
    let role = "tamber t\n\ntheme\n  label = #101010\n";
    apply(
        role,
        &Op::SetToken {
            name: "-exact-label".into(),
            when: vec![],
            value: "#202020".into(),
            at: None,
            comment: None,
        },
        &vocab,
    )
    .unwrap();
    let broken = "tamber t\n\ntheme\n  label = #101010\n  ink = missing\n";
    assert_eq!(
        apply(
            broken,
            &Op::RenameToken {
                from: "label".into(),
                to: "missing".into()
            },
            &vocab
        )
        .unwrap_err()[0]
            .line,
        4
    );
    // A token called `when` renames.
    let when = "tamber t\n\ntheme\n  when = #000000\n  ink = when\n";
    assert_eq!(
        apply(
            when,
            &Op::RenameToken {
                from: "when".into(),
                to: "base".into()
            },
            &vocab
        )
        .unwrap()
        .text,
        "tamber t\n\ntheme\n  base = #000000\n  ink = base\n"
    );
}

#[test]
fn removing_a_token_keeps_fmts_blanks_and_names_find_roles_as_the_checker_does() {
    let src = "tamber t\n\ntheme\n  a = #000000\n\n  b = #111111\n\n  c = #222222\n";
    let vocab = vocab();
    for name in ["a", "b"] {
        let out = apply(src, &Op::RemoveToken { name: name.into() }, &vocab)
            .unwrap()
            .text;
        assert_eq!(format(&out).unwrap(), out, "{name}: {out}");
    }
    // A system role by any case of its name; a plain token by no `-exact-`.
    let roles = "tamber t\n\ntheme\n  highlight = #000000\n  ink = #ffffff\n";
    apply(
        roles,
        &Op::SetToken {
            name: "Highlight".into(),
            when: vec![],
            value: "#010101".into(),
            at: None,
            comment: None,
        },
        &vocab,
    )
    .unwrap();
    assert!(apply(
        roles,
        &Op::RemoveToken {
            name: "-exact-ink".into()
        },
        &vocab
    )
    .is_err());
    // A when names each axis once.
    let when = "tamber t\n\naxes\n  tone = soft | hard\n\ntheme\n  ink = #ffffff\n    when tone=soft: #000000\n";
    let twice = Op::UnsetTokenWhen {
        name: "ink".into(),
        when: vec![
            ("tone".into(), "soft".into()),
            ("tone".into(), "soft".into()),
        ],
    };
    assert!(apply(when, &twice, &vocab).is_err());
    // A keyword is never renamed; an edit's inverse restores a design that did not check.
    let kw = "tamber t\n\ntheme\n  r = #ff0000\n  ink = #000000\n";
    assert!(apply(
        kw,
        &Op::RenameToken {
            from: "r".into(),
            to: "red".into()
        },
        &vocab
    )
    .is_err());
    let broken = "tamber t\n\ntheme\n  ink = missing\n";
    let fixed = apply(
        broken,
        &Op::SetToken {
            name: "ink".into(),
            when: vec![],
            value: "#000000".into(),
            at: None,
            comment: None,
        },
        &vocab,
    )
    .unwrap();
    assert_eq!(
        undo(&fixed.text, &fixed.inverse, &vocab).unwrap().text,
        broken
    );
}

#[test]
fn a_plan_counts_its_hunk_names_each_change_by_file_and_skips_a_file_left_alone() {
    let app = "tamber a\n\ntheme\n  ink = #000000\n  edge = ink\n".to_string();
    let brand = "tamber b\n\ntheme\n  ink = #101010".to_string();
    let files = [
        ("app.tamber".to_string(), app),
        ("brand.tamber".to_string(), brand),
    ];
    let set = |file: &str, value: &str| {
        (
            file.to_string(),
            Op::SetToken {
                name: "ink".into(),
                when: Vec::new(),
                value: value.into(),
                at: None,
                comment: None,
            },
        )
    };
    let planned = exact_tamber::plan(
        &files,
        &[set("brand.tamber", "#202020"), set("app.tamber", "#111111")],
        &vocab(),
        false,
    )
    .unwrap();
    let changed: Vec<(&str, &str)> = planned
        .changed
        .iter()
        .map(|(f, t)| (f.as_str(), t.as_str()))
        .collect();
    assert_eq!(
        changed,
        [
            ("app.tamber", "edge"),
            ("app.tamber", "ink"),
            ("brand.tamber", "ink")
        ]
    );
    assert!(planned
        .diff
        .contains("@@ -4,1 +4,1 @@\n-  ink = #000000\n+  ink = #111111\n"));
    assert!(planned.diff.contains(
        "-  ink = #101010\n\\ No newline at end of file\n+  ink = #202020\n\\ No newline at end of file\n"
    ));
    // Setting a value it already has writes nothing.
    let same =
        exact_tamber::plan(&files, &[set("app.tamber", "#000000")], &vocab(), false).unwrap();
    assert!(same.files.is_empty() && same.diff.is_empty());
}

#[test]
fn an_axis_goes_after_the_axes_there_are_or_in_a_new_block_after_the_header() {
    let with = "tamber t\n\naxes\n  density = default | compact\n\ntheme\n  ink = #000000\n";
    let add = Op::AddAxis {
        name: "theme".into(),
        values: vec!["default".into(), "blue".into()],
        under_dark: false,
    };
    let done = apply(with, &add, &vocab()).unwrap();
    assert_eq!(done.text, "tamber t\n\naxes\n  density = default | compact\n  theme = default | blue\n\ntheme\n  ink = #000000\n");
    assert_eq!(
        undo(&done.text, &done.inverse, &vocab()).unwrap().text,
        with
    );
    let without = "tamber t\n\ntheme\n  ink = #000000\n";
    let done = apply(without, &add, &vocab()).unwrap();
    assert_eq!(
        done.text,
        "tamber t\n\naxes\n  theme = default | blue\n\ntheme\n  ink = #000000\n"
    );
    assert_eq!(format(&done.text).unwrap(), done.text);
    assert!(apply(&done.text, &add, &vocab()).is_err());
    // The header's own comments stay with it.
    let commented = "tamber t\n// the brand\n\ntheme\n  ink = #000000\n";
    let done = apply(commented, &add, &vocab()).unwrap();
    assert_eq!(
        done.text,
        "tamber t\n// the brand\n\naxes\n  theme = default | blue\n\ntheme\n  ink = #000000\n"
    );
}
