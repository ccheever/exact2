//! `set-param` and `unset-param` (LLP 1121 D15, stage 7 B): the app's value
//! of a component's param, a line of the design's `params` block; each undo
//! gives the bytes back.

use crate::common::vocab;
use exact_tamber::{apply, format, parse, undo, Op};

const BASE: &str =
    "tamber app\n\naxes\n  density = regular | compact\n\ntheme\n  radius.md = 12px\n";

fn set(name: &str, when: &[(&str, &str)], value: &str) -> Op {
    Op::SetParam {
        name: name.into(),
        when: when
            .iter()
            .map(|(a, v)| (a.to_string(), v.to_string()))
            .collect(),
        value: value.into(),
    }
}

#[test]
fn set_param_writes_the_params_block_and_undo_restores_absence() {
    let first = apply(BASE, &set("PostButton.radius", &[], "8px"), &vocab()).unwrap();
    assert_eq!(
        first.text,
        format!("{BASE}\nparams\n  PostButton.radius = 8px\n")
    );
    assert_eq!(
        format(&first.text).unwrap(),
        first.text,
        "in tamber fmt's layout"
    );
    // Undoing the first set restores absence: the component's default again.
    assert_eq!(
        undo(&first.text, &first.inverse, &vocab()).unwrap().text,
        BASE
    );
    // A full name, a `when` line, a changed value: one line each.
    let full = "screens/post.contract#PostButton.fill";
    let second = apply(&first.text, &set(full, &[], "radius.md"), &vocab()).unwrap();
    let third = apply(
        &second.text,
        &set(full, &[("density", "compact")], "4px"),
        &vocab(),
    )
    .unwrap();
    assert!(
        third.text.ends_with(&format!(
            "  PostButton.radius = 8px\n  {full} = radius.md\n    when density=compact: 4px\n"
        )),
        "{}",
        third.text
    );
    let fourth = apply(
        &third.text,
        &set("PostButton.radius", &[], "10px"),
        &vocab(),
    )
    .unwrap();
    assert!(
        fourth.text.contains("  PostButton.radius = 10px\n"),
        "{}",
        fourth.text
    );
    let design = parse(&fourth.text).unwrap();
    assert_eq!(design.params.len(), 2);
    assert_eq!(design.params[1].whens.len(), 1);
    // One `when` line removed; then the whole param; the emptied block goes.
    let fifth = apply(
        &fourth.text,
        &Op::UnsetParam {
            name: full.into(),
            when: vec![("density".into(), "compact".into())],
        },
        &vocab(),
    )
    .unwrap();
    assert!(
        !fifth.text.contains("when density=compact"),
        "{}",
        fifth.text
    );
    let sixth = apply(
        &fifth.text,
        &Op::UnsetParam {
            name: full.into(),
            when: Vec::new(),
        },
        &vocab(),
    )
    .unwrap();
    let seventh = apply(
        &sixth.text,
        &Op::UnsetParam {
            name: "PostButton.radius".into(),
            when: Vec::new(),
        },
        &vocab(),
    )
    .unwrap();
    assert_eq!(seventh.text, BASE);
    assert_eq!(
        undo(&seventh.text, &seventh.inverse, &vocab())
            .unwrap()
            .text,
        sixth.text
    );
}

#[test]
fn a_param_set_is_refused_where_it_cannot_be_written() {
    let refused = |src: &str, op: Op| apply(src, &op, &vocab()).unwrap_err()[0].message.clone();
    let m = refused(
        BASE,
        set("PostButton.radius", &[("density", "compact")], "4px"),
    );
    assert!(
        m.contains("set its value before one of its `when` lines"),
        "{m}"
    );
    let m = refused(BASE, set("radius", &[], "4px"));
    assert!(m.contains("`Component.param`"), "{m}");
    let m = refused(
        BASE,
        Op::UnsetParam {
            name: "PostButton.radius".into(),
            when: Vec::new(),
        },
    );
    assert!(m.contains("no line in the design's `params` block"), "{m}");
    // A kind written on a param's line is the component's to say.
    assert!(parse(&format!(
        "{BASE}\nparams\n  PostButton.radius: length = 8px\n"
    ))
    .is_err());
}

#[test]
fn set_param_finds_a_commented_or_indented_block_and_a_rename_follows_its_values() {
    // A header with a comment, as fmt writes one, and a four-space block.
    let commented =
        format!("{BASE}\nparams  // overrides\n    PostButton.radius = 8px\n    // the row's\n");
    let next = apply(&commented, &set("Row.gap", &[], "1px"), &vocab()).unwrap();
    assert_eq!(
        next.text,
        format!("{BASE}\nparams  // overrides\n    PostButton.radius = 8px\n    // the row's\n    Row.gap = 1px\n")
    );
    assert!(parse(&next.text).is_ok());
    // Removing the last line leaves no header behind.
    let one = format!("{BASE}\nparams  // overrides\n  PostButton.radius = 8px\n");
    let unset = Op::UnsetParam {
        name: "PostButton.radius".into(),
        when: Vec::new(),
    };
    let gone = apply(&one, &unset, &vocab()).unwrap();
    assert_eq!(gone.text, BASE);
    // A token renamed: the values naming it follow; a param's name stays.
    let named = format!(
        "{BASE}\nparams\n  PostButton.radius = radius.md\n    when density=compact: radius.md\n"
    );
    let renamed = apply(
        &named,
        &Op::RenameToken {
            from: "radius.md".into(),
            to: "radius.lg".into(),
        },
        &vocab(),
    )
    .unwrap();
    assert!(
        renamed.text.ends_with(
            "params\n  PostButton.radius = radius.lg\n    when density=compact: radius.lg\n"
        ),
        "{}",
        renamed.text
    );
}
