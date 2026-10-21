//! Pencil's `.pen` (LLP 1121 stage 13 B): its variables and themes read
//! into the DTCG importer's layers. Every value is mapped or reported,
//! counted; the transaction lands; and in every context the design selects
//! what Pencil's own rule selects ("the last entry whose theme is
//! satisfied"), computed here independently of the importer.

use crate::common::vocab;
use exact_tamber::import::dtcg::pen;
use exact_tamber::import::Import;
use exact_tamber::{apply_all, check, parse};
use serde_json::Value;

const EMPTY: &str = "tamber app\n";
/// A fresh import's budget, as `crate::budget::IMPORT` has it.
const IMPORT: u64 = 3 << 24;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/import/pen/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn import(name: &str, text: &str) -> Import {
    pen::import_with(name, text, "pen", &parse(EMPTY).unwrap(), &vocab())
        .unwrap_or_else(|e| panic!("{e:?}"))
}

/// The design the import's transaction makes, folded.
fn folded(import: &Import) -> exact_tamber::Folding {
    let text = apply_all(EMPTY, &import.ops, &vocab()).unwrap().text;
    check(&text, &vocab()).unwrap_or_else(|e| panic!("{e:?}\n{text}"))
}

/// Pencil's rule, on the document alone: in `context` (axis → value, every
/// axis), a variable's value is its last entry whose `theme` holds (none
/// always holds); an alias (`"$x"`) is `x`'s in the same context.
fn pencil(doc: &Value, name: &str, context: &[(&str, &str)]) -> Value {
    let entries: Vec<(Value, Option<&serde_json::Map<String, Value>>)> =
        match &doc["variables"][name]["value"] {
            Value::Array(all) => all
                .iter()
                .map(|e| (e["value"].clone(), e["theme"].as_object()))
                .collect(),
            v => vec![(v.clone(), None)],
        };
    // (the last that holds: searched from the end)
    let won = entries
        .iter()
        .rfind(|(_, t)| {
            t.is_none_or(|t| {
                t.iter().all(|(a, v)| {
                    context
                        .iter()
                        .any(|(ca, cv)| ca == a && Some(*cv) == v.as_str())
                })
            })
        })
        .map(|(v, _)| v.clone())
        .unwrap_or(Value::Null);
    match won.as_str().and_then(|s| s.strip_prefix('$')) {
        Some(target) => pencil(doc, target, context),
        None => won,
    }
}

/// A colour as both can write it: lower case, its alpha written (`#rrggbbaa`).
fn hex(v: &str) -> String {
    let v = v.trim().to_ascii_lowercase();
    match v.len() {
        7 => format!("{v}ff"),
        _ => v,
    }
}

/// A folded colour's two sides: `light-dark(a, b)`, or one value both.
fn sides(css: &str) -> (String, String) {
    match css
        .strip_prefix("light-dark(")
        .and_then(|r| r.strip_suffix(')'))
    {
        Some(inner) => {
            let (l, d) = inner.split_once(',').unwrap();
            (hex(l), hex(d))
        }
        None => (hex(css), hex(css)),
    }
}

#[test]
fn a_mode_file_with_dotted_names_maps_its_scheme_its_colours_and_its_pixel_lengths() {
    let text = fixture("mode.pen");
    let import = import("mode.pen", &text);
    // (each entry a value: the page colour's two, and three more)
    assert_eq!(
        (
            import.counted.source,
            import.counted.mapped,
            import.counted.reported
        ),
        (5, 5, 0),
        "{:?}",
        import.unmapped
    );
    let f = folded(&import);
    let doc: Value = serde_json::from_str(&text).unwrap();
    // `mode` = default | dark is the scheme: no app axis, the colour a light-dark pair.
    assert!(f.design.axes.is_empty(), "{:?}", f.design.axes);
    let (l, d) = sides(&f.value("pen.color.page-bg", 0).unwrap().css());
    assert_eq!(
        (l, d),
        (
            hex(pencil(&doc, "color.page-bg", &[("mode", "default")])
                .as_str()
                .unwrap()),
            hex(pencil(&doc, "color.page-bg", &[("mode", "dark")])
                .as_str()
                .unwrap())
        )
    );
    // Numbers every frame reads as pixels are lengths; the notes say which properties decided it.
    assert_eq!(f.value("pen.radius.card", 0).unwrap().css(), "8px");
    assert_eq!(f.value("pen.space.page", 0).unwrap().css(), "16px");
    assert!(
        import.notes.contains(
            &"`radius.card` is a length (px): Pencil reads it as cornerRadius".to_string()
        ),
        "{:?}",
        import.notes
    );
    // Again, onto the design it made: nothing to propose.
    let made = apply_all(EMPTY, &import.ops, &vocab()).unwrap().text;
    let again =
        pen::import_with("mode.pen", &text, "pen", &parse(&made).unwrap(), &vocab()).unwrap();
    assert!(again.ops.is_empty(), "{:?}", again.ops);
}

