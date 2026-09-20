//! Navigation resolves actual compiler namespaces and original identifier ranges.

use serde_json::Value;
use std::{path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Fixture {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("exact-symbols-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
    fn query(&self, source: &str) -> Value {
        serde_json::from_str(&contract::symbols_json(&self.write("app.contract", source)).unwrap())
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn defs(graph: &Value) -> &[Value] {
    graph["definitions"].as_array().unwrap()
}
fn refs(graph: &Value) -> &[Value] {
    graph["references"].as_array().unwrap()
}
fn definition<'a>(graph: &'a Value, kind: &str, name: &str, owner: Option<&str>) -> &'a Value {
    defs(graph)
        .iter()
        .find(|d| d["kind"] == kind && d["name"] == name && d["owner"].as_str() == owner)
        .unwrap_or_else(|| panic!("missing {kind} {name}: {graph}"))
}
fn target<'a>(graph: &'a Value, reference: &Value) -> &'a Value {
    &defs(graph)[reference["to"].as_u64().unwrap() as usize]
}
fn spelling(symbol: &Value) -> String {
    let source = std::fs::read_to_string(symbol["file"].as_str().unwrap()).unwrap();
    let line = source
        .lines()
        .nth(symbol["line"].as_u64().unwrap() as usize - 1)
        .unwrap();
    line[symbol["col"].as_u64().unwrap() as usize - 1
        ..symbol["end_col"].as_u64().unwrap() as usize - 1]
        .into()
}
fn at_line<'a>(graph: &'a Value, name: &str, line: usize) -> Vec<&'a Value> {
    refs(graph)
        .iter()
        .filter(|r| r["name"] == name && r["line"] == line as u64)
        .collect()
}
fn line(source: &str, text: &str) -> usize {
    source.lines().position(|l| l.contains(text)).unwrap() + 1
}

const SOURCE: &str = r##"shape Item
  id: string
  label: string
shape Other
  label: string
fn label(value: Item): string = value.label
fn otherLabel(value: Other): string = value.label
component App
  state textValue = "root"
  state choice = some("chosen")
  resource items = loadItems() as shape list<Item>
  mutation saved as shape Item
  action edit(textValue: string) writes textValue
    textValue = textValue
  action save(item: Item) writes saved
    send saved = saveItem(item)
    refresh items
    focus("entry")
  action tick writes textValue
    textValue = "tick"
  task ticker mount
    every(1000, tick)
  view
    provide accent = "#fff"
      column navigationBack="entry"
        input id="entry" testId="entry-test" value=textValue change=edit
        each item in items key=item.id
          Row(item=item, onPick=save)
        match choice
          case some(textValue)
            text textValue
          case none
            text textValue
        text `é ${match choice { case some(textValue) => textValue, case none => textValue }}`
component Row
  props
    item: Item
    onPick: action
  inject
    accent: string
  view
    button press=onPick(item) background-color=accent
      text label(item)
"##;

#[test]
fn exact_ranges_cover_declarations_sources_locals_fields_and_ids() {
    let f = Fixture::new("ranges");
    let graph = f.query(SOURCE);
    for symbol in defs(&graph).iter().chain(refs(&graph)) {
        let actual = spelling(symbol);
        let name = symbol["name"].as_str().unwrap();
        assert!(
            actual == name || actual == format!("\"{name}\""),
            "{actual:?}: {symbol}"
        );
    }
    for (kind, name) in [
        ("component", "App"),
        ("state", "textValue"),
        ("resource", "items"),
        ("mutation", "saved"),
        ("source", "loadItems"),
        ("source", "saveItem"),
        ("task", "ticker"),
        ("provide", "accent"),
        ("inject", "accent"),
        ("id", "entry"),
        ("testId", "entry-test"),
    ] {
        definition(&graph, kind, name, None);
    }
    assert_eq!(
        refs(&graph)
            .iter()
            .filter(|r| r["kind"] == "id" && r["name"] == "entry")
            .count(),
        2
    );
    let action = at_line(&graph, "tick", line(SOURCE, "every(1000"));
    assert_eq!(action.len(), 1);
    assert_eq!(target(&graph, action[0])["kind"], "action");
}

