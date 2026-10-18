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
        Default::default(),
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
    let same = exact_tamber::plan(
        &files,
        &[set("app.tamber", "#000000")],
        &vocab(),
        false,
        Default::default(),
    )
    .unwrap();
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

/// The app's synthetic tokens re-fold with their file: one that changes is
/// in the answer with its new values; an edit after which one no longer
/// folds (the build would refuse it) is rebuild.
#[test]
fn plan_refolds_the_synthetic_tokens_and_a_lost_one_is_rebuild() {
    let files = vec![(
        "app.tamber".to_string(),
        "tamber app\n\ntheme\n  brand = #c8102e\n".to_string(),
    )];
    // As a state's literal: dropped where it does not fold.
    let exprs = vec![("color-mix(in srgb, brand 60%, white)".to_string(), false)];
    let synthetic = exact_tamber::Synthetic {
        file: "app.tamber",
        exprs: &exprs,
        retention: None,
        params: &exact_tamber::params::NONE,
    };
    let set = |value: &str| {
        vec![(
            "app.tamber".to_string(),
            Op::SetToken {
                name: "brand".into(),
                when: Vec::new(),
                value: value.into(),
                at: None,
                comment: None,
            },
        )]
    };
    let live = exact_tamber::plan(&files, &set("#0a7aff"), &vocab(), false, synthetic).unwrap();
    assert_eq!(live.class, exact_tamber::Class::Live);
    assert_eq!(live.synthetic.len(), 1);
    assert_eq!(live.synthetic[0].0, exprs[0].0);
    assert!(live.changed.iter().all(|(_, t)| !t.starts_with('~')));
    // Over a host's colour the mix no longer folds at build.
    let lost = exact_tamber::plan(&files, &set("AccentColor"), &vocab(), false, synthetic).unwrap();
    assert_eq!(lost.class, exact_tamber::Class::Rebuild);
    // And one that comes to fold is a rebuild too: the plan's table gains it.
    let host = vec![(
        "app.tamber".to_string(),
        "tamber app\n\ntheme\n  brand = AccentColor\n".to_string(),
    )];
    let gained = exact_tamber::plan(&host, &set("#0a7aff"), &vocab(), false, synthetic).unwrap();
    assert_eq!(gained.class, exact_tamber::Class::Rebuild);
    // A colour row's own expression must fold: one that does not on either
    // side is the build's refusal, never a live edit.
    let strict = vec![(exprs[0].0.clone(), true)];
    let row = exact_tamber::Synthetic {
        file: "app.tamber",
        exprs: &strict,
        retention: None,
        params: &exact_tamber::params::NONE,
    };
    let refused = exact_tamber::plan(&host, &set("CanvasText"), &vocab(), false, row).unwrap();
    assert_eq!(refused.class, exact_tamber::Class::Rebuild);
    // A profile's colour is the compiler's to resolve: rebuild, either side.
    let profiled = vec![(
        "app.tamber".to_string(),
        "tamber app\n\ntheme\n  brand = #c8102e\n".to_string(),
    )];
    let to_profile = exact_tamber::plan(
        &profiled,
        &set("color(--brand 0.1 0.2 0.3)"),
        &vocab(),
        false,
        synthetic,
    );
    assert!(to_profile.is_err() || to_profile.unwrap().class == exact_tamber::Class::Rebuild);
}

/// LLP 1121 stage 6 B: a length or number value edit is live, as a colour's
/// is; one that changes the token's kind (written or inferred) is a rebuild,
/// since it moves which rows may name it.
#[test]
fn a_length_or_number_edit_is_live_and_a_kind_change_rebuilds() {
    let files = vec![(
        "app.tamber".to_string(),
        "tamber app\n\naxes\n  density = regular | compact\n\ntheme\n  space.row = 12px\n    when density=compact: 8px\n  opacity.faded: number = 0.5\n  gap = 4px\n".to_string(),
    )];
    let none: Vec<(String, bool)> = Vec::new();
    let synthetic = exact_tamber::Synthetic {
        file: "app.tamber",
        exprs: &none,
        retention: None,
        params: &exact_tamber::params::NONE,
    };
    let set = |name: &str, value: &str| {
        vec![(
            "app.tamber".to_string(),
            Op::SetToken {
                name: name.into(),
                when: Vec::new(),
                value: value.into(),
                at: None,
                comment: None,
            },
        )]
    };
    let plan = |ops: Vec<(String, Op)>| {
        exact_tamber::plan(&files, &ops, &vocab(), false, synthetic).unwrap()
    };
    assert_eq!(
        plan(set("space.row", "16px")).class,
        exact_tamber::Class::Live
    );
    assert_eq!(
        plan(set("space.row", "1.25rem")).class,
        exact_tamber::Class::Live
    );
    assert_eq!(
        plan(set("opacity.faded", "0.75")).class,
        exact_tamber::Class::Live
    );
    // An inferred length becoming a colour moves its kind.
    assert_eq!(
        plan(set("gap", "#ffffff")).class,
        exact_tamber::Class::Rebuild
    );
}

