//! @ref LLP 1071.001 part B, B5 (lower.rs; theme2 07:30, 07:33): which nodes
//! lower, the names they take, and every refusal's code as the `--no-lower`
//! build's, byte for byte.
use crate::emit::lower::{self, Refusal, REFUSED};

const HEAD: &str = "component App\n  state s = \"a\"\n  state t = \"x\"\n  state n = 0\n  state on = false\n  action set(v: string)\n    s = v\n  view\n    column\n";

fn emit(source: &str) -> crate::emit::Output {
    crate::emit::emit(&contract::compile(source).unwrap(), false, false).unwrap()
}

/// `node` (under `HEAD`'s column) and what the build refused it for.
fn refusals(node: &str) -> (crate::emit::Output, Vec<Refusal>) {
    let source = format!("{HEAD}      {node}\n");
    REFUSED.with(|r| r.borrow_mut().clear());
    let out = emit(&source);
    (out, REFUSED.with(|r| r.borrow_mut().drain(..).collect()))
}

/// Condition (1): a refused node keeps the exact inline path, its program
/// and sheet byte for byte the `--no-lower` build's.
fn refused(node: &str, why: Refusal) {
    let (on, seen) = refusals(node);
    let off = lower::without(|| emit(&format!("{HEAD}      {node}\n")));
    assert!(!on.js.contains("Pc("), "{node}: lowered\n{}", on.js);
    assert_eq!(
        on.js, off.js,
        "{node}: the program differs from --no-lower's"
    );
    assert_eq!(
        on.css, off.css,
        "{node}: the sheet differs from --no-lower's"
    );
    assert_eq!(seen, vec![why], "{node}");
}

fn lowered(node: &str) -> String {
    let (on, seen) = refusals(node);
    assert!(seen.is_empty(), "{node}: refused {seen:?}");
    assert!(on.js.contains("Pc("), "{node}: not lowered\n{}", on.js);
    on.js
}

/// The `Pc` call's name argument (`xa…`).
fn name(js: &str) -> String {
    let at = js.find("\"xa").expect("an arm name");
    js[at + 1..].split('"').next().unwrap().to_string()
}

#[test]
fn an_enumerated_node_lowers_to_one_class_write() {
    let js = lowered("text \"x\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\") font-weight=(s == \"b\" ? 600 : 400)");
    assert!(js.contains("v===\"a\"?0:v===\"b\"?1:2)(s_0())"), "{js}");
    assert!(
        js.contains("[[\"#000000\",400],[\"#007AFF\",600],[\"#007AFF\",400]]"),
        "{js}"
    );
    assert!(
        js.contains("[\"color\",\"font-weight\",\"--exact-said-font-weight\"]"),
        "{js}"
    );
    // The rows are `style_row`'s own statements, on the arm's literals.
    assert!(!js.contains("S(e1,"), "{js}");
    assert!(
        js.contains("S($e,\"color\",\"\",()=>($cm)(blank((()=>$b[0])())))"),
        "{js}"
    );
}

#[test]
fn the_bounded_feeds_text_run_lowers_with_seven_arms_and_ten_rows() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../bench/dioxus/exact-bounded/app.contract"
    ))
    .unwrap();
    REFUSED.with(|r| r.borrow_mut().clear());
    let js = emit(&source).js;
    assert_eq!(js.matches("Pc(").count(), 1, "{js}");
    let at = js.find("Pc(").unwrap();
    assert!(js[at..].starts_with("Pc(e17,()=>(v=>v===\"code\"?0:v===\"bold\"?1:v===\"mention\"?2:v===\"italic\"?3:v===\"link\"?4:v===\"tag\"?5:6)(i3()[2])"), "{}", &js[at..at + 200]);
    assert!(js[at..].contains("[\"font-family\",\"font-size\",\"--exact-style-font-weight\",\"line-height\",\"background-color\",\"font-weight\",\"--exact-said-font-weight\",\"font-style\",\"color\",\"text-decoration-line\"]"), "{js}");
}

#[test]
fn r1_a_node_that_hears_a_press_keeps_its_effects() {
    refused(
        "text \"x\" press=set(\"b\") color=(s == \"a\" ? \"#000000\" : \"#007AFF\")",
        Refusal::Pressable,
    );
    refused(
        "button \"x\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\")",
        Refusal::Pressable,
    );
}

#[test]
fn r2_display_keeps_its_effects() {
    refused(
        "text \"x\" display=(s == \"a\" ? \"none\" : \"block\")",
        Refusal::Display,
    );
}