#[test]
fn parameters_and_branch_bindings_shadow_reads_but_never_assignment_targets() {
    let f = Fixture::new("shadow");
    let graph = f.query(SOURCE);
    let both = at_line(&graph, "textValue", line(SOURCE, "textValue = textValue"));
    assert_eq!(both.len(), 2);
    assert_eq!(target(&graph, both[0])["kind"], "state");
    assert_eq!(target(&graph, both[1])["kind"], "parameter");
    let local_line = line(SOURCE, "            text textValue");
    let some = at_line(&graph, "textValue", local_line);
    let none = at_line(&graph, "textValue", local_line + 2);
    assert_eq!(target(&graph, some[0])["kind"], "local");
    assert_eq!(target(&graph, none[0])["kind"], "state");
    let template = at_line(&graph, "textValue", line(SOURCE, "text `é"));
    assert_eq!(template.len(), 2);
    assert_eq!(target(&graph, template[0])["kind"], "local");
    assert_eq!(target(&graph, template[1])["kind"], "state");
}

#[test]
fn field_references_use_the_inferred_shape_and_component_arguments_name_props() {
    let f = Fixture::new("fields");
    let graph = f.query(SOURCE);
    for (function, shape) in [("fn label", "Item"), ("fn otherLabel", "Other")] {
        let reference = at_line(&graph, "label", line(SOURCE, function));
        let field = reference.iter().find(|r| r["kind"] == "field").unwrap();
        assert_eq!(target(&graph, field)["owner"], shape);
    }
    let item_refs = at_line(&graph, "item", line(SOURCE, "Row(item=item"));
    assert_eq!(item_refs.len(), 2);
    assert_eq!(target(&graph, item_refs[0])["kind"], "prop");
    assert_eq!(target(&graph, item_refs[0])["component"], "Row");
    assert_eq!(target(&graph, item_refs[1])["kind"], "local");
    let field = at_line(&graph, "id", line(SOURCE, "each item"));
    assert_eq!(target(&graph, field[0])["owner"], "Item");
}

#[test]
fn imported_definitions_are_canonical_and_imports_point_at_names() {
    let f = Fixture::new("imports");
    let row = f
        .write(
            "lib/row.contract",
            "component Row\n  props\n    label: string\n  view\n    text label\n",
        )
        .canonicalize()
        .unwrap();
    let graph = f.query(
        "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    Row(label=\"hello\")\n",
    );
    let d = definition(&graph, "component", "Row", None);
    assert_eq!(d["file"], row.to_string_lossy().as_ref());
    assert_eq!(
        (d["col"].as_u64(), d["end_col"].as_u64()),
        (Some(11), Some(14))
    );
    let references: Vec<_> = refs(&graph)
        .iter()
        .filter(|r| r["kind"] == "component" && r["name"] == "Row")
        .collect();
    assert_eq!(references.len(), 2);
    assert_eq!(references[0]["col"], 5);
    for r in references {
        assert_eq!(target(&graph, r), d);
    }
}

#[test]
fn shape_and_function_only_modules_are_queryable_without_a_fake_app() {
    let f = Fixture::new("module");
    let graph=f.query("shape Item\n  name: string\nfn title(item: Item): string = item.name\nstyle Card\n  padding=8\n");
    definition(&graph, "shape", "Item", None);
    definition(&graph, "style", "Card", None);
    assert!(refs(&graph)
        .iter()
        .any(|r| r["kind"] == "field" && r["name"] == "name"));
    assert!(defs(&graph).iter().all(|d| d["kind"] != "component"));
}

