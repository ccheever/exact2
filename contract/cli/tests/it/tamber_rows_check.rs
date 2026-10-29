//! The design server's write check of the rows a write reaches (LLP 1121
//! D3 and D7; QUEUE's "the server's write check doesn't see a Contract
//! row's kind", theme2's 14:19 ruling): a write that breaks a literal row
//! that passed is refused in the compiler's own words, at the row, as
//! `contract build` refuses the design it would write.
#![cfg(unix)]

use super::tamber_package_params::reading;
use super::tamber_serve::{digest, Serve};
use serde_json::{json, Value};
use std::path::Path;

pub(super) fn transaction(s: &mut Serve, dir: &Path, op: Value) -> Value {
    s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&dir.join("app.tamber"))}, "ops": [op]}))
        .0
}

/// The server refuses `op`, and `contract build` refuses the design the
/// write would leave (`written`), with the same id and words, at the same
/// row.
pub(super) fn same_words(dir: &Path, design: &str, contract: &str, op: Value, written: &str) {
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), contract).unwrap();
    let mut s = Serve::start(dir, &[]);
    let (first, _) = s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    assert!(first.get("ok").is_some(), "{first}");
    let refused = transaction(&mut s, dir, op);
    drop(s);
    let d = &refused["error"]["diagnostics"][0];
    assert!(d.is_object(), "the write is refused: {refused}");
    // The design it would have written, built.
    std::fs::write(dir.join("app.tamber"), written).unwrap();
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(d["id"], built.id, "{refused}");
    assert_eq!(d["message"], built.message, "{refused}");
    assert_eq!(d["line"], built.span.line, "{refused}");
    assert_eq!(d["col"], built.span.col, "{refused}");
    assert_eq!(d["file"], "app.contract");
    let _ = std::fs::remove_dir_all(dir);
}

const DESIGN: &str =
    "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n  one = 1\n  scale = one\n  mute = #777777\n";
const CONTRACT: &str = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column padding=\"gap\" color=\"mute\"\n      button \"Go\" -exact-press-scale=\"scale\"\n      Card()\n";

#[test]
fn a_kind_change_is_refused_in_the_compilers_words() {
    same_words(
        &reading("rows-kind"),
        DESIGN,
        CONTRACT,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
        &DESIGN.replace("gap = 8px", "gap = #444444"),
    );
}

#[test]
fn a_press_scale_through_an_alias_and_by_a_value_is_refused() {
    same_words(
        &reading("rows-alias"),
        DESIGN,
        CONTRACT,
        json!({"op": "set-token", "name": "scale", "value": "ink"}),
        &DESIGN.replace("scale = one", "scale = ink"),
    );
    same_words(
        &reading("rows-value"),
        DESIGN,
        CONTRACT,
        json!({"op": "set-token", "name": "one", "value": "0"}),
        &DESIGN.replace("one = 1", "one = 0"),
    );
}

#[test]
fn a_removed_token_a_row_names_is_refused() {
    same_words(
        &reading("rows-removed"),
        DESIGN,
        CONTRACT,
        json!({"op": "remove-token", "name": "mute"}),
        &DESIGN.replace("  mute = #777777\n", ""),
    );
}

/// Through a chain of references: `scale = step`, `step = one`, and a write
/// to `one` reaches `-exact-press-scale="scale"`. (A package token over a
/// base name made the wrong kind, `reading.gap: length = space.md` with a
/// colour `space.md`, is the package fold's refusal, at the design line,
/// before any row.)
#[test]
fn a_write_reaches_a_row_through_references() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n  step = one\n  scale = step\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      button \"Go\" -exact-press-scale=\"scale\"\n      Card()\n";
    same_words(
        &reading("rows-reach"),
        design,
        contract,
        json!({"op": "set-token", "name": "one", "value": "0"}),
        &design.replace("one = 1", "one = 0"),
    );
}

/// Through a base name (`.bases`): `pad = calc(space.md * 2)` under
/// `padding="pad"`, and a write that sets the app's `space.md` negative.
#[test]
fn a_write_reaches_a_row_through_a_base_name() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  pad = calc(space.md * 2)\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column padding=\"pad\"\n      Card()\n";
    same_words(
        &reading("rows-base"),
        design,
        contract,
        json!({"op": "set-token", "name": "space.md", "value": "-4px"}),
        &format!("{design}  space.md = -4px\n"),
    );
}

