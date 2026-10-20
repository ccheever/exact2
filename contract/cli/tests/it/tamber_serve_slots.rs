//! The design server's slot check (LLP 1139 D4, LLP 1121 stage 12 D2): a
//! write or a scrub's resting value that would hide an exported library
//! component's slot fill (a node of its chain `opacity: 0`, `width: 0` …,
//! in any tuple or scale column) is refused, with the compiler's own
//! words, by the compiler's own checker; a write that reaches no chain
//! isn't asked; a write to a base name a chain reaches through an alias is.
#![cfg(unix)]

use crate::tamber_serve::{digest, Serve};
use serde_json::{json, Value};

/// An app using `Card` from a private library package `@acme/lib` whose
/// slot chain reads `lib.op` (opacity) and `lib.w` (width, an alias of
/// `space.md`, a cell or more in every column), with `app` as its design.
fn app(tag: &str, design: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-serve-slots-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let lib = dir.join("node_modules/@acme/lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    std::fs::write(
        lib.join("index.contract"),
        "component Card\n  slot\n  view\n    column opacity=\"lib.op\" width=\"lib.w\"\n      children\n",
    )
    .unwrap();
    std::fs::write(
        lib.join("lib.tamber"),
        "tamber lib\n\ntheme\n  lib.op = 1\n  lib.w = space.md\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    dir
}

/// The compiler's refusals of the app as it is on disk.
fn compiled(dir: &std::path::Path) -> Vec<String> {
    let app = dir.join("app.contract");
    let src = std::fs::read_to_string(&app).unwrap();
    match contract::compile_path_source_all(&app, &src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all
            .into_iter()
            .filter(|e| e.id == "lower-anatomy-slot")
            .map(|e| e.message)
            .collect(),
    }
}

const DESIGN: &str = "tamber app\n\naxes\n  density = roomy | tight\n\ntheme\n  other = 2px\n";

fn refusals(reply: &Value) -> Vec<String> {
    reply["error"]["diagnostics"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter(|d| d["id"] == "lower-anatomy-slot")
                .map(|d| d["message"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_write_or_a_resting_scrub_that_hides_a_library_slot_is_refused_as_the_compiler_refuses_it() {
    let dir = app("refused", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    // `lib.op` 0 in one tuple: the write refused, nothing written.
    let op = json!([{"op": "set-token", "name": "lib.op", "value": "1"}, {"op": "set-token", "name": "lib.op", "value": "0", "when": [{"axis": "density", "value": "tight"}]}]);
    let (write, _) =
        s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": op}));
    let said = refusals(&write);
    assert_eq!(said.len(), 1, "{write}");
    assert_eq!(
        std::fs::read_to_string(dir.join("app.tamber")).unwrap(),
        DESIGN
    );
    // A scrub resting there: refused by its verify, the same words.
    let (scrub, _) = s.ask(
        json!({"method": "scrub-fold", "open": true, "verify": true, "files": files(), "ops": op}),
    );
    assert_eq!(refusals(&scrub), said, "{scrub}");
    // The compiler's words for that value, the app built with it: the same.
    std::fs::write(
        dir.join("app.tamber"),
        "tamber app\n\naxes\n  density = roomy | tight\n\ntheme\n  other = 2px\n  lib.op = 1\n    when density=tight: 0\n",
    )
    .unwrap();
    assert_eq!(compiled(&dir), said);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_write_to_a_base_name_a_chain_reaches_through_an_alias_is_checked_one_reaching_none_is_not() {
    let dir = app("closure", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    // `space.md` 0: `lib.w` is `space.md`, the chain's width 0.
    let (base, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-token", "name": "space.md", "value": "0px"}]}));
    assert_eq!(refusals(&base).len(), 1, "{base}");
    // (said in the app's design, never a package's, at the line the write gives `space.md`: reload1's 7dbd6b76ae)
    let at = &base["error"]["diagnostics"][0];
    let line = std::fs::read_to_string(dir.join("app.tamber"))
        .unwrap()
        .lines()
        .count() as u64
        + 1;
    assert_eq!(
        (at["file"].as_str(), at["line"].as_u64()),
        (Some("app.tamber"), Some(line)),
        "{base}"
    );
    // A write no chain reaches is taken, even where a chain's own token is
    // already one the check refuses (the app's line for `lib.op` is 0).
    let bad = "tamber app\n\ntheme\n  other = 2px\n  lib.op = 0\n";
    std::fs::write(dir.join("app.tamber"), bad).unwrap();
    let (other, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-token", "name": "other", "value": "3px"}]}));
    assert!(other["ok"].is_object(), "{other}");
    // …and one that touches it again is not.
    let (again, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-token", "name": "lib.op", "value": "-0.5"}]}));
    assert_eq!(refusals(&again).len(), 1, "{again}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_set_param_that_hides_a_library_slot_is_refused_too() {
    let dir = app("param", DESIGN);
    std::fs::write(
        dir.join("node_modules/@acme/lib/index.contract"),
        "component Card\n  slot\n  params\n    op: number = \"lib.op\"\n  view\n    column opacity=\"param.op\" width=\"lib.w\"\n      children\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    let (ok, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-param", "name": "Card.op", "value": "0.5"}]}));
    assert!(ok["ok"].is_object(), "{ok}");
    let (zero, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-param", "name": "Card.op", "value": "0"}]}));
    assert_eq!(refusals(&zero).len(), 1, "{zero}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The check's cost on a candidate fold, as a scrub's verify pays it:
/// under 25 ms on this fixture (theme2's figure is about 10 on the clone).
#[test]
fn the_slot_check_is_cheap() {
    let dir = app("cost", DESIGN);
    let app = contract::tamber_params::params_of(&dir.join("app.contract"));
    let chains = contract_lower::anatomy::slot::SlotChains::from_ir(
        &contract::tamber_params::library(&app, None),
    );
    let (folding, _) =
        contract::tamber_params::fold_with(DESIGN, &app, &contract_lower::tamber_vocab()).unwrap();
    let t = std::time::Instant::now();
    for _ in 0..100 {
        assert!(
            contract_lower::anatomy::slot::check_slots(&folding, None, &chains, &["lib.op"])
                .is_empty()
        );
    }
    let each = t.elapsed() / 100;
    assert!(
        each < std::time::Duration::from_millis(25),
        "{each:?} a check"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scrub_whose_package_design_moved_since_it_opened_is_a_conflict_at_its_verify() {
    let dir = app("scrub-stale", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    let op = json!([{"op": "set-token", "name": "other", "value": "4px"}]);
    let (open, _) =
        s.ask(json!({"method": "scrub-fold", "open": true, "files": files(), "ops": op}));
    assert!(open["ok"].is_object(), "{open}");
    // The package's default for `lib.op` moves to 0 under the open scrub.
    std::fs::write(
        dir.join("node_modules/@acme/lib/lib.tamber"),
        "tamber lib\n\ntheme\n  lib.op = 0\n  lib.w = space.md\n",
    )
    .unwrap();
    let (verify, _) =
        s.ask(json!({"method": "scrub-fold", "verify": true, "files": files(), "ops": op}));
    assert_eq!(verify["error"]["conflict"], true, "{verify}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_propose_and_a_removed_override_are_checked_too() {
    let dir = app(
        "removed",
        "tamber app\n\ntheme\n  other = 2px\n  radius.sm = 8px\n",
    );
    // The chain's width a bare base name: the app's 8px, the base's own in each column once removed.
    std::fs::write(
        dir.join("node_modules/@acme/lib/index.contract"),
        "component Card\n  slot\n  view\n    column opacity=\"lib.op\" width=\"radius.sm\"\n      children\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    // A propose is checked as a transaction is.
    let (proposed, _) = s.ask(json!({"method": "propose", "files": files(), "ops": [{"op": "set-token", "name": "lib.op", "value": "0"}]}));
    assert_eq!(refusals(&proposed).len(), 1, "{proposed}");
    // Removing the app's radius.sm gives the chain the base's: 0 cells on the terminal.
    let (removed, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "remove-token", "name": "radius.sm"}]}));
    assert_eq!(refusals(&removed).len(), 1, "{removed}");
    // (said at the line it removes, the design as it was: never 1:1)
    let at = &removed["error"]["diagnostics"][0];
    assert_eq!(
        (at["file"].as_str(), at["line"].as_u64()),
        (Some("app.tamber"), Some(5)),
        "{removed}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_removed_override_is_checked_when_the_design_it_replaces_did_not_fold() {
    // An unrelated `bad = missing` keeps the design as it was from folding: the plan's changed
    // names (two folds compared) have no `radius.sm`, the names the write removes do.
    let dir = app(
        "removed-unfolded",
        "tamber app\n\ntheme\n  other = 2px\n  radius.sm = 8px\n  bad = missing\n",
    );
    std::fs::write(
        dir.join("node_modules/@acme/lib/index.contract"),
        "component Card\n  slot\n  view\n    column width=\"radius.sm\"\n      children\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    let (removed, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "remove-token", "name": "radius.sm"}, {"op": "remove-token", "name": "bad"}]}));
    assert_eq!(refusals(&removed).len(), 1, "{removed}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn removing_an_app_only_token_a_library_row_names_is_refused_as_the_build_refuses_it() {
    // The chain's width `cards.pad`, which only the app defines: removed, the row reads a name the fold lacks.
    let dir = app(
        "app-only",
        "tamber app\n\ntheme\n  other = 2px\n  cards.pad = 16px\n",
    );
    std::fs::write(
        dir.join("node_modules/@acme/lib/index.contract"),
        "component Card\n  slot\n  view\n    column width=\"cards.pad\"\n      children\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    let (removed, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "remove-token", "name": "cards.pad"}]}));
    assert_eq!(refusals(&removed).len(), 1, "{removed}");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber app\n\ntheme\n  other = 2px\n",
    )
    .unwrap();
    assert_eq!(compiled(&dir), refusals(&removed));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_package_default_the_app_replaced_selects_no_chain() {
    // The app's own `lib.w` (8px) replaces the package's `space.md`; `lib.op` is already 0.
    let dir = app(
        "replaced",
        "tamber app\n\ntheme\n  other = 2px\n  lib.w = 8px\n  lib.op = 0\n",
    );
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    // A write to space.md: no chain reads it in the candidate, so none is asked.
    let (base, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-token", "name": "space.md", "value": "20px"}]}));
    assert!(base["ok"].is_object(), "{base}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_write_to_a_token_a_rows_expression_names_is_checked() {
    // The chain's width an expression over `lib.w`, which the compiler can't fold on a chain row and refuses as
    // written (reload1's 64d8760971): a write to `lib.w` reaches it, refused in the compiler's own words.
    let dir = app("expr", DESIGN);
    std::fs::write(
        dir.join("node_modules/@acme/lib/index.contract"),
        "component Card\n  slot\n  view\n    column opacity=\"lib.op\" width=\"calc(lib.w * 2)\"\n      children\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    let (written, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-token", "name": "lib.w", "value": "12px"}]}));
    assert_eq!(refusals(&written).len(), 1, "{written}");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber app\n\ntheme\n  other = 2px\n  lib.w = 12px\n",
    )
    .unwrap();
    assert_eq!(compiled(&dir), refusals(&written));
    // A write reaching no chain is still not asked.
    let (other, _) = s.ask(json!({"method": "transaction", "client": "t", "files": {"app.tamber": digest(&dir.join("app.tamber"))}, "ops": [{"op": "set-token", "name": "other", "value": "3px"}]}));
    assert!(other["ok"].is_object(), "{other}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_base_name_reached_only_through_the_apps_own_aliases_is_checked() {
    // The chain reads `lib.w`; the app's `lib.w` names its own `cards.pad`, which names `space.md`.
    // (every token whose fold moves is in the plan's changed names, so this holds with or without the
    // checker's own closure; the closure is for a caller passing only the names it wrote)
    let dir = app(
        "app-alias",
        "tamber app\n\ntheme\n  other = 2px\n  cards.pad = space.md\n  lib.w = cards.pad\n",
    );
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": digest(&dir.join("app.tamber"))});
    let (base, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-token", "name": "space.md", "value": "0px"}]}));
    assert_eq!(refusals(&base).len(), 1, "{base}");
    // A scrub's verify, as the server pays it (its plan, its fold, the check on the chain a safe
    // `lib.w` reaches): well under a frame budget's tenfold.
    let op = json!([{"op": "set-token", "name": "lib.w", "value": "12px"}]);
    let (open, _) =
        s.ask(json!({"method": "scrub-fold", "open": true, "files": files(), "ops": op}));
    assert!(open["ok"].is_object(), "{open}");
    let t = std::time::Instant::now();
    for _ in 0..10 {
        let (v, _) =
            s.ask(json!({"method": "scrub-fold", "verify": true, "files": files(), "ops": op}));
        assert!(v["ok"].is_object(), "{v}");
    }
    let each = t.elapsed() / 10;
    assert!(
        each < std::time::Duration::from_millis(250),
        "{each:?} a verify"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// theme2's bar (about 10 ms a candidate) for the check alone, on a
/// clone-sized design: the Bluesky clone's whole design as the app's, the
/// library package beside it; warm, the median of five, for a touched name
/// that reaches the chain and for every chain at once. The figures are
/// printed for the review record.
#[test]
fn the_slot_check_alone_is_under_ten_milliseconds_on_a_clone_sized_design() {
    let clone = include_str!("../../../../tamber/tests/fmt/bluesky-clone.tamber");
    let dir = app("clone-sized", clone);
    let app = contract::tamber_params::params_of(&dir.join("app.contract"));
    let chains = contract_lower::anatomy::slot::SlotChains::from_ir(
        &contract::tamber_params::library(&app, None),
    );
    let (folding, _) =
        contract::tamber_params::fold_with(clone, &app, &contract_lower::tamber_vocab()).unwrap();
    let median = |touched: &[&str]| {
        let _ = contract_lower::anatomy::slot::check_slots(&folding, None, &chains, touched); // (warm)
        let mut runs: Vec<std::time::Duration> = (0..5)
            .map(|_| {
                let t = std::time::Instant::now();
                let _ =
                    contract_lower::anatomy::slot::check_slots(&folding, None, &chains, touched);
                t.elapsed()
            })
            .collect();
        runs.sort();
        runs[2]
    };
    let one = median(&["lib.op"]);
    let all = median(&[]);
    eprintln!("check_slots on the clone's design ({} tokens): one touched name {one:?}, every chain {all:?} (median of 5, warm)", folding.tokens.len());
    assert!(one < std::time::Duration::from_millis(10), "{one:?}");
    assert!(all < std::time::Duration::from_millis(10), "{all:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The lead's 10:15Z ask (stage 12 gate, iosfix's file): check_slots on the
/// clone's design with the real `@exact/cards` and `@exact/actions` (their
/// chains, as apps/library-gate uses every exported component). Needs the
/// workspace's packages installed (`bun install`), so run by hand:
/// `cargo test -p contract --test it real_packages -- --ignored --nocapture`.
#[test]
#[ignore = "needs bun install's node_modules/@exact; run by hand for the gate's figure"]
fn the_slot_check_alone_on_the_clone_with_the_real_packages() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let clone = include_str!("../../../../tamber/tests/fmt/bluesky-clone.tamber");
    let app = contract::tamber_params::params_of(&root.join("apps/library-gate/app.contract"));
    let ir = contract::tamber_params::library(&app, None);
    let chains = contract_lower::anatomy::slot::SlotChains::from_ir(&ir);
    // (the library components with a slot chain, and every token their rows reach)
    let with_chain = ir
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["library"] == true && c["chain"].as_array().is_some_and(|c| !c.is_empty()))
        .count();
    let mut names: Vec<String> = ir
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["chain"].as_array().cloned().unwrap_or_default())
        .flat_map(|n| n["tokens"].as_array().cloned().unwrap_or_default())
        .filter_map(|t| t.as_str().map(str::to_string))
        .collect();
    names.sort();
    names.dedup();
    assert!(
        with_chain > 0,
        "no library chains: is node_modules/@exact installed?"
    );
    let (folding, _) =
        contract::tamber_params::fold_with(clone, &app, &contract_lower::tamber_vocab()).unwrap();
    let median = |touched: &[&str]| {
        let _ = contract_lower::anatomy::slot::check_slots(&folding, None, &chains, touched);
        let mut runs: Vec<std::time::Duration> = (0..5)
            .map(|_| {
                let t = std::time::Instant::now();
                let _ =
                    contract_lower::anatomy::slot::check_slots(&folding, None, &chains, touched);
                t.elapsed()
            })
            .collect();
        runs.sort();
        runs[2]
    };
    let one = median(&[names.first().map(String::as_str).unwrap_or("space.md")]);
    let all = median(&[]);
    eprintln!("check_slots, the clone's design ({} tokens) with @exact/cards and @exact/actions ({} chains, {} names): one touched name {one:?}, every chain {all:?} (median of 5, warm)", folding.tokens.len(), with_chain, names.len());
    assert!(all < std::time::Duration::from_millis(10), "{all:?}");
}