#[test]
fn duplicate_ids_report_all_authored_targets() {
    let f = Fixture::new("ids");
    let graph=f.query("component App\n  action focusEntry\n    focus(\"entry\")\n  view\n    column\n      input id=\"entry\"\n      input id=\"entry\"\n");
    let refs: Vec<_> = refs(&graph).iter().filter(|r| r["kind"] == "id").collect();
    assert_eq!(refs.len(), 2);
    assert_ne!(refs[0]["to"], refs[1]["to"]);
}

#[test]
fn navigation_uses_build_import_refusals_including_cached_name_checks_and_cycles() {
    let f = Fixture::new("refusals");
    f.write("row.contract", "component Row\n  view\n    text \"row\"\n");
    for source in [
        "use Row from \"./row.contract\"\nuse Missing from \"./row.contract\"\ncomponent App\n  view\n    Row()\n",
        "use Row from \"../row.contract\"\ncomponent App\n  view\n    Row()\n",
        "use App from \"./app.contract\"\ncomponent App\n  view\n    text \"app\"\n",
    ] {
        let root=f.write("app.contract",source);
        let build=contract::compile_path(&root).unwrap_err();
        let query=contract::symbols_json(&root).unwrap_err();
        assert_eq!(build,query);
    }
}

#[test]
fn cli_is_read_only_json_and_refuses_extra_arguments() {
    let f = Fixture::new("cli");
    let root = f.write("app.contract", SOURCE);
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("symbols")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let graph: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(defs(&graph).len() > 20);
    assert_eq!(std::fs::read_to_string(&root).unwrap(), SOURCE);
    assert_eq!(std::fs::read_dir(&f.0).unwrap().count(), 1);
    for args in [
        vec!["symbols"],
        vec!["symbols", "--unknown"],
        vec!["symbols", root.to_str().unwrap(), "extra"],
    ] {
        assert_eq!(
            Command::new(env!("CARGO_BIN_EXE_contract"))
                .args(args)
                .output()
                .unwrap()
                .status
                .code(),
            Some(2)
        );
    }
}

#[test]
fn built_in_type_names_do_not_resolve_to_unrelated_authored_shapes() {
    let f = Fixture::new("primitive-types");
    for ty in ["string", "number", "bool", "unit"] {
        let field = if ty == "string" { "number" } else { "string" };
        let source =
            format!("shape {ty}\n  field: {field}\nfn identity(value: {ty}): {ty} = value\n");
        let graph = f.query(&source);
        definition(&graph, "shape", ty, None);
        assert!(refs(&graph).iter().all(|r| r["kind"] != "shape"), "{graph}");
    }
}

#[test]
fn route_paths_and_router_slot_offsets_follow_the_type_checker() {
    let f = Fixture::new("routes");
    let graph = f.query("routes nav\n  home \"/\"\n  item \"/item/:id\"\nshape Item\n  label: string\ncomponent App\n  state selected = \"one\"\n  resource item = load() as shape Item\n  derive url = path(\"item\", selected)\n  view\n    text `${nav.tab} ${item.label} ${url}`\n");
    let route = refs(&graph)
        .iter()
        .find(|r| r["kind"] == "route" && r["name"] == "item")
        .unwrap();
    assert_eq!(target(&graph, route)["line"], 3);
    assert!(refs(&graph)
        .iter()
        .any(|r| r["kind"] == "state" && r["name"] == "nav"));
    let field = refs(&graph)
        .iter()
        .find(|r| r["kind"] == "field" && r["name"] == "label")
        .unwrap();
    assert_eq!(target(&graph, field)["owner"], "Item");
    let graph = f.query("fn path(value: string): string = value\ncomponent App\n  view\n    text path(\"ordinary string\")\n");
    assert!(refs(&graph)
        .iter()
        .any(|r| r["kind"] == "fn" && r["name"] == "path"));
    assert!(refs(&graph).iter().all(|r| r["kind"] != "route"));
}
