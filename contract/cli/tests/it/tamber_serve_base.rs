//! The base scale on the wire (LLP 1121 stage 11 D, `tamber/1.14`):
//! `describe`'s `base` and `targets`, `resolve` and `explain` of a base name
//! (unset, and set by the app), and `explain`'s `bases` for a token that
//! reaches one. Every cell is the kernel's table's.
#![cfg(unix)]

use crate::tamber_serve::{app, Serve};
use exact_kernel::style::metrics::Column;
use exact_kernel::style::scale;
use serde_json::{json, Value};

const DESIGN: &str = "tamber t

axes
  density = roomy | tight

theme
  card.pad = space.md
    when density=tight: space.sm
  radius.lg = 20px
    when density=tight: 14px
";

/// The column `key` of the kernel's row for `name`, as the wire writes it.
fn cell(name: &str, key: &str) -> String {
    let column = scale::COLUMNS.iter().find(|(k, _)| *k == key).unwrap().1;
    match scale::of(name, column).unwrap() {
        scale::ScaleValue::Px(_) => scale::of(name, column).map(scale::css).unwrap(),
        scale::ScaleValue::Cells(0.0) => "0".into(),
        scale::ScaleValue::Cells(n) => format!("{n}cell"),
        scale::ScaleValue::Full => "full".into(),
    }
}

#[test]
fn base_names_are_rows_read_from_the_kernel_with_the_apps_override() {
    let dir = app("base", DESIGN);
    std::fs::write(
        dir.join("app.json"),
        r#"{"app": {"id": "com.x.base", "name": "Base"}, "host": {"ios": {}, "macos": {}}}"#,
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    // describe: every base name in the table's order, each column the kernel's.
    let (describe, _) = s.ask(json!({"method": "describe"}));
    let base = describe["ok"]["base"].as_array().unwrap();
    assert_eq!(
        base.iter()
            .map(|r| r["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        scale::names().collect::<Vec<_>>()
    );
    let md = base.iter().find(|r| r["name"] == "space.md").unwrap();
    assert_eq!(md["kind"], "length");
    for (key, _) in scale::COLUMNS {
        assert_eq!(
            md["columns"][key]["value"],
            cell("space.md", key),
            "{key}: {md}"
        );
        assert!(md["columns"][key]["source"]["kind"].is_string(), "{md}");
    }
    assert_eq!(md["override"], Value::Null);
    // radius.lg is the app's: its line, its text.
    let lg = base.iter().find(|r| r["name"] == "radius.lg").unwrap();
    assert_eq!(lg["kind"], "radius");
    assert_eq!(
        lg["override"],
        json!({"file": "app.tamber", "line": 9, "col": 3, "text": "20px"})
    );
    let full = base.iter().find(|r| r["name"] == "radius.full").unwrap();
    assert_eq!(full["columns"]["ios"]["value"], "full");
    // targets: what the app ships, each with the column it reads.
    assert_eq!(
        describe["ok"]["targets"],
        json!([{"target": "ios", "column": "ios"}, {"target": "macos", "column": "mac"}])
    );
    // resolve: an unset base name has no per-tuple fold, only its row.
    let (unset, _) = s.ask(json!({"method": "resolve", "token": "space.md"}));
    assert_eq!(unset["ok"]["name"], "space.md");
    assert_eq!(unset["ok"]["base"]["name"], "space.md");
    assert!(unset["ok"]["tuples"].is_null(), "{unset}");
    // A set one: the app token's tuples, and its row.
    let (set, _) = s.ask(json!({"method": "resolve", "token": "radius.lg"}));
    assert_eq!(set["ok"]["tuples"][0]["value"], "20px", "{set}");
    assert_eq!(set["ok"]["tuples"][1]["value"], "14px", "{set}");
    assert_eq!(set["ok"]["base"]["override"]["text"], "20px");
    // explain: an unset base name is a base row; a set one carries its line's fields too.
    let (why, _) = s.ask(json!({"method": "explain", "token": "space.sm"}));
    assert_eq!(why["ok"]["kind"], "base");
    assert_eq!(why["ok"]["base"]["name"], "space.sm");
    let (why, _) = s.ask(json!({"method": "explain", "token": "radius.lg"}));
    assert_eq!(why["ok"]["kind"], "base");
    assert_eq!(why["ok"]["value"], "20px", "{why}");
    assert_eq!(why["ok"]["whens"][0]["value"], "14px");
    // A token that reaches base names says which.
    let (pad, _) = s.ask(json!({"method": "explain", "token": "card.pad"}));
    assert_eq!(pad["ok"]["bases"], json!(["space.md", "space.sm"]), "{pad}");
    // The neutral column is the web's (the kernel's own rule).
    assert_eq!(
        scale::of("space.md", Column::Web),
        scale::of("space.md", Column::Neutral)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// theme2's 18:26 condition: `describe`'s target-to-column map agrees with
/// the column each host's build says it reads, so the two can't drift. Read
/// from the hosts' own sources: the Apple host sets the Mac's column on macOS
/// and iOS's elsewhere; the web hosts and the Linux desktop show the neutral
/// look; Android's reader shows Material.
#[test]
fn each_targets_column_is_the_one_its_host_reads() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let has = |file: &str, needle: &str| {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        assert!(text.contains(needle), "{file} no longer says `{needle}`");
    };
    has("host/apple/src/scale.rs", "Column::Mac");
    has("host/apple/src/scale.rs", "Column::Ios");
    assert_eq!(scale::column_for("macos"), Some("mac"));
    assert_eq!(scale::column_for("ios"), Some("ios"));
    has("host/web/src/host.rs", "roles::Column::Neutral");
    has("host/web-js/src/main.rs", "roles::Column::Neutral");
    assert_eq!(scale::column_for("web"), Some("neutral"));
    has("host/linux/src/app.rs", "show_neutral_look();");
    assert_eq!(scale::column_for("linux"), Some("neutral"));
    has("host/linux/src/canvas/jni.rs", "show_material_look();");
    assert_eq!(scale::column_for("android"), Some("material"));
    // And a column the map names is one the table has.
    for target in ["ios", "macos", "web", "linux", "android", "terminal"] {
        let column = scale::column_for(target).unwrap();
        assert!(scale::COLUMNS.iter().any(|(k, _)| *k == column), "{target}");
    }
}
