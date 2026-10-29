//! LLP 1121 stage 15 B batch 2 (2a): `@exact/actions` exports FollowButton
//! by its entry's `use` line. A consumer app outside the repository, the
//! package installed under its `node_modules` as the in-repo source, `use`s
//! it from "@exact/actions" in every tone, size, shape and icon pattern; it
//! compiles, its plan holds the button's nodes named in the source map by
//! the declaring identity, and both icon tokens with their SF names. The
//! button says its state by its label alone (theme2's 14:15 ruling: the
//! label swap, no `aria-pressed`): the app's words, swapped by the app, in
//! the pill and in the round, icon-only form.
#![cfg(unix)]

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Event, Runner, Value};
use std::path::{Path, PathBuf};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

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

const CONSUMER: &str = r#"use FollowButton from "@exact/actions"

component App
  action press
  view
    column
      FollowButton(id="a", following=false, text="Follow", label="", tone="primary", size="small", shape="pill", icon="both", press=press)
      FollowButton(id="b", following=true, text="Following", label="", tone="inverted", size="small", shape="pill", icon="follow", press=press)
      FollowButton(id="c", following=false, text="Follow back", label="", tone="subtle", size="tiny", shape="pill", icon="following", press=press)
      FollowButton(id="d", following=true, text="Following", label="Following Ada", tone="overlay", size="tiny", shape="round", icon="none", press=press)
"#;

/// A consumer app in a fresh directory, `@exact/actions` the repository's.
fn consumer(tag: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "exact2-actions-exports-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    copy(
        &repo().join("packages/actions"),
        &dir.join("node_modules/@exact/actions"),
    );
    std::fs::write(dir.join("app.contract"), source).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber consumer\n\ntheme\n  ink = #111111\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"app": {"id": "dev.test.actionsexports", "name": "Actions"}, "host": {"ios": {}, "web": {}}}"#,
    )
    .unwrap();
    dir
}

