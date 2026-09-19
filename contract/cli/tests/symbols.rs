//! LLP 1035.005 D2: the action-prop arity check names both sides, `build
//! --json` prints a diagnostic with its end column and related spans, and
//! `symbols` finds definitions and references across a `use`.

use std::path::Path;
use std::process::Command;

#[test]
fn navigation_action_props_may_take_or_ignore_the_location() {
    let source = "component App\n  state n = 0\n  action home writes n\n    n = n + 1\n  view\n    Shell(go=home)\ncomponent Shell\n  props\n    go: action\n  view\n    main navigate=go navigationKey=\"home\" navigationBack=\"back\"\n      text \"Home\"\n";
    contract::compile(source).unwrap();
    contract::compile(&source.replace("action home writes", "action home(url: string) writes"))
        .unwrap();
    let too_many = source.replace(
        "action home writes",
        "action home(url: string, extra: string) writes",
    );
    assert_eq!(
        contract::compile(&too_many).unwrap_err().id,
        "lower-handler-arity"
    );
}

#[test]
fn named_world_arguments_refer_to_values_not_field_labels() {
    let dir = std::env::temp_dir().join(format!("exact-named-symbols-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("app.contract");
    let src = "component App\n  state paused = false\n  state again = false\n  view\n    canvas surface=world(paused=not paused, restart=again)\n";
    std::fs::write(&file, src).unwrap();
    let (defs, refs) = contract::symbols::symbols(&file).unwrap();
    let bindings: Vec<_> = refs.iter().filter(|r| r.span.line == 5).collect();
    assert_eq!(bindings.len(), 2);
    let line = src.lines().nth(4).unwrap();
    for (r, name) in bindings.iter().zip(["paused", "again"]) {
        assert_eq!(r.name, name);
        assert_eq!(defs[r.to].name, name);
        assert_eq!(defs[r.to].kind, "state");
        assert_eq!(
            &line[r.span.col as usize - 1..r.span.end_col as usize - 1],
            name
        );
    }
    assert_eq!(
        bindings[0].span.col as usize - 1,
        line.rfind("paused").unwrap()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

const MISMATCH: &str = "shape Item\n  id: string\ncomponent App\n  state chosen = \"\"\n  action choose(id: string, why: string) writes chosen\n    chosen = id\n  resource items = items() as shape list<Item>\n  view\n    column\n      each item in items key=item.id\n        Row(item=item, pick=choose)\ncomponent Row\n  props\n    item: Item\n    pick: action\n  view\n    button press=pick(item.id)\n      text item.id\n";

#[test]
fn an_action_props_arity_is_inferred_from_its_invocations_and_checked_at_the_binding() {
    let e = contract::compile(MISMATCH).unwrap_err();
    assert_eq!(e.id, "analyze-action-arity");
    // The invocation that fixed the arity, then the declaration and the binding.
    assert_eq!((e.span.line, e.span.col, e.span.end_col), (17, 12, 17));
    assert!(
        e.message
            .contains("`pick` of `Row` is invoked with 1 argument(s)"),
        "{e}"
    );
    assert!(e.message.contains("`choose`, which takes 2"), "{e}");
    let related: Vec<(u32, u32, &str)> = e
        .related
        .iter()
        .map(|r| (r.span.line, r.span.col, r.note.as_str()))
        .collect();
    assert_eq!(
        related,
        [
            (15, 5, "`pick` declared here"),
            (11, 24, "bound to `choose` here")
        ]
    );

    // A curried binding counts the arguments it binds; a matching one passes.
    let curried = MISMATCH.replace("pick=choose)", "pick=choose(\"why\"))");
    contract::compile(&curried).unwrap();
    // A binding that saturates the action leaves the inlined handler with
    // more arguments than parameters, which the type pass refuses first.
    let too_many = MISMATCH.replace("pick=choose)", "pick=choose(\"a\", \"b\"))");
    assert_eq!(contract::compile(&too_many).unwrap_err().id, "type-arity");

    // A prop passed on to a child takes the child's arity.
    let passed_on = "component App\n  state n = 0\n  action bump(by: number) writes n\n    n = n + by\n  action reset writes n\n    n = 0\n  view\n    Outer(go=bump)\n    Outer(go=reset)\ncomponent Outer\n  props\n    go: action\n  view\n    Inner(tap=go)\ncomponent Inner\n  props\n    tap: action\n  view\n    button press=tap(1)\n      text \"x\"\n";
    let e = contract::compile(passed_on).unwrap_err();
    assert_eq!(e.id, "analyze-action-arity");
    assert!(e.message.contains("`reset`, which takes 0"), "{e}");
    assert_eq!(e.related[1].span.line, 9);

    // The same prop invoked two ways is refused inside the component itself.
    let inconsistent = "component App\n  state n = 0\n  action go(by: number) writes n\n    n = by\n  view\n    Row(pick=go)\ncomponent Row\n  props\n    pick: action\n  view\n    button press=pick(1)\n      text \"a\"\n    button press=pick\n      text \"b\"\n";
    let e = contract::compile(inconsistent).unwrap_err();
    assert_eq!(e.id, "analyze-action-arity");
    assert_eq!(e.span.line, 13);
    assert_eq!(e.related[1].span.line, 11);

    // A payload counts: `change=` supplies the new value.
    let payload = "component App\n  state q = \"\"\n  action search(text: string) writes q\n    q = text\n  view\n    Field(edit=search)\ncomponent Field\n  props\n    edit: action\n  view\n    input change=edit value=\"\"\n";
    contract::compile(payload).unwrap();
}

#[test]
fn build_json_prints_the_diagnostic_with_its_end_column_and_related_spans() {
    let dir = std::env::temp_dir().join(format!("exact-json-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("app.contract");
    std::fs::write(&file, MISMATCH).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["build", "--json"])
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(v["id"], "analyze-action-arity");
    assert_eq!(v["file"], file.display().to_string());
    assert_eq!(
        (v["line"].as_u64(), v["col"].as_u64(), v["end_col"].as_u64()),
        (Some(17), Some(12), Some(17))
    );
    let related = v["related"].as_array().unwrap();
    assert_eq!(related.len(), 2);
    assert_eq!(related[0]["line"], 15);
    assert_eq!(related[0]["note"], "`pick` declared here");
    assert_eq!(related[1]["col"], 24);
    assert_eq!(related[1]["end_col"], 28);
    // A syntax error's span carries its end column too.
    std::fs::write(&file, "component A\n  view\n    text \"a\" size=1\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["build", "--json"])
        .arg(&file)
        .output()
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert_eq!(v["id"], "lower-unknown-attr");
    assert_eq!(
        (v["col"].as_u64(), v["end_col"].as_u64()),
        (Some(14), Some(18))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn symbols_lists_definitions_and_references_across_a_use() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/use/app.contract");
    let (defs, refs) = contract::symbols::symbols(&path).unwrap();
    let find = |kind: &str, name: &str| {
        defs.iter()
            .position(|d| d.kind == kind && d.name == name)
            .unwrap_or_else(|| panic!("no {kind} `{name}`"))
    };
    let row = find("component", "StationRow");
    assert!(defs[row].file.ends_with("corpus/use/row.contract"));
    let press = find("prop", "press");
    assert_eq!(defs[press].component.as_deref(), Some("StationRow"));
    assert_eq!(defs[find("source", "stations")].span.line, 8);
    // `use StationRow`, the use site, and its `press=` argument all resolve.
    let to_row: Vec<u32> = refs
        .iter()
        .filter(|r| r.to == row)
        .map(|r| r.span.line)
        .collect();
    assert_eq!(to_row, [4, 15]);
    assert!(refs
        .iter()
        .any(|r| r.to == press && r.span.line == 15 && r.file.ends_with("app.contract")));
    assert!(refs
        .iter()
        .any(|r| r.to == find("action", "pick") && r.span.line == 15));
    assert!(refs
        .iter()
        .any(|r| r.to == find("state", "picked") && r.span.line == 9));
    // The used file's own references point at the same definitions.
    assert!(refs
        .iter()
        .any(|r| r.to == find("shape", "Station") && r.file.ends_with("row.contract")));

    // HTML `id`s: `id=` defines, `focus("…")` and `navigationBack` refer.
    let dir = std::env::temp_dir().join(format!("exact-symbols-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("app.contract");
    std::fs::write(&file, "component App\n  state q = \"\"\n  action focusSearch\n    focus(\"search\")\n  action search(text) writes q\n    q = text\n  view\n    main navigationBack=\"back\"\n      button press=focusSearch id=\"back\"\n        text \"Back\"\n      input id=\"search\" value=q change=search\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("symbols")
        .arg(&file)
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    let defs = v["definitions"].as_array().unwrap();
    let ids: Vec<(&str, u64, u64)> = defs
        .iter()
        .filter(|d| d["kind"] == "id")
        .map(|d| {
            (
                d["name"].as_str().unwrap(),
                d["line"].as_u64().unwrap(),
                d["col"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(ids, [("back", 9, 35), ("search", 11, 16)]);
    let refs = v["references"].as_array().unwrap();
    let id_refs: Vec<(&str, u64)> = refs
        .iter()
        .filter(|r| r["kind"] == "id")
        .map(|r| (r["name"].as_str().unwrap(), r["line"].as_u64().unwrap()))
        .collect();
    assert_eq!(id_refs, [("search", 4), ("back", 8)]);
    let _ = std::fs::remove_dir_all(&dir);
}
