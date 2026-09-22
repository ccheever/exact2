use exact_kernel::Kernel;
use exact_runner::{DataError, DataSource, Runner, Value};
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected {name}")
    }
    fn ready(&self) -> bool {
        false
    }
}
const SOURCE: &str = include_str!("../../corpus/surface.contract");
#[test]
fn bake_defaults_are_the_first_frame_and_never_kept_or_typescript() {
    let p = contract::bake(contract::compile(SOURCE).unwrap(), NoData).unwrap();
    assert_eq!(
        Value::from_bytes(p.bytes(p.resources[0].initial)).unwrap(),
        Value::record(vec![Value::Number(0.)])
    );
    assert!(p.resources[0].reader);
    assert!(!contract::typescript(&p).unwrap().contains("exactSurface"));
    let mut r = Runner::boot(p, NoData, Kernel::with_monospace(), Default::default(), "/").unwrap();
    r.set_surface_record("world", Some(r#"{"beacons":1}"#))
        .unwrap();
    assert_eq!(
        r.resource("hud"),
        Some(&Value::record(vec![Value::Number(1.)]))
    );
    r.set_surface_record("status", Some(r#"{"ready":true}"#))
        .unwrap();
    assert_eq!(
        r.resource("status"),
        Some(&Value::record(vec![Value::Bool(true)]))
    );
    assert_eq!(
        r.resource("same"),
        Some(&Value::record(vec![Value::Bool(false)]))
    );
    r.set_surface_record("world", Some(r#"{"beacons":2,"ready":true}"#))
        .unwrap();
    assert_eq!(
        r.resource("same"),
        Some(&Value::record(vec![Value::Bool(true)]))
    );
    assert!(r.take_store_writes().is_empty());
}
#[test]
fn lint_refuses_arity_nonliteral_nonrecord_and_unknown_surface_by_name() {
    let cases = [
        SOURCE.replace("exactSurface(\"world\")", "exactSurface()"),
        SOURCE.replace(
            "exactSurface(\"world\")",
            "exactSurface(\"world\", \"other\")",
        ),
        SOURCE.replace("exactSurface(\"world\")", "exactSurface(12)"),
        SOURCE
            .replace(
                "  resource hud",
                "  state surfaceName = \"world\"\n  resource hud",
            )
            .replace("exactSurface(\"world\")", "exactSurface(surfaceName)"),
        SOURCE.replace("exactSurface(\"world\")", "exactSurface(\"missing\")"),
    ];
    let mut nonrecord = contract::compile(SOURCE).unwrap();
    let number = nonrecord.fields[0].ty;
    nonrecord.resources[0].ty = number;
    assert!(matches!(
        contract::bake(nonrecord, NoData),
        Err(contract::BakeError::Lint {
            id: "bake-surface-record",
            ..
        })
    ));
    for source in cases {
        let p = contract::compile(&source).unwrap();
        let error = contract::bake(p, NoData).unwrap_err();
        assert!(
            matches!(
                error,
                contract::BakeError::Lint {
                    id: "bake-surface-record",
                    ..
                }
            ),
            "{error:?}"
        );
        assert!(error.to_string().contains("hud"));
    }
}

#[test]
fn viewport_and_delivery_readers_choose_subsets_without_source_unification() {
    for (source, fields) in [
        ("exactViewport", ["width: number", "height: number"]),
        (
            "exactDelivery",
            ["stream: string", "compatibilityId: string"],
        ),
    ] {
        let text = format!("shape A\n  {}\nshape B\n  {}\ncomponent App\n  resource a = {source}() as shape A\n  resource b = {source}() as shape B\n  view\n    text \"facts\"\n", fields[0], fields[1]);
        let p = contract::compile(&text).unwrap();
        // Named once in the plan's source table (LLP 1030 D7), never unified.
        assert_eq!(p.sources.len(), 1);
        contract::bake(p, NoData).unwrap();
    }
}

#[test]
fn control_action_is_a_canvas_child_property() {
    let source = "component Controls\n  view\n    canvas\n      button action=\"jump\" testId=\"jump\"\n        text \"Jump\"\n";
    let plan = contract::compile(source).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(exact_runner::agent::tree(&r).contains(r#""action":"jump""#));
    let outside = source
        .replace("    canvas\n", "")
        .replace("      button", "    button")
        .replace("        text", "      text");
    assert!(contract::compile(&outside)
        .unwrap_err()
        .to_string()
        .contains("analyze-control-parent"));
    assert!(
        contract::compile(&source.replace("button action", "view action"))
            .unwrap_err()
            .to_string()
            .contains("analyze-control-parent")
    );
}

#[test]
fn named_surface_arguments_survive_plan_roundtrip_and_live_updates() {
    let source = r#"component Named
  state paused = false
  state again = false
  action pause writes paused
    paused = not paused
  view
    canvas surface=world(restart=again, seed=min(9, 7), paused=paused)
"#;
    let compiled = contract::compile(source).unwrap();
    let plan = exact_plan::Plan::decode(&compiled.encode()).unwrap();
    assert_eq!(compiled.encode(), plan.encode());
    let mut runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let first = runner.take_surface_updates().pop().unwrap();
    assert_eq!(first.names, ["restart", "seed", "paused"]);
    assert_eq!(
        first.arguments_json(),
        r#"{"restart":false,"seed":7,"paused":false}"#
    );
    runner.act("pause", vec![]).unwrap();
    let next = runner.take_surface_updates().pop().unwrap();
    assert_eq!(next.view, first.view);
    assert_eq!(
        next.arguments_json(),
        r#"{"restart":false,"seed":7,"paused":true}"#
    );
    runner.advance(100.).unwrap();
    assert!(runner.take_surface_updates().is_empty());
    for (call, diagnostic) in [
        ("world(seed=7, seed=8)", "duplicate surface argument `seed`"),
        (
            "world(7, paused=false)",
            "either named or positional surface arguments (`paused` is named)",
        ),
        (
            "world(seed=7, false)",
            "either named or positional surface arguments (`seed` is named)",
        ),
        ("world(seed=missing)", "unknown name `missing`"),
        ("world(seed=min(a=1, b=2))", "named arguments belong"),
    ] {
        let bad = source.replace("world(restart=again, seed=min(9, 7), paused=paused)", call);
        let error = contract::compile(&bad).unwrap_err().to_string();
        assert!(error.contains(diagnostic), "{error}");
    }
    let child = "component App\n  state seed = 11\n  view\n    Scene(seed=seed)\ncomponent Scene\n  props\n    seed: number\n  view\n    canvas surface=world(seed=seed)\n";
    let plan = contract::compile(child).unwrap();
    let mut inlined = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(
        inlined.take_surface_updates()[0].arguments_json(),
        r#"{"seed":11}"#
    );
    let mut signed = first;
    signed.values[1] = Value::Number(-0.0);
    assert!(
        signed.arguments_json().contains("\"seed\":-0"),
        "binding transport preserves signed zero"
    );
    let mut bad = compiled;
    let args = bad.surfaces[0].args;
    let first = args.iter().next().unwrap().0 as usize;
    bad.surface_args[first + 1].name = bad.surface_args[first].name;
    assert!(bad.validate().is_err());
    assert!(
        exact_plan::Plan::decode(&bad.encode()).is_err(),
        "decoder also refuses duplicate argument names"
    );
}

#[test]
fn d6_empty_v5_surface_call_is_named_and_v4_refuses() {
    let plan = contract::compile("component App\n  view\n    canvas surface=world()\n").unwrap();
    assert_eq!(plan.surfaces[0].mode, exact_plan::SurfaceArgsMode::Named);
    let plan = exact_plan::Plan::decode(&plan.encode()).unwrap();
    let mut runner = Runner::boot(
        plan.clone(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(runner.take_surface_updates()[0].arguments_json(), "{}");
    let mut old = plan.encode();
    old[4..8].copy_from_slice(&4u32.to_le_bytes());
    assert!(matches!(
        exact_plan::Plan::decode(&old),
        Err(exact_plan::PlanError::UnsupportedVersion(4))
    ));
}
#[test]
fn r13_colon_call_refuses_with_equals_hint() {
    let error = contract::compile("component App\n  view\n    canvas surface=world(seed: 7)\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("seed="), "{error}");
}

#[test]
fn file_compilation_checks_surface_arguments_at_the_source() {
    let dir = std::env::temp_dir().join(format!("contract-surfaces-{}", std::process::id()));
    std::fs::create_dir_all(dir.join(".shells")).unwrap();
    let declaration = dir.join(".shells/surfaces.json");
    std::fs::write(
        &declaration,
        r#"{"world":[{"name":"seed"},{"name":"paused"},{"name":"restart"}]}"#,
    )
    .unwrap();
    let path = dir.join("app.contract");
    for (call, expected) in [
        ("world(sead=7)", "sead"),
        ("world(1, false, false, 4)", "got 4"),
        ("missing()", "unknown surface"),
    ] {
        let source = format!("component App\n  view\n    canvas surface={call}\n");
        let error = contract::compile_path_source(&path, &source).unwrap_err();
        assert_eq!(error.id, "analyze-surface-arguments");
        assert!(error.message.contains(expected), "{error}");
        assert_eq!(error.file.as_deref(), Some(path.as_path()));
        assert_eq!(error.span.line, 3);
        let marked = &source.lines().nth(2).unwrap()
            [error.span.col as usize - 1..error.span.end_col as usize - 1];
        assert_eq!(
            marked,
            if expected == "sead" {
                "sead"
            } else {
                call.split('(').next().unwrap()
            }
        );
    }
    for call in [
        "world()",
        "world(7)",
        "world(seed=7)",
        "world(paused=true, seed=7)",
    ] {
        let source = format!("component App\n  view\n    canvas surface={call}\n");
        let checked = contract::compile_path_source(&path, &source).unwrap();
        assert_eq!(
            checked.encode(),
            contract::compile(&source).unwrap().encode(),
            "validation leaves plan bytes unchanged"
        );
    }
    let imported = dir.join("child.contract");
    std::fs::write(
        &imported,
        "component Child\n  view\n    canvas surface=world(typo=7)\n",
    )
    .unwrap();
    let source = "use Child from \"./child.contract\"\ncomponent App\n  view\n    Child()\n";
    let error = contract::compile_path_source(&path, source).unwrap_err();
    let diagnostic: serde_json::Value = serde_json::from_str(&error.to_json()).unwrap();
    assert_eq!(diagnostic["file"], imported.to_str().unwrap());
    assert_eq!(diagnostic["line"], 3);
    assert_eq!(error.id, "analyze-surface-arguments");

    for malformed in ["{", r#"{"world":[{"default":0}]}"#, "[]"] {
        std::fs::write(&declaration, malformed).unwrap();
        let error = contract::compile_path_source(&path, source).unwrap_err();
        assert_eq!(error.id, "analyze-surface-declaration");
        assert_eq!(error.file.as_deref(), Some(declaration.as_path()));
    }
    std::fs::remove_file(&declaration).unwrap();
    assert!(
        contract::compile_path_source(&path, source).is_ok(),
        "a plain surface without an emitted interface retains runtime binding"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn r13_surface_call_corpus_fixtures_roundtrip() {
    for source in [
        include_str!("../../corpus/surface-named.contract"),
        include_str!("../../corpus/surface-empty.contract"),
        include_str!("../../corpus/surface-short.contract"),
    ] {
        let p = contract::compile(source).unwrap();
        assert_eq!(
            exact_plan::Plan::decode(&p.encode()).unwrap().encode(),
            p.encode()
        );
    }
}
