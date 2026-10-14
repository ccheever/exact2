//! LLP 1121 D3: a design may mark a token `scheme=fixed`, one colour in every
//! scheme by design (an ink on a fixed fill, a ground under media), which
//! `lower-colour-one-scheme` then leaves alone; an unmarked one-scheme token
//! is reported once, at its declaration, however many rows name it; the mark
//! is refused on a token whose sides differ.

fn app(tag: &str, design: &str, view: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("exact2-tamber-scheme-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
    dir.join("app.contract")
}

/// The `lower-colour-one-scheme` warnings, each `(line, message)`, and the
/// file each is in.
fn one_scheme(path: &std::path::Path) -> Vec<(String, u32, String)> {
    let (result, warnings) = contract::compile_path_all_warned(path, false);
    result.unwrap_or_else(|e| panic!("{e:?}"));
    warnings
        .into_iter()
        .filter(|w| w.id == "lower-colour-one-scheme")
        .map(|w| {
            let file = w.file.as_ref().map_or(String::new(), |f| {
                f.file_name().unwrap().to_string_lossy().into_owned()
            });
            (file, w.span.line, w.message)
        })
        .collect()
}

const VIEW: &str = "component App\n  view\n    column background-color=\"primary\" gap=4\n      text \"a\" color=\"white_100\"\n      text \"b\" color=\"white_100\"\n      text \"c\" color=\"white_100\" text-shadow=\"0 1px 2px white_100\"\n";

#[test]
fn a_fixed_token_is_silent_where_rows_name_it() {
    let path = app(
        "fixed",
        "tamber t\n\ntheme\n  primary = light-dark(#1083fe, #208bfe)\n  white_100 = #ffffff  scheme=fixed\n",
        VIEW,
    );
    assert_eq!(one_scheme(&path), Vec::new());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn an_unmarked_one_scheme_token_is_reported_once_at_its_declaration() {
    let path = app(
        "once",
        "tamber t\n\ntheme\n  primary = light-dark(#1083fe, #208bfe)\n  white_100 = #ffffff\n",
        VIEW,
    );
    let found = one_scheme(&path);
    assert_eq!(found.len(), 1, "{found:?}");
    let (file, line, message) = &found[0];
    assert_eq!((file.as_str(), *line), ("app.tamber", 5), "{message}");
    assert!(
        message.contains("`white_100` is one colour in every mode"),
        "{message}"
    );
    assert!(
        message.contains("light-dark(light, dark)") && message.contains("scheme=fixed"),
        "{message}"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn a_literal_in_a_row_is_still_reported_where_it_is() {
    let path = app(
        "literal",
        "tamber t\n\ntheme\n  white_100 = #ffffff  scheme=fixed\n",
        "component App\n  view\n    column background-color=\"#1083fe\"\n      text \"a\" color=\"white_100\"\n",
    );
    let found = one_scheme(&path);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        (found[0].0.as_str(), found[0].1),
        ("app.contract", 3),
        "{found:?}"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn the_mark_is_refused_where_the_sides_differ() {
    for (tag, design) in [
        (
            "pair",
            "tamber t\n\ntheme\n  ink = light-dark(#000000, #ffffff)  scheme=fixed\n",
        ),
        (
            "role",
            "tamber t\n\ntheme\n  ink = -exact-label  scheme=fixed\n",
        ),
        ("word", "tamber t\n\ntheme\n  ink = #000000  scheme=dark\n"),
    ] {
        let path = app(
            tag,
            design,
            "component App\n  view\n    text \"a\" color=\"ink\"\n",
        );
        let e = contract::compile_path(&path).unwrap_err();
        assert_eq!(e.id, "tamber-scheme-fixed", "{tag}: {e:?}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
    // The same colour on both sides is one colour.
    let path = app(
        "same",
        "tamber t\n\ntheme\n  ink = light-dark(#101010, #101010)  scheme=fixed\n",
        "component App\n  view\n    text \"a\" color=\"ink\"\n",
    );
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn the_ir_carries_the_mark() {
    let f = exact_tamber::check(
        "tamber t\n\ntheme\n  white_100 = #ffffff  scheme=fixed\n  ink = #000000\n",
        &contract_lower::tamber_vocab(),
    )
    .unwrap();
    assert!(f.tokens[0].fixed && !f.tokens[1].fixed);
    let ir = f.ir();
    assert!(
        ir.contains(
            "\"name\": \"white_100\", \"kind\": \"color\", \"role\": null, \"scheme\": \"fixed\""
        ),
        "{ir}"
    );
    assert!(
        ir.contains("\"name\": \"ink\", \"kind\": \"color\", \"role\": null, \"values\""),
        "{ir}"
    );
}
