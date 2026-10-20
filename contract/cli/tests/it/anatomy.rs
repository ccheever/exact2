//! @ref LLP 1139 D2, D6 (stage 12 C1) — a component's anatomy: each part a
//! node of its own view, named once and declared, its groups the table's,
//! the rows a class brings it in its groups (every branch, every state
//! layer), a native control's part the `control` group alone. Every
//! component here is written through [`spell`], r6's spelling (LLP 1139 §9
//! Q7 is open): a change of spelling changes that one function.

use contract::CompileError;
use std::path::Path;

/// A component in r6's spelling: a `parts` section of `name: groups` lines
/// before its view, and `part="name"` written by the view itself.
fn spell(head: &str, parts: &[(&str, &str)], view: &str) -> String {
    let mut s = format!("component App\n{head}");
    if !parts.is_empty() {
        s += "  parts\n";
        for (name, groups) in parts {
            s += &format!("    {name}: {groups}\n");
        }
    }
    s + "  view\n" + view
}

fn errors(src: &str) -> Vec<CompileError> {
    match contract::compile_path_source_all(Path::new("app.contract"), src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all,
    }
}

/// The refusals of `src` with `id`, each `(line, message)`.
fn with(src: &str, id: &str) -> Vec<(u32, String)> {
    errors(src)
        .into_iter()
        .filter(|e| e.id == id)
        .map(|e| (e.span.line, e.message))
        .collect()
}

const STYLES: &str = "style Plain\n  background-color = \"#ffffff\"\n  border-top-left-radius = \"8px\"\n\nstyle Quiet\n  background-color = \"#eeeeee\"\n  box-shadow = \"none\"\n\nstyle Header\n  color = \"#111111\"\n  font-size = \"17px\"\n\n";

#[test]
fn a_component_with_parts_compiles_and_part_reaches_no_plan() {
    let src = STYLES.to_string()
        + &spell(
            "  state quiet = false\n",
            &[("surface", "paint shape space"), ("header", "text space")],
            "    column part=\"surface\" class=(quiet ? Quiet : Plain) padding=\"12px\"\n      text \"Title\" part=\"header\" class=Header\n",
        );
    let all = errors(&src);
    assert!(all.is_empty(), "{all:?}");
    let plan = contract::compile_path_source_all(Path::new("app.contract"), &src, false)
        .unwrap()
        .0;
    assert!(
        !plan.strings.iter().any(|s| s == "surface" || s == "header"),
        "a part's name reaches no plan"
    );
}

#[test]
fn a_part_is_one_declared_node_of_the_components_own_view() {
    let id = "lower-anatomy-part";
    // An entry with no node.
    let src = spell(
        "",
        &[("surface", "paint"), ("body", "space")],
        "    column part=\"surface\"\n      text \"a\"\n",
    );
    let got = with(&src, id);
    assert!(
        got.iter()
            .any(|(l, m)| *l == 4 && m.contains("part `body`") && m.contains("names no node")),
        "{got:?}"
    );
    // A node with no entry.
    let src = spell(
        "",
        &[("surface", "paint")],
        "    column part=\"surface\"\n      text \"a\" part=\"title\"\n",
    );
    let got = with(&src, id);
    assert!(
        got.iter()
            .any(|(l, m)| *l == 6 && m.contains("declares no part `title`")),
        "{got:?}"
    );
    // Two nodes, one part.
    let src = spell(
        "",
        &[("surface", "paint")],
        "    column part=\"surface\"\n      column part=\"surface\"\n        text \"a\"\n",
    );
    let got = with(&src, id);
    assert!(
        got.iter().any(|(l, m)| *l == 6 && m.contains("two nodes")),
        "{got:?}"
    );
    // A name by a computed value, and a name that is no lower-case word.
    let src = spell(
        "  state p = \"surface\"\n",
        &[("Surface", "paint")],
        "    column part=p\n      text \"a\"\n",
    );
    let got = with(&src, id);
    assert!(
        got.iter().any(|(_, m)| m.contains("string literal")),
        "{got:?}"
    );
    assert!(
        got.iter().any(|(_, m)| m.contains("lower-case word")),
        "{got:?}"
    );
    // A node of a used component's slot fill is that component's content.
    let src = spell(
        "",
        &[("inner", "paint")],
        "    Box()\n      text \"a\" part=\"inner\"\n",
    ) + "\ncomponent Box\n  slot\n  view\n    column\n      children\n";
    let got = with(&src, id);
    assert!(got.iter().any(|(_, m)| m.contains("slot fill")), "{got:?}");
}

