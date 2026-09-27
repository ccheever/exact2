//! Action interface diagnostics keep both authored sides, including imports.

use std::{path::PathBuf, process::Command};

fn root(binding: &str, children: &str) -> String {
    format!("component App\n  state n = 0\n  action zero writes n\n    n = 0\n  action one(x: number) writes n\n    n = x\n  action two(x: number, y: number) writes n\n    n = x + y\n  view\n    {binding}\n{children}")
}
fn refusal(source: &str) -> contract::CompileError {
    let error = contract::compile(source).unwrap_err();
    assert_eq!(error.id, "analyze-action-arity", "{error}");
    assert!(error.related.len() >= 2, "{error}");
    error
}
const BUTTON: &str = "component Button\n  props\n    callback: action\n  view\n    button \"run\" press=callback()\n";

#[test]
fn direct_bindings_report_invocation_declaration_and_argument() {
    let source = root("Button(callback=one)", BUTTON);
    let error = refusal(&source);
    let token = |span: contract_syntax::Span| {
        &source.lines().nth(span.line as usize - 1).unwrap()
            [span.col as usize - 1..span.end_col as usize - 1]
    };
    assert_eq!(token(error.span), "callback");
    assert_eq!(token(error.related[0].span), "callback");
    assert_eq!(token(error.related[1].span), "callback");
    assert!(error.related[1].note.contains("one"));
    let json: serde_json::Value = serde_json::from_str(&error.to_json()).unwrap();
    assert!(json["related"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["file"].is_null()));
    contract::compile(&root("Button(callback=zero)", BUTTON)).unwrap();
    contract::compile(&root("Button(callback=one(1))", BUTTON)).unwrap();
}

#[test]
fn requirements_follow_forwarded_and_partially_bound_props() {
    let children = format!("component Wrap\n  props\n    callback: action\n  view\n    Button(callback=callback(1))\n{BUTTON}");
    contract::compile(&root("Wrap(callback=one)", &children)).unwrap();
    let error = refusal(&root("Wrap(callback=two)", &children));
    assert!(error.related[1].note.contains("two"));
    assert!(error.message.contains("Wrap.callback"));
    refusal(&root("Wrap(callback=zero)", &children));
    let reversed = format!("{BUTTON}component Wrap\n  props\n    callback: action\n  view\n    Button(callback=callback(1))\n");
    contract::compile(&root("Wrap(callback=one)", &reversed)).unwrap();
}

#[test]
fn incompatible_invocations_are_rejected_even_in_an_unused_component() {
    let children = "component Unused\n  props\n    callback: action\n  view\n    view\n      button \"a\" press=callback()\n      button \"b\" press=callback(1)\n";
    let error = refusal(&root("text \"hello\"", children));
    assert!(error.message.contains("incompatible"));
    assert!(error.related[1].note.contains("invocation"));
}

#[test]
fn injected_requirements_cross_wrappers_and_use_the_nearest_provider() {
    let children = "component Wrap\n  view\n    Injected()\ncomponent Injected\n  inject\n    callback: action\n  view\n    button \"run\" press=callback()\n";
    refusal(&root("provide callback = one\n      Wrap()", children));
    contract::compile(&root("provide callback = zero\n      Wrap()", children)).unwrap();
    contract::compile(&root(
        "provide callback = one\n      provide callback = zero\n        Wrap()",
        children,
    ))
    .unwrap();
    let error = refusal(&root(
        "provide callback = zero\n      provide callback = one\n        Wrap()",
        children,
    ));
    assert_eq!(error.related[1].span.line, 11);
}

#[test]
fn providers_can_forward_action_props_and_curry_them() {
    let children = "component Wrap\n  props\n    callback: action\n  view\n    provide callback = callback(1)\n      Injected()\ncomponent Injected\n  inject\n    callback: action\n  view\n    button \"run\" press=callback()\n";
    contract::compile(&root("Wrap(callback=one)", children)).unwrap();
    refusal(&root("Wrap(callback=two)", children));
    let children =
        "component Wrap\n  inject\n    callback: action\n  view\n    Button(callback=callback(1))\n"
            .to_owned() + BUTTON;
    contract::compile(&root("provide callback = one\n      Wrap()", &children)).unwrap();
    refusal(&root("provide callback = two\n      Wrap()", &children));
}

#[test]
fn slot_fills_keep_the_callers_provider_context() {
    let children = "component Slot\n  slot\n  action local\n    focus(\"nothing\")\n  view\n    provide callback = local\n      children\ncomponent Injected\n  inject\n    callback: action\n  view\n    button \"run\" press=callback()\n";
    // The slot's inner provider must not rescue the caller's mismatched fill.
    refusal(&root(
        "provide callback = one\n      Slot()\n        Injected()",
        children,
    ));
    contract::compile(&root(
        "provide callback = zero\n      Slot()\n        Injected()",
        children,
    ))
    .unwrap();
}

