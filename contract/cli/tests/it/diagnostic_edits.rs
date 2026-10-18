//! LLP 1116.014 D2: the lines builders wrote that failed to compile, each in
//! a minimal contract that otherwise compiles, and each refused with the edit
//! that fixes it leading the message. Taken from the bake-off's Exact2 builds
//! (rounds 13 and 14, LLP 1116.013's comparison). A failure the logs show
//! next joins the list.
//!
//! The edit is also run: each case names what the message tells the builder
//! to write, and that must build, so a message cannot point at a spelling the
//! compiler refuses too.

const HEAD: &str = "shape Item\n  id: string\n  name: string\n\ncomponent App\n  state on = true\n  action go\n    on = not on\n";

/// A component with `state` (whole lines under the component, actions too)
/// and `view` (the lines under a `column`).
fn app(state: &str, view: &str) -> String {
    let view: String = view.lines().map(|l| format!("      {l}\n")).collect();
    format!("{HEAD}{state}  view\n    column testId=\"root\"\n{view}")
}

struct Case {
    /// What the builder wrote: the component's extra lines, and the view.
    wrote: (&'static str, &'static str),
    id: &'static str,
    /// The edit the message leads with.
    leads: &'static str,
    /// The edits that build, as the message tells them.
    fixed: &'static [(&'static str, &'static str)],
}

const WRITES_LIST: &str =
    "  state queue = []\n  action add\n    queue = [Item(id=\"a\", name=\"A\")]\n";
const ITEMS: &str = "  state items = [Item(id=\"a\", name=\"A\")]\n";

