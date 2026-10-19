//! LLP 1121 stage 11 A2 (the part without a wire change): the design
//! server and `contract tamber` fold the app's design with its packages'
//! tokens, as the build does, so an app token naming a package's is read
//! and edited there too; a refusal in a package's design is said at its
//! line in that file.
#![cfg(unix)]

use super::tamber_serve::{app, Serve};
use serde_json::json;

/// The app at `dir` uses `@acme/reading`, whose design is `design`.
fn with_reading(dir: &std::path::Path, design: &str) {
    let at = dir.join("node_modules/@acme/reading");
    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(
        at.join("package.json"),
        r#"{"name":"@acme/reading","version":"1.0.0","exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    std::fs::write(at.join("index.contract"), "component ReadingCard\n  view\n    column background-color=\"reading.card\"\n      text \"x\"\n").unwrap();
    std::fs::write(at.join("reading.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), "use ReadingCard from \"@acme/reading\"\ncomponent App\n  view\n    column background-color=\"brand\"\n      ReadingCard()\n").unwrap();
}

#[test]
fn the_server_reads_an_app_token_that_names_a_package_token() {
    let dir = app("pkg-alias", "tamber t\n\ntheme\n  brand = reading.card\n");
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = #112233\n");
    let mut s = Serve::start(&dir, &[]);
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "brand"}));
    assert!(resolved["ok"].is_object(), "{resolved}");
    assert!(resolved.to_string().contains("#112233"), "{resolved}");
    let (described, _) = s.ask(json!({"method": "describe"}));
    assert!(described["ok"].is_object(), "{described}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn contract_tamber_check_folds_the_packages_and_says_a_refusal_at_its_file() {
    let dir = app("pkg-check", "tamber t\n\ntheme\n  brand = reading.card\n");
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = #112233\n");
    let check = |dir: &std::path::Path| {
        std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(["tamber", "check"])
            .arg(dir)
            .output()
            .unwrap()
    };
    let ok = check(&dir);
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    // A package token naming nothing: said at its line in the package's file.
    with_reading(
        &dir,
        "tamber reading\n\ntheme\n  reading.card = #112233\n  reading.ink = nowhere\n",
    );
    let bad = check(&dir);
    let said = String::from_utf8_lossy(&bad.stderr);
    assert!(!bad.status.success(), "{said}");
    assert!(said.contains("reading.tamber:5:"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_transaction_on_an_app_token_folds_beside_the_packages_tokens() {
    let dir = app(
        "pkg-edit",
        "tamber t\n\ntheme\n  brand = reading.card\n  ink = #000000\n",
    );
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = #112233\n");
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let files = hello["ok"]["files"].clone();
    // An edit to an unrelated app token plans, though the app names a package token.
    let (edited, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files, "ops": [{"op": "set-token", "name": "ink", "value": "#111111"}]}));
    assert!(edited["ok"].is_object(), "{edited}");
    // A package token is the package's: no app line to set, so refused, and the package file untouched.
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let files = hello["ok"]["files"].clone();
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files, "ops": [{"op": "set-token", "name": "reading.card", "value": "#445566"}]}));
    assert!(refused["error"].is_object(), "{refused}");
    let package =
        std::fs::read_to_string(dir.join("node_modules/@acme/reading/reading.tamber")).unwrap();
    assert!(package.contains("#112233"), "{package}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_server_refuses_what_the_build_refuses_of_a_package_design() {
    // A package token naming the app's token: the build refuses it, and so does the server's fold.
    let dir = app("pkg-refs", "tamber t\n\ntheme\n  brand = #123456\n");
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = brand\n");
    let mut s = Serve::start(&dir, &[]);
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "brand"}));
    assert!(resolved["error"].is_object(), "{resolved}");
    assert!(
        resolved.to_string().contains("reading.tamber"),
        "{resolved}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn contract_tamber_check_reads_an_app_whose_design_is_its_packages_alone() {
    let dir = app("pkg-alone", "tamber t\n\ntheme\n  ink = #000000\n");
    std::fs::remove_file(dir.join("app.tamber")).unwrap();
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = #112233\n");
    std::fs::write(dir.join("app.contract"), "use ReadingCard from \"@acme/reading\"\ncomponent App\n  view\n    column\n      ReadingCard()\n").unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "check"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_package_refusal_names_its_files_lines_in_its_message_too() {
    let dir = app("pkg-unband", "tamber t\n\ntheme\n  brand = reading.card\n");
    with_reading(
        &dir,
        "tamber reading\n\ntheme\n  reading.card = #112233\n  reading.card = #332211\n",
    );
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "check"])
        .arg(&dir)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{said}");
    assert!(
        said.contains("reading.tamber:5:") && said.contains("(line 4)"),
        "{said}"
    );
    assert!(!said.contains("2684354"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_that_leaves_a_package_token_naming_the_apps_is_refused() {
    // The app's override of reading.card is what makes the build valid; removing it would leave the
    // package's own `reading.card = appInk` naming an app token, which the build refuses.
    let dir = app(
        "pkg-unset",
        "tamber t\n\ntheme\n  appInk = #123456\n  reading.card = #445566\n  brand = reading.card\n",
    );
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = appInk\n");
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let files = hello["ok"]["files"].clone();
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files, "ops": [{"op": "remove-token", "name": "reading.card"}]}));
    assert!(refused["error"].is_object(), "{refused}");
    assert!(refused.to_string().contains("appInk"), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tamber_gen_binds_a_packages_only_apps_tokens() {
    let dir = app("pkg-gen", "tamber t\n\ntheme\n  ink = #000000\n");
    std::fs::remove_file(dir.join("app.tamber")).unwrap();
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = #112233\n");
    std::fs::write(dir.join("app.contract"), "use ReadingCard from \"@acme/reading\"\ncomponent App\n  view\n    column\n      ReadingCard()\n").unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "gen", "ts"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ts = std::fs::read_to_string(dir.join(".exact/tamber/tamber.ts")).unwrap();
    assert!(ts.contains("reading.card"), "{ts}");
    let _ = std::fs::remove_dir_all(&dir);
}
