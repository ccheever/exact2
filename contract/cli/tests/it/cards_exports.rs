//! LLP 1121 stage 15 B batch 1: `@exact/cards` exports Avatar and
//! VerifiedBadge by its entry's `use` lines, params unchanged (`size`, and
//! VerifiedBadge's `ink` and `mark`, explicit, with no defaults). A consumer
//! app outside the repository, the package installed under its
//! `node_modules` as the in-repo source, `use`s both from "@exact/cards"
//! with token names for size, ink and mark, as the clone does; it compiles,
//! and its plan holds both components' own nodes, each named in the source
//! map by its declaring identity. The cards' style `Name`, in the same file
//! and not exported, is still refused, so the export is the `use` line's.
//! Batch 2: VerifiedBadge's `badge` is `enum<"none", "verified", "verifier">`
//! (it was a string, "" for none), so batch 1's `""` is refused; PersonRow
//! is exported too, its trailing control in its slot.
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
      VerifiedBadge(badge="none", size="cards.badge-size", ink="cards.badge", mark="cards.badge-mark")
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

#[test]
fn verified_badges_badge_is_an_enum_so_the_old_empty_string_is_refused() {
    let dir = consumer(
        "empty",
        "use VerifiedBadge from \"@exact/cards\"\n\ncomponent App\n  view\n    column\n      VerifiedBadge(badge=\"\", size=\"cards.badge-size\", ink=\"cards.badge\", mark=\"cards.badge-mark\")\n",
    );
    let err =
        contract::compile_path(&dir.join("app.contract")).expect_err("\"\" is no badge value");
    assert!(err.message.contains("badge"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

const PERSON: &str = r#"use PersonRow from "@exact/cards"

component App
  state opened = 0
  action open
    opened = opened + 1
  view
    column
      PersonRow(id="ada", label="View Ada's profile", avatar="", name="Ada", handle="@ada.example", badge="verified", pill="Follows you", description="Maps.", descriptionLines=3, divider="bottom", disabled=false, press=open)
        box testId="trailing" width=24 height=24
      PersonRow(id="fen", label="View Fen's profile", avatar="symbol:person", name="Fen", handle="@fen.example", badge="none", pill="", description="", descriptionLines=1, divider="top", disabled=true, press=open)
"#;

/// A consumer places PersonRow from "@exact/cards": its plan holds the row,
/// named by its identity, with the app's words (no English of the
/// package's: its link's label, its handle line, its pill) and the trailing
/// control the app put in its slot; a disabled row says so.
#[test]
fn a_consumer_uses_person_row_from_the_package_with_its_trailing_slot() {
    use exact_kernel::{Kernel, PropId};
    let dir = consumer("person", PERSON);
    let app = dir.join("app.contract");
    let (plan, map) = contract::compile_path_mapped(&app).unwrap_or_else(|e| panic!("{e}"));
    let json: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let libs: Vec<&str> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["library"].as_str())
        .collect();
    let identity = "@exact/cards/person-row.contract#PersonRow";
    assert!(libs.contains(&identity), "no {identity} in {libs:?}");
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let node = |t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
    let said = |t: &str, id: PropId| node(t).props.str(id).map(str::to_string);
    assert_eq!(
        said("ada-link", PropId::AccessibilityLabel).as_deref(),
        Some("View Ada's profile")
    );
    let disabled = |t: &str| node(t).props.bool(PropId::AccessibilityDisabled);
    assert_eq!(
        (disabled("ada-link"), disabled("fen-link")),
        (Some(false), Some(true))
    );
    // The link holds the avatar and names, not the slot's control: the row
    // has no role (its children stay their own to a screen reader), and the
    // trailing control is the row's, outside the link.
    let role = |t: &str| {
        node(t)
            .props
            .str(PropId::AccessibilityRole)
            .map(str::to_string)
    };
    assert_eq!(
        (role("ada"), role("ada-link").as_deref()),
        (None, Some("link"))
    );
    fn holds_text(k: &Kernel, at: u32, want: &str) -> bool {
        let n = k.node(at).unwrap();
        n.props.str(PropId::Text) == Some(want)
            || n.children().into_iter().any(|c| holds_text(k, c, want))
    }
    fn holds(k: &Kernel, at: u32, target: u32) -> bool {
        k.node(at)
            .unwrap()
            .children()
            .into_iter()
            .any(|c| c == target || holds(k, c, target))
    }
    let under = |outer: &str, inner: &str| holds(k, node(outer).id, node(inner).id);
    assert!(under("ada", "trailing") && !under("ada-link", "trailing"));
    assert!(under("ada-link", "verified-badge"));
    // The link takes the pill and the description too (theme2's 2b
    // condition): only the slot is outside it.
    for text in ["Follows you", "Maps."] {
        let k = r.kernel();
        let found = k
            .find_by_test_id("ada-link")
            .first()
            .map(|key| k.node_by_key(*key).unwrap().id)
            .is_some_and(|link| holds_text(k, link, text));
        assert!(found, "{text} is inside the link");
    }
    // One press target, the link: the row takes no press (so no Tab stop or
    // control of its own on any host), and only the trailing control is
    // outside the link; the slot's control is not dimmed.
    let opacity = |t: &str| format!("{:?}", node(t).computed(exact_kernel::StyleId::Opacity));
    assert_eq!(
        opacity("fen"),
        opacity("ada"),
        "the row itself is not dimmed"
    );
    assert_ne!(
        opacity("fen-link"),
        opacity("ada-link"),
        "the disabled link is"
    );
    let id = |t: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id
    };
    let (link, row) = (id("ada-link"), id("ada"));
    assert!(r.handlers_of(row).is_empty(), "the row takes no press");
    r.dispatch(link, exact_runner::Event::Press).unwrap();
    assert_eq!(r.slot("opened"), Some(&exact_runner::Value::Number(1.0)));
    let _ = std::fs::remove_dir_all(&dir);
}

struct NoData;

impl exact_runner::DataSource for NoData {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
}
