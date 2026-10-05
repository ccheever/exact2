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
        graph.consulted[0],
        dir.0.join("app/node_modules/ui/package.json")
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

// Round 3 (Astra, Grok, 2026-10-05).

#[test]
fn a_whole_attribute_call_resolves_as_the_type_checker_reads_it() {
    let dir = Dir::new("attr-call");
    dir.write(
        "ui.contract",
        "fn length(s: string): number = 7\ncomponent Card\n  view\n    text \"c\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use length, Card from \"./ui.contract\"\ncomponent App\n  state length = 0\n  view\n    column\n      Card()\n      view width=length(\"hi\") height=10\n",
    );
    // The same program with the library's `fn` spelled apart: one plan.
    let flat = dir.write(
        "flat.contract",
        "fn seven(s: string): number = 7\ncomponent App\n  state length = 0\n  view\n    column\n      text \"c\"\n      view width=seven(\"hi\") height=10\n",
    );
    assert_eq!(
        contract::compile_path(&root).unwrap().encode(),
        contract::compile_path(&flat).unwrap().encode()
    );
}

#[test]
fn a_number_fills_the_count_and_none_the_fill_mode() {
    let dir = Dir::new("count-fill");
    dir.write(
        "ui.contract",
        "keyframes infinite\n  to opacity=0\nkeyframes forwards\n  to opacity=0\ncomponent Card\n  view\n    column\n      view animation=\"2 1s infinite\"\n      view animation=\"none 1s forwards\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes infinite\n  to opacity=1\nkeyframes forwards\n  to opacity=1\ncomponent App\n  view\n    Card()\n",
    );
    let text = plan(&root);
    assert!(text.contains("\"2 1s infinite__ui\""), "{text}");
    assert!(text.contains("\"none 1s forwards__ui\""), "{text}");
}

#[test]
fn a_root_shape_named_like_a_roster_function_does_not_capture_a_librarys_call() {
    let dir = Dir::new("root-roster-shape");
    dir.write(
        "ui.contract",
        "component Card\n  view\n    text `${length(\"hi\")}`\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nshape length\n  n: number\ncomponent App\n  derive l = length(n=1)\n  view\n    column\n      Card()\n      text `${l.n}`\n",
    );
    contract::compile_path(&root).unwrap();
}

#[test]
fn a_missing_target_is_watched_so_creating_it_builds_again() {
    let dir = Dir::new("missing-target");
    dir.write(
        "app/node_modules/ui/package.json",
        r#"{"name":"ui","exports":"./index.contract"}"#,
    );
    let root = dir.write(
        "app/app.contract",
        "use Card from \"ui\"\ncomponent App\n  view\n    Card()\n",
    );
    let graph = contract::source_graph(&root);
    assert_eq!(graph.errors.len(), 1);
    assert!(
        graph
            .consulted
            .contains(&dir.0.join("app/node_modules/ui/index.contract")),
        "{:?}",
        graph.consulted
    );
}

// Round 4 (Astra, Grok, 2026-10-05).

#[test]
fn computed_easings_times_tabs_and_infinities_are_read_as_motion_reads_them() {
    let dir = Dir::new("round4-shorthand");
    dir.write(
        "ui.contract",
        "keyframes linear\n  to opacity=0\nkeyframes pulse\n  to opacity=0\nkeyframes slide\n  to opacity=0\ncomponent Card\n  props\n    n: number\n  view\n    column\n      view animation=`steps(${n}, jump-end) linear 1s`\n      view animation=\"\tpulse 1s\"\n      view animation=\"inf slide\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes linear\n  to opacity=1\nkeyframes pulse\n  to opacity=1\nkeyframes slide\n  to opacity=1\ncomponent App\n  view\n    Card(n=3)\n",
    );
    let text = plan(&root);
    assert!(text.contains("jump-end) linear__ui 1s"), "{text}");
    assert!(text.contains("pulse__ui 1s"), "{text}");
    assert!(!text.contains("pulse__uie"), "{text}");
    assert!(text.contains("\"inf slide__ui\""), "{text}");
}

