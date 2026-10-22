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
        ("stray", "its theme names `size=big`, which `themes` does not declare, so it never holds"),
        // (no value in some contexts: Pencil gives none on a phone, so no token can hold it)
        ("--tablet-only", "not set in light, device=phone, and no value of it is invented"),
        // (an entry a later one overrides wherever it would hold: Pencil's last-wins)
        ("--edge", "declared again by a later layer, which wins"),
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

#[test]
fn round_1_themes_names_aliases_kinds_and_the_count() {
    let text = fixture("edges.pen");
    let import = import("edges.pen", &text);
    let f = folded(&import);
    let why = |name: &str| -> Vec<String> {
        import
            .unmapped
            .iter()
            .filter(|u| u.name == name)
            .map(|u| u.why.clone())
            .collect()
    };
    // The first scheme-like axis written is the scheme; the other a proposed axis.
    assert_eq!(
        f.design
            .axes
            .iter()
            .map(|a| a.name.value.as_str())
            .collect::<Vec<_>>(),
        ["amode"]
    );
    // An entry naming an undeclared value (or one in another case) never holds: the good pair stands.
    assert_eq!(
        sides(&f.value("pen.surface", 0).unwrap().css()),
        (hex("#ffffff"), hex("#000000"))
    );
    assert_eq!(why("surface").len(), 2, "{:?}", import.unmapped);
    assert!(why("surface")
        .iter()
        .all(|w| w.ends_with("which `themes` does not declare, so it never holds")));
    // `--x` and `x` are two variables whose token names collide, said; `a` and `a.b` are two tokens.
    // (the first written keeps the name: `--x`'s value; `x` is reported as its collision)
    assert!(why("--x").is_empty(), "{:?}", import.unmapped);
    assert_eq!(why("x").len(), 1, "{:?}", import.unmapped);
    assert_eq!(hex(&f.value("pen.x", 0).unwrap().css()), hex("#111111"));
    assert!(f.value("pen.a", 0).is_some() && f.value("pen.a.b", 0).is_some());
    // An alias chain resolves; one to a string, or to nothing, is reported.
    assert_eq!(hex(&f.value("pen.chain", 0).unwrap().css()), hex("#333333"));
    assert_eq!(why("cross"), ["an alias to `word`, a string, from a color"]);
    assert_eq!(
        why("missing"),
        ["an alias to `nowhere`, which `variables` does not declare"]
    );
    // A number and the number it aliases are one kind: read as a gap and as a line height, both numbers.
    assert_eq!(f.value("pen.base", 0).unwrap().css(), "8");
    assert_eq!(f.value("pen.ratio", 0).unwrap().css(), "8");
    assert!(import.notes.iter().any(|n| n.starts_with("`ratio` is a number: Pencil reads it (with the numbers it aliases or that alias it) as a pixel length (gap) and as lineHeight")), "{:?}", import.notes);
    // A side of a stroke's thickness is pixels; a gradient's size is a fraction.
    assert_eq!(f.value("pen.edge", 0).unwrap().css(), "2px");
    assert_eq!(f.value("pen.spread", 0).unwrap().css(), "0.5");
    assert!(
        import
            .notes
            .contains(&"`spread` is a number: Pencil reads it as size.width".to_string()),
        "{:?}",
        import.notes
    );
    // A value of no entries, or an entry of no shape, is reported, the file read all the same.
    assert_eq!(why("none"), ["its `value` is an empty list"]);
    assert_eq!(why("broken"), ["an entry is no `{value, theme?}`"]);
    assert_eq!(
        import.counted.source,
        import.counted.mapped + import.counted.reported
    );
    assert!(import.notes.contains(
        &"the file's frames and components are not imported (its variables and themes are)"
            .to_string()
    ));
}

#[test]
fn round_1_more_contexts_than_a_design_takes_and_a_name_that_multiplies_are_refused_by_the_real_bounds(
) {
    let design = parse(EMPTY).unwrap();
    let five = r#"["a","b","c","d","e"]"#;
    let wide = format!(
        r##"{{"version":"2.21","themes":{{"p":{five},"q":{five},"r":{five}}},"variables":{{"v":{{"type":"color","value":"#000000"}}}}}}"##
    );
    let why = pen::import_with("x.pen", &wide, "pen", &design, &vocab()).unwrap_err()[0]
        .message
        .clone();
    assert!(why.contains("tuples"), "{why}");
    // A long name over many entries: refused by the real budget before its copies are made.
    let name = "n".repeat(64 * 1024);
    let entries = vec![r##"{"value":"#000000"}"##; 16_384].join(",");
    let big = format!(
        r#"{{"version":"2.21","variables":{{"{name}":{{"type":"color","value":[{entries}]}}}}}}"#
    );
    let why = pen::import_with("x.pen", &big, "pen", &design, &vocab()).unwrap_err()[0]
        .message
        .clone();
    assert!(why.contains("expands past what one import may do"), "{why}");
}

#[test]
fn round_2_the_schemas_properties_dark_listed_first_and_what_no_token_can_hold() {
    let text = fixture("schema.pen");
    let import = import("schema.pen", &text);
    let f = folded(&import);
    let why = |name: &str| -> Vec<String> {
        import
            .unmapped
            .iter()
            .filter(|u| u.name == name)
            .map(|u| u.why.clone())
            .collect()
    };
    // `dark` listed first: still the scheme, `default` the light side (the host's default, whatever Pencil lists first).
    assert!(f.design.axes.is_empty());
    assert_eq!(
        sides(&f.value("pen.page", 0).unwrap().css()),
        (hex("#ffffff"), hex("#000000"))
    );
    // A number that differs by the scheme's side has no token (only a colour has a dark side), reported.
    assert!(f.value("pen.pad", 0).is_none());
    assert_eq!(why("pad").len(), 2, "{:?}", import.unmapped);
    // The schema's pixel properties: a side of strokeWidth, a shadow's offset and blur, an effect's radius;
    // a gradient's centre is a fraction; a shader's uniforms and a descendant's id are no property names.
    for (name, css) in [
        ("line", "1px"),
        ("shadowX", "2px"),
        ("shadowBlur", "6px"),
        ("blurRadius", "10px"),
        ("idWidth", "40px"),
        ("centre", "0.5"),
        ("knob", "3"),
    ] {
        assert_eq!(
            f.value(&format!("pen.{name}"), 0)
                .unwrap_or_else(|| panic!("{name}"))
                .css(),
            css,
            "{name}"
        );
    }
    assert!(
        import
            .notes
            .contains(&"`knob` is a number: Pencil reads it as uniforms.width".to_string()),
        "{:?}",
        import.notes
    );
    // An alias cycle makes no token; a literal in braces is reported, not followed.
    assert!(!why("loopA").is_empty() && !why("loopB").is_empty());
    assert_eq!(
        why("braced"),
        ["a literal in braces, which the importer would read as an alias"]
    );
    // A name declared twice keeps its last; each entry counts on its own, one of no shape reported.
    assert_eq!(hex(&f.value("pen.twice", 0).unwrap().css()), hex("#040404"));
    assert_eq!(
        why("twice"),
        ["declared again later in `variables`, which stands"]
    );
    assert_eq!(why("mixedEntries"), ["an entry is no `{value, theme?}`"]);
    assert_eq!(
        sides(
            &f.value("pen.mixedentries", 0)
                .or_else(|| f.value("pen.mixedEntries", 0))
                .unwrap()
                .css()
        ),
        (hex("#020202"), hex("#030303"))
    );
    assert_eq!(
        import.counted.source,
        import.counted.mapped + import.counted.reported
    );
}
