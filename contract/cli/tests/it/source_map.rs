//! The compiler's map names authored locations, not generated names or runtime text.
use exact_kernel::Kernel;
use exact_plan::Plan;
use exact_runner::{DataError, DataSource, Runner, Value};
use serde_json::Value as Json;
use std::{path::PathBuf, process::Command};

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        let suffix = if cfg!(windows) {
            "é space"
        } else {
            "é space\""
        };
        let path =
            std::env::temp_dir().join(format!("exact-map-{name}-{}-{suffix}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
    fn compile(&self) -> (Plan, Json) {
        let path = self.0.join("app.contract");
        let (plan, map) = contract::compile_path_mapped(&path).unwrap();
        assert_eq!(
            plan.encode(),
            contract::compile_path(&path).unwrap().encode()
        );
        let json: Json = serde_json::from_str(&map.json(&plan.encode())).unwrap();
        assert_eq!(json["digest"], contract::plan_digest(&plan.encode()));
        assert_eq!(json["nodes"].as_array().unwrap().len(), plan.nodes.len());
        (plan, json)
    }
}
impl Drop for App {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        if name == "entries" {
            Ok(Value::list(vec![Value::str("a"), Value::str("b")]))
        } else {
            Err(DataError::UnknownSource(name.into()))
        }
    }
}
fn boot(plan: Plan) -> Runner<NoData> {
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn captured_source_paths_relocate_together_without_reading_newer_contents() {
    let app = App::new("relocate");
    let root = app.write("app.contract", "use Child from \"./ui/child.contract\"\ncomponent App\n  view\n    column\n      Child()\n");
    app.write(
        "ui/child.contract",
        "component Child\n  view\n    text \"child\" testId=\"child\"\n",
    );
    let (plan, mut map) = contract::compile_path_mapped(&root).unwrap();
    let before = map.json(&plan.encode());
    let original = app.0.join("missing-original");
    assert!(map.relocate_sources(&app.0.join("ui"), &original).is_err());
    assert_eq!(
        before,
        map.json(&plan.encode()),
        "a refused relocation changes nothing"
    );
    // Changing or removing original files cannot alter captured line ranges.
    map.relocate_sources(&app.0, &original).unwrap();
    let json: Json = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let runner = boot(plan);
    let child = at(&runner, &json, "child");
    assert_eq!(
        child["file"],
        original.join("ui/child.contract").to_str().unwrap()
    );
    assert_eq!(child["line"], 3);
    assert_eq!(
        child["chain"][0]["file"],
        original.join("app.contract").to_str().unwrap()
    );
    assert_eq!(child["chain"][0]["line"], 5);
}
fn at<'a>(runner: &Runner<NoData>, map: &'a Json, id: &str) -> &'a Json {
    let node = runner.kernel().find_by_test_id(id)[0];
    let view = runner.kernel().node_by_key(node).unwrap().id;
    let (site, _) = runner.site_of(view).unwrap();
    &map["nodes"][site.0 as usize]
}