/// Only the rows the build checks are the map's (Astra's round 1): a style
/// no node applies, and an element's row a shorthand writes, are no row
/// of it, so a write breaking them is the build's to judge (the build
/// accepts the first).
#[test]
fn an_unused_style_and_a_shorthand_are_not_judged() {
    let dir = reading("rows-unused");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n  mute = #777777\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\nstyle Loud\n  color = \"mute\"\ncomponent App\n  view\n    column border=\"1px solid ink\"\n      Card()\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(&mut s, &dir, json!({"op": "remove-token", "name": "mute"}));
    assert!(
        w.get("ok").is_some(),
        "an unused style's token removed: {w}"
    );
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Rows alike (one name, one value) are judged once and each refused at
/// its own place, as the build refuses each.
#[test]
fn rows_alike_are_each_refused_at_their_place() {
    let dir = reading("rows-alike");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  mute = #777777\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column color=\"mute\"\n      text \"a\" color=\"mute\"\n      Card()\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), contract).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(&mut s, &dir, json!({"op": "remove-token", "name": "mute"}));
    drop(s);
    let mut got: Vec<(u64, u64, String)> = w["error"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("{w}"))
        .iter()
        .map(|d| {
            (
                d["line"].as_u64().unwrap(),
                d["col"].as_u64().unwrap(),
                d["message"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    got.sort();
    std::fs::write(
        dir.join("app.tamber"),
        design.replace("  mute = #777777\n", ""),
    )
    .unwrap();
    let src = std::fs::read_to_string(dir.join("app.contract")).unwrap();
    let mut built: Vec<(u64, u64, String)> =
        contract::compile_path_source_all(&dir.join("app.contract"), &src, false)
            .err()
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.id == "tamber-unknown-token")
            .map(|e| (e.span.line as u64, e.span.col as u64, e.message))
            .collect();
    built.sort();
    assert_eq!(got, built);
    let _ = std::fs::remove_dir_all(&dir);
}

/// An arm of a choice is refused at its own literal, as the build refuses
/// it (Astra's round 2).
#[test]
fn an_arm_of_a_choice_is_refused_at_its_literal() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  mute = #777777\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  state on = false\n  view\n    column color=(on ? \"mute\" : \"#000000\")\n      Card()\n";
    same_words(
        &reading("rows-arm"),
        design,
        contract,
        json!({"op": "remove-token", "name": "mute"}),
        &design.replace("  mute = #777777\n", ""),
    );
}

/// A row whose other arm already fails is the build's, though this arm
/// breaks (theme2's Q1: the authored row passed or it didn't). Not refused.
#[test]
fn rows_the_build_judges_otherwise_are_left_to_it() {
    let dir = reading("rows-left");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n  good = 8px\n  bad = #222222\n  one = 1\n  pos = 8px\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  state on = false\n  view\n    column padding=(on ? \"good\" : \"bad\")\n      Card()\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "good", "value": "#333333"}),
    );
    let refused: Vec<&Value> = w["error"]["diagnostics"]
        .as_array()
        .map(|d| d.iter().filter(|d| d["file"] == "app.contract").collect())
        .unwrap_or_default();
    assert!(refused.is_empty(), "{w}");
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A plain row and an arm alike (one name, one literal) are each refused
/// where the build refuses each: a shadow's at the attribute, the arm's at
/// its literal (Astra's round 3: columns 14 and 25).
#[test]
fn a_plain_row_and_an_arm_alike_keep_their_places() {
    let dir = reading("rows-plain-arm");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  mute = #777777\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  state on = false\n  view\n    column box-shadow=(on ? \"0 0 2px mute\" : \"none\")\n      text \"a\" box-shadow=\"0 0 2px mute\"\n      Card()\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), contract).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(&mut s, &dir, json!({"op": "remove-token", "name": "mute"}));
    drop(s);
    let mut got: Vec<(u64, u64, String)> = w["error"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("{w}"))
        .iter()
        .map(|d| {
            (
                d["line"].as_u64().unwrap(),
                d["col"].as_u64().unwrap(),
                d["message"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    got.sort();
    std::fs::write(
        dir.join("app.tamber"),
        design.replace("  mute = #777777\n", ""),
    )
    .unwrap();
    let src = std::fs::read_to_string(dir.join("app.contract")).unwrap();
    let mut built: Vec<(u64, u64, String)> =
        contract::compile_path_source_all(&dir.join("app.contract"), &src, false)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.span.line as u64, e.span.col as u64, e.message))
            .collect();
    built.sort();
    assert_eq!(got, built);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A literal a `derive` binds is checked where it is bound, not as the
/// row's: removing the token it names is the build's (which accepts it
/// here), never a refusal (Astra's round 2).
#[test]
fn a_bound_literal_is_not_judged_as_the_rows() {
    let dir = reading("rows-bound");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n  mute = #777777\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      Tint()\n      Card()\ncomponent Tint\n  derive preferred = \"mute\"\n  derive chosen = (preferred != \"\" ? preferred : \"#000000\")\n  view\n    text \"t\" color=chosen\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(&mut s, &dir, json!({"op": "remove-token", "name": "mute"}));
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A row that already fails is the build's: a write touching its token,
/// turning one failure into another, isn't refused (theme2's Q1).
#[test]
fn a_row_already_failing_does_not_block_a_write() {
    let dir = reading("rows-already");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n  gap = #222222\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column padding=\"gap\"\n      Card()\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "gap", "value": "#333333"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The map is the Contract's: a reused retention build's is a fresh one's
/// (the retention oracle's way).
#[test]
fn the_row_map_of_a_reused_build_is_a_fresh_ones() {
    let dir = reading("rows-oracle");
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    std::fs::write(dir.join("app.contract"), CONTRACT).unwrap();
    let root = dir.join("app.contract");
    let first = contract::retention_build(&root, None);
    assert!(!first.rows.is_empty());
    std::fs::write(
        dir.join("app.tamber"),
        DESIGN.replace("scale = one", "scale = gap"),
    )
    .unwrap();
    let reused = contract::retention_build(&root, first.kept);
    assert!(reused.reused);
    let fresh = contract::retention_build(&root, None);
    assert_eq!(reused.rows, fresh.rows);
    assert_eq!(reused.paths, fresh.paths);
    let _ = std::fs::remove_dir_all(&dir);
}

/// D2's budget, through the check's whole entry (the planned design's fold
/// included): 102 distinct rows (six row names over seventeen tokens, each
/// an alias of one base), every one reached by a write to the base, after
/// the best of ten. The Bluesky clone's map is 833 distinct rows (73,151
/// as written), and a write to one of its tokens reaches at most 7 (a
/// probe, not this test).
/// A passing write binds once; a breaking one folds and binds `before` too.
#[test]
fn a_worst_case_write_is_checked_within_the_budget() {
    let names = [
        "padding",
        "margin-top",
        "gap",
        "row-gap",
        "border-radius",
        "width",
    ];
    let tokens: String = (0..17).map(|i| format!("  t{i} = base\n")).collect();
    let design = |base: &str| format!("tamber t\n\ntheme\n  base = {base}\n{tokens}");
    let rows: Vec<contract_lower::rows::Row> = (0..102)
        .map(|i| contract_syntax::Attr {
            name: names[i % names.len()].into(),
            value: contract_syntax::Expr::Str(
                format!("t{}", i / names.len()),
                contract_syntax::Span::default(),
            ),
            span: contract_syntax::Span::default(),
            written: true,
        })
        .map(|attr| contract_lower::rows::Row {
            attr,
            places: vec![contract_lower::rows::Place::plain(
                contract_syntax::Span::default(),
            )],
            fonts: None,
            computed: None,
            within: None,
        })
        .collect();
    let vocab = contract_lower::tamber_vocab();
    let parse = |t: &str| exact_tamber::parse(t).unwrap();
    let before = design("8px");
    // This thread's CPU time, not the wall clock: a loaded machine (the
    // suite's other tests and servers, another agent's builds) takes the
    // core away mid-run, which is no work the check did (x2-perf4's run at
    // load 28 read 15.7 ms of wall clock).
    let cpu = || {
        let mut t = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: a valid timespec for the call to fill.
        let read = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
        assert_eq!(read, 0, "this thread's CPU clock");
        std::time::Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
    };
    let run = |base: &str| {
        let began = cpu();
        let after = parse(&design(base));
        let folded = exact_tamber::check_design(after.clone(), &vocab).unwrap();
        let broken = contract_lower::rows::check(
            &rows,
            &["base"],
            &folded.design,
            &folded,
            || Some(parse(&before)),
            &[],
            &[],
        );
        (broken.len(), cpu() - began)
    };
    // The best of ten (a run takes a few ms of CPU).
    let best = |base: &str, want: usize| {
        (0..10)
            .map(|_| {
                let (n, took) = run(base);
                assert_eq!(n, want);
                took
            })
            .min()
            .unwrap()
    };
    let took = best("12px", 0);
    assert!(took.as_millis() < 10, "a passing write: {took:?}");
    let took = best("#202020", 102);
    assert!(took.as_millis() < 10, "a breaking write: {took:?}");
}

/// `reading(tag)` with the fonts fixture's two files in its assets, and the
/// contract's declarations of them: one family with a bold face, one
/// without.
fn with_fonts(tag: &str) -> std::path::PathBuf {
    let dir = reading(tag);
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/fonts/assets");
    for name in ["DejaVuSans.ttf", "DejaVuSans-Bold.ttf"] {
        std::fs::copy(fixtures.join(name), dir.join("assets").join(name)).unwrap();
    }
    dir
}

const FONTS: &str = "use Card from \"@acme/reading\"\nfont \"Fixture Sans\"\n  400 = \"assets/DejaVuSans.ttf\"\n  700 = \"assets/DejaVuSans-Bold.ttf\"\nfont \"Regular Only\" = \"assets/DejaVuSans.ttf\"\n";

/// Stage 13 D1: a write to a family token that leaves `font-weight=700`
/// beside it on a family with no bold face is refused as the build
/// refuses it (`lower-font-face`), through the node's `font-family`, which
/// the weight's own row doesn't name.
#[test]
fn a_family_write_breaking_a_weight_beside_it_is_refused() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  family.text = \"Fixture Sans\"\n";
    let contract = format!("{FONTS}component App\n  view\n    column\n      text \"x\" font-family=\"family.text\" font-weight=700\n      Card()\n");
    same_words(
        &with_fonts("rows-font-weight"),
        design,
        &contract,
        json!({"op": "set-token", "name": "family.text", "value": "\"Regular Only\""}),
        &design.replace("\"Fixture Sans\"", "\"Regular Only\""),
    );
}

/// The same through the node's class: lowering reads a class's
/// `font-family` as the node's own.
#[test]
fn a_family_write_through_a_class_is_refused() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  family.text = \"Fixture Sans\"\n";
    let contract = format!("{FONTS}style Body\n  font-family = \"family.text\"\ncomponent App\n  view\n    column\n      text \"x\" class=Body font-style=\"normal\" font-weight=700\n      Card()\n");
    same_words(
        &with_fonts("rows-font-class"),
        design,
        &contract,
        json!({"op": "set-token", "name": "family.text", "value": "\"Regular Only\""}),
        &design.replace("\"Fixture Sans\"", "\"Regular Only\""),
    );
}

/// A type token whose write asks for a bold face its family doesn't ship
/// (`type_tokens`' `bundled_faces`), and one whose write makes it a family
/// token, which `font` doesn't name (`tamber-kind`).
#[test]
fn a_type_token_write_is_judged_as_the_build_judges_it() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  body = 400 1rem \"Regular Only\"\n";
    let contract = format!(
        "{FONTS}component App\n  view\n    column\n      text \"x\" font=\"body\"\n      Card()\n"
    );
    same_words(
        &with_fonts("rows-font-face"),
        design,
        &contract,
        json!({"op": "set-token", "name": "body", "value": "700 1rem \"Regular Only\""}),
        &design.replace("400 1rem", "700 1rem"),
    );
    same_words(
        &with_fonts("rows-font-kind"),
        design,
        &contract,
        json!({"op": "set-token", "name": "body", "value": "\"Regular Only\""}),
        &design.replace("400 1rem \"Regular Only\"", "\"Regular Only\""),
    );
}

/// A write the build accepts is accepted: the family made one with a bold
/// face under `font-weight=700`, and a type token's weight its family
/// ships.
#[test]
fn a_font_write_the_build_accepts_is_accepted() {
    let dir = with_fonts("rows-font-ok");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  family.text = \"Regular Only\"\n  body = 400 1rem \"Fixture Sans\"\n";
    let contract = format!("{FONTS}component App\n  view\n    column\n      text \"x\" font-family=\"family.text\" font-weight=400\n      text \"y\" font=\"body\"\n      Card()\n");
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), &contract).unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "body", "value": "700 1rem \"Fixture Sans\""}),
    );
    assert!(w.get("ok").is_some(), "a shipped bold face: {w}");
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "family.text", "value": "\"Fixture Sans\""}),
    );
    assert!(w.get("ok").is_some(), "a family with the weight: {w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Stage 13 D2: a computed string in a number row may name any number
/// token, so a write making one unfit for the row is refused as the build
/// refuses it (`theme::measure_row`'s words): a state's name, a template
/// with holes, and a choice of a literal and a name.
#[test]
fn a_computed_number_row_is_judged_against_every_number_token() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n";
    for (tag, row) in [
        ("rows-computed-name", "pick"),
        ("rows-computed-template", "`${pick}`"),
        ("rows-computed-choice", "(on ? \"one\" : pick)"),
    ] {
        let contract = format!("use Card from \"@acme/reading\"\ncomponent App\n  state on = false\n  state pick = \"2\"\n  view\n    column\n      button \"Go\" -exact-press-scale={row}\n      Card()\n");
        same_words(
            &reading(tag),
            design,
            &contract,
            json!({"op": "set-token", "name": "one", "value": "0"}),
            &design.replace("one = 1", "one = 0"),
        );
    }
}

