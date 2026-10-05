//! Shape dependency edges must agree with the type resolver's namespaces,
//! and a type bounds its values as the runner does (LLP 1090 D2).

#[test]
fn primitive_named_shapes_do_not_make_primitive_fields_recursive() {
    for name in ["number", "string", "bool", "unit"] {
        for ty in [
            name.to_owned(),
            format!("option<{name}>"),
            format!("list<option<{name}>>"),
        ] {
            let source =
                format!("shape {name}\n  value: {ty}\ncomponent App\n  view\n    text \"hello\"\n");
            contract::compile(&source).unwrap_or_else(|error| panic!("{source}\n{error}"));
        }
    }
}

#[test]
fn primitive_names_do_not_create_false_indirect_cycles() {
    let source = "shape string\n  value: Other\nshape Other\n  value: list<option<string>>\ncomponent App\n  view\n    text \"hello\"\n";
    contract::compile(source).unwrap();
}

#[test]
fn real_cycles_remain_refused_through_wrappers_and_unreachable_declarations() {
    for declarations in [
        "shape Row\n  next: Row\n",
        "shape Row\n  next: option<Row>\n",
        "shape Row\n  next: list<option<Row>>\n",
        "shape Row\n  next: list<Other>\nshape Other\n  previous: option<Row>\n",
    ] {
        let error = contract::compile(&format!(
            "{declarations}component App\n  view\n    text \"hello\"\n"
        ))
        .unwrap_err();
        assert_eq!(error.id, "type-shape-recursive", "{error}");
        assert!(error.message.contains("Row ->"));
    }
}

#[test]
fn unknown_field_types_keep_their_own_diagnostic() {
    let error = contract::compile(
        "shape Row\n  value: list<Missing>\ncomponent App\n  view\n    text \"hello\"\n",
    )
    .unwrap_err();
    assert_eq!(error.id, "type-unknown");
    assert_eq!(error.span.line, 2);
    assert_eq!(error.span.col, 15);
}

fn refused(source: &str) -> String {
    contract::compile(source)
        .err()
        .unwrap_or_else(|| panic!("{source}\ncompiled"))
        .id
        .to_owned()
}

/// An option directly inside an option is refused, declared or inferred:
/// the web erases `some`, so `some(none)` would be `none` there.
#[test]
fn an_option_directly_inside_an_option_is_refused() {
    let app = |decl: &str, view: &str| {
        format!("shape Box\n  values: list<option<number>>\n{decl}component App\n  resource box = load() as shape Box\n  view\n    text {view}\n")
    };
    for source in [
        app("shape Bad\n  value: option<option<number>>\n", "\"a\""),
        app("", "`${some(some(1)) == none}`"),
        app("", "`${some(none) == none}`"),
        // `first` of a list of options is an option of one.
        app("", "`${first(box.values) == none}`"),
        "component App\n  state n = 0\n  action set(v: option<option<number>>)\n    n = 1\n  view\n    text \"a\"\n".to_owned(),
        "component App\n  resource r = load() as shape option<option<string>>\n  view\n    text \"a\"\n".to_owned(),
        // A mutation holds `option<T>` of its answer.
        "component App\n  mutation m as shape option<string>\n  view\n    text \"a\"\n".to_owned(),
    ] {
        assert_eq!(refused(&source), "type-option-option", "{source}");
    }
    // An option in a list in an option, or in a record field, is fine.
    contract::compile(&app(
        "shape Fine\n  value: option<list<option<number>>>\n",
        "`${length(box.values)}`",
    ))
    .unwrap();
}

/// A type nests at most 64 deep (`MAX_VALUE_DEPTH`): through shapes,
/// lists and options alike, written or inferred.
#[test]
fn a_type_nests_at_most_sixty_four_deep() {
    // `S{i}` is `i + 1` deep: a record around `S{i - 1}`.
    let chain = |n: usize| {
        let mut s = "shape S0\n  v: number\n".to_owned();
        for i in 1..n {
            s += &format!("shape S{i}\n  v: S{}\n", i - 1);
        }
        s
    };
    let view = |top: &str, body: &str| {
        format!("component App\n  resource top = load() as shape {top}\n  view\n    text {body}\n")
    };
    contract::compile(&(chain(64) + &view("S63", "\"a\""))).unwrap();
    assert_eq!(
        refused(&(chain(65) + &view("S63", "\"a\""))),
        "type-too-deep"
    );
    // 63 levels of shape and one list is 64; an inferred `some` is 65.
    contract::compile(&(chain(63) + &view("list<S62>", "\"a\""))).unwrap();
    assert_eq!(
        refused(&(chain(64) + &view("S63", "`${some(top) == none}`"))),
        "type-too-deep"
    );
    let lists = |n: usize| "list<".repeat(n) + "number" + &">".repeat(n);
    contract::compile(&view(&lists(64), "\"a\"")).unwrap();
    assert_eq!(refused(&view(&lists(65), "\"a\"")), "type-too-deep");
}