const CASES: &[Case] = &[
    // The platform's tint, in the words a builder has for it.
    Case {
        wrote: ("", "text \"a\" color=\"-exact-tint\""),
        id: "lower-attr-value",
        leads: "write `color=\"AccentColor\"`, the accent:",
        fixed: &[("", "text \"a\" color=\"AccentColor\"")],
    },
    Case {
        wrote: ("", "text \"a\" color=\"-exact-accent\""),
        id: "lower-attr-value",
        leads: "write `color=\"AccentColor\"`, the accent:",
        fixed: &[("", "text \"a\" color=\"AccentColor\"")],
    },
    Case {
        wrote: ("", "text \"a\" color=\"-exact-tint-color\""),
        id: "lower-attr-value",
        leads: "write `color=\"AccentColor\"`, the accent:",
        fixed: &[("", "text \"a\" color=\"AccentColor\"")],
    },
    Case {
        wrote: ("", "column background-color=\"-exact-tint-background\"\n  text \"a\""),
        id: "lower-attr-value",
        leads: "write `background-color=\"AccentColor\"` for the full accent, or `\"Highlight\"` for a tinted wash:",
        fixed: &[
            ("", "column background-color=\"AccentColor\"\n  text \"a\""),
            ("", "column background-color=\"Highlight\"\n  text \"a\""),
        ],
    },
    // UIKit's names, which Exact's roles drop the `system-` from.
    Case {
        wrote: ("", "column background-color=\"-exact-system-grouped-background\"\n  text \"a\""),
        id: "lower-attr-value",
        leads: "write `background-color=\"-exact-grouped-background\"`:",
        fixed: &[("", "column background-color=\"-exact-grouped-background\"\n  text \"a\"")],
    },
    Case {
        wrote: ("", "column background-color=\"-exact-system-fill\"\n  text \"a\""),
        id: "lower-attr-value",
        leads: "write `background-color=\"-exact-fill\"`:",
        fixed: &[("", "column background-color=\"-exact-fill\"\n  text \"a\"")],
    },
    Case {
        wrote: ("", "column background-color=\"-exact-system-background\"\n  text \"a\""),
        id: "lower-attr-value",
        leads: "write `background-color=\"-exact-background\"`:",
        fixed: &[("", "column background-color=\"-exact-background\"\n  text \"a\"")],
    },
    // A role in a shorthand.
    Case {
        wrote: ("", "column border=\"1px solid -exact-tint\"\n  text \"a\""),
        id: "lower-css-shorthand",
        leads: "write `border=\"1px solid AccentColor\"`, the accent:",
        fixed: &[("", "column border=\"1px solid AccentColor\"\n  text \"a\"")],
    },
    // A quoted number.
    Case {
        wrote: ("", "text \"a\" font-weight=\"700\""),
        id: "lower-attr-value",
        leads: "write `font-weight=700`:",
        fixed: &[("", "text \"a\" font-weight=700")],
    },
    // `empty()` outside a resource's `else`.
    Case {
        wrote: ("  state items = empty()\n", "text \"a\""),
        id: "type-unknown-function",
        leads: "write `[]` for an empty list; `empty()` is only a resource's `else`",
        fixed: &[(WRITES_LIST, "text \"a\"")],
    },
    // A `for` with no key.
    Case {
        wrote: (ITEMS, "for i in items\n  text i.name"),
        id: "syntax-expected",
        leads: "add `key=i.<its id field>`,",
        fixed: &[(ITEMS, "for i in items key=i.id\n  text i.name")],
    },
    // A section's header and footer, where UIKit draws them.
    Case {
        wrote: ("", "list appearance=\"auto\"\n  section\n    row\n      text \"x\"\n    header\n      text \"late\""),
        id: "lower-grouped-list",
        leads: "move `header` to the section's first line:",
        fixed: &[("", "list appearance=\"auto\"\n  section\n    header\n      text \"late\"\n    row\n      text \"x\"")],
    },
    Case {
        wrote: ("", "list appearance=\"auto\"\n  section\n    if on\n      header\n        text \"h\"\n    row\n      text \"x\""),
        id: "lower-grouped-list",
        leads: "move `header` to the section's first line, outside `if`:",
        fixed: &[
            ("", "list appearance=\"auto\"\n  section\n    header\n      text (on ? \"h\" : \"g\")\n    row\n      text \"x\""),
            ("", "list appearance=\"auto\"\n  if on\n    section\n      header\n        text \"h\"\n      row\n        text \"x\"\n  else\n    section\n      row\n        text \"x\""),
        ],
    },
    Case {
        wrote: ("", "list appearance=\"auto\"\n  section\n    row\n      text \"x\"\n    if on\n      footer\n        text \"f\""),
        id: "lower-grouped-list",
        leads: "move `footer` to the section's last line, outside `if`:",
        fixed: &[("", "list appearance=\"auto\"\n  section\n    row\n      text \"x\"\n    footer\n      text (on ? \"f\" : \"g\")")],
    },
    // `buttonStyle` on a custom face.
    Case {
        wrote: ("", "button press=go buttonStyle=\"filled\"\n  row\n    text \"a\"\n    text \"b\""),
        id: "lower-button-style",
        leads: "a native button shows `text`s and a symbol `image`, not a `row`",
        fixed: &[
            ("", "button press=go buttonStyle=\"filled\"\n  text \"a\""),
            ("", "button press=go\n  row\n    text \"a\"\n    text \"b\""),
            ("", "button press=go appearance=\"none\"\n  row\n    text \"a\"\n    text \"b\""),
        ],
    },
    Case {
        wrote: ("", "button press=go buttonStyle=\"filled\" aria-label=\"Add\"\n  image \"symbol:add\" width=24"),
        id: "lower-button-style",
        leads: "write `font-size=…` for the symbol's size in a native button, not `width`",
        fixed: &[
            ("", "button press=go buttonStyle=\"filled\" aria-label=\"Add\"\n  image \"symbol:add\" font-size=24"),
            ("", "button press=go buttonStyle=\"filled\" aria-label=\"Add\"\n  image \"symbol:add\""),
            ("", "button press=go aria-label=\"Add\"\n  image \"symbol:add\" width=24"),
        ],
    },
    // LLP 1116.018: the refusals whose repairs lost behaviour, each with the
    // edit that keeps it.
    Case {
        wrote: ("", "image src=\"https://example.com/a.png\" aria-label=\"A\" width=48 height=48"),
        id: "lower-attr-tag",
        leads: "write the source as the image's first argument, `image url`",
        fixed: &[("", "image \"https://example.com/a.png\" aria-label=\"A\" width=48 height=48")],
    },
    Case {
        wrote: ("  timer ticker interval=1000 running=on tick=go\n", "text \"a\""),
        id: "syntax-unknown-section",
        leads: "a timer is a `task`: write `task goTimer when on` with `every(1000, go)`",
        fixed: &[("  task goTimer when on\n    every(1000, go)\n", "text \"a\"")],
    },
    Case {
        wrote: ("", "list appearance=\"auto\"\n  section\n    row\n      text \"x\"\n    footer\n      if on\n        text \"On\" testId=\"on\"\n      else\n        text \"Off\""),
        id: "lower-grouped-list",
        leads: "write this `footer`'s 2 texts as one `text` whose words the same conditions choose",
        fixed: &[("", "list appearance=\"auto\"\n  section\n    row\n      text \"x\"\n    footer\n      text (on ? \"On\" : \"Off\")")],
    },
    Case {
        wrote: ("  state bill = \"\"\n  derive amount = parseFloat(bill)\n", "text `${amount}`"),
        id: "type-refused-idiom",
        leads: "write `parseNumber(s)` for `parseFloat`",
        fixed: &[("  state bill = \"\"\n  derive amount = match parseNumber(bill) { case some(n) => n, case none => 0 }\n", "text `${amount}`")],
    },
    Case {
        wrote: ("  state icon = \"\"\n  action pick(x: string, on: bool)\n    if on\n      icon = x\n", "input type=\"radio\" name=\"icon\" value=\"a\" checked=(icon == \"a\") change=pick(\"a\") aria-label=\"A\""),
        id: "type-handler-payload",
        leads: "declare `on: string` in `pick` (it is the radio's `value`): a radio's `change` runs only when it becomes checked",
        fixed: &[
            ("  state icon = \"\"\n  action pick(x: string, on: string)\n    icon = x\n", "input type=\"radio\" name=\"icon\" value=\"a\" checked=(icon == \"a\") change=pick(\"a\") aria-label=\"A\""),
        ],
    },
    Case {
        wrote: ("  state bill = 0\n  action edit(v: number)\n    bill = v\n", "input type=\"number\" value=`${bill}` input=edit aria-label=\"Bill\""),
        id: "type-handler-payload",
        leads: "declare `v: string` in `edit`, and read the number with `match parseNumber(v)`",
        fixed: &[("  state bill = 0\n  action edit(v: string)\n    bill = match parseNumber(v) { case some(n) => n, case none => 0 }\n", "input type=\"number\" value=`${bill}` input=edit aria-label=\"Bill\"")],
    },
    Case {
        wrote: ("", "button press=go buttonStyle=\"bordered\"\n  text \"Front\" role=\"heading\" aria-level=2"),
        id: "lower-button-style",
        leads: "put the `role` on a node beside the button",
        fixed: &[("", "text \"Card\" role=\"heading\" aria-level=2\nbutton press=go buttonStyle=\"bordered\" aria-label=\"Front\"\n  text \"Front\"")],
    },
];

