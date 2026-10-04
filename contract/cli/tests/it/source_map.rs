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
  resource entries = entries() as shape list<string>
  view
    column testId="root"
      Panel()
        text "from caller" testId="slot"
        Counter(label="slot-counter")
      Counter(label="outside")
      each entry in entries key=entry
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
fn source_snapshot_and_final_baked_bytes_determine_the_map() {
    let app = App::new("snapshot");
    let path = app.write("app.contract", "invalid on disk\n");
    let source = r#"component App
  resource entries = entries() as shape list<string>
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