#[test]
fn imports_nested_instances_and_slot_fills_keep_their_own_source_files() {
    let app = App::new("imports");
    let root = app.write(
        "app.contract",
        r#"use Panel from "./ui/panel.contract"
use Counter from "./ui/counter.contract"
component App
  resource entries = entries() as shape string[]
  view
    column testId="root"
      Panel()
        text "from caller" testId="slot"
        Counter(label="slot-counter")
      Counter(label="outside")
      for entry in entries key=entry
        Counter(label=entry)
"#,
    );
    let panel = app.write(
        "ui/panel.contract",
        r#"use Frame from "./frame.contract"
component Panel
  slot
  view
    Frame()
      children
"#,
    );
    let frame = app.write(
        "ui/frame.contract",
        r#"component Frame
  slot
  view
    column testId="frame"
      children
"#,
    );
    let counter = app.write(
        "ui/counter.contract",
        r#"component Counter
  props
    label: string
  state n = 0
  derive doubled = n * 2
  action bump
    n = n + 1
  view
    button press=bump testId=label
      text `${label}: ${doubled}`
"#,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let slot = at(&runner, &map, "slot");
    assert_eq!(slot["file"], root.to_str().unwrap());
    assert_eq!(slot["line"], 8);
    assert_eq!(slot["component"], "App");
    assert_eq!(slot["chain"], serde_json::json!([]));
    let inner = at(&runner, &map, "frame");
    assert_eq!(inner["file"], frame.to_str().unwrap());
    assert_eq!(inner["component"], "Frame");
    assert_eq!(inner["chain"][0]["component"], "Panel");
    assert_eq!(inner["chain"][0]["file"], panel.to_str().unwrap());
    assert_eq!(inner["chain"][0]["line"], 5);
    assert_eq!(inner["chain"][1]["component"], "App");
    assert_eq!(inner["chain"][1]["file"], root.to_str().unwrap());
    for (id, line) in [("slot-counter", 9), ("outside", 10), ("a", 12), ("b", 12)] {
        let node = at(&runner, &map, id);
        assert_eq!(node["file"], counter.to_str().unwrap());
        assert_eq!(node["line"], 9);
        assert_eq!(node["component"], "Counter");
        assert_eq!(node["chain"].as_array().unwrap().len(), 1);
        assert_eq!(node["chain"][0]["file"], root.to_str().unwrap());
        assert_eq!(node["chain"][0]["line"], line);
    }
    assert_eq!(at(&runner, &map, "a"), at(&runner, &map, "b"));
    let lifted: Vec<_> = map["slots"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(name, _)| name.starts_with("n#"))
        .collect();
    assert_eq!(lifted.len(), 3);
    for (_, location) in lifted {
        assert_eq!(location["file"], counter.to_str().unwrap());
        assert_eq!(location["line"], 4);
        assert_eq!(location["component"], "Counter");
    }
    for location in map["actions"].as_object().unwrap().values() {
        assert_eq!(location["file"], counter.to_str().unwrap());
        assert_eq!(location["line"], 6);
    }
}

#[test]
fn style_origins_follow_the_winning_expanded_row() {
    let app = App::new("styles");
    app.write(
        "app.contract",
        r#"style Spaced
  padding=4 padding-left=9 gap=3
component App
  view
    row class=Spaced padding-left=12 flex-direction="column" testId="styled"
      text "hello"
"#,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = at(&runner, &map, "styled")["bindings"].as_array().unwrap();
    let origin = |name| {
        rows.iter().find(|row| row["row"] == name).unwrap()["origin"]
            .as_str()
            .unwrap()
    };
    assert_eq!(origin("padding_left"), "own");
    assert_eq!(origin("padding_right"), "class:Spaced");
    assert_eq!(origin("row_gap"), "class:Spaced");
    assert_eq!(origin("flex_direction"), "own");
    assert_eq!(origin("display"), "tag");
    let unique: std::collections::BTreeSet<_> =
        rows.iter().map(|r| r["row"].as_str().unwrap()).collect();
    assert_eq!(rows.len(), unique.len());
}

#[test]
fn router_slot_and_lifted_declarations_keep_aligned_locations() {
    let app = App::new("router");
    let path = app.write(
        "app.contract",
        r#"routes nav
  home "/"
component App
  state count = 0
  derive next = count + 1
  view
    Child()
component Child
  state hot = false
  view
    text hot ? "on" : "off"
"#,
    );
    let (plan, map) = app.compile();
    assert_eq!(map["slots"]["nav"]["line"], 1);
    assert_eq!(map["slots"]["count"]["line"], 4);
    assert_eq!(map["slots"]["hot#1"]["line"], 9);
    assert_eq!(map["slots"]["hot#1"]["component"], "Child");
    assert_eq!(map["derives"]["next"]["file"], path.to_str().unwrap());
    assert_eq!(map["derives"]["next"]["line"], 5);
    assert_eq!(map["slots"].as_object().unwrap().len(), plan.slots.len());
}

#[test]
fn resource_locations_include_the_declared_placeholder_owner() {
    let app = App::new("resources");
    let path = app.write("app.contract", "component App\n  resource rows = rows() as shape string else fallback()\n  view\n    text rows\n");
    let (plan, map) = app.compile();
    assert_eq!(
        map["resources"].as_object().unwrap().len(),
        plan.resources.len()
    );
    for name in ["rows", "rows#else"] {
        assert_eq!(map["resources"][name]["file"], path.to_str().unwrap());
        assert_eq!(map["resources"][name]["line"], 2);
        assert_eq!(map["resources"][name]["component"], "App");
    }
}

#[test]
fn source_snapshot_and_final_baked_bytes_determine_the_map() {
    let app = App::new("snapshot");
    let path = app.write("app.contract", "invalid on disk\n");
    let source = r#"component App
  resource entries = entries() as shape string[]
  view
    text "snapshot"
"#;
    let (plan, map) = contract::compile_path_source_mapped(&path, source).unwrap();
    let unbaked = plan.encode();
    let baked = contract::bake(plan, NoData).unwrap();
    assert_ne!(unbaked, baked.encode());
    let json: Json = serde_json::from_str(&map.json(&baked.encode())).unwrap();
    assert_eq!(json["digest"], contract::plan_digest(&baked.encode()));
    assert_eq!(json["nodes"][0]["line"], 4);
    assert_eq!(
        contract::plan_digest(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn map_cli_writes_a_matching_pair_and_preserves_it_on_compile_failure() {
    let app = App::new("cli");
    app.write("app.contract", "component App\n  view\n    text \"hi\"\n");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_contract"))
            .arg("build")
            .args(args)
            .current_dir(&app.0)
            .output()
            .unwrap()
    };
    let result = run(&["--map", "app.contract", "--json", "-o", "out.plan"]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(result.stdout, b"[]\n");
    assert!(result.stderr.is_empty());
    let plan = std::fs::read(app.0.join("out.plan")).unwrap();
    let map = std::fs::read(app.0.join("out.plan.map.json")).unwrap();
    let json: Json = serde_json::from_slice(&map).unwrap();
    assert_eq!(json["digest"], contract::plan_digest(&plan));
    app.write("app.contract", "component App\n  view\n    text missing\n");
    assert_eq!(
        run(&["app.contract", "--map", "-o", "out.plan"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(std::fs::read(app.0.join("out.plan")).unwrap(), plan);
    assert_eq!(std::fs::read(app.0.join("out.plan.map.json")).unwrap(), map);
    for args in [
        vec!["app.contract", "--map"],
        vec!["app.contract", "--map", "--map", "-o", "out.plan"],
    ] {
        assert_eq!(run(&args).status.code(), Some(2));
    }
    assert!(std::fs::read_dir(&app.0).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
}

#[test]
fn measured_bake_refusal_names_the_imported_node_and_its_caller() {
    let app = App::new("bake");
    let root = app.write(
        "app.contract",
        "use Child from \"./child.contract\"\ncomponent App\n  view\n    Child()\n",
    );
    let child = app.write("child.contract", "component Child\n  state width = 0\n  view\n    button width=width height=0\n      text \"hidden\"\n");
    let (plan, map) = contract::compile_path_mapped(&root).unwrap();
    let error = map.bake_error(&contract::bake(plan, NoData).unwrap_err());
    assert_eq!(error.id, "bake-zero-size");
    assert_eq!(error.file.as_deref(), Some(child.as_path()));
    assert_eq!(error.span.line, 4);
    assert_eq!(error.related.len(), 1);
    assert_eq!(error.related[0].file, Some(root));
    assert_eq!(error.related[0].span.line, 4);
    assert!(error.to_string().contains("child.contract:4:"));
    let json: Json = serde_json::from_str(&error.to_json()).unwrap();
    assert_eq!(json["line"], 4);
    let no_node = map.bake_error(&contract::BakeError::Lint {
        id: "bake-layout",
        message: "test".into(),
        site: None,
    });
    assert_eq!(no_node.file, None);
    assert_eq!(no_node.span, contract_syntax::Span::default());
}

#[test]
fn map_write_failure_is_named_and_does_not_replace_the_previous_plan() {
    let app = App::new("output-error");
    app.write("app.contract", "component App\n  view\n    text \"new\"\n");
    app.write("out.plan", "previous bytes");
    std::fs::create_dir(app.0.join("out.plan.map.json")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["build", "app.contract", "--json", "--map", "-o", "out.plan"])
        .current_dir(&app.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let errors: Json = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(errors[0]["id"], "contract-output-write");
    assert_eq!(errors[0]["file"], "out.plan.map.json");
    assert_eq!(
        std::fs::read(app.0.join("out.plan")).unwrap(),
        b"previous bytes"
    );
    assert!(std::fs::read_dir(&app.0).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
}

#[test]
fn the_map_says_each_source_and_the_digest_of_its_text_as_compiled() {
    use sha2::{Digest, Sha256};
    let hex = |text: &str| -> String {
        Sha256::digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    let app = App::new("digests");
    let root_text =
        "use Child from \"./child.contract\"\ncomponent App\n  view\n    column\n      Child()\n";
    let child_text = "component Child\n  view\n    text \"child\" testId=\"child\"\n";
    let root = app.write("app.contract", root_text);
    let child = app.write("child.contract", child_text);
    let design_text = "tamber digests\n\ntheme\n  ink = #101010\n";
    let design = app.write("app.tamber", design_text);
    let (_, json) = app.compile();
    let sources: Vec<(String, String)> = json["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["file"].as_str().unwrap().to_string(),
                s["sha256"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let of = |path: &std::path::Path| {
        sources
            .iter()
            .find(|(f, _)| std::path::Path::new(f) == path)
            .map(|(_, d)| d.clone())
    };
    assert_eq!(of(&root), Some(hex(root_text)));
    assert_eq!(of(&child), Some(hex(child_text)));
    assert_eq!(of(&design), Some(hex(design_text)));
}

/// @ref LLP 1121 D8 — an `:active` row lowers to the press paint, and the
/// map names its `:active` line (the class's or the node's), not the
/// lowered row's node.
#[test]
fn an_active_row_keeps_its_own_location() {
    let app = App::new("active");
    app.write(
        "app.contract",
        r##"style Card
  :active
    color = "#ff0000"
component App
  state n = 0
  action go
    n = n + 1
  view
    column class=Card press=go testId="card"
      :active
        background-color = "#0000ff"
        opacity = 0.5
      text "c"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = at(&runner, &map, "card")["bindings"].as_array().unwrap();
    let row = |name: &str| {
        rows.iter()
            .find(|r| r["row"] == name)
            .unwrap_or_else(|| panic!("{name}: {rows:?}"))
    };
    // Each press row is its `:active` part's alone (theme2's 02:46 and
    // 03:18 rulings): no base value, so no base `origin`, `at` or `attr`.
    for (name, line, origin) in [
        ("press_background_color", 11, "own"),
        ("press_opacity_to", 12, "own"),
        ("press_color", 3, "class:Card"),
    ] {
        let parts = row(name)["states"].as_array().expect(name);
        assert_eq!(parts.len(), 1, "{name}");
        assert_eq!(parts[0]["state"], "active", "{name}");
        assert_eq!(parts[0]["at"]["line"], line, "{name}");
        assert_eq!(parts[0]["origin"], origin, "{name}");
        for base in ["origin", "at", "attr", "state"] {
            assert!(row(name).get(base).is_none(), "{name}: {base}");
        }
    }
    assert!(row("display").get("states").is_none());
}

/// LLP 1122 queue item (1), theme2's 02:46 ruling: a `:hover` or
/// `:focus-visible` row is its row's, keyed by (row, state): beside the
/// base value's place, never in it (review2's 02:46: no collision), and
/// a border colour's four sides each say the part's `border-color` line.
#[test]
fn a_look_row_is_its_rows_part_beside_the_base() {
    let app = App::new("looks-map");
    app.write(
        "app.contract",
        r##"style Card
  background-color = "#eeeeee"
  :hover
    background-color = "#dddddd"
component App
  view
    column class=Card testId="card" opacity=0.5
      :hover
        border-color = "#111111"
      :focus-visible
        opacity = 1
      text "c"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = at(&runner, &map, "card")["bindings"].as_array().unwrap();
    let row = |name: &str| {
        rows.iter()
            .find(|r| r["row"] == name)
            .unwrap_or_else(|| panic!("{name}: {rows:?}"))
    };
    let once = |name: &str| {
        assert_eq!(
            rows.iter().filter(|r| r["row"] == name).count(),
            1,
            "{name}"
        )
    };
    // The base value at its own line, the hover's at the part's.
    let bg = row("background_color");
    once("background_color");
    assert_eq!(
        (bg["origin"].as_str(), bg["at"]["line"].as_u64()),
        (Some("class:Card"), Some(2)),
        "{bg}"
    );
    let parts = bg["states"].as_array().expect("its hover");
    assert_eq!(parts.len(), 1, "{bg}");
    assert_eq!(
        (
            parts[0]["state"].as_str(),
            parts[0]["origin"].as_str(),
            parts[0]["at"]["line"].as_u64()
        ),
        (Some("hover"), Some("class:Card"), Some(4))
    );
    let opacity = row("opacity");
    assert_eq!(
        (opacity["origin"].as_str(), opacity["attr"].as_str()),
        (Some("own"), Some("opacity"))
    );
    assert_eq!(opacity["states"][0]["state"], "focus-visible");
    assert_eq!(opacity["states"][0]["at"]["line"], 11);
    for side in ["top", "right", "bottom", "left"] {
        let name = format!("border_color_{side}");
        once(&name);
        assert_eq!(row(&name)["states"][0]["at"]["line"], 9, "{name}");
        assert_eq!(row(&name)["states"][0]["origin"], "own", "{name}");
    }
}

/// theme2's 03:18 ruling: a row only a state part writes (a `:hover`
/// background on a node with none) has no base: no `origin`, `at` or
/// `attr` of the part's borrowed, only its part in `states`.
#[test]
fn a_row_only_a_look_writes_has_no_base() {
    let app = App::new("looks-no-base");
    app.write(
        "app.contract",
        r##"component App
  view
    column testId="card"
      :hover
        background-color = "#dddddd"
      text "c"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = at(&runner, &map, "card")["bindings"].as_array().unwrap();
    let bg = rows
        .iter()
        .find(|r| r["row"] == "background_color")
        .unwrap_or_else(|| panic!("{rows:?}"));
    for base in ["origin", "at", "attr", "layout"] {
        assert!(bg.get(base).is_none(), "{base}: {bg}");
    }
    assert_eq!(bg["states"][0]["state"], "hover");
    assert_eq!(bg["states"][0]["origin"], "own");
    assert_eq!(bg["states"][0]["at"]["line"], 5);
}

/// LLP 1122 D6: a row an author wrote says where (`set-row`'s address):
/// its attribute's span and name, a shorthand's for an expansion, a class's
/// in the style's own file; a tag's row and a derived one say nowhere.
#[test]
fn a_written_row_says_its_attribute() {
    let app = App::new("written");
    let root = app.write(
        "app.contract",
        r##"use Card from "./card.contract"
component App
  view
    column padding="4px 8px" color="#111111" border-color="#222222" testId="col"
      Card()
"##,
    );
    let card = app.write(
        "card.contract",
        r##"style Raised
  background-color="#ffffff"

component Card
  view
    row class=Raised font-size=14 border="1px solid #333333" testId="card"
      text "c"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = |id: &str| {
        at(&runner, &map, id)["bindings"]
            .as_array()
            .unwrap()
            .clone()
    };
    let row = |id: &str, name: &str| {
        rows(id)
            .into_iter()
            .find(|r| r["row"] == name)
            .unwrap_or_else(|| panic!("{name}: {:?}", rows(id)))
    };
    let file = |p: &PathBuf| p.canonicalize().unwrap().to_string_lossy().to_string();
    let (top, color) = (row("col", "padding_top"), row("col", "text_color"));
    assert_eq!(top["attr"], "padding", "{top}");
    assert_eq!(top["at"]["line"], 4);
    assert_eq!(
        row("col", "padding_left")["at"],
        top["at"],
        "one shorthand, one place"
    );
    assert_eq!(color["attr"], "color");
    assert_eq!(color["at"]["line"], 4);
    // A layout row says so (D14: `set-row` refuses it); a paint row doesn't.
    assert_eq!(top["layout"], true, "{top}");
    assert!(color.get("layout").is_none(), "{color}");
    // A paint shorthand's longhands carry none; a shorthand writing a layout
    // row marks all of its longhands, its colour's too (set-row edits it whole).
    let paint = row("col", "border_color_top");
    assert_eq!(paint["attr"], "border-color", "{paint}");
    assert!(paint.get("layout").is_none(), "{paint}");
    let mixed = row("card", "border_color_top");
    assert_eq!(mixed["attr"], "border", "{mixed}");
    assert_eq!(mixed["layout"], true, "{mixed}");
    let ground = row("card", "background_color");
    assert_eq!(ground["origin"], "class:Raised");
    assert_eq!(ground["attr"], "background-color");
    assert_eq!(ground["at"]["line"], 2);
    assert_eq!(
        file(&PathBuf::from(ground["at"]["file"].as_str().unwrap())),
        file(&card)
    );
    assert_eq!(row("card", "font_size")["at"]["line"], 6);
    for tag in ["display", "flex_direction"] {
        let r = row("card", tag);
        assert!(r.get("at").is_none() && r.get("attr").is_none(), "{r}");
    }
    let _ = root;
}

/// Review round 1: a row the compiler composed or renamed, or a
/// conditional class's, says nowhere: an animation's longhands composed,
/// a symbol's tint from its `color`, a `class=(c ? A : B)` row; a folded
/// `:disabled` or `:checked` row says its base's attribute (a shorthand's
/// for a side), its parts beside it (LLP 1122's queue item (2)).
#[test]
fn a_composed_or_renamed_row_says_nowhere() {
    let app = App::new("composed");
    app.write(
        "app.contract",
        r##"style A
  background-color="#ffffff"

style B
  background-color="#eeeeee"

keyframes pulse
  from opacity=0
  to opacity=1

component App
  state flag = true
  view
    column class=(flag ? A : B) testId="col"
      box animation-name="pulse" animation-duration="1s" testId="anim"
      image "symbol:star" color="#ff0000" testId="sym"
      button "Go" border-color="#111111" disabled=flag testId="btn"
        :disabled
          border-color = "#999999"
      box background-color="#ffffff" aria-checked=flag role="checkbox" testId="chk"
        :checked
          background-color = "#00ff00"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = |id: &str| {
        at(&runner, &map, id)["bindings"]
            .as_array()
            .unwrap()
            .clone()
    };
    // A folded `:checked` row (LLP 1122's queue item (2)): its base's
    // attribute is its base place, its part in `states`, never one for the
    // other.
    let chk = rows("chk");
    let ground = chk.iter().find(|r| r["row"] == "background_color").unwrap();
    assert_eq!(
        (ground["attr"].as_str(), ground["at"]["line"].as_u64()),
        (Some("background-color"), Some(20)),
        "{ground}"
    );
    assert_eq!(ground["states"][0]["state"], "checked", "{ground}");
    assert_eq!(ground["states"][0]["at"]["line"], 22, "{ground}");
    let bad: Vec<_> = ["col", "anim", "sym", "btn", "chk"]
        .iter()
        .flat_map(|id| rows(id).into_iter().map(move |r| (id.to_string(), r)))
        .filter(|(_, r)| {
            r.get("attr").is_some_and(|a| {
                let name = a.as_str().unwrap().replace('-', "_");
                // An own row's attribute says its own name or a shorthand's.
                let row = r["row"].as_str().unwrap();
                r["origin"].as_str().unwrap().contains('/')
                    || (name != row
                        && !row.starts_with(&name)
                        && (name, row) != ("color".into(), "text_color"))
            })
        })
        .collect();
    assert!(bad.is_empty(), "{bad:?}");
    assert!(rows("col")
        .iter()
        .all(|r| r.get("at").is_none() || r["origin"] != "class:A/B"));
}

/// Review round 2: a row's origin is its place's, whatever a rewrite does
/// to its position: a grouped list's inserted rows don't move a
/// conditional class's row into the node's own; a class's composed
/// animation doesn't take a node's own attribute's place. A grouped
/// field's `flex=1` made `flex-grow` says nowhere (recorded).
#[test]
fn a_rewrite_moves_no_row_across_its_origin() {
    let app = App::new("rewrites");
    app.write(
        "app.contract",
        r##"style A
  background-color="#ffffff"

style B
  background-color="#eeeeee"

style Pulse
  animation-name="pulse"
  animation-duration="1s"

keyframes pulse
  from opacity=0
  to opacity=1

component App
  state flag = true
  state words = "a"
  action write(value: string)
    words = value
  action go
    flag = !flag
  view
    column
      list appearance="auto" class=(flag ? A : B) testId="list"
        section
          input value=words change=write flex=1 aria-label="W" testId="field"
      box class=Pulse color="#111111" press=go testId="pulse"
        :active
          color = "#222222"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = |id: &str| {
        at(&runner, &map, id)["bindings"]
            .as_array()
            .unwrap()
            .clone()
    };
    let ground = rows("list")
        .into_iter()
        .find(|r| r["row"] == "background_color")
        .unwrap();
    assert!(
        ground.get("at").is_none(),
        "the conditional class's row says nowhere: {ground}"
    );
    let own = rows("pulse")
        .into_iter()
        .find(|r| r["row"] == "text_color")
        .unwrap();
    assert_eq!(own["origin"], "own", "{own}");
    assert_eq!(own["attr"], "color", "{own}");
    for r in rows("field")
        .iter()
        .filter(|r| r["row"].as_str().unwrap().starts_with("flex"))
    {
        assert!(r.get("attr").is_none() || r["attr"] == "flex", "{r}");
    }
}

/// A look row's origin is its part's by its span, as a base row's is: a
/// class whose animation longhands are composed into one row moves the
/// node's own rows across the class's count, never their spans, so the
/// node's own `:hover` row stays its own (Astra, round 1 of LLP 1122's
/// queue item (1)).
#[test]
fn a_look_row_beside_a_composed_animation_is_still_its_nodes() {
    let app = App::new("looks-animated");
    app.write(
        "app.contract",
        r##"keyframes fade
  from opacity=0
  to opacity=1

style Pulse
  animation-name="fade"
  animation-duration="1s"
  :hover
    background-color="#dddddd"
component App
  view
    column class=Pulse testId="card"
      :hover
        opacity=0.5
      text "c"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = at(&runner, &map, "card")["bindings"].as_array().unwrap();
    let part = |name: &str| {
        rows.iter()
            .find(|r| r["row"] == name)
            .map(|r| r["states"][0].clone())
            .unwrap_or_else(|| panic!("{name}: {rows:?}"))
    };
    assert_eq!(
        (
            part("opacity")["origin"].as_str(),
            part("opacity")["at"]["line"].as_u64()
        ),
        (Some("own"), Some(14))
    );
    assert_eq!(
        (
            part("background_color")["origin"].as_str(),
            part("background_color")["at"]["line"].as_u64()
        ),
        (Some("class:Pulse"), Some(9))
    );
}

/// A conditional class's state part is either style's, as its base row
/// is: its `states` entry names its state and origin and says nowhere
/// (Grok, round 1 of LLP 1122's queue item (1)).
#[test]
fn a_conditional_class_part_says_nowhere() {
    let app = App::new("looks-conditional");
    app.write(
        "app.contract",
        r##"style A
  :hover
    background-color="#dddddd"

style B
  :hover
    background-color="#dddddd"

component App
  state flag = true
  view
    column class=(flag ? A : B) testId="col"
      text "c"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = at(&runner, &map, "col")["bindings"].as_array().unwrap();
    let bg = rows
        .iter()
        .find(|r| r["row"] == "background_color")
        .unwrap_or_else(|| panic!("{rows:?}"));
    assert_eq!(bg["states"][0]["state"], "hover", "{bg}");
    assert_eq!(bg["states"][0]["origin"], "class:A/B", "{bg}");
    assert!(bg["states"][0].get("at").is_none(), "{bg}");
}

/// LLP 1122's queue item (2): a row a `:disabled` or `:checked` part folds
/// over its base says the base's attribute as its base place, where an
/// author wrote it, and each part folded over it in `states`: a param's
/// base, both parts on one row, a class's base and part; none for a base
/// the node takes unwritten (a link's colour) or a conditional class's.
#[test]
fn a_folded_row_says_its_base_and_its_parts() {
    let app = App::new("folds-map");
    app.write(
        "app.contract",
        r##"component App
  state flag = true
  view
    column
      Pill(off=flag)
      box background-color="#ffffff" aria-checked=flag disabled=flag role="checkbox" testId="both"
        :checked
          background-color = "#00ff00"
        :disabled
          background-color = "#999999"
      box class=Card aria-checked=flag role="checkbox" testId="card"
      link href="/x" disabled=flag testId="lnk"
        :disabled
          color = "#999999"
        text "x"
      box class=(flag ? A : B) aria-checked=flag role="checkbox" testId="cond"
        :checked
          background-color = "#00ff00"

style Card
  background-color="#ffffff"
  :checked
    background-color="#00ff00"

style A
  background-color="#ffffff"

style B
  background-color="#ffffff"

component Pill
  params
    fill: color = "#0a7aff"
  props
    off: bool
  view
    button appearance="none" disabled=off background-color="param.fill" testId="pill"
      :disabled
        background-color = "#999999"
      text "Go"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let row = |id: &str, name: &str| {
        let rows = at(&runner, &map, id)["bindings"]
            .as_array()
            .unwrap()
            .clone();
        rows.into_iter()
            .find(|r| r["row"] == name)
            .unwrap_or_else(|| panic!("{id} {name}"))
    };
    let parts = |r: &serde_json::Value| {
        r["states"].as_array().map_or_else(Vec::new, |p| {
            p.iter()
                .map(|p| {
                    (
                        p["state"].as_str().unwrap().to_string(),
                        p["origin"].as_str().unwrap().to_string(),
                        p["at"]["line"].as_u64(),
                    )
                })
                .collect()
        })
    };
    let base = |r: &serde_json::Value| {
        (
            r["origin"].as_str().map(str::to_string),
            r["attr"].as_str().map(str::to_string),
            r["at"]["line"].as_u64(),
        )
    };
    // A param's base: the attribute that names it, never its part's place.
    let pill = row("pill", "background_color");
    assert_eq!(
        base(&pill),
        (
            Some("own".into()),
            Some("background-color".into()),
            Some(37)
        )
    );
    assert_eq!(parts(&pill), [("disabled".into(), "own".into(), Some(39))]);
    // Both parts on one row, `:checked` under `:disabled`.
    let both = row("both", "background_color");
    assert_eq!(base(&both).2, Some(6));
    assert_eq!(
        parts(&both),
        [
            ("checked".into(), "own".into(), Some(8)),
            ("disabled".into(), "own".into(), Some(10))
        ]
    );
    // A class's base and part.
    let card = row("card", "background_color");
    assert_eq!(
        base(&card),
        (
            Some("class:Card".into()),
            Some("background-color".into()),
            Some(21)
        )
    );
    assert_eq!(
        parts(&card),
        [("checked".into(), "class:Card".into(), Some(23))]
    );
    // A base the node takes unwritten (a link's colour): no base fields.
    let lnk = row("lnk", "text_color");
    assert_eq!(base(&lnk), (None, None, None), "{lnk}");
    assert_eq!(parts(&lnk), [("disabled".into(), "own".into(), Some(14))]);
    // A conditional class's base is either style's: no place.
    let cond = row("cond", "background_color");
    assert_eq!(
        base(&cond),
        (Some("class:A/B".into()), None, None),
        "{cond}"
    );
}

/// Two parts of one state on one row each stay in `states`: the node's
/// `:disabled border-top-color` with a `none` arm leaves its class's
/// `:disabled border-color` showing on that side when it is `none`
/// (Astra and Grok, round 2 of LLP 1122's queue item (2)).
#[test]
fn a_folded_row_keeps_each_part_of_a_state() {
    let app = App::new("folds-two-parts");
    app.write(
        "app.contract",
        r##"component App
  state flag = true
  view
    box class=Card disabled=flag testId="card"
      :disabled
        border-top-color = (flag ? "#999999" : none)
      text "c"

style Card
  border-color="#ffffff"
  :disabled
    border-color="#111111"
"##,
    );
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let rows = at(&runner, &map, "card")["bindings"]
        .as_array()
        .unwrap()
        .clone();
    let parts = |name: &str| {
        let r = rows
            .iter()
            .find(|r| r["row"] == name)
            .unwrap_or_else(|| panic!("{name}: {rows:?}"));
        r["states"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["state"].as_str().unwrap().to_string(),
                    p["origin"].as_str().unwrap().to_string(),
                    p["at"]["line"].as_u64(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        parts("border_color_top"),
        [
            ("disabled".to_string(), "class:Card".to_string(), Some(12)),
            ("disabled".to_string(), "own".to_string(), Some(6))
        ]
    );
    assert_eq!(
        parts("border_color_right"),
        [("disabled".to_string(), "class:Card".to_string(), Some(12))]
    );
}

/// Each source says its home, as its components' identities name it
/// (LLP 1121 D15), so a reader names a param by its full name from the
/// file it reads it in (LLP 1122's queue item (2)).
#[test]
fn each_source_says_its_home() {
    let app = App::new("homes");
    app.write(
        "app.contract",
        "component App\n  view\n    Card()\n\ncomponent Card\n  params\n    fill: color = \"#0a7aff\"\n  view\n    box background-color=\"param.fill\" testId=\"card\"\n",
    );
    let (_, map) = app.compile();
    let sources = map["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 1, "{sources:?}");
    assert_eq!(sources[0]["home"], "app.contract", "{sources:?}");
}

/// A node's `walk` into the map's `walks` (stage 15 D3): its chain as the page's walk up its own tree takes it, where that differs
/// from the call-site chain. A slot's host is a step (`slot: true`, named by its own call); a component that
/// renders only another is none; a node with no fill or element-less component on its way has no `walk`.
#[test]
fn steps_follow_the_pages_walk_a_slots_host_one_a_render_only_component_none() {
    let app = App::new("steps");
    let source = r#"component App
  view
    column testId="root"
      Post()
      Panel()
        text "panel fill" testId="panel-fill"
      Outer()
        Card()
          text "wrapped" testId="wrapped"
      Plain()

component Frame
  slot
  view
    column testId="frame"
      children

component Panel
  slot
  view
    Frame()
      children

component Card
  slot
  view
    column testId="card"
      children

component Glyph
  view
    text "g" testId="glyph"

component Controls
  view
    Glyph()

component Outer
  slot
  view
    column testId="outer"
      children

component Post
  view
    column testId="post"
      Card()
        text "body" testId="body"
        Controls()

component Plain
  view
    column testId="plain"
      text "leaf" testId="leaf"
"#;
    app.write("app.contract", source);
    let line = |needle: &str| source.lines().position(|l| l.contains(needle)).unwrap() as u64 + 1;
    let (plan, map) = app.compile();
    let runner = boot(plan);
    let steps = |id: &str| -> Vec<(String, bool, u64)> {
        let node = at(&runner, &map, id);
        node.get("walk")
            .map(|w| &map["walks"][w.as_u64().unwrap() as usize])
            .and_then(|w| w.as_array())
            .map(|s| {
                s.iter()
                    .map(|i| &map["walk_steps"][i.as_u64().unwrap() as usize])
                    .map(|e| {
                        let slot = e["slot"] == true;
                        assert!(e.get("slot").is_none() || slot, "{e}");
                        (
                            e["component"].as_str().unwrap().to_owned(),
                            slot,
                            e["line"].as_u64().unwrap(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    // A caller's fill in a host's slot: the host, then on out by the calls.
    let post = line("component Post");
    let card_in_post = source
        .lines()
        .enumerate()
        .skip(post as usize)
        .find(|(_, l)| l.contains("Card()"))
        .unwrap()
        .0 as u64
        + 1;
    assert_eq!(
        steps("body"),
        [
            ("Card".to_owned(), true, card_in_post),
            ("App".to_owned(), false, line("      Post()"))
        ]
    );
    // The like row: Controls holds no view of its own, so no step; the host, its caller, the app.
    assert_eq!(
        steps("glyph")
            .iter()
            .map(|(c, s, _)| (c.as_str(), *s))
            .collect::<Vec<_>>(),
        [("Card", true), ("Post", false), ("App", false)]
    );
    // Panel renders only Frame: Frame's box is the one step.
    assert_eq!(
        steps("panel-fill")
            .iter()
            .map(|(c, s, _)| (c.as_str(), *s))
            .collect::<Vec<_>>(),
        [("Frame", true)]
    );
    // A host wrapped by another: the nearer, then the wrapper.
    assert_eq!(
        steps("wrapped")
            .iter()
            .map(|(c, s, _)| (c.as_str(), *s))
            .collect::<Vec<_>>(),
        [("Card", true), ("Outer", true)]
    );
    // No fill and no element-less component on the way: the chain is the walk, and no `walk` is written.
    for id in ["leaf", "plain", "post", "outer", "root"] {
        assert!(
            at(&runner, &map, id).get("walk").is_none(),
            "{id}: {}",
            at(&runner, &map, id)
        );
    }
}