/// A literal arm beside a computed one is judged as the build judges it,
/// at its literal: removing the token it names.
#[test]
fn a_literal_arm_beside_a_computed_one_is_judged() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  mute = #777777\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  state on = false\n  state c = \"#000000\"\n  view\n    column color=(on ? \"mute\" : c)\n      Card()\n";
    same_words(
        &reading("rows-computed-arm"),
        design,
        contract,
        json!({"op": "remove-token", "name": "mute"}),
        &design.replace("  mute = #777777\n", ""),
    );
}

/// What the build accepts of a computed row is accepted: a number token
/// still fit for the row, a computed colour row naming no literal token,
/// and a length row's computed string, which the kernel reads at run time.
#[test]
fn a_computed_row_the_build_accepts_is_accepted() {
    let dir = reading("rows-computed-ok");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n  gap = 8px\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  state pick = \"2\"\n  state c = \"#000000\"\n  state g = \"gap\"\n  view\n    column color=c padding=g\n      button \"Go\" -exact-press-scale=pick\n      Card()\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), contract).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    for (name, value) in [("one", "2"), ("gap", "#333333")] {
        let w = transaction(
            &mut s,
            &dir,
            json!({"op": "set-token", "name": name, "value": value}),
        );
        assert!(w.get("ok").is_some(), "{name}: {w}");
    }
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A `font` choice is judged whole, as `tokens_named` judges it (Astra's
/// D1 round 1): a write making one arm's token write other longhands than
/// the other's is refused (`tamber-type-longhands`), though each arm alone
/// passes.
#[test]
fn a_font_choice_is_judged_whole() {
    let design =
        "tamber t\n\ntheme\n  ink = #111111\n  one = 400 1rem Georgia\n  two = 400 1rem Georgia\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  state on = false\n  view\n    column\n      text \"x\" font=(on ? \"one\" : \"two\")\n      Card()\n";
    same_words(
        &reading("rows-font-choice"),
        design,
        contract,
        json!({"op": "set-token", "name": "one", "value": "italic 400 1rem Georgia"}),
        &design.replace("one = 400", "one = italic 400"),
    );
}

/// A write that makes a type token vary by an app axis, written on a node
/// with children, is refused as the build refuses it
/// (`tamber-inherited-axis`); on a text with none it is accepted (Grok's
/// D1 round 1).
#[test]
fn a_type_token_made_to_vary_on_a_container_is_refused() {
    let design = "tamber t\n\naxes\n  density = regular | roomy\n\ntheme\n  ink = #111111\n  vary = 400 1rem Georgia\n    when density=roomy: 400 1.25rem Georgia\n  body = 400 1rem Georgia\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column font=\"body\"\n      text \"x\"\n      Card()\n";
    same_words(
        &reading("rows-font-axis"),
        design,
        contract,
        json!({"op": "set-token", "name": "body", "value": "vary"}),
        &design.replace("body = 400 1rem Georgia", "body = vary"),
    );
    let dir = reading("rows-font-axis-ok");
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      text \"x\" font=\"body\"\n      Card()\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "body", "value": "vary"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// `font-style="italic"` beside a family token, which a write makes a
/// family with no italic face: refused in the build's words.
#[test]
fn a_family_write_breaking_an_italic_beside_it_is_refused() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  family.text = \"Both\"\n";
    let contract = format!("{FONTS}font \"Both\"\n  400 = \"assets/DejaVuSans.ttf\"\n  400 italic = \"assets/DejaVuSans-Bold.ttf\"\ncomponent App\n  view\n    column\n      text \"x\" font-family=\"family.text\" font-style=\"italic\"\n      Card()\n");
    same_words(
        &with_fonts("rows-font-italic"),
        design,
        &contract,
        json!({"op": "set-token", "name": "family.text", "value": "\"Fixture Sans\""}),
        &design.replace("\"Both\"", "\"Fixture Sans\""),
    );
}

/// A computed row whose type its scope doesn't give (the item of a `for`
/// over a call's list) is the build's: a number row's computed string reads
/// every number token, which the build judges. Not refused here; the build
/// refuses it.
#[test]
fn a_computed_row_the_check_cannot_type_is_the_builds() {
    let dir = reading("rows-computed-local");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\nfn picksOf(x: string): string[] = [x]\ncomponent App\n  state pick = \"2\"\n  view\n    column\n      for p in picksOf(pick) key=p\n        button \"Go\" -exact-press-scale=p\n      Card()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "one", "value": "0"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    assert!(contract::compile_path(&dir.join("app.contract")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Stage 13 D3: an `svg` subtree's rows are judged as lowering lowers
/// them, renamed and coerced first (`svg::coerce_lengths`): a rect's `x`
/// is a row, refused as a colour in the build's words.
#[test]
fn an_svg_elements_row_is_judged() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  pos = 8px\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      svg width=10 height=10 viewBox=\"0 0 10 10\"\n        rect x=\"pos\" y=\"2\" width=10 height=10\n      Card()\n";
    same_words(
        &reading("rows-svg"),
        design,
        contract,
        json!({"op": "set-token", "name": "pos", "value": "#123456"}),
        &design.replace("pos = 8px", "pos = #123456"),
    );
}

/// A write removing the last number token, or making it a colour, leaves a
/// computed string in a number row with none to name: refused as the build
/// refuses it (Astra's D2 round 1), though the write touches no number
/// token of the design after it.
#[test]
fn a_write_taking_the_last_number_token_is_judged() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  state pick = \"2\"\n  view\n    column\n      button \"Go\" -exact-press-scale=pick\n      Card()\n";
    same_words(
        &reading("rows-computed-removed"),
        design,
        contract,
        json!({"op": "remove-token", "name": "one"}),
        &design.replace("  one = 1\n", ""),
    );
    same_words(
        &reading("rows-computed-retyped"),
        design,
        contract,
        json!({"op": "set-token", "name": "one", "value": "#123456"}),
        &design.replace("one = 1", "one = #123456"),
    );
}