#[test]
fn a_state_named_t_does_not_stop_the_strings_intrinsic() {
    let dir = Dir::new("t-state");
    dir.write(
        "ui.contract",
        "fn t(k: string): string = \"from-fn\"\ncomponent Card\n  view\n    text \"c\"\n",
    );
    dir.write("strings/en.json", r#"{"hi":"Hello"}"#);
    let root = dir.write(
        "app.contract",
        "use t, Card from \"./ui.contract\"\ncomponent App\n  state t = 0\n  view\n    column\n      Card()\n      text t(\"hi\")\n",
    );
    let text = plan(&root);
    assert!(!text.contains("from-fn"), "{text}");
}

#[test]
fn a_used_style_of_a_roster_name_does_not_shield_a_root_shape() {
    let dir = Dir::new("style-namespace");
    dir.write("styles.contract", "style Pad\n  padding-top=1\n");
    dir.write(
        "ui.contract",
        "use Pad as length from \"./styles.contract\"\ncomponent Card\n  view\n    text `${length(\"hi\")}` class=length\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nshape length\n  n: number\ncomponent App\n  derive l = length(n=1)\n  view\n    column\n      Card()\n      text `${l.n}`\n",
    );
    contract::compile_path(&root).unwrap();
}

#[test]
fn a_package_not_installed_is_watched_where_it_would_be() {
    let dir = Dir::new("not-installed");
    let root = dir.write(
        "app/app.contract",
        "use Card from \"ui\"\ncomponent App\n  view\n    Card()\n",
    );
    let graph = contract::source_graph(&root);
    assert_eq!(graph.errors.len(), 1);
    assert!(
        graph
            .consulted
            .contains(&dir.0.join("app/node_modules/ui/package.json")),
        "{:?}",
        graph.consulted
    );
}

// Round 5 (Astra, Grok, 2026-10-05).

#[test]
fn t_follows_its_innermost_binding() {
    let dir = Dir::new("t-innermost");
    dir.write(
        "ui.contract",
        "fn helper(k: string): string = \"wrong\"\ncomponent Card\n  view\n    text \"c\"\n",
    );
    dir.write("strings/en.json", r#"{"hi":"Hello"}"#);
    // A `match` binder of `t` is nearer than the action: the call is the
    // strings intrinsic.
    let root = dir.write(
        "app.contract",
        "use helper as t, Card from \"./ui.contract\"\ncomponent App\n  state n = 0\n  action t\n    n = 1\n  view\n    column\n      Card()\n      text (match some(1) { case some(t) => t(\"hi\"), case none => \"\" })\n",
    );
    let text = plan(&root);
    assert!(!text.contains("wrong"), "{text}");
}

#[test]
fn a_shorthand_motion_refuses_is_not_rewritten_into_one_it_plays() {
    let dir = Dir::new("third-time");
    dir.write(
        "ui.contract",
        "keyframes infs\n  to opacity=0\ncomponent Card\n  props\n    a: string\n  view\n    view animation=a\n",
    );
    // The literal reaches the library's attribute only through a prop, so it
    // is checked where it lands; here the rewrite itself is what is tested.
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes infs\n  to opacity=1\ncomponent App\n  view\n    column\n      Card(a=\"x\")\n      view animation=\"1s 1s infs\"\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(
        e.as_ref().is_some_and(|e| e.id != "contract-use-missing"),
        "a refused shorthand stays refused: {e:?}"
    );
}

#[test]
fn a_missing_package_is_watched_at_every_ancestor() {
    let dir = Dir::new("hoisted");
    let root = dir.write(
        "repo/a/b/c/d/app.contract",
        "use Card from \"ui\"\ncomponent App\n  view\n    Card()\n",
    );
    let graph = contract::source_graph(&root);
    assert!(
        graph
            .consulted
            .contains(&dir.0.join("repo/node_modules/ui/package.json")),
        "{:?}",
        graph.consulted
    );
}

// Round 6 (Astra, Grok, 2026-10-05).

#[test]
fn a_surface_head_is_the_drawing_modules_not_a_function() {
    let dir = Dir::new("surface-head");
    dir.write(
        "ui.contract",
        "fn chart(n: number): number = n\ncomponent Card\n  view\n    text \"c\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\ncomponent App\n  view\n    column\n      Card()\n      canvas surface=chart() width=10 height=10\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(
        e.as_ref().is_none_or(|e| e.id != "contract-use-missing"),
        "{e:?}"
    );
}

#[test]
fn a_task_does_not_hide_a_prop_named_t() {
    let dir = Dir::new("task-t");
    dir.write(
        "ui.contract",
        "fn t(k: string): string = \"from-fn\"\ncomponent Card\n  view\n    text \"c\"\n",
    );
    dir.write("strings/en.json", r#"{"hi":"Hello"}"#);
    // The checker's scope holds no tasks: the prop `t` is the innermost
    // binding, so `t(…)` is the imported fn, not the strings intrinsic.
    let root = dir.write(
        "app.contract",
        "use t, Card from \"./ui.contract\"\ncomponent App\n  props\n    t: string\n  state n = 0\n  action go\n    n = n + 1\n  task t mount\n    every(1000, go)\n  view\n    column\n      Card()\n      text t(\"hi\")\n",
    );
    match contract::compile_path(&root) {
        Ok(plan) => assert!(format!("{plan:?}").contains("from-fn")),
        // A root with props, or a task named like a prop, is the checker's
        // to refuse; what must not happen is an unknown function.
        Err(e) => assert!(
            e.id != "type-unknown-function" && e.id != "contract-use-missing",
            "{e}"
        ),
    }
}

#[test]
fn many_times_in_a_shorthand_do_not_overflow() {
    let dir = Dir::new("many-times");
    let times = vec!["1s"; 300].join(" ");
    dir.write(
        "ui.contract",
        "keyframes p\n  to opacity=0\ncomponent Card\n  view\n    text \"c\"\n",
    );
    let root = dir.write(
        "app.contract",
        &format!("use Card from \"./ui.contract\"\nkeyframes p\n  to opacity=1\ncomponent App\n  view\n    column\n      Card()\n      view animation=\"{times}\"\n"),
    );
    let _ = contract::compile_path(&root);
}

#[cfg(unix)]
#[test]
fn an_installs_own_path_is_watched_so_relinking_it_rebuilds() {
    let dir = Dir::new("relink");
    dir.write(
        "v1/package.json",
        r#"{"name":"ui","exports":"./index.contract"}"#,
    );
    dir.write(
        "v1/index.contract",
        "component Card\n  view\n    text \"v1\"\n",
    );
    std::fs::create_dir_all(dir.0.join("app/node_modules")).unwrap();
    std::os::unix::fs::symlink(dir.0.join("v1"), dir.0.join("app/node_modules/ui")).unwrap();
    let root = dir.write(
        "app/app.contract",
        "use Card from \"ui\"\ncomponent App\n  view\n    Card()\n",
    );
    let graph = contract::source_graph(&root);
    assert!(
        graph
            .consulted
            .contains(&dir.0.join("app/node_modules/ui/package.json")),
        "{:?}",
        graph.consulted
    );
}

// Round 7 (Astra, Grok, 2026-10-05).

#[test]
fn a_component_argument_named_surface_is_a_call_like_any_other() {
    let dir = Dir::new("surface-arg");
    dir.write(
        "ui.contract",
        "fn val(): number = 2\ncomponent Card\n  view\n    Label(surface=val())\ncomponent Label\n  props\n    surface: number\n  view\n    text `${surface}`\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nfn val(): number = 1\ncomponent App\n  view\n    column\n      Card()\n      text `${val()}`\n",
    );
    // The same program with the library's `fn` spelled apart: one plan.
    let flat = dir.write(
        "flat.contract",
        "fn two(): number = 2\nfn val(): number = 1\ncomponent App\n  view\n    column\n      Label(surface=two())\n      text `${val()}`\ncomponent Label\n  props\n    surface: number\n  view\n    text `${surface}`\n",
    );
    assert_eq!(
        contract::compile_path(&root).unwrap().encode(),
        contract::compile_path(&flat).unwrap().encode()
    );
}

#[test]
fn a_nearer_install_and_the_offered_export_path_are_watched() {
    let dir = Dir::new("nearer");
    dir.write(
        "repo/node_modules/ui/package.json",
        r#"{"name":"ui","exports":"./index.contract"}"#,
    );
    dir.write(
        "repo/node_modules/ui/index.contract",
        "component Card\n  view\n    text \"v1\"\n",
    );
    let root = dir.write(
        "repo/apps/demo/app.contract",
        "use Card from \"ui\"\ncomponent App\n  view\n    Card()\n",
    );
    contract::compile_path(&root).unwrap();
    let graph = contract::source_graph(&root);
    for path in [
        "repo/apps/demo/node_modules/ui/package.json",
        "repo/apps/node_modules/ui/package.json",
        "repo/node_modules/ui/index.contract",
    ] {
        assert!(
            graph.consulted.contains(&dir.0.join(path)),
            "{path}: {:?}",
            graph.consulted
        );
    }
}

// Round 8 (Astra, Grok, 2026-10-05).

#[test]
fn a_component_argument_is_a_value_not_a_style_row() {
    let dir = Dir::new("arg-value");
    let root = dir.write(
        "app.contract",
        "use Activity as Shared from \"exact:motion\"\ntimeline Activity\ncomponent App\n  view\n    Label(animationTimeline=\"clock(Shared)\")\ncomponent Label\n  props\n    animationTimeline: string\n  view\n    text animationTimeline\n",
    );
    let text = plan(&root);
    assert!(text.contains("clock(Shared)"), "{text}");
}

#[test]
fn a_generated_name_is_never_a_binding_of_its_own_file() {
    let dir = Dir::new("generated-own");
    dir.write(
        "ui.contract",
        "fn val(s: string): string = s\ncomponent Card\n  state n = 0\n  action val__ui(x: number)\n    n = x\n  view\n    button press=val__ui(1) testId=\"b\"\n      text `${val(\"x\")}`\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nfn val(s: string): string = s\ncomponent App\n  view\n    column\n      Card()\n      text `${val(\"y\")}`\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(e.as_ref().is_none_or(|e| e.id != "type-argument"), "{e:?}");
}

#[test]
fn a_relative_use_is_watched_by_the_path_written() {
    let dir = Dir::new("relative-written");
    dir.write(
        "lib/v1.contract",
        "component Card\n  view\n    text \"v1\"\n",
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        dir.0.join("lib/v1.contract"),
        dir.0.join("lib/entry.contract"),
    )
    .unwrap();
    let root = dir.write(
        "app.contract",
        "use Card from \"./lib/entry.contract\"\ncomponent App\n  view\n    Card()\n",
    );
    #[cfg(unix)]
    {
        let graph = contract::source_graph(&root);
        assert!(
            graph.consulted.contains(&dir.0.join("lib/entry.contract")),
            "{:?}",
            graph.consulted
        );
    }
}

// Round 9 (Astra, Grok, 2026-10-05).

#[test]
fn a_keyword_beside_a_computed_value_that_is_also_renamed_keyframes_is_refused() {
    // `${easing} linear 1s`: `linear` is the easing if the value is a time,
    // the name if it is an easing. No rename is right for both (round 10).
    let dir = Dir::new("computed-easing");
    dir.write(
        "ui.contract",
        "keyframes linear\n  to opacity=0\ncomponent Card\n  state easing = \"ease\"\n  view\n    view animation=`${easing} linear 1s`\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes linear\n  to opacity=1\ncomponent App\n  view\n    Card()\n",
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert_eq!(e.id, "contract-animation-ambiguous", "{e}");
}

#[test]
fn a_compiler_declared_type_is_no_files_name() {
    let dir = Dir::new("builtin-type");
    dir.write(
        "ui.contract",
        "fn PointerEvent(x: number): number = x\ncomponent Card\n  view\n    text \"c\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\ncomponent App\n  state x = 0\n  action point(e: PointerEvent)\n    x = e.offsetX\n  view\n    column\n      Card()\n      view pointerdown=point width=10 height=10\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(
        e.as_ref().is_none_or(|e| e.id != "contract-use-missing"),
        "{e:?}"
    );
}

#[test]
fn a_shape_named_path_does_not_take_the_routers_call() {
    let dir = Dir::new("shape-path");
    dir.write(
        "ui.contract",
        "shape path\n  n: number\ncomponent Card\n  view\n    text path(\"home\")\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nroutes nav\n  home \"/\"\ncomponent App\n  view\n    column\n      Card()\n      text path(\"home\")\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(
        e.as_ref().is_none_or(|e| e.id != "type-record-base"),
        "{e:?}"
    );
}

// Round 10 (Astra, Grok, 2026-10-05).

#[test]
fn a_computed_part_fills_no_slot_and_the_name_motion_reads_is_renamed() {
    let dir = Dir::new("computed-slot");
    dir.write(
        "ui.contract",
        "keyframes spin\n  to opacity=0\ncomponent Card\n  state dur = \"1s\"\n  view\n    view animation=`${dur} ease spin`\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes spin\n  to opacity=1\ncomponent App\n  view\n    Card()\n",
    );
    let text = plan(&root);
    assert!(text.contains(" ease spin__ui"), "{text}");
}

#[test]
fn a_type_is_a_shape_never_a_renamed_fn() {
    let dir = Dir::new("type-not-fn");
    dir.write(
        "ui.contract",
        "fn PointerEvent(x: number): number = x\ncomponent Card\n  state x = 0\n  action point(e: PointerEvent)\n    x = e.offsetX\n  view\n    view pointerdown=point width=10 height=10\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nfn PointerEvent(x: number): number = x\ncomponent App\n  view\n    Card()\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(e.as_ref().is_none_or(|e| e.id != "type-unknown"), "{e:?}");
}

#[test]
fn a_route_pages_source_is_a_data_source_not_a_fn() {
    let dir = Dir::new("route-pages");
    dir.write(
        "ui.contract",
        "fn ids(x: number): number = x\ncomponent Card\n  view\n    text \"c\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nroutes nav\n  home \"/\"\n  item \"/item/:id\" render=build pages=ids()\ncomponent App\n  view\n    Card()\n",
    );
    let e = contract::compile_path(&root).err();
    assert!(
        e.as_ref().is_none_or(|e| e.id != "contract-use-missing"),
        "{e:?}"
    );
}

// Round 11 (Astra, Grok, 2026-10-05).

#[test]
fn a_keyword_whose_slot_is_settled_is_never_refused() {
    let dir = Dir::new("settled-keyword");
    dir.write(
        "ui.contract",
        "keyframes pulse\n  to opacity=0\ncomponent Card\n  state d = \"1s\"\n  view\n    view animation=`pulse ${d} linear`\n",
    );
    // `linear` beside `${d}`: `pulse` is already the name, so `linear` can
    // only be the easing, whatever `d` is.
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes linear\n  to opacity=1\ncomponent App\n  state d = \"pulse\"\n  view\n    column\n      Card()\n      view animation=`linear ${d}`\n",
    );
    contract::compile_path(&root).unwrap();
}

#[test]
fn a_type_never_reaches_another_files_shape_past_a_fn() {
    let dir = Dir::new("type-foreign-shape");
    dir.write(
        "ui.contract",
        "fn Info(x: number): number = x\ncomponent Card\n  props\n    value: Info\n  view\n    text `${value.n}`\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nshape Info\n  n: number\ncomponent App\n  view\n    Card(value=Info(n=7))\n",
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert_eq!(e.id, "contract-use-missing", "{e}");
}

#[test]
fn a_shape_named_action_never_takes_another_files_bare_action() {
    let dir = Dir::new("shape-action");
    dir.write(
        "ui.contract",
        "shape action\n  n: number\ncomponent Card\n  view\n    text \"c\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\ncomponent App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column\n      Card()\n      Child(onPress=go)\ncomponent Child\n  props\n    onPress: action\n  view\n    button press=onPress testId=\"b\"\n      text \"go\"\n",
    );
    contract::compile_path(&root).unwrap();
}

// Round 12 (Astra, Grok, 2026-10-05).

#[test]
fn an_empty_value_fills_no_slot() {
    let dir = Dir::new("empty-value");
    dir.write(
        "ui.contract",
        "keyframes pulse\n  to opacity=0\ncomponent Card\n  state extra = \"\"\n  view\n    view animation=`pulse 1s 0s linear 1 normal both running ${extra}`\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes pulse\n  to opacity=1\ncomponent App\n  view\n    Card()\n",
    );
    let text = plan(&root);
    assert!(text.contains("pulse__ui 1s 0s linear"), "{text}");
}

#[test]
fn many_computed_parts_are_read_exactly() {
    let dir = Dir::new("many-computed");
    dir.write(
        "ui.contract",
        "keyframes spin\n  to opacity=0\nkeyframes linear\n  to opacity=0\ncomponent Card\n  state a = \"1s\"\n  state b = \"0s\"\n  state c = \"ease\"\n  state d = \"infinite\"\n  view\n    column\n      view animation=`spin ${a} ${b} ${c} ${d}`\n      view animation=`'spin' ${a} ${b} ${c} ${d}`\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./ui.contract\"\nkeyframes spin\n  to opacity=1\ncomponent App\n  view\n    Card()\n",
    );
    let text = plan(&root);
    assert!(text.contains("spin__ui "), "{text}");
    assert!(text.contains("'spin__ui' "), "{text}");
}

#[test]
fn a_dot_dot_path_is_watched_as_a_watcher_names_it() {
    let dir = Dir::new("dotdot-watch");
    dir.write("pages/keep.txt", "");
    dir.write(
        ".parts/entry.contract",
        "component Card\n  view\n    text \"c\"\n",
    );
    let root = dir.write(
        "app.contract",
        "use Card from \"./pages/../.parts/entry.contract\"\ncomponent App\n  view\n    Card()\n",
    );
    let graph = contract::source_graph(&root);
    assert!(
        graph
            .consulted
            .contains(&dir.0.join(".parts/entry.contract")),
        "{:?}",
        graph.consulted
    );
}