#[test]
fn a_consumer_uses_follow_button_from_the_package_and_its_plan_holds_it() {
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
    let identity = "@exact/actions/follow-button.contract#FollowButton";
    assert!(libs.contains(&identity), "no {identity} in {libs:?}");
    // Both icons are the package's tokens (Bluesky's PlusLarge and Check
    // paths), with an SF name, so Apple draws its own plus and checkmark.
    for (icon, sf) in [
        ("actions.follow-add", "plus"),
        ("actions.follow-check", "checkmark"),
    ] {
        let row = plan
            .icons
            .iter()
            .find(|i| plan.str(i.name) == icon)
            .unwrap_or_else(|| panic!("no {icon} in the plan's icons"));
        assert_eq!(plan.str(row.sf), sf, "{icon}'s SF Symbol");
        assert!(plan.str(row.svg).ends_with(".svg"), "{icon}'s SVG");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The app swaps the words; the button says its state by them alone. A
/// pill's spoken label is its text when `label` is "", and a round
/// button's is its `label` ("Follow Ada", then "Following Ada"), each
/// swapped by one press; neither carries `aria-pressed` in either state.
#[test]
fn following_is_said_by_the_label_alone_in_both_states_round_too() {
    let source = r#"use FollowButton from "@exact/actions"

component App
  state following = false
  action flip
    following = not following
  view
    column
      FollowButton(id="pill", following=following, text=(following ? "Following" : "Follow"), label="", tone="primary", size="small", shape="pill", icon="both", press=flip)
      FollowButton(id="round", following=following, text=(following ? "Following" : "Follow"), label=(following ? "Following Ada" : "Follow Ada"), tone="inverted", size="small", shape="round", icon="both", press=flip)
"#;
    let dir = consumer("label", source);
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let view = |r: &Runner<NoData>, t: &str| {
        let key = r.kernel().find_by_test_id(t)[0];
        r.kernel().node_by_key(key).unwrap().id
    };
    let said = |r: &Runner<NoData>| {
        ["pill", "round"].map(|t| {
            let node = r.kernel().node(view(r, t)).unwrap();
            (
                node.props
                    .str(PropId::AccessibilityLabel)
                    .map(str::to_string),
                node.props
                    .str(PropId::AccessibilityPressed)
                    .map(str::to_string),
            )
        })
    };
    let some = |s: &str| Some(s.to_string());
    assert_eq!(
        said(&r),
        [(some("Follow"), None), (some("Follow Ada"), None)]
    );
    let pill = view(&r, "pill");
    r.dispatch(pill, Event::Press).unwrap();
    assert_eq!(
        said(&r),
        [(some("Following"), None), (some("Following Ada"), None)]
    );
    let round = view(&r, "round");
    r.dispatch(round, Event::Press).unwrap();
    assert_eq!(
        said(&r),
        [(some("Follow"), None), (some("Follow Ada"), None)]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The primary tone is Bluesky's literal and an app reaches it by full name
/// (theme2's 2026-10-27 23:51 ruling: no platform-accent default, which
/// would fall under 4.5:1 with white text; an importer maps an accent here).
/// With nothing set, the primary's fill is `light-dark(#006aff, #0f73ff)`
/// and its ink, on the label and the icon (the button's children, not its
/// own text colour), `#ffffff`. An app setting `actions.follow-fill =
/// #e11d48` and `actions.follow-ink = #111111` recolours both, in both
/// schemes, while the overlay keeps its fixed `#dce2ea` fill and `#000000`
/// ink.
#[test]
fn an_app_recolours_the_primary_fill_and_ink_by_full_name_and_not_the_overlay() {
    use exact_kernel::{Color, ColorValue, RowValue, StyleId};
    // The colour of `row` on the node `path` below the button `id` (each
    // step a child index): `[]` the button, `[0, 0]` its icon's image, `[1]`
    // its label (icon "both": the icon box first, then the label).
    let colour = |r: &Runner<NoData>, id: &str, path: &[usize], row: StyleId| -> ColorValue {
        let key = r.kernel().find_by_test_id(id)[0];
        let mut node = r.kernel().node_by_key(key).unwrap();
        for &k in path {
            node = node
                .child_nodes()
                .nth(k)
                .unwrap_or_else(|| panic!("{id}: no child {k} on {path:?}"));
        }
        match node.computed(row) {
            RowValue::ColorValue(c) => c,
            RowValue::Color(c) => ColorValue::Fixed(c),
            other => panic!("{id} {path:?}: {other:?}"),
        }
    };
    let hex = |s: &str| Color::parse(s).unwrap();
    let boot = |dir: &Path| {
        let plan =
            contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"));
        Runner::boot(
            plan,
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    };
    // [light, dark] of the fill, the icon's ink and the label's ink.
    let read = |r: &Runner<NoData>, id: &str| {
        [false, true].map(|dark| {
            [
                colour(r, id, &[], StyleId::BackgroundColor).resolve(dark),
                colour(r, id, &[0, 0], StyleId::TextColor).resolve(dark),
                colour(r, id, &[1], StyleId::TextColor).resolve(dark),
            ]
        })
    };
    let overlay = [[hex("#dce2ea"), hex("#000000"), hex("#000000")]; 2];

    let white = hex("#ffffff");

    // Nothing set, Bluesky's literals; then the app's brand by full name.
    // An overlay pill (with a label, which the round "d" lacks) beside it.
    let source = CONSUMER.replace(
        "      FollowButton(id=\"d\"",
        "      FollowButton(id=\"o\", following=false, text=\"Follow\", label=\"\", tone=\"overlay\", size=\"small\", shape=\"pill\", icon=\"both\", press=press)\n      FollowButton(id=\"d\"",
    );
    for (tag, design, primary) in [
        (
            "fill-none",
            "",
            [
                [hex("#006aff"), white, white],
                [hex("#0f73ff"), white, white],
            ],
        ),
        (
            "fill-set",
            "  actions.follow-fill = #e11d48\n  actions.follow-ink = #111111\n",
            [[hex("#e11d48"), hex("#111111"), hex("#111111")]; 2],
        ),
    ] {
        let dir = consumer(tag, &source);
        std::fs::write(
            dir.join("app.tamber"),
            format!("tamber consumer\n\ntheme\n  ink = #111111\n{design}"),
        )
        .unwrap();
        let r = boot(&dir);
        if design.is_empty() {
            // The default ink is the fixed literal, not a role: Apple's
            // `AccentColorText` also resolves to white, so a resolved read
            // alone would pass a regression to it.
            for path in [&[0, 0][..], &[1][..]] {
                assert_eq!(
                    colour(&r, "a", path, StyleId::TextColor),
                    ColorValue::Fixed(white),
                    "{tag}: the primary's ink at {path:?}, a fixed #ffffff"
                );
            }
        }
        assert_eq!(read(&r, "a"), primary, "{tag}: the primary");
        assert_eq!(read(&r, "o"), overlay, "{tag}: the overlay");
        drop(r);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
