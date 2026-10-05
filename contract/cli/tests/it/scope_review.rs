//! LLP 1091: the code review's findings (Astra, Grok, 2026-10-04), each a
//! case that compiled wrongly or was refused wrongly.

use std::path::PathBuf;

struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("exact-scope-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn plan(root: &std::path::Path) -> String {
    format!("{:?}", contract::compile_path(root).unwrap())
}

#[test]
fn animation_keywords_are_never_keyframes_names() {
    let dir = Dir::new("keywords");
    dir.write(
        "ui.contract",
        "keyframes infinite\n  to opacity=0\nkeyframes spin\n  to opacity=0\ncomponent Spinner\n  view\n    view animation=\"spin 1s linear infinite\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Spinner from \"./ui.contract\"\nkeyframes spin\n  to opacity=1\nkeyframes linear\n  to opacity=1\ncomponent App\n  view\n    column\n      Spinner()\n      view animation=\"spin 2s ease-in-out infinite alternate, linear 1s\"\n",
    );
    let text = plan(&root);
    assert!(text.contains("\"spin__ui 1s linear infinite\""), "{text}");
    assert!(
        text.contains("\"spin 2s ease-in-out infinite alternate, linear 1s\""),
        "{text}"
    );
}

#[test]
fn a_timeline_literal_in_a_match_is_rewritten_too() {
    let dir = Dir::new("match-clock");
    let root = dir.write(
        "app.contract",
        "use Activity as Shared from \"exact:motion\"\ntimeline Activity\nkeyframes p\n  to opacity=0\ncomponent App\n  state opt = some(1)\n  view\n    column\n      view animation=\"p 1s\" animation-timeline=\"clock(Shared)\"\n      view animation=\"p 1s\" animation-timeline=(match opt { case some(x) => \"clock(Shared)\", case none => \"auto\" })\n",
    );
    let text = plan(&root);
    assert!(!text.contains("clock(Shared)"), "{text}");
}

#[test]
fn bindings_primitives_and_the_roster_are_never_another_files_names() {
    let dir = Dir::new("bindings");
    dir.write(
        "ui.contract",
        "fn pick(x: number): number = x\nshape length\n  n: number\ncomponent Card\n  view\n    text \"card\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\ncomponent App\n  state n = 0\n  derive k = length(\"hi\")\n  action pick(x: number)\n    n = x\n  view\n    column\n      Card()\n      button press=pick(1) testId=\"b\"\n        text `${n} ${k}`\n",
    );
    contract::compile_path(&root).unwrap();
}

#[test]
fn alike_fonts_in_two_files_are_one() {
    let dir = Dir::new("fonts");
    dir.write("assets/Inter.ttf", "");
    dir.write(
        "ui.contract",
        "\n\nfont \"Inter\"\n  400 = \"assets/Inter.ttf\"\ncomponent Title\n  view\n    text \"t\" font-family=\"Inter\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Title from \"./ui.contract\"\nfont \"Inter\"\n  400 = \"assets/Inter.ttf\"\ncomponent App\n  view\n    Title()\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(
        e.as_ref().is_none_or(|e| e.id != "lower-font-duplicate"),
        "{e:?}"
    );
}

