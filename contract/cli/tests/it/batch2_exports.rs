//! LLP 1121 stage 15 B batch 2: `@exact/cards` exports EmptyState, and
//! `@exact/lists` MenuItem and MenuGroup. A consumer app outside the
//! repository, both packages installed under its `node_modules` as the
//! in-repo source, `use`s the three from their packages as the clone does:
//! an empty state with every piece and one with the message alone, and a
//! group holding two items in its slot. It compiles, and its plan holds the
//! three components' own nodes, each named in the source map by its
//! declaring identity.
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

const CONSUMER: &str = r#"use EmptyState from "@exact/cards"
use MenuItem, MenuGroup from "@exact/lists"

component App
  state n = 0
  action press
    n = n + 1
  view
    column
      EmptyState(id="chat-empty", icon="symbol:info", iconSize="large", tone="label", title="", message="Say hi to someone", button="New chat", buttonLabel="", buttonIcon="", buttonTone="primary", press=press)
      EmptyState(id="list-empty", icon="", iconSize="medium", tone="medium", title="No lists", message="Lists you make show here.", button="", buttonLabel="", buttonIcon="", buttonTone="secondary", press=press)
      MenuGroup()
        MenuItem(id="pin", label="Pin to your profile", icon="symbol:pin", destructive=false, first=true, pick=press)
        MenuItem(id="delete", label="Delete post", icon="symbol:trash", destructive=true, first=false, pick=press)
"#;

#[test]
fn a_consumer_uses_empty_state_and_the_menu_from_the_packages_and_its_plan_holds_them() {
    let dir = std::env::temp_dir().join(format!("exact2-batch2-exports-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for pkg in ["cards", "lists"] {
        copy(
            &repo().join("packages").join(pkg),
            &dir.join("node_modules/@exact").join(pkg),
        );
    }
    std::fs::write(dir.join("app.contract"), CONSUMER).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber consumer\n\ntheme\n  ink = #111111\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"app": {"id": "dev.test.batch2exports", "name": "Batch 2"}, "host": {"ios": {}, "web": {}}}"#,
    )
    .unwrap();
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
        "@exact/cards/empty-state.contract#EmptyState",
        "@exact/lists/menu.contract#MenuItem",
        "@exact/lists/menu.contract#MenuGroup",
    ] {
        assert!(libs.contains(&identity), "no {identity} in {libs:?}");
    }
    // The app's ids name the empty state's button and each item's column.
    for s in ["chat-empty-button", "pin-item", "delete-item"] {
        assert!(plan.strings.iter().any(|x| x == s), "no {s} in the plan");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
