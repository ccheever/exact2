//! A keyword that only structures a file is still a name where the grammar
//! expects one, and a reserved word is refused only where a name is bound
//! (LLP 1086 D5).

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

/// LLP 1086 D5: the 23 contextual keywords are names at every binder —
/// a prop, a state, a derive, an action and its parameters, a `fn` and its
/// parameters, a `let`, an `each` item and index, a `match` binding, an
/// arrow parameter — and read as names in expressions; `refresh` starts a
/// statement only before a name (kanban F13, shop F6, hn-reader F1,
/// calendar F16).
#[test]
fn every_contextual_word_is_a_name_at_every_binder() {
    const CONTEXTUAL: [&str; 23] = [
        "component",
        "font",
        "shape",
        "style",
        "from",
        "state",
        "derive",
        "resource",
        "mutation",
        "action",
        "task",
        "mount",
        "view",
        "props",
        "provide",
        "inject",
        "slot",
        "children",
        "key",
        "refresh",
        "writes",
        "test",
        "expect",
    ];
    for w in CONTEXTUAL {
        let src = format!(
            r#"fn f{w}({w}: string): string = {w}
shape Item
  name: string
component App
  resource items = loadItems() as shape list<Item>
  state {w} = "a"
  derive d{w} = {w}
  action a{w}({w}: string)
    let l{w} = {w}
    {w} = l{w}
  action {w}2
    {w} = "b"
  view
    column
      Row({w}="x")
      button "go" press=a{w}("x")
      each {w}x, i in map(items, ({w}) => {w}.name) key={w}x
        text {w}x
      match some({w})
        case some(m{w})
          text m{w}
        case none
          text "none"
component Row
  props
    {w}: string
  view
    text f{w}({w})
"#
        );
        let plan = contract::compile(&src);
        assert!(plan.is_ok(), "{w}: {plan:?}");
    }
    // `action refresh`, `press=refresh` and `refresh feed` each have one parse.
    let plan = contract::compile(
        "component App\n  resource feed = loadFeed() as shape list<string>\n  action refresh\n    refresh feed\n  view\n    button \"r\" press=refresh\n",
    );
    assert!(plan.is_ok(), "{plan:?}");
    // A state named `refresh` is assigned, not refreshed.
    let plan = contract::compile(
        "component App\n  state refresh = 0\n  action go\n    refresh = refresh + 1\n  view\n    button \"r\" press=go\n",
    );
    assert!(plan.is_ok(), "{plan:?}");
}

/// The sixteen reserved words are refused at binders, shape names included
/// (`none(value=1)` would read as the literal), and admitted as shape
/// fields, named arguments, members and attribute names (SVG's `in`).
#[test]
fn reserved_words_are_refused_at_binders_and_admitted_as_fields() {
    const RESERVED: [&str; 16] = [
        "when", "if", "else", "each", "in", "match", "case", "as", "fn", "and", "or", "not",
        "true", "false", "none", "some",
    ];
    for w in RESERVED {
        let why = if matches!(w, "true" | "false" | "none" | "some") {
            "it is a literal"
        } else {
            "it shapes an expression"
        };
        let message = format!("`{w}` is reserved in Contract ({why}); choose another name");
        for src in [
            format!("component App\n  state {w} = 1\n  view\n    text \"a\"\n"),
            format!("component App\n  state n = 0\n  action go({w}: number)\n    n = 1\n  view\n    text \"a\"\n"),
            format!("fn f({w}: number): number = 1\ncomponent App\n  view\n    text \"a\"\n"),
            format!("component App\n  view\n    Row(x=1)\ncomponent Row\n  props\n    {w}: number\n  view\n    text \"a\"\n"),
            format!("shape {w}\n  value: number\ncomponent App\n  view\n    text \"a\"\n"),
            format!("component App\n  view\n    Row(x=1)\ncomponent Row\n  inject\n    {w}: number\n  view\n    text \"a\"\n"),
            format!("component App\n  provide\n    {w} = 1\n  view\n    text \"a\"\n"),
        ] {
            let e = contract::compile(&src).unwrap_err();
            assert_eq!(
                (e.id.as_str(), e.message.as_str()),
                ("syntax-expected-name", message.as_str()),
                "{src}"
            );
        }
        let src = format!(
            "shape Flags\n  {w}: number\ncomponent App\n  state f = Flags({w}=1)\n  view\n    text toString(f.{w})\n"
        );
        let plan = contract::compile(&src);
        assert!(plan.is_ok(), "{w}: {plan:?}");
    }
    let e = contract::compile(
        "shape Box\n  value: number\ncomponent App\n  state b = none(value=1)\n  view\n    text \"a\"\n",
    )
    .unwrap_err();
    assert_eq!(
        (e.id.as_str(), e.message.as_str()),
        (
            "syntax-keyword-as-value",
            "`none` is reserved in Contract (it is a literal), so it names no shape or function"
        )
    );
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