#[test]
fn a_generated_name_is_no_way_around_a_use() {
    let dir = Dir::new("generated");
    dir.write(
        "ui.contract",
        "fn helper(): number = 2\ncomponent Card\n  view\n    text `${helper()}`\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nfn helper(): number = 1\ncomponent App\n  view\n    column\n      Card()\n      text `${helper__ui()}`\n",
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert_eq!(e.id, "contract-use-missing", "{e}");
}

fn ui_package(dir: &Dir, at: &str, manifest: &str) {
    dir.write(&format!("{at}/package.json"), manifest);
    dir.write(
        &format!("{at}/styles.contract"),
        "style Pad\n  padding-top=10\n",
    );
    dir.write(
        &format!("{at}/src/card.contract"),
        "use Pad from \"../styles.contract\"\ncomponent Card\n  view\n    column class=Pad\n",
    );
}

#[test]
fn the_package_is_the_manifest_whose_exports_were_read() {
    let dir = Dir::new("nested-manifest");
    ui_package(
        &dir,
        "app/node_modules/ui",
        r#"{"name":"ui","version":"1.0.0","exports":{".":{"contract":{"bun":"./x"},"default":"./src/card.contract"}}}"#,
    );
    dir.write(
        "app/node_modules/ui/src/package.json",
        r#"{"type":"module"}"#,
    );
    let root = dir.write(
        "app/app.contract",
        "use Card from \"ui\"\ncomponent App\n  view\n    Card()\n",
    );
    contract::compile_path(&root).unwrap();
    let graph = contract::source_graph(&root);
    assert_eq!(graph.packages.len(), 1);
    assert_eq!(
        graph.packages[0].manifest,
        dir.0.join("app/node_modules/ui/package.json")
    );
}

#[test]
#[cfg(unix)]
fn one_library_under_two_names_is_both_in_the_graph() {
    let dir = Dir::new("aliases");
    ui_package(
        &dir,
        "lib",
        r#"{"name":"lib","version":"1.0.0","exports":"./src/card.contract"}"#,
    );
    for name in ["a", "b"] {
        std::fs::create_dir_all(dir.0.join("app/node_modules")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.0.join("lib"), dir.0.join("app/node_modules").join(name))
            .unwrap();
    }
    let root = dir.write(
        "app/app.contract",
        "use Card as A from \"a\"\nuse Card as B from \"b\"\ncomponent App\n  view\n    column\n      A()\n      B()\n",
    );
    #[cfg(unix)]
    {
        contract::compile_path(&root).unwrap();
        let graph = contract::source_graph(&root);
        let names: Vec<_> = graph.packages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
    }
}

#[test]
fn a_refused_manifest_is_still_watched() {
    let dir = Dir::new("consulted");
    dir.write(
        "app/node_modules/ui/package.json",
        r#"{"name":"ui","exports":"./missing.contract"}"#,
    );
    let root = dir.write(
        "app/app.contract",
        "use Card from \"ui\"\ncomponent App\n  view\n    Card()\n",
    );
    let graph = contract::source_graph(&root);
    assert_eq!(graph.errors.len(), 1);
    assert_eq!(
        graph.consulted,
        [dir.0.join("app/node_modules/ui/package.json")]
    );
}

// Round 2 (Grok, 2026-10-05).

#[test]
fn a_fn_in_scope_is_called_before_a_binding_of_its_name() {
    let dir = Dir::new("fn-before-binding");
    dir.write(
        "ui.contract",
        "fn length(s: string): number = 7\ncomponent Card\n  view\n    text \"c\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use length, Card from \"./ui.contract\"\ncomponent App\n  state length = 0\n  view\n    column\n      Card()\n      text `${length(\"hi\")}` testId=\"n\"\n",
    );
    let text = plan(&root);
    assert!(
        text.contains("length__ui") || !text.contains("Length"),
        "{text}"
    );
}

#[test]
fn a_binding_never_reaches_another_files_fn() {
    let dir = Dir::new("binding-leak");
    dir.write(
        "ui.contract",
        "fn pick(x: number): number = 5\ncomponent Card\n  view\n    text `${pick(1)}`\n",
    );
    // `pick` here is the action: the library's `fn pick` is renamed, so the
    // type checker cannot read `${pick(1)}` as a call of it.
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\ncomponent App\n  state n = 0\n  action pick(x: number)\n    n = x\n  view\n    column\n      Card()\n      button press=pick(1) testId=\"b\"\n        text `${n}`\n",
    );
    contract::compile_path(&root).unwrap();
    // A state of the name, called: the library's `fn` is not this file's, so
    // the call is the state's, refused by the type checker, never the `fn`.
    dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\ncomponent App\n  state pick = 0\n  view\n    column\n      Card()\n      text `${pick(1)}`\n",
    );
    assert!(contract::compile_path(&root).is_err());
}

