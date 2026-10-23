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
    for prop in ["startAngle", "sweepAngle", "polygonCount", "weight"] {
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
      "children":[{"type":"frame","gap":"$n","position":"$n","layout":{"position":"$n"}}]}"#;
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
    // Each name made is charged with the prefix it carries: a long prefix over many variables is refused.
    let prefix = "p".repeat(256 * 1024);
    let vars: Vec<String> = (0..2500)
        .map(|i| format!(r##""c{i}":{{"type":"color","value":"#000000"}}"##))
        .collect();
    let text = format!(r#"{{"version":"2.21","variables":{{{}}}}}"#, vars.join(","));
    assert!(pen::import_within("x.pen", &text, "pen", &design, &vocab(), IMPORT).is_ok());
    let refused = pen::import_within("x.pen", &text, &prefix, &design, &vocab(), IMPORT);
    assert!(refused.is_err(), "a long prefix over 2500 names is charged");
}
