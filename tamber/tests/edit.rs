//! Edit operations (LLP 1121 stage 3, commit 2): every operation on the
//! clone's formatted design is a small diff in fmt's layout, and its inverse
//! gives the bytes back; a refused edit says why.

use common::vocab;
use exact_tamber::{apply, apply_all, format, Op};

mod common;

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
    assert_eq!(
        format(&edited.text).unwrap(),
        edited.text,
        "{op:?}: not in fmt's layout"
    );
    let back = apply_all(&edited.text, &edited.inverse, &vocab)
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