/// A computed row whose form doesn't say its type (a call) is the build's:
/// a user `fn` returning a number, under `font-weight` beside a family
/// token, isn't judged with the builtin of its name (Astra's D2 round 1).
#[test]
fn a_computed_call_is_the_builds() {
    let dir = reading("rows-computed-call");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\nfn toString(x: number): number = x\ncomponent App\n  view\n    column\n      text \"a\" font-family=\"one\" font-weight=toString(400)\n      Card()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(&mut s, &dir, json!({"op": "remove-token", "name": "one"}));
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A module tag's text row is refused by lowering before any check of it
/// (`native::refused`), so it never passed: a write making its type token
/// a family is not the write's (Astra's D1 round 2).
#[test]
fn a_module_tags_text_row_is_not_judged() {
    let dir = reading("rows-font-module");
    // The module the view shows is one the app declares.
    let manifest = dir.join("app.json");
    let mut json: Value = std::fs::read_to_string(&manifest)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({"name": "T", "app": {"id": "com.exact.t", "name": "T"}}));
    json["modules"] = json!(["my-view"]);
    std::fs::write(&manifest, json.to_string()).unwrap();
    let design = "tamber t\n\ntheme\n  ink = #111111\n  body = 400 1rem Georgia\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      my-view font=\"body\"\n      Card()\n",
    )
    .unwrap();
    let src = std::fs::read_to_string(dir.join("app.contract")).unwrap();
    let built = contract::compile_path_source_all(&dir.join("app.contract"), &src, false)
        .err()
        .unwrap_or_default();
    assert!(
        built.iter().any(|e| e.id == "lower-native-attr"),
        "{built:?}"
    );
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "body", "value": "\"Georgia\""}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    // The build still says only the module tag's refusal of the row.
    let src = std::fs::read_to_string(dir.join("app.contract")).unwrap();
    let after = contract::compile_path_source_all(&dir.join("app.contract"), &src, false)
        .err()
        .unwrap_or_default();
    assert!(
        after.iter().any(|e| e.id == "lower-native-attr"),
        "{after:?}"
    );
    assert!(!after.iter().any(|e| e.id == "tamber-kind"), "{after:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A grouped list's label folded from an `if` chain of texts is one text
/// lowering builds at its first arm's place: the arms' rows are the
/// build's, which refuses the write at the first arm; the check never
/// refuses it at the second (Astra's D1 round 2).
#[test]
fn a_folded_labels_arms_are_the_builds() {
    let dir = reading("rows-folded-label");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  mute = #777777\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  state on = false\n  view\n    column\n      list appearance=\"auto\"\n        section\n          header\n            if on\n              text \"A\" color=\"mute\"\n            else\n              text \"B\" color=\"mute\"\n          row\n            text \"row\"\n      Card()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(&mut s, &dir, json!({"op": "remove-token", "name": "mute"}));
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    // The build refuses it, at the first arm only.
    let src = std::fs::read_to_string(dir.join("app.contract")).unwrap();
    let built: Vec<u32> = contract::compile_path_source_all(&dir.join("app.contract"), &src, false)
        .err()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.id == "tamber-unknown-token")
        .map(|e| e.span.line)
        .collect();
    assert_eq!(built, [10]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// An svg element lowering refuses before its rows (`padding` on a `rect`)
/// is the build's, its rows with it: a write breaking its `x` is not
/// refused here in other words than the build's (Astra's D3 round 1).
#[test]
fn an_element_lowering_refuses_first_is_the_builds() {
    let dir = reading("rows-svg-refused");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  pos = 8px\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      svg width=10 height=10 viewBox=\"0 0 10 10\"\n        rect padding=1 x=\"pos\" width=10 height=10\n      Card()\n",
    )
    .unwrap();
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(built.id, "lower-svg-attr");
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "pos", "value": "#123456"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A look a marked row yields to a grouped list's (`-exact-row-look`) is
/// dropped before lowering checks it (`row_look::yielded`): never judged,
/// so a write breaking it is accepted, as the build accepts it (Astra's D3
/// round 2).
#[test]
fn a_yielded_row_look_is_not_judged() {
    let dir = reading("rows-row-look");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      list appearance=\"auto\" height=300\n        section\n          svg width=20 height=20 -exact-row-look=\"platform\" padding-left=\"gap\"\n      Card()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file whose whole build fails before the write, for a reason of its own
/// (an `svg` with a positional argument, which lowering refuses before its
/// rows and its children's), is refused in the build's own words: once the
/// background build says so, no row of it is the write's (the reviews'
/// round 3 of stage 13 D).
#[test]
fn a_file_whose_build_fails_leaves_its_rows_to_the_build() {
    let dir = reading("rows-build-fails");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  pos = 8px\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      svg \"oops\" width=20 height=20\n        rect x=\"pos\" width=10 height=10\n      Card()\n",
    )
    .unwrap();
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(built.id, "lower-positional");
    let mut s = Serve::start(&dir, &[]);
    // The background whole build's verdict: every answer `rebuild`.
    let ink = json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]});
    let mut said = false;
    for _ in 0..200 {
        if s.ask(ink.clone()).0["ok"]["class"] == "rebuild" {
            said = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(said, "the whole build's failure is never said");
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "pos", "value": "#123456"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// An `audio` without controls shows nothing: lowering replaces its
/// `display` (`media::audio_rows`), so the authored one is never judged,
/// and a write breaking it is accepted, as the build accepts it (Astra's
/// round of 700c41d3d0).
#[test]
fn an_audios_display_is_the_builds() {
    let dir = reading("rows-audio");
    let design = "tamber t\n\ntheme\n  ink = #111111\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  state on = false\n  state pick = \"block\"\n  view\n    column\n      audio \"https://example.com/a.mp3\" display=(on ? \"block\" : pick)\n      Card()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "add-token", "name": "block", "value": "4px"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A write that leaves an element's `font-family` one lowering can't read
/// (a family token removed, its name now a literal list it refuses) stops
/// that element and everything inside it before any row: a row inside it
/// the same write breaks is the build's (Astra's round of 700c41d3d0).
#[test]
fn a_family_lowering_cant_read_leaves_the_rows_inside_it() {
    let dir = reading("rows-family-list");
    let design =
        "tamber t\n\ntheme\n  ink = #111111\n  family.text: family = system-ui\n  pos = 8px\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      svg width=20 height=20 font-family=\"family.text\"\n        rect x=\"pos\" width=10 height=10\n      Card()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = s
        .ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": super::tamber_serve::digest(&dir.join("app.tamber"))}, "ops": [{"op": "remove-token", "name": "family.text"}, {"op": "set-token", "name": "pos", "value": "#444444"}]}))
        .0;
    let refused: Vec<&Value> = w["error"]["diagnostics"]
        .as_array()
        .map(|d| d.iter().filter(|d| d["file"] == "app.contract").collect())
        .unwrap_or_default();
    assert!(refused.is_empty(), "{w}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A marked box that is no grouped list's section row keeps its look, which
/// the build judges first, in source order: the write is refused at it, as
/// the build refuses it (Grok's round of 700c41d3d0).
#[test]
fn a_marked_box_outside_a_section_keeps_its_look() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      column -exact-row-look=\"platform\" padding-left=\"gap\" margin-left=\"gap\"\n      Card()\n";
    same_words(
        &reading("rows-row-look-stray"),
        design,
        contract,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
        &design.replace("gap = 8px", "gap = #444444"),
    );
}

/// A write that fixes the file's failing row is accepted, before the
/// background whole build's verdict and after it (theme2's 17:46 condition
/// on the failed-build gate), and the build passes after it.
#[test]
fn a_write_fixing_the_failing_row_is_accepted_before_and_after_the_verdict() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = #222222\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column padding=\"gap\"\n      Card()\n";
    let fix = json!({"op": "set-token", "name": "gap", "value": "8px"});
    let ink = json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]});
    for wait in [false, true] {
        let dir = reading(if wait {
            "rows-fix-after"
        } else {
            "rows-fix-before"
        });
        std::fs::write(dir.join("app.tamber"), design).unwrap();
        std::fs::write(dir.join("app.contract"), contract).unwrap();
        assert!(contract::compile_path(&dir.join("app.contract")).is_err());
        // Before the verdict for certain: the background build held at its
        // gate until the fixing write is answered.
        let gate = std::path::PathBuf::from(format!("{}.gate", dir.display()));
        let _ = std::fs::remove_file(&gate);
        let env: Vec<(&str, &str)> = match wait {
            false => vec![("EXACT_TAMBER_CHECK_GATE", gate.to_str().unwrap())],
            true => Vec::new(),
        };
        let mut s = Serve::start(&dir, &env);
        s.ask(ink.clone());
        if wait {
            let mut said = false;
            for _ in 0..200 {
                if s.ask(ink.clone()).0["ok"]["class"] == "rebuild" {
                    said = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            assert!(said, "the whole build's failure is never said");
        }
        let w = transaction(&mut s, &dir, fix.clone());
        assert!(w.get("ok").is_some(), "wait {wait}: {w}");
        std::fs::write(&gate, "").unwrap();
        drop(s);
        let _ = std::fs::remove_file(&gate);
        contract::compile_path(&dir.join("app.contract")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// An `audio`'s `display` is lowering's to write from its class's rows and
/// its own: never judged, whatever its class says (Astra's re-check of
/// 27657b9615).
#[test]
fn an_audios_display_with_a_class_is_the_builds() {
    let dir = reading("rows-audio-class");
    let design = "tamber t\n\ntheme\n  ink = #111111\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\nstyle Hidden\n  display = \"none\"\ncomponent App\n  state on = false\n  view\n    column\n      audio \"https://example.com/a.mp3\" class=Hidden controls=on\n      Card()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "add-token", "name": "block", "value": "4px"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A marked box in a `section` outside a grouped list keeps its look: the
/// build judges it first, and the write is refused there in its words
/// (Astra's re-check of 08e003cbc2).
#[test]
fn a_marked_box_in_a_plain_section_keeps_its_look() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      section\n        column -exact-row-look=\"platform\" padding-left=\"gap\" margin-left=\"gap\"\n      Card()\n";
    same_words(
        &reading("rows-row-look-section"),
        design,
        contract,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
        &design.replace("gap = 8px", "gap = #444444"),
    );
}

/// A swipe written by hand in a grouped list's section (a `scroll` naming
/// its row as `swipeContent`): the row it names yields its look as a
/// section's row does, so a write breaking it is accepted, as the build
/// accepts it (Grok's re-check of af82d07cbf).
#[test]
fn a_hand_written_swipes_row_yields_its_look() {
    let dir = reading("rows-row-look-swipe");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  state z = 0\n  action noop\n    z = 1\n  view\n    column\n      list appearance=\"auto\" height=300\n        section\n          scroll swipeContent=\"row-1\" swipeTrailing=\"del-1\" overflow-x=\"scroll\" overflow-y=\"hidden\" height=50 display=\"flex\" flex-direction=\"row\"\n            row id=\"row-1\" width=\"100%\" flex-shrink=0 -exact-row-look=\"platform\" padding-left=\"gap\"\n              text \"x\"\n            button id=\"del-1\" width=40 press=noop\n              text \"d\"\n      Card()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A wrapper in a hand-written swipe (a row holding the content and its
/// action) is no grouped row: its look is judged, first in source order, in
/// the build's words, while the content it holds yields its own (Astra's
/// re-check of 53c45e6877).
#[test]
fn a_marked_wrapper_in_a_swipe_keeps_its_look() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  state z = 0\n  action noop\n    z = 1\n  view\n    column\n      list appearance=\"auto\" height=300\n        section\n          scroll swipeContent=\"row-1\" swipeTrailing=\"del-1\" overflow-x=\"scroll\" overflow-y=\"hidden\" height=50 display=\"flex\" flex-direction=\"row\"\n            row -exact-row-look=\"platform\" padding-left=\"gap\" margin-left=\"gap\"\n              row id=\"row-1\" width=\"100%\" flex-shrink=0 -exact-row-look=\"platform\" padding-left=\"gap\"\n                text \"x\"\n              button id=\"del-1\" width=40 press=noop\n                text \"d\"\n      Card()\n";
    same_words(
        &reading("rows-row-look-wrapper"),
        design,
        contract,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
        &design.replace("gap = 8px", "gap = #444444"),
    );
}

/// A component used as a section's row with another use of it inside: the
/// outer's look yields, the inner's (the same declaration, another
/// instance) is judged, in the build's words (Astra's re-check of
/// 0c1dd8f1a3).
#[test]
fn a_nested_instance_of_a_yielding_row_keeps_its_look() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      list appearance=\"auto\" height=300\n        section\n          Plate()\n            Plate()\n              text \"x\"\n      Card()\ncomponent Plate\n  slot\n  view\n    column -exact-row-look=\"platform\" padding-left=\"gap\"\n      children\n";
    same_words(
        &reading("rows-row-look-nested"),
        design,
        contract,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
        &design.replace("gap = 8px", "gap = #444444"),
    );
}

/// A section's row put in its own swipe (`delete=`) is given to the
/// grouped row rebuilt: it yields its look all the same, and a write
/// breaking it is accepted, as the build accepts it.
#[test]
fn a_deletable_rows_look_yields() {
    let dir = reading("rows-row-look-delete");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  state z = 0\n  action remove(id: string)\n    z = 1\n  view\n    column\n      list appearance=\"auto\" height=300\n        section\n          row delete=remove(\"1\") -exact-row-look=\"platform\" padding-left=\"gap\"\n            text \"One\"\n      Card()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two nested uses of one swipe component share their root's span: the
/// outer swipe naming the inner as its content yields the inner's look, so
/// a write breaking it is accepted, as the build accepts it (Astra's
/// re-check of df08adcb4a).
#[test]
fn a_nested_swipe_components_content_yields_its_look() {
    let dir = reading("rows-row-look-nested-swipe");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      list appearance=\"auto\" height=300\n        section\n          Swipe(name=\"a\", inner=\"b\", pad=\"0px\")\n            Swipe(name=\"b\", inner=\"c\", pad=\"gap\")\n              row id=\"c\"\n                text \"x\"\n      Card()\ncomponent Swipe\n  props\n    name: string\n    inner: string\n    pad: string\n  slot\n  view\n    scroll id=name swipeContent=inner -exact-row-look=\"platform\" padding-right=pad overflow-x=\"scroll\" overflow-y=\"hidden\" height=50 display=\"flex\" flex-direction=\"row\"\n      children\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
    );
    assert!(w.get("ok").is_some(), "{w}");
    drop(s);
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