#[test]
fn a_group_is_one_the_table_names() {
    let src = spell(
        "",
        &[("surface", "paint colour")],
        "    column part=\"surface\"\n      text \"a\"\n",
    );
    let got = with(&src, "lower-anatomy-group");
    assert!(
        got.iter()
            .any(|(l, m)| *l == 3 && m.contains("`colour` is no row group")),
        "{got:?}"
    );
}

#[test]
fn a_class_brings_a_part_only_its_groups_rows() {
    let id = "lower-anatomy-row";
    // Plain: Header's colour and size on a `space` part.
    let src = STYLES.to_string()
        + &spell(
            "",
            &[("body", "space")],
            "    column part=\"body\" class=Header\n      text \"a\"\n",
        );
    let got = with(&src, id);
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(
        got.iter().all(|(_, m)| m.contains("`style Header`")
            && m.contains("part `body`")
            && m.contains("takes `space`")),
        "{got:?}"
    );
    // Every branch of a choice: Quiet's shadow is paint, Plain's corner is not.
    let src = STYLES.to_string()
        + &spell(
            "  state q = false\n",
            &[("surface", "paint")],
            "    column part=\"surface\" class=(q ? Quiet : Plain)\n      text \"a\"\n",
        );
    let got = with(&src, id);
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(
        got[0].1.contains("border-top-left-radius") && got[0].1.contains("`style Plain`"),
        "{got:?}"
    );
    // A state layer's row is its row's: `:disabled color` is text.
    let src = "style Dim\n  :disabled\n    color = \"#888888\"\n\n".to_string()
        + &spell(
            "",
            &[("surface", "paint")],
            "    button part=\"surface\" class=Dim appearance=\"none\"\n      text \"a\"\n",
        );
    let got = with(&src, id);
    assert!(
        got.iter()
            .any(|(_, m)| m.contains("color") && m.contains("`style Dim`")),
        "{got:?}"
    );
    // A shorthand's longhands, each: `padding` is space, `border` is not all paint.
    let src = "style Pad\n  padding = \"8px\"\n  border = \"1px solid #000000\"\n\n".to_string()
        + &spell(
            "",
            &[("surface", "space paint")],
            "    column part=\"surface\" class=Pad\n      text \"a\"\n",
        );
    let got = with(&src, id);
    assert!(
        !got.is_empty() && got.iter().all(|(_, m)| m.contains("`border`")),
        "{got:?}"
    );
    // The node's own rows are its own: no group limits them.
    let src = spell(
        "",
        &[("surface", "space")],
        "    column part=\"surface\" background-color=\"#ff0000\"\n      text \"a\"\n",
    );
    assert!(with(&src, id).is_empty());
}

