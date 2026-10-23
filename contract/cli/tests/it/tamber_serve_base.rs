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
        scale::ScaleSource::Stretch { metric, why } => {
            json!({"kind": "stretch", "ref": metric, "why": why})
        }
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
    // The Mac's space.xl is a stretch, its metric and why on the wire as
    // the tool reads them (stage 13 D5).
    let xl = base.iter().find(|r| r["name"] == "space.xl").unwrap();
    assert_eq!(xl["columns"]["mac"]["source"]["kind"], "stretch");
    assert_eq!(xl["columns"]["mac"]["source"]["ref"], "grouped-label-inset");
    assert!(xl["columns"]["mac"]["source"]["why"]
        .as_str()
        .is_some_and(|w| !w.is_empty()));
    assert_eq!(xl["columns"]["mac"]["value"], "30px");
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
    // Per tuple (stage 13 D4, *1.16*): each tuple's own line and the lines
    // of the tokens it names there, each side. label.gap reaches card.pad
    // only where density is roomy, where card.pad is space.md: space.sm,
    // card.pad's tight line, is in `bases` and in no tuple.
    assert_eq!(
        label["ok"]["bases_by_tuple"],
        json!([
            {"modes": {"density": "roomy"}, "light": ["space.md"], "dark": ["space.md"]},
            {"modes": {"density": "tight"}, "light": [], "dark": []},
        ]),
        "{label}"
    );
    assert_eq!(
        pad["ok"]["bases_by_tuple"],
        json!([
            {"modes": {"density": "roomy"}, "light": ["space.md"], "dark": ["space.md"]},
            {"modes": {"density": "tight"}, "light": ["space.sm"], "dark": ["space.sm"]},
        ]),
        "{pad}"
    );
    // gap.wide's default, space.md, is no tuple's (each has its `when`):
    // in `bases`, in no tuple (Astra's D4 round 2).
    assert_eq!(
        wide["ok"]["bases_by_tuple"],
        json!([
            {"modes": {"density": "roomy"}, "light": [], "dark": []},
            {"modes": {"density": "tight"}, "light": [], "dark": []},
        ]),
        "{wide}"
    );
    // A token reaching no base has neither field.
    assert!(face["ok"]["bases_by_tuple"].is_null(), "{face}");
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

