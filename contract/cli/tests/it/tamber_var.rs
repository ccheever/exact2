//! @ref LLP 1153 D6 — `var(--name)` is the token `name`: it compiles to the
//! plan the name compiles to, for a colour, a length, a number, a type, a
//! family, a box and a text shadow, a duration and an easing; a name the
//! design lacks is refused naming the tokens the row could take, fallback or
//! not, in a row or where a component's argument carries it to one; a
//! fallback is warned once; a token of another kind is refused as its name
//! is.

use std::path::PathBuf;

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-var-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

const DESIGN: &str = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  ink = light-dark(#111111, #eeeeee)\n    when density=compact: light-dark(#222222, #dddddd)\n  paper = #ffffff  scheme=fixed\n  space.row = 12px\n    when density=compact: 8px\n  faded: number = 0.5\n  type.body = -exact-body/1.35 \"InterVariable\"\n  face = \"InterVariable\", system-ui\n  shadow.card: box-shadow = 0 1px 3px ink\n  glow: text-shadow = 0 0 2px #ff0000\n  duration.fast = 150ms\n  easing.standard = cubic-bezier(0.2, 0, 0, 1)\n";

/// One view, its tokens spelled by `t`: as the token's name, or as `var()`.
fn view(t: &dyn Fn(&str) -> String) -> String {
    format!(
        "keyframes appear\n  from opacity=0\n  to opacity=1\n\nstyle Card\n  background-color=\"{paper}\"\n\ncomponent App\n  view\n    column class=Card padding=\"{row}\" color=\"{ink}\" box-shadow=\"{shadow}\" transition=\"opacity {fast} {ease}\"\n      text \"a\" font=\"{body}\" opacity=\"{faded}\" border=\"1px solid {ink}\" text-shadow=\"{glow}\"\n      text \"b\" font-family=\"{face}\" animation=\"appear {fast}\"\n      Badge(tint=\"{ink}\")\n\ncomponent Badge\n  props\n    tint: string\n  view\n    text \"c\" color=tint\n",
        paper = t("paper"),
        row = t("space.row"),
        ink = t("ink"),
        shadow = t("shadow.card"),
        fast = t("duration.fast"),
        ease = t("easing.standard"),
        glow = t("glow"),
        body = t("type.body"),
        faded = t("faded"),
        face = t("face"),
    )
}

#[test]
fn a_var_compiles_to_the_plan_its_name_does() {
    let named = app(
        "named",
        &[
            ("app.tamber", DESIGN),
            ("app.contract", &view(&|n| n.to_string())),
        ],
    );
    let var = app(
        "var",
        &[
            ("app.tamber", DESIGN),
            ("app.contract", &view(&|n| format!("var(--{n})"))),
        ],
    );
    let named = contract::compile_path(&named).unwrap_or_else(|e| panic!("{e}"));
    let var = contract::compile_path(&var).unwrap_or_else(|e| panic!("{e}"));
    // The varying colour and length are kept as tokens, read by the rows.
    let kept: Vec<&str> = var.theme.iter().map(|t| var.str(t.name)).collect();
    assert!(
        kept.contains(&"ink") && kept.contains(&"space.row"),
        "{kept:?}"
    );
    assert!(named.encode() == var.encode(), "the plans differ");
    // An inline `theme`, the design a source text has, is named the same way.
    let inline = |ink: &str| {
        let src = format!("theme\n  brand = \"#c8102e\"\n\ncomponent App\n  view\n    text \"a\" color=\"{ink}\"\n");
        contract::compile(&src)
            .unwrap_or_else(|e| panic!("{e}"))
            .encode()
    };
    assert!(
        inline("brand") == inline("VAR( --brand )"),
        "the inline plans differ"
    );
}

#[test]
fn a_name_the_design_lacks_is_refused_naming_the_tokens_of_its_kind() {
    let path = app(
        "unknown",
        &[
            ("app.tamber", DESIGN),
            ("app.contract", "component App\n  view\n    column color=\"var(--inc)\" padding=\"var(--space.rows, 4px)\"\n      text \"a\" background-color=\"var(--surface-card)\"\n      text \"b\" border=\"1px solid var(--line)\" color=\"var(--exact-label)\"\n"),
        ],
    );
    let Err(all) = contract::compile_path_all(&path, false) else {
        panic!("compiled");
    };
    let at = |what: &str| {
        all.iter()
            .find(|e| e.message.contains(what))
            .unwrap_or_else(|| panic!("no refusal names {what}: {all:#?}"))
    };
    let colour = at("`--inc`");
    assert_eq!(colour.id, "tamber-unknown-token");
    assert!(
        colour
            .message
            .contains("its colour tokens are `ink`, `paper`")
            && !colour.message.contains("space.row")
            && !colour.message.contains("fallback"),
        "{}",
        colour.message
    );
    // A fallback does not save a misspelt name; a length row lists lengths.
    let length = at("`--space.rows`");
    assert_eq!(length.id, "tamber-unknown-token");
    assert!(
        length.message.contains("its length tokens are `space.row`")
            && length.message.contains("the fallback is not used"),
        "{}",
        length.message
    );
    // A shorthand lists every kind it takes; a role is named bare.
    assert!(at("`--line`").message.contains("colour or length tokens"));
    assert!(
        at("`--exact-label`")
            .message
            .contains("`-exact-label` is a role"),
        "{all:#?}"
    );
    // Each refusal is said once, and nothing else of its row.
    assert_eq!(all.len(), 5, "{all:#?}");
    // A source text with no design says where tokens come from.
    let e = contract::compile("component App\n  view\n    text \"a\" color=\"var(--ink, #111)\"\n")
        .unwrap_err();
    assert_eq!(e.id, "tamber-unknown-token");
    assert!(
        e.message.contains("this app has no design") && e.message.contains("tamber import css"),
        "{}",
        e.message
    );
}

#[test]
fn a_name_the_design_lacks_is_refused_where_an_argument_carries_it_to_a_row() {
    for tint in ["var(--missing, #000)", "var(--missing)"] {
        let view = format!("component App\n  view\n    column\n      Badge(tint=\"{tint}\")\n\ncomponent Badge\n  props\n    tint: string\n  view\n    text \"c\" color=tint\n");
        let path = app(
            "carried",
            &[("app.tamber", DESIGN), ("app.contract", &view)],
        );
        let e = contract::compile_path(&path).unwrap_err();
        assert_eq!(e.id, "tamber-unknown-token", "{tint}: {e}");
        assert!(
            e.message.contains("its colour tokens are `ink`, `paper`"),
            "{tint}: {}",
            e.message
        );
    }
    // An argument no style row reads is text, left as written.
    let view = "component App\n  view\n    column\n      Label(words=\"var(--missing)\")\n\ncomponent Label\n  props\n    words: string\n  view\n    text words\n";
    let path = app("text", &[("app.tamber", DESIGN), ("app.contract", view)]);
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_fallback_is_ignored_and_warned_once() {
    let path = app(
        "fallback",
        &[
            ("app.tamber", DESIGN),
            ("app.contract", "component App\n  view\n    column color=\"var(--ink, #000)\" padding=\"var(--space.row, 4px)\"\n      text \"a\" border=\"1px solid var(--ink, rgb(0 0 0))\"\n"),
        ],
    );
    let (result, warnings) = contract::compile_path_all_warned(&path, false);
    result.unwrap_or_else(|e| panic!("{e:#?}"));
    let said: Vec<_> = warnings
        .iter()
        .filter(|w| w.id == "tamber-var-fallback")
        .collect();
    assert_eq!(said.len(), 1, "{warnings:#?}");
    assert!(
        said[0]
            .message
            .starts_with("`var(--ink, #000)`: `ink` is a token of the design")
            && said[0].message.contains("2 other `var()`s"),
        "{}",
        said[0].message
    );
    // A design that does not parse fails the build; no fallback is said to
    // name one of its tokens.
    let path = app(
        "unparsed",
        &[
            ("app.tamber", "tamber t\n\ntheme\n  ink #111111\n"),
            (
                "app.contract",
                "component App\n  view\n    text \"a\" color=\"var(--typo, red)\"\n",
            ),
        ],
    );
    let (result, warnings) = contract::compile_path_all_warned(&path, false);
    let Err(all) = result else {
        panic!("compiled");
    };
    assert!(all.iter().any(|e| e.id.starts_with("tamber-")), "{all:#?}");
    assert!(
        !warnings.iter().any(|w| w.id == "tamber-var-fallback"),
        "{warnings:#?}"
    );
}

#[test]
fn a_token_of_another_kind_is_refused_as_its_name_is() {
    let refused = |name: &str, view: &str| {
        let path = app(name, &[("app.tamber", DESIGN), ("app.contract", view)]);
        contract::compile_path(&path).unwrap_err()
    };
    let e = refused(
        "length-in-colour",
        "component App\n  view\n    column color=\"var(--space.row)\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    assert!(e.message.contains("takes a colour"), "{}", e.message);
    let e = refused(
        "colour-in-length",
        "component App\n  view\n    column padding=\"var(--ink)\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
}