#[test]
fn a_native_controls_part_is_the_control_group_alone() {
    let id = "lower-anatomy-control";
    // `control` on what draws no native control.
    let src = spell(
        "",
        &[("surface", "control")],
        "    column part=\"surface\"\n      text \"a\"\n",
    );
    let got = with(&src, id);
    assert!(
        got.iter().any(|(_, m)| m.contains("no native control")),
        "{got:?}"
    );
    // A native control's part with another group.
    let src = spell(
        "  state on = false\n",
        &[("toggle", "control paint")],
        "    input type=\"checkbox\" switch checked=on aria-label=\"On\" part=\"toggle\"\n",
    );
    let got = with(&src, id);
    assert!(
        got.iter().any(|(_, m)| m.contains("`control` group alone")),
        "{got:?}"
    );
    // Its rows, its own or its classes': `accent-color` only on an input.
    let src = "style Tinted\n  accent-color = \"#0a7aff\"\n  background-color = \"#ffffff\"\n\n".to_string()
        + &spell("  state on = false\n", &[("toggle", "control")], "    input type=\"checkbox\" switch checked=on aria-label=\"On\" part=\"toggle\" class=Tinted\n");
    let got = with(&src, id);
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(got[0].1.contains("background-color"), "{got:?}");
    // A native button: its native rows pass; a corner needs `appearance="auto"`.
    let src = spell(
        "",
        &[("go", "control")],
        "    button \"Go\" part=\"go\" font-weight=\"600\" border-top-left-radius=\"8px\"\n",
    );
    let got = with(&src, id);
    assert!(
        got.iter().any(
            |(_, m)| m.contains("appearance=\\\"auto\\\"") || m.contains("appearance=\"auto\"")
        ),
        "{got:?}"
    );
    let src = spell("", &[("go", "control")], "    button \"Go\" part=\"go\" font-weight=\"600\" border-top-left-radius=\"8px\" appearance=\"auto\"\n");
    assert!(with(&src, id).is_empty(), "{:?}", with(&src, id));
}

