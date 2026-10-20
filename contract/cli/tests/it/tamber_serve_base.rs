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
  space.xl = space.lg
  gap.wide = space.md
    when density=roomy: 12px
    when density=tight: 8px
  card.inner = card.pad
  card.outer = card.round
  card.round = radius.lg
  label.gap = 4px
    when density=roomy: card.pad
  body.text: type = space.lg
  body.face: family = \"space.md\", serif
";

/// A cell's source as the shape writes it (theme2's base-row shape).
fn source(s: scale::ScaleSource) -> Value {
    match s {
        scale::ScaleSource::Metric(m) => json!({"kind": "metric", "ref": m}),
        scale::ScaleSource::Measured(id) => json!({"kind": "measured", "id": id}),
        scale::ScaleSource::Guideline(id) => json!({"kind": "guideline", "cite": id}),
        scale::ScaleSource::Full => json!({"kind": "full"}),
        scale::ScaleSource::Terminal => json!({"kind": "terminal"}),
    }
}

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
        r#"{"app": {"id": "com.x.base", "name": "Base"}, "host": {"ios": {}, "macos": {}}, "deploy": {"store": {"steam": "0"}}}"#,
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
    // Every cell of every row: the kernel's value and its source, exactly.
    for r in base {
        let name = r["name"].as_str().unwrap();
        for (key, column) in scale::COLUMNS {
            let row = scale::row(name).unwrap();
            assert_eq!(r["columns"][key]["value"], cell(name, key), "{name} {key}");
            assert_eq!(
                r["columns"][key]["source"],
                source(scale::cell(row, column).source),
                "{name} {key}"
            );
        }
    }
    let md = base.iter().find(|r| r["name"] == "space.md").unwrap();
    assert_eq!(md["kind"], "length");
    assert_eq!(md["override"], Value::Null);
    // radius.lg is the app's: its line, its text.
    let lg = base.iter().find(|r| r["name"] == "radius.lg").unwrap();
    assert_eq!(lg["kind"], "radius");
    assert_eq!(
        lg["override"],
        json!({"file": "app.tamber", "line": 9, "col": 3, "text": "20px"})
    );
    // The terminal's spelling, pinned on its own.
    assert_eq!(md["columns"]["terminal"]["value"], "1cell");
    let xs = base.iter().find(|r| r["name"] == "space.xs").unwrap();
    assert_eq!(xs["columns"]["terminal"]["value"], "0");
    let full = base.iter().find(|r| r["name"] == "radius.full").unwrap();
    assert_eq!(full["columns"]["ios"]["value"], "full");
    // targets: what the app ships, each with the column it reads.
    // (a target the kernel names no column for stays, its column null: the set is draws')
    assert_eq!(
        describe["ok"]["targets"],
        json!([{"target": "ios", "column": "ios"}, {"target": "macos", "column": "mac"}, {"target": "steam", "column": null}])
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
    // A token that reaches base names says which; a quoted family naming one does not.
    let (pad, _) = s.ask(json!({"method": "explain", "token": "card.pad"}));
    assert_eq!(pad["ok"]["bases"], json!(["space.md", "space.sm"]), "{pad}");
    let (face, _) = s.ask(json!({"method": "explain", "token": "body.face"}));
    assert_eq!(face["ok"]["token"], "body.face", "{face}");
    assert!(face["ok"]["bases"].is_null(), "{face}");
    // A default no tuple selects, a type's size, and a chain through another token count too.
    let (wide, _) = s.ask(json!({"method": "explain", "token": "gap.wide"}));
    assert_eq!(wide["ok"]["bases"], json!(["space.md"]), "{wide}");
    let (text, _) = s.ask(json!({"method": "explain", "token": "body.text"}));
    assert_eq!(text["ok"]["bases"], json!(["space.lg"]), "{text}");
    let (inner, _) = s.ask(json!({"method": "explain", "token": "card.inner"}));
    assert_eq!(
        inner["ok"]["bases"],
        json!(["space.md", "space.sm"]),
        "{inner}"
    );
    // Through a token naming an overridden base (radius.lg is the app's).
    let (outer, _) = s.ask(json!({"method": "explain", "token": "card.outer"}));
    assert_eq!(outer["ok"]["bases"], json!(["radius.lg"]), "{outer}");
    // Every branch of a named token counts, not only the tuples that select it
    // (the protocol's `bases` is as wide as `names`).
    let (label, _) = s.ask(json!({"method": "explain", "token": "label.gap"}));
    assert_eq!(
        label["ok"]["bases"],
        json!(["space.md", "space.sm"]),
        "{label}"
    );
    // An overridden base that names another base says so too.
    let (xl, _) = s.ask(json!({"method": "explain", "token": "space.xl"}));
    assert_eq!(xl["ok"]["kind"], "base");
    assert_eq!(xl["ok"]["bases"], json!(["space.lg"]), "{xl}");
    // A base name read from a file that is not the design's: refused, as any token is.
    let (missing, _) =
        s.ask(json!({"method": "resolve", "token": "space.md", "file": "none.tamber"}));
    assert!(missing["error"].is_object(), "{missing}");
    let (missing, _) =
        s.ask(json!({"method": "explain", "token": "space.md", "file": "none.tamber"}));
    assert!(missing["error"].is_object(), "{missing}");
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
    // (macOS's branch the Mac's, the other iOS's: the order matters)
    let apple = std::fs::read_to_string(root.join("host/apple/src/scale.rs")).unwrap();
    let (mac, ios) = (
        apple.find("Column::Mac").unwrap(),
        apple.find("Column::Ios").unwrap(),
    );
    assert!(
        apple[..mac].contains("cfg!(target_os = \"macos\")") && mac < ios,
        "host/apple/src/scale.rs"
    );
    assert_eq!(scale::column_for("macos"), Some("mac"));
    assert_eq!(scale::column_for("ios"), Some("ios"));
    has("host/web/src/host.rs", "roles::Column::Neutral");
    has("host/web-js/src/main.rs", "roles::Column::Neutral");
    assert_eq!(scale::column_for("web"), Some("neutral"));
    assert_eq!(scale::column_for("windows"), Some("neutral"));
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