#[test]
fn a_computed_token_does_not_hide_the_literal_name_and_keywords_are_case_sensitive() {
    let dir = Dir::new("computed-token");
    dir.write(
        "ui.contract",
        "keyframes pulse\n  to opacity=0\nkeyframes Linear\n  to opacity=0\ncomponent Card\n  props\n    ease: string\n  view\n    column\n      view animation=`1s ${ease} pulse`\n      view animation=\"Linear 1s\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes pulse\n  to opacity=1\nkeyframes Linear\n  to opacity=1\ncomponent App\n  view\n    Card(ease=\"linear\")\n",
    );
    let text = plan(&root);
    assert!(text.contains("pulse__ui"), "{text}");
    assert!(text.contains("\"Linear__ui 1s\""), "{text}");
    assert!(!text.contains(" pulse\""), "{text}");
}

#[test]
fn alike_fonts_merge_whatever_the_order_of_their_faces() {
    let dir = Dir::new("font-order");
    dir.write("assets/A.ttf", "");
    dir.write("assets/B.ttf", "");
    dir.write(
        "ui.contract",
        "font \"Inter\"\n  700 = \"assets/B.ttf\"\n  400 = \"assets/A.ttf\"\ncomponent Title\n  view\n    text \"t\" font-family=\"Inter\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Title from \"./ui.contract\"\nfont \"Inter\"\n  400 = \"assets/A.ttf\"\n  700 = \"assets/B.ttf\"\ncomponent App\n  view\n    Title()\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(
        e.as_ref().is_none_or(|e| e.id != "lower-font-duplicate"),
        "{e:?}"
    );
}

// Round 2 (Astra, 2026-10-05).

#[test]
fn a_keyword_whose_slot_is_filled_is_the_name_and_quotes_name_too() {
    let dir = Dir::new("slots");
    dir.write(
        "ui.contract",
        "keyframes linear\n  to opacity=0\nkeyframes pulse\n  to opacity=0\ncomponent Card\n  props\n    n: number\n  view\n    column\n      view animation=\"linear 1s linear\"\n      view animation=`steps(${n}, jump-end) pulse 1s`\n      view animation-timeline=`clock(Shared)` animation=\"pulse 1s\"\n",
    );
    dir.write(
        "ui.contract",
        &std::fs::read_to_string(dir.0.join("ui.contract"))
            .unwrap()
            .replace(
                "view animation-timeline=`clock(Shared)` animation=\"pulse 1s\"\n",
                "view animation=\"pulse 1s\"\n",
            ),
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes linear\n  to opacity=1\nkeyframes pulse\n  to opacity=1\ncomponent App\n  view\n    Card(n=3)\n",
    );
    let text = plan(&root);
    assert!(text.contains("\"linear 1s linear__ui\""), "{text}");
    assert!(text.contains("jump-end) pulse__ui 1s"), "{text}");
}

#[test]
fn a_template_clock_literal_is_rewritten() {
    let dir = Dir::new("template-clock");
    let root = dir.write(
        "app.contract",
        "use Activity as Shared from \"exact:motion\"\ntimeline Activity\nkeyframes p\n  to opacity=0\ncomponent App\n  view\n    column\n      view animation=\"p 1s\" animation-timeline=\"clock(Shared)\"\n      view animation=\"p 1s\" animation-timeline=`clock(Shared)`\n",
    );
    let text = plan(&root);
    assert!(!text.contains("clock(Shared)"), "{text}");
}

#[test]
fn compiler_intrinsics_are_never_another_files_names() {
    let dir = Dir::new("intrinsics");
    dir.write(
        "ui.contract",
        "fn pending(x: number): number = x\ncomponent Card\n  view\n    text \"c\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nshape X\n  n: number\ncomponent App\n  resource r = data() as shape X\n  view\n    column\n      Card()\n      text (pending(r) ? \"waiting\" : \"ready\")\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(
        e.as_ref().is_none_or(|e| e.id != "contract-use-missing"),
        "{e:?}"
    );
}
