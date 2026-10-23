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
const EMPTY_PEN: &str = r#"{"version":"2.21"}"#;

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
    pencil_within(doc, name, context, 256)
}

/// [`pencil`], an alias chain followed at most `left` deep (a cycle is no value).
fn pencil_within(doc: &Value, name: &str, context: &[(&str, &str)], left: usize) -> Value {
    if left == 0 {
        return Value::Null;
    }
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
        Some(target) => pencil_within(doc, target, context, left - 1),
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

#[test]
fn round_3_names_shapes_and_contexts_a_hostile_file_uses_are_bounded() {
    let design = parse(EMPTY).unwrap();
    let go = |text: &str| pen::import_with("x.pen", text, "pen", &design, &vocab());
    // An axis or value named past 64 characters: refused (a report would say it in every tuple's words).
    let long = "a".repeat(65);
    let why = go(&format!(
        r#"{{"version":"2.21","themes":{{"{long}":["x","y"]}}}}"#
    ))
    .unwrap_err()[0]
        .message
        .clone();
    assert!(why.contains("more than 64 characters"), "{why}");
    // A 64 KiB `type` named by many aliases: each report says it in 20 characters.
    let ty = "t".repeat(64 * 1024);
    let aliases: Vec<String> = (0..2000)
        .map(|i| format!(r#""c{i}":{{"type":"color","value":"$s"}}"#))
        .collect();
    let text = format!(
        r#"{{"version":"2.21","variables":{{"s":{{"type":"{ty}","value":"x"}},{}}}}}"#,
        aliases.join(",")
    );
    let import = go(&text).unwrap();
    assert!(
        import.unmapped.iter().all(|u| u.why.len() < 200),
        "a report's words are bounded"
    );
    // 100,000 entries of no shape: each reported, in no more than a moment (no square of them).
    let entries = vec!["7"; 100_000].join(",");
    let t = std::time::Instant::now();
    let import = go(&format!(
        r#"{{"version":"2.21","variables":{{"v":{{"type":"color","value":[{entries}]}}}}}}"#
    ))
    .unwrap();
    assert_eq!(import.counted.reported, 100_000);
    assert!(
        t.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        t.elapsed()
    );
    // A value that is an object (DTCG's `$ref`) is no Pencil value: reported, never followed.
    let import = go(r##"{"version":"2.21","variables":{"s":{"type":"string","value":"#ff0000"},"c":{"type":"color","value":{"$ref":"#/s"}}}}"##).unwrap();
    assert!(
        import.unmapped.iter().any(|u| u.name == "c"
            && u.why == "a value of no Pencil shape (a colour, number, string or boolean)"),
        "{:?}",
        import.unmapped
    );
}

#[test]
fn round_3_binding_context_duplicates_and_two_proposed_axes() {
    let design = parse(EMPTY).unwrap();
    let text = r##"{
      "version": "2.21",
      "themes": { "density": ["roomy", "tight"], "brand": ["blue", "red"] },
      "variables": {
        "scale": { "type": "number", "value": 2 },
        "meta": { "type": "number", "value": 3 },
        "uniformsId": { "type": "number", "value": 4 },
        "gap": { "type": "number", "value": 8 },
        "nudge": { "type": "number", "value": 5 },
        "kept": { "type": "number", "value": [ { "value": 1 }, { "value": "$leading" } ] },
        "leading": { "type": "number", "value": 1.5 },
        "kept": { "type": "number", "value": 16 },
        "ink": { "type": "color", "value": [ { "value": "#000000" }, { "value": "#111111", "theme": { "density": "tight" } }, { "value": "#222222", "theme": { "brand": "red" } } ] }
      },
      "children": [ { "type": "script", "inputs": { "gap": "$scale" }, "metadata": { "type": "x", "width": "$meta" },
        "descendants": { "uniforms": { "width": "$uniformsId" } }, "id": "$gap", "gap": "$gap", "padding": "$kept",
        "children": [ { "type": "frame", "offset": { "x": "$nudge" }, "lineHeight": "$leading", "fill": "$ink" } ] } ]
    }"##;
    let import = pen::import_with("x.pen", text, "pen", &design, &vocab()).unwrap();
    let f = folded(&import);
    // A script's inputs and metadata are the author's names; a descendant's id is no property; `id` binds nothing;
    // an `offset` that is no shadow's is no pixel offset.
    for (name, css) in [
        ("scale", "2"),
        ("meta", "3"),
        ("uniformsId", "4px"),
        ("gap", "8px"),
        ("nudge", "5"),
    ] {
        assert_eq!(
            f.value(&format!("pen.{name}"), 0)
                .unwrap_or_else(|| panic!("{name}"))
                .css(),
            css,
            "{name}"
        );
    }
    // A name declared twice: the one that stands decides its kind (the discarded alias joins nothing), and every
    // entry of the discarded one is counted.
    assert_eq!(f.value("pen.kept", 0).unwrap().css(), "16px");
    assert_eq!(
        import.unmapped.iter().filter(|u| u.name == "kept").count(),
        2
    );
    // Two proposed axes and no scheme: each tuple as Pencil says.
    let doc: Value = serde_json::from_str(text).unwrap();
    for d in ["roomy", "tight"] {
        for b in ["blue", "red"] {
            let t = f.tuple(&[("density", d), ("brand", b)]).unwrap();
            let want = pencil(&doc, "ink", &[("density", d), ("brand", b)]);
            assert_eq!(
                hex(&f.value("pen.ink", t).unwrap().css()),
                hex(want.as_str().unwrap()),
                "{d} {b}"
            );
        }
    }
    assert_eq!(
        import.counted.source,
        import.counted.mapped + import.counted.reported
    );
}

#[test]
fn round_3_recheck_long_names_long_chains_and_only_bound_properties() {
    let design = parse(EMPTY).unwrap();
    let go = |text: &str| pen::import_with("x.pen", text, "pen", &design, &vocab());
    // A name past 128 characters: reported, never named by an alias's report.
    let long = "n".repeat(64 * 1024);
    let aliases: Vec<String> = (0..2000)
        .map(|i| format!(r#""c{i}":{{"type":"color","value":"$s"}}"#))
        .collect();
    let text = format!(
        r#"{{"version":"2.21","variables":{{"{long}":{{"type":"color","value":"${long}"}},"s":{{"type":"color","value":"${long}"}},{}}}}}"#,
        aliases.join(",")
    );
    let import = go(&text).unwrap();
    assert!(
        import.unmapped.iter().all(|u| u.why.len() < 300),
        "a report's words are bounded"
    );
    assert!(import
        .unmapped
        .iter()
        .any(|u| u.why == "a name of more than 128 characters"));
    // A long chain of colour aliases: its cycle check is charged, refused or done in no more than a moment.
    let chain: Vec<String> = (0..18_000)
        .map(|i| {
            if i == 0 {
                r##""v0":{"type":"color","value":"#000000"}"##.to_string()
            } else {
                format!(r#""v{i}":{{"type":"color","value":"$v{}"}}"#, i - 1)
            }
        })
        .collect();
    let t = std::time::Instant::now();
    let _ = go(&format!(
        r#"{{"version":"2.21","variables":{{{}}}}}"#,
        chain.join(",")
    ));
    assert!(
        t.elapsed() < std::time::Duration::from_secs(20),
        "{:?}",
        t.elapsed()
    );
    // Only a property that takes a variable is a use: an `id`, a `context`, a position or a layout enum is none,
    // while a script input of any name (`id` included) is the author's own.
    let text = r#"{"version":"2.21","variables":{"n":{"type":"number","value":8},"m":{"type":"number","value":9}},
      "children":[{"type":"frame","gap":"$n","id":"$n","context":"$n","x":"$n","layout":"$n"},
                  {"type":"script","inputs":{"id":"$m"},"gap":"$m"}]}"#;
    let import = go(text).unwrap();
    let f = folded(&import);
    assert_eq!(f.value("pen.n", 0).unwrap().css(), "8px");
    assert_eq!(f.value("pen.m", 0).unwrap().css(), "9");
}

#[test]
fn round_3_recheck_2_collisions_more_bindings_and_themes() {
    let design = parse(EMPTY).unwrap();
    let go = |text: &str| pen::import_with("x.pen", text, "pen", &design, &vocab());
    // A long name reported never takes a token name, so many names colliding with it copy nothing of it.
    let dashes = "-".repeat(64 * 1024);
    // (punctuation alone, each name distinct and each read as `x`, as the long one is)
    let others: Vec<String> = (0..500)
        .map(|i: usize| {
            let name: String = (0..5)
                .map(|d| ['!', '?', '*', '~', '@'][i / 5usize.pow(d) % 5])
                .collect();
            format!(r##""{name}":{{"type":"color","value":"#000000"}}"##)
        })
        .collect();
    let import = go(&format!(r##"{{"version":"2.21","variables":{{"{dashes}":{{"type":"color","value":"#000000"}},{}}}}}"##, others.join(","))).unwrap();
    assert!(import.unmapped.iter().all(|u| u.why.len() < 300));
    // innerRadius, a gradient stop's position, fontStyle and content take variables: a use each, none pixels.
    let text = r#"{"version":"2.21","themes":{"opacity":["a","b"]},"variables":{"r":{"type":"number","value":2},"f":{"type":"string","value":"Inter"},"g":{"type":"number","value":4}},
      "children":[{"type":"ellipse","gap":"$r","innerRadius":"$r"},{"type":"text","fontFamily":"$f","content":"$f"},
                  {"type":"frame","gap":"$g","theme":{"opacity":"$g"}}]}"#;
    let import = go(text).unwrap();
    let f = folded(&import);
    assert_eq!(f.value("pen.r", 0).unwrap().css(), "2");
    assert!(
        import.unmapped.iter().any(|u| u.name == "f"),
        "a string read as content too is no family: {:?}",
        import.unmapped
    );
    // A node's theme binds nothing: `g` is read as a gap alone.
    assert_eq!(f.value("pen.g", 0).unwrap().css(), "4px");
}

#[test]
fn round_3_recheck_2_every_schema_binding_counts() {
    let design = parse(EMPTY).unwrap();
    for prop in [
        "startAngle",
        "sweepAngle",
        "polygonCount",
        "weight",
        "flipX",
        "flipY",
        "clip",
        "underline",
        "strikethrough",
        "stroke",
        "colors",
    ] {
        let text = format!(
            r#"{{"version":"2.21","variables":{{"n":{{"type":"number","value":8}}}},"children":[{{"type":"frame","gap":"$n","{prop}":"$n"}}]}}"#
        );
        let import = pen::import_with("x.pen", &text, "pen", &design, &vocab()).unwrap();
        assert_eq!(
            folded(&import).value("pen.n", 0).unwrap().css(),
            "8",
            "{prop}"
        );
    }
    for prop in ["icon", "library", "model", "fontStyle"] {
        let text = format!(
            r#"{{"version":"2.21","variables":{{"s":{{"type":"string","value":"Inter"}}}},"children":[{{"type":"text","fontFamily":"$s","{prop}":"$s"}}]}}"#
        );
        let import = pen::import_with("x.pen", &text, "pen", &design, &vocab()).unwrap();
        assert!(import.unmapped.iter().any(|u| u.name == "s"), "{prop}");
    }
    // A key the schema doesn't bind (`spread`) is no use: `n` is read as a gap alone.
    let text = r#"{"version":"2.21","variables":{"n":{"type":"number","value":8}},"children":[{"type":"frame","gap":"$n","spread":"$n"}]}"#;
    let import = pen::import_with("x.pen", text, "pen", &design, &vocab()).unwrap();
    assert_eq!(folded(&import).value("pen.n", 0).unwrap().css(), "8px");
}

#[test]
fn round_3_recheck_3_descendants_positions_and_prefix() {
    let design = parse(EMPTY).unwrap();
    let go = |text: &str| pen::import_with("x.pen", text, "pen", &design, &vocab());
    // A descendant whose id is `theme` has overrides like any node's: its opacity is a use, so `n` is a number.
    let text = r#"{"version":"2.21","variables":{"n":{"type":"number","value":0.5}},
      "children":[{"type":"frame","gap":"$n"},{"type":"ref","ref":"a","descendants":{"theme":{"opacity":"$n"}}}]}"#;
    assert_eq!(
        folded(&go(text).unwrap()).value("pen.n", 0).unwrap().css(),
        "0.5"
    );
    // A position outside a gradient's stops binds nothing: `n` is read as a gap alone.
    let text = r#"{"version":"2.21","variables":{"n":{"type":"number","value":8}},
      "children":[{"type":"frame","gap":"$n","position":"$n","layout":{"position":"$n"},
                   "fill":{"type":"mesh_gradient","points":[{"position":"$n"}]}}]}"#;
    assert_eq!(
        folded(&go(text).unwrap()).value("pen.n", 0).unwrap().css(),
        "8px"
    );
    // A stop's position is a use, and no pixel one.
    let text = r##"{"version":"2.21","variables":{"n":{"type":"number","value":8}},
      "children":[{"type":"frame","gap":"$n","fill":{"type":"gradient","colors":[{"color":"#000000","position":"$n"}]}}]}"##;
    assert_eq!(
        folded(&go(text).unwrap()).value("pen.n", 0).unwrap().css(),
        "8"
    );
    // A prefix is bounded as a name is, and each name made is charged with the prefix it carries.
    let refused = pen::import_with("x.pen", EMPTY_PEN, &"p".repeat(129), &design, &vocab());
    assert!(refused.unwrap_err()[0]
        .message
        .contains("a prefix is at most 128"));
    // A descendant whose id is `descendants` is a node too: its own `theme` binds nothing.
    let text = r#"{"version":"2.21","themes":{"opacity":["a"]},"variables":{"n":{"type":"number","value":8}},
      "children":[{"type":"frame","gap":"$n"},{"type":"ref","ref":"a","descendants":{"descendants":{"theme":{"opacity":"$n"}}}}]}"#;
    assert_eq!(
        folded(&go(text).unwrap()).value("pen.n", 0).unwrap().css(),
        "8px"
    );
    // A `descendants` map's string value is no override: `n` is read as a gap alone.
    let text = r#"{"version":"2.21","variables":{"n":{"type":"number","value":8}},
      "children":[{"type":"frame","gap":"$n"},{"type":"ref","ref":"a","descendants":{"opacity":"$n"}}]}"#;
    assert_eq!(
        folded(&go(text).unwrap()).value("pen.n", 0).unwrap().css(),
        "8px"
    );
    // A node id `type` in a `descendants` map is no type: `width` under it is still pixels.
    let text = r#"{"version":"2.21","variables":{"n":{"type":"number","value":8}},
      "children":[{"type":"frame","gap":"$n"},{"type":"ref","ref":"a","descendants":{"type":"gradient","node":{"width":"$n"}}}]}"#;
    assert_eq!(
        folded(&go(text).unwrap()).value("pen.n", 0).unwrap().css(),
        "8px"
    );
    // A node id in a `descendants` map is no property: `top` under an id `strokeWidth` is no pixel side,
    // and `position` under an id `colors` no stop.
    for (id, inner) in [
        ("strokeWidth", r#"{"top":"$n"}"#),
        ("colors", r#"{"position":"$n"}"#),
    ] {
        let text = format!(
            r#"{{"version":"2.21","variables":{{"n":{{"type":"number","value":8}}}},
              "children":[{{"type":"frame","gap":"$n"}},{{"type":"ref","ref":"a","descendants":{{"{id}":{inner}}}}}]}}"#
        );
        assert_eq!(
            folded(&go(&text).unwrap()).value("pen.n", 0).unwrap().css(),
            "8px",
            "{id}"
        );
    }
    // An override's `inputs` is a map of the author's names, as a typed node's is: an input named `width` is no
    // pixel property, so `n` read there alone is a number.
    let text = r#"{"version":"2.21","variables":{"n":{"type":"number","value":8}},
      "children":[{"type":"ref","ref":"a","descendants":{"chart":{"inputs":{"width":"$n"}}}}]}"#;
    assert_eq!(
        folded(&go(text).unwrap()).value("pen.n", 0).unwrap().css(),
        "8"
    );
    let prefix = "p".repeat(128);
    let vars: Vec<String> = (0..2500)
        .map(|i| format!(r##""c{i}":{{"type":"color","value":"#000000"}}"##))
        .collect();
    let text = format!(r#"{{"version":"2.21","variables":{{{}}}}}"#, vars.join(","));
    let within = |prefix: &str, limit: u64| {
        pen::import_within("x.pen", &text, prefix, &design, &vocab(), limit).is_ok()
    };
    // (the least budget the short prefix takes, found by halving)
    let (mut low, mut high) = (0, IMPORT);
    while high - low > 1000 {
        let mid = (low + high) / 2;
        if within("pen", mid) {
            high = mid;
        } else {
            low = mid;
        }
    }
    assert!(within("pen", high));
    assert!(
        !within(&prefix, high + 200_000),
        "2500 names' prefixes are charged"
    );
}

#[test]
fn round_3_recheck_4_a_failing_colour_chain_walked_from_every_link_is_charged() {
    let design = parse(EMPTY).unwrap();
    let themes = r#""themes":{"a":["x","y"],"b":["x","y"],"c":["x","y"]}"#;
    let file = |vars: Vec<String>| {
        format!(
            r#"{{"version":"2.21",{themes},"variables":{{{}}}}}"#,
            vars.join(",")
        )
    };
    let n = 2000;
    // Each colour its own literal: what the file costs without a walk.
    let plain = file(
        (0..n)
            .map(|i| format!(r##""c{i:04}":{{"type":"color","value":"#000000"}}"##))
            .collect(),
    );
    let within =
        |text: &str, limit: u64| pen::import_within("x.pen", text, "pen", &design, &vocab(), limit);
    let (mut low, mut high) = (0, IMPORT);
    while high - low > 1000 {
        let mid = (low + high) / 2;
        if within(&plain, mid).is_ok() {
            high = mid;
        } else {
            low = mid;
        }
    }
    // A chain to nothing, longer than 256: each link walks 257 in each of 8 tuples, about 4 million steps.
    let chain = file(
        (0..n)
            .map(|i| match i + 1 < n {
                true => format!(r#""c{i:04}":{{"type":"color","value":"$c{:04}"}}"#, i + 1),
                false => format!(r#""c{i:04}":{{"type":"color","value":"$missing"}}"#),
            })
            .collect(),
    );
    let refused = within(&chain, 2 * high + 1_000_000).unwrap_err();
    assert!(refused[0].message.contains("expands past"), "{refused:?}");
}

#[test]
fn round_3_recheck_5_each_tuple_s_copy_of_a_prefixed_name_is_charged() {
    let design = parse(EMPTY).unwrap();
    let axes: Vec<String> = (0..6).map(|i| format!(r#""a{i}":["x","y"]"#)).collect();
    let mut vars = vec![r##""base":{"type":"color","value":"#000000"}"##.to_string()];
    vars.extend((0..500).map(|i| format!(r#""c{i:03}":{{"type":"color","value":"$base"}}"#)));
    let text = format!(
        r#"{{"version":"2.21","themes":{{{}}},"variables":{{{}}}}}"#,
        axes.join(","),
        vars.join(",")
    );
    let within = |prefix: &str, limit: u64| {
        pen::import_within("x.pen", &text, prefix, &design, &vocab(), limit).is_ok()
    };
    let (mut low, mut high) = (0, IMPORT);
    while high - low > 1000 {
        let mid = (low + high) / 2;
        if within("p", mid) {
            high = mid;
        } else {
            low = mid;
        }
    }
    // 500 aliases in 64 tuples, each holding its 384-byte name twice: some 24 million more.
    let long = "€".repeat(128);
    assert!(!within(&long, high + 1_000_000), "the copies are charged");
    assert!(within(&long, IMPORT));
}

#[test]
fn round_3_recheck_6_each_tuple_s_copy_of_why_a_colour_failed_is_charged() {
    let design = parse(EMPTY).unwrap();
    let axes: Vec<String> = (0..6).map(|i| format!(r#""a{i}":["x","y"]"#)).collect();
    let file = |cycle: &str| {
        let mut vars = vec![format!(
            r#""{cycle}":{{"type":"color","value":"${cycle}"}}"#
        )];
        vars.extend(
            (0..300).map(|i| format!(r#""c{i:03}":{{"type":"color","value":"${cycle}"}}"#)),
        );
        format!(
            r#"{{"version":"2.21","themes":{{{}}},"variables":{{{}}}}}"#,
            axes.join(","),
            vars.join(",")
        )
    };
    let within = |text: &str, limit: u64| {
        pen::import_within("x.pen", text, "pen", &design, &vocab(), limit).is_ok()
    };
    let short = file("x");
    let (mut low, mut high) = (0, IMPORT);
    while high - low > 1000 {
        let mid = (low + high) / 2;
        if within(&short, mid) {
            high = mid;
        } else {
            low = mid;
        }
    }
    // 300 aliases in 64 tuples, each holding a why that names a 384-byte cycle: some 7 million more.
    let long = file(&"€".repeat(128));
    assert!(!within(&long, high + 1_000_000), "the failures are charged");
    assert!(within(&long, IMPORT));
}

#[test]
fn round_3_recheck_7_a_refusal_carried_down_a_long_chain_is_one_charged_walk() {
    // The design's `pen.c0000` is a length: the import's colour of that name is refused, and every colour of
    // the chain that names it goes with it, in one walk, not a pass a link.
    let design = parse("tamber app\n\ntheme\n  pen.c0000: length = 8px\n").unwrap();
    let mut vars = vec![r##""c0000":{"type":"color","value":"#000000"}"##.to_string()];
    vars.extend(
        (1..4000).map(|i| format!(r#""c{i:04}":{{"type":"color","value":"$c{:04}"}}"#, i - 1)),
    );
    let text = format!(r#"{{"version":"2.21","variables":{{{}}}}}"#, vars.join(","));
    let t = std::time::Instant::now();
    let import = pen::import_with("x.pen", &text, "pen", &design, &vocab());
    assert!(
        t.elapsed() < std::time::Duration::from_secs(20),
        "{:?}",
        t.elapsed()
    );
    if let Ok(import) = import {
        assert!(
            import
                .unmapped
                .iter()
                .any(|u| u.name == "c3999" && u.why.contains("names")),
            "the chain's end goes too"
        );
    }
}

/// The lead's sweep (2026-10-23): one hostile file for each pass that walks or folds the design or keeps in
/// proportion to its input, each under the wire's 1 MiB. Through the real budget each is done or refused within
/// a bound of time; through a budget of a few times its own size each is refused, and at once: no pass runs
/// uncharged past what it was given.
#[test]
fn every_pass_over_a_hostile_file_is_charged_to_the_budget() {
    let axes = |n: usize| {
        (0..n)
            .map(|i| format!(r#""a{i}":["x","y"]"#))
            .collect::<Vec<_>>()
            .join(",")
    };
    let scheme = r#""mode":["light","dark"]"#;
    let themed = |name: &str, kind: &str, a: &str, b: &str| {
        format!(
            r#""{name}":{{"type":"{kind}","value":[{{"value":{a}}},{{"value":{b},"theme":{{"a0":"y","mode":"dark"}}}}]}}"#
        )
    };
    let file = |themes: String, vars: Vec<String>, children: &str| {
        format!(
            r#"{{"version":"2.21","themes":{{{themes}}},"variables":{{{}}},"children":[{children}]}}"#,
            vars.join(",")
        )
    };
    let mut files: Vec<(&str, String, exact_tamber::Design)> = Vec::new();
    let empty = parse(EMPTY).unwrap();
    // Winners, resolution and lines: many variables in every one of 128 tuples.
    files.push((
        "wide",
        file(
            format!("{scheme},{}", axes(6)),
            (0..8000)
                .map(|i| {
                    themed(
                        &format!("c{i}"),
                        "color",
                        r##""#000000""##,
                        r##""#ffffff""##,
                    )
                })
                .collect(),
            "",
        ),
        empty.clone(),
    ));
    // Values of another kind, long, kept in every tuple: families, each read as `fontFamily`, so resolved, laid
    // and probed.
    let long = format!(r#""{}""#, "Inter ".repeat(150));
    let families = (0..50)
        .map(|i| format!(r#"{{"type":"text","fontFamily":"$s{i}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    files.push((
        "strings",
        file(
            format!("{scheme},{}", axes(6)),
            (0..50)
                .map(|i| themed(&format!("s{i}"), "string", &long, &long))
                .collect(),
            &families,
        ),
        empty.clone(),
    ));
    // A number as a CSS expression that multiplies when folded: no Pencil number, refused before any fold.
    let mut grow = vec![r#""g0":{"type":"number","value":"space.md"}"#.to_string()];
    grow.extend((1..19).map(|i| {
        format!(
            r#""g{i}":{{"type":"number","value":"clamp(g{p}, g{p}, g{p})"}}"#,
            p = i - 1
        )
    }));
    let gaps = (0..19)
        .map(|i| format!(r#"{{"type":"frame","gap":"$g{i}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    files.push(("expression", file(axes(1), grow, &gaps), empty.clone()));
    // Alias chains: every link walked, past 256 too.
    files.push((
        "chain",
        file(
            format!("{scheme},{}", axes(6)),
            std::iter::once(r##""c0":{"type":"color","value":"#000000"}"##.to_string())
                .chain(
                    (1..12000)
                        .map(|i| format!(r#""c{i}":{{"type":"color","value":"$c{}"}}"#, i - 1)),
                )
                .collect(),
            "",
        ),
        empty.clone(),
    ));
    // Numbers, their kinds by use: many nodes binding many of them.
    let nodes = (0..6000)
        .map(|i| {
            format!(
                r#"{{"type":"frame","gap":"$n{}","opacity":"$n{}"}}"#,
                i % 2000,
                (i + 1) % 2000
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    files.push((
        "uses",
        file(
            axes(4),
            (0..2000)
                .map(|i| format!(r#""n{i}":{{"type":"number","value":{i}}}"#))
                .collect(),
            &nodes,
        ),
        empty.clone(),
    ));
    // Landing: the design's own token of an imported name, a chain naming it.
    let named = parse("tamber app\n\ntheme\n  pen.c0: length = 8px\n").unwrap();
    files.push((
        "landing",
        file(
            axes(6),
            std::iter::once(r##""c0":{"type":"color","value":"#000000"}"##.to_string())
                .chain(
                    (1..6000)
                        .map(|i| format!(r#""c{i}":{{"type":"color","value":"$c{}"}}"#, i - 1)),
                )
                .collect(),
            "",
        ),
        named,
    ));
    for (what, text, design) in &files {
        assert!(text.len() < 1 << 20, "{what}: {} bytes", text.len());
        let t = std::time::Instant::now();
        let full = pen::import_within("x.pen", text, "pen", design, &vocab(), IMPORT);
        assert!(
            t.elapsed() < std::time::Duration::from_secs(20),
            "{what}: {:?} on the real budget",
            t.elapsed()
        );
        // What each reaches, asserted: the families are resolved, probed and laid (each made or said why); the
        // expressions are reported as no numbers, never folded.
        if *what == "strings" {
            let import = full.as_ref().expect("families within the budget");
            assert!(
                import.counted.mapped + import.counted.reported >= 50,
                "{:?}",
                import.counted
            );
        }
        if *what == "expression" {
            let import = full
                .as_ref()
                .expect("a refused value, not a refused import");
            assert!(
                import
                    .unmapped
                    .iter()
                    .filter(|u| u.name.starts_with('g'))
                    .all(|u| u.why.contains("no number")),
                "{:?}",
                import.unmapped
            );
        }
        // A budget of three times the file: done or refused, and either quickly (what multiplies is refused;
        // the expressions, reported at once, are done).
        let t = std::time::Instant::now();
        let small = pen::import_within(
            "x.pen",
            text,
            "pen",
            design,
            &vocab(),
            3 * text.len() as u64,
        );
        let took = t.elapsed();
        assert!(
            took < std::time::Duration::from_secs(2),
            "{what}: {took:?} on a small budget"
        );
        if *what != "expression" {
            assert!(small.is_err(), "{what}: done within three times its size");
        }
    }
}