/// Stage 12 D1 (*1.15*): a length over unset base names answers each
/// column's value beside its expression, in resolve's tuples and on
/// explain's value and `when` lines; one over an app-set base name, or
/// none, has no columns.
#[test]
fn a_length_over_base_names_answers_each_column() {
    let dir = app(
        "base-columns",
        "tamber t

axes
  density = roomy | tight
  tone = calm | loud

theme
  pad2 = calc(space.md * 2)
  third = calc(space.md / 3)
  round = calc(radius.full - 1px)
  spec = 1px
    when density=tight: calc(space.xs * 2)
    when density=tight tone=loud: calc(space.sm * 2)
  shadowed = 1px
    when tone=loud: calc(space.lg * 2)
    when density=roomy tone=loud: calc(space.md * 3)
    when density=tight tone=loud: calc(space.sm * 3)
  covered = calc(space.xl * 2)
    when density=roomy: calc(space.xs * 5)
    when density=tight: calc(space.xs * 6)
  neg = calc(space.md * -1)
  gap.m = max(space.sm, 12px)
  edge = 4px
    when density=tight: calc(space.xs + 2px)
  space.lg = 20px
  wide = calc(space.lg * 2)
  plain = 8px
  alias = space.md
",
    );
    let mut s = Serve::start(&dir, &[]);
    // Each column's px, from the kernel's own table (not its evaluator):
    // `space.md` twice; the larger of `space.sm` and 12px.
    let px = |name: &str, key: &str| {
        let column = scale::COLUMNS.iter().find(|(k, _)| *k == key).unwrap().1;
        match scale::of(name, column).unwrap() {
            scale::ScaleValue::Px(n) | scale::ScaleValue::Cells(n) => n,
            scale::ScaleValue::Full => unreachable!(),
        }
    };
    // As the wire writes a column: the f32 the evaluator folds, shortest;
    // the terminal in column-width cells (`1ch` is 8px to the evaluator).
    let text = |key: &str, n: f64| {
        let n = n as f32;
        match key {
            "terminal" if n == 0.0 => "0".to_string(),
            "terminal" => format!("{}cell", n),
            _ => format!("{n}px"),
        }
    };
    let (pad2, _) = s.ask(json!({"method": "resolve", "token": "pad2"}));
    for (key, _) in scale::COLUMNS {
        assert_eq!(
            pad2["ok"]["tuples"][0]["columns"][key]["value"],
            text(key, px("space.md", key) * 2.0),
            "{key} {pad2}"
        );
    }
    let (gap, _) = s.ask(json!({"method": "resolve", "token": "gap.m"}));
    for key in ["ios", "mac", "neutral", "material"] {
        assert_eq!(
            gap["ok"]["tuples"][0]["columns"][key]["value"],
            text(key, px("space.sm", key).max(12.0)),
            "{key} {gap}"
        );
    }
    // The terminal's mixed: a cell is 8px to the evaluator, 12px one and a half.
    assert_eq!(
        gap["ok"]["tuples"][0]["columns"]["terminal"]["value"],
        text(
            "terminal",
            (px("space.sm", "terminal") * 8.0).max(12.0) / 8.0
        ),
        "{gap}"
    );
    // A fraction, as the evaluator's f32 has it; radius.full in one.
    let (third, _) = s.ask(json!({"method": "resolve", "token": "third"}));
    assert_eq!(
        third["ok"]["tuples"][0]["columns"]["ios"]["value"],
        format!("{}px", px("space.md", "ios") as f32 / 3.0),
        "{third}"
    );
    let (round, _) = s.ask(json!({"method": "resolve", "token": "round"}));
    // (radius.full is 9999px on a drawing column, 0 cells on the terminal)
    assert_eq!(
        round["ok"]["tuples"][0]["columns"]["ios"]["value"], "9998px",
        "{round}"
    );
    assert_eq!(
        round["ok"]["tuples"][0]["columns"]["terminal"]["value"], "-0.125cell",
        "{round}"
    );
    // A negative keeps its sign.
    let (neg, _) = s.ask(json!({"method": "resolve", "token": "neg"}));
    assert_eq!(
        neg["ok"]["tuples"][0]["columns"]["ios"]["value"],
        text("ios", -px("space.md", "ios")),
        "{neg}"
    );
    assert_eq!(
        neg["ok"]["tuples"][0]["columns"]["terminal"]["value"],
        text("terminal", -px("space.md", "terminal")),
        "{neg}"
    );
    // explain: the default's own, when it reads one.
    let (pad2e, _) = s.ask(json!({"method": "explain", "token": "pad2"}));
    assert_eq!(
        pad2e["ok"]["columns"]["ios"]["value"],
        text("ios", px("space.md", "ios") * 2.0),
        "{pad2e}"
    );
    // The line a tuple chooses is the one naming the most axes: each line its own columns.
    let (spec, _) = s.ask(json!({"method": "explain", "token": "spec"}));
    assert_eq!(
        spec["ok"]["whens"][0]["columns"]["ios"]["value"],
        text("ios", px("space.xs", "ios") * 2.0),
        "{spec}"
    );
    assert_eq!(
        spec["ok"]["whens"][1]["columns"]["ios"]["value"],
        text("ios", px("space.sm", "ios") * 2.0),
        "{spec}"
    );
    // A line no tuple chooses (both two-axis lines cover it): no columns.
    let (shadowed, _) = s.ask(json!({"method": "explain", "token": "shadowed"}));
    // (the two lines that win read base names of their own: the shadowed one's would be space.lg's)
    assert!(
        shadowed["ok"]["whens"][0]["columns"].is_null(),
        "{shadowed}"
    );
    assert_eq!(
        shadowed["ok"]["whens"][1]["columns"]["ios"]["value"],
        text("ios", px("space.md", "ios") * 3.0),
        "{shadowed}"
    );
    assert_eq!(
        shadowed["ok"]["whens"][2]["columns"]["ios"]["value"],
        text("ios", px("space.sm", "ios") * 3.0),
        "{shadowed}"
    );
    // A default no tuple chooses: none, though it reads a base name.
    let (covered, _) = s.ask(json!({"method": "explain", "token": "covered"}));
    assert!(covered["ok"]["columns"].is_null(), "{covered}");
    // explain: the default (4px) has none; the tight line's own.
    let (edge, _) = s.ask(json!({"method": "explain", "token": "edge"}));
    assert!(edge["ok"]["columns"].is_null(), "{edge}");
    assert_eq!(
        edge["ok"]["whens"][0]["columns"]["ios"]["value"],
        text("ios", px("space.xs", "ios") + 2.0),
        "{edge}"
    );
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "edge"}));
    assert!(
        resolved["ok"]["tuples"][0]["columns"].is_null(),
        "{resolved}"
    );
    // (roomy's two tuples 4px; tight's two the expression's)
    assert!(
        resolved["ok"]["tuples"][1]["columns"].is_null(),
        "{resolved}"
    );
    assert!(
        !resolved["ok"]["tuples"][2]["columns"].is_null(),
        "{resolved}"
    );
    // One over an app-set base name is one value; one over none, too.
    for name in ["wide", "plain"] {
        let (r, _) = s.ask(json!({"method": "resolve", "token": name}));
        assert!(r["ok"]["tuples"][0]["columns"].is_null(), "{name} {r}");
        let (e, _) = s.ask(json!({"method": "explain", "token": name}));
        assert!(e["ok"]["columns"].is_null(), "{name} {e}");
    }
    // An alias of one: each host's too.
    let (alias, _) = s.ask(json!({"method": "resolve", "token": "alias"}));
    assert_eq!(
        alias["ok"]["tuples"][0]["columns"]["mac"]["value"],
        text("mac", px("space.md", "mac")),
        "{alias}"
    );
    let (wide, _) = s.ask(json!({"method": "resolve", "token": "wide"}));
    assert_eq!(wide["ok"]["tuples"][0]["value"], "40px");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Stage 12 B3 (*1.15*): a shadow token's lengths over base names are