/// Stage 6 C: a type token's style, weight, size or line height changing is
/// live; the longhands it writes changing, or its family, or a family
/// token's stack, rebuilds (the lead's ruling, D6).
#[test]
fn a_type_value_edit_is_live_and_a_longhand_or_family_change_rebuilds() {
    let files = vec![(
        "app.tamber".to_string(),
        "tamber app\n\ntheme\n  type.body = 600 -exact-body/1.35 \"InterVariable\"\n  family.mono: family = ui-monospace\n".to_string(),
    )];
    let none: Vec<(String, bool)> = Vec::new();
    let synthetic = exact_tamber::Synthetic {
        file: "app.tamber",
        exprs: &none,
        retention: None,
        params: &exact_tamber::params::NONE,
    };
    let set = |name: &str, value: &str| {
        vec![(
            "app.tamber".to_string(),
            Op::SetToken {
                name: name.into(),
                when: Vec::new(),
                value: value.into(),
                at: None,
                comment: None,
            },
        )]
    };
    let class = |name: &str, value: &str| {
        exact_tamber::plan(&files, &set(name, value), &vocab(), false, synthetic)
            .unwrap()
            .class
    };
    use exact_tamber::Class::{Live, Rebuild};
    assert_eq!(
        class("type.body", "700 -exact-headline/1.5 \"InterVariable\""),
        Live
    );
    assert_eq!(class("type.body", "700 18px/22px \"InterVariable\""), Live);
    assert_eq!(
        class("type.body", "-exact-body/1.35 \"InterVariable\""),
        Rebuild,
        "the weight unsaid"
    );
    assert_eq!(
        class("type.body", "600 -exact-body/1.35 Georgia"),
        Rebuild,
        "another family"
    );
    assert_eq!(class("family.mono", "monospace"), Rebuild);
}

