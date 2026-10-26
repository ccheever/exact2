//! LLP 1121 stage 15 B batch 1: `@exact/cards` exports Avatar and
//! VerifiedBadge by its entry's `use` lines, params unchanged (`size`, and
//! VerifiedBadge's `ink` and `mark`, explicit, with no defaults). A consumer
//! app outside the repository, the package installed under its
//! `node_modules` as the in-repo source, `use`s both from "@exact/cards"
//! with token names for size, ink and mark, as the clone does; it compiles,
//! and its plan holds both components' own nodes, each named in the source
//! map by its declaring identity. The cards' style `Name`, in the same file
//! and not exported, is still refused, so the export is the `use` line's.
#![cfg(unix)]

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let dest = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy(&path, &dest);
        } else {
            std::fs::copy(&path, &dest).unwrap();
        }
    }
}

const CONSUMER: &str = r#"use Avatar, VerifiedBadge from "@exact/cards"

component App
  view
    column
      Avatar(src="", size="cards.avatar")
      Avatar(src="symbol:person", size="cards.avatar")
      VerifiedBadge(badge="verified", size="cards.badge-size", ink="cards.badge", mark="cards.badge-mark")
      VerifiedBadge(badge="verifier", size="cards.badge-size", ink="cards.badge", mark="cards.badge-mark")
      VerifiedBadge(badge="", size="cards.badge-size", ink="cards.badge", mark="cards.badge-mark")
"#;

/// A consumer app in a fresh directory, `@exact/cards` the repository's.
fn consumer(tag: &str, source: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("exact2-cards-exports-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy(
        &repo().join("packages/cards"),
        &dir.join("node_modules/@exact/cards"),
    );
    std::fs::write(dir.join("app.contract"), source).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber consumer\n\ntheme\n  ink = #111111\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"app": {"id": "dev.test.cardsexports", "name": "Cards"}, "host": {"ios": {}, "web": {}}}"#,
    )
    .unwrap();
    dir
}

#[test]
fn a_consumer_uses_avatar_and_verified_badge_from_the_package_and_its_plan_holds_both() {
    let dir = consumer("uses", CONSUMER);
    let app = dir.join("app.contract");
    let (plan, map) = contract::compile_path_mapped(&app).unwrap_or_else(|e| panic!("{e}"));
    let json: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let libs: Vec<&str> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["library"].as_str())
        .collect();
    for identity in [
        "@exact/cards/common.contract#Avatar",
        "@exact/cards/common.contract#VerifiedBadge",
    ] {
        assert!(libs.contains(&identity), "no {identity} in {libs:?}");
    }
    // Each badge's own labelled column, and both stacked symbols (the
    // ground and the check, stage 14 C's icon tokens), are in the plan.
    for s in ["verified-badge", "verifier-badge"] {
        assert!(plan.strings.iter().any(|x| x == s), "no {s} in the plan");
    }
    for icon in [
        "cards.verified",
        "cards.verified-check",
        "cards.verifier",
        "cards.verifier-check",
    ] {
        assert!(
            plan.icons.iter().any(|i| plan.str(i.name) == icon),
            "no {icon} in the plan's icons"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cards_unexported_style_is_still_refused() {
    let dir = consumer(
        "name",
        "use Name from \"@exact/cards\"\n\ncomponent App\n  view\n    text \"a\" class=Name\n",
    );
    let err = contract::compile_path(&dir.join("app.contract")).expect_err("Name is not exported");
    assert!(err.message.contains("Name"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}
