//! A keyword that only structures a file is still a name where the grammar
//! expects one: a shape field, a prop, a member after `.`, a named argument.

#[test]
fn state_and_key_name_fields_props_members_and_arguments() {
    let plan = contract::compile(
        r#"
shape Step
  key: string
  state: string
shape Steps
  items: list<Step>
component App
  resource steps = steps() as shape Steps
  view
    column
      each s in steps.items key=s.key
        Mark(state=s.state)
component Mark
  props
    state: string
  view
    when state == "done"
      text "done"
    else
      text state
"#,
    );
    assert!(plan.is_ok(), "{plan:?}");
}

#[test]
fn a_keyword_that_shapes_syntax_is_still_refused_as_a_name() {
    let error = contract::compile("shape S\n  when: string\ncomponent App\n  view\n    column\n")
        .unwrap_err();
    assert_eq!(error.id, "syntax-expected-name", "{error:?}");
    let error =
        contract::compile("component App\n  props\n    state: string\n  view\n    text state(1)\n")
            .unwrap_err();
    assert_eq!(error.id, "syntax-keyword-as-value", "{error:?}");
}

/// A negative number in a style is a literal (it was "not a literal", and a
/// pixel string was refused with advice to write the refused form).
#[test]
fn a_style_takes_a_negative_number() {
    let plan = contract::compile(
        "style Tight\n  letter-spacing=-0.204\n  margin-top=-4\ncomponent App\n  view\n    text \"a\" class=Tight\n",
    );
    assert!(plan.is_ok(), "{plan:?}");
}
