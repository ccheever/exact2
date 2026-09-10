use exact_plan::{Opcode, Plan, RegionKind};

fn compile(source: &str) -> Result<Plan, String> {
    let file = contract_syntax::parse(source).map_err(|e| e.to_string())?;
    let types = contract_types::check(&file).map_err(|e| e.to_string())?;
    let analysis = contract_analyze::check(&file, &types).map_err(|e| e.to_string())?;
    contract_lower::lower(&file, &types, &analysis, None).map_err(|e| e.to_string())
}

const CHILD: &str = r#"component App
  view
    column
      Child(id="a")
      Child(id="b")
component Child
  props
    id: string
  state n = 0
  resource value = read(id) as shape string else id
  mutation saved as shape string
  action tick writes n, saved
    n = n + 1
    refresh value
    send saved = save(id)
  task poll mount
    every(100, tick)
  view
    column
      text value
      text `${pending(value)}/${pending(saved)}`
"#;

#[test]
fn child_declarations_are_owned_and_timer_captures_are_passed() {
    let plan = compile(CHILD).unwrap();
    assert_eq!(plan.regions.len(), 2);
    assert!(plan.regions.iter().all(|r| r.kind == RegionKind::Scope));
    for i in 0..2 {
        let owner = plan.resources[i].owner.unwrap();
        assert_eq!(plan.slots[i].owner, Some(owner));
        assert_eq!(plan.slot(plan.mutations[i].slot).owner, Some(owner));
        assert_eq!(plan.timers[i].owner, Some(owner));
        assert_eq!(plan.timers[i].args.len, 1);
        assert_ne!(plan.resources[i].fallback.len, 0);
    }
    assert_eq!(plan.sources.len(), 2);
    assert_eq!(Plan::decode(&plan.encode()).unwrap(), plan);
    assert_eq!(compile(CHILD).unwrap().encode(), plan.encode());
}

#[test]
fn resource_fallbacks_are_typed_and_cycles_refused() {
    compile("component App\n  resource values = read() as shape list<string> else []\n  view\n    text \"values\"\n").unwrap();
    let bad = compile("component App\n  resource value = read() as shape string else 42\n  view\n    text value\n").unwrap_err();
    assert!(bad.contains("type-resource-fallback"), "{bad}");
    for declarations in [
        "  resource value = read() as shape string else value\n",
        "  resource value = read() as shape string else other\n  resource other = read() as shape string else value\n",
        "  resource value = read(other) as shape string else \"\"\n  derive other = value\n",
    ] {
        let error = compile(&format!("component App\n{declarations}  view\n    text value\n")).unwrap_err();
        assert!(error.contains("type-resource-cycle"), "{error}");
    }
}

#[test]
fn child_source_signatures_are_unified() {
    let source = CHILD.replace("send saved = save(id)", "send saved = read(1)");
    assert!(compile(&source)
        .unwrap_err()
        .contains("type-source-signature"));
}

#[test]
fn nested_owned_arguments_include_scope_and_when_frames() {
    let source = r#"component App
  resource ids = ids() as shape list<string>
  view
    column
      each outer in ids key=outer
        when true
          Child(id=outer)
component Child
  props
    id: string
  resource value = read(id) as shape string else id
  view
    text value
"#;
    let plan = compile(source).unwrap();
    let arg = plan.arg(plan.resources[1].args.iter().next().unwrap()).expr;
    let bytes = &plan.code[arg.offset as usize..(arg.offset + arg.len) as usize];
    assert_eq!(bytes[0], Opcode::LoadItem as u8);
    assert_eq!(u16::from_le_bytes([bytes[1], bytes[2]]), 2);
}

#[test]
fn a_stateful_child_can_be_the_single_root_and_pure_children_stay_inline() {
    let source = "component App\n  view\n    Counter()\ncomponent Counter\n  state n = 1\n  view\n    text `${n}`\n";
    let plan = compile(source).unwrap();
    assert_eq!(plan.regions[0].kind, RegionKind::Scope);
    let pure = source
        .replace("  state n = 1\n", "")
        .replace("`${n}`", "\"pure\"");
    assert!(compile(&pure).unwrap().regions.is_empty());
}

#[test]
fn forwarded_slots_expand_at_each_insertion_with_live_providers() {
    let source = r#"component App
  state label = "caller"
  view
    column
      Shell()
        Leaf(label=label)
component Shell
  slot
  state n = 0
  view
    column
      provide token = "inside"
        Relay()
          children
component Relay
  slot
  view
    column
      children
      children
component Leaf
  props
    label: string
  inject
    token: string
  resource value = read(label, token) as shape string else token
  view
    text value
"#;
    let plan = compile(source).unwrap();
    assert_eq!(
        plan.resources.len(),
        2,
        "each insertion owns independent declarations"
    );
    assert_ne!(plan.resources[0].owner, plan.resources[1].owner);
    for resource in &plan.resources {
        let code = plan.arg(resource.args.iter().nth(1).unwrap()).expr;
        let bytes = &plan.code[code.offset as usize..(code.offset + code.len) as usize];
        assert_eq!(bytes[0], Opcode::Str as u8);
        let string = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
        assert_eq!(plan.str(exact_plan::StrId(string)), "inside");
    }
}

#[test]
fn owned_state_can_control_nested_scope_and_resource_arguments() {
    let source = r#"component App
  view
    Holder()
component Holder
  state selected = some("a")
  state rows = ["a", "b"]
  view
    column
      match selected
        case some(id)
          each item in rows key=item
            Leaf(id=id, item=item)
        case none
          text "empty"
component Leaf
  props
    id: string
    item: string
  resource value = read(id, item) as shape string else `${id}/${item}`
  state n = 1
  state next = n + 1
  view
    text `${value}/${next}`
"#;
    let plan = compile(source).unwrap();
    let resource = &plan.resources[0];
    let codes: Vec<_> = resource
        .args
        .iter()
        .map(|id| {
            let c = plan.arg(id).expr;
            plan.code[c.offset as usize..(c.offset + c.len) as usize].to_vec()
        })
        .collect();
    assert_eq!(codes[0][0], Opcode::LoadBound as u8);
    assert_eq!(u16::from_le_bytes(codes[0][1..3].try_into().unwrap()), 2);
    assert_eq!(codes[1][0], Opcode::LoadItem as u8);
    assert_eq!(u16::from_le_bytes(codes[1][1..3].try_into().unwrap()), 1);
}

#[test]
fn text_runs_allow_nonvisual_owned_scopes() {
    compile("component App\n  view\n    text\n      Run()\ncomponent Run\n  state n = 0\n  view\n    text `${n}`\n").unwrap();
}

#[test]
fn compiler_reject_corpus_keeps_its_diagnostics() {
    let mut failures = Vec::new();
    for case in include_str!("../../corpus/rejects.txt").split("\n---\n") {
        let Some(rest) = case.trim().strip_prefix("== ") else {
            continue;
        };
        let (id, source) = rest.split_once('\n').unwrap();
        // Module resolution belongs to the CLI, outside these compiler passes.
        if id == "contract-use-unresolved" {
            continue;
        }
        let source = source
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        match compile(&source) {
            Err(error) if error.contains(&format!("[{id}]")) => {}
            Err(error) => failures.push(format!("expected {id}, got {error}")),
            Ok(_) => failures.push(format!("expected {id}, compiled")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn resource_child_corpus_is_accepted() {
    compile(include_str!("../../corpus/component-resource.contract")).unwrap();
}