/// answered as written in `length_texts` (null among `lengths`), and its
/// `bases` list them; a shadow over none has no `length_texts`.
#[test]
fn a_shadow_over_base_names_answers_its_lengths_as_written() {
    let dir = app(
        "base-shadow",
        "tamber t\n\ntheme\n  card.shadow: box-shadow = 0 space.md calc(space.sm * 2) #000\n  plain.shadow: box-shadow = 0 1px 2px #000\n",
    );
    let mut s = Serve::start(&dir, &[]);
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "card.shadow"}));
    let tuple = &resolved["ok"]["tuples"][0];
    // The numbers' field keeps its type: null where a length is over base
    // names, every length's text beside (theme2's T1).
    assert_eq!(
        tuple["shadows"][0]["lengths"],
        json!([0.0, null, null]),
        "{resolved}"
    );
    assert_eq!(
        tuple["shadows"][0]["length_texts"],
        json!(["0px", "space.md", "calc(space.sm * 2)"]),
        "{resolved}"
    );
    assert!(
        tuple["value"]
            .as_str()
            .is_some_and(|v| v.starts_with("0px space.md calc(space.sm * 2) ")),
        "{resolved}"
    );
    let (why, _) = s.ask(json!({"method": "explain", "token": "card.shadow"}));
    assert_eq!(why["ok"]["bases"], json!(["space.md", "space.sm"]), "{why}");
    let (plain, _) = s.ask(json!({"method": "resolve", "token": "plain.shadow"}));
    let one = &plain["ok"]["tuples"][0]["shadows"][0];
    assert_eq!(one["lengths"], json!([0.0, 1.0, 2.0]), "{plain}");
    assert!(one.get("length_texts").is_none(), "{plain}");
}