#[test]
fn what_builders_wrote_is_refused_with_the_edit_leading_and_the_edit_builds() {
    for case in CASES {
        let source = app(case.wrote.0, case.wrote.1);
        let error =
            contract::compile(&source).expect_err(&format!("{:?} compiled:\n{source}", case.wrote));
        assert_eq!(error.id, case.id, "{}\n{source}", error.message);
        assert!(
            error.message.starts_with(case.leads),
            "the message should lead with {:?}:\n{}\n{source}",
            case.leads,
            error.message
        );
        for (state, view) in case.fixed {
            let fixed = app(state, view);
            if let Err(e) = contract::compile(&fixed) {
                panic!(
                    "the edit {:?} does not build: {} ({})\n{fixed}",
                    case.leads, e.message, e.id
                );
            }
        }
    }
}

/// A message that names an edit never edits what is not wrong: a role that is
/// valid, a `-exact-` word that is no colour, and a colour nothing is near
/// keep their messages.
#[test]
fn a_value_nothing_names_an_edit_for_keeps_its_rule() {
    let message = |view: &str| contract::compile(&app("", view)).unwrap_err().message;
    let unknown = message("text \"a\" color=\"-exact-text\"");
    assert!(
        unknown.starts_with("choose a role (the roles: `contract vocab color`):"),
        "{unknown}"
    );
    let bleu = message("text \"a\" color=\"bleu\"");
    assert!(
        bleu.starts_with("`color=\"bleu\"` is not a valid `color`: a color is a CSS colour"),
        "{bleu}"
    );
    // A quoted word is not a number to unquote; CSS's two weight keywords
    // are named by their numbers.
    let heavy = message("text \"a\" font-weight=\"heavy\"");
    assert!(heavy.ends_with("expected integer"), "{heavy}");
    let bold = message("text \"a\" font-weight=\"bold\"");
    assert!(bold.starts_with("write `font-weight=700`"), "{bold}");
    // A value the row refuses even as a number is not told to be one.
    let negative = message("text \"a\" line-clamp=\"-1\"");
    assert!(negative.ends_with("expected integer"), "{negative}");
}

/// LLP 1116.018: two top-level nodes are refused with the wrapper that keeps
/// both, named with what they are.
#[test]
fn a_view_of_two_nodes_is_told_to_wrap_both() {
    let two = "component App\n  view\n    text \"Notes\" role=\"heading\" aria-level=1\n    text \"Nothing yet\"\n";
    let error = contract::compile(two).unwrap_err();
    assert_eq!(error.id, "lower-one-root");
    assert!(
        error
            .message
            .starts_with("wrap the view's 2 top-level nodes (`text`, `text`) in one `column`"),
        "{}",
        error.message
    );
    let wrapped = "component App\n  view\n    column\n      text \"Notes\" role=\"heading\" aria-level=1\n      text \"Nothing yet\"\n";
    contract::compile(wrapped).unwrap();
}
