//! @ref LLP 1153 D6 — `var(--name)` is the token `name`: it compiles to the
//! plan the name compiles to, for a colour, a length, a number, a type, a
//! family, a box and a text shadow, a duration and an easing; a name the
//! design lacks is refused naming the tokens the row could take, fallback or
//! not, in a row or where a component's argument carries it to one (a
//! `fn`'s result and a `derive`'s value name a token as an argument does); a
//! `var()` whose name data completes is the name it makes; a fallback is
//! warned once; a token of another kind is refused as its name is.

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

const DESIGN: &str = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  ink = light-dark(#111111, #eeeeee)\n    when density=compact: light-dark(#222222, #dddddd)\n  paper = #ffffff  scheme=fixed\n  space.row = 12px\n    when density=compact: 8px\n  card.min = 9.5rem\n  faded: number = 0.5\n  type.body = -exact-body/1.35 \"InterVariable\"\n  face = \"InterVariable\", system-ui\n  shadow.card: box-shadow = 0 1px 3px ink\n  glow: text-shadow = 0 0 2px #ff0000\n  duration.fast = 150ms\n  easing.standard = cubic-bezier(0.2, 0, 0, 1)\n";

/// One view, its tokens spelled by `t`: as the token's name, or as `var()`.
fn view(t: &dyn Fn(&str) -> String) -> String {
    format!(
        "keyframes appear\n  from opacity=0\n  to opacity=1\n\nstyle Card\n  background-color=\"{paper}\"\n\nfn tint(on: bool): string = on ? \"{ink}\" : \"{paper}\"\n\ncomponent App\n  state on = true\n  derive shade = on ? \"{paper}\" : \"{ink}\"\n  view\n    column class=Card padding=\"{row}\" color=\"{ink}\" box-shadow=\"{shadow}\" transition=\"opacity {fast} {ease}\"\n      text \"a\" font=\"{body}\" opacity=\"{faded}\" border=\"1px solid {ink}\" text-shadow=\"{glow}\"\n      text \"b\" font-family=\"{face}\" animation=\"appear {fast}\"\n      Badge(tint=\"{ink}\")\n      text \"d\" color=tint(on) background-color=shade\n\ncomponent Badge\n  props\n    tint: string\n  view\n    text \"c\" color=tint\n",
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

/// A `var()` whose name data completes, the whole of a template, is the name
/// that template makes, which the runtime reads against the design: in a
/// row, a `fn`'s result and a `derive`, as `type-${t}` is.
#[test]
fn a_var_whose_name_data_completes_is_that_name() {
    const TYPES: &str = "tamber t\n\ntheme\n  type-fire = #c2410c  scheme=fixed\n  type-water = #2a5fc4  scheme=fixed\n";
    let plan = |label: &str, tint: &str| {
        let view = format!("fn tint(t: string): string = `{tint}`\n\ncomponent App\n  state t = \"fire\"\n  derive shade = `{tint}`\n  view\n    column\n      text \"a\" color=tint(t) background-color=shade\n      text \"b\" border-color=`{tint}`\n");
        let dir = app(label, &[("app.tamber", TYPES), ("app.contract", &view)]);
        contract::compile_path(&dir)
            .unwrap_or_else(|e| panic!("{e}"))
            .encode()
    };
    assert!(
        plan("completed-name", "type-${t}") == plan("completed-var", "var(--type-${t})"),
        "the plans differ"
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

/// A length token in a grid track list is its value, as a browser writes in
/// a `var()` before it parses the tracks; a token that varies by app mode is
/// no track.
#[test]
fn a_length_token_in_grid_tracks_is_its_value() {
    let grid = |tracks: &str| {
        let path = app(
            "grid",
            &[
                ("app.tamber", DESIGN),
                ("app.contract", &format!("component App\n  view\n    view display=\"grid\" grid-template-columns=\"{tracks}\"\n      text \"a\"\n")),
            ],
        );
        contract::compile_path(&path)
            .map(|p| p.encode())
            .map_err(|e| e.to_string())
    };
    let written = b"repeat(auto-fill, minmax(min(100%, 9.5rem), 1fr))";
    for tracks in [
        "repeat(auto-fill, minmax(min(100%, var(--card.min)), 1fr))",
        "repeat(auto-fill, minmax(min(100%, card.min), 1fr))",
    ] {
        let plan = grid(tracks).unwrap_or_else(|e| panic!("{tracks}: {e}"));
        assert!(
            plan.windows(written.len()).any(|w| w == written),
            "{tracks}"
        );
    }
    let varies = grid("repeat(2, var(--space.row))").unwrap_err();
    assert!(
        varies.contains("with one value in every app mode"),
        "{varies}"
    );
}

/// A length token inside `calc()`, `min()`, `max()` or `clamp()` is read
/// where the row is drawn, as a `var()` there is: a `rem` token, and one
/// that varies by app mode, compile.
#[test]
fn a_length_token_in_a_math_function_compiles() {
    for (i, margin) in [
        "calc(var(--space.row) * -1)",
        "calc(space.row * -1)",
        "max(var(--card.min), 50%)",
        "clamp(4px, calc(100% - 2 * var(--card.min)), 40rem)",
    ]
    .into_iter()
    .enumerate()
    {
        let path = app(
            &format!("math-{i}"),
            &[
                ("app.tamber", DESIGN),
                ("app.contract", &format!("component App\n  view\n    column margin-top=\"{margin}\"\n      text \"a\"\n")),
            ],
        );
        if let Err(e) = contract::compile_path(&path) {
            panic!("{margin}: {e}");
        }
    }
}