/// LLP 1139 D5: the Tamber IR file's `library` block, each component that
/// names parts with its parts (groups, and the slot's holder) and its
/// variant props (an enum or bool a class condition or a row's `?:` reads).
#[test]
fn the_ir_lists_parts_slot_and_variants() {
    let dir = std::env::temp_dir().join(format!("exact2-anatomy-ir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let card = STYLES.to_string()
        + "component Card\n  props\n    tone: enum<\"plain\", \"quiet\">\n    dense: bool\n    title: string\n  slot\n  parts\n    surface: paint shape space\n    body: space\n  view\n    column part=\"surface\" class=(tone == \"quiet\" ? Quiet : Plain)\n      text title\n      column part=\"body\" gap=(dense ? \"4px\" : \"8px\")\n        children\n";
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"./card.contract\"\n\ncomponent App\n  view\n    Card(tone=\"plain\", dense=false, title=\"T\")\n      text \"a\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("card.contract"), card).unwrap();
    let app = contract::tamber_params::params_of(&dir.join("app.contract"));
    let ir = contract::tamber_params::library(&app, None);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        ir,
        serde_json::json!([{
            "identity": "card.contract#Card",
            "name": "Card",
            "parts": [
                {"name": "surface", "groups": ["paint", "shape", "space"], "slot": false},
                {"name": "body", "groups": ["space"], "slot": true},
            ],
            "variants": [
                {"prop": "tone", "options": ["plain", "quiet"]},
                {"prop": "dense", "options": ["false", "true"]},
            ],
            // No library's (C2, LLP 1139 D0): no slot chain recorded.
            "library": false,
            "chain": [],
        }]),
        "{ir}"
    );
}

/// Round 1 (Astra, Grok): a native button's `control` part is refused
/// wherever lowering would demote the button to a bare box, by a row's
/// value too; a class row the node sets itself never reaches it; a node is
/// one part; and an anatomy refusal is said beside an unrelated type error.
#[test]
fn round_1_edges() {
    let id = "lower-anatomy-control";
    for (name, rows) in [
        ("stretch", "align-items=\"stretch\""),
        ("block", "display=\"block\""),
    ] {
        let src = spell(
            "",
            &[("go", "control")],
            &format!("    button \"Go\" part=\"go\" {rows}\n"),
        );
        let got = with(&src, id);
        assert!(
            got.iter().any(|(_, m)| m.contains("bare box")),
            "{name}: {got:?} / {:?}",
            errors(&src)
        );
    }
    // A class row the node overrides doesn't reach the part.
    let src = "style Wide\n  background-color = \"#ffffff\"\n  width = \"80px\"\n\n".to_string()
        + &spell(
            "",
            &[("surface", "paint")],
            "    column part=\"surface\" class=Wide width=\"100px\"\n      text \"a\"\n",
        );
    let got = with(&src, "lower-anatomy-row");
    assert!(got.is_empty(), "{got:?}");
    // Two names on one node: refused (the parser's duplicate attribute,
    // before the anatomy's own "a node is one part").
    let src = spell(
        "",
        &[("a", "paint"), ("b", "paint")],
        "    column part=\"a\" part=\"b\"\n      text \"x\"\n",
    );
    let all = errors(&src);
    assert!(
        all.iter().any(|e| e.id == "syntax-duplicate-attr"),
        "{all:?}"
    );
    // Beside a type error, the anatomy's own refusal is said too.
    let src = spell(
        "  state p = \"x\"\n",
        &[("surface", "paint")],
        "    column part=p disabled=missing\n      text \"a\"\n",
    );
    let all = errors(&src);
    assert!(all.iter().any(|e| e.id == "lower-anatomy-part"), "{all:?}");
    // The formatter keeps a part's `=` tight.
    let src = spell(
        "",
        &[("surface", "paint")],
        "    column part=\"surface\" padding=\"4px\"\n      text \"a\"\n",
    );
    assert_eq!(contract_syntax::fmt::format(&src).unwrap(), src);
}

/// Round 1 (Astra, Grok): a variant is an enum or bool prop a `class` or a
/// style row's `?:` or `match` reads, anywhere in it (an arm too), not one
/// another attribute reads, nor a name a `for` binds over a prop's.
#[test]
fn variants_are_read_by_classes_and_rows() {
    let dir = std::env::temp_dir().join(format!("exact2-anatomy-var-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let card = "component Card\n  props\n    loud: bool\n    tone: enum<\"red\", \"blue\">\n    active: bool\n    items: string[]\n    dense: bool\n  parts\n    surface: paint\n  view\n    column part=\"surface\" aria-label=(loud ? \"a\" : \"b\") background-color=(active ? tone : \"blue\")\n      for dense in items key=dense\n        text dense padding=(dense == \"x\" ? \"1px\" : \"2px\")\n";
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"./card.contract\"\n\ncomponent App\n  view\n    Card(loud=false, tone=\"red\", active=false, items=[], dense=false)\n",
    )
    .unwrap();
    std::fs::write(dir.join("card.contract"), card).unwrap();
    let app = contract::tamber_params::params_of(&dir.join("app.contract"));
    let ir = contract::tamber_params::library(&app, None);
    let _ = std::fs::remove_dir_all(&dir);
    let props: Vec<&str> = ir[0]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["prop"].as_str().unwrap())
        .collect();
    assert_eq!(props, ["tone", "active"], "{ir}");
}

/// Round 2 (Astra, Grok): a `control` button is judged where lowering
/// draws it, in its instance's context (inside a grouped `list` it is a
/// row, a bare box), and after the keyframes its animation names exist; a
/// name a `match` arm binds in a row is no variant.
#[test]
fn round_2_edges() {
    let id = "lower-anatomy-control";
    let src = spell(
        "",
        &[("go", "control")],
        "    list appearance=\"auto\"\n      section\n        button \"Go\" part=\"go\"\n",
    );
    let got = with(&src, id);
    assert!(
        got.iter().any(|(_, m)| m.contains("bare box")),
        "{:?}",
        errors(&src)
    );
    let src = "keyframes Fade\n  from opacity=0\n  to opacity=1\n\n".to_string()
        + &spell(
            "",
            &[("go", "control")],
            "    button \"Go\" part=\"go\" animation=\"Fade 200ms\"\n",
        );
    assert!(errors(&src).is_empty(), "{:?}", errors(&src));
}

#[test]
fn a_match_arms_binding_is_no_variant() {
    let dir = std::env::temp_dir().join(format!("exact2-anatomy-var2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let card = "component Card\n  props\n    dense: bool\n    pick: option<string>\n  parts\n    surface: paint\n  view\n    column part=\"surface\" padding=(match pick { case some(dense) => dense, case none => \"1px\" })\n      text \"a\"\n";
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"./card.contract\"\n\ncomponent App\n  view\n    Card(dense=false, pick=none)\n",
    )
    .unwrap();
    std::fs::write(dir.join("card.contract"), card).unwrap();
    let app = contract::tamber_params::params_of(&dir.join("app.contract"));
    let ir = contract::tamber_params::library(&app, None);
    let _ = std::fs::remove_dir_all(&dir);
    let props: Vec<&str> = ir[0]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["prop"].as_str().unwrap())
        .collect();
    assert!(!props.contains(&"dense"), "{ir}");
}

/// Round 3 (Astra): a node's shorthand overrides its class's longhands
/// (and the reverse) in the same layer; a button no group names `control`
/// and no `appearance="auto"` says native is the authored pressable, which
/// takes the paint groups.
#[test]
fn round_3_edges() {
    let src = "style Spaced\n  margin-top = \"8px\"\n  background-color = \"#ffffff\"\n\n"
        .to_string()
        + &spell(
            "",
            &[("surface", "paint")],
            "    column part=\"surface\" class=Spaced margin=\"0px\"\n      text \"a\"\n",
        );
    assert!(
        with(&src, "lower-anatomy-row").is_empty(),
        "{:?}",
        errors(&src)
    );
    let src = "style Post\n  background-color = \"#0a7aff\"\n  border-top-left-radius = \"8px\"\n\n"
        .to_string()
        + &spell(
            "",
            &[("post", "paint shape")],
            "    button \"Post\" part=\"post\" class=Post\n",
        );
    let all = errors(&src);
    assert!(
        !all.iter().any(|e| e.id.starts_with("lower-anatomy")),
        "{all:?}"
    );
}

/// Re-check after round 3 (Astra): a default button the platform draws as
/// its own, named for the paint groups, is refused where lowering decides
/// it; with `appearance="none"` it is the authored pressable.
#[test]
fn a_native_buttons_part_is_control_where_lowering_draws_it() {
    let src = spell(
        "",
        &[("face", "paint")],
        "    button \"Go\" part=\"face\"\n",
    );
    let got = with(&src, "lower-anatomy-control");
    assert!(
        got.iter().any(|(_, m)| m.contains("draws as its own")),
        "{:?}",
        errors(&src)
    );
    let src = spell(
        "",
        &[("face", "paint")],
        "    button \"Go\" part=\"face\" appearance=\"none\" background-color=\"#0a7aff\"\n",
    );
    assert!(
        with(&src, "lower-anatomy-control").is_empty(),
        "{:?}",
        errors(&src)
    );
}

/// LLP 1139 D5, for stage D's selection report (rt1): the source map names
/// the part each node is, since `part=` reaches no plan or host.
#[test]
fn the_source_map_names_a_nodes_part() {
    let src = STYLES.to_string()
        + &spell(
            "",
            &[("surface", "paint shape space"), ("header", "text space")],
            "    column part=\"surface\" padding=\"12px\"\n      text \"Title\" part=\"header\" class=Header\n      text \"plain\"\n",
        );
    let (plan, map) =
        contract::compile_path_source_mapped(Path::new("app.contract"), &src).unwrap();
    let json: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let parts: Vec<Option<&str>> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["part"].as_str())
        .collect();
    assert!(
        parts.contains(&Some("surface")) && parts.contains(&Some("header")),
        "{parts:?}"
    );
    assert!(parts.contains(&None), "{parts:?}");
}