#[test]
fn navigate_optional_arity_survives_multiple_bindings_and_intersection() {
    let nav = "component Page\n  props\n    callback: action\n  view\n    view navigationKey=\"home\" navigationBack=\"back\" navigate=callback\n";
    let zero = "component App\n  action zero\n    focus(\"nothing\")\n  action one(path: string)\n    focus(path)\n  view\n    view\n      Page(callback=zero)\n      Page(callback=one)\n";
    // Two navigation roots cannot both lower into one app; inspect the shared
    // interface with both bindings, then compile each legal root separately.
    let file = contract_syntax::parse(&(zero.to_owned() + nav)).unwrap();
    let checked = contract_types::check(&file, contract_lower::tags::style).unwrap();
    contract_analyze::check(&checked).unwrap();
    for action in ["zero", "one"] {
        let source = zero.replace(
            "    view\n      Page(callback=zero)\n      Page(callback=one)",
            &format!("    Page(callback={action})"),
        ) + nav;
        contract::compile(&source).unwrap();
    }
    let exact = nav.replace(
        "    view navigationKey=\"home\" navigationBack=\"back\" navigate=callback",
        "    view navigationKey=\"home\" navigationBack=\"back\" navigate=callback\n      button \"run\" press=callback()",
    );
    refusal(&(zero.to_owned() + &exact));
    let invalid = nav.replace("navigate=callback", "navigate=callback(\"/bad\")");
    refusal(&(zero.to_owned() + &invalid));
}

#[test]
fn all_event_payload_arities_share_the_lowering_rule() {
    for (event, count) in [
        ("press", 0),
        ("change", 1),
        ("hover", 1),
        ("timeupdate", 1),
        ("durationchange", 1),
        ("pan", 2),
        ("scroll", 2),
        ("heightrelease", 2),
        ("transformgeometry", 4),
        ("transformrelease", 6),
        ("reorderdrop", 2),
        ("reachstart", 0),
        ("reachend", 0),
    ] {
        assert_eq!(
            contract_analyze::handler_arity(event, 0),
            Some(count..=count)
        );
        let children = format!("component Unused\n  props\n    callback: action\n  view\n    view {event}=callback\n      button \"match\" press=callback({})\n", (0..count).map(|_| "1").collect::<Vec<_>>().join(", "));
        // This component is uninstantiated, so only interface analysis sees its
        // handlers; its two demands agree before lowering specializes events.
        contract::compile(&root("text \"hello\"", &children)).unwrap();
        let wrong = children.replace(
            "button \"match\" press=callback(",
            "button \"match\" press=callback(1, ",
        );
        if count > 0 {
            refusal(&root("text \"hello\"", &wrong));
        }
    }
    // Edges carry no payload: their action takes exactly the bound arguments.
    assert_eq!(
        contract_analyze::handler_arity("reachstart", 1),
        Some(1..=1)
    );
    assert_eq!(contract_analyze::handler_arity("refresh", 2), Some(2..=2));
    assert!(contract_analyze::handler_arity("navigate", 1).is_none());
}

struct App(PathBuf);
impl App {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("exact-related-imports-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for App {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn related_import_locations_resolve_independently_and_cli_prints_them() {
    let app = App::new();
    let root = app.write(
        "app.contract",
        &root("Wrap(callback=one)", "").replacen(
            "component App",
            "use Wrap from \"./wrap.contract\"\ncomponent App",
            1,
        ),
    );
    app.write("wrap.contract", "use Button from \"./button.contract\"\ncomponent Wrap\n  props\n    callback: action\n  view\n    Button(callback=callback)\n");
    let child = app.write("button.contract", BUTTON).canonicalize().unwrap();
    let error = contract::compile_path(&root).unwrap_err();
    assert_eq!(error.id, "analyze-action-arity");
    assert_eq!(error.file.as_ref(), Some(&child));
    assert_eq!(error.related[0].file.as_ref(), Some(&child));
    assert_eq!(error.related[1].file.as_ref(), Some(&root));
    assert_eq!(
        error.related[2].file.as_ref(),
        Some(&app.0.join("wrap.contract").canonicalize().unwrap())
    );
    assert!(error.related[2].note.contains("forwarded"));
    let output = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["build", root.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json[0]["file"], child.to_str().unwrap());
    assert_eq!(json[0]["related"][1]["file"], root.to_str().unwrap());
    let human = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["build", root.to_str().unwrap()])
        .output()
        .unwrap();
    let text = String::from_utf8(human.stderr).unwrap();
    assert!(text.contains("action interface declared here"));
    assert!(text.contains("bound to `one` here"));
}

#[test]
fn every_binding_is_checked_and_valid_lifted_child_actions_keep_their_arity() {
    refusal(&root(
        "view\n      Button(callback=zero)\n      Button(callback=one)",
        BUTTON,
    ));
    let children = format!("component Stateful\n  props\n    value: number\n  state n = 0\n  action local writes n\n    n = value\n  view\n    Button(callback=local)\n{BUTTON}");
    contract::compile(&root("Stateful(value=7)", &children)).unwrap();
    let invalid = children.replace(
        "action local writes n",
        "action local(extra: number) writes n",
    );
    refusal(&root("Stateful(value=7)", &invalid));
}

#[test]
fn unused_action_parameters_do_not_acquire_an_invented_arity() {
    let child = "component Idle\n  props\n    callback: action\n  view\n    text \"idle\"\n";
    contract::compile(&root(
        "view\n      Idle(callback=zero)\n      Idle(callback=one)\n      Idle(callback=two)",
        child,
    ))
    .unwrap();
}
