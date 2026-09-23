//! Shape dependency edges must agree with the type resolver's namespaces.

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