#[test]
fn r3_a_custom_property_a_reader_finds_inline_keeps_its_effects() {
    // `--exact-fit`, which navigation.js 1060 finds by `[style*="--exact-fit"]`.
    refused(
        "text \"x\" -exact-fit=(s == \"a\" ? \"shrink\" : \"none\")",
        Refusal::CustomProperty,
    );
    // A companion only `var()` reads is refused too, while the type companions are not (the feed's).
    refused(
        "column gap=(s == \"a\" ? 1 : 2)\n        text \"x\"",
        Refusal::CustomProperty,
    );
}

#[test]
fn r4_a_glue_written_property_keeps_its_effects() {
    refused(
        "text \"x\" visibility=(s == \"a\" ? \"hidden\" : \"visible\")",
        Refusal::GlueWritten,
    );
    refused(
        "text \"x\" position=(s == \"a\" ? \"relative\" : \"static\")",
        Refusal::GlueWritten,
    );
}

#[test]
fn r5_a_marked_row_keeps_its_effects() {
    refused("column border-top-style=\"solid\" border-top-width=(s == \"a\" ? 1 : 2)\n        text \"x\"", Refusal::SidePath);
}

#[test]
fn r6_a_modes_channel_keeps_its_effects() {
    let dir = std::env::temp_dir().join(format!("exact-web-js-lower-r6-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), "tamber t\n\naxes\n  ink = tint | plain\n\ntheme\n  text.primary = #000001\n    when ink=plain: #000002\n").unwrap();
    std::fs::write(
        dir.join("app.contract"),
        format!("{HEAD}      text \"x\" color=(s == \"a\" ? \"text.primary\" : \"#007AFF\")\n"),
    )
    .unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    REFUSED.with(|r| r.borrow_mut().clear());
    let on = crate::emit::emit(&plan, false, false).unwrap();
    let seen: Vec<Refusal> = REFUSED.with(|r| r.borrow_mut().drain(..).collect());
    let off = lower::without(|| crate::emit::emit(&plan, false, false).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(on.js.contains("Sx(S,"), "{}", on.js);
    assert_eq!((on.js, on.css), (off.js, off.css));
    assert_eq!(seen, vec![Refusal::SidePath]);
}

#[test]
fn r7_a_node_another_module_classes_keeps_its_effects() {
    refused(
        "text \"x\" markup=\"markdown\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\")",
        Refusal::ClassOwned,
    );
}

#[test]
fn r8_a_string_arm_outside_the_safe_kinds_keeps_its_effects() {
    refused(
        "text \"x\" color=(s == \"a\" ? \"rgb(0 0 0)\" : \"#007AFF\")",
        Refusal::ValueKind,
    );
}

#[test]
fn r9_color_scheme_keeps_its_effects() {
    refused(
        "column color-scheme=(s == \"a\" ? \"dark\" : \"light\")\n        text \"x\"",
        Refusal::ColorScheme,
    );
}

#[test]
fn two_fields_keep_their_effects() {
    refused("text \"x\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\") font-weight=(t == \"x\" ? 600 : 400)", Refusal::TwoFields);
}

#[test]
fn a_non_literal_arm_keeps_its_effects() {
    refused(
        "text \"x\" color=(s == \"a\" ? s : \"#007AFF\")",
        Refusal::NonLiteralArm,
    );
    refused(
        "text \"x\" font-weight=(s == \"a\" ? 600 + 100 : 400)",
        Refusal::NonLiteralArm,
    );
}

#[test]
fn seventeen_arms_keep_their_effects() {
    let chain: String = (0..16)
        .map(|k| format!("s == \"v{k}\" ? {} : ", 100 + k))
        .collect();
    refused(
        &format!("text \"x\" font-weight=({chain}400)"),
        Refusal::TooManyArms,
    );
    // Sixteen (fifteen literals and the rest) still lower.
    let chain: String = (0..15)
        .map(|k| format!("s == \"v{k}\" ? {} : ", 100 + k))
        .collect();
    lowered(&format!("text \"x\" font-weight=({chain}400)"));
}

#[test]
fn a_binding_that_is_no_enum_comparison_keeps_its_effects() {
    refused("text \"x\" font-weight=(on ? 600 : 400)", Refusal::NonEnum);
}

/// Condition (3): the names are `xa`-prefixed, so no `c<n>` class nor a
/// host class can be one, and the same plan names them alike every build.
#[test]
fn arm_names_are_namespaced_stable_and_collide_with_no_class() {
    let node = "text \"x\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\")";
    let (one, two) = (name(&lowered(node)), name(&lowered(node)));
    assert_eq!(one, two);
    assert!(
        one.starts_with("xa") && one[2..].bytes().all(|b| b.is_ascii_alphanumeric()),
        "{one}"
    );
    // Every class the host's sheets and modules name, and the app's own `c<n>`.
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../web");
    let mut named = Vec::new();
    for file in ["index.html", "neutral.css", "markup-editor.js", "atoms.js"] {
        let text = std::fs::read_to_string(format!("{root}/{file}")).unwrap();
        let b = text.as_bytes();
        for (at, _) in text.match_indices('.') {
            let end = (at + 1..b.len())
                .find(|&j| !(b[j].is_ascii_alphanumeric() || b[j] == b'-' || b[j] == b'_'))
                .unwrap_or(b.len());
            if end > at + 1 && b[at + 1].is_ascii_alphabetic() {
                named.push(text[at + 1..end].to_string());
            }
        }
    }
    named.extend(["exact-atom", "exact-markdown-editor", "md-empty", "md-hide"].map(String::from));
    assert!(named.len() > 50, "{}", named.len());
    assert!(
        !named.iter().any(|n| n.starts_with("xa")),
        "a host class takes the arm prefix"
    );
    let out = emit(&format!("{HEAD}      {node}\n"));
    assert!(
        !out.css.contains(".xa"),
        "the arms are the page's, not app.css's"
    );
}

/// Condition (7): an arm whose literal changes is a new name, so a rule
/// built for the old one never stands for it.
#[test]
fn a_changed_literal_is_a_new_name() {
    let was = name(&lowered(
        "text \"x\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\")",
    ));
    let now = name(&lowered(
        "text \"x\" color=(s == \"a\" ? \"#000001\" : \"#007AFF\")",
    ));
    assert_ne!(was, now);
}

/// A development build (`--sites`) lowers too, its counter on the key: the
/// conformance harness and the agent drive these builds.
#[test]
fn a_development_build_lowers_and_counts_the_key() {
    let plan = contract::compile(&format!(
        "{HEAD}      text \"x\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\")\n"
    ))
    .unwrap();
    let js = crate::emit::emit(&plan, true, false).unwrap().js;
    assert!(
        js.contains("Pc(e1,pf(1,()=>(v=>v===\"a\"?0:1)(s_0()))"),
        "{js}"
    );
    assert!(!js.contains("const p1_"), "{js}");
}

/// Round 1 B1: a tinted image's template rows (symbols.js `template`) are
/// glue-written there; a dynamic source makes the tint a binding too.
#[test]
fn a_tinted_images_template_rows_keep_their_effects() {
    refused(
        "image \"symbol:star\" -exact-tint-color=\"#FF0000\" width=20 height=20 background-color=(s == \"a\" ? \"#00FF00\" : \"#0000FF\")",
        Refusal::GlueWritten,
    );
    let svg =
        "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='8'/%3E";
    refused(
        &format!("image (t == \"x\" ? \"{svg}\" : \"{svg}#b\") -exact-tint-color=\"#FF0000\" width=20 height=20 background-color=(s == \"a\" ? \"#00FF00\" : \"#0000FF\")"),
        Refusal::CustomProperty,
    );
    // An untinted image's background still lowers.
    lowered("image \"symbol:star\" width=20 height=20 background-color=(s == \"a\" ? \"#00FF00\" : \"#0000FF\")");
}

/// Round 1 B2: `revert-layer` is refused; the CSS-wide keywords that mean
/// the same inline and in an author rule are not.
#[test]
fn revert_layer_is_refused_and_the_other_wide_keywords_are_not() {
    assert!(!lower::safe_text("revert-layer"));
    assert!(!lower::safe_text("REVERT-LAYER"));
    for ok in [
        "revert",
        "inherit",
        "initial",
        "unset",
        "#007AFF",
        "ui-monospace",
        "50%",
    ] {
        assert!(lower::safe_text(ok), "{ok}");
    }
    lowered("text \"x\" font-weight=(s == \"a\" ? \"inherit\" : 400)");
}

/// Round 1 B3: numbers as JavaScript writes and compares them.
#[test]
fn numbers_are_javascripts() {
    use lower::Lit;
    assert_eq!(Lit::N(f64::INFINITY.to_bits()).js(), "Infinity");
    assert_eq!(Lit::N(f64::NEG_INFINITY.to_bits()).js(), "(-Infinity)");
    assert_eq!(
        lower::compared(Lit::N((-0.0f64).to_bits())),
        Ok(Lit::N(0.0f64.to_bits()))
    );
    assert_eq!(
        lower::compared(Lit::N(f64::NAN.to_bits())),
        Err(Refusal::ValueKind)
    );
    // A number key lowers, compared as `===` compares (the contract writes -0
    // as a negation, which is no literal; `compared` above folds it).
    let js = lowered("text \"x\" font-weight=(n == 0 ? 600 : n == 2 ? 700 : 400)");
    assert!(js.contains("(v=>v===0?0:v===2?1:2)("), "{js}");
    assert!(js.contains("[[600],[700],[400]]"), "{js}");
}

/// Round 2: a binding of another kind between two style rows ran between
/// their effects, which one class write cannot keep: the node keeps its
/// effects. Rows side by side (the other binding before or after) lower.
#[test]
fn a_binding_between_the_rows_keeps_their_effects() {
    refused(
        "text \"x\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\") aria-label=t font-weight=(s == \"b\" ? 600 : 400)",
        Refusal::Interleaved,
    );
    lowered("text \"x\" aria-label=t color=(s == \"a\" ? \"#000000\" : \"#007AFF\") font-weight=(s == \"b\" ? 600 : 400)");
}

/// Rounds 2 and 3: what the runtime and the glues read inline is refused,
/// each property byte for byte as the inline build (lower.rs READ).
#[test]
fn an_inline_read_property_keeps_its_effects() {
    // Each property a contract can bind, byte for byte as the inline build.
    for (node, prop) in [
        (
            "column scroll-behavior=(s == \"a\" ? \"smooth\" : \"auto\")",
            "scroll-behavior",
        ),
        (
            "column flex-direction=(s == \"a\" ? \"row\" : \"column\")",
            "flex-direction",
        ),
        (
            "column appearance=(s == \"a\" ? \"none\" : \"auto\")",
            "appearance",
        ),
        (
            "column popover=\"auto\" position-area=(s == \"a\" ? \"top\" : \"bottom\")",
            "position-area",
        ),
        (
            "column transition=(s == \"a\" ? \"color 1s\" : \"none\")",
            "transition",
        ),
        (
            "column position=\"absolute\" right=(s == \"a\" ? 4 : 8)",
            "right",
        ),
        (
            "column position=\"absolute\" bottom=(s == \"a\" ? 4 : 8)",
            "bottom",
        ),
        (
            "column transform-origin=(s == \"a\" ? \"left\" : \"right\")",
            "transform-origin",
        ),
    ] {
        let _ = prop;
        refused(&format!("{node}\n        text \"x\""), Refusal::GlueWritten);
    }
    // The rest no contract attribute here binds (mask-size, overflow-anchor,
    // view-transition-name: the compiler refuses the attribute), or only a
    // motion node binds (animation, style_names.rs; refused there as R5 too);
    // the refusal stands for any row that writes them.
    for prop in [
        "mask-size",
        "overflow-anchor",
        "view-transition-name",
        "animation",
    ] {
        assert_eq!(
            lower::refuse_prop(prop, false),
            Err(Refusal::GlueWritten),
            "{prop}"
        );
    }
}

/// Round 2: a hatched node is page code's, which may class it; a live time
/// is its glue's (live-time-glue.js reads its widths inline).
#[test]
fn hatched_and_live_time_nodes_keep_their_effects() {
    refused(
        "text \"x\" hatch=\"chart\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\")",
        Refusal::ClassOwned,
    );
    refused(
        "time datetime=1760000000000 color=(s == \"a\" ? \"#000000\" : \"#007AFF\")",
        Refusal::GlueWritten,
    );
}

/// Round 2: each name half is padded to 13 digits; `revert-layer` is a
/// whole token.
#[test]
fn names_are_padded_and_revert_layer_is_a_token() {
    let js = lowered("text \"x\" color=(s == \"a\" ? \"#000000\" : \"#007AFF\")");
    assert_eq!(name(&js).len(), 2 + 26, "{}", name(&js));
    assert!(lower::safe_text("not-revert-layer-ish"));
    assert!(!lower::safe_text("a revert-layer"));
}

/// A `none` arm and a blank one write nothing there, as `S` removes the row.
#[test]
fn a_none_and_a_blank_arm_lower() {
    let js = lowered("text \"x\" font-weight=(s == \"a\" ? none : 700) font-family=(s == \"b\" ? \"\" : \"serif\")");
    assert!(js.contains("[[null,4],[700,null],[700,4]]"), "{js}");
}