/// `bases_by_tuple` is what the checker resolved in each tuple (stage 13
/// D4, round 1): a function's name, a shadow's `inset`, a type's style and
/// a quoted family are no references, though a token is named like each.
#[test]
fn bases_by_tuple_is_what_each_tuple_resolves() {
    let design = "tamber t

axes
  density = roomy | tight

theme
  calc = space.sm
  inset = space.sm
  italic = space.sm
  pad.c = calc(space.md * 2)
  shade: box-shadow = inset 0 space.md 8px #000000
  body.i: type = italic space.lg
  body.q: type = space.lg 'space.md'
  mixed = 4px
    when density=tight: calc(space.xs * 2)
  first = second
  second = space.md
    when density=tight: space.sm
  hidden = other
    when density=roomy: dep
    when density=tight: 4px
  dep = space.lg
  other = space.xl
";
    let dir = app("base-by-tuple", design);
    let mut s = Serve::start(&dir, &[]);
    let by = |s: &mut Serve, name: &str| {
        s.ask(json!({"method": "explain", "token": name})).0["ok"]["bases_by_tuple"].clone()
    };
    let both = |a: &[&str], b: &[&str]| {
        json!([
            {"modes": {"density": "roomy"}, "light": a, "dark": a},
            {"modes": {"density": "tight"}, "light": b, "dark": b},
        ])
    };
    assert_eq!(by(&mut s, "pad.c"), both(&["space.md"], &["space.md"]));
    assert_eq!(by(&mut s, "shade"), both(&["space.md"], &["space.md"]));
    assert_eq!(by(&mut s, "body.i"), both(&["space.lg"], &["space.lg"]));
    assert_eq!(by(&mut s, "body.q"), both(&["space.lg"], &["space.lg"]));
    // Each tuple its own line.
    assert_eq!(by(&mut s, "mixed"), both(&[], &["space.xs"]));
    // A token first folded inside another's value is recorded as its own
    // (`second`, declared after `first`, which names it); one first folded
    // in a line no tuple selects (`other`, the default of `hidden`)
    // too, and that line gives `hidden` nothing (Astra's D4 round 2).
    assert_eq!(by(&mut s, "first"), both(&["space.md"], &["space.sm"]));
    assert_eq!(by(&mut s, "second"), both(&["space.md"], &["space.sm"]));
    assert_eq!(by(&mut s, "hidden"), both(&["space.lg"], &[]));
    assert_eq!(by(&mut s, "other"), both(&["space.xl"], &["space.xl"]));
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Why `bases_by_tuple`'s light and dark are always alike (theme2's 19:47
/// and 20:31): a base name is a length's, and a length (or a token over a
/// base, a shadow or a type) can't vary by an axis `under scheme=dark`,
/// which the checker refuses (`tamber-under-kind`), so no tuple's dark side
/// reads another line than its light one.
#[test]
fn a_lengths_when_under_scheme_dark_is_refused() {
    let vocab = contract_lower::tamber_vocab();
    for value in ["12px", "space.lg", "calc(space.md * 2)"] {
        let src = format!("tamber t\n\naxes\n  dark-style = dim | black  under scheme=dark\n\ntheme\n  gap: length = 8px\n    when dark-style=black: {value}\n");
        let refused = exact_tamber::check(&src, &vocab).err().unwrap_or_default();
        assert!(
            refused.iter().any(|d| d.id == "tamber-under-kind"),
            "{value}: {refused:?}"
        );
    }
}
