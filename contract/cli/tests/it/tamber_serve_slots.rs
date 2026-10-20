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
            contract_lower::anatomy::slot::check_slots(&folding, &chains, &["lib.op"]).is_empty()
        );
    }
    let each = t.elapsed() / 100;
    assert!(
        each < std::time::Duration::from_millis(25),
        "{each:?} a check"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