#[test]
fn two_axes_combined_themes_last_wins_aliases_kinds_by_use_and_what_is_reported() {
    let text = fixture("axes.pen");
    let import = import("axes.pen", &text);
    let f = folded(&import);
    let doc: Value = serde_json::from_str(&text).unwrap();
    // `Mode` is the scheme; `device` a proposed axis, `phone` its default.
    let device = f
        .design
        .axes
        .iter()
        .find(|a| a.name.value == "device")
        .expect("device axis");
    assert_eq!(device.values, ["phone", "tablet"]);
    // Every context: the design selects what Pencil does, the scheme's two sides by `Mode` (`--edge` in dark on a
    // tablet: the later, less specific entry wins, as Pencil says, not the one naming more axes).
    for dev in ["phone", "tablet"] {
        let t = f.tuple(&[("device", dev)]).unwrap();
        for (var, token) in [
            ("--primary", "pen.primary"),
            ("--primary-foreground", "pen.primary-foreground"),
            ("--edge", "pen.edge"),
        ] {
            let want = |mode: &str| pencil(&doc, var, &[("Mode", mode), ("device", dev)]);
            let (l, d) = sides(&f.value(token, t).unwrap_or_else(|| panic!("{token}")).css());
            match (want("Light"), want("Dark")) {
                (Value::String(wl), Value::String(wd)) => {
                    assert_eq!((l, d), (hex(&wl), hex(&wd)), "{var} device={dev}")
                }
                other => panic!("{var} device={dev}: Pencil has {other:?}"),
            }
        }
        let gap = pencil(&doc, "gap", &[("Mode", "Light"), ("device", dev)]);
        assert_eq!(
            f.value("pen.gap", t).unwrap().css(),
            format!("{}px", gap),
            "gap device={dev}"
        );
    }
    // Kinds by use: fontWeight and lineHeight are numbers; one read as a pixel length and as lineHeight is a
    // number, said; one no frame reads is a number; a string read as fontFamily is a family.
    assert_eq!(f.value("pen.weight", 0).unwrap().css(), "600");
    assert_eq!(f.value("pen.leading", 0).unwrap().css(), "1.5");
    assert_eq!(f.value("pen.mixed", 0).unwrap().css(), "4");
    assert!(import.notes.iter().any(|n| n == "`mixed` is a number: Pencil reads it as a pixel length (gap) and as lineHeight, so it is no length"), "{:?}", import.notes);
    assert!(
        import
            .notes
            .iter()
            .any(|n| n == "`unused` is a number: no frame reads it"),
        "{:?}",
        import.notes
    );
    assert!(
        f.tokens.iter().any(|t| t.name == "pen.font"),
        "a family token"
    );
    // Reported: the other string, the boolean, an unknown type, a theme naming an undeclared axis, the import.
    let reported: Vec<(&str, &str)> = import
        .unmapped
        .iter()
        .map(|u| (u.name.as_str(), u.why.as_str()))
        .collect();
    for (name, why) in [
        ("label", "no Tamber kind: a string or a boolean"),
        ("flag", "no Tamber kind: a string or a boolean"),
        ("odd", "a Pencil `type` the importer does not name (`gradient`)"),
        ("stray", "its theme names `size=big`, which `themes` does not declare"),
        // (no value in some contexts: Pencil gives none on a phone, so no token can hold it)
        ("tablet-only", "not set in light, device=phone, and no value of it is invented"),
        // (an entry a later one overrides wherever it would hold: Pencil's last-wins)
        ("edge", "declared again by a later layer, which wins"),
        ("imports.kit", "an imported `.pen`: not followed (a client's upload has no directory to find it in); import that file on its own"),
    ] {
        assert!(reported.contains(&(name, why)), "{name}: {reported:?}");
    }
    assert_eq!(
        import.counted.source,
        import.counted.mapped + import.counted.reported
    );
}

#[test]
fn a_file_of_another_version_or_too_deep_or_past_the_budget_is_refused() {
    let design = parse(EMPTY).unwrap();
    let refused = |text: &str, limit: u64| {
        pen::import_within("x.pen", text, "pen", &design, &vocab(), limit).unwrap_err()[0]
            .message
            .clone()
    };
    assert_eq!(
        refused(r#"{"version": "3.0", "children": []}"#, IMPORT),
        "x.pen: a `.pen` of version 3.0: this importer reads 2.x"
    );
    assert_eq!(
        refused(r#"{"children": []}"#, IMPORT),
        "x.pen: no `version`: is this a Pencil `.pen` file?"
    );
    // A node tree deeper than JSON is read to: refused as JSON is.
    let deep = format!(
        r#"{{"version": "2.21", "children": {}0{}}}"#,
        "[".repeat(200),
        "]".repeat(200)
    );
    assert!(
        refused(&deep, IMPORT).contains("recursion limit"),
        "{}",
        refused(&deep, IMPORT)
    );
    // The walk of the frames is charged: a tree that spends the budget is refused with its message.
    let wide = format!(
        r#"{{"version": "2.21", "children": [{}]}}"#,
        vec![r#"{"type":"frame","fill":"$x"}"#; 2000].join(",")
    );
    let why = refused(&wide, 5_000 + wide.len() as u64);
    assert!(why.contains("expands past what one import may do"), "{why}");
    assert!(pen::import_within("x.pen", &wide, "pen", &design, &vocab(), IMPORT).is_ok());
}
