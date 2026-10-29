//! Inline atoms in Contract (LLP 1045.000 D3, D7): the `atom` declaration,
//! a Markdown node's `atoms` and `atomFaces`, and the `atompress` event.
use exact_kernel::{Kernel, PropId, StyleId};
use exact_runner::{DataError, DataSource, Event, Runner, Value};
use serde_json::{json, Value as Json};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}
fn boot(source: &str) -> Runner<NoData> {
    Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn id(runner: &Runner<NoData>, test: &str) -> u32 {
    let key = runner.kernel().find_by_test_id(test)[0];
    runner.kernel().node_by_key(key).unwrap().id
}
fn prop(runner: &Runner<NoData>, test: &str, prop: PropId) -> Option<String> {
    let key = runner.kernel().find_by_test_id(test)[0];
    let node = runner.kernel().node_by_key(key).unwrap();
    node.props.str(prop).map(str::to_string)
}
fn json_prop(runner: &Runner<NoData>, test: &str, p: PropId) -> Json {
    serde_json::from_str(&prop(runner, test, p).unwrap()).unwrap()
}
fn refused(source: &str) -> String {
    match contract::compile(source) {
        Ok(_) => panic!("compiled:\n{source}"),
        Err(e) => format!("{e:?}"),
    }
}

const APP: &str = r##"atom citation link="T3-Citation:"
atom mention prefix="@" role="link"
atom skill prefix="$€" background-color="light-dark(#efe7ff, #2c2340)"
component App
  state draft = "[q](t3-citation:1) @a "
  state pressed = ""
  state selected = ""
  derive faces = [AtomFace(source="@a", label="A", icon="", title="Alice", color="light-dark(#d52c36, #ff6762)", backgroundColor="")]
  action press(e: AtomEvent)
    pressed = e.kind + " " + e.source + " " + e.href + " " + e.start.toString() + "-" + e.end.toString() + (e.metaKey ? " meta" : "")
  action select(s: MarkdownSelection)
    selected = s.atom
  view
    column
      textarea value=draft markup="markdown" atoms="citation mention skill" atomFaces=faces atompress=press select=select testId="editor"
      text draft markup="markdown" atoms="citation" atompress=press testId="reader"
      text pressed testId="pressed"
      text selected testId="selected"
"##;

#[test]
fn a_markdown_field_carries_its_kinds_paint_and_faces() {
    let runner = boot(APP);
    assert_eq!(
        json_prop(&runner, "editor", PropId::Atoms),
        json!([
            {"name": "citation", "link": "t3-citation:"},
            {"name": "mention", "prefix": "@", "role": "link"},
            {"name": "skill", "prefix": "$€"},
        ])
    );
    let styles = json_prop(&runner, "editor", PropId::AtomStyles);
    assert_eq!(styles["citation"], json!([]));
    assert_eq!(
        styles["skill"],
        json!([[
            StyleId::BackgroundColor as u16,
            "light-dark(#efe7ff, #2c2340)"
        ]])
    );
    // The colours as the wire's text (`bridge::atom_face_paint`).
    assert_eq!(
        json_prop(&runner, "editor", PropId::AtomFaces),
        json!([{"source": "@a", "label": "A", "icon": "", "title": "Alice",
                "color": "light-dark(#d52c36ff, #ff6762ff)", "backgroundColor": ""}])
    );
    assert_eq!(
        json_prop(&runner, "reader", PropId::Atoms),
        json!([{"name": "citation", "link": "t3-citation:"}])
    );
    assert_eq!(prop(&runner, "reader", PropId::AtomFaces), None);
}

/// A face's colours are read as a bound colour row's (LLP 1045.000 D2): a
/// theme entry's name as its colour, a platform colour only as one of the
/// plan's literals (LLP 1095 D3, D4), and anything else that is no colour
/// as nothing, so its kind's paint stands, with a journal line. A host gets
/// the wire's text.
#[test]
fn a_faces_colours_are_read_as_a_bound_colours_are() {
    let runner = boot(
        r##"theme
  brandRed="light-dark(#d52c36, #ff6762)"
atom mention prefix="@"
component App
  state draft = "@a @b @c "
  state name = "systemRedColor"
  derive faces = [AtomFace(source="@a", label="", icon="", title="", color="brandRed", backgroundColor="-exact-platform-color(macos systemBlueColor, #0000ff)"), AtomFace(source="@b", label="", icon="", title="", color=`-exact-platform-color(macos ${name}, #ff0000)`, backgroundColor="light-dark(#d52c36 #ff6762)"), AtomFace(source="@c", label="", icon="", title="", color="", backgroundColor="")]
  view
    textarea value=draft markup="markdown" atoms="mention" atomFaces=faces testId="editor"
"##,
    );
    let faces = json_prop(&runner, "editor", PropId::AtomFaces);
    let paint = |i: usize, key: &str| faces[i][key].as_str().unwrap().to_string();
    assert_eq!(paint(0, "color"), "light-dark(#d52c36ff, #ff6762ff)");
    assert_eq!(
        paint(0, "backgroundColor"),
        "-exact-platform-color(macos systemBlueColor, #0000ffff)"
    );
    for (i, key) in [(1, "color"), (1, "backgroundColor"), (2, "color")] {
        assert_eq!(paint(i, key), "", "{i} {key}");
    }
    let refused: Vec<&str> = runner
        .journal()
        .filter(|l| l.contains("invalid atomFaces colour"))
        .collect();
    assert_eq!(refused.len(), 2, "{refused:?}");
    assert!(refused[0].contains(r#""-exact-platform-color(macos systemRedColor, #ff0000)""#));
    assert!(refused[1].contains(r#""light-dark(#d52c36 #ff6762)""#));
}

/// A design with a token whose slice the `dark-style` axis picks, and one
/// that does not vary.
const MODES: &str = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = light-dark(#111111, #eeeeee)\n    when dark-style=dim: light-dark(#111111, #cccccc)\n  brand = #c8102e\n";

/// An app beside its design, booted from its baked plan.
fn designed(name: &str, contract: &str) -> Runner<NoData> {
    let dir = std::env::temp_dir().join(format!("exact2-atoms-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), MODES).unwrap();
    std::fs::write(dir.join("app.contract"), contract).unwrap();
    let plan = contract::bake(
        contract::compile_path(&dir.join("app.contract")).unwrap(),
        NoData,
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// A face's colour that names a token, here from state, is read again when
/// the app's mode or the token changes, as a bound `color` is (LLP 1045.000
/// D2): the pill recolours.
#[test]
fn a_faces_token_colour_follows_set_mode_and_set_token() {
    let mut r = designed(
        "modes",
        "atom mention prefix=\"@\"\ncomponent App\n  state c = \"ink\"\n  derive faces = [AtomFace(source=\"@a\", label=\"\", icon=\"\", title=\"\", color=c, backgroundColor=\"brand\")]\n  action dim\n    setMode(\"dark-style\", \"dim\")\n  view\n    column\n      button press=dim testId=\"dim\"\n        text \"dim\"\n      textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=faces testId=\"editor\"\n",
    );
    let paint = |r: &Runner<NoData>, key: &str| {
        json_prop(r, "editor", PropId::AtomFaces)[0][key]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(paint(&r, "color"), "light-dark(#111111ff, #eeeeeeff)");
    assert_eq!(paint(&r, "backgroundColor"), "#c8102eff");
    let dim = id(&r, "dim");
    r.dispatch(dim, Event::Press).unwrap();
    assert_eq!(
        paint(&r, "color"),
        "light-dark(#111111ff, #ccccccff)",
        "setMode reads it again in the new slice"
    );
    let (_, set) = r.set_token("brand", "#00ff00", &[]).unwrap();
    assert_eq!(
        paint(&r, "backgroundColor"),
        "#00ff00ff",
        "and a token's new value"
    );
    assert_eq!(set, 1, "the faces, once");
    assert_eq!(paint(&r, "color"), "light-dark(#111111ff, #ccccccff)");
}

/// A colour in a profile's space is read as a bound `color` reads it (LLP
/// 1100 D3): a built-in space with no declaration, the app's own with its
/// `color-profile`; the wire's text then names it, as Apple draws it.
#[test]
fn a_faces_profile_colour_is_read_as_a_bound_colours_is() {
    let runner = boot(
        "color-profile --press src=\"assets/press.icc\" rendering-intent=\"perceptual\"\natom mention prefix=\"@\"\ncomponent App\n  derive faces = [AtomFace(source=\"@a\", label=\"\", icon=\"\", title=\"\", color=\"color(--dci-p3 1 0 0)\", backgroundColor=\"color(--press 0.1 0.8 0.2)\")]\n  view\n    textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=faces testId=\"editor\"\n",
    );
    let faces = json_prop(&runner, "editor", PropId::AtomFaces);
    assert_eq!(faces[0]["color"], "color(--dci-p3 1 0 0)");
    assert_eq!(faces[0]["backgroundColor"], "color(--press 0.1 0.8 0.2)");
    assert!(
        !runner.journal().any(|l| l.contains("invalid atomFaces")),
        "{:?}",
        runner.journal().collect::<Vec<_>>()
    );
}

/// The bridge reads `AtomFace` in the compiler's field order, and its list
/// is what a plan that binds faces puts in its compatibility id.
#[test]
fn the_bridge_reads_atom_face_in_the_compilers_order() {
    let file = contract_syntax::parse("component G\n  view\n    column\n").unwrap();
    let shapes = contract_types::check_declarations(&file).unwrap();
    let fields: Vec<&str> = shapes.map["AtomFace"]
        .iter()
        .map(|(f, _)| f.as_str())
        .collect();
    assert_eq!(fields, exact_runner::decorations::ATOM_FACE_FIELDS);
}

#[test]
fn an_atom_press_and_the_selected_atom_reach_the_actions() {
    let mut runner = boot(APP);
    let editor = id(&runner, "editor");
    let event = exact_runner::Event::of_host_kind(
        59,
        "0,18,Meta+\ncitation\nt3-citation:1\n[q](t3-citation:1)",
    )
    .unwrap();
    runner.dispatch(editor, event).unwrap();
    assert_eq!(
        prop(&runner, "pressed", PropId::Text).unwrap(),
        "citation [q](t3-citation:1) t3-citation:1 0-18 meta"
    );
    let reader = id(&runner, "reader");
    let event =
        Event::of_host_kind(59, "0,18,\ncitation\nt3-citation:1\n[q](t3-citation:1)").unwrap();
    runner.dispatch(reader, event).unwrap();
    assert!(prop(&runner, "pressed", PropId::Text)
        .unwrap()
        .ends_with("0-18"));
    let select = Event::of_host_kind(21, "\n0\nlink\n[q](t3-citation:1)\nt3-citation:1").unwrap();
    runner.dispatch(editor, select).unwrap();
    assert_eq!(
        prop(&runner, "selected", PropId::Text).unwrap(),
        "[q](t3-citation:1)"
    );
}

#[test]
fn a_malformed_atom_press_is_refused_before_it_runs() {
    for payload in [
        "0,18\ncitation\nx",
        "5,2,\ncitation\n\n@a",
        "0,3,\ncitation\n\n@a",
        "0,2,\n\n\n@a",
        "0.5,2,\nmention\n\n@a",
    ] {
        assert!(Event::of_host_kind(59, payload).is_err(), "{payload:?}");
    }
}

#[test]
fn the_compiler_refuses_what_an_atom_cannot_be() {
    let field = |decl: &str, attrs: &str| {
        format!(
            "{decl}\ncomponent App\n  state d = \"\"\n  view\n    textarea value=d markup=\"markdown\" {attrs}\n"
        )
    };
    let cases = [
        (field("atom a padding=4 link=\"x:\"", "atoms=\"a\""), "lower-atom-style"),
        (field("atom a role=\"link\"", "atoms=\"a\""), "lower-atom-style"),
        (field("atom a link=\"x:\" prefix=\"@\"", "atoms=\"a\""), "lower-atom-style"),
        (field("atom a link=\"not a scheme\"", "atoms=\"a\""), "lower-atom-style"),
        (field("atom a role=\"tab\" prefix=\"@\"", "atoms=\"a\""), "lower-atom-style"),
        (field("atom a prefix=\"@\"\natom b prefix=\"$@\"", "atoms=\"a b\""), "lower-atom-duplicate"),
        (field("atom a link=\"x:\"\natom b link=\"X:\"", "atoms=\"a b\""), "lower-atom-duplicate"),
        (field("atom a link=\"relative\"\natom b link=\"relative\"", "atoms=\"a b\""), "lower-atom-duplicate"),
        (field("atom a link=\"x:\"", "atoms=\"b\""), "lower-atom-unknown"),
        (
            "atom a link=\"x:\"\ncomponent App\n  state d = \"\"\n  view\n    textarea value=d atoms=\"a\"\n".into(),
            "lower-atom-field",
        ),
        (
            "atom a link=\"x:\"\ncomponent App\n  view\n    input atoms=\"a\" markup=\"markdown\"\n".into(),
            "lower-atom-field",
        ),
    ];
    for (source, code) in cases {
        let error = refused(&source);
        assert!(error.contains(code), "{code} for\n{source}\n{error}");
    }
    // A face names its paint, empty for its kind's: a field is never defaulted.
    let unpainted = APP.replace(
        ", color=\"light-dark(#d52c36, #ff6762)\", backgroundColor=\"\"",
        "",
    );
    assert!(
        refused(&unpainted).contains("type-record-missing"),
        "{unpainted}"
    );
    // Two kinds of different forms in one list are fine.
    contract::compile(&field(
        "atom a link=\"x:\"\natom b link=\"relative\"\natom c prefix=\"@\"",
        "atoms=\"a b c\"",
    ))
    .unwrap();
}