/// D7's retention decides the class (theme2's ruling, LLP 1121 stage 5): a
/// token the plan does not keep moves nothing, one it keeps added, gone or
/// starting to vary moves its retained shape; `exports` bounds what an app
/// with bindings keeps.
#[test]
fn the_class_asks_the_plans_retention() {
    let files = vec![(
        "app.tamber".to_string(),
        "tamber app\n\naxes\n  tone = warm | cool\n\ntheme\n  ink = #111111\n  spare = #222222\n"
            .to_string(),
    )];
    let add = vec![(
        "app.tamber".to_string(),
        Op::AddToken {
            name: "fresh".into(),
            kind: None,
            value: "#123456".into(),
            meta: Vec::new(),
            whens: Vec::new(),
            comment: None,
            before: None,
        },
    )];
    let when = |name: &str| {
        vec![(
            "app.tamber".to_string(),
            Op::SetToken {
                name: name.into(),
                when: vec![("tone".into(), "cool".into())],
                value: "#0a7aff".into(),
                at: None,
                comment: None,
            },
        )]
    };
    let class = |ops: &[(String, Op)], retention: &exact_tamber::Retention| {
        let synthetic = exact_tamber::Synthetic {
            file: "app.tamber",
            exprs: &[],
            retention: Some(retention),
            params: &exact_tamber::params::NONE,
        };
        exact_tamber::plan(&files, ops, &vocab(), false, synthetic)
            .unwrap()
            .class
    };
    use exact_tamber::Class::{Live, Rebuild};
    let plain = exact_tamber::Retention {
        words: ["ink".to_string()].into(),
        ..Default::default()
    };
    assert_eq!(class(&add, &plain), Live, "a token no view names");
    assert_eq!(
        class(&when("spare"), &plain),
        Live,
        "an unkept token starting to vary"
    );
    assert_eq!(
        class(&when("ink"), &plain),
        Rebuild,
        "a kept token starting to vary"
    );
    // A token a colour keyframe names is folded into the plan's keyframes:
    // its value is structure (rt1 round 2), another's is not.
    let value = |name: &str| {
        vec![(
            "app.tamber".to_string(),
            Op::SetToken {
                name: name.into(),
                when: Vec::new(),
                value: "#0a7aff".into(),
                at: None,
                comment: None,
            },
        )]
    };
    let keyframed = exact_tamber::Retention {
        words: ["ink".to_string(), "spare".to_string()].into(),
        folded: ["ink".to_string()].into(),
        ..Default::default()
    };
    assert_eq!(
        class(&value("ink"), &keyframed),
        Rebuild,
        "a keyframe's token"
    );
    assert_eq!(class(&value("spare"), &keyframed), Live, "a row's token");
    assert_eq!(class(&value("ink"), &plain), Live, "no keyframe names it");
    let bound = exact_tamber::Retention {
        bindable: true,
        ..Default::default()
    };
    assert_eq!(class(&add, &bound), Rebuild, "every token kept");
    assert_eq!(class(&when("spare"), &bound), Rebuild);
    let design = exact_tamber::parse(
        "tamber app\n\nexports\n  ink\n\ntheme\n  ink = #111111\n  spare = #222222\n",
    )
    .unwrap();
    assert!(bound.keeps(&design, "ink", None, exact_tamber::Kind::Color));
    assert!(
        !bound.keeps(&design, "spare", None, exact_tamber::Kind::Color),
        "exports bounds it"
    );
    assert!(
        plain.keeps(
            &design,
            "CanvasText",
            Some("CanvasText"),
            exact_tamber::Kind::Color
        ),
        "a role override is always kept"
    );
    // A computed length or number row may name any token of those kinds,
    // a computed colour row any colour token (stage 6 B's rule).
    let measured = exact_tamber::Retention {
        computed_measure: true,
        ..Default::default()
    };
    assert!(measured.keeps(&design, "gap", None, exact_tamber::Kind::Length));
    assert!(!measured.keeps(&design, "spare", None, exact_tamber::Kind::Color));
    // A role override respelled: the plan's row carries the name as written.
    let role = vec![(
        "app.tamber".to_string(),
        "tamber app\n\ntheme\n  CanvasText = #000000\n".to_string(),
    )];
    let respell = vec![
        (
            "app.tamber".to_string(),
            Op::RemoveToken {
                name: "CanvasText".into(),
            },
        ),
        (
            "app.tamber".to_string(),
            Op::AddToken {
                name: "canvastext".into(),
                kind: None,
                value: "#000000".into(),
                meta: Vec::new(),
                whens: Vec::new(),
                comment: None,
                before: None,
            },
        ),
    ];
    let synthetic = exact_tamber::Synthetic {
        file: "app.tamber",
        exprs: &[],
        retention: Some(&plain),
        params: &exact_tamber::params::NONE,
    };
    let planned = exact_tamber::plan(&role, &respell, &vocab(), false, synthetic).unwrap();
    assert_eq!(planned.class, Rebuild, "a role override respelled");
    // A kept token that comes to be a role the web draws in the accent: the
    // JS target writes it apart.
    let accent = vec![(
        "app.tamber".to_string(),
        Op::SetToken {
            name: "ink".into(),
            when: Vec::new(),
            value: "AccentColor".into(),
            at: None,
            comment: None,
        },
    )];
    assert_eq!(
        class(&accent, &plain),
        Rebuild,
        "a kept token taking the accent"
    );
    // Which tuples take the accent, and a value the structure carries whole
    // (a platform colour's custom property), are the plan's too.
    let shaped = |text: &str, ops: &[(String, Op)]| {
        let files = vec![("app.tamber".to_string(), text.to_string())];
        let synthetic = exact_tamber::Synthetic {
            file: "app.tamber",
            exprs: &[],
            retention: Some(&plain),
            params: &exact_tamber::params::NONE,
        };
        exact_tamber::plan(&files, ops, &vocab(), false, synthetic)
            .unwrap()
            .class
    };
    let set = |when: Vec<(String, String)>, value: &str| {
        (
            "app.tamber".to_string(),
            Op::SetToken {
                name: "ink".into(),
                when,
                value: value.into(),
                at: None,
                comment: None,
            },
        )
    };
    let swapped = [
        set(Vec::new(), "CanvasText"),
        set(vec![("tone".into(), "cool".into())], "AccentColor"),
    ];
    assert_eq!(
        shaped(
            "tamber app\n\naxes\n  tone = warm | cool\n\ntheme\n  ink = AccentColor\n    when tone=cool: CanvasText\n",
            &swapped
        ),
        Rebuild,
        "the accent's tuples swapped"
    );
    assert_eq!(
        shaped(
            "tamber app\n\ntheme\n  ink = -exact-platform-color(web var(--ink), #111111)\n",
            &[set(
                Vec::new(),
                "-exact-platform-color(web var(--ink), #222222)"
            )]
        ),
        Rebuild,
        "a value naming a custom property"
    );
}

/// A role override a colour keyframe names by the role's spelling
/// (`-exact-label` for `label`, `canvastext` for `CanvasText`) is folded into
/// the keyframes too: its value edit rebuilds.
#[test]
fn a_keyframes_role_spelling_folds_its_override() {
    for (design, name, word) in [
        (
            "tamber t\n\ntheme\n  label = #101010\n",
            "label",
            "-exact-label",
        ),
        (
            "tamber t\n\ntheme\n  CanvasText = #101010\n",
            "CanvasText",
            "canvastext",
        ),
    ] {
        let files = vec![("app.tamber".to_string(), design.to_string())];
        let ops = vec![(
            "app.tamber".to_string(),
            Op::SetToken {
                name: name.into(),
                when: Vec::new(),
                value: "#0a7aff".into(),
                at: None,
                comment: None,
            },
        )];
        let class = |folded: &[&str]| {
            let retention = exact_tamber::Retention {
                folded: folded.iter().map(|w| w.to_string()).collect(),
                ..Default::default()
            };
            let synthetic = exact_tamber::Synthetic {
                file: "app.tamber",
                exprs: &[],
                retention: Some(&retention),
                params: &exact_tamber::params::NONE,
            };
            exact_tamber::plan(&files, &ops, &vocab(), false, synthetic)
                .unwrap()
                .class
        };
        assert_eq!(
            class(&[word]),
            exact_tamber::Class::Rebuild,
            "{name} by {word}"
        );
        assert_eq!(class(&[]), exact_tamber::Class::Live, "{name}, no keyframe");
    }
}
