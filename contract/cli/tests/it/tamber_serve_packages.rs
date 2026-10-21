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
    // A package token's set-token is the app's override (*1.14*): a line in the app's design, never the package's.
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let files = hello["ok"]["files"].clone();
    let (set, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files, "ops": [{"op": "set-token", "name": "reading.card", "value": "#445566"}]}));
    assert!(set["ok"].is_object(), "{set}");
    assert_eq!(set["ok"]["changed"][0]["file"], "app.tamber", "{set}");
    let design = std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    assert!(design.contains("reading.card = #445566"), "{design}");
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

#[test]
fn package_rows_name_their_package_and_file_and_the_package_files_are_read_only() {
    let dir = app("pkg-rows", "tamber t\n\ntheme\n  brand = reading.card\n");
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = #112233\n    when dark-style=dim: #000000\n  reading.ink = #222222\n");
    std::fs::write(dir.join("app.tamber"), "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  brand = reading.card\n  reading.ink = #333333\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let packages = &hello["ok"]["packages"];
    assert_eq!(packages[0]["package"], "@acme/reading", "{hello}");
    assert_eq!(
        packages[0]["file"], "@acme/reading/reading.tamber",
        "{hello}"
    );
    assert_eq!(
        packages[0]["digest"].as_str().map(str::len),
        Some(64),
        "{hello}"
    );
    let (described, _) = s.ask(json!({"method": "describe"}));
    let rows = described["ok"]["ir"]["tokens"].as_array().unwrap();
    let row = |n: &str| {
        rows.iter()
            .find(|r| r["name"] == n)
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(
        (
            row("reading.card")["package"].clone(),
            row("reading.card")["file"].clone()
        ),
        (
            json!("@acme/reading"),
            json!("@acme/reading/reading.tamber")
        ),
        "{described}"
    );
    // Set for this app: the package kept, the file the app's design.
    assert_eq!(
        (
            row("reading.ink")["package"].clone(),
            row("reading.ink")["file"].clone()
        ),
        (json!("@acme/reading"), json!("app.tamber")),
        "{described}"
    );
    // The app's own: no package fields.
    assert!(
        row("brand")["package"].is_null() && row("brand")["file"].is_null(),
        "{described}"
    );
    assert!(described["ok"]["packages"].is_array(), "{described}");
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "reading.card"}));
    assert_eq!(
        resolved["ok"]["identity"], "@acme/reading/reading.tamber#reading.card",
        "{resolved}"
    );
    let (explained, _) = s.ask(json!({"method": "explain", "token": "reading.ink"}));
    assert_eq!(explained["ok"]["file"], "app.tamber", "{explained}");
    assert_eq!(
        explained["ok"]["package_value"]["value"], "#222222",
        "{explained}"
    );
    // Ops on a package's design, and add-token of a package's or a base name, are refused.
    let files = hello["ok"]["files"].clone();
    let ask = |s: &mut Serve, op: serde_json::Value| {
        s.ask(json!({"method": "transaction", "client": "t", "files": files, "ops": [op]}))
            .0
    };
    let on_package = ask(
        &mut s,
        json!({"op": "set-token", "name": "reading.card", "value": "#999999", "file": "@acme/reading/reading.tamber"}),
    );
    assert!(on_package.to_string().contains("read-only"), "{on_package}");
    let add_package = ask(
        &mut s,
        json!({"op": "add-token", "name": "reading.card", "value": "#999999"}),
    );
    assert!(
        add_package.to_string().contains("`set-token` sets it"),
        "{add_package}"
    );
    let add_base = ask(
        &mut s,
        json!({"op": "add-token", "name": "space.md", "value": "12px"}),
    );
    assert!(
        add_base.to_string().contains("`set-token` sets it"),
        "{add_base}"
    );
    let add_namespace = ask(
        &mut s,
        json!({"op": "add-token", "name": "reading.extra", "value": "#999999"}),
    );
    assert!(
        add_namespace.to_string().contains("owns its names"),
        "{add_namespace}"
    );
    let set_when = ask(
        &mut s,
        json!({"op": "set-token", "name": "reading.card", "value": "#999999", "when": [{"axis": "dark-style", "value": "dim"}]}),
    );
    assert!(
        set_when.to_string().contains("isn't set for this app yet"),
        "{set_when}"
    );
    // A base name's set-token is the app's line too.
    let set_base = ask(
        &mut s,
        json!({"op": "set-token", "name": "space.md", "value": "12px"}),
    );
    assert!(set_base["ok"].is_object(), "{set_base}");
    assert!(std::fs::read_to_string(dir.join("app.tamber"))
        .unwrap()
        .contains("space.md = 12px"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_override_transform_follows_the_ops_before_it_and_a_scrub_takes_it_too() {
    let dir = app("pkg-batch", "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  brand = reading.card\n");
    with_reading(
        &dir,
        "tamber reading\n\ntheme\n  reading.card = #112233\n  reading.ink = #222222\n",
    );
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": super::tamber_serve::digest(&dir.join("app.tamber"))});
    // Two sets in one batch: the first is the override line, the second edits it.
    let (two, _) = s.ask(json!({"method": "propose", "files": files(), "ops": [{"op": "set-token", "name": "reading.ink", "value": "#333333"}, {"op": "set-token", "name": "reading.ink", "value": "#444444"}]}));
    assert!(two["ok"].is_object(), "{two}");
    // A set, then a `when` for it: allowed once the batch has set it.
    let (when, _) = s.ask(json!({"method": "propose", "files": files(), "ops": [{"op": "set-token", "name": "reading.ink", "value": "#333333"}, {"op": "set-token", "name": "reading.ink", "value": "#000000", "when": [{"axis": "dark-style", "value": "dim"}]}]}));
    assert!(when["ok"].is_object(), "{when}");
    // Renaming an app token onto a package's name, or into its namespace, is refused.
    let (rename, _) = s.ask(json!({"method": "propose", "files": files(), "ops": [{"op": "rename-token", "from": "brand", "to": "reading.extra"}]}));
    assert!(
        rename
            .to_string()
            .contains("rename to a name of the app's own"),
        "{rename}"
    );
    // A revert's or an import's add-token of a package name restores the override line.
    let (restored, _) = s.ask(json!({"method": "propose", "files": files(), "restore": true, "ops": [{"op": "add-token", "name": "reading.ink", "value": "#555555"}]}));
    assert!(restored["ok"].is_object(), "{restored}");
    // A scrub of a package token's first set plans as the write would.
    let (scrub, _) = s.ask(json!({"method": "scrub-fold", "open": true, "files": files(), "ops": [{"op": "set-token", "name": "reading.ink", "value": "#666666"}]}));
    assert!(scrub["ok"].is_object(), "{scrub}");
    // The local wire's own hello lists where package designs are or would be; a client's never has paths.
    let (watched, _) = s.ask(json!({"method": "hello", "paths": true}));
    let roots = watched["ok"]["watch"]["roots"].clone();
    assert!(
        roots
            .as_array()
            .is_some_and(|w| w.iter().any(|p| p.as_str().unwrap().ends_with("reading"))),
        "{watched}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_override_round_trip_and_an_app_design_that_is_a_packages_file() {
    let dir = app("pkg-trip", "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  brand = reading.card\n");
    with_reading(
        &dir,
        "tamber reading\n\ntheme\n  reading.card = #112233\n    when dark-style=dim: #000000\n",
    );
    let mut s = Serve::start(&dir, &[]);
    let files = || json!({"app.tamber": super::tamber_serve::digest(&dir.join("app.tamber"))});
    let (set, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-token", "name": "reading.card", "value": "#445566"}]}));
    let inverse = set["ok"]["inverse"].clone();
    // explain says the package's line, its `when` the override dropped.
    let (explained, _) = s.ask(json!({"method": "explain", "token": "reading.card"}));
    assert_eq!(
        explained["ok"]["package_value"]["whens"][0]["value"], "#000000",
        "{explained}"
    );
    // Undone: back to the package's value, the app's design as it was.
    let (undone, _) = s.ask(json!({"method": "undo", "client": "t", "inverse": inverse}));
    assert!(undone["ok"].is_object(), "{undone}");
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "reading.card"}));
    assert_eq!(
        resolved["ok"]["file"], "@acme/reading/reading.tamber",
        "{resolved}"
    );
    // Set again, then removed: the package's again.
    let (_, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "set-token", "name": "reading.card", "value": "#445566"}]}));
    let (removed, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files(), "ops": [{"op": "remove-token", "name": "reading.card"}]}));
    assert!(removed["ok"].is_object(), "{removed}");
    let (back, _) = s.ask(json!({"method": "explain", "token": "reading.card"}));
    assert_eq!(back["ok"]["file"], "@acme/reading/reading.tamber", "{back}");
    drop(s);
    // An app whose design is a package's file: its writes are refused, the package untouched.
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "node_modules/@acme/reading/reading.tamber"}"#,
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "reading.card", "value": "#999999"}]}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let package =
        std::fs::read_to_string(dir.join("node_modules/@acme/reading/reading.tamber")).unwrap();
    assert!(
        package.contains("#112233") && !package.contains("#999999"),
        "{package}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_package_file_stays_read_only_when_the_apps_sources_dont_load() {
    let dir = app("pkg-broken", "tamber t\n\ntheme\n  brand = #123456\n");
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = #112233\n");
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "node_modules/@acme/reading/reading.tamber"}"#,
    )
    .unwrap();
    // An unrelated import that doesn't resolve: the app's load fails, so its packages are unknown.
    std::fs::write(
        dir.join("app.contract"),
        "use Gone from \"./gone.contract\"\ncomponent App\n  view\n    text \"a\"\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "reading.card", "value": "#999999"}]}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let package =
        std::fs::read_to_string(dir.join("node_modules/@acme/reading/reading.tamber")).unwrap();
    assert!(!package.contains("#999999"), "{package}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_package_file_stays_read_only_without_its_manifest() {
    // node_modules/@acme/reading links to vendor/reading in the app; the app's design names that file.
    let dir = app("pkg-linked", "tamber t\n\ntheme\n  brand = #123456\n");
    let vendor = dir.join("vendor/reading");
    std::fs::create_dir_all(&vendor).unwrap();
    std::fs::write(
        vendor.join("reading.tamber"),
        "tamber reading\n\ntheme\n  reading.card = #112233\n",
    )
    .unwrap();
    std::fs::write(
        vendor.join("index.contract"),
        "component ReadingCard\n  view\n    text \"x\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("node_modules/@acme")).unwrap();
    std::os::unix::fs::symlink(&vendor, dir.join("node_modules/@acme/reading")).unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "vendor/reading/reading.tamber"}"#,
    )
    .unwrap();
    // No package.json in the package: the app's load can't say it is one.
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "reading.card", "value": "#999999"}]}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    assert!(!std::fs::read_to_string(vendor.join("reading.tamber"))
        .unwrap()
        .contains("#999999"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_typed_package_tokens_override_keeps_its_kind() {
    let dir = app("pkg-typed", "tamber t\n\ntheme\n  brand = #123456\n");
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = #112233\n  reading.shadow: box-shadow = 0px 1px 4px #000000\n");
    let mut s = Serve::start(&dir, &[]);
    let files = json!({"app.tamber": super::tamber_serve::digest(&dir.join("app.tamber"))});
    let (set, _) = s.ask(json!({"method": "transaction", "client": "t", "files": files, "ops": [{"op": "set-token", "name": "reading.shadow", "value": "0px 2px 8px #000000"}]}));
    assert!(set["ok"].is_object(), "{set}");
    let design = std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    assert!(
        design.contains("reading.shadow: box-shadow = 0px 2px 8px #000000"),
        "{design}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_undo_of_a_file_that_has_become_a_packages_is_refused() {
    let dir = app("pkg-undo", "tamber t\n\ntheme\n  brand = #123456\n");
    std::fs::create_dir_all(dir.join("theme")).unwrap();
    std::fs::write(
        dir.join("theme/colors.tamber"),
        "tamber t\n\ntheme\n  brand = #123456\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.json"), r#"{"tamber": "theme/colors.tamber"}"#).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (set, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "brand", "value": "#654321"}]}));
    let inverse = set["ok"]["inverse"].clone();
    assert!(inverse.is_string(), "{set}");
    // The design's directory becomes a package's (linked from node_modules), its bytes as they were.
    std::fs::create_dir_all(dir.join("node_modules/@acme")).unwrap();
    std::os::unix::fs::symlink(dir.join("theme"), dir.join("node_modules/@acme/theme")).unwrap();
    let (undone, _) = s.ask(json!({"method": "undo", "client": "t", "inverse": inverse}));
    assert!(undone.to_string().contains("read-only"), "{undone}");
    assert!(std::fs::read_to_string(dir.join("theme/colors.tamber"))
        .unwrap()
        .contains("#654321"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hoisted_package_link_keeps_its_file_read_only_without_its_manifest() {
    // ws/node_modules/@acme/reading links to ws/demo/vendor/reading; demo's design names that file.
    let ws = std::env::temp_dir().join(format!(
        "exact2-tamber-serve-pkg-hoisted-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&ws);
    let dir = ws.join("demo");
    std::fs::create_dir_all(dir.join("vendor/reading")).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    text \"a\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("vendor/reading/reading.tamber"),
        "tamber reading\n\ntheme\n  reading.card = #112233\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "vendor/reading/reading.tamber"}"#,
    )
    .unwrap();
    std::fs::create_dir_all(ws.join("node_modules/@acme")).unwrap();
    std::os::unix::fs::symlink(
        dir.join("vendor/reading"),
        ws.join("node_modules/@acme/reading"),
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "reading.card", "value": "#999999"}]}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn a_package_linked_from_a_nested_node_modules_keeps_its_file_read_only() {
    let dir = app("pkg-nested", "tamber t\n\ntheme\n  brand = #123456\n");
    std::fs::create_dir_all(dir.join("vendor/reading")).unwrap();
    std::fs::write(
        dir.join("vendor/reading/reading.tamber"),
        "tamber reading\n\ntheme\n  reading.card = #112233\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("src/node_modules/@acme")).unwrap();
    std::os::unix::fs::symlink(
        dir.join("vendor/reading"),
        dir.join("src/node_modules/@acme/reading"),
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "vendor/reading/reading.tamber"}"#,
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "reading.card", "value": "#999999"}]}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_package_linked_from_inside_another_packages_node_modules_keeps_its_file_read_only() {
    let dir = app("pkg-deep", "tamber t\n\ntheme\n  brand = #123456\n");
    std::fs::create_dir_all(dir.join("vendor/reading")).unwrap();
    std::fs::write(
        dir.join("vendor/reading/reading.tamber"),
        "tamber reading\n\ntheme\n  reading.card = #112233\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("node_modules/outer/node_modules/@acme")).unwrap();
    std::fs::write(
        dir.join("node_modules/outer/package.json"),
        r#"{"name":"outer"}"#,
    )
    .unwrap();
    std::os::unix::fs::symlink(
        dir.join("vendor/reading"),
        dir.join("node_modules/outer/node_modules/@acme/reading"),
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "vendor/reading/reading.tamber"}"#,
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "reading.card", "value": "#999999"}]}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_resolved_package_root_and_an_unreadable_manifest_keep_their_files_read_only() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    // A root the app's load resolved, its manifest naming nothing: node_modules/p links its files to vendor/p.
    let dir = app("pkg-resolved", "tamber t\n\ntheme\n  brand = #123456\n");
    std::fs::create_dir_all(dir.join("vendor/p")).unwrap();
    std::fs::write(dir.join("vendor/p/package.json"), "{}").unwrap();
    std::fs::write(
        dir.join("vendor/p/index.contract"),
        "component P\n  view\n    text \"x\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("vendor/p/design.tamber"),
        "tamber t\n\ntheme\n  brand = #123456\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("node_modules/p")).unwrap();
    symlink(
        "../../vendor/p/package.json",
        dir.join("node_modules/p/package.json"),
    )
    .unwrap();
    symlink(
        "../../vendor/p/index.contract",
        dir.join("node_modules/p/index.contract"),
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use P from \"p\"\ncomponent App\n  view\n    P()\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "vendor/p/design.tamber"}"#,
    )
    .unwrap();
    let write = |dir: &std::path::Path| {
        let mut s = Serve::start(dir, &[]);
        let (hello, _) = s.ask(json!({"method": "hello"}));
        s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "brand", "value": "#654321"}]})).0
    };
    let refused = write(&dir);
    assert!(refused.to_string().contains("read-only"), "{refused}");
    // The same with a source past it that can't be read: the roots it resolved still count.
    std::fs::write(
        dir.join("app.contract"),
        "use P from \"p\"\nuse Gone from \"./gone.contract\"\ncomponent App\n  view\n    P()\n",
    )
    .unwrap();
    let refused = write(&dir);
    assert!(refused.to_string().contains("read-only"), "{refused}");
    assert!(std::fs::read_to_string(dir.join("vendor/p/design.tamber"))
        .unwrap()
        .contains("#123456"));
    // A source that can't be read before the package is reached: a directory with a package.json is taken for one.
    std::fs::write(
        dir.join("app.contract"),
        "use Gone from \"./gone.contract\"\nuse P from \"p\"\ncomponent App\n  view\n    P()\n",
    )
    .unwrap();
    let refused = write(&dir);
    assert!(refused.to_string().contains("read-only"), "{refused}");
    assert!(std::fs::read_to_string(dir.join("vendor/p/design.tamber"))
        .unwrap()
        .contains("#123456"));
    // The app's Contract unreadable: its load failed, and the package-looking directory is refused.
    std::fs::write(
        dir.join("app.contract"),
        "use P from \"p\"\ncomponent App\n  view\n    P()\n",
    )
    .unwrap();
    std::fs::set_permissions(
        dir.join("app.contract"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let refused = write(&dir);
    std::fs::set_permissions(
        dir.join("app.contract"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
    // A named manifest that can't be read: refused (fail closed), not taken for none.
    let dir = app("pkg-unreadable", "tamber t\n\ntheme\n  brand = #123456\n");
    std::fs::create_dir_all(dir.join("vendor/q")).unwrap();
    std::fs::write(dir.join("vendor/q/package.json"), r#"{"name":"q"}"#).unwrap();
    std::fs::set_permissions(
        dir.join("vendor/q/package.json"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    std::fs::write(
        dir.join("vendor/q/design.tamber"),
        "tamber t\n\ntheme\n  brand = #123456\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "vendor/q/design.tamber"}"#,
    )
    .unwrap();
    let refused = write(&dir);
    std::fs::set_permissions(
        dir.join("vendor/q/package.json"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(refused["error"].is_object(), "{refused}");
    assert!(std::fs::read_to_string(dir.join("vendor/q/design.tamber"))
        .unwrap()
        .contains("#123456"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_write_judges_package_files_on_a_fresh_load_not_a_cached_one() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let dir = app("pkg-fresh", "tamber t\n\ntheme\n  brand = #123456\n");
    std::fs::create_dir_all(dir.join("vendor/p")).unwrap();
    std::fs::write(dir.join("vendor/p/package.json"), "{}").unwrap();
    std::fs::write(
        dir.join("vendor/p/index.contract"),
        "component P\n  view\n    text \"x\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("vendor/p/design.tamber"),
        "tamber t\n\ntheme\n  brand = #123456\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("node_modules/p")).unwrap();
    symlink(
        "../../vendor/p/package.json",
        dir.join("node_modules/p/package.json"),
    )
    .unwrap();
    symlink(
        "../../vendor/p/index.contract",
        dir.join("node_modules/p/index.contract"),
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "vendor/p/design.tamber"}"#,
    )
    .unwrap();
    // The server starts with no Contract (a clean, cached load of nothing) ...
    std::fs::remove_file(dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    // ... then one appears that can't be read: the write's own load fails, and the write is refused.
    std::fs::write(
        dir.join("app.contract"),
        "use P from \"p\"\ncomponent App\n  view\n    P()\n",
    )
    .unwrap();
    std::fs::set_permissions(
        dir.join("app.contract"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "brand", "value": "#654321"}]}));
    std::fs::set_permissions(
        dir.join("app.contract"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(refused.to_string().contains("read-only"), "{refused}");
    assert!(std::fs::read_to_string(dir.join("vendor/p/design.tamber"))
        .unwrap()
        .contains("#123456"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_written_when_the_servers_own_state_is_in_a_packages_directory() {
    let dir = app("pkg-state", "tamber t\n\ntheme\n  ink = #111111\n");
    std::fs::create_dir_all(dir.join(".exact")).unwrap();
    std::fs::write(dir.join(".exact/package.json"), r#"{"name":"p"}"#).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "ink", "value": "#222222"}]}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_written_when_the_servers_own_state_is_a_resolved_packages_root() {
    use std::os::unix::fs::symlink;
    let dir = app("pkg-state-root", "tamber t\n\ntheme\n  brand = #123456\n");
    std::fs::create_dir_all(dir.join(".exact")).unwrap();
    std::fs::write(dir.join(".exact/package.json"), "{}").unwrap();
    std::fs::write(
        dir.join(".exact/index.contract"),
        "component P\n  view\n    text \"x\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("node_modules/p")).unwrap();
    symlink(
        "../../.exact/package.json",
        dir.join("node_modules/p/package.json"),
    )
    .unwrap();
    symlink(
        "../../.exact/index.contract",
        dir.join("node_modules/p/index.contract"),
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use P from \"p\"\ncomponent App\n  view\n    P()\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-token", "name": "brand", "value": "#654321"}]}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_contract_only_write_is_refused_when_the_servers_state_is_a_packages() {
    let dir = app(
        "pkg-state-contract",
        "tamber t\n\ntheme\n  ink = #111111\n  accent.pressed = #222222\n",
    );
    std::fs::write(
        dir.join("app.contract"),
        "style Post\n  color=\"ink\"\n\ncomponent App\n  view\n    text \"a\" class=Post\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join(".exact")).unwrap();
    std::fs::write(dir.join(".exact/package.json"), r#"{"name":"p"}"#).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "t", "files": hello["ok"]["files"], "ops": [{"op": "set-state", "style": "Post", "state": "active", "row": "background-color", "value": "accent.pressed"}]}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// tamber/1.15: a site's file is the app's name for it,
/// `node_modules/@acme/reading/index.contract`, as `explain` gives it, never
/// the machine's absolute path, which a remote client must not see: no
/// answer holds the app's directory, as given or canonical.
#[test]
fn no_answer_names_the_machines_directories() {
    let dir = app("pkg-paths", "tamber t\n\ntheme\n  brand = reading.card\n");
    with_reading(&dir, "tamber reading\n\ntheme\n  reading.card = #112233\n");
    let at = dir.join("node_modules/@acme/reading");
    std::fs::write(at.join("index.contract"), "component ReadingCard\n  params\n    inset: length = \"4px\"\n  view\n    column background-color=\"reading.card\" padding=\"param.inset\"\n      text \"x\"\n").unwrap();
    let file = dir.join("app.tamber");
    let mut s = Serve::start(&dir, &[]);
    let mut answers = Vec::new();
    for request in [
        json!({"method": "hello", "client": "one"}),
        json!({"method": "describe"}),
        json!({"method": "uses", "token": "ReadingCard.inset"}),
        json!({"method": "uses", "token": "reading.card"}),
        json!({"method": "uses", "token": "brand"}),
        json!({"method": "explain", "token": "ReadingCard.inset"}),
        json!({"method": "explain", "token": "reading.card"}),
        json!({"method": "resolve", "token": "ReadingCard.inset"}),
        json!({"method": "propose", "ops": [{"op": "set-param", "name": "ReadingCard.inset", "value": "8px"}]}),
        json!({"method": "transaction", "client": "one", "files": {"app.tamber": super::tamber_serve::digest(&file)}, "ops": [{"op": "set-token", "name": "brand", "value": "#445566"}]}),
    ] {
        let (answer, events) = s.ask(request.clone());
        assert!(answer.get("ok").is_some(), "{request}: {answer}");
        answers.push((request, answer, events));
    }
    // `uses` on the package component's param: its sites in the package's
    // file, named from the app's directory.
    let sites = answers[2].1["ok"]["sites"].as_array().unwrap();
    assert!(!sites.is_empty(), "{}", answers[2].1);
    for site in sites {
        assert_eq!(
            site["file"], "node_modules/@acme/reading/index.contract",
            "{site}"
        );
    }
    let brand = answers[4].1["ok"]["sites"].as_array().unwrap();
    assert!(
        brand.iter().all(|x| x["file"] == "app.contract"),
        "{}",
        answers[4].1
    );
    let canonical = dir.canonicalize().unwrap();
    for (request, answer, events) in &answers {
        let text = format!("{answer} {events:?}");
        for d in [&dir, &canonical] {
            assert!(!text.contains(&*d.to_string_lossy()), "{request}: {text}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1 of the path fix: only path metadata is named from the app's
/// directory; what an author wrote, a path to the app's own directory in it
/// too, comes back byte for byte (a proposal's diff, describe's metadata,
/// a scrub's fold), to a local client as to any.
#[test]
fn authored_text_holding_the_apps_path_is_kept() {
    let dir = app("authored-path", "tamber t\n\ntheme\n  ink = #111111\n");
    let canonical = dir.canonicalize().unwrap();
    let written = format!("{}/assets/icon.svg", canonical.display());
    let file = dir.join("app.tamber");
    std::fs::write(
        &file,
        format!("tamber t\n\ntheme\n  ink = #111111  help=\"{written}\"\n"),
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let op = json!([{"op": "set-token", "name": "ink", "value": "#222222"}]);
    let (proposed, _) = s.ask(json!({"method": "propose", "ops": op}));
    let diff = proposed["ok"]["diff"]
        .as_str()
        .unwrap_or_else(|| panic!("{proposed}"));
    assert!(
        diff.contains(&format!("+  ink = #222222  help=\"{written}\"")),
        "{diff}"
    );
    let (described, _) = s.ask(json!({"method": "describe"}));
    assert_eq!(
        described["ok"]["meta"]["tokens"]["ink"],
        json!([["help", written]]),
        "{}",
        described["ok"]["meta"]
    );
    let digest = super::tamber_serve::digest(&file);
    let (folded, _) = s.ask(
        json!({"method": "scrub-fold", "open": true, "files": {"app.tamber": digest}, "ops": op}),
    );
    assert_eq!(
        folded["ok"]["values"][0]["css"],
        json!(["#222222ff"]),
        "{folded}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 2 of the path fix (Astra 2 and 3): a package linked from outside
/// the app. A motion token its view writes keeps its sites (`motion_sites`
/// keeps the loader's paths through the join; the reply names them), and
/// `explain` reads a param's default as written from where the file is.
#[test]
fn a_package_linked_from_outside_keeps_its_motion_sites_and_its_defaults() {
    let dir = app(
        "pkg-linked-outside",
        "tamber t\n\ntheme\n  duration.fade: duration = 200ms\n",
    );
    let outside =
        std::env::temp_dir().join(format!("exact2-linked-outside-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("package.json"),
        r#"{"name":"@acme/linked","version":"1.0.0","exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    std::fs::write(outside.join("index.contract"), "component LinkedCard\n  params\n    inset: length = \"4px\"\n  view\n    column padding=\"param.inset\" transition=\"opacity duration.fade\"\n      text \"x\"\n").unwrap();
    std::fs::create_dir_all(dir.join("node_modules/@acme")).unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("node_modules/@acme/linked")).unwrap();
    std::fs::write(dir.join("app.contract"), "use LinkedCard from \"@acme/linked\"\ncomponent App\n  view\n    column\n      LinkedCard()\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (uses, _) = s.ask(json!({"method": "uses", "token": "duration.fade"}));
    let sites = uses["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{uses}"));
    assert!(!sites.is_empty(), "{uses}");
    for site in sites {
        assert!(!site["site"].is_null(), "an instance found: {site}");
        let file = site["file"].as_str().unwrap();
        assert!(
            file == "@acme/linked/index.contract"
                || file == "node_modules/@acme/linked/index.contract",
            "{site}"
        );
    }
    let (explained, _) = s.ask(json!({"method": "explain", "token": "LinkedCard.inset"}));
    assert_eq!(
        explained["ok"]["param"]["default"], "\"4px\"",
        "{explained}"
    );
    let text = format!("{uses} {explained}");
    for d in [outside.clone(), outside.canonicalize().unwrap()] {
        assert!(!text.contains(&*d.to_string_lossy()), "{text}");
    }
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&outside);
}
