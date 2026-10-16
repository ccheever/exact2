//! LLP 1116.016 D5: TypeScript that Contract refuses is refused with an edit,
//! and the edit compiles.

/// A component whose derive `c` is `expr`, over a few states.
fn app(expr: &str) -> String {
    format!(
        "shape User\n  id: string\n  name: string\ncomponent App\n  state xs = [1, 2]\n  state s = \"ab\"\n  state a = \"x\"\n  state b = \"y\"\n  state n = 3\n  state label: enum<\"Total: \", \"Count: \"> = \"Total: \"\n  state picked: option<number> = none\n  state users = [User(id=\"1\", name=\"Ada\")]\n  derive c = {expr}\n  view\n    text \"x\"\n"
    )
}

/// The first error `expr` is refused with: its id and message.
fn refused(expr: &str) -> (String, String) {
    let src = app(expr);
    match contract::compile(&src) {
        Ok(_) => panic!("`{expr}` compiled"),
        Err(e) => (e.id.to_string(), e.message.clone()),
    }
}

#[test]
fn each_refused_form_names_an_edit_that_compiles() {
    // (written, the id, words the message holds, the edit it names)
    let rows: &[(&str, &str, &str, &str)] = &[
        (
            "xs[0]",
            "syntax-refused-idiom",
            "`xs.at(i)`, an option",
            "xs.at(0)",
        ),
        (
            "s[0]",
            "syntax-refused-idiom",
            "`s.slice(i, i + 1)`",
            "s.slice(0, 1)",
        ),
        ("a === b", "syntax-refused-idiom", "write `==`", "a == b"),
        ("a !== b", "syntax-refused-idiom", "write `!=`", "a != b"),
        (
            "picked ?? 0",
            "syntax-refused-idiom",
            "`match x {",
            "match picked { case some(v) => v, case none => 0 }",
        ),
        (
            "a.localeCompare(b)",
            "syntax-method-call",
            "`a < b` compares by UTF-16 code unit",
            "a < b",
        ),
        (
            "String(n)",
            "type-refused-idiom",
            "`x.toString()`",
            "n.toString()",
        ),
        (
            "Math.floor(n)",
            "syntax-method-call",
            "write `floor(n)`",
            "floor(n)",
        ),
        (
            "\"Total: \" + n",
            "type-operand",
            "write a template",
            "`Total: ${n}`",
        ),
        (
            "label + n",
            "type-operand",
            "write a template",
            "`${label}${n}`",
        ),
        (
            "n + label",
            "type-operand",
            "write a template",
            "`${n}${label}`",
        ),
        (
            "label.at(0)",
            "type-argument",
            "`s.slice(i, i + 1)`",
            "label.slice(0, 1)",
        ),
        (
            "s.at(0)",
            "type-argument",
            "`s.slice(i, i + 1)`",
            "s.slice(0, 1)",
        ),
        (
            "xs.sort((p, q) => p - q)",
            "syntax-method-call",
            "compute it in the data source",
            "xs",
        ),
        (
            "users.find(u => u.id == \"1\")",
            "syntax-method-call",
            "`first(xs.filter(x => …))`",
            "first(users.filter(u => u.id == \"1\"))",
        ),
        (
            "xs.some(x => x > 1)",
            "syntax-method-call",
            "`xs.map(x => …).includes(true)`",
            "xs.map(x => x > 1).includes(true)",
        ),
        (
            "[...xs, 3]",
            "syntax-refused-idiom",
            "`xs.concat([x])`",
            "xs.concat([3])",
        ),
        (
            "Number(s)",
            "type-refused-idiom",
            "`parseNumber(s)`",
            "parseNumber(s)",
        ),
        (
            "useEffect(n)",
            "type-refused-idiom",
            "a `derive` is a value computed from others",
            "n",
        ),
    ];
    for (written, id, words, edit) in rows {
        let (got, message) = refused(written);
        assert_eq!(&got, id, "`{written}`: {message}");
        assert!(message.contains(words), "`{written}`: {message}");
        if let Err(e) = contract::compile(&app(edit)) {
            panic!("`{written}`'s edit `{edit}` does not compile: {e}");
        }
    }
}

#[test]
fn a_ternary_a_list_of_lists_and_a_list_argument_are_not_indexing() {
    for expr in [
        "n > 1 ? 1 : 2",
        "n > 1 ? (n > 2 ? 1 : 2) : 3",
        "[[1, 2], [3]].length",
        "xs.concat([3])",
        "label + s",
        "s + label",
    ] {
        if let Err(e) = contract::compile(&app(expr)) {
            panic!("`{expr}`: {e}");
        }
    }
}
